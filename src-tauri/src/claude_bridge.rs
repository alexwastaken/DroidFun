use crate::{
    core::unix_now,
    providers::{ClaudeReport, UsageWindowStored},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn home() -> Result<PathBuf, String> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or("Home directory unavailable".into())
}
fn quote(s: &str) -> String {
    if cfg!(windows) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}
fn managed(directory: &Path) -> Result<Value, String> {
    let executable = directory.join(if cfg!(windows) {
        "deskbot-statusline.exe"
    } else {
        "deskbot-statusline"
    });
    let command = format!(
        "{} --claude-statusline {}",
        quote(&executable.to_string_lossy()),
        quote(&directory.join("bridge.json").to_string_lossy())
    );
    Ok(json!({"type":"command","command":command}))
}
fn refresh_helper(directory: &Path) -> Result<(), String> {
    let destination = directory.join(if cfg!(windows) {
        "deskbot-statusline.exe"
    } else {
        "deskbot-statusline"
    });
    let temp = directory.join(format!("helper-{}.tmp", std::process::id()));
    let exe = std::env::current_exe().map_err(|_| "App executable unavailable")?;
    fs::copy(exe, &temp)
        .and_then(|_| fs::rename(&temp, destination))
        .map_err(|_| "Could not update the native status-line helper".to_string())
}
pub fn refresh_installed_helper(directory: &Path) -> Result<(), String> {
    if status(directory)?["enabled"].as_bool() == Some(true) {
        refresh_helper(directory)?;
    }
    Ok(())
}
pub fn status(directory: &Path) -> Result<Value, String> {
    let settings = home()?.join(".claude/settings.json");
    let v: Value = if settings.exists() {
        serde_json::from_slice(&fs::read(&settings).map_err(|_| "Could not read Claude settings")?)
            .map_err(|_| "Claude settings are invalid JSON")?
    } else {
        json!({})
    };
    Ok(
        json!({"enabled":v.get("statusLine")==Some(&managed(directory)?),"command":managed(directory)?["command"],"hasExisting":v.get("statusLine").is_some(),"explanation":"Records only allowance percentages, reset times and session ID locally. Existing status-line output is preserved. No prompts or transcripts are saved."}),
    )
}
pub fn install(directory: &Path, report_directory: &Path) -> Result<Value, String> {
    fs::create_dir_all(directory).map_err(|_| "Could not create bridge directory")?;
    let settings = home()?.join(".claude/settings.json");
    let mut v: Value = if settings.exists() {
        serde_json::from_slice(&fs::read(&settings).map_err(|_| "Could not read Claude settings")?)
            .map_err(|_| "Claude settings are invalid JSON")?
    } else {
        json!({})
    };
    if !v.is_object() {
        return Err("Claude settings must be a JSON object".into());
    }
    let new = managed(directory)?;
    if v.get("statusLine") == Some(&new) {
        refresh_helper(directory)?;
        return status(directory);
    }
    let backup = directory.join("statusline-backup.json");
    if backup.exists() {
        return Err(
            "A previous bridge backup exists. Restore that installation before enabling a new one."
                .into(),
        );
    }
    let original = v.get("statusLine").cloned().unwrap_or(Value::Null);
    if !original.is_null()
        && (original["type"].as_str() != Some("command") || original["command"].as_str().is_none())
    {
        return Err("This Claude status line cannot be safely chained; keep it unchanged".into());
    }
    fs::write(
        &backup,
        serde_json::to_vec_pretty(&json!({"original":original})).unwrap(),
    )
    .map_err(|_| "Could not save original status line")?;
    fs::write(
        directory.join("bridge.json"),
        serde_json::to_vec_pretty(&json!({"directory":report_directory,"original":original}))
            .unwrap(),
    )
    .map_err(|_| "Could not save bridge configuration")?;
    refresh_helper(directory)?;
    v["statusLine"] = new;
    if let Some(parent) = settings.parent() {
        fs::create_dir_all(parent).map_err(|_| "Could not create Claude settings directory")?;
    }
    fs::write(
        settings.with_extension("deskbot.tmp"),
        serde_json::to_vec_pretty(&v).unwrap(),
    )
    .and_then(|_| fs::rename(settings.with_extension("deskbot.tmp"), &settings))
    .map_err(|_| "Could not update Claude status line; original backup retained")?;
    status(directory)
}
pub fn uninstall(directory: &Path) -> Result<Value, String> {
    let settings = home()?.join(".claude/settings.json");
    let mut v: Value =
        serde_json::from_slice(&fs::read(&settings).map_err(|_| "Could not read Claude settings")?)
            .map_err(|_| "Claude settings are invalid JSON")?;
    if v.get("statusLine") != Some(&managed(directory)?) {
        return Err("Claude status line was changed outside Deskbot. It has been preserved; restore the saved original manually if needed.".into());
    }
    let backup: Value = serde_json::from_slice(
        &fs::read(directory.join("statusline-backup.json"))
            .map_err(|_| "Original status-line backup is missing")?,
    )
    .map_err(|_| "Invalid backup")?;
    if backup["original"].is_null() {
        v.as_object_mut().unwrap().remove("statusLine");
    } else {
        v["statusLine"] = backup["original"].clone();
    }
    fs::write(
        settings.with_extension("deskbot.tmp"),
        serde_json::to_vec_pretty(&v).unwrap(),
    )
    .and_then(|_| fs::rename(settings.with_extension("deskbot.tmp"), &settings))
    .map_err(|_| "Could not restore original Claude setting")?;
    // Keep a recoverable archive so reinstall never destroys the prior command.
    fs::rename(
        directory.join("statusline-backup.json"),
        directory.join(format!("statusline-backup-{}.json", unix_now())),
    )
    .map_err(|_| "Original restored, but backup could not be archived")?;
    status(directory)
}
fn allowance(v: &Value, minutes: u64) -> Option<UsageWindowStored> {
    let used = v["used_percentage"].as_f64().filter(|v| v.is_finite())?;
    let reset = v["resets_at"].as_u64().filter(|v| *v > 0)?;
    Some(UsageWindowStored {
        used_percent: used,
        resets_at: reset,
        minutes,
    })
}
pub fn sanitized_report(input: &[u8], now: u64) -> Option<ClaudeReport> {
    if input.len() > 65536 {
        return None;
    }
    let v: Value = serde_json::from_slice(input).ok()?;
    let session = v["session_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 128)?;
    Some(ClaudeReport {
        version: 1,
        profile: "default".into(),
        session_id: session.into(),
        observed_at: now,
        five_hour: allowance(&v["rate_limits"]["five_hour"], 300),
        seven_day: allowance(&v["rate_limits"]["seven_day"], 10080),
    })
}
fn save_report(directory: &Path, input: &[u8]) -> Result<(), String> {
    let Some(report) = sanitized_report(input, unix_now()) else {
        return Ok(());
    };
    fs::create_dir_all(directory).map_err(|_| "Could not create report directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Could not protect report directory")?;
    }
    let id = format!("{:x}", Sha256::digest(report.session_id.as_bytes()));
    let path = directory.join(format!("{id}.json"));
    let temp = directory.join(format!("{id}-{}.tmp", std::process::id()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(&temp)
        .and_then(|mut f| f.write_all(&serde_json::to_vec(&report).unwrap()))
        .and_then(|_| fs::rename(temp, path))
        .map_err(|_| "Could not save Claude report".to_string())
}
pub fn run(config_path: &Path) -> Result<(), String> {
    let config: Value =
        serde_json::from_slice(&fs::read(config_path).map_err(|_| "Bridge config missing")?)
            .map_err(|_| "Bridge config invalid")?;
    let mut input = Vec::new();
    std::io::stdin()
        .take(65537)
        .read_to_end(&mut input)
        .map_err(|_| "Could not read status-line input")?;
    if let Some(directory) = config["directory"].as_str() {
        let _ = save_report(Path::new(directory), &input);
    }
    if let Some(command) = config["original"]["command"].as_str() {
        let mut process = if cfg!(windows) {
            let mut c = Command::new("cmd.exe");
            c.args(["/D", "/S", "/C", command]);
            c
        } else {
            let mut c = Command::new("/bin/sh");
            c.args(["-c", command]);
            c
        };
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            process.process_group(0);
        }
        let mut child = process
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_| "Could not start original status line")?;
        if let Some(mut stdin) = child.stdin.take() {
            std::thread::spawn(move || {
                let _ = stdin.write_all(&input);
                if input.len() > 65536 {
                    let _ = std::io::copy(&mut std::io::stdin(), &mut stdin);
                }
            });
        }
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            if child
                .try_wait()
                .map_err(|_| "Status line wait failed")?
                .is_some()
            {
                break;
            }
            if Instant::now() >= end {
                // The original command is in a group created by this helper;
                // stop its children too so inherited output pipes cannot hang.
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    } else {
        println!("Claude · Deskbot usage bridge");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_allowlisted_fields_are_saved_and_missing_limits_are_not_zero() {
        let report=sanitized_report(br#"{"session_id":"session-a","transcript_path":"secret","model":{"name":"private"},"rate_limits":{"five_hour":{"used_percentage":42,"resets_at":9999}}}"#,1000).unwrap();
        let saved = serde_json::to_string(&report).unwrap();
        assert!(!saved.contains("secret"));
        assert!(!saved.contains("private"));
        let w = crate::providers::UsageWindow::from(report.five_hour.as_ref().unwrap());
        let d = crate::providers::usage_card("claude-code", "session-a", &w, 1000);
        assert_eq!(d.headline, "58%");
        assert_eq!(d.state_at(1600), "stale");
        assert_eq!(d.state_at(4600), "unavailable");
        let missing =
            sanitized_report(br#"{"session_id":"session-a","rate_limits":{}}"#, 1001).unwrap();
        assert!(missing.five_hour.is_none());
        assert!(missing.seven_day.is_none());
        assert!(sanitized_report(&vec![b'x'; 65537], 1000).is_none());
    }
}
