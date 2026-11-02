use crate::workshop_protocol::{self as protocol, Decoder, Packet};
use serde::{Deserialize, Serialize};
use serialport::{SerialPort, SerialPortType};
use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The observed Waveshare Arduino-Pico descriptor. Enumeration never opens a port.
const WAVESHARE_VID: u16 = 0x2e8a;
const WAVESHARE_PID: u16 = 0x0003;

const USB_IN_USE_DETAIL: &str = "This droid’s USB connection is in use by another app. Quit DroidFun App or any serial monitor, then choose Check again. The droid does not need to be reset.";
const NO_REPLY_DETAIL: &str = "USB was detected, but the droid did not reply. Close other apps using it, reconnect its USB data cable, then choose Check again. This does not mean its firmware is missing.";

fn open_error(error: serialport::Error) -> String {
    // OS exclusivity protects a board from simultaneous app/Workshop access.
    // An occupied port is not evidence of missing firmware.
    let description = error.description.to_ascii_lowercase();
    if error.kind() == serialport::ErrorKind::Io(std::io::ErrorKind::WouldBlock)
        || description.contains("resource busy")
        || description.contains("exclusive lock")
        || description.contains("sharing violation")
        || description.contains("being used by another process")
    {
        USB_IN_USE_DETAIL.into()
    } else if cfg!(target_os = "linux")
        && error.kind() == serialport::ErrorKind::Io(std::io::ErrorKind::PermissionDenied)
    {
        crate::linux_usb::PERMISSION_DETAIL.into()
    } else {
        format!("Could not open droid USB connection: {error}. Close other USB apps and check the cable, then choose Check again.")
    }
}

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
    candidates.retain(|port| serials.insert(port.serial.to_ascii_uppercase()));
    candidates
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    pub model: String,
    pub hardware_profile: String,
    pub generation: u32,
}

