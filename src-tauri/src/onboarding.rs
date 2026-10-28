use crate::{connection::ConnectionSnapshot, core::Engine};
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

/// One successful HELLO per attachment, with bounded retries for transient
/// failures. A deliberate scan starts a new generation; passive polls do not.
#[derive(Default)]
pub struct ProbeSchedule {
    generation: u64,
    successful: HashSet<String>,
    failed: HashMap<String, (u8, Instant)>,
}

impl ProbeSchedule {
    pub fn refresh<'a>(&mut self, generation: u64, attached: impl Iterator<Item = &'a str>) {
        if self.generation != generation {
            self.successful.clear();
            self.failed.clear();
            self.generation = generation;
        }
        let attached: HashSet<_> = attached.collect();
        self.successful
            .retain(|serial| attached.contains(serial.as_str()));
        self.failed
            .retain(|serial, _| attached.contains(serial.as_str()));
    }

    pub fn ready(&self, serial: &str, now: Instant) -> bool {
        !self.successful.contains(serial)
            && self.failed.get(serial).is_none_or(|(attempts, last)| {
                *attempts < 3
                    && now.saturating_duration_since(*last)
                        >= Duration::from_secs(u64::from(*attempts) * 2)
            })
    }

    pub fn record_success(&mut self, serial: &str) {
        self.successful.insert(serial.into());
        self.failed.remove(serial);
    }

    pub fn record_failure(&mut self, serial: &str, now: Instant) {
        let failed = self.failed.entry(serial.into()).or_insert((0, now));
        failed.0 = failed.0.saturating_add(1);
        failed.1 = now;
    }
}

pub const FACTORY_MODELS: [&str; 4] = ["boo", "claw", "meep", "nimbus"];
pub const HARDWARE_PROFILE: &str = "rp2040-zero-gp4-gp5-ssd1306";
pub const MANAGED_FIRMWARE_HELP: &str = "This droid uses factory or custom firmware. Its model and screen wiring must be preserved. Contact Droid Fun support for firmware service; app updates are still available in Settings → App.";

fn version(value: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit())) { return None; }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}

/// The customer recovery image is standard wiring 1.2.4. Never downgrade a
/// newer/factory/custom image or erase its identity, even if malformed.
pub fn managed_firmware(firmware: &str, profile: Option<&str>, provisioning: bool, identity_present: bool) -> bool {
    provisioning || identity_present || profile.is_some_and(|p| p != "GP4-GP5")
        || version(firmware).is_none_or(|v| v > (1, 2, 4))
}
pub fn older_firmware(firmware: &str) -> bool {
    version(firmware).is_some_and(|v| v < (1, 2, 4))
}
pub fn allow_customer_reflash(status: &ConnectionSnapshot) -> Result<(), String> {
    if status.firmware_managed || status.firmware.as_deref().is_none_or(|f|
        managed_firmware(f, status.pin_profile.as_deref(), status.provisioning_supported, status.identity.is_some())) {
        Err(MANAGED_FIRMWARE_HELP.into())
    } else { Ok(()) }
}

/// Called only with a single attached board and a validated HELLO. Existing
/// profiles take precedence over factory defaults; manual selection is never
/// replaced. New profiles use the normal isolated-profile transaction.
pub fn adopt(engine: &mut Engine, status: &ConnectionSnapshot) -> Result<(), String> {
    if engine.settings.selected_device_serial.is_some() { return Err("A droid is already selected".into()); }
    let serial = status.board_id.as_deref().ok_or("Missing board identity")?;
    let known = engine.settings.droids.iter().any(|p| p.serial.eq_ignore_ascii_case(serial));
    if known { return engine.select_droid(Some(serial), None); }
    let identity = status.identity.as_ref().ok_or("Choose your model once to connect this droid")?;
    if !["connected", "display_error"].contains(&status.state.as_str())
        || !FACTORY_MODELS.contains(&identity.model.as_str())
        || identity.hardware_profile != HARDWARE_PROFILE || identity.generation == 0 {
        return Err("The droid's factory identity could not be verified".into());
    }
    engine.select_droid(Some(serial), Some(&identity.model))
}

