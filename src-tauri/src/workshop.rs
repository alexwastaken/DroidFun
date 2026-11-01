//! Private, single-board manufacturing tool. It never starts consumer workers.
use crate::{
    workshop_connection::{self as connection, DeviceIdentity, Link, RobotPort},
    workshop_firmware as firmware_setup, workshop_protocol as protocol, UsbOperationGuard,
};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

const FIRMWARE_VERSION: &str = "1.3.2";
const PROFILE: &str = "rp2040-zero-gp4-gp5-ssd1306";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Wiring {
    #[default]
    Standard,
    Mistake,
}

impl Wiring {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "standard" => Ok(Self::Standard),
            "mistake" => Ok(Self::Mistake),
            _ => Err("Choose Standard or Mistake screen wiring.".into()),
        }
    }
    fn firmware_version(self) -> &'static str {
        match self {
            Self::Standard => FIRMWARE_VERSION,
            Self::Mistake => "1.3.2-mistake",
        }
    }
    fn pin_profile(self) -> &'static str {
        match self {
            Self::Standard => "GP4-GP5",
            Self::Mistake => "GP5-SDA-GP4-SCL",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Board {
    id: String,
    name: String,
    kind: String,
    serial: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Scan {
    boards: Vec<Board>,
    consumer_running: bool,
    wipe_supported: bool,
}

/// Read-only evidence from one HELLO exchange, never inferred from USB presence.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Inspection {
    target: String,
    state: String,
    message: String,
    firmware: Option<String>,
    board_id: Option<String>,
    identity: Option<DeviceIdentity>,
    wiring: Option<Wiring>,
    display_health: Option<String>,
    startup_saved: bool,
    ready_to_verify: bool,
}

fn installed_wiring(status: &connection::ConnectionSnapshot) -> Option<Wiring> {
    // A version suffix alone is not evidence of the actual pins in use.
    match status.pin_profile.as_deref() {
        Some("GP4-GP5") => Some(Wiring::Standard),
        Some("GP5-SDA-GP4-SCL") => Some(Wiring::Mistake),
        _ => None,
    }
}

fn release_number(version: &str) -> Option<[u32; 3]> {
    let parts: Vec<_> = version
        .strip_suffix("-mistake")
        .unwrap_or(version)
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<_, _>>()
        .ok()?;
    parts.try_into().ok()
}

fn installed_inspection(target: &str, status: &connection::ConnectionSnapshot) -> Inspection {
    let wiring = installed_wiring(status);
    let version = status.firmware.as_deref().unwrap_or("");
    let bundled = release_number(FIRMWARE_VERSION).unwrap();
    let state = if wiring.is_some_and(|w| version == w.firmware_version()) {
        "current"
    } else if release_number(version).is_some_and(|v| v < bundled) {
        "update_available"
    } else if release_number(version).is_some_and(|v| v > bundled) {
        "newer"
    } else {
        "unknown"
    };
    let ready_to_verify = state == "current"
        && status.identity.is_some()
        && status.provisioning_supported
        && status.standalone_saved
        && status.standalone_count == 1
        && status.standalone_content_crc.is_some()
        && status.display_health.as_deref() == Some("ready");
    Inspection {
        target: target.into(),
        state: state.into(),
        message: match state {
            "current" if ready_to_verify => "Firmware and bot identity are already installed. Check this board for shipping without reinstalling.",
            "current" => "Firmware is already installed. Finish the bot identity, startup face or screen setup before shipping.",
            "update_available" => "DroidFun firmware is installed. A newer version is included in Workshop.",
            "newer" => "This board has newer firmware than this Workshop. Reinstalling would replace it with the older bundled version.",
            _ => "DroidFun firmware responded, but its version or wiring could not be matched to this Workshop. Review the details before programming.",
        }.into(),
        firmware: status.firmware.clone(),
        board_id: status.board_id.clone(),
        identity: status.identity.clone(),
        wiring,
        display_health: status.display_health.clone(),
        startup_saved: status.standalone_saved,
        ready_to_verify,
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    #[serde(skip)]
    job: u64,
    #[serde(skip)]
    startup_content_crc: Option<u32>,
    phase: String,
    message: String,
    busy: bool,
    firmware_version: String,
    wiring: Wiring,
    model: Option<String>,
    board_id: Option<String>,
    identity: Option<DeviceIdentity>,
    firmware_verified: bool,
    identity_verified: bool,
    display_ready: bool,
    disconnected_observed: bool,
    power_off_confirmed: bool,
    reconnect_verified: bool,
    screen_confirmed: bool,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            job: 0,
            startup_content_crc: None,
            phase: "idle".into(),
            message: "Choose a bot and connect one board.".into(),
            busy: false,
            firmware_version: FIRMWARE_VERSION.into(),
            wiring: Wiring::Standard,
            model: None,
            board_id: None,
            identity: None,
            firmware_verified: false,
            identity_verified: false,
            display_ready: false,
            disconnected_observed: false,
            power_off_confirmed: false,
            reconnect_verified: false,
            screen_confirmed: false,
        }
    }
}

struct WorkshopState(Mutex<Status>);

fn snapshot(app: &AppHandle) -> Result<Status, String> {
    app.state::<WorkshopState>()
        .0
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "Workshop state is unavailable.".into())
}

