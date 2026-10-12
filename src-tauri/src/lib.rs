pub mod claude_account;
pub mod claude_bridge;
mod connection;
mod core;
mod local_widgets;
mod network;
mod persistence;
mod platform;
mod protocol;
mod providers;
mod reliability;

use connection::{ConnectionSnapshot, Link, RobotPort};
use core::{Engine, Location, Preferences, Settings, Snapshot, SourceConfig};
use providers::{Providers, UsageWindow};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_dialog::DialogExt;

struct AppState {
    engine: Mutex<Engine>,
    connection: Mutex<ConnectionSnapshot>,
    settings_path: PathBuf,
    started: Instant,
    platform: platform::PlatformInfo,
    diagnostics: Mutex<Option<String>>,
    events: Mutex<std::collections::VecDeque<serde_json::Value>>,
    pending: Mutex<
        std::collections::HashMap<String, Vec<std::sync::Weak<std::sync::atomic::AtomicBool>>>,
    >,
    providers: Arc<Providers>,
    link_metrics: Mutex<connection::LinkMetrics>,
}

fn publish(app: &AppHandle, snapshot: &Snapshot) {
    reliability::transition(snapshot);
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.emit("deskbot-state", snapshot);
        }
    }
}

fn publish_connection(app: &AppHandle, next: ConnectionSnapshot) {
    let state = app.state::<AppState>();
    if let Ok(mut current) = state.connection.lock() {
        if *current != next {
            *current = next.clone();
            if let Ok(mut events) = state.events.lock() {
                if events.len() >= 100 {
                    events.pop_front();
                }
                events.push_back(serde_json::json!({"at":core::unix_now(),"state":next.state,"displayHealth":next.display_health}));
            }
            drop(current);
            let _ = app.emit("deskbot-connection", next);
        }
    };
}

fn mutate<F>(
    app: &AppHandle,
    state: State<AppState>,
    save: bool,
    operation: F,
) -> Result<Snapshot, String>
where
    F: FnOnce(&mut Engine) -> Result<(), String>,
{
    let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
    let previous = engine.clone();
    if let Err(error) = operation(&mut engine) {
        *engine = previous;
        return Err(error);
    }
    if save {
        if let Err(error) = persistence::save(&state.settings_path, &engine.settings) {
            *engine = previous;
            return Err(format!("Could not save settings: {error}"));
        }
    }
    if let Ok(mut pending) = state.pending.lock() {
        for widget in &previous.settings.widgets {
            if !engine.settings.widgets.iter().any(|w| {
                w.id == widget.id && w.enabled == widget.enabled && w.config == widget.config
            }) {
                if let Some(requests) = pending.remove(&widget.id) {
                    for flag in requests.into_iter().filter_map(|w| w.upgrade()) {
                        flag.store(true, std::sync::atomic::Ordering::Release);
                    }
                }
            }
        }
    }
    let snapshot = engine.snapshot();
    drop(engine);
    publish(app, &snapshot);
    Ok(snapshot)
}

fn register_request(
    state: &AppState,
    id: &str,
) -> Result<Arc<std::sync::atomic::AtomicBool>, String> {
    let mut pending = state
        .pending
        .lock()
        .map_err(|_| "Request state unavailable")?;
    pending.retain(|_, flags| {
        flags.retain(|f| f.strong_count() > 0);
        !flags.is_empty()
    });
    if pending.values().map(Vec::len).sum::<usize>() >= 64 {
        return Err("Provider requests are busy; try again shortly".into());
    }
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    pending
        .entry(id.into())
        .or_default()
        .push(Arc::downgrade(&flag));
    Ok(flag)
}

#[tauri::command]
fn get_state(state: State<AppState>) -> Result<Snapshot, String> {
    Ok(state.engine.lock().map_err(|e| e.to_string())?.snapshot())
}

