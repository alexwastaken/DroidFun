//! USB setup for boards already exposing the RP2040 ROM boot drive.
//! Discovery never opens serial ports or changes a device.
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const FIRMWARE: &[u8] =
    include_bytes!("../../firmware/releases/droidfun-rp2040-zero-gp4-gp5-v1.2.4.uf2");
const DISPLAY_TEST_FIRMWARE: &[u8] =
    include_bytes!("../../firmware/releases/droidfun-display-test-v1.uf2");
static INSTALLING: AtomicBool = AtomicBool::new(false);
static SCANNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupDrive {
    pub path: String,
    pub name: String,
}

fn valid_info(info: &str) -> bool {
    info.lines().any(|line| line.trim() == "Board-ID: RPI-RP2")
        && info
            .lines()
            .any(|line| line.trim().starts_with("UF2 Bootloader v"))
}

fn volume_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![PathBuf::from("/Volumes")]
    }
    #[cfg(target_os = "linux")]
    {
        let mut roots = vec![
            PathBuf::from("/media"),
            PathBuf::from("/run/media"),
            PathBuf::from("/mnt"),
        ];
        for parent in ["/media", "/run/media"] {
            if let Ok(entries) = fs::read_dir(parent) {
                roots.extend(
                    entries
                        .flatten()
                        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                        .map(|e| e.path()),
                );
            }
        }
        roots
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

fn is_boot_drive(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return false;
    }
    // A folder named RPI-RP2 on the host is not a mounted robot.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let Some(parent) = path.parent() else {
            return false;
        };
        if fs::metadata(parent).is_ok_and(|p| p.dev() == meta.dev()) {
            return false;
        }
    }
    let info = path.join("INFO_UF2.TXT");
    let Ok(meta) = fs::symlink_metadata(&info) else {
        return false;
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 4096 {
        return false;
    }
    fs::read_to_string(info).is_ok_and(|s| valid_info(&s))
}

fn allowed_mount_path(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        return s.len() == 3 && s.as_bytes()[0].is_ascii_alphabetic() && &s[1..] == ":\\";
    }
    #[cfg(not(windows))]
    {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return false;
        };
        let boot_name = name == "RPI-RP2"
            || name
                .strip_prefix("RPI-RP2 ")
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
        boot_name
            && path
                .parent()
                .is_some_and(|p| volume_roots().contains(&p.to_path_buf()))
    }
}

