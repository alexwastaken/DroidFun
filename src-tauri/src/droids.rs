//! Per-board app profiles. USB identity is the key; friendly names are only labels.
use crate::core::{Engine, QuietHours, Settings, Widget};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DroidProfile {
    pub serial: String,
    pub name: String,
    pub model: String,
    pub widgets: Vec<Widget>,
    pub brightness: u8,
    pub manual_display_off: bool,
    pub orientation: u16,
    pub quiet_hours: QuietHours,
    pub companion_face: bool,
    pub companion_animation: bool,
    pub companion_speed: String,
    pub companion_glances: bool,
    pub companion_mouth: bool,
}
fn valid_serial(serial: &str) -> bool {
    !serial.is_empty()
        && serial.len() <= 128
        && serial
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn model_name(model: &str) -> &'static str {
    match model {
        "boo" => "Boo",
        "nimbus" => "Nimbus",
        "meep" => "Meep",
        "claw" => "Claw",
        _ => "Droid",
    }
}
impl DroidProfile {
    fn capture(settings: &Settings, serial: &str, name: String) -> Self {
        let p = &settings.preferences;
        Self {
            serial: serial.into(),
            name,
            model: p.bot_model.clone(),
            widgets: settings.widgets.clone(),
            brightness: settings.brightness,
            manual_display_off: settings.manual_display_off,
            orientation: p.orientation,
            quiet_hours: p.quiet_hours.clone(),
            companion_face: p.companion_face,
            companion_animation: p.companion_animation,
            companion_speed: p.companion_speed.clone(),
            companion_glances: p.companion_glances,
            companion_mouth: p.companion_mouth,
        }
    }
    fn apply(&self, settings: &mut Settings) {
        settings.widgets = self.widgets.clone();
        settings.brightness = self.brightness;
        settings.manual_display_off = self.manual_display_off;
        let p = &mut settings.preferences;
        p.bot_model = self.model.clone();
        p.setup_complete = true;
        p.orientation = self.orientation;
        p.quiet_hours = self.quiet_hours.clone();
        p.companion_face = self.companion_face;
        p.companion_animation = self.companion_animation;
        p.companion_speed = self.companion_speed.clone();
        p.companion_glances = self.companion_glances;
        p.companion_mouth = self.companion_mouth;
    }
    fn validate(&self) -> Result<(), String> {
        if !valid_serial(&self.serial)
            || self.name.trim().is_empty()
            || self.name.chars().count() > 40
            || self.name.chars().any(char::is_control)
            || !["boo", "meep", "claw", "nimbus"].contains(&self.model.as_str())
        {
            return Err("Choose a droid name of 1–40 characters and a supported model.".into());
        }
        let mut settings = Settings::default();
        self.apply(&mut settings);
        // This temporary settings value contains no nested profiles.
        settings.validate()
    }
}

pub fn validate(profiles: &[DroidProfile]) -> Result<(), String> {
    if profiles.len() > 32 {
        return Err("At most 32 droids can be remembered.".into());
    }
    let mut seen = std::collections::HashSet::new();
    for profile in profiles {
        profile.validate()?;
        if !seen.insert(profile.serial.to_ascii_uppercase()) {
            return Err("Each remembered droid must have a different identity.".into());
        }
    }
    Ok(())
}

pub fn remember(settings: &mut Settings) -> Result<(), String> {
    let Some(serial) = settings.selected_device_serial.clone() else {
        return Ok(());
    };
    if !valid_serial(&serial) {
        return Err("This droid has an invalid identity.".into());
    }
    if !["boo", "meep", "claw", "nimbus"].contains(&settings.preferences.bot_model.as_str()) {
        return Ok(());
    }
    let index = settings
        .droids
        .iter()
        .position(|p| p.serial.eq_ignore_ascii_case(&serial));
    let name = index
        .map(|i| settings.droids[i].name.clone())
        .unwrap_or_else(|| {
            let base = model_name(&settings.preferences.bot_model);
            (1..)
                .map(|n| format!("{base} {n}"))
                .find(|name| !settings.droids.iter().any(|p| &p.name == name))
                .unwrap()
        });
    let profile = DroidProfile::capture(settings, &serial, name);
    if let Some(i) = index {
        settings.droids[i] = profile;
    } else {
        settings.droids.push(profile);
    }
    validate(&settings.droids)
}

pub fn select(
    current: &Settings,
    serial: Option<&str>,
    model: Option<&str>,
) -> Result<Settings, String> {
    let mut next = current.clone();
    remember(&mut next)?;
    let Some(serial) = serial else {
        next.selected_device_serial = None;
        return Ok(next);
    };
    if !valid_serial(serial) {
        return Err("This droid has an invalid identity.".into());
    }
    let existing = next
        .droids
        .iter()
        .find(|p| p.serial.eq_ignore_ascii_case(serial))
        .cloned();
    if let Some(profile) = existing {
        profile.apply(&mut next);
    } else {
        let chosen = model.unwrap_or(&current.preferences.bot_model);
        if !["boo", "meep", "claw", "nimbus"].contains(&chosen) {
            return Err("Choose Nimbus, Boo, Meep or Claw to finish adding this droid.".into());
        }
        if !next.droids.is_empty() && model.is_none() {
            return Err("Choose the new droid’s model before connecting it.".into());
        }
        if next.droids.is_empty() {
            // The first physical board inherits the user's existing playlist.
            let mut engine = Engine::new(next, Instant::now());
            engine.set_bot_model(chosen)?;
            next = engine.settings;
        } else {
            // New physical droids start fresh. App-wide preferences stay shared.
            let mut defaults = Engine::new(Settings::default(), Instant::now());
            defaults.set_bot_model(chosen)?;
            DroidProfile::capture(&defaults.settings, serial, String::new()).apply(&mut next);
        }
    }
    next.selected_device_serial = Some(serial.into());
    remember(&mut next)?;
    next.validate()?;
    Ok(next)
}