fn validated_identity(value: Option<&serde_json::Value>) -> Option<DeviceIdentity> {
    let value = value?;
    let model = value.get("model")?.as_str()?;
    let hardware_profile = value.get("hardwareProfile")?.as_str()?;
    let generation = u32::try_from(value.get("generation")?.as_u64()?).ok()?;
    if !protocol::FACTORY_MODELS.contains(&model)
        || hardware_profile != protocol::IDENTITY_HARDWARE_PROFILE
        || generation == 0
    {
        return None;
    }
    Some(DeviceIdentity {
        model: model.into(),
        hardware_profile: hardware_profile.into(),
        generation,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSnapshot {
    pub state: String,
    pub detail: String,
    pub port: Option<String>,
    pub board_id: Option<String>,
    pub firmware: Option<String>,
    pub pin_profile: Option<String>,
    pub display_health: Option<String>,
    pub standalone_supported: bool,
    pub standalone_saved: bool,
    pub standalone_count: u8,
    pub standalone_content_crc: Option<u32>,
    pub identity: Option<DeviceIdentity>,
    pub provisioning_supported: bool,
}

impl Default for ConnectionSnapshot {
    fn default() -> Self {
        Self {
            state: "preview".into(),
            detail: "Choose a detected robot to connect".into(),
            port: None,
            board_id: None,
            firmware: None,
            pin_profile: None,
            display_health: None,
            standalone_supported: false,
            standalone_saved: false,
            standalone_count: 0,
            standalone_content_crc: None,
            identity: None,
            provisioning_supported: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelloAck {
    firmware: String,
    #[serde(default)]
    profile: Option<String>,
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
    #[serde(default)]
    standalone_content_crc: Option<u32>,
    #[serde(default)]
    identity: Option<serde_json::Value>,
    #[serde(default)]
    provisioning_supported: bool,
}

fn session_id() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    ((nanos as u32) ^ ((nanos >> 32) as u32) ^ std::process::id()).max(1)
}

fn exchange<T: std::io::Read + std::io::Write + ?Sized>(
    port: &mut T,
    decoder: &mut Decoder,
    request: &Packet,
    expected: u8,
) -> Result<Packet, String> {
    let wire = request.encode()?;
    for attempt in 0..2 {
        port.write_all(&wire)
            .map_err(|e| format!("USB write failed: {e}"))?;
        let deadline = Instant::now()
            + Duration::from_millis(
                if matches!(request.kind, protocol::SAVED_COMMIT | protocol::PROVISION) {
                    3000
                } else if request.kind == protocol::HELLO && attempt == 0 {
                    // Let newly attached firmware finish startup and enable
                    // serial replies before closing and reopening its port.
                    1500
                } else {
                    500
                },
            );
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
                                if serde_json::from_slice::<serde_json::Value>(&reply.payload)
                                    .ok()
                                    .is_some_and(|v| v["code"] == "i2c_error")
                                {
                                    return Err("USB is connected, but the screen is not responding. Reconnect power; if this continues, contact Droid Fun support.".into());
                                }
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
    Err(NO_REPLY_DETAIL.into())
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

fn validate_saved_ack(
    ack: &serde_json::Value,
    crc: u32,
    count: usize,
    content_crc: u32,
) -> Result<Option<u32>, String> {
    if ack["saved"] != true
        || ack["count"].as_u64() != Some(count as u64)
        || ack["playlistCrc"].as_u64() != Some(crc as u64)
    {
        return Err("The bot did not confirm that the complete playlist was saved. Reconnect and try again.".into());
    }
    // Firmware before 1.3.0 has no content fingerprint. A present field must
    // match exactly; the old full-file transport CRC is a constant residue for
    // valid DFP1 files and cannot establish which face is persisted.
    match ack.get("standaloneContentCrc") {
        None => Ok(None),
        Some(value) if value.as_u64() == Some(content_crc as u64) => Ok(Some(content_crc)),
        Some(_) => Err(
            "The bot did not confirm the expected saved screen content. Reconnect and try again."
                .into(),
        ),
    }
}

fn validate_provision_ack(
    ack: &serde_json::Value,
    model: &str,
    generation: u32,
) -> Result<DeviceIdentity, String> {
    let identity = validated_identity(ack.get("identity"))
        .ok_or("The board did not return a valid persisted model identity.")?;
    if ack["provisioned"] != true || identity.model != model || identity.generation != generation {
        return Err(
            "The board did not confirm the requested model identity and generation.".into(),
        );
    }
    Ok(identity)
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
            .map_err(open_error)?;
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
                pin_profile: ack.profile,
                display_health: Some(ack.display_health),
                standalone_supported: ack.standalone_supported,
                standalone_saved: ack.standalone_saved,
                standalone_count: ack.standalone_count,
                standalone_content_crc: ack.standalone_content_crc,
                identity: validated_identity(ack.identity.as_ref()),
                provisioning_supported: ack.provisioning_supported,
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

    /// Workshop only: explicitly persist the model, bound to the selected UID.
    /// A successful readback is not a physical power-cycle or screen check.
    pub fn provision_identity(
        &mut self,
        model: &str,
        expected_board_id: &str,
    ) -> Result<DeviceIdentity, String> {
        if !self.status.provisioning_supported {
            return Err("Install identity-capable firmware before programming the model.".into());
        }
        if !self
            .status
            .board_id
            .as_deref()
            .is_some_and(|id| id.eq_ignore_ascii_case(expected_board_id))
        {
            return Err("The connected board does not match the selected flash identity.".into());
        }
        let payload = protocol::provision_payload(model, expected_board_id)?;
        let expected_generation = self
            .status
            .identity
            .as_ref()
            .map_or(Some(1), |identity| {
                if identity.model == model {
                    Some(identity.generation)
                } else {
                    identity.generation.checked_add(1)
                }
            })
            .ok_or("The board identity generation cannot be advanced.")?;
        let ack = self.command(protocol::PROVISION, payload)?;
        let identity = validate_provision_ack(&ack, model, expected_generation)?;
        self.status.identity = Some(identity.clone());
        Ok(identity)
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
        // encode() writes generation zero; persisted firmware normalizes that
        // field to zero before hashing the same body without its CRC trailer.
        let content_crc = protocol::crc32(&bytes[..bytes.len() - 4]);
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
        self.status.standalone_content_crc =
            validate_saved_ack(&ack, checksum, playlist.frames.len(), content_crc)?;
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
    #[test]
    fn occupied_usb_does_not_suggest_reinstalling_firmware() {
        for error in [
            serialport::Error::new(serialport::ErrorKind::NoDevice, "Resource busy"),
            serialport::Error::new(serialport::ErrorKind::NoDevice, "Unable to acquire exclusive lock on serial port"),
            serialport::Error::new(serialport::ErrorKind::Io(std::io::ErrorKind::WouldBlock), "Try again"),
        ] {
            assert_eq!(open_error(error), USB_IN_USE_DETAIL);
        }
        assert!(USB_IN_USE_DETAIL.contains("Quit DroidFun App"));
        assert_ne!(open_error(serialport::Error::new(serialport::ErrorKind::Io(std::io::ErrorKind::NotFound), "No such file")), USB_IN_USE_DETAIL);
    }

    #[cfg(unix)]
    #[test]
    fn native_exclusive_serial_open_reports_the_other_app() {
        // Native pseudo terminals exercise locking without a physical droid.
        let (_master, mut owner) = serialport::TTYPort::pair().unwrap();
        owner.set_exclusive(true).unwrap();
        let candidate = RobotPort {path: owner.name().unwrap(), serial: "TEST".into(), product: "Test".into(), vid: WAVESHARE_VID, pid: WAVESHARE_PID};
        assert_eq!(Link::connect(&candidate).err().unwrap(), USB_IN_USE_DETAIL);
    }

    #[test]
    fn newly_started_droid_can_answer_first_hello_after_half_a_second() {
        struct StartingDroid { response: std::io::Cursor<Vec<u8>>, starting: bool, writes: usize }
        impl std::io::Read for StartingDroid {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.starting {
                    self.starting = false;
                    std::thread::sleep(Duration::from_millis(600));
                    return Err(std::io::ErrorKind::TimedOut.into());
                }
                std::io::Read::read(&mut self.response, buffer)
            }
        }
        impl std::io::Write for StartingDroid {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> { self.writes += 1; Ok(bytes.len()) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let hello = Packet::new(protocol::HELLO, 101, 1, vec![]);
        let ack = Packet::new(protocol::HELLO_ACK, 101, 1, b"{}".to_vec());
        let mut droid = StartingDroid {response: std::io::Cursor::new(ack.encode().unwrap()), starting: true, writes: 0};
        assert_eq!(exchange(&mut droid, &mut Decoder::new(), &hello, protocol::HELLO_ACK).unwrap(), ack);
        assert_eq!(droid.writes, 1, "startup should not cause a second HELLO or a serial reopen");
    }

    use super::*;
    #[test]
    fn factory_identity_requires_known_model_profile_and_nonzero_generation() {
        let good = serde_json::json!({"model":"meep","hardwareProfile":protocol::IDENTITY_HARDWARE_PROFILE,"generation":2});
        assert_eq!(validated_identity(Some(&good)).unwrap().model, "meep");
        assert!(validated_identity(None).is_none());
        for (key, value) in [
            ("model", serde_json::json!("unknown")),
            ("hardwareProfile", serde_json::json!("other-pins")),
            ("generation", serde_json::json!(0)),
            ("generation", serde_json::json!(4294967296u64)),
            ("generation", serde_json::json!("2")),
        ] {
            let mut bad = good.clone();
            bad[key] = value;
            assert!(validated_identity(Some(&bad)).is_none());
        }
        assert!(validated_identity(Some(&serde_json::json!(null))).is_none());
    }

    #[test]
    fn every_workshop_model_is_accepted_in_handshake_and_provision_readback() {
        for model in protocol::FACTORY_MODELS {
            let identity = serde_json::json!({
                "model": model,
                "hardwareProfile": protocol::IDENTITY_HARDWARE_PROFILE,
                "generation": 7
            });
            let hello: HelloAck = serde_json::from_value(serde_json::json!({
                "firmware": "1.3.2", "boardId": "5303284728CB1C9C",
                "width": 128, "height": 64, "displayHealth": "ready",
                "provisioningSupported": true, "identity": identity.clone()
            })).unwrap();
            let parsed = validated_identity(hello.identity.as_ref())
                .unwrap_or_else(|| panic!("valid {model} handshake identity rejected"));
            assert_eq!(parsed.model, model);
            let ack = serde_json::json!({"provisioned": true, "identity": identity});
            assert_eq!(validate_provision_ack(&ack, model, 7).unwrap().model, model);
            assert!(validate_provision_ack(&ack, model, 8).is_err());
            let other = if model == "claw" { "meep" } else { "claw" };
            assert!(validate_provision_ack(&ack, other, 7).is_err());
        }
    }

    #[test]
    fn legacy_handshake_has_no_factory_identity_or_write_capability() {
        let ack: HelloAck = serde_json::from_value(serde_json::json!({
            "firmware":"1.2.2","boardId":"0123456789ABCDEF","width":128,"height":64,"displayHealth":"ready"
        })).unwrap();
        assert!(ack.identity.is_none());
        assert!(!ack.provisioning_supported);
        assert!(ack.standalone_content_crc.is_none());
    }

    #[test]
    fn provision_ack_must_match_requested_model_and_generation() {
        let ack = serde_json::json!({"provisioned":true,"identity":{"model":"boo","hardwareProfile":protocol::IDENTITY_HARDWARE_PROFILE,"generation":7}});
        assert!(validate_provision_ack(&ack, "boo", 7).is_ok());
        assert!(validate_provision_ack(&ack, "meep", 7).is_err());
        assert!(validate_provision_ack(&ack, "boo", 6).is_err());
        let mut rejected = ack;
        rejected["provisioned"] = serde_json::json!(false);
        assert!(validate_provision_ack(&rejected, "boo", 7).is_err());
    }
    #[test]
    fn save_ack_requires_matching_persistent_playlist() {
        assert!(validate_saved_ack(
            &serde_json::json!({"saved":true,"count":2,"playlistCrc":42}),
            42,
            2,
            99
        )
        .is_ok());
        for ack in [
            serde_json::json!({"saved":true,"count":2}),
            serde_json::json!({"saved":false,"count":2,"playlistCrc":42}),
            serde_json::json!({"saved":true,"count":1,"playlistCrc":42}),
            serde_json::json!({"saved":true,"count":2,"playlistCrc":43}),
        ] {
            assert!(validate_saved_ack(&ack, 42, 2, 99).is_err());
        }
    }
    #[test]
    fn saved_content_rejects_an_older_different_single_frame_despite_equal_transport_crc() {
        let make = |pixel: u8| {
            protocol::StandalonePlaylist {
                frames: vec![protocol::StandaloneFrame {
                    frame: vec![pixel; protocol::MAX_PAYLOAD],
                    duration_sec: 3,
                }],
                brightness: 85,
                orientation: 0,
            }
            .encode()
            .unwrap()
        };
        let expected = make(0x55);
        let older = make(0xaa);
        let transport_crc = protocol::crc32(&expected);
        assert_eq!(transport_crc, protocol::crc32(&older));
        let expected_crc = protocol::crc32(&expected[..expected.len() - 4]);
        let old_crc = protocol::crc32(&older[..older.len() - 4]);
        assert_eq!(expected_crc, 0x3e20575b); // Shared C++ fixture.
        assert_ne!(expected_crc, old_crc);
        let mut ack = serde_json::json!({"saved":true,"count":1,"playlistCrc":transport_crc,"standaloneContentCrc":expected_crc});
        assert_eq!(
            validate_saved_ack(&ack, transport_crc, 1, expected_crc).unwrap(),
            Some(expected_crc)
        );
        for wrong in [
            serde_json::json!(old_crc),
            serde_json::json!(null),
            serde_json::json!(4294967296u64),
            serde_json::json!("1042306907"),
        ] {
            ack["standaloneContentCrc"] = wrong;
            assert!(validate_saved_ack(&ack, transport_crc, 1, expected_crc).is_err());
        }
        ack.as_object_mut().unwrap().remove("standaloneContentCrc");
        assert_eq!(
            validate_saved_ack(&ack, transport_crc, 1, expected_crc).unwrap(),
            None
        );
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
            make("/dev/tty.usbmodem1101", "FIRST"),
            make("/dev/cu.usbmodem1101", "first"),
            make("/dev/cu.usbmodem1201", "second"),
        ]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].path, "/dev/cu.usbmodem1101");
        assert_eq!(result[1].serial, "second");
    }
}
