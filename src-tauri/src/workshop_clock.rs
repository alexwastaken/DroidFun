//! Workshop adds an optional Calendar without changing the existing playlist.
use crate::core::Widget;
pub(crate) const CALENDAR_ID: &str = "workshop-calendar";
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
}
