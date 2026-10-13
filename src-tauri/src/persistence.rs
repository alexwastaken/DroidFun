//! Version-safe settings storage. Never replace an unknown newer schema.
use crate::core::Settings;
use std::{fs, io::Write, path::Path};

pub fn load(path: &Path) -> Result<Settings, String> {
    for candidate in [path.to_path_buf(), path.with_extension("json.bak")] {
        if let Ok(bytes) = fs::read(&candidate) {
            if bytes.len() > 262144 {
                return Err("Settings exceeded the size limit; original file preserved".into());
            }
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if value["version"].as_u64().is_some_and(|v| v > 1) {
                    return Err("These settings belong to a newer DroidFun App version. Install that version; your file has been preserved.".into());
                }
                if let Ok(mut settings) = serde_json::from_value::<Settings>(value) {
                    if settings.validate().is_ok() {
                        if candidate != path && path.exists() {
                            fs::copy(
                                path,
                                path.with_extension(format!(
                                    "unreadable-{}.json",
                                    crate::core::unix_now()
                                )),
                            )
                            .map_err(|_| "Could not preserve unreadable settings")?;
                        }
                        // Existing installations have already chosen their preview/device.
                        settings.preferences.setup_complete = true;
                        return Ok(settings);
                    }
                }
            }
        }
    }
    if path.exists() {
        fs::copy(
            path,
            path.with_extension(format!("unreadable-{}.json", crate::core::unix_now())),
        )
        .map_err(|_| "Could not preserve unreadable settings")?;
    }
    Ok(Settings::default())
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    settings.validate()?;
    if path.exists() {
        if let Ok(value) =
            serde_json::from_slice::<serde_json::Value>(&fs::read(path).map_err(|e| e.to_string())?)
        {
            if value["version"].as_u64().is_some_and(|v| v > 1) {
                return Err("Newer settings preserved; update DroidFun App before changing them".into());
            }
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("json.tmp");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    file.write_all(&serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    if path.exists() {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        if serde_json::from_slice::<Settings>(&bytes).is_ok_and(|s| s.validate().is_ok()) {
            fs::copy(path, path.with_extension("json.bak")).map_err(|e| e.to_string())?;
        }
    }
    fs::rename(&temp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn future_schema_is_preserved_and_backup_recovers() {
        let dir = std::env::temp_dir().join(format!(
            "deskbot-settings-{}-{}",
            std::process::id(),
            crate::core::unix_now()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        save(&path, &Settings::default()).unwrap();
        let mut settings = Settings::default();
        settings.brightness = 44;
        save(&path, &settings).unwrap();
        fs::write(&path, b"{broken").unwrap();
        assert_eq!(load(&path).unwrap().brightness, 85);
        save(&path, &settings).unwrap();
        assert_eq!(
            serde_json::from_slice::<Settings>(&fs::read(path.with_extension("json.bak")).unwrap())
                .unwrap()
                .brightness,
            85
        );
        let future = b"{\"version\":99}";
        fs::write(&path, future).unwrap();
        assert!(load(&path).is_err());
        assert!(save(&path, &settings).is_err());
        assert_eq!(fs::read(&path).unwrap(), future);
        fs::remove_dir_all(dir).unwrap();
    }
}
