use std::os::windows::process::CommandExt;
// Native Windows test fixture, compiled only by the renewal tests. Never shipped.
// No account credentials, external HTTP requests or model service are used.
use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct InputRecord {
    event_type: u16,
    padding: u16,
    event: [u32; 4],
}
#[link(name = "kernel32")]
extern "system" {
    fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
    fn PeekConsoleInputW(
        handle: *mut std::ffi::c_void,
        records: *mut InputRecord,
        length: u32,
        read: *mut u32,
    ) -> i32;
}
fn assert_no_keyboard_input() {
    let mut records = [InputRecord::default(); 32];
    let mut read = 0;
    assert_ne!(
        unsafe {
            PeekConsoleInputW(
                GetStdHandle(-10i32 as u32),
                records.as_mut_ptr(),
                records.len() as u32,
                &mut read,
            )
        },
        0
    );
    assert!(
        records[..read as usize]
            .iter()
            .all(|record| record.event_type != 1),
        "renewal must not send keyboard input or answer a trust dialog"
    );
}
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--owned-child"] || args == ["--unrelated-decoy"] {
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    assert_eq!(
        args,
        [
            "--setting-sources",
            "",
            "--settings",
            "{\"disableAllHooks\":true,\"disableSkillShellExecution\":true}",
            "--permission-mode",
            "plan",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}"
        ]
    );
    assert!(
        io::stdin().is_terminal() && io::stdout().is_terminal() && io::stderr().is_terminal(),
        "real Windows console required"
    );
    assert_eq!(env::var("DISABLE_AUTOUPDATER").unwrap(), "1");
    assert_eq!(env::var("CLAUDE_CODE_SAFE_MODE").unwrap(), "1");
    assert_eq!(
        env::var("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC").unwrap(),
        "1"
    );
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
    ] {
        assert!(
            env::var_os(key).is_none(),
            "inherited authentication override survived"
        );
    }
    let cwd = env::current_dir().unwrap();
    let profile = std::path::PathBuf::from(env::var_os("CLAUDE_CONFIG_DIR").unwrap());
    assert!(profile.join("fixture-profile-only").is_file());
    assert!(!cwd.join(".claude.json").exists());
    let child = Command::new(env::current_exe().unwrap())
        .arg("--owned-child")
        .creation_flags(0x00000008) // DETACHED_PROCESS: job cleanup must cover non-console descendants.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    fs::write(
        cwd.join("pids.tmp"),
        format!("{} {}", std::process::id(), child.id()),
    )
    .unwrap();
    fs::rename(cwd.join("pids.tmp"), cwd.join("owned-pids")).unwrap();
    let mode = fs::read_to_string(cwd.join("scenario")).unwrap();
    if mode == "failure" {
        std::process::exit(13);
    }
    if mode == "success" {
        for _ in 0..4 {
            assert_no_keyboard_input();
            std::thread::sleep(Duration::from_millis(50));
        }
        let expires = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600)
            * 1000;
        fs::write(profile.join(".credentials.json"), format!("{{\"claudeAiOauth\":{{\"accessToken\":\"fixture-renewed\",\"expiresAt\":{expires}}}}}")).unwrap();
    }
    let bytes = vec![b'X'; 16384];
    loop {
        assert_no_keyboard_input();
        if mode == "flood" {
            io::stdout().write_all(&bytes).unwrap();
            io::stdout().flush().unwrap();
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
