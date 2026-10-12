//! Local clocks/timers and narrowly sampled host CPU/RAM. No network or processes.
use crate::core::{DataCard, SourceConfig, Widget};
use chrono::{DateTime, Local, Utc};
use serde_json::{json, Value};

pub fn dynamic(kind: &str) -> bool {
    matches!(kind, "clock" | "pomodoro" | "meeting")
}
pub fn local(kind: &str) -> bool {
    dynamic(kind) || matches!(kind, "cpu" | "memory")
}

pub fn validate(c: &SourceConfig) -> Result<(), String> {
    if (c.clock_zone != "local" && c.clock_zone.parse::<chrono_tz::Tz>().is_err())
        || !["12", "24"].contains(&c.clock_format.as_str())
        || !["weekday", "date", "none"].contains(&c.clock_date.as_str())
        || !(1..=120).contains(&c.focus_minutes)
        || !(1..=60).contains(&c.break_minutes)
        || !(1..=120).contains(&c.long_break_minutes)
        || !(2..=12).contains(&c.focus_sessions)
        || c.timer_elapsed > 315_360_000
        || c.timer_started_at > 4_102_444_800
        || c.meeting_starts_at > 4_102_444_800
        || !(1..=480).contains(&c.meeting_minutes)
        || !(1..=30).contains(&c.monitor_interval)
        || !["used", "available"].contains(&c.memory_metric.as_str())
        || !["gib", "gb"].contains(&c.memory_unit.as_str())
    {
        return Err(
            "Choose a valid time zone, timer length, meeting time or monitor setting".into(),
        );
    }
    Ok(())
}

pub struct TimerView {
    pub phase: &'static str,
    pub session: u64,
    pub remaining: u64,
    pub duration: u64,
}
pub fn elapsed(c: &SourceConfig, wall: u64) -> u64 {
    c.timer_elapsed.saturating_add(if c.timer_running {
        wall.saturating_sub(c.timer_started_at)
    } else {
        0
    })
}
pub fn timer(c: &SourceConfig, wall: u64) -> TimerView {
    let focus = c.focus_minutes as u64 * 60;
    let rest = c.break_minutes as u64 * 60;
    let long = c.long_break_minutes as u64 * 60;
    let n = c.focus_sessions as u64;
    let cycle = (focus + rest) * (n - 1) + focus + long;
    let mut position = elapsed(c, wall) % cycle;
    for session in 1..=n {
        if position < focus {
            return TimerView {
                phase: "FOCUS",
                session,
                remaining: focus - position,
                duration: focus,
            };
        }
        position -= focus;
        let duration = if session == n { long } else { rest };
        if position < duration {
            return TimerView {
                phase: if session == n { "LONG BREAK" } else { "BREAK" },
                session,
                remaining: duration - position,
                duration,
            };
        }
        position -= duration;
    }
    unreachable!("Timer position is bounded by its validated cycle")
}

fn card(
    source: &str,
    wall: u64,
    value: String,
    footer: String,
    detail: String,
    values: Value,
) -> DataCard {
    DataCard {
        source_id: source.into(),
        scope_id: String::new(),
        observed_at: wall,
        fetched_at: wall,
        stale_at: wall + 60,
        expires_at: wall + 120,
        state: "fresh".into(),
        headline: value,
        footer,
        detail,
        values,
    }
}

