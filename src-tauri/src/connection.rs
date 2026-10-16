use crate::protocol::{self, Decoder, Packet};
use serde::{Deserialize, Serialize};
use serialport::{SerialPort, SerialPortType};
use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The observed Waveshare Arduino-Pico descriptor. Enumeration never opens a port.
const WAVESHARE_VID: u16 = 0x2e8a;
const WAVESHARE_PID: u16 = 0x0003;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RobotPort {
    pub path: String,
    pub serial: String,
    pub product: String,
    pub vid: u16,
    pub pid: u16,
}

pub fn robot_ports() -> Result<Vec<RobotPort>, String> {
    let ports = serialport::available_ports()
        .map_err(|error| format!("USB enumeration failed: {error}"))?;
    let candidates = ports
        .into_iter()
        .filter_map(|port| {
            let SerialPortType::UsbPort(usb) = port.port_type else {
                return None;
            };
            if usb.vid != WAVESHARE_VID || usb.pid != WAVESHARE_PID {
                return None;
            }
            let serial = usb.serial_number?;
            Some(RobotPort {
                path: port.port_name,
                serial,
                product: usb.product.unwrap_or_else(|| "RP2040 Zero".into()),
                vid: usb.vid,
                pid: usb.pid,
            })
        })
        .collect();
    Ok(unique_robots(candidates))
}