pub fn rename(settings: &mut Settings, serial: &str, name: &str) -> Result<(), String> {
    remember(settings)?;
    let mut profile = settings
        .droids
        .iter()
        .find(|p| p.serial.eq_ignore_ascii_case(serial))
        .cloned()
        .ok_or("Connect this droid before naming it.")?;
    profile.name = name.trim().into();
    profile.validate()?;
    *settings
        .droids
        .iter_mut()
        .find(|p| p.serial.eq_ignore_ascii_case(serial))
        .unwrap() = profile;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_board_inherits_existing_playlist_and_same_model_boards_stay_separate() {
        let mut original = Settings::default();
        original.preferences.bot_model = "meep".into();
        original.brightness = 41;
        original.widgets[0].label = "Original face".into();
        let mut first = select(&original, Some("BOARD_A"), None).unwrap();
        assert_eq!(first.widgets[0].label, "Original face");
        first.preferences.orientation = 180;
        first.preferences.quiet_hours.enabled = true;
        first.manual_display_off = true;
        first.preferences.offline_mode = true;
        first.preferences.launch_at_login = true;
        let mut second = select(&first, Some("BOARD_B"), Some("meep")).unwrap();
        assert_eq!(second.widgets[0].label, "Happy face");
        assert_eq!(second.brightness, 85);
        assert!(!second.manual_display_off);
        assert!(second.preferences.offline_mode && second.preferences.launch_at_login);
        second.widgets[0].label = "Second face".into();
        second.brightness = 66;
        let restored = select(&second, Some("board_a"), None).unwrap();
        assert_eq!(restored.widgets[0].label, "Original face");
        assert_eq!(restored.brightness, 41);
        assert!(restored.manual_display_off);
        assert_eq!(restored.preferences.orientation, 180);
        assert!(restored.preferences.quiet_hours.enabled);
        assert_eq!(restored.droids[0].name, "Meep 1");
        assert_eq!(restored.droids[1].name, "Meep 2");
        let again = select(&restored, Some("BOARD_B"), None).unwrap();
        assert_eq!(again.widgets[0].label, "Second face");
        assert_eq!(again.brightness, 66);
    }
    #[test]
    fn update_pause_restore_keeps_latest_edits_and_renaming_is_identity_based() {
        let mut s = Settings::default();
        s.preferences.bot_model = "claw".into();
        let mut first = select(&s, Some("A"), None).unwrap();
        rename(&mut first, "A", "Office Claw").unwrap();
        first.brightness = 92;
        let paused = select(&first, None, None).unwrap();
        let restored = select(&paused, Some("A"), None).unwrap();
        assert_eq!(restored.brightness, 92);
        assert_eq!(restored.droids[0].name, "Office Claw");
        assert!(select(&restored, Some("B"), None).is_err());
        let second = select(&restored, Some("B"), Some("meep")).unwrap();
        assert_eq!(second.preferences.bot_model, "meep");
        assert!(second.widgets[0].enabled);
        let back = select(&second, Some("A"), None).unwrap();
        assert_eq!(back.preferences.bot_model, "claw");
        assert_eq!(back.droids[0].name, "Office Claw");
    }
    #[test]
    fn restored_boo_and_nimbus_profiles_are_selectable_and_isolated() {
        let now = std::time::Instant::now();
        let mut engine = crate::core::Engine::new(Settings::default(), now);
        engine.set_bot_model("boo").unwrap();
        let mut boo = select(&engine.settings, Some("BOO_BOARD"), Some("boo")).unwrap();
        boo.brightness = 37;
        remember(&mut boo).unwrap();
        let mut nimbus = select(&boo, Some("CLOUD_BOARD"), Some("nimbus")).unwrap();
        assert_eq!(nimbus.preferences.bot_model, "nimbus");
        nimbus.brightness = 82;
        remember(&mut nimbus).unwrap();
        let restored = select(&nimbus, Some("BOO_BOARD"), None).unwrap();
        assert_eq!(restored.preferences.bot_model, "boo");
        assert_eq!(restored.brightness, 37);
        assert_eq!(select(&restored, Some("CLOUD_BOARD"), None).unwrap().brightness, 82);
        restored.validate().unwrap();
    }

    #[test]
    fn profiles_reject_duplicate_identity_invalid_names_and_invalid_playlists() {
        let mut s = Settings::default();
        s.preferences.bot_model = "meep".into();
        let mut s = select(&s, Some("A"), None).unwrap();
        assert!(rename(&mut s, "A", "").is_err());
        assert!(rename(&mut s, "A", "hello\nworld").is_err());
        let mut duplicate = s.droids[0].clone();
        duplicate.serial = "a".into();
        s.droids.push(duplicate);
        assert!(s.validate().is_err());
        s.droids.pop();
        s.droids[0].widgets.clear();
        assert!(s.validate().is_err());
    }
}