fn change(app: &AppHandle, update: impl FnOnce(&mut Status)) {
    if let Ok(mut status) = app.state::<WorkshopState>().0.lock() {
        update(&mut status);
        let _ = app.emit("workshop-progress", status.clone());
    }
}

fn stage(app: &AppHandle, phase: &str, message: &str) {
    change(app, |s| {
        s.phase = phase.into();
        s.message = message.into();
    });
}

fn consumer_name(name: &str, path: Option<&str>) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "deskbot" | "deskbot.exe" | "droidfun-app" | "droidfun-app.exe" | "droidfun app" | "droidfun app.exe"
    ) || path.is_some_and(|p| {
        p.contains("/DroidFun App.app/Contents/MacOS/")
            || p.contains("/Deskbot.app/Contents/MacOS/")
    })
}

fn consumer_processes() -> Vec<u32> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing()
            .with_exe(sysinfo::UpdateKind::OnlyIfNotSet)
            .without_tasks(),
    );
    system
        .processes()
        .iter()
        .filter(|(pid, process)| {
            pid.as_u32() != std::process::id()
                && !matches!(
                    process.status(),
                    sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead
                )
                && consumer_name(
                    &process.name().to_string_lossy(),
                    process.exe().and_then(|p| p.to_str()),
                )
        })
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

fn consumer_running() -> bool {
    !consumer_processes().is_empty()
}

// An explicit button requests a normal application quit. Never force-kill an
// app or assume that a successful request means its USB handle is released.
#[tauri::command]
pub(crate) async fn workshop_close_consumer(app: AppHandle) -> Result<(), String> {
    let operation = operation(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        #[cfg(target_os = "macos")]
        {
            for pid in consumer_processes() {
                let running = objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32);
                if let Some(running) = running {
                    if !running.terminate() {
                        return Err("DroidFun App could not be closed. Open it and choose DroidFun App → Quit; if it belongs to another Mac login, quit it there.".into());
                    }
                }
            }
            let deadline = Instant::now() + Duration::from_secs(8);
            loop {
                if !consumer_running() { return Ok(()); }
                if Instant::now() >= deadline {
                    return Err("DroidFun App is still open. Finish any prompt in that app and choose Quit, then scan again.".into());
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        }
        #[cfg(not(target_os = "macos"))]
        Err("Quit DroidFun App from its menu, then scan again.".into())
    }).await.map_err(|_| "Could not request that DroidFun App close.".to_string())?
}

fn require_consumer_closed() -> Result<(), String> {
    if consumer_running() {
        return Err("Quit DroidFun App before using Workshop. Closing its window may leave it running in the background.".into());
    }
    Ok(())
}

fn boards() -> Result<Vec<Board>, String> {
    let mut found: Vec<_> = connection::robot_ports()?
        .into_iter()
        .map(|p| Board {
            id: format!("serial:{}", p.serial),
            name: format!("RP2040-Zero · {}", p.serial),
            kind: "serial".into(),
            serial: Some(p.serial),
        })
        .collect();
    found.extend(firmware_setup::drives()?.into_iter().map(|p| Board {
        id: format!("setup:{}", p.path),
        name: format!("Programming mode · {}", p.name),
        kind: "setup".into(),
        serial: None,
    }));
    Ok(found)
}

fn one_board<'a>(found: &'a [Board], target: &str) -> Result<&'a Board, String> {
    if found.len() != 1 {
        return Err("Connect exactly one board before programming or verifying. Disconnect any other RP2040 boards.".into());
    }
    found.iter().find(|b| b.id == target).ok_or_else(|| {
        "The selected board has changed or disconnected. Scan again before continuing.".into()
    })
}

fn observed_unplug(initial: &Status, current: &Status, found: &[Board]) -> bool {
    initial.phase == "awaiting_disconnect"
        && current.job == initial.job
        && current.phase == "awaiting_disconnect"
        && !current.busy
        && found.is_empty()
}

fn exact_port(serial: &str) -> Result<RobotPort, String> {
    let found = boards()?;
    if found.len() != 1
        || found[0]
            .serial
            .as_deref()
            .is_none_or(|s| !s.eq_ignore_ascii_case(serial))
    {
        return Err(
            "Only the programmed board should be connected. Check its board ID and USB cable."
                .into(),
        );
    }
    connection::robot_ports()?
        .into_iter()
        .find(|p| p.serial.eq_ignore_ascii_case(serial))
        .ok_or_else(|| "The programmed board is no longer connected.".into())
}

fn canceled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::SeqCst) {
        Err("The USB operation timed out. Reconnect the board before trying again.".into())
    } else {
        Ok(())
    }
}

struct Operation {
    app: AppHandle,
    _usb: Arc<UsbOperationGuard>,
}
impl Drop for Operation {
    fn drop(&mut self) {
        change(&self.app, |s| s.busy = false);
    }
}

fn operation(app: &AppHandle) -> Result<Operation, String> {
    crate::showcase::require_board_mode(app)?;
    let usb = UsbOperationGuard::acquire()?;
    change(app, |s| s.busy = true);
    Ok(Operation {
        app: app.clone(),
        _usb: usb,
    })
}

#[tauri::command]
pub(crate) fn workshop_status(app: AppHandle) -> Result<Status, String> {
    snapshot(&app)
}