/// Registry hints select a local profile, never fabricate firmware identity or
/// authorization. A saved profile or the board's own identity always wins.
pub fn adopt_hint(engine: &mut Engine, status: &ConnectionSnapshot, model: &str) -> Result<(), String> {
    if engine.settings.preferences.offline_mode || engine.settings.selected_device_serial.is_some()
        || status.identity.is_some() || !FACTORY_MODELS.contains(&model)
        || !["connected", "display_error"].contains(&status.state.as_str()) {
        return Err("Choose this droid's model once to connect it.".into());
    }
    let serial = status.board_id.as_deref().filter(|id| crate::droid_registry::valid_id(id)).ok_or("Invalid droid ID")?;
    if engine.settings.droids.iter().any(|d| d.serial.eq_ignore_ascii_case(serial)) {
        engine.select_droid(Some(serial),None)
    } else { engine.select_droid(Some(serial),Some(model)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connection::DeviceIdentity, core::Settings};
    fn status(model: &str, serial: &str) -> ConnectionSnapshot {
        ConnectionSnapshot { state: "connected".into(), board_id: Some(serial.into()),
            firmware: Some("1.3.2".into()), display_health: Some("ready".into()),
            identity: Some(DeviceIdentity {model: model.into(), hardware_profile: HARDWARE_PROFILE.into(), generation: 1}),
            ..ConnectionSnapshot::default() }
    }
    #[test]
    fn all_factory_models_onboard_without_a_model_question() {
        for model in FACTORY_MODELS {
            let mut engine = Engine::new(Settings::default(), Instant::now());
            adopt(&mut engine, &status(model, "BOARD_A")).unwrap();
            assert_eq!(engine.settings.preferences.bot_model, model);
            assert_eq!(engine.settings.droids.len(), 1);
            assert_eq!(engine.settings.selected_device_serial.as_deref(), Some("BOARD_A"));
        }
    }
    #[test]
    fn registry_hints_cannot_replace_factory_identity_or_saved_settings() {
        let mut engine=Engine::new(Settings::default(),Instant::now());
        let mut board=status("meep","0123456789ABCDEF");
        assert!(adopt_hint(&mut engine,&board,"claw").is_err());
        board.identity=None;
        adopt_hint(&mut engine,&board,"meep").unwrap();
        engine.settings.brightness=31;
        engine.select_droid(None,None).unwrap();
        adopt_hint(&mut engine,&board,"claw").unwrap();
        assert_eq!(engine.settings.preferences.bot_model,"meep");
        assert_eq!(engine.settings.brightness,31);
        assert!(board.identity.is_none(),"a lookup is not factory identity");
    }
    #[test]
    fn offline_or_unsupported_registry_hints_never_create_profiles() {
        let mut engine=Engine::new(Settings::default(),Instant::now());
        let mut board=status("boo","0123456789ABCDEF");board.identity=None;
        assert!(adopt_hint(&mut engine,&board,"other").is_err());
        engine.settings.preferences.offline_mode=true;
        assert!(adopt_hint(&mut engine,&board,"boo").is_err());
        assert!(engine.settings.droids.is_empty());
    }
    #[test]
    fn remembering_a_droid_preserves_edits_and_new_boards_are_isolated() {
        let mut engine = Engine::new(Settings::default(), Instant::now());
        adopt(&mut engine, &status("meep", "BOARD_A")).unwrap();
        engine.settings.brightness = 33;
        engine.settings.widgets[0].label = "My own face".into();
        engine.select_droid(None, None).unwrap();
        adopt(&mut engine, &status("nimbus", "BOARD_B")).unwrap();
        assert_eq!(engine.settings.brightness, 85);
        assert_ne!(engine.settings.widgets[0].label, "My own face");
        engine.select_droid(None, None).unwrap();
        adopt(&mut engine, &status("meep", "board_a")).unwrap();
        assert_eq!(engine.settings.brightness, 33);
        assert_eq!(engine.settings.widgets[0].label, "My own face");
    }
    #[test]
    fn no_identity_or_selected_droid_never_silently_changes_model() {
        let mut engine = Engine::new(Settings::default(), Instant::now());
        let mut legacy = status("meep", "BOARD_A"); legacy.identity = None;
        assert!(adopt(&mut engine, &legacy).is_err());
        assert!(engine.settings.droids.is_empty());
        adopt(&mut engine, &status("meep", "BOARD_A")).unwrap();
        assert!(adopt(&mut engine, &status("claw", "BOARD_B")).is_err());
        assert_eq!(engine.settings.preferences.bot_model, "meep");
    }
    #[test]
    fn recovery_never_downgrades_factory_custom_or_swapped_wiring() {
        for (firmware, profile, provisioning, identity) in [
            ("1.3.2", Some("GP4-GP5"), false, false),
            ("1.3.2-mistake", Some("GP5-SDA-GP4-SCL"), true, true),
            ("1.2.4", Some("GP5-SDA-GP4-SCL"), false, false),
            ("custom", None, false, false),
            ("1.2.4", None, false, true),
            ("1.2.4", None, true, false),
        ] { assert!(managed_firmware(firmware, profile, provisioning, identity)); }
        for firmware in ["1.0.2", "1.2.3", "1.2.4"] {
            assert!(!managed_firmware(firmware, Some("GP4-GP5"), false, false));
        }
        assert!(older_firmware("1.2.3"));
        assert!(!older_firmware("1.2.4"));
        assert!(!older_firmware("1.3.2-mistake"));
        assert!(allow_customer_reflash(&ConnectionSnapshot::default()).is_err());
    }
    #[test]
    fn transient_hello_failure_retries_after_backoff() {
        let mut probes = ProbeSchedule::default();
        let now = Instant::now();
        probes.refresh(0, ["board"].into_iter());
        assert!(probes.ready("board", now));
        probes.record_failure("board", now);
        assert!(!probes.ready("board", now + Duration::from_millis(1999)));
        assert!(probes.ready("board", now + Duration::from_secs(2)));
        probes.record_success("board");
        assert!(!probes.ready("board", now + Duration::from_secs(30)));
    }

    #[test]
    fn three_failed_probes_require_a_manual_scan_to_retry() {
        let mut probes = ProbeSchedule::default();
        let now = Instant::now();
        probes.record_failure("board", now);
        probes.record_failure("board", now + Duration::from_secs(2));
        assert!(!probes.ready("board", now + Duration::from_millis(5999)));
        assert!(probes.ready("board", now + Duration::from_secs(6)));
        probes.record_failure("board", now + Duration::from_secs(6));
        probes.refresh(0, ["board"].into_iter());
        assert!(!probes.ready("board", now + Duration::from_secs(3600)));
        probes.refresh(1, ["board"].into_iter());
        assert!(probes.ready("board", now + Duration::from_secs(3600)));
    }

    #[test]
    fn successful_legacy_hello_is_not_reprobed_by_passive_polls() {
        let mut probes = ProbeSchedule::default();
        let now = Instant::now();
        // A successful HELLO is definitive even when it has no factory model.
        probes.record_success("legacy-board");
        for seconds in [1, 2, 30, 3600] {
            probes.refresh(0, ["legacy-board"].into_iter());
            assert!(!probes.ready("legacy-board", now + Duration::from_secs(seconds)));
        }
        probes.refresh(1, ["legacy-board"].into_iter());
        assert!(probes.ready("legacy-board", now));
    }

    #[test]
    fn unplug_clears_success_and_failure_history_only_for_absent_boards() {
        let mut probes = ProbeSchedule::default();
        let now = Instant::now();
        probes.record_success("legacy-board");
        probes.record_failure("starting-board", now);
        probes.refresh(0, ["legacy-board"].into_iter());
        assert!(!probes.ready("legacy-board", now));
        assert!(probes.ready("starting-board", now));
        probes.refresh(0, [].into_iter());
        probes.refresh(0, ["legacy-board", "starting-board"].into_iter());
        assert!(probes.ready("legacy-board", now));
        assert!(probes.ready("starting-board", now));
    }

}
