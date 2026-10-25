//! RP2040 ROM vendor USB transport. Discovery reads the IORegistry, never the
//! mounted ROM filesystem. Only an explicit install opens the USB interface.
use crate::firmware_setup::SetupDrive;
use std::sync::atomic::AtomicBool;

pub fn supported() -> bool {
    #[cfg(target_os = "macos")]
    return native::supported();
    #[cfg(not(target_os = "macos"))]
    false
}

pub fn drives() -> Result<Vec<SetupDrive>, String> {
    #[cfg(target_os = "macos")]
    return native::drives();
    #[cfg(not(target_os = "macos"))]
    Ok(Vec::new())
}

pub fn install(
    path: &str,
    expected_board_id: Option<&str>,
    firmware: &[u8],
    cancel: &AtomicBool,
    report: impl Fn(&str),
) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    return native::install(path, expected_board_id, firmware, cancel, report);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path, expected_board_id, firmware, cancel, report);
        Err("Direct USB setup requires macOS 13 or newer.".into())
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use plist::Value;
    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
        path::{Path, PathBuf},
        process::{Command, ExitStatus, Stdio},
        sync::{
            atomic::{AtomicU64, Ordering},
            OnceLock,
        },
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    const PREFIX: &str = "picoboot:";
    const OUTPUT_LIMIT: u64 = 4 * 1024 * 1024;
    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    static OS_SUPPORTED: OnceLock<bool> = OnceLock::new();

    struct TempArea(PathBuf);
    impl TempArea {
        fn new() -> Result<Self, String> {
            for _ in 0..8 {
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let id = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("droidfun-usb-{}-{stamp}-{id}", std::process::id()));
                match fs::DirBuilder::new().mode(0o700).create(&path) {
                    Ok(()) => return Ok(Self(path)),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(_) => return Err("Could not prepare private USB setup files.".into()),
                }
            }
            Err("Could not prepare private USB setup files.".into())
        }

        fn file(&self, name: &str) -> Result<File, String> {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.0.join(name))
                .map_err(|_| "Could not prepare private USB setup files.".into())
        }
    }
    impl Drop for TempArea {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Output {
        status: ExitStatus,
        bytes: Vec<u8>,
    }

    // Output goes to one private local file, so a full stdout/stderr pipe cannot
    // deadlock a child. Limit output, kill and reap the exact child on every
    // cancellation/timeout, then remove all temporary files through the guard.
    fn run(mut command: Command, budget: Duration, cancel: &AtomicBool) -> Result<Output, String> {
        if cancel.load(Ordering::SeqCst) {
            return Err("USB setup canceled.".into());
        }
        let temp = TempArea::new()?;
        let output_path = temp.0.join("output");
        let output = temp.file("output")?;
        let err = output
            .try_clone()
            .map_err(|_| "Could not prepare USB setup output.")?;
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::from(output))
            .stderr(Stdio::from(err))
            .env("LC_ALL", "C")
            .spawn()
            .map_err(|e| format!("Could not start USB setup helper: {e}"))?;
        let deadline = Instant::now() + budget;
        let status = loop {
            let failure = if cancel.load(Ordering::SeqCst) {
                Some("USB setup canceled.")
            } else if Instant::now() >= deadline {
                Some("USB setup helper timed out. Reconnect your bot before trying again.")
            } else if fs::metadata(&output_path).is_ok_and(|m| m.len() > OUTPUT_LIMIT) {
                Some("USB setup helper returned too much output.")
            } else {
                None
            };
            if let Some(error) = failure {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Could not check USB setup helper.".into());
                }
            }
        };
        let mut bytes = Vec::new();
        File::open(output_path)
            .and_then(|f| f.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes))
            .map_err(|_| "Could not read USB setup result.")?;
        if bytes.len() as u64 > OUTPUT_LIMIT {
            return Err("USB setup helper returned too much output.".into());
        }
        Ok(Output { status, bytes })
    }

    fn usable_helper(path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok_and(|m| {
            m.is_file() && !m.file_type().is_symlink() && m.permissions().mode() & 0o111 != 0
        }) && path.parent().is_some_and(|p| {
            fs::symlink_metadata(p.join("libusb-1.0.0.dylib"))
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
        })
    }

    fn helper() -> Result<PathBuf, String> {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(contents) = exe.parent().and_then(Path::parent) {
                let path = contents.join("Resources/picotool-macos-universal/picotool");
                if usable_helper(&path) {
                    return Ok(path);
                }
            }
        }
        #[cfg(debug_assertions)]
        {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("resources/picotool-macos-universal/picotool");
            if usable_helper(&path) {
                return Ok(path);
            }
        }
        Err("The direct USB setup helper is missing. Reinstall the current DroidFun App.".into())
    }

    fn os_supported() -> bool {
        *OS_SUPPORTED.get_or_init(check_os_supported)
    }

    fn check_os_supported() -> bool {
        let mut command = Command::new("/usr/bin/sw_vers");
        command.arg("-productVersion");
        run(command, Duration::from_secs(2), &AtomicBool::new(false)).is_ok_and(|out| {
            out.status.success()
                && String::from_utf8_lossy(&out.bytes)
                    .trim()
                    .split('.')
                    .next()
                    .and_then(|s| s.parse::<u32>().ok())
                    .is_some_and(|major| major >= 13)
        })
    }

    pub fn supported() -> bool {
        helper().is_ok() && os_supported()
    }

    fn target(path: &str) -> Result<&str, String> {
        let serial = path
            .strip_prefix(PREFIX)
            .ok_or("Invalid USB setup target.")?;
        if serial.is_empty()
            || serial.len() > 64
            || !serial.as_bytes()[0].is_ascii_alphanumeric()
            || !serial
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Invalid USB setup target.".into());
        }
        Ok(serial)
    }

    fn serials_from_plist(bytes: &[u8]) -> Result<Vec<String>, String> {
        fn rom_product(d: &plist::Dictionary) -> bool {
            let products: Vec<_> = ["USBProductName", "kUSBProductString", "USB Product Name"]
                .iter()
                .filter_map(|key| d.get(*key))
                .collect();
            !products.is_empty() && products.iter().all(|v| v.as_string() == Some("RP2 Boot"))
        }

        fn visit(value: &Value, found: &mut Vec<String>) {
            match value {
                Value::Array(values) => values.iter().for_each(|v| visit(v, found)),
                Value::Dictionary(d) => {
                    let number = |key| d.get(key).and_then(Value::as_unsigned_integer);
                    // Arduino runtime firmware can reuse the ROM's VID/PID.
                    // Require its exact ROM product descriptor as well; a
                    // running RP2040 Zero must never appear as an installer.
                    if number("idVendor") == Some(0x2e8a)
                        && number("idProduct") == Some(3)
                        && number("bDeviceClass") == Some(0)
                        && rom_product(d)
                    {
                        if let Some(serial) = d
                            .get("USB Serial Number")
                            .or_else(|| d.get("kUSBSerialNumberString"))
                            .and_then(Value::as_string)
                        {
                            if target(&format!("{PREFIX}{serial}")).is_ok() {
                                found.push(serial.to_owned());
                            }
                        }
                    }
                    if let Some(children) = d.get("IORegistryEntryChildren") {
                        visit(children, found);
                    }
                }
                _ => {}
            }
        }
        let value = Value::from_reader(std::io::Cursor::new(bytes))
            .map_err(|_| "Could not read the Mac USB device list.")?;
        let mut found = Vec::new();
        visit(&value, &mut found);
        found.sort();
        if found.windows(2).any(|w| w[0] == w[1]) {
            return Err("More than one setup device has the same USB identity. Connect only the intended bot.".into());
        }
        Ok(found)
    }

    fn discover(cancel: &AtomicBool) -> Result<Vec<String>, String> {
        let mut command = Command::new("/usr/sbin/ioreg");
        command.args(["-a", "-p", "IOUSB", "-l", "-w", "0"]);
        let out = run(command, Duration::from_secs(3), cancel)?;
        if !out.status.success() {
            return Err("Could not read the Mac USB device list.".into());
        }
        serials_from_plist(&out.bytes)
    }

    pub fn drives() -> Result<Vec<SetupDrive>, String> {
        helper()?;
        if !os_supported() {
            return Err("Direct USB setup requires macOS 13 or newer.".into());
        }
        Ok(discover(&AtomicBool::new(false))?
            .into_iter()
            .map(|serial| SetupDrive {
                name: "RP2040 bot · Direct USB setup".into(),
                path: format!("{PREFIX}{serial}"),
            })
            .collect())
    }

    fn board_id(value: &str) -> Result<String, String> {
        let value = value.trim().strip_prefix("0x").unwrap_or(value.trim());
        if value.len() != 16 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("The USB helper did not identify this board's flash chip.".into());
        }
        Ok(value.to_ascii_uppercase())
    }

    fn parse_identity(output: &str, expected: Option<&str>) -> Result<Option<String>, String> {
        let mut types = Vec::new();
        let mut ids = Vec::new();
        let mut device_sections = 0;
        for line in output.lines() {
            if line.trim() == "Device Information" {
                device_sections += 1;
            }
            if let Some((key, value)) = line.split_once(':') {
                match key.trim().to_ascii_lowercase().as_str() {
                    "type" => types.push(value.trim()),
                    "flash id" => ids.push(board_id(value)?),
                    _ => {}
                }
            }
        }
        if device_sections != 1 || types != ["RP2040"] || ids.len() > 1 {
            return Err("The selected USB device was not identified as one RP2040 bot.".into());
        }
        let id = ids.pop();
        if let Some(expected) = expected {
            if id.as_deref() != Some(board_id(expected)?.as_str()) {
                return Err("The board in setup mode does not match your selected bot. Reconnect the intended bot.".into());
            }
        }
        // picotool 2.0.0 gates its flash-ID output on guess_flash_size() > 0.
        // Erased flash can produce only the RP2040 type. None is allowed solely
        // for first setup, guarded by one-ROM checks before info and load.
        // Known-board updates always require a matching reported identity.
        // https://github.com/raspberrypi/picotool/blob/2.0.0/main.cpp#L3053-L3061
        Ok(id)
    }

    fn preflight_selector(
        selected: &str,
        discovered: &[String],
        expected: Option<&str>,
    ) -> Result<Option<String>, String> {
        if !discovered.iter().any(|s| s == selected) {
            return Err(
                "Your bot is no longer in USB setup mode. Reconnect it and scan again.".into(),
            );
        }
        if let Some(expected) = expected {
            return Ok(Some(board_id(expected)?));
        }
        if discovered.len() != 1 {
            return Err(
                "Connect only the bot you want to set up before installing its software.".into(),
            );
        }
        Ok(None)
    }

    fn helper_command(helper: &Path, flash_id: Option<&str>, action: &str) -> Command {
        let mut command = Command::new(helper);
        command.arg(action);
        // On RP2040 picotool's --ser selects the flash ID, not the shared ROM
        // USB descriptor. Leave VID/PID to its native chip detection: an
        // explicit PID override bypasses the known RP2040 model path in 2.0.0.
        if let Some(id) = flash_id {
            command.args(["--ser", id]);
        }
        command
    }

    fn helper_error(bytes: &[u8]) -> String {
        // Progress bars overwrite their line with carriage returns. Keep the
        // diagnostic tail, where picotool reports the actual failure, instead
        // of filling the UI with the beginning of the loading progress.
        let text = String::from_utf8_lossy(bytes);
        let detail = text
            .split(['\r', '\n'])
            .map(str::trim)
            .filter(|line| {
                !line.is_empty()
                    && !line.starts_with("Loading into ")
                    && !line.starts_with("Verifying ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let message = detail
            .chars()
            .rev()
            .take(1200)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        format!(
            "Direct USB setup could not finish. Reconnect your bot and try again. {}",
            message.trim()
        )
    }

    fn verified(out: &Output) -> bool {
        // Only used for load_command(), which always supplies -v -x.
        // picotool 2.0.0 main.cpp load_guts prints "OK" after readback and calls
        // fail(ERROR_VERIFICATION_FAILED) on any mismatch; main returns that
        // nonzero error code. "Verified OK" belongs to its unrelated OTP path.
        // https://github.com/raspberrypi/picotool/blob/2.0.0/main.cpp#L3819-L3855
        out.status.success()
    }

    fn load_command(helper: &Path, id: Option<&str>, firmware_path: &Path) -> Command {
        let mut command = helper_command(helper, id, "load");
        command.args(["-v", "-x"]).arg(firmware_path);
        command
    }

    pub fn install(
        path: &str,
        expected_board_id: Option<&str>,
        firmware: &[u8],
        cancel: &AtomicBool,
        report: impl Fn(&str),
    ) -> Result<Option<String>, String> {
        let serial = target(path)?;
        let helper = helper()?;
        if !os_supported() {
            return Err("Direct USB setup requires macOS 13 or newer.".into());
        }
        if cancel.load(Ordering::SeqCst) {
            return Err("USB setup canceled.".into());
        }
        if firmware.is_empty() {
            return Err("The bundled robot software is empty.".into());
        }
        report("Checking your bot through direct USB…");
        let selector = preflight_selector(serial, &discover(cancel)?, expected_board_id)?;
        let mut info = helper_command(&helper, selector.as_deref(), "info");
        info.arg("-d");
        let identity = run(info, Duration::from_secs(8), cancel)?;
        if !identity.status.success() {
            return Err(helper_error(&identity.bytes));
        }
        let id = parse_identity(&String::from_utf8_lossy(&identity.bytes), expected_board_id)?;
        let temp = TempArea::new()?;
        let mut file = temp.file("firmware.uf2")?;
        file.write_all(firmware)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Could not prepare the bundled robot software.")?;
        drop(file);
        // Recheck just before the sole write. With a known ID, picotool binds
        // the load to that flash chip. A blank board without an ID requires
        // exactly one ROM again; picotool's default load independently refuses
        // multiple accessible boot devices, including RP2350 devices.
        preflight_selector(serial, &discover(cancel)?, id.as_deref())?;
        report(
            "Installing and verifying your bot’s software through direct USB… Keep USB connected.",
        );
        let load = load_command(&helper, id.as_deref(), &temp.0.join("firmware.uf2"));
        let out = run(load, Duration::from_secs(25), cancel)?;
        if !verified(&out) {
            return Err(helper_error(&out.bytes));
        }
        report("Software verified. Waiting for your bot to restart…");
        Ok(id)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn fixture(devices: &[(u64, u64, &str)]) -> Vec<u8> {
            let children = devices
                .iter()
                .map(|(vid, pid, serial)| {
                    let mut d = plist::Dictionary::new();
                    d.insert("idVendor".into(), Value::Integer((*vid).into()));
                    d.insert("idProduct".into(), Value::Integer((*pid).into()));
                    d.insert("bDeviceClass".into(), Value::Integer(0u64.into()));
                    d.insert("USB Serial Number".into(), Value::String((*serial).into()));
                    d.insert("USBProductName".into(), Value::String("RP2 Boot".into()));
                    Value::Dictionary(d)
                })
                .collect();
            let mut root = plist::Dictionary::new();
            root.insert("IORegistryEntryChildren".into(), Value::Array(children));
            let mut bytes = Vec::new();
            Value::Array(vec![Value::Dictionary(root)])
                .to_writer_xml(&mut bytes)
                .unwrap();
            bytes
        }

        #[test]
        fn discovery_filters_exact_rom_identity_and_preserves_multiple_targets() {
            assert_eq!(
                serials_from_plist(&fixture(&[
                    (0x2e8a, 3, "E0C9125B0D9B"),
                    (0x2e8a, 3, "OTHER123"),
                    (0x2e8a, 10, "RUNTIME"),
                    (1, 3, "UNRELATED")
                ]))
                .unwrap(),
                ["E0C9125B0D9B", "OTHER123"]
            );
            assert!(
                serials_from_plist(&fixture(&[(0x2e8a, 3, "SAME"), (0x2e8a, 3, "SAME")])).is_err()
            );
        }

        #[test]
        fn discovery_rejects_runtime_with_rom_vid_pid_and_requires_rom_product() {
            for (key, product, accepted) in [
                ("USBProductName", "RP2 Boot", true),
                ("kUSBProductString", "RP2 Boot", true),
                ("USB Product Name", "RP2 Boot", true),
                ("USBProductName", "RP2040 Zero", false),
                ("kUSBProductString", "RP2040 Zero", false),
                ("USBProductName", "RP2 Boot extra", false),
                ("USBProductName", "", false),
            ] {
                let mut value = Value::from_reader(std::io::Cursor::new(fixture(&[(
                    0x2e8a,
                    3,
                    "53032847490B8B1C",
                )])))
                .unwrap();
                let device = value.as_array_mut().unwrap()[0]
                    .as_dictionary_mut()
                    .unwrap()
                    .get_mut("IORegistryEntryChildren")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()[0]
                    .as_dictionary_mut()
                    .unwrap();
                device.remove("USBProductName");
                device.insert(key.into(), Value::String(product.into()));
                let mut bytes = Vec::new();
                value.to_writer_xml(&mut bytes).unwrap();
                assert_eq!(
                    !serials_from_plist(&bytes).unwrap().is_empty(),
                    accepted,
                    "{key}: {product}"
                );
            }
            let bytes = fixture(&[(0x2e8a, 3, "E0C9125B0D9B")]);
            let mut value = Value::from_reader(std::io::Cursor::new(bytes)).unwrap();
            let device = value.as_array_mut().unwrap()[0]
                .as_dictionary_mut()
                .unwrap()
                .get_mut("IORegistryEntryChildren")
                .unwrap()
                .as_array_mut()
                .unwrap()[0]
                .as_dictionary_mut()
                .unwrap();
            device.remove("USBProductName");
            let mut bytes = Vec::new();
            value.to_writer_xml(&mut bytes).unwrap();
            assert!(serials_from_plist(&bytes).unwrap().is_empty());

            // Composite application firmware is not the ROM interface even if
            // it copies the ROM's product name and VID/PID.
            let mut value =
                Value::from_reader(std::io::Cursor::new(fixture(&[(0x2e8a, 3, "RUNTIME")])))
                    .unwrap();
            let device = value.as_array_mut().unwrap()[0]
                .as_dictionary_mut()
                .unwrap()
                .get_mut("IORegistryEntryChildren")
                .unwrap()
                .as_array_mut()
                .unwrap()[0]
                .as_dictionary_mut()
                .unwrap();
            device.insert("bDeviceClass".into(), Value::Integer(239u64.into()));
            let mut bytes = Vec::new();
            value.to_writer_xml(&mut bytes).unwrap();
            assert!(serials_from_plist(&bytes).unwrap().is_empty());
        }

        #[test]
        fn target_and_device_identity_fail_closed() {
            for value in [
                "/Volumes/RPI-RP2",
                "picoboot:",
                "picoboot:--force",
                "picoboot:A/B",
                "picoboot:A\nB",
            ] {
                assert!(target(value).is_err());
            }
            let info = "Device Information\n type: RP2040\n flash size: 2048K\n flash id: 0x53032847490B8B1C\n";
            assert_eq!(
                parse_identity(info, Some("53032847490b8b1c")).unwrap(),
                Some("53032847490B8B1C".to_string())
            );
            assert!(parse_identity(info, Some("AAAAAAAAAAAAAAAA")).is_err());
            assert!(parse_identity(&info.replace("RP2040", "RP2350"), None).is_err());
            assert!(parse_identity(&format!("{info}{info}"), None).is_err());
            assert!(parse_identity("type: RP2040", None).is_err());
        }

        #[test]
        fn blank_rp2040_allows_first_setup_only_and_ambiguous_info_is_rejected() {
            let blank = "Device Information\n type: RP2040\n";
            assert_eq!(parse_identity(blank, None).unwrap(), None);
            assert!(parse_identity(blank, Some("53032847490B8B1C")).is_err());
            assert!(parse_identity(&format!("{blank}{blank}"), None).is_err());
            assert!(parse_identity(&blank.replace("RP2040", "RP2350"), None).is_err());
            assert!(parse_identity(&format!("{blank} flash id: invalid\n"), None).is_err());
            assert!(parse_identity(
                &format!("{blank} flash id: 0x53032847490B8B1C\n flash id: 0x53032847490B8B1C\n"),
                None
            )
            .is_err());
            assert!(parse_identity(&format!("{blank}Device Information\n"), None).is_err());
            let command = load_command(
                Path::new("/not-executed/picotool"),
                None,
                Path::new("/private/fixed.uf2"),
            );
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                ["load", "-v", "-x", "/private/fixed.uf2"]
            );
        }

        #[test]
        fn flash_selector_uses_verified_identity_and_unknown_setup_requires_one_rom() {
            let devices = vec!["E0C9125B0D9B".to_string()];
            assert_eq!(
                preflight_selector("E0C9125B0D9B", &devices, Some("53032847490b8b1c")).unwrap(),
                Some("53032847490B8B1C".to_string())
            );
            assert_eq!(
                preflight_selector("E0C9125B0D9B", &devices, None).unwrap(),
                None
            );
            assert!(preflight_selector("MISSING", &devices, None).is_err());
            assert!(preflight_selector(
                "E0C9125B0D9B",
                &["E0C9125B0D9B".into(), "SECOND".into()],
                None
            )
            .is_err());
            let command = helper_command(
                Path::new("/not-executed/picotool"),
                Some("53032847490B8B1C"),
                "load",
            );
            let args: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
            assert_eq!(args, ["load", "--ser", "53032847490B8B1C"]);
            let command = helper_command(Path::new("/not-executed/picotool"), None, "info");
            assert_eq!(command.get_args().collect::<Vec<_>>(), ["info"]);
        }

        #[test]
        fn child_timeout_and_cancellation_are_bounded_and_reaped() {
            let mut sleeping = Command::new("/bin/sleep");
            sleeping.arg("5");
            let start = Instant::now();
            assert!(
                run(sleeping, Duration::from_millis(80), &AtomicBool::new(false))
                    .err()
                    .unwrap()
                    .contains("timed out")
            );
            assert!(start.elapsed() < Duration::from_secs(2));
            let cancel = std::sync::Arc::new(AtomicBool::new(false));
            let delayed = cancel.clone();
            let trigger = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(80));
                delayed.store(true, Ordering::SeqCst);
            });
            let mut sleeping = Command::new("/bin/sleep");
            sleeping.arg("5");
            assert!(run(sleeping, Duration::from_secs(2), &cancel)
                .err()
                .unwrap()
                .contains("canceled"));
            trigger.join().unwrap();
        }

        #[test]
        fn readback_success_uses_fixed_verify_command_exit_status() {
            use std::os::unix::process::ExitStatusExt;
            let command = load_command(
                Path::new("/not-executed/picotool"),
                Some("53032847490B8B1C"),
                Path::new("/private/fixed.uf2"),
            );
            let args: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
            assert_eq!(
                args,
                [
                    "load",
                    "--ser",
                    "53032847490B8B1C",
                    "-v",
                    "-x",
                    "/private/fixed.uf2"
                ]
            );
            let success = b"Loading into Flash: 100%\rVerifying Flash: 100%\n  OK\nThe device was rebooted to start the application.\n";
            assert!(verified(&Output {
                status: ExitStatus::from_raw(0),
                bytes: success.to_vec()
            }));
            assert!(verified(&Output {
                status: ExitStatus::from_raw(0),
                bytes: Vec::new()
            }));
            assert!(!verified(&Output {
                status: ExitStatus::from_raw(1 << 8),
                bytes: success.to_vec()
            }));
            assert!(!verified(&Output {
                status: ExitStatus::from_raw(1 << 8),
                bytes: b"FAILED\nERROR: The device contents did not match the file".to_vec()
            }));
        }

        #[test]
        fn helper_error_preserves_failure_after_long_progress() {
            let mut text = "Loading into Flash: 46%\r".repeat(200);
            text.push_str("Verifying Flash: 46%\r  FAILED\nERROR: The device contents did not match the file\n");
            let error = helper_error(text.as_bytes());
            assert!(error.contains("The device contents did not match the file"));
            assert!(!error.contains("Loading into"));
            assert!(error.len() < 1400);
        }
    }
}