#[tauri::command]
pub(crate) async fn workshop_scan(app: AppHandle) -> Result<Scan, String> {
    let initial = snapshot(&app)?;
    if initial.busy {
        return Err("Wait for the current board operation to finish.".into());
    }
    let result = tokio::time::timeout(
        Duration::from_secs(6),
        tauri::async_runtime::spawn_blocking(|| {
            Ok::<_, String>(Scan {
                boards: boards()?,
                consumer_running: consumer_running(),
                wipe_supported: crate::picoboot::supported(),
            })
        }),
    )
    .await
    .map_err(|_| "USB scan timed out. Reconnect the cable and scan again.")?
    .map_err(|_| "USB scan stopped.")??;
    change(&app, |s| {
        if observed_unplug(&initial, s, &result.boards) {
            s.disconnected_observed = true;
            s.phase = "awaiting_unplug_confirmation".into();
            s.message =
                "No board detected. Confirm that you unplugged all power before reconnecting."
                    .into();
        }
    });
    Ok(result)
}

#[tauri::command]
pub(crate) async fn workshop_inspect(target: String, app: AppHandle) -> Result<Inspection, String> {
    if snapshot(&app)?.busy {
        return Err("Wait for the current board operation to finish.".into());
    }
    // Hold the USB lock until the worker really exits, even if its caller times out.
    let guard = UsbOperationGuard::acquire()?;
    let result=tokio::time::timeout(Duration::from_secs(8), tauri::async_runtime::spawn_blocking(move || -> Result<Inspection,String> {
        let _guard = guard;
        require_consumer_closed()?;
        let found = boards()?;
        let board = one_board(&found, &target)?;
        let Some(serial) = &board.serial else {
            return Ok(Inspection {
                target,
                state: "bootloader".into(),
                message: "This board is in programming mode. It is ready for firmware installation. Existing firmware cannot be read in this mode; reconnect normally to check it first.".into(),
                firmware: None, board_id: None, identity: None, wiring: None,
                display_health: None, startup_saved: false, ready_to_verify: false,
            });
        };
        let candidate = exact_port(serial)?;
        // HELLO reads status only. Never restart, provision, write a display frame,
        // save a playlist or install firmware during automatic inspection.
        let link = Link::connect(&candidate)?;
        exact_port(serial)?;
        Ok(installed_inspection(&target, &link.status))
    })).await.map_err(|_| "Board inspection timed out. Reconnect USB, then check again.".to_string())?
        .map_err(|_| "Board inspection stopped. Check USB again.".to_string())??;
    if let (Some(id),Some(identity),Some(firmware))=(&result.board_id,&result.identity,&result.firmware){
        if let Err(e)=crate::workshop_registry::observe(&app,crate::workshop_registry::Record{board_id:id.clone(),model:identity.model.clone(),firmware:firmware.clone(),wiring:result.wiring.map(|w|match w{Wiring::Standard=>"standard",Wiring::Mistake=>"mistake"}).unwrap_or("").into(),prepared:false,reset_preparation:false}){let _=app.emit("workshop-registry-error",e);}
    }
    Ok(result)
}

#[tauri::command]
pub(crate) async fn workshop_record_existing(target:String,model:String,app:AppHandle)->Result<(),String>{
    if !["boo","claw","meep","nimbus"].contains(&model.as_str()){return Err("Choose the model for this droid.".into());}
    let inspected=workshop_inspect(target,app.clone()).await?;
    crate::workshop_registry::observe(&app,crate::workshop_registry::Record{board_id:inspected.board_id.ok_or("Reconnect this droid normally to read its ID.")?,model:inspected.identity.map(|i|i.model).unwrap_or(model),firmware:inspected.firmware.ok_or("Could not read installed firmware")?,wiring:inspected.wiring.map(|w|match w{Wiring::Standard=>"standard",Wiring::Mistake=>"mistake"}).unwrap_or("").into(),prepared:false,reset_preparation:false})
}

fn prepare_serial(serial: &str, cancel: &AtomicBool, app: &AppHandle) -> Result<String, String> {
    let candidate = exact_port(serial)?;
    let before = firmware_setup::drives()?;
    canceled(cancel)?;
    stage(
        app,
        "preparing",
        "Restarting the selected board into programming mode…",
    );
    let mut usb = serialport::new(&candidate.path, 115200)
        .dtr_on_open(true)
        .timeout(Duration::from_millis(500))
        .open()
        .map_err(|_| {
            "Could not open this board. Quit other serial tools, reconnect USB, and try again."
                .to_string()
        })?;
    canceled(cancel)?;
    crate::request_usb_restart(&mut usb, cancel, || {
        std::thread::sleep(Duration::from_millis(100))
    })?;
    drop(usb);
    if let Some(path) = wait_for_setup(&before, cancel, Duration::from_secs(15))? {
        return Ok(path);
    }
    #[cfg(target_os = "macos")]
    {
        require_consumer_closed()?;
        exact_port(serial)?;
        canceled(cancel)?;
        stage(
            app,
            "preparing",
            "Trying the board’s standard USB restart sequence…",
        );
        // Arduino-Pico's uf2conv uses standard termios rates and drops DTR
        // before applying 1200 baud. serialport's macOS rate setter uses the
        // different IOSSIOSPEED path. Retry the upstream sequence once.
        let requested = standard_usb_restart(serial, cancel);
        canceled(cancel)?;
        if let Some(path) = wait_for_setup(&before, cancel, Duration::from_secs(15))? {
            return Ok(path);
        }
        requested?;
    }
    Err("The board did not enter programming mode. For a bare board, hold BOOT while plugging in its USB cable, then scan again.".into())
}

