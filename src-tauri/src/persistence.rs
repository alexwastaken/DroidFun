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
                if value["version"].as_u64().is_some_and(|v| v > crate::core::SETTINGS_VERSION as u64) {
                    return Err("These settings belong to a newer DroidFun App version. Install that version; your file has been preserved.".into());
                }
                if let Ok(mut settings) = serde_json::from_value::<Settings>(value) {
                    if settings.version == 1 || settings.version == 2 { settings.version = crate::core::SETTINGS_VERSION; }
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
                        crate::droids::remember(&mut settings)?;
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
            if value["version"].as_u64().is_some_and(|v| v > crate::core::SETTINGS_VERSION as u64) {
                return Err(
                    "Newer settings preserved; update DroidFun App before changing them".into(),
                );
            }
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    if bytes.len() > 262144 { return Err("Your saved droid settings are too large. Shorten a playlist before saving again.".into()); }
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
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    if path.exists() {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        if serde_json::from_slice::<Settings>(&bytes).is_ok_and(|mut s| { if s.version == 1 || s.version == 2 { s.version = crate::core::SETTINGS_VERSION; } s.validate().is_ok() }) {
            if serde_json::from_slice::<Settings>(&bytes).is_ok_and(|s| s.version == 1) {
                let archive = path.with_extension("before-droid-profiles.json");
                match fs::OpenOptions::new().write(true).create_new(true).open(&archive) {
                    Ok(mut original) => {
                        #[cfg(unix)] {
                            use std::os::unix::fs::PermissionsExt;
                            original.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
                        }
                        if let Err(error) = original.write_all(&bytes).and_then(|_| original.sync_all()) {
                            let _ = fs::remove_file(&archive);
                            return Err(format!("Could not preserve your original droid settings: {error}"));
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
                    Err(error) => return Err(format!("Could not preserve your original droid settings: {error}")),
                }
            }
            if serde_json::from_slice::<Settings>(&bytes).is_ok_and(|s| s.version == 2) {
                let archive = path.with_extension("before-custom-screens.json");
                match fs::OpenOptions::new().write(true).create_new(true).open(&archive) {
                    Ok(mut original) => {
                        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; original.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?; }
                        if let Err(e) = original.write_all(&bytes).and_then(|_| original.sync_all()) { let _ = fs::remove_file(&archive); return Err(e.to_string()); }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {},
                    Err(e) => return Err(format!("Could not preserve your previous settings: {e}")),
                }
            }
            fs::copy(path, path.with_extension("json.bak")).map_err(|e| e.to_string())?;
        }
    }
    fs::rename(&temp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_two_customizations_migrate_and_new_images_round_trip_with_an_original_archive() {
        let root=std::env::temp_dir().join(format!("droidfun-custom-screens-{}",std::process::id()));fs::create_dir_all(&root).unwrap();let path=root.join("settings.json");
        let mut old=Settings::default();old.version=2;old.brightness=38;old.preferences.bot_model="boo".into();old.widgets[0].label="My saved face".into();
        let mut value=serde_json::to_value(&old).unwrap();for w in value["widgets"].as_array_mut().unwrap(){for key in ["textMotion","scrollSpeed","imagePixels"] {w["config"].as_object_mut().unwrap().remove(key);}}
        let original=serde_json::to_vec(&value).unwrap();fs::write(&path,&original).unwrap();let loaded=load(&path).unwrap();assert_eq!(loaded.version,3);assert_eq!(loaded.brightness,38);assert_eq!(loaded.widgets[0].label,"My saved face");assert_eq!(loaded.widgets[0].config.text_motion,"static");
        let mut e=crate::core::Engine::new(loaded,std::time::Instant::now());e.add_widget("image").unwrap();let image=e.settings.widgets.last_mut().unwrap();image.config.image_pixels="a5".repeat(1024);image.enabled=true;save(&path,&e.settings).unwrap();assert_eq!(fs::read(path.with_extension("before-custom-screens.json")).unwrap(),original);
        let restored=load(&path).unwrap();assert_eq!(restored.widgets.last().unwrap().config.image_pixels,"a5".repeat(1024));assert_eq!(restored.brightness,38);
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;assert_eq!(fs::metadata(path.with_extension("before-custom-screens.json")).unwrap().permissions().mode()&0o777,0o600);}
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_settings_migrate_with_per_droid_profile_and_keep_original_backup() {
        let dir = std::env::temp_dir().join(format!("droidfun-profile-migration-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap(); let path = dir.join("settings.json");
        let mut legacy = Settings::default(); legacy.version = 1;
        legacy.selected_device_serial = Some("LEGACY_A".into()); legacy.preferences.bot_model = "meep".into(); legacy.brightness = 37;
        for w in &mut legacy.widgets { w.enabled = false; }
        let mut value = serde_json::to_value(&legacy).unwrap(); value.as_object_mut().unwrap().remove("droids");
        let bytes = serde_json::to_vec(&value).unwrap(); fs::write(&path,&bytes).unwrap();
        let upgraded = load(&path).unwrap(); assert_eq!(upgraded.version,3);assert_eq!(upgraded.brightness,37);
        assert!(upgraded.widgets.iter().all(|w| !w.enabled));assert_eq!(upgraded.droids[0].name,"Meep 1");
        assert_eq!(fs::read(&path).unwrap(), bytes); save(&path,&upgraded).unwrap();
        assert_eq!(fs::read(path.with_extension("json.bak")).unwrap(),bytes);
        assert_eq!(load(&path).unwrap().droids[0].serial,"LEGACY_A");
        let mut changed = upgraded.clone(); changed.brightness = 42; save(&path,&changed).unwrap();
        assert_eq!(fs::read(path.with_extension("before-droid-profiles.json")).unwrap(),bytes);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn oversized_profiles_do_not_replace_current_settings() {
        let dir=std::env::temp_dir().join(format!("droidfun-profile-size-{}",std::process::id()));fs::create_dir_all(&dir).unwrap();let path=dir.join("settings.json");
        let mut s=Settings::default();s.preferences.bot_model="meep".into();save(&path,&s).unwrap();let before=fs::read(&path).unwrap();
        s.widgets=(0..30).map(|i|{let mut w=s.widgets[0].clone();w.id=format!("face{i}");w}).collect();
        for i in 0..16 { s=crate::droids::select(&s,Some(&format!("BOARD_{i}")),Some("meep")).unwrap();s.widgets=(0..30).map(|j|{let mut w=s.widgets[0].clone();w.id=format!("face{j}");w}).collect();crate::droids::remember(&mut s).unwrap(); }
        assert!(serde_json::to_vec_pretty(&s).unwrap().len()>262144);assert!(save(&path,&s).is_err());assert_eq!(fs::read(&path).unwrap(),before);fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn upgrading_an_all_off_playlist_does_not_apply_new_bot_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "droidfun-onboarding-upgrade-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut existing = Settings::default();
        for widget in &mut existing.widgets {
            widget.enabled = false;
        }
        existing.brightness = 41;
        existing.manual_display_off = true;
        save(&path, &existing).unwrap();
        let mut engine = crate::core::Engine::new(load(&path).unwrap(), std::time::Instant::now());
        for model in ["claw", "meep"] {
            engine.set_bot_model(model).unwrap();
            assert!(engine.settings.widgets.iter().all(|w| !w.enabled));
            assert_eq!(engine.settings.brightness, 41);
            assert!(engine.settings.manual_display_off);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_settings_require_bot_selection_and_preserve_all_existing_screens() {
        let dir = std::env::temp_dir().join(format!("droidfun-profiles-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut expected = Settings::default();
        expected.widgets[1].enabled = true;
        expected.widgets[3].enabled = true;
        expected.widgets[1].label = "My forecast".into();
        let mut legacy = serde_json::to_value(&expected).unwrap();
        let preferences = legacy["preferences"].as_object_mut().unwrap();
        preferences.remove("botModel");
        preferences.remove("offlineMode");
        for widget in legacy["widgets"].as_array_mut().unwrap() {
            let config = widget["config"].as_object_mut().unwrap();
            for key in [
                "customText",
                "textSize",
                "textAlign",
                "textValign",
                "textBorder",
                "displayInvert",
                "displayCompanion",
                "displayLayout",
            ] {
                config.remove(key);
            }
        }
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.preferences.bot_model, "");
        assert!(!loaded.preferences.offline_mode);
        assert_eq!(
            serde_json::to_value(&loaded.widgets).unwrap(),
            serde_json::to_value(&expected.widgets).unwrap()
        );
        let mut engine = crate::core::Engine::new(loaded, std::time::Instant::now());
        for model in ["claw", "meep"] {
            engine.set_bot_model(model).unwrap();
            save(&path, &engine.settings).unwrap();
            let reopened = load(&path).unwrap();
            assert_eq!(reopened.preferences.bot_model, model);
            assert_eq!(
                serde_json::to_value(&reopened.widgets).unwrap(),
                serde_json::to_value(&expected.widgets).unwrap()
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn branding_upgrade_and_saved_selection_preserve_playlist() {
        let dir = std::env::temp_dir().join(format!("deskbot-branding-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let mut expected = Settings::default();
        expected.brightness = 44;
        expected.preferences.appearance = "dark".into();
        let mut legacy = serde_json::to_value(&expected).unwrap();
        legacy["preferences"]
            .as_object_mut()
            .unwrap()
            .remove("branding");
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let mut loaded = load(&path).unwrap();
        assert_eq!(loaded.preferences.branding, "droidfun");
        assert_eq!(loaded.brightness, 44);
        assert_eq!(loaded.preferences.appearance, "dark");
        assert_eq!(
            serde_json::to_value(&loaded.widgets).unwrap(),
            legacy["widgets"]
        );
        for branding in ["green", "droidfun"] {
            loaded.preferences.branding = branding.into();
            save(&path, &loaded).unwrap();
            let reopened = load(&path).unwrap();
            assert_eq!(reopened.preferences.branding, branding);
            assert_eq!(
                serde_json::to_value(&reopened.widgets).unwrap(),
                legacy["widgets"]
            );
            assert_eq!(reopened.brightness, 44);
            assert_eq!(reopened.preferences.appearance, "dark");
        }
        loaded.preferences.branding = "unknown".into();
        assert!(loaded.validate().is_err());
        fs::remove_dir_all(dir).unwrap();
    }

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
