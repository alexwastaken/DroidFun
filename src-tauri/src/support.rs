//! Customer support checks use allowlisted facts, never logs or screen contents.
use crate::{connection::ConnectionSnapshot, core::Snapshot};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Symptom {
    NoPower,
    BlankScreen,
    Disconnects,
    Screens,
    Other,
}
#[derive(Clone, Copy, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Observation {
    Visible,
    Blank,
    #[default]
    NotChecked,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub code: &'static str,
    pub status: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
}
#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VisualTest {
    pub requested: bool,
    pub acknowledged: bool,
    pub observation: Observation,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub format_version: u8,
    pub report_id: String,
    pub created_at: u64,
    pub app_version: &'static str,
    pub os: &'static str,
    pub architecture: &'static str,
    pub symptom: Symptom,
    pub board_id: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub wiring: Option<String>,
    pub connection_state: String,
    pub model_source: &'static str,
    pub standalone_supported: bool,
    pub saved_screens: u8,
    pub runtime_devices: usize,
    pub recovery_devices: usize,
    pub enumeration_complete: bool,
    pub observed_seconds: u8,
    pub connection_changes: usize,
    pub acknowledged_commands: u64,
    pub brightness: u8,
    pub screen_on: bool,
    pub enabled_screens: usize,
    pub unavailable_sources: usize,
    pub checks: Vec<Check>,
    pub next_steps: Vec<&'static str>,
    pub can_test_screen: bool,
    pub can_reinstall: bool,
    pub visual_test: VisualTest,
    pub privacy: &'static str,
}
pub struct Facts {
    pub runtime_devices: usize,
    pub recovery_devices: usize,
    pub enumeration_complete: bool,
    pub target_present: bool,
    pub acknowledged_commands: u64,
    pub connection_changes: usize,
    pub target_changed: bool,
}
fn bounded(value: Option<&str>, limit: usize) -> Option<String> {
    value
        .filter(|v| {
            !v.is_empty()
                && v.len() <= limit
                && v.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c))
        })
        .map(str::to_owned)
}
pub fn build(
    id: String,
    symptom: Symptom,
    c: &ConnectionSnapshot,
    s: &Snapshot,
    f: Facts,
) -> Report {
    let mut checks = Vec::new();
    let mut add = |code, status, title, detail| {
        checks.push(Check {
            code,
            status,
            title,
            detail,
        })
    };
    if !f.enumeration_complete {
        add(
            "DF-USB-05",
            "warning",
            "USB search could not finish",
            "Try another direct computer port, reconnect the cable, then run diagnostics again.",
        );
    } else if f.recovery_devices > 0 {
        add("DF-USB-02", "attention", "Droid is in recovery mode", "Unplug and reconnect normally. If it stays in recovery mode, save this report for support; its installed wiring cannot be verified here.");
    } else if f.runtime_devices == 0 {
        add("DF-USB-01", "attention", "No USB data connection found", "Try another USB data cable and a direct computer port. A power-only cable can light a droid without letting the app find it.");
    } else if f.runtime_devices > 1 && !f.target_present {
        add("DF-USB-03", "warning", "Choose one droid", "Connect one droid at a time or select it under Your droids, then run diagnostics again.");
    } else if !f.target_present {
        add(
            "DF-USB-04",
            "attention",
            "Selected droid is not connected",
            "Reconnect the selected droid or choose the attached droid under Your droids.",
        );
    } else {
        add(
            "DF-USB-00",
            "passed",
            "USB data device found",
            "The selected droid is visible to this computer.",
        );
    }
    let verified = f.target_present
        && !f.target_changed
        && bounded(c.board_id.as_deref(), 128).is_some()
        && c.firmware.is_some()
        && ["connected", "display_error"].contains(&c.state.as_str());
    if c.state == "permission_needed" {
        add("DF-LINK-02", "attention", "USB permission needed", "Use Allow USB access under Connection, then reconnect the droid and run diagnostics again.");
    } else if f.target_changed {
        add(
            "DF-LINK-04",
            "warning",
            "Droid selection changed",
            "Keep one droid selected throughout the check, then run it again.",
        );
    } else if verified && f.acknowledged_commands > 0 {
        add("DF-LINK-00", "passed", "Droid replied during this check", "The app received fresh replies. This proves USB communication, not that pixels are physically visible.");
    } else if f.target_present {
        add("DF-LINK-01", "attention", "No fresh DroidFun reply", "Close other apps using the droid, try another data cable and reconnect. Do not assume firmware is missing from a failed reply.");
    } else {
        add(
            "DF-LINK-03",
            "not_checked",
            "Droid reply could not be checked",
            "A working USB data connection is needed to check the droid software.",
        );
    }
    if f.connection_changes >= 3 {
        add("DF-LINK-05", "warning", "Connection changed repeatedly", "Compare a different data cable and direct USB port. If it keeps disconnecting, send this report to support.");
    }
    if verified && c.display_health.as_deref() == Some("ready") {
        add("DF-SCREEN-00", "passed", "Screen controller reports ready", "Use Show test pattern and tell us whether you can see it. Software cannot confirm physical visibility.");
    } else if verified && c.display_health.as_deref().is_some_and(|v| v != "ready") {
        add("DF-SCREEN-01", "attention", "Screen controller reports a fault", "USB works but the screen cannot be confirmed. A cable, display connection or hardware fault may need service; reinstalling cannot repair hardware.");
    } else {
        add("DF-SCREEN-02", "not_checked", "Screen controller not checked", "Reconnect the droid first. No screen-health result is assumed while it is disconnected.");
    }
    if !s.display_on || s.host_display_asleep {
        add("DF-DISPLAY-01", "warning", "Display is currently resting or off", "Check Screen on, quiet hours and computer sleep. Show test pattern temporarily bypasses these settings without changing them.");
    }
    if s.settings.brightness < 10 {
        add("DF-DISPLAY-02", "warning", "Brightness is very low", "Increase Brightness under Display. The temporary test pattern uses a visible brightness.");
    }
    let enabled = s.settings.widgets.iter().filter(|w| w.enabled).count();
    if enabled == 0 {
        add("DF-PLAY-01", "warning", "No screens are enabled", "Enable a screen in Your screens. A face or custom text works without internet on Nimbus, Boo and Meep.");
    }
    if s.playback == crate::core::Playback::Paused {
        add(
            "DF-PLAY-02",
            "warning",
            "Screen cycle is paused",
            "Choose Resume cycle to move through the playlist again.",
        );
    }
    if s.settings.preferences.offline_mode {
        add("DF-DATA-01", "warning", "Online data is paused", "Turn off Pause online data for weather, stocks, account usage and app updates. Local screens and diagnostics still work.");
    }
    let unavailable = s
        .data
        .values()
        .filter(|v| !["fresh", "demo", "ready"].contains(&v.state.as_str()))
        .count();
    if unavailable > 0 {
        add("DF-DATA-02", "warning", "Some screen data is unavailable", "Open Edit for the affected screen and check its data source or sign-in. A source issue does not mean the droid firmware is broken.");
    }
    let can_reinstall = verified
        && f.acknowledged_commands > 0
        && f.enumeration_complete
        && f.runtime_devices == 1
        && f.recovery_devices == 0
        && c.firmware_recovery_supported
        && crate::onboarding::allow_customer_reflash(c).is_ok();
    let mut next_steps: Vec<_> = checks
        .iter()
        .filter(|c| c.status == "attention" || c.status == "warning")
        .take(4)
        .map(|c| c.detail)
        .collect();
    if next_steps.is_empty() {
        next_steps.push("USB and reported screen checks look good. If the display still looks wrong, show the test pattern and record what you see.");
    }
    next_steps.push("Save this reviewed report and attach it to your message to DroidFun support. Nothing is sent automatically.");
    Report { format_version: 2, report_id: id, created_at: crate::core::unix_now(), app_version: env!("CARGO_PKG_VERSION"), os: std::env::consts::OS, architecture: std::env::consts::ARCH,
        symptom, board_id: bounded(c.board_id.as_deref(),128), model: bounded(c.identity.as_ref().map(|i|i.model.as_str()).or(Some(s.settings.preferences.bot_model.as_str())),16), firmware: bounded(c.firmware.as_deref(),64), wiring: bounded(c.pin_profile.as_deref(),32),
        connection_state: if ["connected","display_error","permission_needed","reconnecting","preview","setup_needed","choose_device","needs_model","identifying","connecting","diagnostic","discovery_error"].contains(&c.state.as_str()) { c.state.clone() } else { "unknown".into() },
        model_source: if c.identity.is_some() {"droid_last_reported"} else {"app_selection"}, standalone_supported:c.standalone_supported, saved_screens:c.standalone_count,
        runtime_devices: f.runtime_devices, recovery_devices: f.recovery_devices, enumeration_complete: f.enumeration_complete, observed_seconds: 6, connection_changes:f.connection_changes, acknowledged_commands:f.acknowledged_commands,
        brightness:s.settings.brightness, screen_on:s.display_on, enabled_screens:enabled, unavailable_sources:unavailable, checks,next_steps,
        can_test_screen:verified && f.acknowledged_commands > 0 && f.enumeration_complete && f.recovery_devices == 0, can_reinstall, visual_test:VisualTest::default(),
        privacy:"Includes droid hardware ID, app/system version, firmware/wiring, technical check results and your test observation. Excludes credentials, account details, names, custom text/images, locations, stock symbols and local file paths. Saved locally; never sent automatically." }
}

