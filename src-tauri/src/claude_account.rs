//! Account allowance reader using the installed Claude Code sign-in.
//! The account usage endpoint is a Claude-client compatibility integration,
//! not an advertised public API. Never perform inference or save credentials.
use crate::{
    core::{unix_now, DataCard},
    providers::{usage_card, UsageWindow},
};
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

fn config_dir(profile: &str) -> Result<PathBuf, String> {
    if !profile.is_empty() {
        let path = PathBuf::from(profile);
        if !path.is_absolute() {
            return Err("Choose an absolute Claude profile folder".into());
        }
        return Ok(path);
    }
    if let Some(path) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(path));
    }
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(|p| PathBuf::from(p).join(".claude"))
        .ok_or("Home directory unavailable".into())
}
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn credential_json(profile: &str) -> Result<Value, String> {
    let directory = config_dir(profile)?;
    #[cfg(target_os = "macos")]
    {
        let user = std::env::var("USER").map_err(|_| "User identity unavailable")?;
        let suffix = if !profile.is_empty() || std::env::var_os("CLAUDE_CONFIG_DIR").is_some() {
            format!(
                "-{}",
                &format!(
                    "{:x}",
                    Sha256::digest(directory.to_string_lossy().as_bytes())
                )[..8]
            )
        } else {
            String::new()
        };
        let service = format!("Claude Code-credentials{suffix}");
        let mut child = ChildGuard(
            Command::new("/usr/bin/security")
                .args(["find-generic-password", "-s", &service, "-a", &user, "-w"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| "Claude sign-in could not be read")?,
        );
        let stdout = child.0.stdout.take().ok_or("Credential read stopped")?;
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout.take(65537).read_to_end(&mut bytes).map(|_| bytes);
            let _ = tx.send(result);
        });
        if let Ok(Ok(bytes)) = rx.recv_timeout(Duration::from_secs(5)) {
            if bytes.len() <= 65536 {
                if let Ok(value) = serde_json::from_slice(&bytes) {
                    return Ok(value);
                }
            }
        }
    }
    let path = directory.join(".credentials.json");
    if fs::metadata(&path)
        .map(|m| m.len() <= 65536)
        .unwrap_or(false)
    {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(value) = serde_json::from_slice(&bytes) {
                return Ok(value);
            }
        }
    }
    Err("Connect your Claude account in Edit. DroidFun App uses your local Claude Code sign-in to read the shared account allowance.".into())
}
fn stored_token(profile: &str) -> Result<String, String> {
    let credentials = credential_json(profile)?;
    let auth = &credentials["claudeAiOauth"];
    let token = auth["accessToken"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 16384)
        .ok_or(
            "Your local Claude sign-in needs reconnecting. Click Connect Claude account in Edit.",
        )?;
    if auth["expiresAt"]
        .as_u64()
        .is_some_and(|t| t > 0 && t / 1000 <= unix_now())
    {
        return Err(
            "Your local Claude sign-in expired. Click Connect Claude account in Edit.".into(),
        );
    }
    Ok(token.into())
}

fn token(profile: &str) -> Result<String, String> {
    match stored_token(profile) {
        Ok(token) => Ok(token),
        Err(error) if error.contains("sign-in expired") => {
            refresh_with_installed_client(profile)?;
            stored_token(profile)
        }
        Err(error) => Err(error),
    }
}

