//! Hidden Windows terminal for the installed credential owner. No shell, model
//! prompt, keyboard input, credential writer or workspace-trust approval.
use std::{
    collections::BTreeMap,
    ffi::{c_void, OsStr, OsString},
    mem::{size_of, size_of_val, zeroed},
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Storage::FileSystem::ReadFile,
    System::{
        Console::{ClosePseudoConsole, CreatePseudoConsole, COORD, HPCON},
        JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Pipes::{CreatePipe, PeekNamedPipe},
        Threading::{
            CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
            UpdateProcThreadAttribute, WaitForSingleObject, CREATE_UNICODE_ENVIRONMENT,
            EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_JOB_LIST,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTUPINFOEXW,
        },
    },
};

const ERROR: &str = "Windows could not start automatic Claude sign-in recovery. Use Connect Claude account in Edit.";
const ARGS: &[&str] = &[
    "--safe-mode",
    "--permission-mode",
    "plan",
    "--strict-mcp-config",
    "--mcp-config",
    "{\"mcpServers\":{}}",
];

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Attributes {
    storage: Vec<usize>,
}
impl Attributes {
    fn new() -> Result<Self, String> {
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err(ERROR.into());
        }
        let mut this = Self {
            storage: vec![0; bytes.div_ceil(size_of::<usize>())],
        };
        if unsafe { InitializeProcThreadAttributeList(this.ptr(), 2, 0, &mut bytes) } == 0 {
            // The list is not initialized, so don't run its destructor.
            this.storage.clear();
            return Err(ERROR.into());
        }
        Ok(this)
    }
    fn ptr(&mut self) -> *mut c_void {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if !self.storage.is_empty() {
            unsafe {
                DeleteProcThreadAttributeList(self.ptr());
            }
        }
    }
}
struct Session {
    console: HPCON,
    job: Handle,
    process: Option<Handle>,
    input: Option<Handle>,
    output: Option<Handle>,
}
impl Drop for Session {
    fn drop(&mut self) {
        // Only our private job is terminated. Its non-inheritable handle has
        // KILL_ON_JOB_CLOSE too, so an app exit also cleans up owned descendants.
        unsafe {
            TerminateJobObject(self.job.0, 1);
        }
        if let Some(process) = &self.process {
            unsafe {
                WaitForSingleObject(process.0, 2000);
            }
        }
        self.input.take();
        // Closing output BEFORE ClosePseudoConsole avoids its documented
        // final-frame deadlock on pre-24H2 Windows. No reader thread is left.
        self.output.take();
        unsafe {
            ClosePseudoConsole(self.console);
        }
    }
}

fn wide(value: &OsStr) -> Result<Vec<u16>, String> {
    let mut out: Vec<_> = value.encode_wide().collect();
    if out.contains(&0) {
        return Err(ERROR.into());
    }
    out.push(0);
    Ok(out)
}
// CreateProcess receives a mutable command line, but never a shell program.
// Encode the standard Windows argv quoting rules, including trailing slashes.
fn argument(value: &OsStr) -> Result<Vec<u16>, String> {
    let mut out = vec![b'"' as u16];
    let mut slashes = 0;
    for c in value.encode_wide() {
        if c == 0 {
            return Err(ERROR.into());
        }
        if c == b'\\' as u16 {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            b'\\' as u16,
            slashes * if c == b'"' as u16 { 2 } else { 1 },
        ));
        if c == b'"' as u16 {
            out.push(b'\\' as u16);
        }
        out.push(c);
        slashes = 0;
    }
    out.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
    out.push(b'"' as u16);
    Ok(out)
}
fn environment(profile: Option<&Path>) -> Result<Vec<u16>, String> {
    // Case-insensitive keys and ordering are required for a Windows block.
    let mut values: BTreeMap<String, OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().to_uppercase(), v))
        .collect();
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
    ] {
        values.remove(key);
    }
    for (key, value) in [
        ("DISABLE_AUTOUPDATER", "1"),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
        ("TERM", "xterm-256color"),
    ] {
        values.insert(key.into(), value.into());
    }
    if let Some(profile) = profile {
        values.insert("CLAUDE_CONFIG_DIR".into(), profile.as_os_str().into());
    }
    let mut out = Vec::new();
    for (key, value) in values {
        let mut pair = OsString::from(key);
        pair.push("=");
        pair.push(value);
        out.extend(wide(&pair)?);
    }
    out.push(0);
    Ok(out)
}
fn pipe() -> Result<(Handle, Handle), String> {
    let (mut read, mut write) = (null_mut(), null_mut());
    if unsafe { CreatePipe(&mut read, &mut write, null(), 0) } == 0 {
        return Err(ERROR.into());
    }
    Ok((Handle(read), Handle(write)))
}

