//! DroidFun App version 1 wire framing. No serial or Tauri dependency lives here.

pub const VERSION: u8 = 1;
pub const MAX_PAYLOAD: usize = 1024;
pub const HEADER_BYTES: usize = 16;
pub const MAX_DECODED: usize = HEADER_BYTES + MAX_PAYLOAD + 4;
pub const MAX_ENCODED: usize = 1100;

pub const HELLO: u8 = 0x01;
pub const HELLO_ACK: u8 = 0x02;
pub const FRAME: u8 = 0x10;
pub const DISPLAY: u8 = 0x11;
#[allow(dead_code)] // Part of the supported firmware protocol; normal face output uses FRAME.
pub const FALLBACK: u8 = 0x12;
pub const PING: u8 = 0x20;
pub const ACK: u8 = 0x21;
pub const ERROR: u8 = 0x22;
pub const SAVED_BEGIN: u8 = 0x30;
pub const SAVED_CHUNK: u8 = 0x31;
pub const SAVED_COMMIT: u8 = 0x32;
pub const MAX_SAVED_FRAMES: usize = 30;

#[derive(Clone, Debug)]
pub struct StandaloneFrame {
    pub frame: Vec<u8>,
    pub duration_sec: u16,
}

#[derive(Clone, Debug)]
pub struct StandalonePlaylist {
    pub frames: Vec<StandaloneFrame>,
    pub brightness: u8,
    pub orientation: u16,
}

impl StandalonePlaylist {
    /// DFP1 is also the on-flash format. Generation zero is reserved for uploads;
    /// the firmware assigns a generation only after validating a complete upload.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        if self.frames.is_empty() || self.frames.len() > MAX_SAVED_FRAMES {
            return Err("Choose between 1 and 30 text or face screens to save".into());
        }
        if self.brightness > 100 || ![0, 180].contains(&self.orientation) {
            return Err("Invalid saved display settings".into());
        }
        let mut bytes = b"DFP1".to_vec();
        bytes.extend_from_slice(&[1, self.frames.len() as u8, self.brightness, 0]);
        bytes.extend_from_slice(&self.orientation.to_le_bytes());
        bytes.extend_from_slice(&[0; 6]);
        for frame in &self.frames {
            if frame.frame.len() != MAX_PAYLOAD || !(3..=120).contains(&frame.duration_sec) {
                return Err("Saved screens must be 128 × 64 pixels and last 3–120 seconds".into());
            }
            bytes.extend_from_slice(&frame.duration_sec.to_le_bytes());
            bytes.extend_from_slice(&frame.frame);
        }
        bytes.extend_from_slice(&crc32(&bytes).to_le_bytes());
        Ok(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub kind: u8,
    pub session: u32,
    pub sequence: u32,
    pub flags: u16,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(kind: u8, session: u32, sequence: u32, payload: Vec<u8>) -> Self {
        Self {
            kind,
            session,
            sequence,
            flags: 0,
            payload,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        if self.payload.len() > MAX_PAYLOAD {
            return Err("Payload exceeds 1024 bytes".into());
        }
        if self.flags != 0 {
            return Err("Unsupported packet flags".into());
        }
        let mut decoded = Vec::with_capacity(HEADER_BYTES + self.payload.len() + 4);
        decoded.extend_from_slice(b"DB");
        decoded.push(VERSION);
        decoded.push(self.kind);
        decoded.extend_from_slice(&self.session.to_le_bytes());
        decoded.extend_from_slice(&self.sequence.to_le_bytes());
        decoded.extend_from_slice(&(self.payload.len() as u16).to_le_bytes());
        decoded.extend_from_slice(&self.flags.to_le_bytes());
        decoded.extend_from_slice(&self.payload);
        decoded.extend_from_slice(&crc32(&decoded).to_le_bytes());
        let mut encoded = cobs_encode(&decoded);
        encoded.push(0);
        if encoded.len() > MAX_ENCODED {
            return Err("Encoded packet is too large".into());
        }
        Ok(encoded)
    }

    pub fn decode(encoded_without_delimiter: &[u8]) -> Result<Self, String> {
        if encoded_without_delimiter.is_empty() || encoded_without_delimiter.len() > MAX_ENCODED - 1
        {
            return Err("Invalid encoded packet length".into());
        }
        let data = cobs_decode(encoded_without_delimiter)?;
        if data.len() < HEADER_BYTES + 4 || data.len() > MAX_DECODED {
            return Err("Invalid decoded packet length".into());
        }
        if &data[0..2] != b"DB" {
            return Err("Invalid packet magic".into());
        }
        if data[2] != VERSION {
            return Err("Unsupported protocol version".into());
        }
        let length = u16::from_le_bytes([data[12], data[13]]) as usize;
        if length > MAX_PAYLOAD || data.len() != HEADER_BYTES + length + 4 {
            return Err("Payload length mismatch".into());
        }
        let flags = u16::from_le_bytes([data[14], data[15]]);
        if flags != 0 {
            return Err("Unknown mandatory flags".into());
        }
        let crc_offset = HEADER_BYTES + length;
        let supplied = u32::from_le_bytes(data[crc_offset..][..4].try_into().unwrap());
        if supplied != crc32(&data[..crc_offset]) {
            return Err("CRC mismatch".into());
        }
        Ok(Self {
            kind: data[3],
            session: u32::from_le_bytes(data[4..8].try_into().unwrap()),
            sequence: u32::from_le_bytes(data[8..12].try_into().unwrap()),
            flags,
            payload: data[HEADER_BYTES..crc_offset].to_vec(),
        })
    }
}

/// CRC-32/ISO-HDLC, including the standard reflected 0xEDB88320 polynomial.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (if crc & 1 != 0 { 0xEDB8_8320 } else { 0 });
        }
    }
    !crc
}