// Let the credential owner renew its own token. Never implement another
// refresh-token writer or send an inference prompt. Only a previously trusted,
// DroidFun App-owned scratch folder is eligible; no trust prompt is answered here.
#[cfg(unix)]
fn refresh_with_installed_client(profile: &str) -> Result<(), String> {
    use std::sync::Mutex;
    static REFRESH: Mutex<()> = Mutex::new(());
    let _lock = crate::network::lock_cancellable(&REFRESH)?;
    if stored_token(profile).is_ok() {
        return Ok(());
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("Home unavailable")?;
    let scratch = home.join(".cache/deskbot/claude-usage-check");
    let metadata = if profile.is_empty() {
        home.join(".claude.json")
    } else {
        config_dir(profile)?.join(".claude.json")
    };
    let trusted = fs::metadata(&metadata).is_ok_and(|m| m.len() <= 4 * 1024 * 1024)
        && fs::read(&metadata)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .is_some_and(|v| {
                v["projects"][scratch.to_string_lossy().as_ref()]["hasTrustDialogAccepted"] == true
            });
    if !trusted || !scratch.is_dir() {
        return Err("Your local Claude sign-in expired. Open Claude Code to refresh it, or use Connect Claude account in Edit.".into());
    }
    refresh_client_terminal(
        profile,
        &scratch,
        &executable()?,
        || stored_token(profile).is_ok(),
        Duration::from_secs(20),
    )
}

#[cfg(unix)]
fn refresh_client_terminal(
    profile: &str,
    scratch: &std::path::Path,
    executable: &std::path::Path,
    refreshed: impl Fn() -> bool,
    timeout: Duration,
) -> Result<(), String> {
    use std::{
        os::fd::{AsRawFd, FromRawFd},
        os::unix::process::CommandExt,
    };
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    } != 0
    {
        return Err("Could not start the installed Claude credential refresh".into());
    }
    let mut reader = unsafe { fs::File::from_raw_fd(master) };
    let terminal = unsafe { fs::File::from_raw_fd(slave) };
    unsafe {
        libc::fcntl(reader.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK);
    }
    let mut command = Command::new(executable);
    command
        .args([
            "--safe-mode",
            "--permission-mode",
            "plan",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
        ])
        .current_dir(scratch)
        .env("DISABLE_AUTOUPDATER", "1")
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        .env("TERM", "xterm-256color")
        .stdin(
            terminal
                .try_clone()
                .map_err(|_| "Could not open refresh terminal")?,
        )
        .stdout(
            terminal
                .try_clone()
                .map_err(|_| "Could not open refresh terminal")?,
        )
        .stderr(terminal);
    if !profile.is_empty() {
        command.env("CLAUDE_CONFIG_DIR", config_dir(profile)?);
    }
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = ChildGuard(
        command
            .spawn()
            .map_err(|_| "Installed Claude credential refresh could not start")?,
    );
    struct SessionGuard(i32);
    impl Drop for SessionGuard {
        fn drop(&mut self) {
            unsafe {
                libc::kill(-self.0, libc::SIGKILL);
            }
        }
    }
    // This child created its own session above. Reap its helpers as well as
    // the terminal client, without touching any existing Claude session.
    let _session = SessionGuard(child.0.id() as i32);
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let mut discarded = [0; 4096];
        // Bound draining too, so an unexpectedly chatty client cannot stall
        // timeout/cancellation. Never persist terminal contents.
        for _ in 0..16 {
            if !matches!(reader.read(&mut discarded), Ok(n) if n > 0) {
                break;
            }
        }
        if crate::network::canceled() {
            return Err("Request canceled because the source changed".into());
        }
        if refreshed() {
            return Ok(());
        }
        if child
            .0
            .try_wait()
            .map_err(|_| "Credential refresh stopped")?
            .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err("Your local Claude sign-in needs reconnecting. Open Claude Code or use Connect Claude account in Edit.".into())
}

