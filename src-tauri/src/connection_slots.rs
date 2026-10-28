//! Four attached droids can be used in a session. Remembered profiles are kept
//! separately, so unplugging a board never deletes its screens or settings.
use crate::connection::RobotPort;
use serde::Serialize;

pub const MAX_CONNECTED: usize = 4;
pub const LIMIT_MESSAGE: &str =
    "Up to 4 droids can connect at once. Unplug one of your other droids to use this one.";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortChoice {
    #[serde(flatten)]
    pub port: RobotPort,
    pub connectable: bool,
}

#[derive(Default)]
pub struct Slots {
    admitted: Vec<String>,
    initialized: bool,
}

impl Slots {
    pub fn refresh(
        &mut self,
        mut ports: Vec<RobotPort>,
        selected: Option<&str>,
    ) -> Vec<PortChoice> {
        // Enumeration order and the currently edited profile must not change
        // which four boards have slots. Only unplugging releases a slot.
        ports.sort_by_key(|p| p.serial.to_ascii_uppercase());
        ports.dedup_by(|a, b| a.serial.eq_ignore_ascii_case(&b.serial));
        self.admitted
            .retain(|id| ports.iter().any(|p| p.serial.eq_ignore_ascii_case(id)));
        if !self.initialized {
            if let Some(port) = ports
                .iter()
                .find(|p| selected.is_some_and(|id| p.serial.eq_ignore_ascii_case(id)))
            {
                self.admitted.push(port.serial.to_ascii_uppercase());
            }
            self.initialized = true;
        }
        for port in &ports {
            let id = port.serial.to_ascii_uppercase();
            if self.admitted.len() < MAX_CONNECTED && !self.admitted.contains(&id) {
                self.admitted.push(id);
            }
        }
        // Keep the arrows in attachment order, even if USB paths change.
        ports.sort_by_key(|p| {
            self.admitted
                .iter()
                .position(|id| p.serial.eq_ignore_ascii_case(id))
                .unwrap_or(MAX_CONNECTED)
        });
        ports
            .into_iter()
            .map(|port| {
                let connectable = self
                    .admitted
                    .iter()
                    .any(|id| port.serial.eq_ignore_ascii_case(id));
                PortChoice { port, connectable }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ports(ids: &[&str]) -> Vec<RobotPort> {
        ids.iter()
            .map(|id| RobotPort {
                path: format!("usb/{id}"),
                serial: (*id).into(),
                product: "DroidFun".into(),
                vid: 0x2e8a,
                pid: 3,
            })
            .collect()
    }
    fn allowed(choices: &[PortChoice]) -> Vec<&str> {
        choices
            .iter()
            .filter(|p| p.connectable)
            .map(|p| p.port.serial.as_str())
            .collect()
    }
    #[test]
    fn four_slots_survive_selection_enumeration_and_path_changes() {
        let mut slots = Slots::default();
        let first = slots.refresh(ports(&["E", "D", "C", "B", "A"]), Some("e"));
        assert_eq!(allowed(&first), ["E", "A", "B", "C"]);
        assert!(
            !first
                .iter()
                .find(|p| p.port.serial == "D")
                .unwrap()
                .connectable
        );
        let again = slots.refresh(ports(&["A", "C", "E", "D", "B"]), Some("B"));
        assert_eq!(allowed(&again), ["E", "A", "B", "C"]);
    }
    #[test]
    fn unplugging_releases_a_slot_without_displacing_connected_droids() {
        let mut slots = Slots::default();
        slots.refresh(ports(&["A", "B", "C", "D", "E"]), None);
        assert_eq!(
            allowed(&slots.refresh(ports(&["A", "C", "D", "E"]), Some("A"))),
            ["A", "C", "D", "E"]
        );
        let returned = slots.refresh(ports(&["A", "B", "C", "D", "E"]), Some("B"));
        assert_eq!(allowed(&returned), ["A", "C", "D", "E"]);
        assert!(
            !returned
                .iter()
                .find(|p| p.port.serial == "B")
                .unwrap()
                .connectable
        );
        assert!(slots.refresh(vec![], None).is_empty());
        assert_eq!(
            allowed(&slots.refresh(ports(&["E", "B"]), None)),
            ["B", "E"]
        );
    }
    #[test]
    fn aliases_and_casing_never_take_extra_slots() {
        let mut slots = Slots::default();
        let first = slots.refresh(ports(&["a", "A", "B", "C", "D"]), Some("A"));
        assert_eq!(first.len(), 4);
        assert_eq!(allowed(&first).len(), 4);
        assert_eq!(
            allowed(&slots.refresh(ports(&["A", "b", "C", "d", "E"]), None)),
            ["A", "b", "C", "d"]
        );
    }
}