pub fn dynamic_card(w: &Widget, wall: u64) -> DataCard {
    let c = &w.config;
    let utc = DateTime::<Utc>::from_timestamp(wall as i64, 0).unwrap_or_default();
    match w.kind.as_str() {
        "clock" => {
            let pattern = if c.clock_format == "12" {
                "%-I:%M"
            } else {
                "%H:%M"
            };
            let footer_pattern = match c.clock_date.as_str() {
                "none" => "",
                "date" => "%Y-%m-%d",
                _ => "%a %b %d",
            };
            let (value, zone, suffix, footer, second) = if c.clock_zone == "local" {
                let d = utc.with_timezone(&Local);
                (
                    d.format(pattern).to_string(),
                    d.format("%Z").to_string(),
                    d.format("%p").to_string(),
                    d.format(footer_pattern).to_string(),
                    d.format("%S").to_string(),
                )
            } else {
                let tz: chrono_tz::Tz = c.clock_zone.parse().unwrap_or(chrono_tz::UTC);
                let d = utc.with_timezone(&tz);
                (
                    d.format(pattern).to_string(),
                    d.format("%Z").to_string(),
                    d.format("%p").to_string(),
                    d.format(footer_pattern).to_string(),
                    d.format("%S").to_string(),
                )
            };
            let context = if c.clock_format == "12" {
                format!("{zone} {suffix}")
            } else {
                zone
            };
            let status = if c.clock_seconds {
                format!("{second} SECONDS")
            } else {
                String::new()
            };
            card(
                "clock",
                wall,
                value,
                footer,
                format!("{} time · {}-hour clock", c.clock_zone, c.clock_format),
                json!({"context":context,"status":status}),
            )
        }
        "pomodoro" => {
            let t = timer(c, wall);
            let context = if c.timer_running {
                format!(
                    "{} {}/{}",
                    if t.phase == "LONG BREAK" {
                        "REST"
                    } else {
                        t.phase
                    },
                    t.session,
                    c.focus_sessions
                )
            } else {
                "PAUSED".into()
            };
            let next = if t.phase == "FOCUS" {
                format!(
                    "BREAK: {} MIN",
                    if t.session == c.focus_sessions as u64 {
                        c.long_break_minutes
                    } else {
                        c.break_minutes
                    }
                )
            } else {
                "NEXT: FOCUS".into()
            };
            card(
                "pomodoro",
                wall,
                format!("{:02}:{:02}", t.remaining / 60, t.remaining % 60),
                next,
                format!(
                    "{} · {} · session {} of {}",
                    t.phase,
                    if c.timer_running { "running" } else { "paused" },
                    t.session,
                    c.focus_sessions
                ),
                json!({"context":context,"progress":100.0*(t.duration-t.remaining) as f64/t.duration as f64,"phase":t.phase,"remainingSeconds":t.remaining}),
            )
        }
        "meeting" => {
            let starts = c.meeting_starts_at;
            let ends = starts + c.meeting_minutes as u64 * 60;
            let start = DateTime::<Utc>::from_timestamp(starts as i64, 0)
                .unwrap_or_default()
                .with_timezone(&Local);
            let (value, status, footer) = if starts == 0 {
                (
                    "SET TIME".into(),
                    "EDIT THIS WIDGET".into(),
                    "NO MEETING SET".into(),
                )
            } else if wall < starts {
                let left = starts - wall;
                (
                    if left >= 86400 {
                        format!("{}D {}H", left / 86400, left % 86400 / 3600)
                    } else if left >= 3600 {
                        format!("{}H {}M", left / 3600, left % 3600 / 60)
                    } else if left >= 60 {
                        format!("{} MIN", left.div_ceil(60))
                    } else {
                        format!("{left} SEC")
                    },
                    format!("STARTS {}", start.format("%H:%M")),
                    format!("{} MIN MEETING", c.meeting_minutes),
                )
            } else if wall < ends {
                (
                    "NOW".into(),
                    format!("{} MIN LEFT", (ends - wall).div_ceil(60)),
                    "MEETING STARTED".into(),
                )
            } else {
                (
                    "DONE".into(),
                    "MEETING ENDED".into(),
                    start.format("%a %b %d").to_string(),
                )
            };
            card(
                "meeting",
                wall,
                value,
                footer,
                if starts == 0 {
                    "Choose the meeting date/time in Edit".into()
                } else {
                    format!(
                        "{} · {} · {} minutes",
                        w.label,
                        start.format("%a %b %d %H:%M %Z"),
                        c.meeting_minutes
                    )
                },
                json!({"context":w.label,"status":status}),
            )
        }
        _ => unreachable!("Only dynamic local widget kinds"),
    }
}