pub fn record_observation(report: &mut Report, observation: Observation) -> Result<(), String> {
    if !report.visual_test.requested {
        return Err("Show a test pattern before recording its result.".into());
    }
    report.visual_test.observation = observation;
    report.checks.retain(|c| !c.code.starts_with("DF-VISUAL"));
    match observation {
        Observation::Visible=>report.checks.push(Check {code:"DF-VISUAL-00",status:"passed",title:"You confirmed a visible test pattern",detail:"The physical screen showed the checkerboard. Review screen settings or the affected data source if normal screens still look wrong."}),
        Observation::Blank=>report.checks.push(Check {code:"DF-VISUAL-01",status:"attention",title:"You could not see the test pattern",detail:"Save this report and contact support. A screen, power or internal wiring fault may need service even when USB replies succeed."}),
        Observation::NotChecked=>{},
    }
    Ok(())
}

#[derive(Clone)]
pub struct Pattern {
    // The live worker retains the USB exclusion while a native write is in flight.
    pub _operation: Option<Arc<crate::UsbOperationGuard>>,
    pub board_id: String,
    pub expires: Instant,
    pub acknowledged: Arc<AtomicBool>,
}
impl Pattern {
    pub fn active(&self, serial: &str, now: Instant) -> bool {
        self.board_id.eq_ignore_ascii_case(serial) && now < self.expires
    }
    pub fn acknowledge(&self) {
        self.acknowledged.store(true, Ordering::SeqCst);
    }
}
pub fn test_frame() -> Vec<u8> {
    let mut frame = vec![0u8; 1024];
    for y in 0..64 {
        for x in 0..128 {
            if x == 0
                || x == 127
                || y == 0
                || y == 63
                || (x > 7 && x < 120 && y > 7 && y < 56 && ((x / 8 + y / 8) % 2 == 0))
            {
                frame[(y / 8) * 128 + x] |= 1 << (y % 8);
            }
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    fn facts() -> Facts {
        Facts {
            runtime_devices: 1,
            recovery_devices: 0,
            enumeration_complete: true,
            target_present: true,
            acknowledged_commands: 3,
            connection_changes: 0,
            target_changed: false,
        }
    }
    fn snapshot() -> Snapshot {
        crate::core::Engine::new(crate::core::Settings::default(), Instant::now()).snapshot()
    }
    fn connection() -> ConnectionSnapshot {
        ConnectionSnapshot {
            state: "connected".into(),
            board_id: Some("ABC123".into()),
            firmware: Some("1.2.4".into()),
            pin_profile: Some("GP4-GP5".into()),
            display_health: Some("ready".into()),
            firmware_recovery_supported: true,
            ..Default::default()
        }
    }
    #[test]
    fn fresh_checks_do_not_claim_visibility_or_repair_unknown_boards() {
        let mut f = facts();
        f.acknowledged_commands = 0;
        let r = build(
            "test".into(),
            Symptom::NoPower,
            &connection(),
            &snapshot(),
            f,
        );
        assert!(!r.can_test_screen);
        assert!(r.checks.iter().any(|c| c.code == "DF-LINK-01"));
        let mut f = facts();
        f.target_changed = true;
        assert!(!build("test".into(), Symptom::Other, &connection(), &snapshot(), f).can_reinstall);
        let mut c = connection();
        c.firmware = None;
        let r = build("test".into(), Symptom::Other, &c, &snapshot(), facts());
        assert!(!r.can_reinstall && !r.can_test_screen);
    }
    #[test]
    fn factory_and_swapped_wiring_never_offer_generic_recovery() {
        for firmware in ["1.3.2", "1.3.2-mistake", "custom"] {
            let mut c = connection();
            c.firmware = Some(firmware.into());
            let r = build(
                "test".into(),
                Symptom::BlankScreen,
                &c,
                &snapshot(),
                facts(),
            );
            assert!(!r.can_reinstall);
            assert!(r.can_test_screen);
        }
        let mut f = facts();
        f.recovery_devices = 1;
        let r = build(
            "test".into(),
            Symptom::NoPower,
            &connection(),
            &snapshot(),
            f,
        );
        assert!(!r.can_reinstall && !r.can_test_screen);
        assert!(r.checks.iter().any(|c| c.code == "DF-USB-02"));
    }
    #[test]
    fn report_omits_custom_content_errors_paths_and_accounts() {
        let mut s = snapshot();
        s.settings.widgets[0].label = "SECRET NAME".into();
        s.settings.widgets[0].config.custom_text = "SECRET TEXT".into();
        s.settings.widgets[0].config.image_pixels = "ab".repeat(1024);
        let mut c = connection();
        c.detail = "SECRET PATH /Users/private".into();
        c.firmware = Some("bad /Users/private".into());
        let json =
            serde_json::to_string(&build("test".into(), Symptom::Other, &c, &s, facts())).unwrap();
        for secret in ["SECRET", "/Users/", &"ab".repeat(1024)] {
            assert!(!json.contains(secret));
        }
        assert!(json.contains("ABC123"));
    }
    #[test]
    fn missing_usb_and_display_policy_have_actionable_codes() {
        let mut s = snapshot();
        s.display_on = false;
        s.settings.brightness = 0;
        let mut f = facts();
        f.runtime_devices = 0;
        f.target_present = false;
        let r = build(
            "test".into(),
            Symptom::NoPower,
            &ConnectionSnapshot::default(),
            &s,
            f,
        );
        for code in [
            "DF-USB-01",
            "DF-SCREEN-02",
            "DF-DISPLAY-01",
            "DF-DISPLAY-02",
        ] {
            assert!(r.checks.iter().any(|c| c.code == code));
        }
        assert!(!r.can_test_screen && !r.can_reinstall);
    }
    #[test]
    fn pattern_is_bounded_and_expires_for_the_same_board_only() {
        let now = Instant::now();
        let p = Pattern {
            _operation: None,
            board_id: "ABC".into(),
            expires: now + std::time::Duration::from_secs(8),
            acknowledged: Arc::new(AtomicBool::new(false)),
        };
        assert!(p.active("abc", now));
        assert!(!p.active("other", now));
        assert!(!p.active("ABC", p.expires));
        let f = test_frame();
        assert_eq!(f.len(), 1024);
        assert_eq!(f[0], 255);
        assert_eq!(f[127], 255);
        assert_ne!(f[128 + 16], f[128 + 24]);
    }
    #[test]
    fn recovery_and_physical_observation_require_current_evidence() {
        let mut r = build(
            "test".into(),
            Symptom::BlankScreen,
            &connection(),
            &snapshot(),
            facts(),
        );
        assert!(r.can_reinstall);
        assert!(record_observation(&mut r, Observation::Visible).is_err());
        r.visual_test.requested = true;
        r.visual_test.acknowledged = true;
        record_observation(&mut r, Observation::Visible).unwrap();
        record_observation(&mut r, Observation::Blank).unwrap();
        assert_eq!(
            r.checks
                .iter()
                .filter(|c| c.code.starts_with("DF-VISUAL"))
                .count(),
            1
        );
        assert!(r
            .checks
            .iter()
            .any(|c| c.code == "DF-VISUAL-01" && c.status == "attention"));
        let mut f = facts();
        f.enumeration_complete = false;
        let r = build("test".into(), Symptom::Other, &connection(), &snapshot(), f);
        assert!(!r.can_reinstall && !r.can_test_screen);
        assert!(r.checks.iter().any(|c| c.code == "DF-USB-05"));
        let mut c = connection();
        c.state = "permission_needed".into();
        let r = build("test".into(), Symptom::Other, &c, &snapshot(), facts());
        assert!(r.checks.iter().any(|c| c.code == "DF-LINK-02"));
        assert!(!r.can_reinstall);
    }
}