#[cfg(not(unix))]
fn refresh_with_installed_client(_profile: &str) -> Result<(), String> {
    Err("Your local Claude sign-in expired. Open Claude Code to refresh it, or use Connect Claude account in Edit.".into())
}
static CONNECTING: AtomicBool = AtomicBool::new(false);
struct LoginGuard;
impl Drop for LoginGuard {
    fn drop(&mut self) {
        CONNECTING.store(false, Ordering::Release);
    }
}
pub fn status(profile: &str) -> Value {
    match stored_token(profile) {
        Ok(_) => {
            json!({"signedIn":true,"connecting":CONNECTING.load(Ordering::Acquire),"explanation":"A local sign-in is available. Reads the account allowance shared by Claude desktop, web and Code."})
        }
        Err(e) => {
            json!({"signedIn":false,"connecting":CONNECTING.load(Ordering::Acquire),"explanation":e})
        }
    }
}
pub fn normalize(v: &Value, slot: &str, now: u64) -> Result<DataCard, String> {
    let (minutes, label) = match slot {
        "five_hour" => (300, "Claude · five hours"),
        "seven_day" => (10080, "Claude · seven days"),
        _ => return Err("Choose a Claude allowance window".into()),
    };
    let w = &v[slot];
    let used = w["utilization"]
        .as_f64()
        .filter(|n| n.is_finite())
        .ok_or("Claude did not return this account allowance window")?;
    let reset = w["resets_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.timestamp())
        .filter(|t| *t > now as i64)
        .ok_or("Claude allowance reset time was absent or expired")?;
    Ok(usage_card(
        "claude-account",
        "local-claude-account",
        &UsageWindow {
            bucket: "claude-account".into(),
            slot: slot.into(),
            label: label.into(),
            used_percent: used,
            minutes,
            resets_at: reset as u64,
        },
        now,
    ))
}
pub fn read(client: &Client, slot: &str, profile: &str) -> Result<DataCard, String> {
    let token = token(profile)?;
    let value = crate::network::json(
        client,
        "https://api.anthropic.com/api/oauth/usage",
        &[],
        Some(&token),
        &[
            ("anthropic-beta", "oauth-2025-04-20"),
            ("Accept", "application/json"),
        ],
        65536,
    )
    .map_err(|e| {
        if e.contains("HTTP 401") || e.contains("HTTP 403") {
            "Claude rejected the local sign-in. Click Connect Claude account in Edit.".into()
        } else if e.contains("HTTP 429") {
            format!("Claude account usage is rate limited. {e}")
        } else {
            e
        }
    })?;
    let mut card = normalize(&value, slot, unix_now())?;
    // Token changes invalidate the runtime account scope; the token is never persisted.
    card.scope_id = format!("{:x}", Sha256::digest(token.as_bytes()));
    Ok(card)
}
fn executable() -> Result<PathBuf, String> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    if let Some(home) = home {
        let path = PathBuf::from(home).join(if cfg!(windows) {
            ".local/bin/claude.exe"
        } else {
            ".local/bin/claude"
        });
        if path.is_file() {
            return Ok(path);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&path) {
            let p = p.join(if cfg!(windows) {
                "claude.exe"
            } else {
                "claude"
            });
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err("Install Claude Code, then use Connect Claude account to sign in. No model session is needed.".into())
}
pub fn login(profile: &str) -> Result<(), String> {
    if CONNECTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Claude sign-in is already open in your browser".into());
    }
    let _guard = LoginGuard;
    let directory = config_dir(profile)?;
    let mut command = Command::new(executable()?);
    if !profile.is_empty() {
        command.env("CLAUDE_CONFIG_DIR", directory);
    }
    let mut child = ChildGuard(
        command
            .args(["auth", "login", "--claudeai"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Could not open Claude sign-in")?,
    );
    let deadline = Instant::now() + Duration::from_secs(600);
    while Instant::now() < deadline {
        if let Some(result) = child.0.try_wait().map_err(|_| "Claude sign-in stopped")? {
            if result.success() {
                token(profile)?;
                return Ok(());
            }
            return Err(
                "Claude sign-in was not completed. Try Connect Claude account again.".into(),
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err("Claude sign-in timed out. Try Connect Claude account again.".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn credential_refresh_uses_a_terminal_without_sending_input_and_reaps_client() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("deskbot-refresh-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let client = dir.join("client");
        fs::write(&client, "#!/bin/sh\ntest -t 0 && test -t 1 && test -t 2 || exit 2\necho $$ > pid\necho renewed > refreshed\nwhile :; do sleep 1; done\n").unwrap();
        fs::set_permissions(&client, fs::Permissions::from_mode(0o700)).unwrap();
        refresh_client_terminal(
            "",
            &dir,
            &client,
            || dir.join("refreshed").is_file(),
            Duration::from_secs(3),
        )
        .unwrap();
        let pid: i32 = fs::read_to_string(dir.join("pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "refresh client must be reaped"
        );
        fs::write(&client, "#!/bin/sh\nexit 1\n").unwrap();
        assert!(
            refresh_client_terminal("", &dir, &client, || false, Duration::from_secs(1)).is_err()
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn account_windows_are_separate_and_missing_is_not_zero() {
        let v = json!({"five_hour":{"utilization":42.4,"resets_at":"2026-10-12T01:00:00Z"},"seven_day":{"utilization":13,"resets_at":"2026-10-17T01:00:00Z"}});
        let now = 1791763200;
        let d = normalize(&v, "five_hour", now).unwrap();
        assert_eq!(d.headline, "57%");
        assert_eq!(d.footer, "5H ALLOWANCE");
        assert_eq!(normalize(&v, "seven_day", now).unwrap().headline, "87%");
        assert!(normalize(&json!({"five_hour":null}), "five_hour", now).is_err());
        assert!(normalize(&v, "five_hour", now + 2 * 86400).is_err());
    }
}