fn unique_robots(mut candidates: Vec<RobotPort>) -> Vec<RobotPort> {
    // macOS exposes both tty and cu aliases for the same USB device. Prefer
    // the callout endpoint and present one choice per physical board.
    candidates.sort_by_key(|port| (port.path.starts_with("/dev/tty."), port.path.clone()));
    let mut serials = HashSet::new();
    candidates.retain(|port| serials.insert(port.serial.clone()));
    candidates
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSnapshot {
    pub state: String,
    pub detail: String,
    pub port: Option<String>,
    pub board_id: Option<String>,
    pub firmware: Option<String>,
    pub display_health: Option<String>,
    pub standalone_supported: bool,
    pub standalone_saved: bool,
    pub standalone_count: u8,
}

impl Default for ConnectionSnapshot {
    fn default() -> Self {
        Self {
            state: "preview".into(),
            detail: "Choose a detected robot to connect".into(),
            port: None,
            board_id: None,
            firmware: None,
            display_health: None,
            standalone_supported: false,
            standalone_saved: false,
            standalone_count: 0,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelloAck {
    firmware: String,
    board_id: String,
    width: u16,
    height: u16,
    display_health: String,
    #[serde(default)]
    max_fps: u16,
    #[serde(default)]
    standalone_supported: bool,
    #[serde(default)]
    standalone_saved: bool,
    #[serde(default)]
    standalone_count: u8,
}

fn session_id() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    ((nanos as u32) ^ ((nanos >> 32) as u32) ^ std::process::id()).max(1)
}

fn exchange(
    port: &mut dyn SerialPort,
    decoder: &mut Decoder,
    request: &Packet,
    expected: u8,
) -> Result<Packet, String> {
    let wire = request.encode()?;
    for _ in 0..2 {
        port.write_all(&wire)
            .map_err(|e| format!("USB write failed: {e}"))?;
        let deadline = Instant::now()
            + Duration::from_millis(if request.kind == protocol::SAVED_COMMIT {
                3000
            } else {
                500
            });
        while Instant::now() < deadline {
            let mut input = [0u8; 128];
            match port.read(&mut input) {
                Ok(n) => {
                    for byte in &input[..n] {
                        if let Some(Ok(reply)) = decoder.push(*byte) {
                            if reply.session != request.session
                                || reply.sequence != request.sequence
                            {
                                continue;
                            }
                            if reply.kind == protocol::ERROR {
                                let message = String::from_utf8_lossy(&reply.payload);
                                return Err(format!("Robot rejected command: {message}"));
                            }
                            if reply.kind == expected {
                                return Ok(reply);
                            }
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
                Err(error) => return Err(format!("USB read failed: {error}")),
            }
        }
        decoder.reset();
    }
    Err("No reply from the bot after two attempts. Reconnect it and check its firmware.".into())
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkMetrics {
    pub command_acks: u64,
    pub frame_acks: u64,
    pub last_frame_crc: Option<u32>,
    pub last_ack_at: u64,
}

fn validate_frame_ack(payload: &[u8], health: &serde_json::Value) -> Result<u32, String> {
    let expected = protocol::crc32(payload);
    if health["displayHealth"] != "ready" || health["frameCrc"].as_u64() != Some(expected as u64) {
        return Err("Robot did not confirm the expected frame CRC and display health".into());
    }
    Ok(expected)
}

fn validate_saved_ack(ack: &serde_json::Value, crc: u32, count: usize) -> Result<(), String> {
    if ack["saved"] != true
        || ack["count"].as_u64() != Some(count as u64)
        || ack["playlistCrc"].as_u64() != Some(crc as u64)
    {
        return Err("The bot did not confirm that the complete playlist was saved. Reconnect and try again.".into());
    }
    Ok(())
}

pub struct Link {
    port: Box<dyn SerialPort>,
    decoder: Decoder,
    session: u32,
    sequence: u32,
    last_command: Instant,
    last_frame: Vec<u8>,
    last_frame_at: Instant,
    frame_interval: Duration,
    last_display: Option<(bool, u8, u16)>,
    pub status: ConnectionSnapshot,
    pub metrics: LinkMetrics,
}

impl Link {
    pub fn connect(candidate: &RobotPort) -> Result<Self, String> {
        // The caller has explicitly selected a serial number from the approved USB descriptor.
        let mut port = serialport::new(&candidate.path, 115_200)
            .timeout(Duration::from_millis(50))
            .open()
            .map_err(|error| {
                if cfg!(target_os = "linux")
                    && error.kind()
                        == serialport::ErrorKind::Io(std::io::ErrorKind::PermissionDenied)
                {
                    crate::linux_usb::PERMISSION_DETAIL.to_string()
                } else {
                    format!("Could not open robot port: {error}")
                }
            })?;
        port.write_data_terminal_ready(true)
            .map_err(|error| format!("Could not enable USB serial replies: {error}"))?;
        let mut decoder = Decoder::new();
        let session = session_id();
        let hello = Packet::new(
            protocol::HELLO,
            session,
            1,
            br#"{"client":"Deskbot","protocol":1}"#.to_vec(),
        );
        let response = exchange(&mut *port, &mut decoder, &hello, protocol::HELLO_ACK)?;
        let ack: HelloAck = serde_json::from_slice(&response.payload)
            .map_err(|_| "Robot returned an invalid DroidFun App handshake".to_string())?;
        if ack.board_id.is_empty()
            || ack.board_id.len() > 128
            || ack.firmware.len() > 64
            || ack.width != 128
            || ack.height != 64
        {
            return Err("Robot firmware or screen size is incompatible".into());
        }
        if !ack.board_id.eq_ignore_ascii_case(&candidate.serial) {
            return Err("Firmware board ID does not match the selected USB device".into());
        }
        Ok(Self {
            port,
            decoder,
            session,
            sequence: 2,
            last_command: Instant::now(),
            last_frame: Vec::new(),
            last_frame_at: Instant::now() - Duration::from_secs(1),
            frame_interval: Duration::from_millis(
                1000 / if ack.max_fps == 0 {
                    5
                } else {
                    ack.max_fps.min(5)
                } as u64,
            ),
            last_display: None,
            metrics: LinkMetrics::default(),
            status: ConnectionSnapshot {
                state: if ack.display_health == "ready" {
                    "connected"
                } else {
                    "display_error"
                }
                .into(),
                detail: if ack.display_health == "ready" {
                    "DroidFun App handshake succeeded"
                } else {
                    "USB responds, but the OLED reports a display fault"
                }
                .into(),
                port: Some(candidate.path.clone()),
                board_id: Some(ack.board_id),
                firmware: Some(ack.firmware),
                display_health: Some(ack.display_health),
                standalone_supported: ack.standalone_supported,
                standalone_saved: ack.standalone_saved,
                standalone_count: ack.standalone_count,
            },
        })
    }

    fn command(&mut self, kind: u8, payload: Vec<u8>) -> Result<serde_json::Value, String> {
        let packet = Packet::new(kind, self.session, self.sequence, payload);
        // Reserve the next sequence even on a rejection or lost response. Reusing
        // it for different bytes would collide with the firmware's retry cache.
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Sequence limit reached; reconnect required")?;
        let reply = exchange(&mut *self.port, &mut self.decoder, &packet, protocol::ACK)?;
        let health: serde_json::Value = serde_json::from_slice(&reply.payload)
            .map_err(|_| "Robot returned invalid ACK health")?;
        if kind == protocol::FRAME {
            self.metrics.last_frame_crc = Some(validate_frame_ack(&packet.payload, &health)?);
            self.metrics.frame_acks += 1;
        }
        self.metrics.command_acks += 1;
        self.metrics.last_ack_at = crate::core::unix_now();
        self.last_command = Instant::now();
        if health["hostFrameNeeded"] == true {
            self.last_frame.clear();
            self.last_display = None;
        }
        // Firmware ACKs carry bounded JSON health, so a display fault stays visible.
        {
            #[cfg(feature = "device-smoke")]
            eprintln!("reply kind={kind:#x}: {health}");
            if let Some(value) = health.get("displayHealth").and_then(|v| v.as_str()) {
                self.status.display_health = Some(value.chars().take(64).collect());
                if value != "ready" {
                    self.status.state = "display_error".into();
                    self.status.detail = format!("Display reports {value}");
                } else {
                    self.status.state = "connected".into();
                    self.status.detail =
                        "Robot acknowledged the playlist. Check the OLED to confirm the image."
                            .into();
                }
            }
        }
        Ok(health)
    }

    pub fn save_standalone(
        &mut self,
        playlist: &protocol::StandalonePlaylist,
    ) -> Result<(), String> {
        if !self.status.standalone_supported {
            return Err(
                "Install bot firmware 1.2.0 or newer from Settings → Robot before saving screens"
                    .into(),
            );
        }
        let bytes = playlist.encode()?;
        let checksum = protocol::crc32(&bytes);
        let mut begin = (bytes.len() as u16).to_le_bytes().to_vec();
        begin.extend_from_slice(&checksum.to_le_bytes());
        self.command(protocol::SAVED_BEGIN, begin)?;
        for (index, chunk) in bytes.chunks(protocol::MAX_PAYLOAD - 2).enumerate() {
            let mut payload = ((index * (protocol::MAX_PAYLOAD - 2)) as u16)
                .to_le_bytes()
                .to_vec();
            payload.extend_from_slice(chunk);
            self.command(protocol::SAVED_CHUNK, payload)?;
        }
        let ack = self.command(protocol::SAVED_COMMIT, Vec::new())?;
        validate_saved_ack(&ack, checksum, playlist.frames.len())?;
        self.status.standalone_saved = true;
        self.status.standalone_count = playlist.frames.len() as u8;
        self.status.detail = "Screens saved and verified on the bot".into();
        // Restore the desktop display after an upload, even if the frame hasn't changed.
        self.last_frame.clear();
        self.last_display = None;
        Ok(())
    }

    pub fn update(
        &mut self,
        frame: &[u8],
        display_on: bool,
        brightness: u8,
        orientation: u16,
    ) -> Result<bool, String> {
        // The firmware may have replaced our cached frame with its fallback
        // after a stalled host. Restore display policy and pixels even if the
        // current static widget has not changed since the last ACK.
        if self.last_command.elapsed() >= Duration::from_secs(4) {
            self.last_frame.clear();
            self.last_display = None;
        }
        let mut changed = false;
        if self.last_display != Some((display_on, brightness, orientation)) {
            let contrast = ((brightness as u16 * 255 + 50) / 100) as u8;
            let payload = serde_json::to_vec(&serde_json::json!({
                "on": display_on, "contrast": contrast, "orientation": orientation
            }))
            .map_err(|e| e.to_string())?;
            self.command(protocol::DISPLAY, payload)?;
            self.last_display = Some((display_on, brightness, orientation));
            changed = true;
        }
        if frame != self.last_frame
            && display_on
            && self.last_frame_at.elapsed() >= self.frame_interval
        {
            if frame.len() != protocol::MAX_PAYLOAD {
                return Err("Renderer supplied an invalid frame".into());
            }
            self.command(protocol::FRAME, frame.to_vec())?;
            self.last_frame = frame.to_vec();
            self.last_frame_at = Instant::now();
            changed = true;
        }
        if self.last_command.elapsed() >= Duration::from_secs(2) {
            self.command(protocol::PING, Vec::new())?;
            changed = true;
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_ack_requires_matching_persistent_playlist() {
        assert!(validate_saved_ack(
            &serde_json::json!({"saved":true,"count":2,"playlistCrc":42}),
            42,
            2
        )
        .is_ok());
        for ack in [
            serde_json::json!({"saved":true,"count":2}),
            serde_json::json!({"saved":false,"count":2,"playlistCrc":42}),
            serde_json::json!({"saved":true,"count":1,"playlistCrc":42}),
            serde_json::json!({"saved":true,"count":2,"playlistCrc":43}),
        ] {
            assert!(validate_saved_ack(&ack, 42, 2).is_err());
        }
    }
    #[test]
    fn frame_ack_must_confirm_the_sent_bitmap() {
        let frame = vec![0x55; 1024];
        let crc = protocol::crc32(&frame);
        assert!(validate_frame_ack(
            &frame,
            &serde_json::json!({"displayHealth":"ready","frameCrc":crc})
        )
        .is_ok());
        for health in [
            serde_json::json!({"displayHealth":"ready","frameCrc":crc ^ 1}),
            serde_json::json!({"displayHealth":"ready"}),
            serde_json::json!({"displayHealth":"i2c_error","frameCrc":crc}),
        ] {
            assert!(validate_frame_ack(&frame, &health).is_err());
        }
    }
    #[test]
    fn initial_status_has_no_selected_port() {
        let snapshot = ConnectionSnapshot::default();
        assert_eq!(snapshot.state, "preview");
        assert!(snapshot.port.is_none());
    }

    #[test]
    fn macos_aliases_produce_one_robot_and_prefer_callout_port() {
        let make = |path: &str, serial: &str| RobotPort {
            path: path.into(),
            serial: serial.into(),
            product: "RP2040 Zero".into(),
            vid: WAVESHARE_VID,
            pid: WAVESHARE_PID,
        };
        let result = unique_robots(vec![
            make("/dev/tty.usbmodem1101", "first"),
            make("/dev/cu.usbmodem1101", "first"),
            make("/dev/cu.usbmodem1201", "second"),
        ]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path, "/dev/cu.usbmodem1101");
        assert_eq!(result[1].serial, "second");
    }
}