pub fn drives() -> Result<Vec<SetupDrive>, String> {
    // Direct USB avoids the mounted FAT drive, which can block on macOS even
    // while the bootloader's USB interface remains responsive.
    if crate::picoboot::supported() {
        return crate::picoboot::drives();
    }
    if SCANNING.swap(true, Ordering::SeqCst) {
        return Err("USB scan is still waiting for the drive. Reconnect the USB cable if it does not finish.".into());
    }
    struct ScanGuard;
    impl Drop for ScanGuard {
        fn drop(&mut self) {
            SCANNING.store(false, Ordering::SeqCst);
        }
    }
    let _guard = ScanGuard;
    let mut candidates = Vec::new();
    for root in volume_roots() {
        if let Ok(entries) = fs::read_dir(root) {
            candidates.extend(entries.flatten().map(|e| e.path()));
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
        use windows_sys::Win32::System::WindowsProgramming::DRIVE_REMOVABLE;
        for letter in b'A'..=b'Z' {
            let root = format!("{}:\\", letter as char);
            let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
            if unsafe { GetDriveTypeW(wide.as_ptr()) } == DRIVE_REMOVABLE {
                candidates.push(PathBuf::from(root));
            }
        }
    }
    let mut found: Vec<_> = candidates
        .into_iter()
        // Never touch unrelated disks or stale app installer volumes.
        .filter(|p| allowed_mount_path(p) && is_boot_drive(p))
        .map(|p| SetupDrive {
            name: p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("RP2040 USB drive")
                .to_string(),
            path: p.to_string_lossy().into_owned(),
        })
        .collect();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found.dedup_by(|a, b| a.path == b.path);
    Ok(found)
}

struct InstallGuard;
impl Drop for InstallGuard {
    fn drop(&mut self) {
        INSTALLING.store(false, Ordering::SeqCst);
    }
}

/// Return the newly enumerated serial identity. Connection/HELLO is checked by
/// the normal link worker after selection, never inferred from a successful copy.
pub fn install(
    path: &str,
    expected_board_id: Option<&str>,
    cancel: &AtomicBool,
    report: impl Fn(&str),
) -> Result<String, String> {
    install_image(path, expected_board_id, FIRMWARE, cancel, report)
}

pub fn install_display_test(
    path: &str,
    expected_board_id: Option<&str>,
    cancel: &AtomicBool,
    report: impl Fn(&str),
) -> Result<String, String> {
    if !path.starts_with("picoboot:") || !crate::picoboot::supported() {
        return Err(
            "The standalone display test requires direct USB setup on a supported Mac.".into(),
        );
    }
    install_image(
        path,
        expected_board_id,
        DISPLAY_TEST_FIRMWARE,
        cancel,
        report,
    )
}

fn install_image(
    path: &str,
    expected_board_id: Option<&str>,
    firmware: &[u8],
    cancel: &AtomicBool,
    report: impl Fn(&str),
) -> Result<String, String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("A robot installation is already running.".into());
    }
    let _guard = InstallGuard;
    if path.starts_with("picoboot:") {
        let before = crate::connection::robot_ports()?;
        let serial = crate::picoboot::install(path, expected_board_id, firmware, cancel, &report)?;
        report("Software verified. Waiting for your bot to restart…");
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if cancel.load(Ordering::SeqCst) {
                return Err("USB setup was interrupted. Scan USB to check your bot.".into());
            }
            if let Some(serial) = returning_board(
                &before,
                &crate::connection::robot_ports()?,
                serial.as_deref(),
            )? {
                return Ok(serial);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        return Err("Software was verified, but your bot has not reconnected yet. Scan USB to check its connection.".into());
    }
    report("Checking your bot’s USB setup drive…");
    if !allowed_mount_path(Path::new(path)) || !is_boot_drive(Path::new(path)) {
        return Err(
            "That robot setup drive is no longer connected. Reconnect USB and scan again.".into(),
        );
    }
    let before = crate::connection::robot_ports()?;
    if cancel.load(Ordering::SeqCst) {
        return Err("USB setup canceled. Reconnect your bot and try again.".into());
    }
    report("Preparing the USB drive…");
    let destination = Path::new(path).join("DroidFun-setup-v1.2.4.uf2");
    // Never follow or overwrite a pre-existing file on a removable drive.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| "Could not start robot setup. Reconnect the USB cable and scan again.")?;
    if cancel.load(Ordering::SeqCst) {
        return Err("USB setup canceled. Reconnect your bot and try again.".into());
    }
    report("Sending your bot’s software… Keep the USB cable connected.");
    file.write_all(firmware).map_err(|_| "The USB transfer was interrupted. Reconnect the cable and scan again; setup is not confirmed.")?;
    let flushed = file.sync_all();
    drop(file);
    if cancel.load(Ordering::SeqCst) {
        return Err("USB setup was interrupted. Scan USB to check your bot.".into());
    }
    report("Software sent. Waiting for your bot to restart…");
    // ROM may eject the drive before the final flush completes. Require the
    // new serial device either way; a missing board is never reported as ready.
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if cancel.load(Ordering::SeqCst) {
            return Err("USB setup was interrupted. Scan USB to check your bot.".into());
        }
        if let Ok(ports) = crate::connection::robot_ports() {
            let new: Vec<_> = ports
                .into_iter()
                .filter(|p| !before.iter().any(|old| old.serial == p.serial))
                .collect();
            if new.len() == 1 {
                if expected_board_id
                    .is_some_and(|expected| !expected.eq_ignore_ascii_case(&new[0].serial))
                {
                    return Err(
                        "A different bot reconnected. Scan USB and choose the intended bot.".into(),
                    );
                }
                return Ok(new[0].serial.clone());
            }
            if new.len() > 1 {
                return Err("More than one robot restarted. Scan USB and choose the intended robot to connect.".into());
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(if flushed.is_err() { "USB setup could not be confirmed. Reconnect the cable and scan again." } else {
        "Software was sent, but the robot has not restarted as a controller. Reconnect USB and scan again."
    }.into())
}

fn returning_board(
    before: &[crate::connection::RobotPort],
    current: &[crate::connection::RobotPort],
    expected: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(expected) = expected {
        return Ok(current
            .iter()
            .find(|port| port.serial.eq_ignore_ascii_case(expected))
            .map(|port| port.serial.clone()));
    }
    let new: Vec<_> = current
        .iter()
        .filter(|port| {
            !before
                .iter()
                .any(|old| old.serial.eq_ignore_ascii_case(&port.serial))
        })
        .collect();
    match new.as_slice() {
        [] => Ok(None),
        [port] if port.serial.len() == 16 && port.serial.bytes().all(|b| b.is_ascii_hexdigit()) => {
            Ok(Some(port.serial.clone()))
        }
        [_] => Err("Software was verified, but the returning bot’s identity could not be checked. Scan USB again.".into()),
        _ => Err("More than one robot restarted. Scan USB and choose the intended robot to connect.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_install_requires_one_new_valid_runtime_identity() {
        let port = |serial: &str| crate::connection::RobotPort {
            path: "/dev/test".into(),
            serial: serial.into(),
            product: "RP2040 Zero".into(),
            vid: 0x2e8a,
            pid: 3,
        };
        let old = port("1111111111111111");
        let new = port("abcdef0123456789");
        assert_eq!(
            returning_board(&[old.clone()], &[old.clone()], None).unwrap(),
            None
        );
        assert_eq!(
            returning_board(&[old.clone()], &[old.clone(), new.clone()], None).unwrap(),
            Some(new.serial.clone())
        );
        assert!(returning_board(&[], &[old.clone(), new.clone()], None).is_err());
        assert!(returning_board(&[], &[port("unknown")], None).is_err());
        assert_eq!(
            returning_board(&[], &[old.clone()], Some(&new.serial)).unwrap(),
            None
        );
        assert_eq!(
            returning_board(&[], &[old, new.clone()], Some("ABCDEF0123456789")).unwrap(),
            Some(new.serial)
        );
    }
    #[test]
    fn accepts_only_rp2040_bootloader_identity() {
        assert!(valid_info(
            "UF2 Bootloader v3.0\r\nModel: Raspberry Pi RP2\r\nBoard-ID: RPI-RP2\r\n"
        ));
        for value in [
            "Board-ID: RPI-RP2",
            "UF2 Bootloader v3.0\nBoard-ID: RPI-RP2350",
            "UF2 Bootloader v3.0\nBoard-ID: RPI-RP2-other",
        ] {
            assert!(!valid_info(value));
        }
    }
    #[test]
    fn bundled_firmware_matches_verified_release() {
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!("{:x}", Sha256::digest(FIRMWARE)),
            "b250689c741fc35984015724e90d0f248ac8ae37e88263264eee4f006f669923"
        );
    }
    #[test]
    fn bundled_display_test_matches_verified_build() {
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!("{:x}", Sha256::digest(DISPLAY_TEST_FIRMWARE)),
            "a118cb30e4f732954b01a42b31afc1d4615b38620014d75d70ca2db97de4b20a"
        );
    }
    #[test]
    fn display_test_refuses_mounted_drive_installation() {
        assert!(
            install_display_test("/Volumes/RPI-RP2", None, &AtomicBool::new(false), |_| {})
                .is_err()
        );
    }
    #[test]
    fn refuses_a_host_folder_or_arbitrary_install_path() {
        assert!(!is_boot_drive(Path::new(env!("CARGO_MANIFEST_DIR"))));
        assert!(install(
            "/tmp/not-a-droidfun-robot",
            None,
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err());
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn only_checks_boot_volume_names_without_touching_other_disks() {
        assert!(allowed_mount_path(Path::new("/Volumes/RPI-RP2")));
        assert!(allowed_mount_path(Path::new("/Volumes/RPI-RP2 1")));
        for p in [
            "/Volumes/DroidFun App",
            "/Volumes/RPI-RP2-fake",
            "/tmp/RPI-RP2",
            "/Volumes/RPI-RP2/nested",
            "/Volumes/RPI-RP2/../RPI-RP2",
        ] {
            assert!(!allowed_mount_path(Path::new(p)));
        }
    }
}