fn wait_for_setup(
    before: &[firmware_setup::SetupDrive],
    cancel: &AtomicBool,
    budget: Duration,
) -> Result<Option<String>, String> {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        canceled(cancel)?;
        let found = firmware_setup::drives()?;
        let new: Vec<_> = found
            .iter()
            .filter(|d| !before.iter().any(|old| old.path == d.path))
            .collect();
        if found.len() == 1 && new.len() == 1 {
            return Ok(Some(new[0].path.clone()));
        }
        if found.len() > 1 {
            return Err("More than one board entered programming mode. Disconnect the other boards and scan again.".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn standard_usb_restart(serial: &str, cancel: &AtomicBool) -> Result<(), String> {
    use serialport::SerialPort;
    use std::os::fd::AsRawFd;

    fn baud(port: &serialport::TTYPort, rate: libc::speed_t) -> Result<(), String> {
        let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
        // The TTYPort owns this live descriptor until the operation finishes.
        if unsafe { libc::tcgetattr(port.as_raw_fd(), settings.as_mut_ptr()) } != 0 {
            return Err("Could not read this board’s USB restart settings.".into());
        }
        let mut settings = unsafe { settings.assume_init() };
        if unsafe { libc::cfsetispeed(&mut settings, rate) } != 0
            || unsafe { libc::cfsetospeed(&mut settings, rate) } != 0
            || unsafe { libc::tcsetattr(port.as_raw_fd(), libc::TCSANOW, &settings) } != 0
        {
            return Err("Could not apply this board’s standard USB restart settings.".into());
        }
        Ok(())
    }

    let candidate = exact_port(serial)?;
    canceled(cancel)?;
    let mut usb = serialport::new(&candidate.path, 9600)
        .dtr_on_open(true)
        .timeout(Duration::from_millis(500))
        .open_native()
        .map_err(|_| "Could not reopen the selected board for USB restart.")?;
    canceled(cancel)?;
    if exact_port(serial)?.path != candidate.path {
        return Err("The selected USB port changed. Scan again before programming.".into());
    }
    baud(&usb, libc::B9600)?;
    canceled(cancel)?;
    usb.write_data_terminal_ready(true)
        .map_err(|_| "Could not prepare this board’s USB restart.")?;
    canceled(cancel)?;
    std::thread::sleep(Duration::from_millis(100));
    canceled(cancel)?;
    usb.write_data_terminal_ready(false)
        .map_err(|_| "Could not request this board’s USB restart.")?;
    canceled(cancel)?;
    // This is the sole triggering call in the fallback. The same UID is
    // checked again in ROM before any firmware write by firmware_setup.
    let trigger = baud(&usb, libc::B1200);
    if trigger.is_err() || cancel.load(Ordering::SeqCst) {
        // A failed native call may still have applied the setting. Disarm
        // before close whenever the operation did not finish normally.
        let _ = baud(&usb, libc::B115200);
    }
    drop(usb);
    trigger?;
    canceled(cancel)
}

fn check_identity(
    actual: Option<&DeviceIdentity>,
    expected: &DeviceIdentity,
) -> Result<(), String> {
    if actual.is_some_and(|a| {
        a.model == expected.model
            && a.hardware_profile == expected.hardware_profile
            && a.generation == expected.generation
    }) {
        Ok(())
    } else {
        Err("The board did not return the model identity that Workshop saved. Do not ship this board yet.".into())
    }
}

fn check_firmware(
    status: &connection::ConnectionSnapshot,
    serial: &str,
    wiring: Wiring,
) -> Result<(), String> {
    if status
        .board_id
        .as_deref()
        .is_none_or(|id| !id.eq_ignore_ascii_case(serial))
        || status.firmware.as_deref() != Some(wiring.firmware_version())
        || match status.pin_profile.as_deref() {
            Some(profile) => profile != wiring.pin_profile(),
            None => wiring != Wiring::Standard,
        }
    {
        return Err(
            "The returning board identity, firmware or screen wiring did not match this programming job."
                .into(),
        );
    }
    Ok(())
}

fn check_saved_face(
    status: &connection::ConnectionSnapshot,
    expected_crc: u32,
) -> Result<(), String> {
    if status.standalone_saved
        && status.standalone_count == 1
        && status.standalone_content_crc == Some(expected_crc)
    {
        Ok(())
    } else {
        Err("The startup face did not match what Workshop saved. Program this board again before shipping it.".into())
    }
}

fn current_model(model: &str) -> bool {
    protocol::FACTORY_MODELS.contains(&model)
}

fn program(
    target: &str,
    model: &str,
    wiring: Wiring,
    fresh: bool,
    app: &AppHandle,
    cancel: &AtomicBool,
) -> Result<(), String> {
    require_consumer_closed()?;
    if !current_model(model) {
        return Err("Choose Nimbus, Boo, Meep or Claw before programming.".into());
    }
    let detected = boards()?;
    let board = one_board(&detected, target)?;
    canceled(cancel)?;
    if fresh && !crate::picoboot::supported() {
        return Err("Wipe and reinstall requires direct USB on a supported Mac.".into());
    }
    if fresh {
        if let Some(serial) = &board.serial {
            reset_preparation(app, serial, model, wiring)?;
        }
    }
    let (path, expected) = if let Some(serial) = &board.serial {
        (prepare_serial(serial, cancel, app)?, Some(serial.clone()))
    } else {
        (
            board
                .id
                .strip_prefix("setup:")
                .ok_or("Invalid programming target.")?
                .to_string(),
            None,
        )
    };
    canceled(cancel)?;
    require_consumer_closed()?;
    stage(
        app,
        "programming",
        "Installing and verifying the bundled firmware…",
    );
    let report = |message: &str| {
        if !cancel.load(Ordering::SeqCst) {
            stage(app, "programming", message);
        }
    };
    let serial = if fresh {
        firmware_setup::install_fresh(&path, expected.as_deref(), wiring == Wiring::Mistake, cancel, report)
    } else { match wiring {
        Wiring::Standard => firmware_setup::install(&path, expected.as_deref(), cancel, report),
        Wiring::Mistake => {
            firmware_setup::install_mistake(&path, expected.as_deref(), cancel, report)
        }
    } }?;
    if fresh { reset_preparation(app, &serial, model, wiring)?; }
    canceled(cancel)?;
    change(app, |s| {
        s.board_id = Some(serial.clone());
    });
    stage(
        app,
        "identifying",
        "Checking the returning board and saving its bot identity…",
    );
    require_consumer_closed()?;
    let candidate = exact_port(&serial)?;
    let mut link = Link::connect(&candidate)?;
    canceled(cancel)?;
    check_firmware(&link.status, &serial, wiring)?;
    change(app, |s| {
        s.firmware_verified = true;
    });
    if !link.status.provisioning_supported {
        return Err("This firmware does not support Workshop identification.".into());
    }
    let identity = link.provision_identity(model, &serial)?;
    canceled(cancel)?;
    if identity.model != model || identity.hardware_profile != PROFILE {
        return Err("The saved model or wiring profile did not match this job.".into());
    }
    change(app, |s| {
        s.identity_verified = true;
        s.identity = Some(identity.clone());
    });
    stage(
        app,
        "screen",
        "Saving the startup face and checking screen communication…",
    );
    let face = crate::core::render(None, true);
    let playlist = protocol::StandalonePlaylist {
        frames: vec![protocol::StandaloneFrame {
            frame: face.clone(),
            duration_sec: 8,
        }],
        brightness: 85,
        orientation: 0,
    };
    let bytes = playlist.encode()?;
    let startup_content_crc = protocol::crc32(&bytes[..bytes.len() - 4]);
    link.save_standalone(&playlist)?;
    canceled(cancel)?;
    check_saved_face(&link.status, startup_content_crc)?;
    change(app, |s| s.startup_content_crc = Some(startup_content_crc));
    link.update(&face, true, 85, 0)?;
    canceled(cancel)?;
    if link.status.display_health.as_deref() != Some("ready") {
        return Err("The model is saved, but the screen is not responding. Check the screen and wiring before continuing.".into());
    }
    drop(link);
    canceled(cancel)?;
    change(app, |s| {
        s.display_ready = true;
        s.phase = "awaiting_disconnect".into();
        s.message = "Programmed. Unplug all power from this board and leave it disconnected until Workshop asks you to reconnect.".into();
    });
    Ok(())
}

async fn finish_job(
    app: AppHandle,
    job: tauri::async_runtime::JoinHandle<Result<(), String>>,
    cancel: Arc<AtomicBool>,
    budget_seconds: u64,
) -> Result<Status, String> {
    let result = match tokio::time::timeout(Duration::from_secs(budget_seconds), job).await {
        Ok(result) => result
            .map_err(|_| "Workshop’s board operation stopped unexpectedly.".to_string())
            .and_then(|r| r),
        Err(_) => {
            cancel.store(true, Ordering::SeqCst);
            Err("USB timed out. A driver may still be finishing; keep Workshop open until the operation stops, then reconnect the board.".into())
        }
    };
    if let Err(message) = result {
        stage(&app, "error", &message);
        return Err(message);
    }
    snapshot(&app)
}

#[tauri::command]
pub(crate) async fn workshop_program(
    target: String,
    model: String,
    wiring: String,
    app: AppHandle,
) -> Result<Status, String> {
    start_program(target, model, wiring, false, app).await
}

#[tauri::command]
pub(crate) async fn workshop_wipe_reinstall(target: String, model: String, wiring: String, confirmed_target: String, app: AppHandle) -> Result<Status, String> {
    if target.is_empty() || target != confirmed_target {
        return Err("Review and confirm the connected board before wiping it.".into());
    }
    start_program(target, model, wiring, true, app).await
}

fn reset_preparation(app: &AppHandle, serial: &str, model: &str, wiring: Wiring) -> Result<(), String> {
    crate::workshop_registry::observe(app, crate::workshop_registry::Record {
        board_id: serial.into(), model: model.into(), firmware: wiring.firmware_version().into(),
        wiring: match wiring { Wiring::Standard => "standard", Wiring::Mistake => "mistake" }.into(),
        prepared: false, reset_preparation: true,
    })
}

async fn start_program(target: String, model: String, wiring: String, fresh: bool, app: AppHandle) -> Result<Status, String> {
    if !current_model(&model) {
        return Err("Choose a bot model first.".into());
    }
    let wiring = Wiring::parse(&wiring)?;
    let operation = operation(&app)?;
    change(&app, |s| {
        *s = Status {
            job: s.job.wrapping_add(1),
            busy: true,
            phase: "preparing".into(),
            message: "Checking the selected board…".into(),
            model: Some(model.clone()),
            wiring,
            firmware_version: wiring.firmware_version().into(),
            ..Status::default()
        };
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_app = app.clone();
    let job = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        program(&target, &model, wiring, fresh, &worker_app, &worker_cancel)
    });
    finish_job(app, job, cancel, if fresh { 160 } else { 100 }).await
}

#[tauri::command]
pub(crate) async fn workshop_verify_installed(
    target: String,
    model: String,
    wiring: String,
    app: AppHandle,
) -> Result<Status, String> {
    if !current_model(&model) {
        return Err("Choose the installed bot model first.".into());
    }
    let wiring = Wiring::parse(&wiring)?;
    let operation = operation(&app)?;
    change(&app, |s| {
        *s = Status {
            job: s.job.wrapping_add(1),
            busy: true,
            phase: "verifying".into(),
            message: "Checking installed firmware, bot identity and saved face…".into(),
            model: Some(model.clone()),
            wiring,
            firmware_version: wiring.firmware_version().into(),
            ..Status::default()
        };
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_app = app.clone();
    let job = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        require_consumer_closed()?;
        let found = boards()?;
        let board = one_board(&found, &target)?;
        let serial = board
            .serial
            .as_deref()
            .ok_or("Reconnect the board normally to read installed firmware.")?;
        let link = Link::connect(&exact_port(serial)?)?;
        canceled(&worker_cancel)?;
        check_firmware(&link.status, serial, wiring)?;
        let inspection = installed_inspection(&target, &link.status);
        if !inspection.ready_to_verify || inspection.wiring != Some(wiring) {
            return Err("This board needs preparation before it can pass the shipping checks. Review its firmware, saved identity, startup face and screen wiring.".into());
        }
        let identity = link
            .status
            .identity
            .clone()
            .ok_or("This board has no saved bot identity.")?;
        if identity.model != model || identity.hardware_profile != PROFILE {
            return Err("The selected bot does not match the installed identity. Choose the detected bot or explicitly reprogram this board.".into());
        }
        let startup_content_crc = link.status.standalone_content_crc;
        drop(link);
        canceled(&worker_cancel)?;
        exact_port(serial)?;
        change(&worker_app, |s| {
            s.board_id = Some(serial.into());
            s.identity = Some(identity);
            s.startup_content_crc = startup_content_crc;
            s.firmware_verified = true;
            s.identity_verified = true;
            s.display_ready = true;
            s.phase = "awaiting_disconnect".into();
            s.message = "Installed firmware and identity checked without reinstalling. Unplug all power from this board, then confirm below before reconnecting.".into();
        });
        Ok(())
    });
    finish_job(app, job, cancel, 10).await
}

#[tauri::command]
pub(crate) async fn workshop_confirm_unplugged(app: AppHandle) -> Result<Status, String> {
    let previous = snapshot(&app)?;
    if previous.busy
        || !previous.disconnected_observed
        || previous.phase != "awaiting_unplug_confirmation"
    {
        return Err(
            "Unplug all power and wait for Workshop to show that no board is detected.".into(),
        );
    }
    let operation = operation(&app)?;
    let worker_app = app.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let job = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        if !boards()?.is_empty() {
            return Err("A board is still detected. Unplug all power before confirming.".into());
        }
        canceled(&worker_cancel)?;
        change(&worker_app, |s| {
            s.power_off_confirmed = true;
            s.phase = "awaiting_reconnect".into();
            s.message = "You confirmed all power is unplugged. Reconnect the same board’s USB cable, then verify it below.".into();
        });
        Ok::<_, String>(())
    });
    finish_job(app, job, cancel, 8).await
}

#[tauri::command]
pub(crate) async fn workshop_verify_reconnect(app: AppHandle) -> Result<Status, String> {
    let previous = snapshot(&app)?;
    if !previous.disconnected_observed
        || !previous.power_off_confirmed
        || !["awaiting_reconnect", "error"].contains(&previous.phase.as_str())
    {
        return Err(
            "Unplug the programmed board and confirm that all power is off before reconnecting."
                .into(),
        );
    }
    let serial = previous
        .board_id
        .clone()
        .ok_or("No programmed board to verify.")?;
    let identity = previous
        .identity
        .clone()
        .ok_or("The model identity has not been saved.")?;
    let startup_content_crc = previous
        .startup_content_crc
        .ok_or("The startup face has not been saved and verified.")?;
    let wiring = previous.wiring;
    let operation = operation(&app)?;
    stage(
        &app,
        "verifying",
        "Reading this board’s identity after reconnection…",
    );
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_app = app.clone();
    let job = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        require_consumer_closed()?;
        let candidate = exact_port(&serial)?;
        let link = Link::connect(&candidate)?;
        canceled(&worker_cancel)?;
        check_firmware(&link.status, &serial, wiring)?;
        check_identity(link.status.identity.as_ref(), &identity)?;
        check_saved_face(&link.status, startup_content_crc)?;
        if link.status.display_health.as_deref() != Some("ready") {
            return Err("The screen did not respond after reconnection. Check the hardware before shipping this board.".into());
        }
        drop(link);
        canceled(&worker_cancel)?;
        change(&worker_app, |s| {
            s.reconnect_verified = true;
            s.phase = "awaiting_screen".into();
            s.message = "Identity and startup face are retained. Look at the physical screen and confirm that the face is readable.".into();
        });
        Ok(())
    });
    finish_job(app, job, cancel, 10).await
}

