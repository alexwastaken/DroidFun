//! Quiet clock and calendar layouts for Workshop. Both use the computer's real clock.
use crate::core::{SourceConfig, Widget, FRAME_BYTES};
use chrono::{DateTime, Local, TimeZone, Utc};
use font8x8::{UnicodeFonts, BASIC_FONTS};
pub(crate) const CALENDAR_ID: &str = "workshop-calendar";
struct Parts {
    time: String,
    period: String,
    month: String,
    day: String,
    weekday: String,
    year: String,
}
fn formatted<T: TimeZone>(d: DateTime<T>, twelve: bool) -> Parts
where
    T::Offset: std::fmt::Display,
{
    Parts {
        time: d
            .format(if twelve { "%-I:%M" } else { "%H:%M" })
            .to_string(),
        period: if twelve {
            d.format("%p").to_string()
        } else {
            String::new()
        },
        month: d.format("%B").to_string().to_uppercase(),
        day: d.format("%-d").to_string(),
        weekday: d.format("%A").to_string().to_uppercase(),
        year: d.format("%Y").to_string(),
    }
}
fn parts(config: &SourceConfig, wall: u64) -> Parts {
    let d =
        DateTime::<Utc>::from_timestamp(i64::try_from(wall).unwrap_or(0), 0).unwrap_or_default();
    if config.clock_zone == "local" {
        formatted(d.with_timezone(&Local), config.clock_format == "12")
    } else {
        formatted(
            d.with_timezone(
                &config
                    .clock_zone
                    .parse::<chrono_tz::Tz>()
                    .unwrap_or(chrono_tz::UTC),
            ),
            config.clock_format == "12",
        )
    }
}
fn centered(frame: &mut [u8], text: &str, y: usize, scale: usize) {
    let width = text.chars().count() * 8 * scale;
    let x0 = 128usize.saturating_sub(width) / 2;
    for (i, ch) in text.chars().enumerate() {
        if let Some(glyph) = BASIC_FONTS.get(ch) {
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) != 0 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                let x = x0 + (i * 8 + col) * scale + dx;
                                let yy = y + row * scale + dy;
                                if x < 128 && yy < 64 {
                                    frame[(yy / 8) * 128 + x] |= 1 << (yy % 8);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
pub(crate) fn render(widget: &Widget, wall: u64) -> Vec<u8> {
    let p = parts(&widget.config, wall);
    let mut frame = vec![0; FRAME_BYTES];
    if widget.id == CALENDAR_ID {
        centered(&mut frame, &p.month, 10, 1);
        centered(&mut frame, &p.day, 25, 3);
        match widget.config.clock_date.as_str() {
            "none" => {}
            "date" => centered(&mut frame, &p.year, 53, 1),
            _ => centered(&mut frame, &p.weekday, 53, 1),
        }
    } else {
        centered(&mut frame, &p.time, 20, 3);
        if !p.period.is_empty() {
            centered(&mut frame, &p.period, 51, 1);
        }
    }
    if widget.config.display_invert {
        for b in &mut frame {
            *b = !*b;
        }
    }
    frame
}
// Represent Calendar as another clock widget, so older Workshop versions can still read the settings.
pub(crate) fn add_calendar(settings: &mut crate::core::Settings) {
    if settings.widgets.iter().any(|w| w.id == CALENDAR_ID) || settings.widgets.len() >= 30 {
        return;
    }
    let mut config = settings
        .widgets
        .iter()
        .find(|w| w.kind == "clock")
        .map(|w| w.config.clone())
        .unwrap_or_default();
    config.mode = "live".into();
    config.clock_date = "weekday".into();
    config.clock_seconds = false;
    let index = settings
        .widgets
        .iter()
        .position(|w| w.kind == "clock")
        .map(|n| n + 1)
        .unwrap_or(settings.widgets.len());
    settings.widgets.insert(
        index,
        Widget {
            id: CALENDAR_ID.into(),
            kind: "clock".into(),
            label: "Calendar".into(),
            enabled: false,
            duration_sec: 8,
            headline: String::new(),
            footer: String::new(),
            config,
        },
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    fn widget(calendar: bool) -> Widget {
        Widget {
            id: if calendar { CALENDAR_ID } else { "clock" }.into(),
            kind: "clock".into(),
            label: "Clock".into(),
            enabled: true,
            duration_sec: 8,
            headline: String::new(),
            footer: String::new(),
            config: SourceConfig {
                clock_zone: "UTC".into(),
                ..Default::default()
            },
        }
    }
    fn epoch(s: &str) -> u64 {
        DateTime::parse_from_rfc3339(s).unwrap().timestamp() as u64
    }
    #[test]
    fn clock_is_time_only_and_calendar_tracks_the_selected_time_zone() {
        let mut w = widget(false);
        let t = epoch("2026-11-02T13:05:59Z");
        let p = parts(&w.config, t);
        assert_eq!(p.time, "1:05");
        assert_eq!(p.period, "PM");
        let frame = render(&w, t);
        assert_eq!(frame.len(), 1024);
        assert!(frame.iter().any(|b| *b != 0));
        assert_eq!(frame, render(&w, t - 59));
        w.config.clock_format = "24".into();
        assert_eq!(parts(&w.config, t).time, "13:05");
        assert_ne!(frame, render(&w, t));
        let mut calendar = widget(true);
        calendar.config.clock_zone = "America/Los_Angeles".into();
        let before = epoch("2027-01-01T07:59:00Z");
        let after = before + 60;
        let p = parts(&calendar.config, before);
        assert_eq!(
            (
                p.month.as_str(),
                p.day.as_str(),
                p.weekday.as_str(),
                p.year.as_str()
            ),
            ("DECEMBER", "31", "THURSDAY", "2026")
        );
        let p = parts(&calendar.config, after);
        assert_eq!(
            (
                p.month.as_str(),
                p.day.as_str(),
                p.weekday.as_str(),
                p.year.as_str()
            ),
            ("JANUARY", "1", "FRIDAY", "2027")
        );
        assert_ne!(render(&calendar, before), render(&calendar, after));
        let p = parts(&calendar.config, epoch("2028-02-29T20:00:00Z"));
        assert_eq!(p.day, "29");
        assert_eq!(p.month, "FEBRUARY");
    }
    #[test]
    fn migration_preserves_existing_playlist_and_does_not_duplicate_calendar() {
        let mut e =
            crate::core::Engine::new(crate::core::Settings::default(), std::time::Instant::now());
        e.add_widget("clock").unwrap();
        let mut s = e.settings;
        let old_clock = s.widgets.iter_mut().find(|w| w.kind == "clock").unwrap();
        old_clock.config.clock_zone = "Asia/Tokyo".into();
        old_clock.duration_sec = 17;
        old_clock.enabled = false;
        let before = s.widgets.clone();
        add_calendar(&mut s);
        add_calendar(&mut s);
        s.validate().unwrap();
        assert_eq!(s.widgets.iter().filter(|w| w.id == CALENDAR_ID).count(), 1);
        assert_eq!(
            s.widgets
                .iter()
                .filter(|w| w.id != CALENDAR_ID)
                .cloned()
                .collect::<Vec<_>>(),
            before
                .into_iter()
                .filter(|w| w.id != CALENDAR_ID)
                .collect::<Vec<_>>()
        );
        let calendar = s.widgets.iter().find(|w| w.id == CALENDAR_ID).unwrap();
        assert!(!calendar.enabled);
        assert_eq!(calendar.config.clock_zone, "Asia/Tokyo");
    }
    #[test]
    fn rendered_layouts_fit_and_honor_inversion() {
        let t = epoch("2026-11-02T13:05:00Z");
        let mut previews = Vec::new();
        for (name, calendar, format) in [
            ("clock-12", false, "12"),
            ("clock-24", false, "24"),
            ("calendar", true, "12"),
        ] {
            let mut w = widget(calendar);
            w.config.clock_format = format.into();
            let frame = render(&w, t);
            // Leave the first nine rows empty below the housing lip.
            for y in 0..9 {
                for x in 0..128 {
                    assert_eq!(frame[(y / 8) * 128 + x] & (1 << (y % 8)), 0);
                }
            }
            w.config.display_invert = true;
            assert_eq!(render(&w, t), frame.iter().map(|b| !*b).collect::<Vec<_>>());
            previews.push(serde_json::json!({"name":name,"frame":frame}));
        }
        if let Ok(path) = std::env::var("WORKSHOP_CLOCK_PREVIEW_FILE") {
            std::fs::write(path, serde_json::to_vec(&previews).unwrap()).unwrap();
        }
    }
}