fn cobs_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 254 + 1);
    let mut code_index = 0;
    out.push(0);
    let mut code = 1u8;
    for byte in data {
        if *byte == 0 {
            out[code_index] = code;
            code_index = out.len();
            out.push(0);
            code = 1;
        } else {
            out.push(*byte);
            code += 1;
            if code == 0xFF {
                out[code_index] = code;
                code_index = out.len();
                out.push(0);
                code = 1;
            }
        }
    }
    out[code_index] = code;
    out
}

fn cobs_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let code = data[i] as usize;
        if code == 0 {
            return Err("Zero inside COBS packet".into());
        }
        i += 1;
        let end = i + code - 1;
        if end > data.len() {
            return Err("Truncated COBS packet".into());
        }
        out.extend_from_slice(&data[i..end]);
        i = end;
        if code != 0xFF && i < data.len() {
            out.push(0);
        }
    }
    Ok(out)
}

/// A malformed/oversized packet cannot poison the next valid packet.
pub struct Decoder {
    bytes: Vec<u8>,
    discard: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            discard: false,
        }
    }
    pub fn reset(&mut self) {
        self.bytes.clear();
        self.discard = false;
    }
    pub fn push(&mut self, byte: u8) -> Option<Result<Packet, String>> {
        if byte == 0 {
            if self.discard || self.bytes.is_empty() {
                self.reset();
                return None;
            }
            let result = Packet::decode(&self.bytes);
            self.reset();
            return Some(result);
        }
        if self.discard {
            return None;
        }
        if self.bytes.len() >= MAX_ENCODED - 1 {
            self.bytes.clear();
            self.discard = true;
            return None;
        }
        self.bytes.push(byte);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_playlist_has_firmware_compatible_header_records_and_bounds() {
        let frame = StandaloneFrame {
            frame: vec![0x55; 1024],
            duration_sec: 3,
        };
        let mut playlist = StandalonePlaylist {
            frames: vec![frame.clone()],
            brightness: 85,
            orientation: 0,
        };
        let encoded = playlist.encode().unwrap();
        assert_eq!(encoded.len(), 1046);
        assert_eq!(&encoded[..16], b"DFP1\x01\x01\x55\0\0\0\0\0\0\0\0\0");
        assert_eq!(&encoded[16..18], &[3, 0]);
        assert_eq!(&encoded[18..1042], &[0x55; 1024]);
        // Cross-language fixture also checked by firmware/tests/standalone_test.cpp.
        assert_eq!(crc32(&encoded[..1042]), 0x3e20_575b);
        assert_eq!(
            u32::from_le_bytes(encoded[1042..].try_into().unwrap()),
            crc32(&encoded[..1042])
        );
        playlist.frames = vec![frame.clone(); 30];
        assert_eq!(playlist.encode().unwrap().len(), 30800);
        playlist.frames.push(frame.clone());
        assert!(playlist.encode().is_err());
        playlist.frames.clear();
        assert!(playlist.encode().is_err());
        playlist.frames.push(frame);
        for duration in [0, 2, 121, u16::MAX] {
            playlist.frames[0].duration_sec = duration;
            assert!(playlist.encode().is_err());
        }
        playlist.frames[0].duration_sec = 120;
        playlist.orientation = 180;
        assert!(playlist.encode().is_ok());
        playlist.orientation = 90;
        assert!(playlist.encode().is_err());
        playlist.orientation = 0;
        playlist.brightness = 101;
        assert!(playlist.encode().is_err());
        playlist.brightness = 100;
        playlist.frames[0].frame.pop();
        assert!(playlist.encode().is_err());
    }

    #[test]
    fn standard_crc_and_binary_roundtrip() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        for size in [0, 1, 254, 255, 1024] {
            let payload: Vec<u8> = (0..size).map(|n| (n % 17) as u8).collect();
            let packet = Packet::new(FRAME, 0x1234, 42, payload);
            let wire = packet.encode().unwrap();
            assert_eq!(*wire.last().unwrap(), 0);
            assert_eq!(Packet::decode(&wire[..wire.len() - 1]).unwrap(), packet);
        }
    }

    #[test]
    fn rejects_corruption_and_recovers_after_oversize() {
        let packet = Packet::new(PING, 7, 1, vec![]);
        let mut wire = packet.encode().unwrap();
        wire[5] ^= 1;
        assert!(Packet::decode(&wire[..wire.len() - 1]).is_err());
        let mut decoder = Decoder::new();
        for _ in 0..MAX_ENCODED + 5 {
            decoder.push(0x55);
        }
        assert!(decoder.push(0).is_none());
        let good = packet.encode().unwrap();
        let found = good
            .into_iter()
            .filter_map(|b| decoder.push(b))
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(found, packet);
    }

    #[test]
    fn rejects_flags_and_bad_length() {
        let mut packet = Packet::new(PING, 7, 1, vec![]);
        packet.flags = 1;
        assert!(packet.encode().is_err());
        assert!(Packet::new(FRAME, 1, 1, vec![0; 1025]).encode().is_err());
    }
}