#[tauri::command]
pub(crate) async fn workshop_confirm_screen(app: AppHandle) -> Result<Status, String> {
    let previous = snapshot(&app)?;
    if previous.busy || previous.phase != "awaiting_screen" || !previous.reconnect_verified {
        return Err("Finish the reconnect check before confirming the screen.".into());
    }
    let serial = previous.board_id.ok_or("No programmed board to confirm.")?;
    let operation = operation(&app)?;
    let worker_app = app.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let job = tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        exact_port(&serial)?;
        canceled(&worker_cancel)?;
        change(&worker_app, |s| {
            s.screen_confirmed = true;
            s.phase = "complete".into();
            s.message = "Programming and reconnect checks passed. You confirmed a readable face on this board.".into();
        });
        Ok::<_, String>(())
    });
    let result=finish_job(app.clone(), job, cancel, 8).await?;
    if result.phase=="complete" {
        if let (Some(id),Some(model))=(&result.board_id,&result.model){
            if let Err(e)=crate::workshop_registry::observe(&app,crate::workshop_registry::Record{board_id:id.clone(),model:model.clone(),firmware:result.firmware_version.clone(),wiring:match result.wiring{Wiring::Standard=>"standard",Wiring::Mistake=>"mistake"}.into(),prepared:true,reset_preparation:false}){let _=app.emit("workshop-registry-error",e);}
        }
    }
    Ok(result)
}

