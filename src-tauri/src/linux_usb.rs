//! Linux desktop serial access is installed by the native package. The app
//! remains unprivileged; repair can execute only one fixed, root-owned helper.
use serde::Serialize;

pub const PERMISSION_DETAIL: &str =
    "Permission is needed to connect to this robot. Choose Set up robot access.";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessStatus {
    pub supported: bool,
    pub repair_available: bool,
    pub rule_installed: bool,
}

pub fn status() -> AccessStatus {
    #[cfg(target_os = "linux")]
    return native::status();
    #[cfg(not(target_os = "linux"))]
    AccessStatus {
        supported: false,
        repair_available: false,
        rule_installed: false,
    }
}

pub fn setup() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return native::setup();
    #[cfg(not(target_os = "linux"))]
    Err("USB permission setup is available only on Linux.".into())
}

#[cfg(target_os = "linux")]
mod native {
    use super::AccessStatus;
    use std::{
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt},
        path::Path,
        process::{Command, Stdio},
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };
    const HELPER: &str = "/usr/lib/droidfun/setup-robot-access";
    const POLICY: &str = "/usr/share/polkit-1/actions/io.droidfun.usb-access.policy";
    const RULE: &str = "/usr/lib/udev/rules.d/70-droidfun.rules";
    const SOURCE: &str = "/usr/share/droidfun/70-droidfun.rules";
    const PKEXEC: &str = "/usr/bin/pkexec";
    const SHELL: &str = "/usr/bin/sh";
    static BUSY: AtomicBool = AtomicBool::new(false);
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::Release);
        }
    }

    // /lib and /bin can be distro symlinks. Resolve first, then check every
    // component: neither the package helper nor its parent may be user writable.
    fn trusted(path: &Path, executable: bool) -> bool {
        let Ok(path) = fs::canonicalize(path) else {
            return false;
        };
        for (i, part) in path.ancestors().enumerate() {
            let Ok(meta) = fs::symlink_metadata(part) else {
                return false;
            };
            if meta.uid() != 0 || meta.permissions().mode() & 0o022 != 0 {
                return false;
            }
            if i == 0 && (!meta.is_file() || (executable && meta.permissions().mode() & 0o111 == 0))
            {
                return false;
            }
            if i > 0 && !meta.is_dir() {
                return false;
            }
        }
        true
    }
    pub fn status() -> AccessStatus {
        AccessStatus {
            supported: true,
            repair_available: trusted(Path::new(HELPER), false)
                && trusted(Path::new(SHELL), true)
                && trusted(Path::new(PKEXEC), true)
                && trusted(Path::new(POLICY), false)
                && trusted(Path::new(SOURCE), false),
            rule_installed: fs::read(RULE)
                .is_ok_and(|bytes| bytes == include_bytes!("../linux/70-droidfun.rules")),
        }
    }
    fn result(code: Option<i32>) -> Result<(), String> {
        match code {
            Some(0) => Ok(()),
            Some(126) => Err("Setup was cancelled. Robot access was not changed by this request.".into()),
            Some(127) => Err("Administrator approval was not granted. Try again from your desktop session.".into()),
            _ => Err("USB setup could not finish. Reconnect the robot and try again. If this continues, reinstall DroidFun using your software installer.".into()),
        }
    }
    fn run(mut command: Command, budget: Duration) -> Result<(), String> {
        let mut child = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().map_err(|_| "Could not open the administrator approval dialog. Reinstall the current Linux package.".to_string())?;
        let deadline = Instant::now() + budget;
        loop {
            match child.try_wait() {
                Ok(Some(exit)) => return result(exit.code()),
                Ok(None) => {}
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Could not check USB setup. Please try again.".into());
                }
            }
            if Instant::now() >= deadline {
                // Kill only the exact child. If it has already become root, the
                // fixed helper has its own bounded udev calls and will finish;
                // reap it separately without blocking the app or killing others.
                let _ = child.kill();
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Err(
                    "USB setup timed out. Close any pending approval dialog and try again.".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    pub fn setup() -> Result<(), String> {
        if BUSY.swap(true, Ordering::AcqRel) {
            return Err("USB setup is already open.".into());
        }
        let _guard = Guard;
        if !status().repair_available {
            return Err("Install or reinstall the latest DroidFun using your software installer to enable robot access setup.".into());
        }
        let mut command = Command::new(PKEXEC);
        // Fixed interpreter + fixed script only. The policy also binds argv1;
        // no user-supplied command, argument, path, or password input.
        command.arg("--disable-internal-agent").arg(SHELL).arg(HELPER);
        run(command, Duration::from_secs(120))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn linux_usb_untrusted_helper_is_rejected() {
            let root =
                std::env::temp_dir().join(format!("droidfun-permissions-{}", std::process::id()));
            fs::create_dir_all(&root).unwrap();
            let file = root.join("helper");
            fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o777)).unwrap();
            assert!(!trusted(&file, true));
            assert!(!trusted(&root.join("missing"), true));
            fs::remove_dir_all(&root).unwrap();
        }
        #[test]
        fn linux_usb_cancel_denial_failure_and_success_are_distinct() {
            for (code, text) in [
                (0, ""),
                (126, "cancelled"),
                (127, "not granted"),
                (4, "could not finish"),
            ] {
                let mut command = Command::new("/bin/sh");
                command.args(["-c", &format!("exit {code}")]);
                let result = run(command, Duration::from_secs(2));
                if code == 0 {
                    assert!(result.is_ok());
                } else {
                    assert!(result.unwrap_err().contains(text));
                }
            }
        }
        #[test]
        fn linux_usb_approval_wait_is_bounded() {
            let mut command = Command::new("/bin/sleep");
            command.arg("5");
            let start = Instant::now();
            assert!(run(command, Duration::from_millis(100))
                .unwrap_err()
                .contains("timed out"));
            assert!(start.elapsed() < Duration::from_secs(2));
        }
    }
}
