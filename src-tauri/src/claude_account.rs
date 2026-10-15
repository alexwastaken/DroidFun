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
    collections::{BTreeMap, VecDeque},
    fs,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

#[cfg(all(test, windows))]
#[path = "claude_windows_tests.rs"]
mod windows_tests;

#[derive(Default)]
struct Diagnostics {
    events: VecDeque<Value>,
    windows: BTreeMap<String, Value>,
}
fn diagnostic_state() -> &'static Mutex<Diagnostics> {
    static STATE: OnceLock<Mutex<Diagnostics>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(Diagnostics::default()))
}
fn event(state: &str) {
    let mut d = diagnostic_state().lock().unwrap_or_else(|e| e.into_inner());
    if d.events.len() >= 32 {
        d.events.pop_front();
    }
    d.events.push_back(json!({"at":unix_now(),"state":state}));
}
// Only fixed state labels and observation times. Never include token contents,
// account identity, allowance amounts, profile paths or exact token expiry.
pub fn diagnostics() -> Value {
    let d = diagnostic_state().lock().unwrap_or_else(|e| e.into_inner());
    json!({"events":d.events,"windows":d.windows})
}

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
fn credential_json(profile: &str, _timeout: Duration) -> Result<Value, String> {
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
        if let Ok(Ok(bytes)) = rx.recv_timeout(_timeout) {
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
    stored_token_with_timeout(profile, Duration::from_secs(5))
}
fn stored_token_with_timeout(profile: &str, timeout: Duration) -> Result<String, String> {
    let credentials = credential_json(profile, timeout)?;
    token_from_credentials(&credentials, unix_now())
}
fn token_from_credentials(credentials: &Value, now: u64) -> Result<String, String> {
    let auth = &credentials["claudeAiOauth"];
    let token = auth["accessToken"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 16384)
        .ok_or(
            "Your local Claude sign-in needs reconnecting. Click Connect Claude account in Edit.",
        )?;
    if auth["expiresAt"]
        .as_u64()
        .is_some_and(|t| t > 0 && t / 1000 <= now)
    {
        return Err(
            "Your local Claude sign-in expired. Click Connect Claude account in Edit.".into(),
        );
    }
    Ok(token.into())
}

fn token(profile: &str) -> Result<String, String> {
    token_with_refresh(
        || stored_token(profile),
        || refresh_with_installed_client(profile, None),
    )
}
fn token_with_refresh(
    mut stored: impl FnMut() -> Result<String, String>,
    mut refresh: impl FnMut() -> Result<(), String>,
) -> Result<String, String> {
    match stored() {
        Ok(token) => Ok(token),
        Err(error) if error.contains("sign-in expired") => {
            event("credential_expired");
            match refresh().and_then(|_| stored()) {
                Ok(token) => {
                    event("credential_renewed");
                    Ok(token)
                }
                Err(error) => {
                    event("renewal_failed_or_unavailable");
                    Err(error)
                }
            }
        }
        Err(error) => Err(error),
    }
}

// Let the credential owner renew its own token. Never implement another
// refresh-token writer or send an inference prompt. No trust prompt is answered
// here; Unix retains its existing pretrusted-folder prerequisite.
fn refresh_with_installed_client(profile: &str, rejected: Option<&str>) -> Result<(), String> {
    static REFRESH: Mutex<()> = Mutex::new(());
    let _lock = crate::network::lock_cancellable(&REFRESH)?;
    if stored_token(profile).is_ok_and(|token| rejected != Some(token.as_str())) {
        return Ok(());
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or("Home unavailable")?;
    let scratch = home.join(".cache/deskbot/claude-usage-check");
    #[cfg(unix)]
    {
        let metadata = if profile.is_empty() && std::env::var_os("CLAUDE_CONFIG_DIR").is_none() {
            home.join(".claude.json")
        } else {
            config_dir(profile)?.join(".claude.json")
        };
        let trusted = fs::metadata(&metadata).is_ok_and(|m| m.len() <= 4 * 1024 * 1024)
            && fs::read(&metadata)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .is_some_and(|v| {
                    v["projects"][scratch.to_string_lossy().as_ref()]["hasTrustDialogAccepted"]
                        == true
                });
        if !trusted || !scratch.is_dir() {
            return Err("Your local Claude sign-in expired. Open Claude Code to refresh it, or use Connect Claude account in Edit.".into());
        }
    }
    // Windows starts the credential owner in an empty app-owned folder. It
    // sends no input and never accepts or persists a workspace-trust decision.
    // Authentication can refresh during client startup; if that client needs
    // interactive setup instead, the bounded attempt reports reconnecting.
    #[cfg(windows)]
    fs::create_dir_all(&scratch).map_err(|_| "Could not prepare Claude sign-in recovery")?;
    let executable = executable()?;
    event("renewal_started");
    refresh_client_terminal(
        profile,
        &scratch,
        &executable,
        |remaining| {
            stored_token_with_timeout(profile, remaining.min(Duration::from_secs(5)))
                .is_ok_and(|token| rejected != Some(token.as_str()))
        },
        Duration::from_secs(20),
    )
}

#[cfg(unix)]
fn refresh_client_terminal(
    profile: &str,
    scratch: &std::path::Path,
    executable: &std::path::Path,
    refreshed: impl Fn(Duration) -> bool,
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
        if refreshed(deadline.saturating_duration_since(Instant::now())) {
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
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(500)),
        );
    }
    Err("Your local Claude sign-in needs reconnecting. Open Claude Code or use Connect Claude account in Edit.".into())
}