#[tauri::command]
pub(crate) fn workshop_next(app: AppHandle) -> Result<Status, String> {
    if snapshot(&app)?.busy {
        return Err("Wait for the current operation to finish.".into());
    }
    change(&app, |s| {
        *s = Status {
            job: s.job.wrapping_add(1),
            ..Status::default()
        }
    });
    snapshot(&app)
}

pub(crate) fn initialize(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(WorkshopState(Mutex::new(Status::default())));
    let root=app.path().app_config_dir()?.join("registry");
    app.manage(crate::workshop_registry::Store::new(root).map_err(std::io::Error::other)?);
    Ok(())
}
pub(crate) fn can_show_screens(app: &AppHandle) -> bool {
    snapshot(app).is_ok_and(|s| !s.busy && matches!(s.phase.as_str(), "idle"|"error"|"complete"))
}
pub(crate) fn busy(app: &AppHandle) -> bool { snapshot(app).is_ok_and(|s|s.busy) }

#[cfg(test)]
mod tests {
    fn installed(version: &str, pins: Option<&str>) -> super::connection::ConnectionSnapshot {
        super::connection::ConnectionSnapshot {
            board_id: Some("5303284728CB1C9C".into()),
            firmware: Some(version.into()),
            pin_profile: pins.map(str::to_owned),
            identity: Some(super::DeviceIdentity {
                model: "nimbus".into(),
                hardware_profile: super::PROFILE.into(),
                generation: 1,
            }),
            display_health: Some("ready".into()),
            provisioning_supported: true,
            standalone_saved: true,
            standalone_count: 1,
            standalone_content_crc: Some(123),
            ..Default::default()
        }
    }