pub(crate) fn refresh(
    profile: Option<&Path>,
    scratch: &Path,
    executable: &Path,
    refreshed: impl Fn(Duration) -> bool,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    if crate::network::canceled() {
        return Err("Request canceled because the source changed".into());
    }
    let exe = wide(executable.as_os_str())?;
    let cwd = wide(scratch.as_os_str())?;
    let mut command = argument(executable.as_os_str())?;
    for arg in ARGS {
        command.push(b' ' as u16);
        command.extend(argument(OsStr::new(arg))?);
    }
    command.push(0);
    let env = environment(profile)?;
    let (input_read, input_write) = pipe()?;
    let (output_read, output_write) = pipe()?;
    let job = unsafe { CreateJobObjectW(null(), null()) };
    if job.is_null() {
        return Err(ERROR.into());
    }
    let job = Handle(job);
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of_val(&limits) as u32,
        )
    } == 0
    {
        return Err(ERROR.into());
    }
    let mut console = 0;
    if unsafe {
        CreatePseudoConsole(
            COORD { X: 80, Y: 24 },
            input_read.0,
            output_write.0,
            0,
            &mut console,
        )
    } < 0
    {
        return Err(ERROR.into());
    }
    let mut session = Session {
        console,
        job,
        process: None,
        input: Some(input_write),
        output: Some(output_read),
    };
    let mut attrs = Attributes::new()?;
    let job_handle = session.job.0;
    if unsafe {
        UpdateProcThreadAttribute(
            attrs.ptr(),
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            console as *const c_void,
            size_of::<HPCON>(),
            null_mut(),
            null(),
        )
    } == 0
        || unsafe {
            UpdateProcThreadAttribute(
                attrs.ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                (&job_handle as *const HANDLE).cast(),
                size_of::<HANDLE>(),
                null_mut(),
                null(),
            )
        } == 0
    {
        return Err(ERROR.into());
    }
    let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.lpAttributeList = attrs.ptr();
    let mut process: PROCESS_INFORMATION = unsafe { zeroed() };
    // Atomic job assignment happens before the first thread runs. Handles
    // are not inherited; the pseudoconsole attribute supplies console I/O.
    if unsafe {
        CreateProcessW(
            exe.as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            env.as_ptr().cast(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    } == 0
    {
        return Err(ERROR.into());
    }
    session.process = Some(Handle(process.hProcess));
    let _thread = Handle(process.hThread);
    drop(input_read);
    drop(output_write);
    while Instant::now() < deadline {
        // Bounded draining prevents a chatty client from blocking its startup
        // or starving expiry/cancellation. Terminal output is only discarded.
        for _ in 0..16 {
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    session.output.as_ref().unwrap().0,
                    null_mut(),
                    0,
                    null_mut(),
                    &mut available,
                    null_mut(),
                )
            } == 0
                || available == 0
            {
                break;
            }
            let mut bytes = [0u8; 4096];
            let mut read = 0;
            if unsafe {
                ReadFile(
                    session.output.as_ref().unwrap().0,
                    bytes.as_mut_ptr(),
                    available.min(bytes.len() as u32),
                    &mut read,
                    null_mut(),
                )
            } == 0
                || read == 0
            {
                break;
            }
        }
        if crate::network::canceled() {
            return Err("Request canceled because the source changed".into());
        }
        if refreshed(deadline.saturating_duration_since(Instant::now())) {
            return Ok(());
        }
        match unsafe { WaitForSingleObject(session.process.as_ref().unwrap().0, 0) } {
            WAIT_TIMEOUT => {}
            WAIT_OBJECT_0 => break,
            _ => return Err(ERROR.into()),
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
        );
    }
    Err("Your local Claude sign-in needs reconnecting. Use Connect Claude account in Edit.".into())
}
