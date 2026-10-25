//! Bounded reader for the standalone screen diagnostic sketch.
//! The caller must suspend the normal link worker before reading this port.
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const MAX_BYTES: usize = 16 * 1024;
const MAX_LINE: usize = 2 * 1024;
static READING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Report {
    pub kind: String,
    pub version: u8,
    #[serde(rename = "boardId")]
    pub board_id: String,
    pub uptime_ms: u64,
    /// Logic levels measured on the standard SDA and SCL pins.
    pub sda: u8,
    pub scl: u8,
    pub ack_3c: bool,
    pub ack_3d: bool,
    pub address: Option<u8>,
    pub phase: String,
    pub write_ok: bool,
    pub rgb_ready: bool,
}

fn valid_identity(value: &str) -> bool {
    value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_report(line: &[u8], expected: &str) -> Option<Report> {
    let report: Report = serde_json::from_slice(line).ok()?;
    if report.kind != "droidfun_display_test"
        || report.version != 1
        || !valid_identity(&report.board_id)
        || !report.board_id.eq_ignore_ascii_case(expected)
        || report.sda > 1
        || report.scl > 1
        || !matches!(report.address, None | Some(0x3c | 0x3d))
        || !matches!(
            report.phase.as_str(),
            "starting"
                | "no_ack"
                | "write_error"
                | "ssd1306_all_on"
                | "ssd1306_checker"
                | "sh1106_all_on"
                | "sh1106_checker"
        )
    {
        return None;
    }
    Some(report)
}

#[derive(Default)]
struct Reports {
    total: usize,
    line: Vec<u8>,
    discard_line: bool,
}

impl Reports {
    fn feed(&mut self, bytes: &[u8], expected: &str) -> Result<Option<Report>, String> {
        self.total = self.total.saturating_add(bytes.len());
        if self.total > MAX_BYTES {
            return Err(
                "The display test sent too much data. Its result was not confirmed.".into(),
            );
        }
        for &byte in bytes {
            if byte == b'\n' {
                let report = if self.discard_line {
                    None
                } else {
                    parse_report(&self.line, expected)
                };
                self.line.clear();
                self.discard_line = false;
                if report.is_some() {
                    return Ok(report);
                }
            } else if !self.discard_line {
                if self.line.len() == MAX_LINE {
                    self.line.clear();
                    self.discard_line = true;
                } else {
                    self.line.push(byte);
                }
            }
        }
        Ok(None)
    }
}

struct ReadGuard;
impl Drop for ReadGuard {
    fn drop(&mut self) {
        READING.store(false, Ordering::SeqCst);
    }
}

/// Read one identity-checked report without sending any sketch commands.
/// This never installs firmware or starts the normal DroidFun protocol worker.
pub fn read(serial: &str, cancel: &AtomicBool) -> Result<Report, String> {
    READING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "A display test read is already running.".to_string())?;
    let _guard = ReadGuard;
    check_cancel(cancel)?;
    if !valid_identity(serial) {
        return Err("Choose a detected bot before reading its display test.".into());
    }
    let candidate = crate::connection::robot_ports()?
        .into_iter()
        .find(|port| port.serial.eq_ignore_ascii_case(serial))
        .ok_or("The test bot is no longer on USB. Reconnect it and scan again.")?;
    check_cancel(cancel)?;
    let mut port = serialport::new(&candidate.path, 115_200)
        .dtr_on_open(true)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|_| "Could not open the test bot. Close other serial tools and try again.")?;
    // Native driver calls cannot be interrupted. Keep the read guard until
    // they return, then abandon the job before doing any further USB work.
    check_cancel(cancel)?;
    port.write_data_terminal_ready(true)
        .map_err(|_| "Could not enable the display test’s USB report.")?;
    check_cancel(cancel)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut reports = Reports::default();
    let mut bytes = [0u8; 512];
    while Instant::now() < deadline {
        check_cancel(cancel)?;
        let result = port.read(&mut bytes);
        check_cancel(cancel)?;
        match result {
            Ok(0) => std::thread::sleep(Duration::from_millis(10)),
            Ok(count) => {
                if let Some(report) = reports.feed(&bytes[..count], &candidate.serial)? {
                    return Ok(report);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => {
                return Err("The display test’s USB connection was interrupted. Its result was not confirmed.".into());
            }
        }
    }
    Err("No matching display test report arrived within five seconds. The test is not confirmed; you can read again or restore normal software.".into())
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::SeqCst) {
        Err("The display test read was canceled. Its result was not confirmed.".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: &str = "53032847490B8B1C";

    fn fixture() -> serde_json::Value {
        serde_json::json!({
            "kind": "droidfun_display_test", "version": 1, "boardId": BOARD,
            "uptime_ms": 1250, "sda": 1, "scl": 1, "ack_3c": false,
            "ack_3d": false, "address": null, "phase": "no_ack",
            "write_ok": false, "rgb_ready": true
        })
    }

    fn parse(value: &serde_json::Value) -> Option<Report> {
        parse_report(&serde_json::to_vec(value).unwrap(), BOARD)
    }

    #[test]
    fn validates_identity_marker_and_version() {
        let report = parse(&fixture()).unwrap();
        assert_eq!(report.board_id, BOARD);
        assert_eq!(report.address, None);
        assert!(parse_report(
            &serde_json::to_vec(&fixture()).unwrap(),
            &BOARD.to_lowercase()
        )
        .is_some());
        for (field, value) in [
            ("kind", serde_json::json!("other_sketch")),
            ("version", serde_json::json!(2)),
            ("boardId", serde_json::json!("53032847490B8B1D")),
            ("boardId", serde_json::json!("53032847490B8B1Z")),
        ] {
            let mut wrong = fixture();
            wrong[field] = value;
            assert!(parse(&wrong).is_none(), "accepted invalid {field}");
        }
    }

    #[test]
    fn validates_levels_addresses_phases_and_types() {
        for address in [0x3c, 0x3d] {
            let mut good = fixture();
            good["address"] = serde_json::json!(address);
            good["phase"] = serde_json::json!("ssd1306_checker");
            assert!(parse(&good).is_some());
        }
        for (field, value) in [
            ("sda", serde_json::json!(2)),
            ("scl", serde_json::json!(-1)),
            ("address", serde_json::json!(0x3e)),
            ("address", serde_json::json!("0x3c")),
            ("phase", serde_json::json!("passed")),
            ("uptime_ms", serde_json::json!(-1)),
            ("write_ok", serde_json::json!("true")),
            ("rgb_ready", serde_json::json!(null)),
        ] {
            let mut wrong = fixture();
            wrong[field] = value;
            assert!(parse(&wrong).is_none(), "accepted invalid {field}");
        }
        assert!(parse_report(b"{bad json}", BOARD).is_none());
        let mut missing = fixture();
        missing.as_object_mut().unwrap().remove("ack_3c");
        assert!(parse(&missing).is_none());
    }

    #[test]
    fn skips_foreign_and_oversized_lines_then_reads_split_report() {
        let mut stream = Reports::default();
        assert!(stream
            .feed(b"unrelated boot message\n", BOARD)
            .unwrap()
            .is_none());
        let mut overlong = vec![b'x'; MAX_LINE + 1];
        overlong.push(b'\n');
        assert!(stream.feed(&overlong, BOARD).unwrap().is_none());
        assert!(stream.line.capacity() <= MAX_LINE);
        let json = serde_json::to_vec(&fixture()).unwrap();
        let middle = json.len() / 2;
        assert!(stream.feed(&json[..middle], BOARD).unwrap().is_none());
        assert!(stream.feed(&json[middle..], BOARD).unwrap().is_none());
        assert!(stream.feed(b"\r\n", BOARD).unwrap().is_some());
    }

    #[test]
    fn caps_total_bytes_without_waiting_for_newline() {
        let mut stream = Reports::default();
        assert!(stream
            .feed(&vec![b'x'; MAX_BYTES], BOARD)
            .unwrap()
            .is_none());
        assert!(stream.feed(b"x", BOARD).is_err());
        assert!(stream.line.len() <= MAX_LINE);
    }
}