    #[test]
    fn inspection_recognizes_both_current_firmware_variants_and_requires_readback() {
        use super::*;
        for wiring in [Wiring::Standard, Wiring::Mistake] {
            let status = installed(wiring.firmware_version(), Some(wiring.pin_profile()));
            let inspected = installed_inspection("serial:board", &status);
            assert_eq!(inspected.state, "current");
            assert_eq!(inspected.wiring, Some(wiring));
            assert!(inspected.ready_to_verify);
            for incomplete in [
                connection::ConnectionSnapshot {
                    identity: None,
                    ..status.clone()
                },
                connection::ConnectionSnapshot {
                    display_health: Some("fault".into()),
                    ..status.clone()
                },
                connection::ConnectionSnapshot {
                    standalone_count: 2,
                    ..status.clone()
                },
                connection::ConnectionSnapshot {
                    standalone_content_crc: None,
                    ..status.clone()
                },
                connection::ConnectionSnapshot {
                    provisioning_supported: false,
                    ..status.clone()
                },
            ] {
                assert!(!installed_inspection("serial:board", &incomplete).ready_to_verify);
            }
        }
    }

    #[test]
    fn inspection_never_calls_unknown_or_newer_firmware_an_update() {
        use super::*;
        assert_eq!(
            installed_inspection("x", &installed("1.2.9", Some("GP4-GP5"))).state,
            "update_available"
        );
        assert_eq!(
            installed_inspection("x", &installed("1.3.1-mistake", Some("GP5-SDA-GP4-SCL"))).state,
            "update_available"
        );
        assert_eq!(
            installed_inspection("x", &installed("1.10.0", Some("GP4-GP5"))).state,
            "newer"
        );
        for version in ["", "custom", "1.3.2-beta", "1.3.2.0"] {
            assert_eq!(
                installed_inspection("x", &installed(version, Some("GP4-GP5"))).state,
                "unknown"
            );
        }
        assert_eq!(
            installed_inspection("x", &installed("1.3.2", Some("GP5-SDA-GP4-SCL"))).state,
            "unknown"
        );
        let missing = installed_inspection("x", &installed("1.3.2-mistake", None));
        assert_eq!(missing.state, "unknown");
        assert_eq!(missing.wiring, None);
        assert!(!missing.ready_to_verify);
    }

