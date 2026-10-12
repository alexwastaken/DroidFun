//! Opt-in local measurements and scoped fault injection for reliability runs.
//! No listener, account contents, or additional provider requests.
use std::{path::PathBuf, sync::OnceLock};

pub fn directory() -> Option<&'static PathBuf> {
    static DIRECTORY: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIRECTORY
        .get_or_init(|| std::env::var_os("DESKBOT_RELIABILITY_DIR").map(PathBuf::from))
        .as_ref()
}

pub fn fault(name: &str) -> bool {
    directory().is_some_and(|dir| dir.join(name).is_file())
}

pub fn transition(snapshot: &crate::core::Snapshot) {
    let Some(dir) = directory() else {
        return;
    };
    use std::{
        io::Write,
        sync::Mutex,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    static LAST: Mutex<Option<(String, String, bool)>> = Mutex::new(None);
    static START: OnceLock<Instant> = OnceLock::new();
    let playback = serde_json::to_string(&snapshot.playback).unwrap_or_default();
    let key = (snapshot.current_id.clone(), playback, snapshot.display_on);
    let Ok(mut last) = LAST.lock() else {
        return;
    };
    if last.as_ref() == Some(&key) {
        return;
    }
    let index = snapshot
        .settings
        .widgets
        .iter()
        .position(|w| w.id == snapshot.current_id);
    let kind = index.map(|i| snapshot.settings.widgets[i].kind.as_str());
    let value = serde_json::json!({"atMs":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),"elapsedMs":START.get_or_init(Instant::now).elapsed().as_millis(),"index":index,"kind":kind,"playback":snapshot.playback,"displayOn":snapshot.display_on,"temporary":snapshot.temporary});
    let write = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("transitions.jsonl"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        let bytes = serde_json::to_vec(&value)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")
    })();
    if write.is_ok() {
        *last = Some(key);
    }
}

pub fn write(value: &serde_json::Value) -> std::io::Result<()> {
    let Some(dir) = directory() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir)?;
    let temp = dir.join("telemetry.tmp");
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(&serde_json::to_vec(value)?)?;
    drop(file);
    std::fs::rename(temp, dir.join("telemetry.json"))
}
