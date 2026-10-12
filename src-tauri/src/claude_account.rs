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
    Err("Connect your Claude account in Edit. Deskbot uses your local Claude Code sign-in to read the shared account allowance.".into())
}
fn token(profile: &str) -> Result<String, String> {
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
static CONNECTING: AtomicBool = AtomicBool::new(false);
struct LoginGuard;
impl Drop for LoginGuard {
    fn drop(&mut self) {
        CONNECTING.store(false, Ordering::Release);
    }
}
pub fn status(profile: &str) -> Value {
    match token(profile) {
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