#[tauri::command]
fn get_connection(state: State<AppState>) -> Result<ConnectionSnapshot, String> {
    Ok(state.connection.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn list_robot_ports() -> Result<Vec<RobotPort>, String> {
    connection::robot_ports()
}

#[tauri::command]
fn select_robot(
    serial: Option<String>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    if let Some(ref value) = serial {
        if !connection::robot_ports()?
            .iter()
            .any(|port| port.serial == *value)
        {
            return Err("That RP2040-Zero is not currently detected".into());
        }
    }
    mutate(&app, state, true, |engine| {
        engine.settings.selected_device_serial = serial;
        engine.settings.validate()
    })
}

#[tauri::command]
fn preview_frame(id: String, state: State<AppState>) -> Result<Vec<u8>, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?;
    let widget = engine
        .settings
        .widgets
        .iter()
        .find(|w| w.id == id)
        .ok_or("Widget not found")?;
    Ok(engine.render_widget(&widget.id))
}

#[tauri::command]
fn configure_source(
    id: String,
    label: String,
    config: SourceConfig,
    headline: String,
    footer: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| {
        e.configure(&id, label.clone(), headline, footer)?;
        e.configure_source(&id, label, config)
    })
}
#[tauri::command]
fn add_widget(kind: String, app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| e.add_widget(&kind))
}
#[tauri::command]
fn remove_widget(id: String, app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| e.remove_widget(&id))
}
#[tauri::command]
async fn search_weather(name: String, state: State<'_, AppState>) -> Result<Vec<Location>, String> {
    let providers = state.providers.clone();
    tauri::async_runtime::spawn_blocking(move || providers.search_locations(&name))
        .await
        .map_err(|_| "Location search stopped".to_string())?
}
#[tauri::command]
async fn codex_windows(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<UsageWindow>, String> {
    let providers = state.providers.clone();
    tauri::async_runtime::spawn_blocking(move || providers.codex_windows(&path))
        .await
        .map_err(|_| "Account read stopped".to_string())?
}
#[tauri::command]
async fn source_info(
    profile: Option<String>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let bridge = state
        .settings_path
        .parent()
        .ok_or("Settings directory unavailable")?
        .join("claude-bridge");
    tauri::async_runtime::spawn_blocking(move || Ok(serde_json::json!({"claudeBridge":claude_bridge::status(&bridge)?,"claudeAccount":claude_account::status(profile.as_deref().unwrap_or(""))}))).await.map_err(|_| "Source setup stopped".to_string())?
}
#[tauri::command]
async fn connect_claude_account(
    profile: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let providers = state.providers.clone();
    tauri::async_runtime::spawn_blocking(move || {
        claude_account::login(profile.as_deref().unwrap_or(""))?;
        providers.invalidate_claude();
        Ok(())
    })
    .await
    .map_err(|_| "Claude connection stopped".to_string())?
}
#[tauri::command]
async fn set_claude_bridge(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let bridge = state
        .settings_path
        .parent()
        .ok_or("Settings directory unavailable")?
        .join("claude-bridge");
    let report_dir = state
        .settings_path
        .parent()
        .unwrap()
        .join("providers/claude/default");
    tauri::async_runtime::spawn_blocking(move || {
        if enabled {
            claude_bridge::install(&bridge, &report_dir)
        } else {
            claude_bridge::uninstall(&bridge)
        }
    })
    .await
    .map_err(|_| "Bridge setup stopped".to_string())?
}
#[tauri::command]
async fn refresh_widget(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Snapshot, String> {
    let widget = state
        .engine
        .lock()
        .map_err(|e| e.to_string())?
        .settings
        .widgets
        .iter()
        .find(|w| w.id == id)
        .cloned()
        .ok_or("Widget not found")?;
    if widget.config.mode != "live" {
        return Err("Choose Live data in Edit first".into());
    }
    let providers = state.providers.clone();
    let requested_config = widget.config.clone();
    providers.retry_authentication(&widget);
    let cancel = register_request(&state, &id)?;
    let request_flag = cancel.clone();
    let card = tauri::async_runtime::spawn_blocking(move || {
        providers.fetch_cancelable(&widget, request_flag)
    })
    .await
    .map_err(|_| "Refresh stopped".to_string())?;
    mutate(&app, state, false, |e| {
        if !cancel.load(std::sync::atomic::Ordering::Acquire)
            && e.settings
                .widgets
                .iter()
                .any(|w| w.id == id && w.config == requested_config)
        {
            e.set_data(&id, card);
        }
        Ok(())
    })
}

#[tauri::command]
fn set_enabled(
    id: String,
    enabled: bool,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| e.set_enabled(&id, enabled))
}
#[tauri::command]
fn move_widget(
    id: String,
    direction: i8,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| e.move_widget(&id, direction))
}
#[tauri::command]
fn set_duration(
    id: String,
    duration_sec: u16,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| e.set_duration(&id, duration_sec))
}
#[tauri::command]
fn configure_widget(
    id: String,
    label: String,
    headline: String,
    footer: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| {
        e.configure(&id, label, headline, footer)
    })
}
#[tauri::command]
fn set_brightness(
    brightness: u8,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| e.set_brightness(brightness))
}
#[tauri::command]
fn set_display_on(on: bool, app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| {
        e.set_display_on(on);
        Ok(())
    })
}
#[tauri::command]
fn toggle_pause(app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, false, |e| {
        e.toggle_pause();
        Ok(())
    })
}
#[tauri::command]
fn next_widget(app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, false, |e| {
        e.next();
        Ok(())
    })
}
#[tauri::command]
fn show_now(id: String, app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, false, |e| e.show_now(&id))
}
#[tauri::command]
fn pin_widget(id: String, app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, false, |e| e.pin(&id))
}
#[tauri::command]
fn show_face(app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, false, |e| {
        e.show_face();
        Ok(())
    })
}

