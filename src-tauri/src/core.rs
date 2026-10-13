use font8x8::{UnicodeFonts, BASIC_FONTS};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 64;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT / 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Widget {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub enabled: bool,
    pub duration_sec: u16,
    pub headline: String,
    pub footer: String,
    #[serde(default)]
    pub config: SourceConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub region: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourceConfig {
    pub mode: String,
    pub location: Option<Location>,
    pub unit: String,
    pub symbol: String,
    pub currency: String,
    pub limit_id: String,
    pub window: String,
    pub session_id: String,
    pub cli_path: String,
    pub claude_source: String,
    pub claude_profile: String,
    pub claude_report_directory: String,
    pub claude_report_profile: String,
    pub face_expression: String,
    pub blink: bool,
    pub animation_speed: String,
    pub weather_layout: String,
    pub show_change: bool,
    pub clock_zone: String,
    pub clock_format: String,
    pub clock_seconds: bool,
    pub clock_date: String,
    pub focus_minutes: u16,
    pub break_minutes: u16,
    pub long_break_minutes: u16,
    pub focus_sessions: u8,
    pub timer_running: bool,
    pub timer_started_at: u64,
    pub timer_elapsed: u64,
    pub meeting_starts_at: u64,
    pub meeting_minutes: u16,
    pub monitor_interval: u8,
    pub show_graph: bool,
    pub memory_metric: String,
    pub memory_unit: String,
}
impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            mode: "demo".into(),
            location: None,
            unit: "fahrenheit".into(),
            symbol: String::new(),
            currency: "USD".into(),
            limit_id: "codex".into(),
            window: "primary".into(),
            session_id: String::new(),
            cli_path: String::new(),
            claude_source: "account".into(),
            claude_profile: String::new(),
            claude_report_directory: String::new(),
            claude_report_profile: "default".into(),
            face_expression: "happy".into(),
            blink: true,
            animation_speed: "normal".into(),
            weather_layout: "current".into(),
            show_change: false,
            clock_zone: "local".into(),
            clock_format: "12".into(),
            clock_seconds: false,
            clock_date: "weekday".into(),
            focus_minutes: 25,
            break_minutes: 5,
            long_break_minutes: 15,
            focus_sessions: 4,
            timer_running: false,
            timer_started_at: 0,
            timer_elapsed: 0,
            meeting_starts_at: 0,
            meeting_minutes: 30,
            monitor_interval: 2,
            show_graph: true,
            memory_metric: "used".into(),
            memory_unit: "gib".into(),
        }
    }
}
impl SourceConfig {
    pub fn validate(&self, kind: &str) -> Result<(), String> {
        crate::local_widgets::validate(self)?;
        if crate::local_widgets::local(kind) && self.mode != "live" {
            return Err("Local widgets use the computer's actual time and readings".into());
        }
        if !["demo", "live"].contains(&self.mode.as_str())
            || !["celsius", "fahrenheit"].contains(&self.unit.as_str())
            || self.symbol.len() > 20
            || !self
                .symbol
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-^".contains(&c))
            || self.currency.len() != 3
            || !self.currency.bytes().all(|c| c.is_ascii_uppercase())
            || self.limit_id.len() > 128
            || self.session_id.len() > 128
            || !["account", "statusline"].contains(&self.claude_source.as_str())
            || !["happy", "neutral", "sleepy", "surprised"].contains(&self.face_expression.as_str())
            || !["slow", "normal", "quick"].contains(&self.animation_speed.as_str())
            || !["current", "high_low"].contains(&self.weather_layout.as_str())
            || self.claude_profile.len() > 1024
            || self.claude_report_directory.len() > 1024
            || self.claude_report_profile.is_empty()
            || self.claude_report_profile.len() > 128
            || (!self.claude_profile.is_empty()
                && !std::path::Path::new(&self.claude_profile).is_absolute())
            || (!self.claude_report_directory.is_empty()
                && !std::path::Path::new(&self.claude_report_directory).is_absolute())
            || self.cli_path.len() > 1024
            || !["primary", "secondary", "five_hour", "seven_day"].contains(&self.window.as_str())
        {
            return Err("Invalid data-source settings".into());
        }
        if let Some(l) = &self.location {
            if l.name.trim().is_empty()
                || l.name.len() > 128
                || !l.latitude.is_finite()
                || !l.longitude.is_finite()
                || !(-90.0..=90.0).contains(&l.latitude)
                || !(-180.0..=180.0).contains(&l.longitude)
            {
                return Err("Choose a valid weather location".into());
            }
        }
        if self.mode == "live"
            && ((kind == "weather" && self.location.is_none())
                || (kind == "stock" && self.symbol.is_empty()))
        {
            return Err("Configure the location or symbol first".into());
        }
        Ok(())
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCard {
    pub source_id: String,
    pub scope_id: String,
    pub observed_at: u64,
    pub fetched_at: u64,
    pub stale_at: u64,
    pub expires_at: u64,
    pub state: String,
    pub headline: String,
    pub footer: String,
    pub detail: String,
    pub values: serde_json::Value,
}
impl DataCard {
    pub fn state_at(&self, now: u64) -> &str {
        if ["fresh", "stale"].contains(&self.state.as_str()) {
            if now >= self.expires_at {
                "unavailable"
            } else if now >= self.stale_at {
                "stale"
            } else {
                &self.state
            }
        } else {
            &self.state
        }
    }
    pub fn usable(&self, now: u64) -> bool {
        ["fresh", "stale"].contains(&self.state_at(now))
    }
    pub fn summary(&self, now: u64) -> Self {
        let mut card = self.clone();
        card.state = self.state_at(now).into();
        if !card.usable(now) && ["fresh", "stale"].contains(&self.state.as_str()) {
            card.headline = "WAITING".into();
            card.footer = "UPDATING".into();
            card.detail =
                "The snapshot expired or its allowance reset. Waiting for a new report.".into();
        }
        card
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QuietHours {
    pub enabled: bool,
    pub start_minute: u16,
    pub end_minute: u16,
}
impl Default for QuietHours {
    fn default() -> Self {
        Self {
            enabled: false,
            start_minute: 1320,
            end_minute: 420,
        }
    }
}
impl QuietHours {
    pub fn contains(&self, minute: u16) -> bool {
        self.enabled
            && if self.start_minute < self.end_minute {
                (self.start_minute..self.end_minute).contains(&minute)
            } else {
                minute >= self.start_minute || minute < self.end_minute
            }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub orientation: u16,
    pub quiet_hours: QuietHours,
    pub appearance: String,
    pub launch_at_login: bool,
    pub window_mode: String,
    pub setup_complete: bool,
    pub companion_face: bool,
    pub companion_animation: bool,
    pub companion_speed: String,
    pub companion_glances: bool,
    pub companion_mouth: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            orientation: 0,
            quiet_hours: QuietHours::default(),
            appearance: "system".into(),
            launch_at_login: false,
            window_mode: "auto".into(),
            setup_complete: false,
            companion_face: true,
            companion_animation: true,
            companion_speed: "quick".into(),
            companion_glances: true,
            companion_mouth: true,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if ![0, 180].contains(&self.orientation)
            || !["system", "light", "dark"].contains(&self.appearance.as_str())
            || !["auto", "window", "background"].contains(&self.window_mode.as_str())
            || !["slow", "normal", "quick"].contains(&self.companion_speed.as_str())
            || self.quiet_hours.start_minute >= 1440
            || self.quiet_hours.end_minute >= 1440
            || (self.quiet_hours.enabled
                && self.quiet_hours.start_minute == self.quiet_hours.end_minute)
        {
            return Err(
                "Choose a valid orientation, appearance, and distinct quiet-hours times".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u8,
    pub widgets: Vec<Widget>,
    pub brightness: u8,
    #[serde(default)]
    pub selected_device_serial: Option<String>,
    #[serde(default)]
    pub preferences: Preferences,
    #[serde(default)]
    pub manual_display_off: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let widget = |id: &str,
                      kind: &str,
                      label: &str,
                      duration_sec: u16,
                      headline: &str,
                      footer: &str,
                      enabled: bool| Widget {
            id: id.into(),
            kind: kind.into(),
            label: label.into(),
            enabled,
            duration_sec,
            headline: headline.into(),
            footer: footer.into(),
            config: SourceConfig::default(),
        };
        Self {
            version: 1,
            brightness: 85,
            selected_device_serial: None,
            preferences: Preferences::default(),
            manual_display_off: false,
            widgets: vec![
                widget("face", "face", "Happy face", 12, "", "", true),
                widget(
                    "weather",
                    "weather",
                    "Weather",
                    10,
                    "68 F",
                    "PARTLY CLOUDY",
                    false,
                ),
                widget(
                    "stock",
                    "stock",
                    "Stocks",
                    8,
                    "123.45",
                    "+1.20% CLOSE",
                    false,
                ),
                widget(
                    "codex",
                    "codex",
                    "Codex usage",
                    8,
                    "75%",
                    "5H LEFT · DEMO",
                    false,
                ),
                widget(
                    "claude",
                    "claude",
                    "Claude usage",
                    8,
                    "58%",
                    "7D LEFT · DEMO",
                    false,
                ),
            ],
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.widgets.is_empty() || self.widgets.len() > 30 {
            return Err("Unsupported settings version or playlist size".into());
        }
        self.preferences.validate()?;
        if self.brightness > 100 {
            return Err("Invalid brightness".into());
        }
        if self
            .selected_device_serial
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 128)
        {
            return Err("Invalid selected device".into());
        }
        let mut ids = std::collections::HashSet::new();
        for widget in &self.widgets {
            widget.config.validate(&widget.kind)?;
            if widget.id.is_empty()
                || widget.id.len() > 128
                || !ids.insert(&widget.id)
                || !(3..=120).contains(&widget.duration_sec)
            {
                return Err("Invalid widget ID or duration".into());
            }
            if ![
                "face", "weather", "stock", "codex", "claude", "clock", "pomodoro", "meeting",
                "cpu", "memory",
            ]
            .contains(&widget.kind.as_str())
            {
                return Err("Unknown widget type".into());
            }
            if widget.label.chars().count() > 24
                || widget.headline.chars().count() > 24
                || widget.footer.chars().count() > 24
            {
                return Err("Widget text is too long".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Playback {
    Running,
    Paused,
    Pinned,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: Settings,
    pub current_id: String,
    pub current_label: String,
    pub playback: Playback,
    pub remaining_sec: u16,
    pub total_sec: u32,
    pub display_on: bool,
    pub temporary: bool,
    pub display_reason: String,
    pub quiet_active: bool,
    pub quiet_override_until: u64,
    pub frame: Vec<u8>,
    pub animation_phase: u64,
    pub data: HashMap<String, DataCard>,
}

#[derive(Clone)]
struct Temporary {
    saved_id: String,
    saved_remaining_ms: u64,
    saved_playback: Playback,
}

#[derive(Clone)]
pub struct Engine {
    pub settings: Settings,
    current_id: String,
    remaining_ms: u64,
    playback: Playback,
    display_on: bool,
    temporary: Option<Temporary>,
    last_tick: Instant,
    data: HashMap<String, DataCard>,
    quiet_active: bool,
    quiet_override_until: u64,
    phase_ms: u64,
    reduced_motion: bool,
    local_second: u64,
}

impl Engine {
    pub fn new(settings: Settings, now: Instant) -> Self {
        let current_id = settings
            .widgets
            .iter()
            .find(|w| w.enabled)
            .map(|w| w.id.clone())
            .unwrap_or_else(|| "face".into());
        let remaining_ms = settings
            .widgets
            .iter()
            .find(|w| w.id == current_id)
            .map(|w| w.duration_sec as u64 * 1000)
            .unwrap_or(12_000);
        let display_on = !settings.manual_display_off;
        Self {
            settings,
            current_id,
            remaining_ms,
            playback: Playback::Running,
            display_on,
            temporary: None,
            last_tick: now,
            data: HashMap::new(),
            quiet_active: false,
            quiet_override_until: 0,
            phase_ms: 0,
            reduced_motion: false,
            local_second: 0,
        }
    }

    fn current_widget(&self) -> Option<&Widget> {
        self.settings
            .widgets
            .iter()
            .find(|w| w.id == self.current_id)
    }

    fn eligible(&self, w: &Widget) -> bool {
        w.enabled
            && (w.kind == "face"
                || crate::local_widgets::dynamic(&w.kind)
                || w.config.mode == "demo"
                || self.data.get(&w.id).is_some_and(|d| d.usable(unix_now())))
    }
    fn enabled(&self, id: &str) -> bool {
        self.settings
            .widgets
            .iter()
            .any(|w| w.id == id && self.eligible(w))
    }
    pub fn render_widget(&self, id: &str) -> Vec<u8> {
        self.render_current(id, true)
    }
    fn animation_state(&self) -> CompanionPose {
        if self.reduced_motion
            || !self.settings.preferences.companion_animation
            || self.playback == Playback::Paused
            || self.quiet_active
            || !self.display_on
        {
            return CompanionPose::default();
        }
        CompanionPose::at(
            self.phase_ms,
            &self.settings.preferences.companion_speed,
            self.settings.preferences.companion_glances,
            self.settings.preferences.companion_mouth,
        )
    }
    fn face_closed(&self, w: &Widget) -> bool {
        let period = match w.config.animation_speed.as_str() {
            "slow" => 6000,
            "quick" => 2400,
            _ => 3600,
        };
        !self.reduced_motion
            && self.playback != Playback::Paused
            && w.config.blink
            && self.phase_ms % period >= period - 200
    }
    fn current_animation_state(&self) -> CompanionPose {
        match self.current_widget() {
            Some(w) if w.kind == "face" => {
                let mut pose = self.animation_state();
                pose.closed = self.face_closed(w);
                pose
            }
            Some(_) if self.settings.preferences.companion_face => self.animation_state(),
            _ => CompanionPose::default(),
        }
    }
    fn render_current(&self, id: &str, on: bool) -> Vec<u8> {
        let Some(mut w) = self.settings.widgets.iter().find(|w| w.id == id).cloned() else {
            return render(None, on);
        };
        let mut local_values = serde_json::Value::Null;
        if crate::local_widgets::dynamic(&w.kind) {
            let d = crate::local_widgets::dynamic_card(&w, unix_now());
            w.headline = d.headline;
            w.footer = d.footer;
            local_values = d.values;
        } else if w.kind != "face" && w.config.mode == "live" {
            if let Some(d) = self.data.get(id) {
                let d = d.summary(unix_now());
                local_values = d.values.clone();
                w.headline = d.headline;
                w.footer = if d.state == "stale" {
                    "STALE CACHED".into()
                } else {
                    d.footer
                };
                if w.kind == "stock" {
                    w.label = w.config.symbol.clone();
                }
            } else {
                w.headline = "WAITING".into();
                w.footer = "SETUP / LOADING".into();
            }
        } else if w.kind != "face" {
            w.footer = format!("DEMO {}", w.footer);
        }
        if w.kind == "face" && on {
            let mut f = Frame::new();
            let mut pose = self.animation_state();
            pose.closed = self.face_closed(&w);
            f.animated_face(&w.config.face_expression, pose);
            f.bytes
        } else {
            let companion = self
                .settings
                .preferences
                .companion_face
                .then(|| self.animation_state());
            if crate::local_widgets::local(&w.kind) {
                return render_local(&w, on, companion, &local_values);
            }
            render_card(
                Some(&w),
                on,
                self.settings
                    .preferences
                    .companion_face
                    .then(|| self.animation_state()),
            )
        }
    }
    pub fn set_data(&mut self, id: &str, card: DataCard) {
        self.data.insert(id.into(), card);
        if self.current_id == id && !self.enabled(id) {
            self.temporary = None;
            if self.playback == Playback::Pinned {
                self.playback = Playback::Running;
            }
            self.advance_to_next();
        }
    }
    pub fn configure_source(
        &mut self,
        id: &str,
        label: String,
        mut config: SourceConfig,
    ) -> Result<(), String> {
        config.symbol = config.symbol.trim().to_ascii_uppercase();
        if label.trim().is_empty() || label.chars().count() > 24 {
            return Err("Use a name under 25 characters".into());
        }
        let w = self
            .settings
            .widgets
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or("Widget not found")?;
        config.validate(&w.kind)?;
        if w.kind == "pomodoro" {
            if (
                config.focus_minutes,
                config.break_minutes,
                config.long_break_minutes,
                config.focus_sessions,
            ) == (
                w.config.focus_minutes,
                w.config.break_minutes,
                w.config.long_break_minutes,
                w.config.focus_sessions,
            ) {
                config.timer_running = w.config.timer_running;
                config.timer_started_at = w.config.timer_started_at;
                config.timer_elapsed = w.config.timer_elapsed;
            } else {
                config.timer_running = false;
                config.timer_started_at = 0;
                config.timer_elapsed = 0;
            }
        }
        w.label = label.trim().into();
        w.config = config;
        let local_card = crate::local_widgets::dynamic(&w.kind)
            .then(|| crate::local_widgets::dynamic_card(w, unix_now()));
        self.data.remove(id);
        if let Some(card) = local_card {
            self.data.insert(id.into(), card);
        }
        if self.current_id == id && !self.enabled(id) {
            self.advance_to_next();
        }
        Ok(())
    }
    pub fn add_widget(&mut self, kind: &str) -> Result<(), String> {
        if ![
            "weather", "stock", "codex", "claude", "clock", "pomodoro", "meeting", "cpu", "memory",
        ]
        .contains(&kind)
            || self.settings.widgets.len() >= 30
        {
            return Err("Choose a widget type; limit is 30 widgets".into());
        }
        let id = format!(
            "{kind}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let template = Settings::default()
            .widgets
            .into_iter()
            .find(|w| w.kind == kind)
            .unwrap_or_else(|| Widget {
                id: String::new(),
                kind: kind.into(),
                label: match kind {
                    "clock" => "Clock",
                    "pomodoro" => "Pomodoro",
                    "meeting" => "Meeting",
                    "cpu" => "CPU",
                    "memory" => "Memory",
                    _ => "Widget",
                }
                .into(),
                enabled: true,
                duration_sec: 8,
                headline: String::new(),
                footer: String::new(),
                config: SourceConfig {
                    mode: "live".into(),
                    ..SourceConfig::default()
                },
            });
        self.settings.widgets.push(Widget {
            id,
            enabled: crate::local_widgets::local(kind),
            ..template
        });
        Ok(())
    }
    pub fn remove_widget(&mut self, id: &str) -> Result<(), String> {
        if id == "face" {
            return Err("Keep the built-in face for fallback".into());
        }
        if !self.settings.widgets.iter().any(|w| w.id == id) {
            return Err("Widget not found".into());
        }
        self.settings.widgets.retain(|w| w.id != id);
        self.data.remove(id);
        if self.current_id == id {
            self.temporary = None;
            self.playback = Playback::Running;
            self.advance_to_next();
        }
        Ok(())
    }

    pub fn timer_action_at(&mut self, id: &str, action: &str, wall: u64) -> Result<(), String> {
        let w = self
            .settings
            .widgets
            .iter_mut()
            .find(|w| w.id == id && w.kind == "pomodoro")
            .ok_or("Pomodoro widget not found")?;
        let c = &mut w.config;
        match action {
            "start" => {
                if !c.timer_running {
                    c.timer_started_at = wall;
                    c.timer_running = true;
                }
            }
            "pause" => {
                c.timer_elapsed = crate::local_widgets::elapsed(c, wall);
                c.timer_running = false;
                c.timer_started_at = 0;
            }
            "reset" => {
                c.timer_running = false;
                c.timer_started_at = 0;
                c.timer_elapsed = 0;
            }
            "skip" => {
                c.timer_elapsed = crate::local_widgets::elapsed(c, wall)
                    .saturating_add(crate::local_widgets::timer(c, wall).remaining);
                c.timer_started_at = if c.timer_running { wall } else { 0 };
            }
            _ => return Err("Choose Start, Pause, Reset or Skip phase".into()),
        }
        self.data
            .insert(id.into(), crate::local_widgets::dynamic_card(w, wall));
        Ok(())
    }

    fn next_eligible(&self) -> Option<&Widget> {
        if self.settings.widgets.is_empty() {
            return None;
        }
        let start = self
            .settings
            .widgets
            .iter()
            .position(|w| w.id == self.current_id)
            .unwrap_or(0);
        (1..=self.settings.widgets.len())
            .map(|step| &self.settings.widgets[(start + step) % self.settings.widgets.len()])
            .find(|w| self.eligible(w))
    }

    fn advance_to_next(&mut self) {
        if let Some((id, duration)) = self.next_eligible().map(|w| (w.id.clone(), w.duration_sec)) {
            self.current_id = id;
            self.remaining_ms = duration as u64 * 1000;
        } else {
            self.current_id = "face".into();
            self.remaining_ms = 12_000;
        }
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        use chrono::Timelike;
        let local = chrono::Local::now();
        self.tick_at(now, unix_now(), (local.hour() * 60 + local.minute()) as u16)
    }
    pub fn tick_at(&mut self, now: Instant, wall: u64, minute: u16) -> bool {
        let mut local_changed = false;
        if self.local_second != wall {
            self.local_second = wall;
            for w in &self.settings.widgets {
                if crate::local_widgets::dynamic(&w.kind) {
                    self.data
                        .insert(w.id.clone(), crate::local_widgets::dynamic_card(w, wall));
                    local_changed = true;
                }
            }
        }
        let was_quiet = self.quiet_active;
        self.quiet_active = self.settings.preferences.quiet_hours.contains(minute)
            && wall >= self.quiet_override_until;
        let policy_changed = was_quiet != self.quiet_active;
        if was_quiet && !self.quiet_active {
            self.remaining_ms = self
                .current_widget()
                .map(|w| w.duration_sec as u64 * 1000)
                .unwrap_or(12000);
        }
        let prior_animation = self.current_animation_state();
        let elapsed = now.saturating_duration_since(self.last_tick).as_millis() as u64;
        self.last_tick = now;
        if elapsed > 60000 {
            self.phase_ms = 0;
        } else if !self.quiet_active && self.display_on && self.playback != Playback::Paused {
            self.phase_ms = self.phase_ms.wrapping_add(elapsed);
        }
        let animation_changed = prior_animation != self.current_animation_state();
        if self.quiet_active {
            return policy_changed || local_changed;
        }
        if self.current_id != "face" && !self.enabled(&self.current_id) {
            self.advance_to_next();
        }
        if self.playback != Playback::Running || elapsed == 0 {
            return policy_changed || animation_changed || local_changed;
        }
        // A suspended computer should resume at a full dwell, not replay hours of transitions.
        if elapsed > 60_000 {
            if let Some(saved) = self.temporary.take() {
                self.current_id = saved.saved_id;
                self.playback = saved.saved_playback;
            }
            if !self.enabled(&self.current_id) {
                self.advance_to_next();
            }
            self.remaining_ms = self
                .current_widget()
                .map(|w| w.duration_sec as u64 * 1000)
                .unwrap_or(12_000);
            return true;
        }
        let old_remaining = self.remaining_ms;
        let mut left = elapsed;
        while left >= self.remaining_ms && self.remaining_ms > 0 {
            left -= self.remaining_ms;
            if let Some(saved) = self.temporary.take() {
                self.current_id = saved.saved_id;
                self.remaining_ms = saved.saved_remaining_ms;
                self.playback = saved.saved_playback;
                if !self.enabled(&self.current_id) {
                    self.advance_to_next();
                }
                if self.playback != Playback::Running {
                    return true;
                }
            } else {
                self.advance_to_next();
            }
        }
        self.remaining_ms = self.remaining_ms.saturating_sub(left);
        policy_changed
            || local_changed
            || animation_changed
            || old_remaining.div_ceil(1000) != self.remaining_ms.div_ceil(1000)
            || elapsed >= old_remaining
    }

    pub fn snapshot(&self) -> Snapshot {
        let current = self.current_widget();
        Snapshot {
            settings: self.settings.clone(),
            current_id: self.current_id.clone(),
            current_label: current
                .map(|w| w.label.clone())
                .unwrap_or_else(|| "Happy face".into()),
            playback: self.playback,
            remaining_sec: self.remaining_ms.div_ceil(1000) as u16,
            total_sec: self
                .settings
                .widgets
                .iter()
                .filter(|w| self.eligible(w))
                .map(|w| w.duration_sec as u32)
                .sum(),
            display_on: self.display_on && !self.quiet_active,
            display_reason: if !self.display_on {
                "Manually off"
            } else if self.quiet_active {
                "Quiet hours"
            } else if self.quiet_override_until > unix_now() {
                "Quiet hours overridden for 15 minutes"
            } else {
                "On"
            }
            .into(),
            quiet_active: self.quiet_active,
            quiet_override_until: self.quiet_override_until,
            temporary: self.temporary.is_some(),
            frame: self.render_current(&self.current_id, self.display_on && !self.quiet_active),
            animation_phase: self.phase_ms,
            data: self
                .data
                .iter()
                .map(|(id, d)| (id.clone(), d.summary(unix_now())))
                .collect(),
        }
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        let widget = self
            .settings
            .widgets
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or("Widget not found")?;
        widget.enabled = enabled;
        if !self.enabled(&self.current_id) {
            self.temporary = None;
            if self.playback == Playback::Pinned {
                self.playback = Playback::Running;
            }
            self.advance_to_next();
        }
        Ok(())
    }

    pub fn reorder_widget(&mut self, id: &str, before: &str) -> Result<(), String> {
        if id == before {
            return Ok(());
        }
        let from = self
            .settings
            .widgets
            .iter()
            .position(|w| w.id == id)
            .ok_or("Widget not found")?;
        if !self.settings.widgets.iter().any(|w| w.id == before) {
            return Err("Drop target not found".into());
        }
        let widget = self.settings.widgets.remove(from);
        let to = self
            .settings
            .widgets
            .iter()
            .position(|w| w.id == before)
            .unwrap();
        self.settings.widgets.insert(to, widget);
        Ok(())
    }

    pub fn move_widget(&mut self, id: &str, direction: i8) -> Result<(), String> {
        let from = self
            .settings
            .widgets
            .iter()
            .position(|w| w.id == id)
            .ok_or("Widget not found")?;
        let to = from as isize + direction as isize;
        if ![-1, 1].contains(&direction) || to < 0 || to >= self.settings.widgets.len() as isize {
            return Err("Cannot move widget there".into());
        }
        self.settings.widgets.swap(from, to as usize);
        Ok(())
    }

    pub fn set_duration(&mut self, id: &str, duration_sec: u16) -> Result<(), String> {
        if !(3..=120).contains(&duration_sec) {
            return Err("Duration must be 3 to 120 seconds".into());
        }
        let widget = self
            .settings
            .widgets
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or("Widget not found")?;
        let old = widget.duration_sec as u64 * 1000;
        widget.duration_sec = duration_sec;
        if self.current_id == id {
            let new = duration_sec as u64 * 1000;
            if self.playback == Playback::Pinned {
                self.remaining_ms = new;
            } else {
                let elapsed = old.saturating_sub(self.remaining_ms);
                if elapsed >= new {
                    self.advance_to_next();
                } else {
                    self.remaining_ms = new - elapsed;
                }
            }
        }
        Ok(())
    }

    pub fn configure(
        &mut self,
        id: &str,
        label: String,
        headline: String,
        footer: String,
    ) -> Result<(), String> {
        if label.trim().is_empty()
            || label.chars().count() > 24
            || headline.chars().count() > 24
            || footer.chars().count() > 24
        {
            return Err("Use a label and keep each line under 25 characters".into());
        }
        let widget = self
            .settings
            .widgets
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or("Widget not found")?;
        widget.label = label.trim().into();
        widget.headline = headline.trim().into();
        widget.footer = footer.trim().into();
        Ok(())
    }

    pub fn next(&mut self) {
        self.temporary = None;
        self.advance_to_next();
    }

    pub fn toggle_pause(&mut self) {
        self.temporary = None;
        self.playback = match self.playback {
            Playback::Running => Playback::Paused,
            _ => Playback::Running,
        };
        self.last_tick = Instant::now();
    }

    pub fn pin(&mut self, id: &str) -> Result<(), String> {
        if !self.enabled(id) {
            return Err("Enable the widget before pinning it".into());
        }
        self.temporary = None;
        self.current_id = id.into();
        self.remaining_ms = self.current_widget().unwrap().duration_sec as u64 * 1000;
        self.playback = Playback::Pinned;
        Ok(())
    }

    pub fn show_now(&mut self, id: &str) -> Result<(), String> {
        if !self.enabled(id) {
            return Err("Enable the widget before showing it".into());
        }
        if self.temporary.is_none() {
            self.temporary = Some(Temporary {
                saved_id: self.current_id.clone(),
                saved_remaining_ms: self.remaining_ms,
                saved_playback: self.playback,
            });
        }
        self.current_id = id.into();
        self.remaining_ms = self.current_widget().unwrap().duration_sec as u64 * 1000;
        self.playback = Playback::Running;
        self.last_tick = Instant::now();
        Ok(())
    }

    pub fn show_face(&mut self) {
        self.temporary = None;
        self.current_id = "face".into();
        self.remaining_ms = self
            .current_widget()
            .map(|w| w.duration_sec as u64 * 1000)
            .unwrap_or(12_000);
        self.playback = Playback::Pinned;
    }

    pub fn set_brightness(&mut self, brightness: u8) -> Result<(), String> {
        if brightness > 100 {
            return Err("Brightness must be 0 to 100".into());
        }
        self.settings.brightness = brightness;
        Ok(())
    }

    pub fn set_preferences(&mut self, preferences: Preferences) -> Result<(), String> {
        preferences.validate()?;
        if !preferences.quiet_hours.enabled {
            self.quiet_override_until = 0;
        }
        self.settings.preferences = preferences;
        self.tick(Instant::now());
        Ok(())
    }
    pub fn override_quiet(&mut self) {
        self.quiet_override_until = unix_now() + 900;
        self.tick(Instant::now());
    }
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }
    pub fn restore_data(&mut self, cards: HashMap<String, DataCard>) {
        for (id, card) in cards {
            if card.usable(unix_now()) {
                self.data.insert(id, card);
            }
        }
    }
    pub fn set_display_on(&mut self, on: bool) {
        self.display_on = on;
        self.settings.manual_display_off = !on;
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct CompanionPose {
    closed: bool,
    glance: i8,
    mouth: u8,
    bounce: bool,
}
impl CompanionPose {
    fn at(ms: u64, speed: &str, glances: bool, mouths: bool) -> Self {
        let blink_period = match speed {
            "slow" => 6000,
            "quick" => 2400,
            _ => 3600,
        };
        let cycle = match speed {
            "slow" => 12000,
            "quick" => 6400,
            _ => 8000,
        };
        let phase = (ms % cycle) * 6400 / cycle;
        Self {
            closed: ms % blink_period >= blink_period - 200,
            glance: if !glances {
                0
            } else {
                match phase {
                    600..=799 | 1600..=1799 => -1,
                    800..=1599 => -2,
                    _ => 0,
                }
            },
            mouth: if !mouths {
                0
            } else {
                match phase {
                    2200..=2599 => 1,
                    2600..=2999 => 2,
                    _ => 0,
                }
            },
            bounce: (4200..4400).contains(&phase),
        }
    }
}

struct Frame {
    bytes: Vec<u8>,
}
impl Frame {
    fn new() -> Self {
        Self {
            bytes: vec![0; FRAME_BYTES],
        }
    }
    fn pixel(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            return;
        }
        let index = (y as usize / 8) * WIDTH + x as usize;
        self.bytes[index] |= 1 << (y as usize % 8);
    }
    fn text(&mut self, s: &str, x: i32, y: i32, scale: i32, max_chars: usize) {
        let mut text: Vec<char> = s
            .chars()
            .map(|c| match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => {
                    'A'
                }
                'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'E',
                'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => {
                    'O'
                }
                'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
                'ñ' | 'Ñ' => 'N',
                'ç' | 'Ç' => 'C',
                'ł' | 'Ł' => 'L',
                '−' => '-',
                _ => c.to_ascii_uppercase(),
            })
            .collect();
        if text.len() > max_chars {
            text.truncate(max_chars);
            for c in text.iter_mut().rev().take(3) {
                *c = '.';
            }
        }
        for (i, ch) in text.into_iter().enumerate() {
            let degree = [0x06, 0x09, 0x09, 0x06, 0, 0, 0, 0];
            if let Some(glyph) = if ch == '°' {
                Some(degree)
            } else {
                BASIC_FONTS.get(ch).or_else(|| BASIC_FONTS.get('?'))
            } {
                for (row, bits) in glyph.iter().enumerate() {
                    for col in 0..8 {
                        if (bits >> col) & 1 == 1 {
                            for dy in 0..scale {
                                for dx in 0..scale {
                                    self.pixel(
                                        x + (i as i32 * 8 + col) * scale + dx,
                                        y + row as i32 * scale + dy,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fn expression(&mut self, expression: &str, closed: bool) {
        if expression == "happy" && !closed {
            self.face();
            return;
        }
        for cx in [38, 90] {
            if closed || expression == "sleepy" {
                for x in cx - 9..=cx + 9 {
                    self.pixel(x, 25);
                    self.pixel(x, 26);
                }
            } else {
                for y in 12..37 {
                    for x in cx - 8..=cx + 8 {
                        let distance =
                            ((x - cx) as f32 / 8.0).powi(2) + ((y - 24) as f32 / 12.0).powi(2);
                        if (0.57..=1.0).contains(&distance) {
                            self.pixel(x, y);
                        }
                    }
                }
            }
        }
        match expression {
            "surprised" => {
                for y in 42..54 {
                    for x in 58..70 {
                        let d = (x - 64) * (x - 64) + (y - 48) * (y - 48);
                        if (20..=36).contains(&d) {
                            self.pixel(x, y);
                        }
                    }
                }
            }
            "neutral" | "sleepy" => {
                for x in 56..73 {
                    self.pixel(x, 47);
                    self.pixel(x, 48);
                }
            }
            _ => {
                for x in 56..72 {
                    let dx = (x - 64) as f32 / 8.0;
                    let y = 43 + (5.0 * (1.0 - dx * dx)) as i32;
                    self.pixel(x, y);
                    self.pixel(x, y + 1);
                }
            }
        }
    }
    fn face(&mut self) {
        for cx in [38, 90] {
            for y in 12..37 {
                for x in (cx - 8)..=(cx + 8) {
                    let d = ((x - cx) as f32 / 8.0).powi(2) + ((y - 24) as f32 / 12.0).powi(2);
                    if (0.57..=1.0).contains(&d) {
                        self.pixel(x, y);
                    }
                }
            }
        }
        for x in 56..72 {
            let dx = (x - 64) as f32 / 8.0;
            let y = 43 + (5.0 * (1.0 - dx * dx)) as i32;
            self.pixel(x, y);
            self.pixel(x, y + 1);
        }
    }
    fn bar(&mut self, x: i32, y: i32, len: i32) {
        for xx in x..x + len.clamp(0, 116) {
            for yy in y..y + 3 {
                self.pixel(xx, yy);
            }
        }
    }
    // Reuse the original happy-face bitmap at half size, including its blink.
    // Crop its blank margins into a dedicated 40 × 26 corner above the data.
    fn animated_face(&mut self, expression: &str, pose: CompanionPose) {
        self.expression(expression, pose.closed);
        if expression != "happy" {
            return;
        }
        if !pose.closed && pose.glance != 0 {
            // The familiar hollow-eye outlines stay intact. Tiny pupils look
            // down and left toward the information, then disappear on return.
            for cx in [38, 90] {
                for y in 25..29 {
                    for x in cx + pose.glance as i32 * 2 - 1..=cx + pose.glance as i32 * 2 + 1 {
                        self.pixel(x, y);
                    }
                }
            }
        }
        if pose.mouth != 0 {
            for y in 40..56 {
                for x in 52..77 {
                    self.bytes[(y / 8) * WIDTH + x] &= !(1 << (y % 8));
                }
            }
            let (rx, ry) = if pose.mouth == 1 {
                (7.0, 5.0)
            } else {
                (4.0, 6.0)
            };
            for y in 41..55 {
                for x in 56..73 {
                    let d = ((x as f32 - 64.0) / rx).powi(2) + ((y as f32 - 48.0) / ry).powi(2);
                    if (0.48..=1.0).contains(&d) {
                        self.pixel(x, y);
                    }
                }
            }
        }
    }
    fn companion(&mut self, pose: CompanionPose) {
        let mut original = Frame::new();
        original.animated_face("happy", pose);
        for y in 0..25 {
            for x in 0..37 {
                let mut lit = false;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let sx = 28 + x * 2 + dx;
                        let sy = 8 + y * 2 + dy;
                        lit |= original.bytes[(sy / 8) * WIDTH + sx] & (1 << (sy % 8)) != 0;
                    }
                }
                if lit {
                    self.pixel(88 + x as i32, y as i32 + i32::from(pose.bounce));
                }
            }
        }
    }
}

pub fn render(widget: Option<&Widget>, display_on: bool) -> Vec<u8> {
    render_card(widget, display_on, Some(CompanionPose::default()))
}
fn render_card(
    widget: Option<&Widget>,
    display_on: bool,
    companion: Option<CompanionPose>,
) -> Vec<u8> {
    let mut frame = Frame::new();
    if !display_on {
        return frame.bytes;
    }
    let kind = widget.map(|w| w.kind.as_str()).unwrap_or("face");
    if kind == "face" {
        frame.expression(
            widget
                .map(|w| w.config.face_expression.as_str())
                .unwrap_or("happy"),
            false,
        );
        return frame.bytes;
    }
    let Some(widget) = widget else {
        frame.face();
        return frame.bytes;
    };
    let title = match kind {
        "weather" => &widget.label,
        "stock" => &widget.label,
        "codex" => "CODEX",
        "claude" => "CLAUDE",
        _ => "DROID FUN",
    };
    frame.text(title, 4, 2, 1, if companion.is_some() { 10 } else { 15 });
    if let Some(pose) = companion {
        frame.companion(pose);
    }
    let (scale, capacity) = if widget.headline.chars().count() <= 7 {
        (2, 7)
    } else {
        (1, 15)
    };
    frame.text(
        &widget.headline,
        4,
        if companion.is_some() { 28 } else { 19 },
        scale,
        capacity,
    );
    if kind == "codex" || kind == "claude" {
        if let Ok(percent) = widget.headline.trim_end_matches('%').parse::<i32>() {
            frame.bar(4, 47, percent.clamp(0, 100) * 116 / 100);
        }
    }
    frame.text(&widget.footer, 4, 55, 1, 15);
    frame.bytes
}

fn render_local(
    widget: &Widget,
    on: bool,
    companion: Option<CompanionPose>,
    values: &serde_json::Value,
) -> Vec<u8> {
    let mut f = Frame::new();
    if !on {
        return f.bytes;
    }
    f.text(
        match widget.kind.as_str() {
            "clock" => "CLOCK",
            "pomodoro" => "POMODORO",
            "meeting" => "MEETING",
            "cpu" => "CPU",
            _ => "MEMORY",
        },
        4,
        2,
        1,
        if companion.is_some() { 10 } else { 15 },
    );
    f.text(
        values["context"].as_str().unwrap_or("LOADING"),
        4,
        14,
        1,
        if companion.is_some() { 10 } else { 15 },
    );
    if let Some(pose) = companion {
        f.companion(pose);
    }
    let scale = if widget.headline.chars().count() <= 7 {
        2
    } else {
        1
    };
    f.text(
        &widget.headline,
        4,
        28,
        scale,
        if scale == 2 { 7 } else { 15 },
    );
    let history = values["history"].as_array().filter(|h| h.len() > 1);
    if let Some(series) = history {
        let mut previous: Option<(i32, i32)> = None;
        for (i, value) in series.iter().enumerate() {
            let height =
                (value.as_f64().unwrap_or(0.0).clamp(0.0, 100.0) * 7.0 / 100.0).round() as i32;
            let x = 4 + i as i32 * 119 / (series.len() as i32 - 1);
            let y = 52 - height;
            if let Some((mut xx, mut yy)) = previous {
                let dx = (x - xx).abs();
                let dy = -(y - yy).abs();
                let sx = if xx < x { 1 } else { -1 };
                let sy = if yy < y { 1 } else { -1 };
                let mut error = dx + dy;
                loop {
                    f.pixel(xx, yy);
                    if xx == x && yy == y {
                        break;
                    }
                    let e2 = 2 * error;
                    if e2 >= dy {
                        error += dy;
                        xx += sx;
                    }
                    if e2 <= dx {
                        error += dx;
                        yy += sy;
                    }
                }
            } else {
                f.pixel(x, y);
            }
            previous = Some((x, y));
        }
    } else if let Some(percent) = values["progress"].as_f64() {
        for x in 4..124 {
            f.pixel(x, 49);
            f.pixel(x, 52);
        }
        for y in 49..53 {
            f.pixel(4, y);
            f.pixel(123, y);
        }
        for x in 6..6 + (percent.clamp(0.0, 100.0) * 116.0 / 100.0).round() as i32 {
            f.pixel(x, 50);
            f.pixel(x, 51);
        }
    } else {
        f.text(values["status"].as_str().unwrap_or(""), 4, 45, 1, 15);
    }
    f.text(&widget.footer, 4, 55, 1, 15);
    f.bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn companion_gestures_preserve_all_local_widget_information_and_original_identity() {
        assert_eq!(crate::protocol::crc32(&render(None, true)), 2039582962);
        for kind in ["clock", "pomodoro", "meeting", "cpu", "memory"] {
            let w = Widget {
                id: kind.into(),
                kind: kind.into(),
                label: "Test".into(),
                enabled: true,
                duration_sec: 3,
                headline: "12:34".into(),
                footer: "MON OCT 12".into(),
                config: SourceConfig {
                    mode: "live".into(),
                    ..SourceConfig::default()
                },
            };
            let values = serde_json::json!({"context":"LOCAL","progress":50});
            let base = render_local(&w, true, Some(CompanionPose::default()), &values);
            for ms in [600, 800, 1600, 2200, 2600, 4200, 4600] {
                let pose = CompanionPose::at(ms, "quick", true, true);
                let animated = render_local(&w, true, Some(pose), &values);
                assert_ne!(base, animated, "{kind} at {ms}");
                for y in 0..64 {
                    for x in 0..128 {
                        if x < 88 || y >= 26 {
                            let bit = 1 << (y % 8);
                            assert_eq!(
                                base[y / 8 * 128 + x] & bit,
                                animated[y / 8 * 128 + x] & bit
                            );
                        }
                    }
                }
            }
        }
        assert!(!CompanionPose::at(2199, "quick", true, true).closed);
        assert!(CompanionPose::at(2200, "quick", true, true).closed);
        assert!(!CompanionPose::at(2400, "quick", true, true).closed);
    }

    #[test]
    fn pomodoro_controls_persist_independently_of_playlist_pause_and_configuration_edits() {
        let mut e = Engine::new(Settings::default(), Instant::now());
        e.add_widget("pomodoro").unwrap();
        let id = e.settings.widgets.last().unwrap().id.clone();
        e.timer_action_at(&id, "start", 1000).unwrap();
        e.toggle_pause();
        let c = &e.settings.widgets.last().unwrap().config;
        assert_eq!(crate::local_widgets::timer(c, 1030).remaining, 1470);
        e.timer_action_at(&id, "pause", 1030).unwrap();
        assert_eq!(
            crate::local_widgets::timer(&e.settings.widgets.last().unwrap().config, 2000).remaining,
            1470
        );
        e.timer_action_at(&id, "skip", 2000).unwrap();
        assert_eq!(
            crate::local_widgets::timer(&e.settings.widgets.last().unwrap().config, 2000).phase,
            "BREAK"
        );
        e.timer_action_at(&id, "start", 2000).unwrap();
        let mut edited = e.settings.widgets.last().unwrap().config.clone();
        edited.timer_running = false;
        e.configure_source(&id, "Work".into(), edited.clone())
            .unwrap();
        assert!(e.settings.widgets.last().unwrap().config.timer_running);
        edited.focus_minutes = 30;
        e.configure_source(&id, "Work".into(), edited).unwrap();
        let c = &e.settings.widgets.last().unwrap().config;
        assert!(!c.timer_running);
        assert_eq!(crate::local_widgets::timer(c, 3000).remaining, 1800);
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&e.settings).unwrap()).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.widgets.last().unwrap().config.focus_minutes, 30);
    }

    #[test]
    fn companion_animation_preserves_every_data_pixel_and_can_be_disabled() {
        let now = Instant::now();
        let mut engine = Engine::new(Settings::default(), now);
        for kind in ["weather", "stock", "codex", "claude"] {
            let w = engine
                .settings
                .widgets
                .iter_mut()
                .find(|w| w.kind == kind)
                .unwrap();
            w.enabled = true;
            w.label = "An unusually long accented location".into();
            let id = w.id.clone();
            engine.current_id = id.clone();
            engine.playback = Playback::Pinned;
            engine.phase_ms = 0;
            engine.last_tick = now;
            let open = engine.render_widget(&id);
            assert!(engine.tick_at(now + Duration::from_millis(4600), 10000, 720));
            let closed = engine.render_widget(&id);
            assert_ne!(open, closed);
            let pixel =
                |frame: &[u8], x: usize, y: usize| frame[(y / 8) * WIDTH + x] & (1 << (y % 8)) != 0;
            assert!(pixel(&open, 93, 2));
            assert!(!pixel(&closed, 93, 2));
            assert!(!pixel(&open, 93, 8)); // The original rounded eyes have hollow centers.
            assert!(pixel(&closed, 93, 8));
            assert!(pixel(&open, 106, 20)); // The original smile remains while blinking.
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    if x < 88 || y >= 26 {
                        assert_eq!(pixel(&open, x, y), pixel(&closed, x, y));
                    }
                }
            }
            engine.phase_ms = 4200;
            let beaming = engine.render_widget(&id);
            assert_ne!(open, beaming);
            for i in WIDTH * 4..FRAME_BYTES {
                assert_eq!(open[i], beaming[i]);
            }
            engine.set_reduced_motion(true);
            assert_eq!(open, engine.render_widget(&id));
            engine.set_reduced_motion(false);
            engine.settings.preferences.companion_animation = false;
            assert_eq!(open, engine.render_widget(&id));
            engine.settings.preferences.companion_animation = true;
            engine.settings.preferences.companion_face = false;
            let without = engine.render_widget(&id);
            assert!(!pixel(&without, 106, 20));
            engine.settings.preferences.companion_face = true;
            engine.set_display_on(false);
            assert!(engine.snapshot().frame.iter().all(|b| *b == 0));
            engine.set_display_on(true);
        }
    }

    #[test]
    fn quiet_hours_overnight_override_and_manual_off_precedence() {
        let now = Instant::now();
        let mut settings = Settings::default();
        settings.preferences.quiet_hours = QuietHours {
            enabled: true,
            start_minute: 1320,
            end_minute: 420,
        };
        let mut e = Engine::new(settings, now);
        e.tick_at(now + Duration::from_secs(3), 10000, 1319);
        assert_eq!(e.snapshot().remaining_sec, 9);
        e.tick_at(now + Duration::from_secs(4), 10001, 1320);
        assert!(!e.snapshot().display_on);
        assert_eq!(e.snapshot().display_reason, "Quiet hours");
        e.tick_at(now + Duration::from_secs(8), 10005, 1);
        assert_eq!(e.snapshot().remaining_sec, 9);
        e.quiet_override_until = 10905;
        e.tick_at(now + Duration::from_secs(9), 10006, 1);
        assert!(e.snapshot().display_on);
        e.set_display_on(false);
        assert!(!e.snapshot().display_on);
        assert_eq!(e.snapshot().display_reason, "Manually off");
        e.set_display_on(true);
        e.tick_at(now + Duration::from_secs(10), 10905, 2);
        assert!(!e.snapshot().display_on);
        e.tick_at(now + Duration::from_secs(11), 10906, 420);
        assert!(e.snapshot().display_on);
        assert_eq!(e.snapshot().remaining_sec, 11);
        // Spring-forward and fall-back local clock changes are evaluated directly, without UTC arithmetic.
        assert!(e.settings.preferences.quiet_hours.contains(180));
        assert!(!e.settings.preferences.quiet_hours.contains(720));
    }
    #[test]
    fn expressions_blink_and_reduced_motion_have_distinct_frames() {
        let now = Instant::now();
        let mut e = Engine::new(Settings::default(), now);
        let happy = e.render_widget("face");
        e.tick_at(now + Duration::from_millis(3500), 10000, 720);
        let blink = e.render_widget("face");
        assert_ne!(happy, blink);
        e.set_reduced_motion(true);
        assert_eq!(happy, e.render_widget("face"));
        for expression in ["neutral", "sleepy", "surprised"] {
            e.settings.widgets[0].config.face_expression = expression.into();
            assert_ne!(happy, e.render_widget("face"));
        }
    }
    #[test]
    fn drag_reorder_preserves_the_current_dwell_and_stable_ids() {
        let now = Instant::now();
        let mut e = Engine::new(Settings::default(), now);
        e.set_enabled("weather", true).unwrap();
        e.set_enabled("stock", true).unwrap();
        e.tick_at(now + Duration::from_secs(4), 10000, 720);
        e.reorder_widget("stock", "weather").unwrap();
        assert_eq!(e.snapshot().remaining_sec, 8);
        assert_eq!(e.snapshot().current_id, "face");
        e.tick_at(now + Duration::from_secs(12), 10008, 720);
        assert_eq!(e.snapshot().current_id, "stock");
    }

    #[test]
    fn unavailable_live_widget_is_skipped_and_expiry_removes_it() {
        let mut settings = Settings::default();
        settings.widgets[1].enabled = true;
        settings.widgets[1].config.mode = "live".into();
        settings.widgets[1].config.location = Some(Location {
            name: "Test".into(),
            latitude: 0.0,
            longitude: 0.0,
            country: String::new(),
            region: String::new(),
        });
        let mut e = Engine::new(settings, Instant::now());
        e.next();
        assert_eq!(e.snapshot().current_id, "face");
        let now = unix_now();
        let card = DataCard {
            source_id: "fixture".into(),
            scope_id: "weather".into(),
            observed_at: now,
            fetched_at: now,
            stale_at: now + 10,
            expires_at: now + 20,
            state: "fresh".into(),
            headline: "12 C".into(),
            footer: "RAIN".into(),
            detail: "fixture".into(),
            values: serde_json::json!({"temperature":12}),
        };
        e.set_data("weather", card.clone());
        e.next();
        assert_eq!(e.snapshot().current_id, "weather");
        e.set_data(
            "weather",
            DataCard {
                expires_at: now,
                ..card
            },
        );
        assert_eq!(e.snapshot().current_id, "face");
        assert_eq!(e.snapshot().data["weather"].headline, "WAITING");
    }

    #[test]
    fn multiple_instances_have_independent_source_settings() {
        let mut e = Engine::new(Settings::default(), Instant::now());
        e.add_widget("weather").unwrap();
        let id = e.settings.widgets.last().unwrap().id.clone();
        e.configure_source(
            &id,
            "Oslo".into(),
            SourceConfig {
                mode: "live".into(),
                location: Some(Location {
                    name: "Oslo".into(),
                    latitude: 59.9,
                    longitude: 10.7,
                    country: "Norway".into(),
                    region: String::new(),
                }),
                unit: "celsius".into(),
                ..SourceConfig::default()
            },
        )
        .unwrap();
        assert_eq!(e.settings.widgets[1].config.mode, "demo");
        assert_eq!(e.settings.widgets.last().unwrap().config.unit, "celsius");
        e.remove_widget(&id).unwrap();
        assert_eq!(e.settings.widgets.len(), 5);
        assert!(e.remove_widget("face").is_err());
    }

    #[test]
    fn cycle_keeps_time_when_reordered_and_skips_disabled_widget() {
        let now = Instant::now();
        let mut e = Engine::new(Settings::default(), now);
        e.set_enabled("weather", true).unwrap();
        e.set_enabled("stock", true).unwrap();
        e.tick(now + Duration::from_secs(4));
        assert_eq!(e.snapshot().remaining_sec, 8);
        e.move_widget("stock", -1).unwrap();
        assert_eq!(e.snapshot().remaining_sec, 8);
        e.set_enabled("stock", false).unwrap();
        e.tick(now + Duration::from_secs(12));
        assert_eq!(e.snapshot().current_id, "weather");
    }

    #[test]
    fn temporary_show_restores_paused_position() {
        let now = Instant::now();
        let mut e = Engine::new(Settings::default(), now);
        e.set_enabled("weather", true).unwrap();
        e.tick(now + Duration::from_secs(3));
        e.toggle_pause();
        let remaining = e.snapshot().remaining_sec;
        e.show_now("weather").unwrap();
        e.tick(Instant::now() + Duration::from_secs(10));
        assert_eq!(e.snapshot().current_id, "face");
        assert_eq!(e.snapshot().playback, Playback::Paused);
        assert_eq!(e.snapshot().remaining_sec, remaining);
    }

    #[test]
    fn face_frame_uses_page_packed_bits() {
        let bytes = render(None, true);
        assert_eq!(bytes.len(), 1024);
        assert!(bytes.iter().any(|byte| *byte != 0));
        assert_eq!(render(None, false), vec![0; 1024]);
    }

    #[test]
    fn shortening_elapsed_duration_advances_and_disabled_pin_resumes_cycle() {
        let now = Instant::now();
        let mut e = Engine::new(Settings::default(), now);
        e.set_enabled("weather", true).unwrap();
        e.tick(now + Duration::from_secs(6));
        e.set_duration("face", 5).unwrap();
        assert_eq!(e.snapshot().current_id, "weather");
        e.pin("weather").unwrap();
        e.set_enabled("weather", false).unwrap();
        assert_eq!(e.snapshot().current_id, "face");
        assert_eq!(e.snapshot().playback, Playback::Running);
    }

    #[test]
    fn waking_after_long_sleep_restarts_current_dwell() {
        let now = Instant::now();
        let mut e = Engine::new(Settings::default(), now);
        e.set_enabled("weather", true).unwrap();
        e.tick(now + Duration::from_secs(4));
        e.tick(now + Duration::from_secs(3604));
        assert_eq!(e.snapshot().current_id, "face");
        assert_eq!(e.snapshot().remaining_sec, 12);
    }
}