pub fn cpu_card(w: &Widget, wall: u64, percent: f32, history: &[f32]) -> DataCard {
    card(
        "host-cpu",
        wall,
        format!("{:.0}%", percent.clamp(0.0, 100.0)),
        "HOST CPU LOAD".into(),
        format!(
            "Whole-computer CPU · {:.1}% · sampled every {}s",
            percent, w.config.monitor_interval
        ),
        json!({"context":"ALL CORES","progress":percent,"history":if w.config.show_graph {history.to_vec()} else {vec![]}}),
    )
}
pub fn memory_card(w: &Widget, wall: u64, used: u64, available: u64, total: u64) -> DataCard {
    let free = w.config.memory_metric == "available";
    let bytes = if free { available } else { used };
    let (divisor, unit) = if w.config.memory_unit == "gb" {
        (1e9, "GB")
    } else {
        (1073741824.0, "GIB")
    };
    let percent = 100.0 * bytes as f64 / total as f64;
    card(
        "host-memory",
        wall,
        format!("{:.1}{unit}", bytes as f64 / divisor),
        format!("OF {:.0}{unit} {:.0}%", total as f64 / divisor, percent),
        format!(
            "{} memory · {:.1} {unit} of {:.1} {unit}",
            if free { "Available" } else { "Used" },
            bytes as f64 / divisor,
            total as f64 / divisor
        ),
        json!({"context":if free {"FREE RAM"}else{"USED RAM"},"progress":percent}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timer_pause_resume_skip_and_long_sleep_are_derived_from_saved_anchor() {
        let mut c = SourceConfig::default();
        c.focus_minutes = 1;
        c.break_minutes = 1;
        c.long_break_minutes = 2;
        c.focus_sessions = 2;
        c.timer_running = true;
        c.timer_started_at = 1000;
        assert_eq!(
            (timer(&c, 1059).phase, timer(&c, 1059).remaining),
            ("FOCUS", 1)
        );
        assert_eq!(
            (timer(&c, 1060).phase, timer(&c, 1060).remaining),
            ("BREAK", 60)
        );
        assert_eq!(
            (timer(&c, 1180).phase, timer(&c, 1180).remaining),
            ("LONG BREAK", 120)
        );
        c.timer_elapsed = elapsed(&c, 1070);
        c.timer_running = false;
        assert_eq!(timer(&c, 9999).remaining, 50);
        c.timer_running = true;
        c.timer_started_at = 9999;
        assert_eq!(timer(&c, 10009).remaining, 40);
        let saved = serde_json::to_string(&c).unwrap();
        let restored: SourceConfig = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            timer(&restored, 10009 + 30000).remaining,
            timer(&c, 10009).remaining
        );
    }
    #[test]
    fn clock_timezones_and_meeting_boundaries_use_actual_epoch() {
        let mut w = Widget {
            id: "test".into(),
            kind: "clock".into(),
            label: "Team sync".into(),
            enabled: true,
            duration_sec: 3,
            headline: String::new(),
            footer: String::new(),
            config: SourceConfig::default(),
        };
        w.config.clock_zone = "Asia/Tokyo".into();
        w.config.clock_format = "24".into();
        assert_eq!(dynamic_card(&w, 1791833640).headline, "04:34");
        w.kind = "meeting".into();
        w.config.meeting_starts_at = 1000;
        w.config.meeting_minutes = 30;
        assert_eq!(dynamic_card(&w, 999).headline, "1 SEC");
        assert_eq!(dynamic_card(&w, 1000).headline, "NOW");
        assert_eq!(dynamic_card(&w, 2800).headline, "DONE");
    }
    #[test]
    fn memory_reports_selected_units_and_available_memory_without_inventing_values() {
        let w = Widget {
            id: "test".into(),
            kind: "memory".into(),
            label: "RAM".into(),
            enabled: true,
            duration_sec: 3,
            headline: String::new(),
            footer: String::new(),
            config: SourceConfig::default(),
        };
        let d = memory_card(
            &w,
            100,
            12 * 1024u64.pow(3),
            20 * 1024u64.pow(3),
            32 * 1024u64.pow(3),
        );
        assert_eq!(d.headline, "12.0GIB");
        assert_eq!(d.values["progress"], 37.5);
    }
}