    #[test]
    fn wiring_selection_requires_exact_firmware_and_reported_pins() {
        use super::*;
        assert!(matches!(Wiring::default(), Wiring::Standard));
        assert!(Wiring::parse("other").is_err());
        for wiring in [Wiring::Standard, Wiring::Mistake] {
            let status = connection::ConnectionSnapshot {
                board_id: Some("5303284728CB1C9C".into()),
                firmware: Some(wiring.firmware_version().into()),
                pin_profile: Some(wiring.pin_profile().into()),
                ..Default::default()
            };
            assert!(check_firmware(&status, "5303284728cb1c9c", wiring).is_ok());
            let other = if wiring == Wiring::Standard {
                Wiring::Mistake
            } else {
                Wiring::Standard
            };
            assert!(check_firmware(&status, "5303284728CB1C9C", other).is_err());
            assert!(check_firmware(&status, "0000000000000000", wiring).is_err());
            assert!(check_firmware(
                &connection::ConnectionSnapshot {
                    pin_profile: Some(other.pin_profile().into()),
                    ..status.clone()
                },
                "5303284728CB1C9C",
                wiring
            )
            .is_err());
            let missing = connection::ConnectionSnapshot {
                pin_profile: None,
                ..status
            };
            assert_eq!(
                check_firmware(&missing, "5303284728CB1C9C", wiring).is_ok(),
                wiring == Wiring::Standard
            );
        }
    }

    #[test]
    fn programming_only_accepts_the_current_lineup() {
        assert!(super::current_model("meep"));
        assert!(super::current_model("claw"));
        assert!(super::current_model("boo"));
        assert!(super::current_model("nimbus"));
        assert!(!super::current_model(""));
    }

    use super::*;
    fn board(id: &str) -> Board {
        Board {
            id: id.into(),
            name: id.into(),
            kind: "setup".into(),
            serial: None,
        }
    }
    #[test]
    fn programming_needs_one_current_exact_target() {
        assert!(one_board(&[], "setup:a").is_err());
        assert!(one_board(&[board("setup:a"), board("setup:b")], "setup:a").is_err());
        assert!(one_board(&[board("setup:b")], "setup:a").is_err());
        assert!(one_board(&[board("setup:a")], "setup:a").is_ok());
    }
    #[test]
    fn consumer_process_guard_recognizes_packaged_and_legacy_apps() {
        assert!(consumer_name("deskbot", None));
        assert!(consumer_name("Deskbot.exe", None));
        assert!(consumer_name("droidfun-app", None));
        assert!(consumer_name("DroidFun-App.exe", None));
        assert!(consumer_name(
            "renamed",
            Some("/Applications/DroidFun App.app/Contents/MacOS/renamed")
        ));
        assert!(!consumer_name(
            "droidfun-workshop",
            Some("/Applications/DroidFun Workshop.app/Contents/MacOS/droidfun-workshop")
        ));
    }
    #[test]
    fn unplug_evidence_requires_current_job_and_no_runtime_or_setup_board() {
        let waiting = Status {
            job: 2,
            phase: "awaiting_disconnect".into(),
            ..Status::default()
        };
        assert!(observed_unplug(&waiting, &waiting, &[]));
        assert!(!observed_unplug(&waiting, &waiting, &[board("setup:a")]));
        assert!(!observed_unplug(&waiting, &waiting, &[board("serial:a")]));
        assert!(!observed_unplug(
            &Status {
                job: 1,
                ..waiting.clone()
            },
            &waiting,
            &[]
        ));
        assert!(!observed_unplug(
            &Status {
                phase: "idle".into(),
                ..waiting.clone()
            },
            &waiting,
            &[]
        ));
        assert!(!observed_unplug(
            &waiting,
            &Status {
                busy: true,
                ..waiting.clone()
            },
            &[]
        ));
    }
    #[test]
    fn reconnect_requires_all_saved_identity_fields() {
        let expected = DeviceIdentity {
            model: "meep".into(),
            hardware_profile: PROFILE.into(),
            generation: 4,
        };
        assert!(check_identity(None, &expected).is_err());
        assert!(check_identity(Some(&expected), &expected).is_ok());
        for actual in [
            DeviceIdentity {
                model: "boo".into(),
                ..expected.clone()
            },
            DeviceIdentity {
                hardware_profile: "other".into(),
                ..expected.clone()
            },
            DeviceIdentity {
                generation: 3,
                ..expected.clone()
            },
        ] {
            assert!(check_identity(Some(&actual), &expected).is_err());
        }
    }
    #[test]
    fn old_one_screen_playlist_cannot_pass_startup_face_readback() {
        let mut status = connection::ConnectionSnapshot {
            standalone_saved: true,
            standalone_count: 1,
            ..connection::ConnectionSnapshot::default()
        };
        assert!(check_saved_face(&status, 123).is_err());
        status.standalone_content_crc = Some(122);
        assert!(check_saved_face(&status, 123).is_err());
        status.standalone_content_crc = Some(123);
        assert!(check_saved_face(&status, 123).is_ok());
        status.standalone_count = 2;
        assert!(check_saved_face(&status, 123).is_err());
    }
}