#[cfg(windows)]
fn refresh_client_terminal(
    profile: &str,
    scratch: &std::path::Path,
    executable: &std::path::Path,
    refreshed: impl Fn(Duration) -> bool,
    timeout: Duration,
) -> Result<(), String> {
    let directory = if profile.is_empty() {
        None
    } else {
        Some(config_dir(profile)?)
    };
    crate::windows_claude_terminal::refresh(
        directory.as_deref(),
        scratch,
        executable,
        refreshed,
        timeout,
    )
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
        .filter(|n| n.is_finite() && (0.0..=100.0).contains(n))
        .ok_or("Claude did not return this account allowance window")?;
    let reset = match &w["resets_at"] {
        Value::Null => None,
        Value::String(s) => Some(
            chrono::DateTime::parse_from_rfc3339(s)
                .map_err(|_| "Claude returned an invalid allowance reset time")?
                .timestamp(),
        ),
        _ => return Err("Claude returned an invalid allowance reset time".into()),
    };
    if !reset.is_some_and(|t| t > now as i64) {
        // An inactive/reset window is not expired authentication. Only an
        // explicit provider zero may produce 100% remaining. An elapsed or
        // absent reset alongside nonzero usage supplies status, not a current
        // allowance amount. Keep polling and cycling that truthful status.
        let idle = used == 0.0;
        return Ok(DataCard {
            source_id: "claude-account".into(),
            scope_id: "local-claude-account".into(),
            fetched_at: now,
            observed_at: now,
            stale_at: now + 300,
            expires_at: now + 600,
            state: "fresh".into(),
            headline: if idle { "100%" } else { "WAITING" }.into(),
            footer: format!(
                "{} {}",
                if slot == "five_hour" { "5H" } else { "7D" },
                if idle { "ALLOWANCE" } else { "RESET PENDING" }
            ),
            detail: if idle {
                format!("{label} · unused allowance reported · no active reset time returned · Claude desktop + web + Code")
            } else {
                format!("{label} · waiting for a current allowance window; no remaining percentage inferred · Claude desktop + web + Code")
            },
            values: json!({"allowanceState":if idle {"inactive"} else {"reset_pending"},"usedPercent":if idle {Some(0.0)} else {None},"remainingPercent":if idle {Some(100.0)} else {None},"resetsAt":null,"windowDurationMins":minutes,"slot":slot}),
        });
    }
    let mut card = usage_card(
        "claude-account",
        "local-claude-account",
        &UsageWindow {
            bucket: "claude-account".into(),
            slot: slot.into(),
            label: label.into(),
            used_percent: used,
            minutes,
            resets_at: reset.unwrap() as u64,
        },
        now,
    );
    card.values["allowanceState"] = json!("active");
    Ok(card)
}
pub fn read(client: &Client, slot: &str, profile: &str) -> Result<DataCard, String> {
    read_with_recovery(
        client,
        "https://api.anthropic.com/api/oauth/usage",
        slot,
        || token(profile),
        |rejected| refresh_with_installed_client(profile, Some(rejected)),
    )
}
#[cfg(test)]
fn read_with_token(
    client: &Client,
    url: &str,
    slot: &str,
    mut acquire: impl FnMut() -> Result<String, String>,
) -> Result<DataCard, String> {
    read_with_recovery(client, url, slot, &mut acquire, |_| {
        Err("Claude rejected the local sign-in. Click Connect Claude account in Edit.".into())
    })
}
fn read_with_recovery(
    client: &Client,
    url: &str,
    slot: &str,
    mut acquire: impl FnMut() -> Result<String, String>,
    mut renew_rejected: impl FnMut(&str) -> Result<(), String>,
) -> Result<DataCard, String> {
    let mut token = acquire()?;
    let request = |token: &str| {
        crate::network::json(
            client,
            url,
            &[],
            Some(token),
            &[
                ("anthropic-beta", "oauth-2025-04-20"),
                ("Accept", "application/json"),
            ],
            65536,
        )
    };
    let first = request(&token);
    let value = match first {
        Err(e) if e.contains("HTTP 401") => {
            // Expiry can race the HTTP call or another client can rotate the
            // shared sign-in. Re-read through the normal renewal path, then
            // retry once only if it produced a different token.
            event("credential_rejected");
            let mut next = acquire()?;
            if next == token {
                if let Err(e) = renew_rejected(&token) {
                    event("renewal_failed_or_unavailable");
                    return Err(e);
                }
                next = acquire()?;
                if next != token {
                    event("credential_renewed_after_rejection");
                }
            }
            if next == token {
                return Err(
                    "Claude rejected the local sign-in. Click Connect Claude account in Edit."
                        .into(),
                );
            }
            token = next;
            request(&token)
        }
        other => other,
    }
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
    let state = card.values["allowanceState"].as_str().unwrap_or("unknown");
    diagnostic_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .windows
        .insert(
            slot.into(),
            json!({"state":state,"fetchedAt":card.fetched_at}),
        );
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
    #[test]
    #[ignore = "Reads the live account endpoint; emits only response-shape categories"]
    fn live_redacted_allowance_shape() {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        let token = match stored_token("") {
            Ok(token) => token,
            Err(_) => {
                println!("live-shape: local-sign-in-unavailable");
                return;
            }
        };
        let value = match crate::network::json(
            &client,
            "https://api.anthropic.com/api/oauth/usage",
            &[],
            Some(&token),
            &[
                ("anthropic-beta", "oauth-2025-04-20"),
                ("Accept", "application/json"),
            ],
            65536,
        ) {
            Ok(value) => value,
            Err(_) => {
                println!("live-shape: endpoint-unavailable");
                return;
            }
        };
        for slot in ["five_hour", "seven_day"] {
            let w = &value[slot];
            let reset = match &w["resets_at"] {
                Value::Null => "absent",
                Value::String(s) => match chrono::DateTime::parse_from_rfc3339(s) {
                    Ok(t) if t.timestamp() > unix_now() as i64 => "future",
                    Ok(_) => "elapsed",
                    Err(_) => "malformed",
                },
                _ => "malformed",
            };
            let state = match w["utilization"].as_f64() {
                Some(n) if n == 0.0 => "explicit-zero",
                Some(n) if n.is_finite() && (0.0..=100.0).contains(&n) => "valid-positive",
                _ => "missing-or-invalid",
            };
            println!(
                "live-shape: {slot}: window={}, utilization={state}, reset={reset}",
                if w.is_object() { "present" } else { "absent" }
            );
        }
    }
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
            |_| dir.join("refreshed").is_file(),
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
            refresh_client_terminal("", &dir, &client, |_| false, Duration::from_secs(1)).is_err()
        );
        fs::write(
            &client,
            "#!/bin/sh\necho $$ > pid\nwhile :; do sleep 1; done\n",
        )
        .unwrap();
        let start = Instant::now();
        assert!(
            refresh_client_terminal("", &dir, &client, |_| false, Duration::from_millis(300))
                .is_err()
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "renewal deadline must remain bounded"
        );
        let pid: i32 = fs::read_to_string(dir.join("pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "timed-out renewal client must be reaped"
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
        let pending = normalize(&v, "five_hour", now + 2 * 86400).unwrap();
        assert_eq!(pending.headline, "WAITING");
        assert_eq!(pending.values["allowanceState"], "reset_pending");
        assert!(pending.values["remainingPercent"].is_null());
    }

    #[test]
    fn idle_and_elapsed_windows_do_not_invent_usage_or_reset_times() {
        let now = unix_now();
        for reset in [Value::Null, json!("2000-01-01T00:00:00Z")] {
            let idle = normalize(
                &json!({"five_hour":{"utilization":0,"resets_at":reset}}),
                "five_hour",
                now,
            )
            .unwrap();
            assert_eq!(idle.headline, "100%");
            assert_eq!(idle.values["allowanceState"], "inactive");
            assert!(idle.values["resetsAt"].is_null());
            assert!(idle.usable(now + 299));
            assert!(!idle.usable(now + 600));
            let pending = normalize(
                &json!({"five_hour":{"utilization":12,"resets_at":reset}}),
                "five_hour",
                now,
            )
            .unwrap();
            assert_eq!(pending.headline, "WAITING");
            assert_eq!(pending.footer, "5H RESET PENDING");
            assert!(pending.values["usedPercent"].is_null());
            assert!(pending.values["remainingPercent"].is_null());
            assert!(pending.usable(now));
        }
        for invalid in [json!(-1), json!(101), Value::Null, json!("0")] {
            assert!(normalize(
                &json!({"five_hour":{"utilization":invalid,"resets_at":null}}),
                "five_hour",
                now
            )
            .is_err());
        }
        assert!(normalize(
            &json!({"five_hour":{"utilization":0,"resets_at":"bad"}}),
            "five_hour",
            now
        )
        .is_err());
        assert!(normalize(&json!({"five_hour":null}), "five_hour", now).is_err());
    }

    #[test]
    fn missing_credentials_and_failed_renewal_never_fabricate_a_token() {
        let mut renewals = 0;
        let result = token_with_refresh(
            || token_from_credentials(&json!({}), unix_now()),
            || {
                renewals += 1;
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(renewals, 0);
        let expired = json!({"claudeAiOauth":{"accessToken":"fixture-expired","expiresAt":1}});
        assert!(token_with_refresh(
            || token_from_credentials(&expired, unix_now()),
            || Err("fixture renewal unavailable".into())
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn expired_sign_in_renews_with_owned_client_then_fetches_real_card() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            os::unix::fs::PermissionsExt,
        };
        let now = unix_now();
        let dir = std::env::temp_dir().join(format!("deskbot-renewal-flow-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        // Isolated fixture credentials only; the user's credential store is
        // never altered and no model prompt is submitted.
        let credentials = dir.join("fixture.json");
        fs::write(
            &credentials,
            json!({"claudeAiOauth":{"accessToken":"fixture-expired","expiresAt":1}}).to_string(),
        )
        .unwrap();
        let renewed =
            json!({"claudeAiOauth":{"accessToken":"fixture-renewed","expiresAt":(now+3600)*1000}});
        let client_path = dir.join("client");
        fs::write(&client_path, format!("#!/bin/sh\ntest -t 0 && test -t 1 && test -t 2 || exit 2\necho $$ > pid\nprintf '%s' '{}' > fixture.json\nwhile :; do sleep 1; done\n", renewed)).unwrap();
        fs::set_permissions(&client_path, fs::Permissions::from_mode(0o700)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/usage", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let mut b = [0; 1024];
                let n = socket.read(&mut b).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&b[..n]);
            }
            assert!(String::from_utf8_lossy(&request).contains("Bearer fixture-renewed"));
            let body=json!({"five_hour":{"utilization":25,"resets_at":chrono::DateTime::from_timestamp((now+1800) as i64,0).unwrap().to_rfc3339()}}).to_string();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let stored = || {
            token_from_credentials(
                &serde_json::from_slice::<Value>(&fs::read(&credentials).unwrap()).unwrap(),
                unix_now(),
            )
        };
        let mut renewals = 0;
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let card = read_with_token(&client, &url, "five_hour", || {
            token_with_refresh(stored, || {
                renewals += 1;
                refresh_client_terminal(
                    "",
                    &dir,
                    &client_path,
                    |_| stored().is_ok(),
                    Duration::from_secs(3),
                )
            })
        })
        .unwrap();
        server.join().unwrap();
        assert_eq!(renewals, 1);
        assert_eq!(card.headline, "75%");
        assert_eq!(card.values["allowanceState"], "active");
        assert!(card.usable(unix_now()));
        let pid: i32 = fs::read_to_string(dir.join("pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "owned renewal client must be reaped"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn token_rotation_during_401_retries_once_and_persistent_rejection_is_bounded() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/usage", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for index in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let mut b = [0; 1024];
                    let n = socket.read(&mut b).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&b[..n]);
                }
                let expected = if index == 0 {
                    "Bearer fixture-old"
                } else {
                    "Bearer fixture-new"
                };
                assert!(String::from_utf8_lossy(&request).contains(expected));
                let status = if index == 0 {
                    "401 Unauthorized"
                } else {
                    "200 OK"
                };
                let body = if index == 0 {
                    "{}"
                } else {
                    r#"{"five_hour":{"utilization":0,"resets_at":null}}"#
                };
                write!(
                    socket,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let client = Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let mut calls = 0;
        let card = read_with_token(&client, &url, "five_hour", || {
            calls += 1;
            Ok(if calls == 1 {
                "fixture-old"
            } else {
                "fixture-new"
            }
            .into())
        })
        .unwrap();
        server.join().unwrap();
        assert_eq!(calls, 2);
        assert_eq!(card.values["allowanceState"], "inactive");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/usage", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut b = [0; 2048];
            socket.read(&mut b).unwrap();
            socket.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
        });
        let error = read_with_token(
            &client,
            &url,
            "five_hour",
            || Ok("fixture-unchanged".into()),
        )
        .unwrap_err();
        server.join().unwrap();
        assert!(error.contains("sign-in"));
    }

    #[test]
    fn server_rejection_renews_even_when_local_token_looks_valid() {
        use std::{
            cell::Cell,
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/usage", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for index in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let mut b = [0; 1024];
                    let n = socket.read(&mut b).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&b[..n]);
                }
                assert!(String::from_utf8_lossy(&request).contains(if index == 0 {
                    "Bearer fixture-rejected"
                } else {
                    "Bearer fixture-renewed"
                }));
                let (status, body) = if index == 0 {
                    ("401 Unauthorized", "{}")
                } else {
                    (
                        "200 OK",
                        r#"{"five_hour":{"utilization":0,"resets_at":null}}"#,
                    )
                };
                write!(
                    socket,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let renewed = Cell::new(false);
        let count = Cell::new(0);
        let client = Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let card = read_with_recovery(
            &client,
            &url,
            "five_hour",
            || {
                Ok(if renewed.get() {
                    "fixture-renewed"
                } else {
                    "fixture-rejected"
                }
                .into())
            },
            |rejected| {
                assert_eq!(rejected, "fixture-rejected");
                count.set(count.get() + 1);
                renewed.set(true);
                Ok(())
            },
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(count.get(), 1);
        assert_eq!(card.values["allowanceState"], "inactive");
        assert!(card.usable(unix_now()));
    }
}