#[tauri::command]
fn timer_action(
    id: String,
    action: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| {
        e.timer_action_at(&id, &action, core::unix_now())
    })
}
#[tauri::command]
fn time_zones() -> Vec<String> {
    chrono_tz::TZ_VARIANTS
        .iter()
        .map(|tz| tz.name().to_string())
        .collect()
}

fn monitor_worker(app: AppHandle) {
    let mut system = sysinfo::System::new();
    let mut warmed = false;
    let mut last: std::collections::HashMap<String, (Instant, SourceConfig)> =
        std::collections::HashMap::new();
    let mut history: std::collections::HashMap<String, std::collections::VecDeque<(u64, f32)>> =
        std::collections::HashMap::new();
    loop {
        let state = app.state::<AppState>();
        let widgets = state
            .engine
            .lock()
            .map(|e| {
                e.settings
                    .widgets
                    .iter()
                    .filter(|w| w.enabled && matches!(w.kind.as_str(), "cpu" | "memory"))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let ids: std::collections::HashSet<_> = widgets.iter().map(|w| w.id.clone()).collect();
        last.retain(|id, _| ids.contains(id));
        history.retain(|id, _| ids.contains(id));
        if widgets.is_empty() {
            warmed = false;
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        if !widgets.iter().any(|w| w.kind == "cpu") {
            warmed = false;
        }
        let due: Vec<_> = widgets
            .into_iter()
            .filter(|w| {
                last.get(&w.id).is_none_or(|(at, config)| {
                    config != &w.config
                        || at.elapsed() >= Duration::from_secs(w.config.monitor_interval as u64)
                })
            })
            .collect();
        let wants_cpu = due.iter().any(|w| w.kind == "cpu");
        if wants_cpu {
            system.refresh_cpu_usage();
        }
        if !warmed && wants_cpu {
            warmed = true;
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        if due.iter().any(|w| w.kind == "memory") {
            system.refresh_memory();
        }
        for w in due {
            let wall = core::unix_now();
            let card = if w.kind == "cpu" {
                if !sysinfo::IS_SUPPORTED_SYSTEM || system.cpus().is_empty() {
                    continue;
                }
                let percent = system.global_cpu_usage();
                if !percent.is_finite() {
                    continue;
                }
                let values = history.entry(w.id.clone()).or_default();
                values.push_back((wall, percent));
                while values
                    .front()
                    .is_some_and(|(at, _)| wall.saturating_sub(*at) > 60)
                    || values.len() > 61
                {
                    values.pop_front();
                }
                local_widgets::cpu_card(
                    &w,
                    wall,
                    percent,
                    &values.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
                )
            } else {
                let total = system.total_memory();
                if total == 0 {
                    continue;
                }
                local_widgets::memory_card(
                    &w,
                    wall,
                    system.used_memory(),
                    system.available_memory(),
                    total,
                )
            };
            last.insert(w.id.clone(), (Instant::now(), w.config.clone()));
            let snapshot = if let Ok(mut e) = state.engine.lock() {
                if e.settings.widgets.iter().any(|current| {
                    current.id == w.id && current.enabled && current.config == w.config
                }) {
                    e.set_data(&w.id, card);
                    Some(e.snapshot())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(snapshot) = snapshot {
                publish(&app, &snapshot);
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[tauri::command]
fn platform_info(state: State<AppState>, app: AppHandle) -> serde_json::Value {
    serde_json::json!({"os":state.platform.os,"trayAvailable":state.platform.tray_available,"explanation":state.platform.explanation,"launchAtLogin":app.autolaunch().is_enabled().unwrap_or(false)})
}
#[tauri::command]
fn set_preferences(
    preferences: Preferences,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    preferences.validate()?;
    let old = app
        .autolaunch()
        .is_enabled()
        .map_err(|_| "Could not read startup registration")?;
    if old != preferences.launch_at_login {
        if preferences.launch_at_login {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        }
        .map_err(|_| "Could not update start at login")?;
    }
    let result = mutate(&app, state, true, |e| e.set_preferences(preferences));
    if result.is_err() {
        let _ = if old {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
    }
    result
}
#[tauri::command]
fn override_quiet(app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, false, |e| {
        e.override_quiet();
        Ok(())
    })
}
#[tauri::command]
fn set_reduced_motion(
    reduced: bool,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, false, |e| {
        e.set_reduced_motion(reduced);
        Ok(())
    })
}
#[tauri::command]
fn reorder_widget(
    id: String,
    before: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| e.reorder_widget(&id, &before))
}
#[tauri::command]
fn reset_deskbot(app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    let before = state
        .settings_path
        .with_extension(format!("before-reset-{}.json", core::unix_now()));
    if state.settings_path.exists() {
        fs::copy(&state.settings_path, before)
            .map_err(|_| "Could not preserve your settings before reset")?;
    }
    let startup = app
        .autolaunch()
        .is_enabled()
        .map_err(|_| "Could not read startup registration")?;
    if startup {
        app.autolaunch()
            .disable()
            .map_err(|_| "Could not remove start at login")?;
    }
    let result = mutate(&app, state, true, |e| {
        *e = Engine::new(Settings::default(), Instant::now());
        Ok(())
    });
    if result.is_err() && startup {
        let _ = app.autolaunch().enable();
    }
    result
}
#[tauri::command]
fn diagnostics_preview(state: State<AppState>) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let e = state.engine.lock().map_err(|_| "State unavailable")?;
    let c = state
        .connection
        .lock()
        .map_err(|_| "Connection unavailable")?;
    let widgets:Vec<_>=e.settings.widgets.iter().map(|w|serde_json::json!({"kind":w.kind,"enabled":w.enabled,"mode":w.config.mode,"durationSeconds":w.duration_sec})).collect();
    let cards:Vec<_>=e.snapshot().data.values().map(|d|serde_json::json!({"source":d.source_id,"state":d.state,"fetchedAt":d.fetched_at,"observedAt":d.observed_at,"expiresAt":d.expires_at})).collect();
    let events = state
        .events
        .lock()
        .map_err(|_| "Event history unavailable")?;
    let report=serde_json::to_string_pretty(&serde_json::json!({"formatVersion":1,"appVersion":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"uptimeSeconds":state.started.elapsed().as_secs(),"trayAvailable":state.platform.tray_available,"connection":{"state":c.state,"firmware":c.firmware,"displayHealth":c.display_health,"redactedBoard":c.board_id.as_ref().map(|id|format!("{:x}",Sha256::digest(id.as_bytes()))[..8].to_string())},"display":{"brightness":e.settings.brightness,"orientation":e.settings.preferences.orientation,"reason":e.snapshot().display_reason},"widgets":widgets,"providers":cards,"connectionEvents":*events,"excluded":"credentials, account identity, location, symbols, prompts, transcripts, local paths"})).map_err(|_|"Could not prepare diagnostics")?;
    *state
        .diagnostics
        .lock()
        .map_err(|_| "Diagnostics unavailable")? = Some(report.clone());
    Ok(report)
}
#[tauri::command]
async fn export_firmware(app: AppHandle) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .dialog()
            .file()
            .set_file_name("deskbot-rp2040-zero-gp4-gp5-v1.1.0.uf2")
            .add_filter("RP2040 firmware", &["uf2"])
            .blocking_save_file();
        let Some(path) = path else {
            return Ok(false);
        };
        fs::write(
            path.into_path().map_err(|_| "Choose a local file")?,
            include_bytes!("../../firmware/releases/deskbot-rp2040-zero-gp4-gp5-v1.1.0.uf2"),
        )
        .map_err(|_| "Could not save firmware")?;
        Ok(true)
    })
    .await
    .map_err(|_| "Firmware export stopped".to_string())?
}
#[tauri::command]
async fn export_diagnostics(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let report = state
        .diagnostics
        .lock()
        .map_err(|_| "Diagnostics unavailable")?
        .clone()
        .ok_or("Preview diagnostics before saving")?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = app
            .dialog()
            .file()
            .set_file_name("Deskbot-diagnostics.json")
            .add_filter("JSON", &["json"])
            .blocking_save_file();
        let Some(path) = path else {
            return Ok(false);
        };
        let path = path
            .into_path()
            .map_err(|_| "Choose a local diagnostics file")?;
        fs::write(path, report).map_err(|_| "Could not save diagnostics")?;
        Ok(true)
    })
    .await
    .map_err(|_| "Diagnostics export stopped".to_string())?
}

fn show_window(app: &AppHandle) {
    platform::show(app, false);
}

fn tray_action(app: &AppHandle, id: &str) {
    match id {
        "open" => show_window(app),
        "settings" => {
            platform::show_settings(app);
        }
        "quit" => app.exit(0),
        _ => {
            let state = app.state::<AppState>();
            let _ = mutate(app, state, id == "display", |engine| {
                match id {
                    "pause" => engine.toggle_pause(),
                    "next" => engine.next(),
                    "face" => engine.show_face(),
                    "display" => engine.set_display_on(engine.settings.manual_display_off),
                    _ => return Err("Unknown tray action".into()),
                }
                Ok(())
            });
        }
    }
}

fn connection_worker(app: AppHandle) {
    let mut backoff = 0;
    loop {
        if reliability::fault("link-interruption") {
            publish_connection(
                &app,
                ConnectionSnapshot {
                    state: "reconnecting".into(),
                    detail: "Reliability test: host link temporarily interrupted".into(),
                    ..ConnectionSnapshot::default()
                },
            );
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        let selected = {
            let state = app.state::<AppState>();
            state
                .engine
                .lock()
                .ok()
                .and_then(|engine| engine.settings.selected_device_serial.clone())
        };
        let Some(serial) = selected else {
            let detected = connection::robot_ports().unwrap_or_default().len();
            publish_connection(
                &app,
                ConnectionSnapshot {
                    detail: if detected > 0 {
                        "RP2040-Zero detected. Choose Connect robot to check its sketch.".into()
                    } else {
                        "No RP2040-Zero USB serial device detected".into()
                    },
                    ..ConnectionSnapshot::default()
                },
            );
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let candidate = connection::robot_ports()
            .ok()
            .and_then(|ports| ports.into_iter().find(|port| port.serial == serial));
        let Some(candidate) = candidate else {
            publish_connection(
                &app,
                ConnectionSnapshot {
                    state: "reconnecting".into(),
                    detail: "Selected robot is not present on USB".into(),
                    ..ConnectionSnapshot::default()
                },
            );
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        publish_connection(
            &app,
            ConnectionSnapshot {
                state: "connecting".into(),
                detail: "Checking the Deskbot firmware handshake".into(),
                port: Some(candidate.path.clone()),
                ..ConnectionSnapshot::default()
            },
        );
        match Link::connect(&candidate) {
            Ok(mut link) => {
                backoff = 0;
                publish_connection(&app, link.status.clone());
                loop {
                    if reliability::fault("link-interruption") {
                        break;
                    }
                    let current = {
                        let state = app.state::<AppState>();
                        state.engine.lock().ok().map(|engine| {
                            (
                                engine.settings.selected_device_serial.clone(),
                                engine.snapshot(),
                            )
                        })
                    };
                    let Some((selected_now, snapshot)) = current else {
                        break;
                    };
                    if selected_now.as_deref() != Some(serial.as_str()) {
                        break;
                    }
                    match link.update(
                        &snapshot.frame,
                        snapshot.display_on,
                        snapshot.settings.brightness,
                        snapshot.settings.preferences.orientation,
                    ) {
                        Ok(changed) => {
                            if changed {
                                publish_connection(&app, link.status.clone());
                            }
                        }
                        Err(error) => {
                            publish_connection(
                                &app,
                                ConnectionSnapshot {
                                    state: if error.contains("i2c_error") {
                                        "display_error"
                                    } else {
                                        "reconnecting"
                                    }
                                    .into(),
                                    detail: error,
                                    port: Some(candidate.path.clone()),
                                    board_id: link.status.board_id.clone(),
                                    firmware: link.status.firmware.clone(),
                                    display_health: link.status.display_health.clone(),
                                },
                            );
                            break;
                        }
                    }
                    if let Ok(mut metrics) = app.state::<AppState>().link_metrics.lock() {
                        *metrics = link.metrics.clone();
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            Err(error) => {
                let setup = error.contains("No Deskbot reply")
                    || error.contains("invalid Deskbot handshake");
                publish_connection(
                    &app,
                    ConnectionSnapshot {
                        state: if setup {
                            "setup_needed"
                        } else {
                            "reconnecting"
                        }
                        .into(),
                        detail: error,
                        port: Some(candidate.path),
                        ..ConnectionSnapshot::default()
                    },
                );
                if setup {
                    std::thread::sleep(Duration::from_secs(10));
                    continue;
                }
            }
        }
        let delays = [1, 2, 5, 10];
        std::thread::sleep(Duration::from_secs(delays[backoff]));
        backoff = (backoff + 1).min(delays.len() - 1);
    }
}

fn provider_worker(app: AppHandle, kind: &str) {
    let mut last_active = Instant::now();
    loop {
        let state = app.state::<AppState>();
        let visible = app
            .get_webview_window("main")
            .is_some_and(|w| w.is_visible().unwrap_or(false));
        let connected = state
            .connection
            .lock()
            .map(|c| c.state == "connected")
            .unwrap_or(false);
        if visible || connected {
            last_active = Instant::now();
        }
        if last_active.elapsed() < Duration::from_secs(30) {
            let widgets = state
                .engine
                .lock()
                .map(|e| e.settings.widgets.clone())
                .unwrap_or_default();
            for widget in widgets
                .into_iter()
                .filter(|w| w.enabled && w.config.mode == "live" && w.kind == kind)
            {
                let Ok(cancel) = register_request(&state, &widget.id) else {
                    continue;
                };
                let card = state.providers.fetch_cancelable(&widget, cancel.clone());
                if cancel.load(std::sync::atomic::Ordering::Acquire) {
                    continue;
                }
                let snapshot = if let Ok(mut engine) = state.engine.lock() {
                    // Ignore a response if the user changed configuration during its request.
                    if engine
                        .settings
                        .widgets
                        .iter()
                        .any(|w| w.id == widget.id && w.enabled && w.config == widget.config)
                    {
                        engine.set_data(&widget.id, card);
                        Some(engine.snapshot())
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(snapshot) = snapshot {
                    publish(&app, &snapshot);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_window(app)
        }))
        .setup(|app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            // An already enabled integration follows app updates without changing Claude settings.
            let bridge = app.path().app_config_dir()?.join("claude-bridge");
            std::thread::spawn(move || {
                let _ = claude_bridge::refresh_installed_helper(&bridge);
            });
            let settings = persistence::load(&settings_path).map_err(std::io::Error::other)?;
            let background = std::env::args().any(|a| a == "--background");
            let mut engine = Engine::new(settings, Instant::now());
            engine.tick(Instant::now());
            let providers = Arc::new(
                Providers::new(app.path().app_config_dir()?.join("providers"))
                    .map_err(std::io::Error::other)?,
            );
            engine.restore_data(providers.startup_cards(&engine.settings.widgets));
            if reliability::fault("static-face") {
                if let Some(id) = engine.settings.widgets.iter().find(|w|w.kind=="face").map(|w|w.id.clone()) {
                    engine.pin(&id).map_err(std::io::Error::other)?;
                }
            }
            app.manage(AppState {
                engine: Mutex::new(engine),
                connection: Mutex::new(ConnectionSnapshot::default()),
                settings_path,
                providers,
                link_metrics: Mutex::new(connection::LinkMetrics::default()),
                started: Instant::now(),
                platform: platform::info(),
                diagnostics: Mutex::new(None),
                events: Mutex::new(std::collections::VecDeque::new()),
                pending: Mutex::new(std::collections::HashMap::new()),
            });
            if let Some(window) = app.get_webview_window("main") {
                window.set_size(tauri::LogicalSize::new(440.0, 760.0))?;
            }

            let open = MenuItem::with_id(app, "open", "Open Deskbot", true, None::<&str>)?;
            let pause =
                MenuItem::with_id(app, "pause", "Pause or resume cycle", true, None::<&str>)?;
            let next = MenuItem::with_id(app, "next", "Next widget", true, None::<&str>)?;
            let face = MenuItem::with_id(app, "face", "Show face", true, None::<&str>)?;
            let display =
                MenuItem::with_id(app, "display", "Display on or off", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Deskbot", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&open, &pause, &next, &face, &display, &settings, &quit],
            )?;
            let tray = TrayIconBuilder::with_id("deskbot")
                .menu(&menu)
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray.png"
                ))?)
                .icon_as_template(cfg!(target_os = "macos"))
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| tray_action(app, event.id.as_ref()))
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        platform::show(tray.app_handle(), true);
                    }
                });
            tray.build(app)?;
            let state = app.state::<AppState>();
            let prefs = state.engine.lock().unwrap().settings.preferences.clone();
            if background
                && (state.platform.tray_available || prefs.window_mode == "background")
                && prefs.window_mode != "window"
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.destroy();
                }
            } else {
                platform::show(app.handle(), false);
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(200));
                let state = handle.state::<AppState>();
                let snapshot = {
                    let Ok(mut engine) = state.engine.lock() else {
                        continue;
                    };
                    if engine.tick(Instant::now()) {
                        Some(engine.snapshot())
                    } else {
                        None
                    }
                };
                if let Some(snapshot) = snapshot {
                    publish(&handle, &snapshot);
                }
            });
            let connection_handle = app.handle().clone();
            std::thread::spawn(move || connection_worker(connection_handle));
            for kind in ["weather", "stock", "codex", "claude"] {
                let provider_handle = app.handle().clone();
                std::thread::spawn(move || provider_worker(provider_handle, kind));
            }
            let monitor_handle=app.handle().clone();
            std::thread::spawn(move||monitor_worker(monitor_handle));
            if reliability::directory().is_some() {
                let handle = app.handle().clone();
                std::thread::spawn(move || loop {
                    let state = handle.state::<AppState>();
                    let snapshot = state.engine.lock().ok().map(|e| e.snapshot());
                    if let Some(s) = snapshot {
                        let now = core::unix_now();
                        let widgets: Vec<_> = s.settings.widgets.iter().enumerate().map(|(index,w)| serde_json::json!({"index":index,"kind":w.kind,"enabled":w.enabled,"mode":w.config.mode,"durationSeconds":w.duration_sec,"current":w.id==s.current_id,"dataState":s.data.get(&w.id).map(|d|d.state_at(now)),"fetchedAt":s.data.get(&w.id).map(|d|d.fetched_at),"expiresAt":s.data.get(&w.id).map(|d|d.expires_at)})).collect();
                        let connection = state.connection.lock().ok().map(|c|serde_json::json!({"state":c.state,"firmware":c.firmware,"displayHealth":c.display_health}));
                        let metrics = state.link_metrics.lock().ok().map(|m|serde_json::to_value(&*m).unwrap_or_default());
                        let pending = state.pending.lock().ok().map(|p|p.values().map(|v|v.iter().filter(|w|w.strong_count()>0).count()).sum::<usize>());
                        let events = state.events.lock().ok().map(|e|e.clone());
                        let visible = handle.get_webview_window("main").is_some_and(|w|w.is_visible().unwrap_or(false));
                        let value = serde_json::json!({"formatVersion":1,"at":now,"pid":std::process::id(),"uptimeSeconds":state.started.elapsed().as_secs(),"controlsVisible":visible,"connection":connection,"link":metrics,"playback":s.playback,"displayOn":s.display_on,"displayReason":s.display_reason,"orientation":s.settings.preferences.orientation,"brightness":s.settings.brightness,"frameCrc":protocol::crc32(&s.frame),"widgets":widgets,"pendingRequests":pending,"connectionEvents":events,"faults":{"networkOffline":reliability::fault("network-offline"),"linkInterruption":reliability::fault("link-interruption")}});
                        if let Err(error) = reliability::write(&value) { eprintln!("Reliability telemetry could not be written: {error}"); }
                    }
                    std::thread::sleep(Duration::from_secs(5));
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                let mode = state
                    .engine
                    .lock()
                    .map(|e| e.settings.preferences.window_mode.clone())
                    .unwrap_or_else(|_| "window".into());
                if mode == "window" || (!state.platform.tray_available && mode != "background") {
                    let _ = window.minimize();
                } else {
                    // The engine, USB link and providers live outside this
                    // panel. Reopening restores their current state.
                    let _ = window.destroy();
                }
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            set_preferences,
            platform_info,
            override_quiet,
            set_reduced_motion,
            reorder_widget,
            reset_deskbot,
            diagnostics_preview,
            export_diagnostics,
            export_firmware,
            configure_source,
            add_widget,
            remove_widget,
            search_weather,
            codex_windows,
            source_info,
            set_claude_bridge,
            connect_claude_account,
            refresh_widget,
            get_state,
            get_connection,
            list_robot_ports,
            select_robot,
            preview_frame,
            set_enabled,
            move_widget,
            set_duration,
            configure_widget,
            set_brightness,
            set_display_on,
            toggle_pause,
            next_widget,
            show_now,
            pin_widget,
            show_face
            ,timer_action,time_zones
        ])
        .build(tauri::generate_context!())
        .expect("Deskbot could not start")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                let state = app.state::<AppState>();
                if state.platform.tray_available {
                    // Last control window closed; explicit Quit uses exit(0).
                    api.prevent_exit();
                }
            }
        });
}
