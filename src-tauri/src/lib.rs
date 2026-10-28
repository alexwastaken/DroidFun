mod app_update;
pub mod claude_account;
pub mod claude_bridge;
mod connection;
mod core;
mod display_test;
mod droids;
mod droid_registry;
mod firmware_setup;
mod host_display;
pub mod linux_usb;
mod local_widgets;
mod network;
mod onboarding;
mod persistence;
mod picoboot;
mod platform;
mod protocol;
mod providers;
mod reliability;
mod support;
#[cfg(windows)]
mod windows_claude_terminal;

use connection::{ConnectionSnapshot, Link, RobotPort};
use core::{Engine, Location, Preferences, Settings, Snapshot, SourceConfig, Widget};
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
    support_report: Mutex<Option<support::Report>>,
    support_pattern: Mutex<Option<support::Pattern>>,
    events: Mutex<std::collections::VecDeque<serde_json::Value>>,
    pending: Mutex<
        std::collections::HashMap<String, Vec<std::sync::Weak<std::sync::atomic::AtomicBool>>>,
    >,
    providers: Arc<Providers>,
    link_metrics: Mutex<connection::LinkMetrics>,
    standalone: Mutex<Option<StandaloneRequest>>,
    save_generation: std::sync::atomic::AtomicU64,
}
struct StandaloneRequest {
    // The request owns the lock while queued and throughout the USB flash
    // transaction, even if its caller times out or closes the webview.
    _operation: Arc<UsbOperationGuard>,
    generation: u64,
    settings: Vec<u8>,
    serial: String,
    playlist: protocol::StandalonePlaylist,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    response: std::sync::mpsc::SyncSender<Result<usize, String>>,
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
    if next.state != "connected" {
        if let Ok(mut pending) = state.standalone.lock() {
            if let Some(request) = pending.take() {
                let _ = request.response.send(Err(
                    "The bot disconnected before saving. Reconnect it and try again.".into(),
                ));
            }
        }
    }
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
        if let Err(error) = droids::remember(&mut engine.settings) {
            *engine = previous;
            return Err(error);
        }
        if let Err(error) = persistence::save(&state.settings_path, &engine.settings) {
            *engine = previous;
            return Err(format!("Could not save settings: {error}"));
        }
    }
    if save {
        state
            .save_generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }
    // Update while holding the engine lock: request registration and result
    // application use the same lock, so no previous generation slips through.
    let profile_changed = previous.settings.selected_device_serial != engine.settings.selected_device_serial;
    let policy_changed = profile_changed || previous.settings.preferences.bot_model
        != engine.settings.preferences.bot_model
        || previous.settings.preferences.offline_mode != engine.settings.preferences.offline_mode;
    state.providers.set_policy(
        &engine.settings.preferences.bot_model,
        engine.settings.preferences.offline_mode,
    );
    if let Ok(mut pending) = state.pending.lock() {
        if policy_changed {
            for flag in pending.values().flatten().filter_map(|w| w.upgrade()) {
                flag.store(true, std::sync::atomic::Ordering::Release);
            }
            pending.clear();
        }
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
    if profile_changed {
        let cached = state.providers.startup_cards(&engine.settings.widgets);
        engine.restore_data(cached);
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
fn linux_usb_status() -> linux_usb::AccessStatus {
    linux_usb::status()
}

#[tauri::command]
async fn setup_robot_access() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(linux_usb::setup)
        .await
        .map_err(|_| "USB setup stopped. Please try again.".to_string())?
}

#[tauri::command]
fn list_robot_ports() -> Result<Vec<RobotPort>, String> {
    connection::robot_ports()
}

#[tauri::command]
async fn select_robot(
    serial: Option<String>,
    bot_model: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Snapshot, String> {
    // A timed-out USB operation can still own a blocked native driver call.
    // Do not let a manual Connect start the normal worker against that port.
    let _operation = if serial.is_some() {
        Some(UsbOperationGuard::acquire()?)
    } else {
        None
    };
    let mut chosen_model = bot_model;
    let mut registry_ticket: Option<network::Ticket> = None;
    if let Some(ref value) = serial {
        let candidate = connection::robot_ports()?.into_iter()
            .find(|port| port.serial.eq_ignore_ascii_case(value))
            .ok_or("That droid is not currently detected. Reconnect its cable and try again.")?;
        let remembered = state.engine.lock().map_err(|_| "Settings are busy")?.settings.droids
            .iter().any(|p| p.serial.eq_ignore_ascii_case(value));
        if chosen_model.is_none() && !remembered {
            let guard = _operation.clone();
            let job = tauri::async_runtime::spawn_blocking(move || {
                let _guard = guard;
                let link = Link::connect(&candidate)?;
                Ok::<_,String>(link.status)
            });
            let status = tokio::time::timeout(Duration::from_secs(5), job).await
                .map_err(|_| "Checking this droid took too long. Reconnect USB and try again.")?
                .map_err(|_| "The droid check stopped. Try again.")??;
            chosen_model = status.identity.map(|identity| identity.model);
            if chosen_model.is_none() {
                if let Ok(ticket) = state.providers.request_ticket("droid_setup") {
                    chosen_model = droid_registry::lookup(value,ticket.clone()).await.filter(|_|ticket.current());
                    if chosen_model.is_some() { registry_ticket=Some(ticket); }
                }
            }
            if chosen_model.is_none() { return Err("Choose this droid’s model once so we can remember it.".into()); }
            if !connection::robot_ports()?.iter().any(|p|p.serial.eq_ignore_ascii_case(value)) { return Err("Reconnect your droid and try again.".into()); }
        }
    }
    let snapshot = mutate(&app, state, true, |engine| {
        if registry_ticket.as_ref().is_some_and(|ticket| !ticket.current() || engine.settings.preferences.offline_mode) {
            return Err("Online lookup was paused. Choose the model once or check the connection again.".into());
        }
        engine.select_droid(serial.as_deref(), chosen_model.as_deref())
    })?;
    publish_connection(&app, ConnectionSnapshot {
        state: if serial.is_some() { "connecting" } else { "preview" }.into(),
        detail: if serial.is_some() { "Getting your droid ready…" } else { "Choose a droid to connect." }.into(),
        ..ConnectionSnapshot::default()
    });
    Ok(snapshot)
}

#[tauri::command]
fn rename_droid(serial: String, name: String, app: AppHandle, state: State<AppState>) -> Result<Snapshot, String> {
    mutate(&app, state, true, |engine| droids::rename(&mut engine.settings, &serial, &name))
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

fn standalone_playlist(engine: &Engine) -> Result<protocol::StandalonePlaylist, String> {
    if !["boo", "meep", "nimbus"].contains(&engine.settings.preferences.bot_model.as_str()) {
        return Err("Saving screens to the bot is available for Nimbus, Boo and Meep Bot. Claw Bot needs a computer for live account usage.".into());
    }
    let widgets: Vec<_> = engine
        .settings
        .widgets
        .iter()
        .filter(|w| {
            w.enabled
                && ["face", "text", "image"].contains(&w.kind.as_str())
                && (w.kind != "text" || w.config.text_motion != "scroll")
                && core::kind_allowed(&engine.settings.preferences.bot_model, &w.kind)
        })
        .collect();
    if widgets.is_empty() {
        return Err(
            "Enable at least one face, still text or image screen before saving to your bot.".into(),
        );
    }
    if widgets.len() > 30 {
        return Err("A bot can save up to 30 faces, still text screens or images.".into());
    }
    // Save the intended expression, never an incidental blink or motion pose.
    let mut static_engine = engine.clone();
    static_engine.set_reduced_motion(true);
    let frames = widgets
        .into_iter()
        .map(|widget| {
            Ok(protocol::StandaloneFrame {
                frame: static_engine.preview_draft(widget)?,
                duration_sec: widget.duration_sec,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(protocol::StandalonePlaylist {
        frames,
        brightness: engine.settings.brightness,
        orientation: engine.settings.preferences.orientation,
    })
}
#[tauri::command]
async fn save_to_bot(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let engine = state.engine.lock().map_err(|_| "State unavailable")?;
        let playlist = standalone_playlist(&engine)?;
        let serial = engine
            .settings
            .selected_device_serial
            .clone()
            .ok_or("Connect your bot over USB before saving screens.")?;
        let connection = state
            .connection
            .lock()
            .map_err(|_| "Connection state unavailable")?;
        if connection.state != "connected" {
            return Err("Connect your bot over USB before saving screens.".into());
        }
        if !connection.standalone_supported {
            return Err("Update your bot to firmware 1.2.0 or newer to save screens for use with a USB wall charger.".into());
        }
        let mut pending = state
            .standalone
            .lock()
            .map_err(|_| "Save queue unavailable")?;
        if pending
            .as_ref()
            .is_some_and(|r| r.cancel.load(std::sync::atomic::Ordering::Acquire))
        {
            pending.take();
        }
        if pending.is_some() {
            return Err("A save is already waiting. Please wait for it to finish.".into());
        }
        let operation = UsbOperationGuard::acquire()?;
        *pending = Some(StandaloneRequest {
            _operation: operation,
            generation: state
                .save_generation
                .load(std::sync::atomic::Ordering::Acquire),
            settings: serde_json::to_vec(&engine.settings)
                .map_err(|_| "Could not capture screens")?,
            serial,
            playlist,
            cancel: cancel.clone(),
            response: tx,
        });
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(Duration::from_secs(60)).map_err(|_| {
            cancel.store(true, std::sync::atomic::Ordering::Release);
            "The bot has not confirmed this save. Keep it connected, check its status, then try again.".to_string()
        })?
    }).await.map_err(|_| "Save stopped before confirmation".to_string())??;
    Ok(serde_json::json!({"saved":true,"count":result,
        "message":"Saved to your bot. It can now show these screens using a USB wall charger. Faces are saved as still images; live data and clocks still need the app."}))
}
fn process_standalone_request(app: &AppHandle, link: &mut Link, serial: &str) {
    let state = app.state::<AppState>();
    let request = state
        .standalone
        .lock()
        .ok()
        .and_then(|mut pending| pending.take());
    let Some(request) = request else {
        return;
    };
    let result = (|| {
        let engine = state.engine.lock().map_err(|_| "State unavailable")?;
        if request.cancel.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Save canceled before it started".into());
        }
        if request.generation
            != state
                .save_generation
                .load(std::sync::atomic::Ordering::Acquire)
            || request.serial != serial
            || engine.settings.selected_device_serial.as_deref() != Some(serial)
            || serde_json::to_vec(&engine.settings).map_err(|_| "Could not check screens")?
                != request.settings
        {
            return Err(
                "Your selected bot or screens changed before saving. Please save again.".into(),
            );
        }
        // Keep the selected device/profile stable throughout this bounded USB
        // transaction. No additional serial connection is opened.
        link.save_standalone(&request.playlist)?;
        Ok(request.playlist.frames.len())
    })();
    publish_connection(app, link.status.clone());
    let _ = request.response.send(result);
}

#[tauri::command]
fn preview_draft(widget: Widget, state: State<AppState>) -> Result<Vec<u8>, String> {
    state
        .engine
        .lock()
        .map_err(|e| e.to_string())?
        .preview_draft(&widget)
}
#[tauri::command]
fn set_bot_model(
    bot_model: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| e.set_bot_model(&bot_model))
}
#[tauri::command]
fn set_offline_mode(
    offline: bool,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Snapshot, String> {
    mutate(&app, state, true, |e| {
        let mut preferences = e.settings.preferences.clone();
        preferences.offline_mode = offline;
        e.set_preferences(preferences)
    })
}
fn source_ticket(state: &AppState, kind: &str) -> Result<network::Ticket, String> {
    let _engine = state.engine.lock().map_err(|_| "State unavailable")?;
    state.providers.request_ticket(kind)
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
    let ticket = source_ticket(&state, "weather")?;
    let providers = state.providers.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ticket.enter();
        let result = providers.search_locations(&name);
        network::ensure_allowed()?;
        result
    })
    .await
    .map_err(|_| "Location search stopped".to_string())?
}
#[tauri::command]
async fn codex_windows(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<UsageWindow>, String> {
    let ticket = source_ticket(&state, "codex")?;
    let providers = state.providers.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ticket.enter();
        let result = providers.codex_windows(&path);
        network::ensure_allowed()?;
        result
    })
    .await
    .map_err(|_| "Account read stopped".to_string())?
}
#[tauri::command]
async fn source_info(
    profile: Option<String>,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let ticket = source_ticket(&state, "claude")?;
    let bridge = state
        .settings_path
        .parent()
        .ok_or("Settings directory unavailable")?
        .join("claude-bridge");
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ticket.enter();
        network::ensure_allowed()?;
        let result = serde_json::json!({"claudeBridge":claude_bridge::status(&bridge)?,"claudeAccount":claude_account::status(profile.as_deref().unwrap_or(""))});
        network::ensure_allowed()?;
        Ok(result)
    }).await.map_err(|_| "Source setup stopped".to_string())?
}
#[tauri::command]
async fn connect_claude_account(
    profile: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let ticket = source_ticket(&state, "claude")?;
    let providers = state.providers.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ticket.enter();
        network::ensure_allowed()?;
        claude_account::login(profile.as_deref().unwrap_or(""))?;
        network::ensure_allowed()?;
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
    let ticket = if enabled {
        Some(source_ticket(&state, "claude")?)
    } else {
        None
    };
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
        let _guard = ticket.as_ref().map(network::Ticket::enter);
        if enabled {
            network::authorized(|| claude_bridge::install(&bridge, &report_dir))
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
    let (widget, ticket, cancel) = {
        let engine = state.engine.lock().map_err(|e| e.to_string())?;
        let widget = engine
            .settings
            .widgets
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or("Widget not found")?;
        if widget.config.mode != "live" {
            return Err("Choose Live data in Edit first".into());
        }
        if !["weather", "stock", "codex", "claude"].contains(&widget.kind.as_str()) {
            return Err("This screen works locally and does not need an online refresh.".into());
        }
        let ticket = state.providers.request_ticket(&widget.kind)?;
        let cancel = register_request(&state, &id)?;
        (widget, ticket, cancel)
    };
    let providers = state.providers.clone();
    let requested_config = widget.config.clone();
    let requested_kind = widget.kind.clone();
    providers.retry_authentication(&widget);
    let request_flag = cancel.clone();
    let request_ticket = ticket.clone();
    let card = tauri::async_runtime::spawn_blocking(move || {
        let _guard = request_ticket.enter();
        providers.fetch_cancelable(&widget, request_flag)
    })
    .await
    .map_err(|_| "Refresh stopped".to_string())?;
    mutate(&app, state, false, |e| {
        if ticket.current()
            && !e.settings.preferences.offline_mode
            && core::kind_allowed(&e.settings.preferences.bot_model, &requested_kind)
            && !cancel.load(std::sync::atomic::Ordering::Acquire)
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
                    .filter(|w| {
                        w.enabled
                            && matches!(w.kind.as_str(), "cpu" | "memory")
                            && core::kind_allowed(&e.settings.preferences.bot_model, &w.kind)
                    })
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
                    current.id == w.id
                        && current.enabled
                        && current.config == w.config
                        && core::kind_allowed(&e.settings.preferences.bot_model, &current.kind)
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
static SUPPORT_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
struct SupportRun;
impl SupportRun {
    fn acquire() -> Result<Self,String> {
        if USB_OPERATION.load(std::sync::atomic::Ordering::SeqCst) { return Err("Wait for the current USB operation or app installation to finish.".into()); }
        SUPPORT_RUNNING.compare_exchange(false,true,std::sync::atomic::Ordering::SeqCst,std::sync::atomic::Ordering::SeqCst).map_err(|_|"A diagnostic check is already running. Wait for it to finish.")?;
        Ok(Self)
    }
}
impl Drop for SupportRun { fn drop(&mut self) { SUPPORT_RUNNING.store(false,std::sync::atomic::Ordering::SeqCst); } }
static SUPPORT_ENUMERATING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
struct SupportEnumeration;
impl Drop for SupportEnumeration { fn drop(&mut self) { SUPPORT_ENUMERATING.store(false,std::sync::atomic::Ordering::SeqCst); } }

#[tauri::command]
async fn run_support_diagnostics(symptom: support::Symptom, app: AppHandle) -> Result<support::Report,String> {
    let _run=SupportRun::acquire()?;
    let initial_selection=app.state::<AppState>().engine.lock().map_err(|_|"Settings are busy")?.settings.selected_device_serial.clone();
    rescan_robot();
    let scan=async {
        // A stalled native driver can outlive its timeout. Keep its permit in
        // the worker so repeat checks cannot accumulate blocked scan threads.
        if SUPPORT_ENUMERATING.compare_exchange(false,true,std::sync::atomic::Ordering::SeqCst,std::sync::atomic::Ordering::SeqCst).is_err() {return None;}
        let permit=SupportEnumeration;
        tokio::time::timeout(Duration::from_secs(5),tauri::async_runtime::spawn_blocking(move || {let _permit=permit;(connection::robot_ports(),firmware_setup::drives())})).await.ok().and_then(Result::ok)
    };
    let observe=async {
        let mut prior: Option<(Option<String>,String)>=None;
        let mut changes=0; let mut replies=support::ReplyEvidence::default(); let mut target_changed=false;
        for _ in 0..25 {
            let state=app.state::<AppState>();
            let c=state.connection.lock().map_err(|_|"Connection is busy")?.clone();
            let selected=state.engine.lock().map_err(|_|"Settings are busy")?.settings.selected_device_serial.clone();
            target_changed |= selected != initial_selection || USB_OPERATION.load(std::sync::atomic::Ordering::SeqCst);
            let metrics=state.link_metrics.lock().map_err(|_|"USB counters are busy")?.clone();
            replies.observe(initial_selection.as_deref(),c.board_id.as_deref(),&metrics);
            if let Some((board,status))=&prior {
                if board!=&c.board_id || status!=&c.state {changes+=1;}
            }
            prior=Some((c.board_id,c.state));
            if prior.is_some() {tokio::time::sleep(Duration::from_millis(240)).await;}
        }
        Ok::<_,String>((changes,replies.total,target_changed))
    };
    let (scan,observations)=tokio::join!(scan,observe);
    let (changes,replies,target_changed)=observations?;
    let (ports,drives,complete)=match scan {Some((Ok(ports),Ok(drives)))=>(ports,drives,true),_ => (Vec::new(),Vec::new(),false)};
    let state=app.state::<AppState>();
    let snapshot=state.engine.lock().map_err(|_|"Settings are busy")?.snapshot();
    let mut connection=state.connection.lock().map_err(|_|"Connection is busy")?.clone();
    let target=initial_selection.or_else(||connection.board_id.clone()).or_else(||if ports.len()==1 {Some(ports[0].serial.clone())} else {None});
    let target_present=target.as_ref().is_some_and(|id|ports.iter().any(|p|p.serial.eq_ignore_ascii_case(id)));
    if connection.board_id.is_none() {connection.board_id=target.clone();}
    let identity_matches=target.as_deref().is_some_and(|id|connection.board_id.as_deref().is_some_and(|b|id.eq_ignore_ascii_case(b)));
    static SEQUENCE: std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
    let id=format!("DF-{:X}-{:X}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),SEQUENCE.fetch_add(1,std::sync::atomic::Ordering::SeqCst));
    let report=support::build(id,symptom,&connection,&snapshot,support::Facts {runtime_devices:ports.len(),recovery_devices:drives.len(),enumeration_complete:complete,target_present:target_present && identity_matches,acknowledged_commands:replies,connection_changes:changes,target_changed:target_changed || (target.is_some() && !identity_matches)});
    *state.support_report.lock().map_err(|_|"Support report is busy")?=Some(report.clone());
    Ok(report)
}

#[tauri::command]
async fn show_support_pattern(report_id: String, app: AppHandle) -> Result<support::Report,String> {
    let _run=SupportRun::acquire()?;
    let _usb=UsbOperationGuard::acquire()?;
    let state=app.state::<AppState>();
    let report=state.support_report.lock().map_err(|_|"Support report is busy")?.clone().filter(|r|r.report_id==report_id && r.can_test_screen && core::unix_now().saturating_sub(r.created_at)<=300).ok_or("Run diagnostics again before testing this droid.")?;
    let board=report.board_id.ok_or("Droid identity is unavailable. Run diagnostics again.")?;
    let connection=state.connection.lock().map_err(|_|"Connection is busy")?.clone();
    let selected=state.engine.lock().map_err(|_|"Settings are busy")?.settings.selected_device_serial.clone();
    if !["connected","display_error"].contains(&connection.state.as_str()) || !connection.board_id.as_deref().is_some_and(|id|id.eq_ignore_ascii_case(&board)) || !selected.as_deref().is_some_and(|id|id.eq_ignore_ascii_case(&board)) {return Err("This droid is no longer connected. Run diagnostics again.".into());}
    let acknowledged=Arc::new(std::sync::atomic::AtomicBool::new(false));
    *state.support_pattern.lock().map_err(|_|"Screen test is busy")?=Some(support::Pattern {_operation:Some(_usb.clone()),board_id:board,expires:Instant::now()+Duration::from_secs(8),acknowledged:acknowledged.clone()});
    tokio::time::sleep(Duration::from_secs(8)).await;
    *state.support_pattern.lock().map_err(|_|"Screen test is busy")?=None;
    let mut cached=state.support_report.lock().map_err(|_|"Support report is busy")?;
    let report=cached.as_mut().filter(|r|r.report_id==report_id).ok_or("This support report was replaced. Run diagnostics again.")?;
    report.visual_test.requested=true;
    report.visual_test.acknowledged=acknowledged.load(std::sync::atomic::Ordering::SeqCst);
    report.visual_test.observation=support::Observation::NotChecked;
    Ok(report.clone())
}

#[tauri::command]
fn record_support_observation(report_id: String, observation: support::Observation, state: State<AppState>) -> Result<support::Report,String> {
    let mut cached=state.support_report.lock().map_err(|_|"Support report is busy")?;
    let report=cached.as_mut().filter(|r|r.report_id==report_id && r.visual_test.requested).ok_or("Show a test pattern before recording its result.")?;
    support::record_observation(report,observation)?;
    Ok(report.clone())
}

#[tauri::command]
async fn export_support_report(report_id: String, app: AppHandle) -> Result<bool,String> {
    let state=app.state::<AppState>();
    let report=state.support_report.lock().map_err(|_|"Support report is busy")?.clone().filter(|r|r.report_id==report_id).ok_or("This support report was replaced. Review the latest report before saving.")?;
    let bytes=serde_json::to_vec_pretty(&report).map_err(|_|"Could not prepare support report")?;
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Write;
        let Some(path)=app.dialog().file().set_file_name(format!("DroidFun-support-{}.json",report.report_id)).add_filter("JSON",&["json"]).blocking_save_file() else {return Ok(false);};
        let path=path.into_path().map_err(|_|"Choose a local report file")?;
        let mut file=fs::OpenOptions::new().write(true).create(true).truncate(true).open(path).map_err(|_|"Could not save support report")?;
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;file.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|_|"Could not protect support report")?;}
        file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|_|"Could not finish saving support report")?;
        Ok(true)
    }).await.map_err(|_|"Support report export stopped".to_string())?
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
    let cards:Vec<_>=e.snapshot().data.values().map(|d|serde_json::json!({"source":d.source_id,"state":d.state,"fetchedAt":d.fetched_at,"observedAt":d.observed_at,"expiresAt":d.expires_at,"allowanceState":d.values.get("allowanceState")})).collect();
    let events = state
        .events
        .lock()
        .map_err(|_| "Event history unavailable")?;
    let report=serde_json::to_string_pretty(&serde_json::json!({"formatVersion":1,"appVersion":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"uptimeSeconds":state.started.elapsed().as_secs(),"trayAvailable":state.platform.tray_available,"connection":{"state":c.state,"firmware":c.firmware,"displayHealth":c.display_health,"redactedBoard":c.board_id.as_ref().map(|id|format!("{:x}",Sha256::digest(id.as_bytes()))[..8].to_string())},"display":{"brightness":e.settings.brightness,"orientation":e.settings.preferences.orientation,"reason":e.snapshot().display_reason},"widgets":widgets,"providers":cards,"connectionEvents":*events,"claudeAccount":claude_account::diagnostics(),"excluded":"credentials, account identity, location, symbols, prompts, transcripts, local paths"})).map_err(|_|"Could not prepare diagnostics")?;
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
            .set_file_name("DroidFun-RP2040-Zero-firmware-v1.2.4.uf2")
            .add_filter("RP2040 firmware", &["uf2"])
            .blocking_save_file();
        let Some(path) = path else {
            return Ok(false);
        };
        fs::write(
            path.into_path().map_err(|_| "Choose a local file")?,
            include_bytes!("../../firmware/releases/droidfun-rp2040-zero-gp4-gp5-v1.2.4.uf2"),
        )
        .map_err(|_| "Could not save firmware")?;
        Ok(true)
    })
    .await
    .map_err(|_| "Firmware export stopped".to_string())?
}
#[tauri::command]
async fn list_setup_drives() -> Result<Vec<firmware_setup::SetupDrive>, String> {
    tokio::time::timeout(
        Duration::from_secs(5),
        tauri::async_runtime::spawn_blocking(firmware_setup::drives),
    )
    .await
    .map_err(|_| {
        "USB scan is waiting for the drive. Reconnect your USB cable and scan again.".to_string()
    })?
    .map_err(|_| "USB setup scan stopped".to_string())?
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparedUpdate {
    path: String,
    board_id: String,
}

struct RecoveryTicket { path: String, serial: String, created: Instant }
impl RecoveryTicket {
    fn matches(&self, path: &str, serial: Option<&str>, now: Instant) -> bool {
        self.path == path && serial.is_some_and(|id| id.eq_ignore_ascii_case(&self.serial))
            && now.saturating_duration_since(self.created) < Duration::from_secs(300)
    }
}
static CUSTOMER_RECOVERY: Mutex<Option<RecoveryTicket>> = Mutex::new(None);
static DIAGNOSTIC_BOARD: Mutex<Option<String>> = Mutex::new(None);

static DISCOVERY_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[tauri::command]
fn rescan_robot() {
    DISCOVERY_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

static USB_OPERATION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) struct UsbOperationGuard;

impl UsbOperationGuard {
    pub(crate) fn acquire() -> Result<Arc<Self>, String> {
        USB_OPERATION
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .map_err(|_| {
                "An app update or USB operation is still running. Wait for it to finish before trying again."
                    .to_string()
            })?;
        Ok(Arc::new(Self))
    }
}

impl Drop for UsbOperationGuard {
    fn drop(&mut self) {
        USB_OPERATION.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn restart_canceled(cancel: &std::sync::atomic::AtomicBool) -> bool {
    cancel.load(std::sync::atomic::Ordering::SeqCst)
}

// A small port interface keeps the actual reset sequence testable without
// opening hardware. The native driver calls themselves cannot be interrupted.
trait RestartPort {
    fn dtr(&mut self, state: bool) -> Result<(), String>;
    fn baud(&mut self, rate: u32) -> Result<(), String>;
}

impl RestartPort for Box<dyn serialport::SerialPort> {
    fn dtr(&mut self, state: bool) -> Result<(), String> {
        self.write_data_terminal_ready(state)
            .map_err(|_| "Could not request USB restart".into())
    }

    fn baud(&mut self, rate: u32) -> Result<(), String> {
        self.set_baud_rate(rate)
            .map_err(|_| "Could not prepare USB update".into())
    }
}

fn request_usb_restart(
    usb: &mut impl RestartPort,
    cancel: &std::sync::atomic::AtomicBool,
    settle: impl FnOnce(),
) -> Result<(), String> {
    let canceled =
        || "USB restart was canceled. Scan USB to check your bot’s current state.".to_string();
    if restart_canceled(cancel) {
        return Err(canceled());
    }
    let dtr = usb.dtr(true);
    if restart_canceled(cancel) {
        return Err(canceled());
    }
    dtr?;
    // A baud call may have changed the device even if its driver later reports
    // an error. Restore a non-triggering rate before dropping the port whenever
    // the 1200-baud operation is canceled or fails.
    let baud = usb.baud(1200);
    if restart_canceled(cancel) || baud.is_err() {
        let _ = usb.baud(115200);
        return Err(if restart_canceled(cancel) {
            canceled()
        } else {
            baud.unwrap_err()
        });
    }
    settle();
    if restart_canceled(cancel) {
        let _ = usb.baud(115200);
        return Err(canceled());
    }
    let reset = usb.dtr(false);
    if restart_canceled(cancel) || reset.is_err() {
        // The reset may already have reached the board. This is best-effort
        // cleanup, not a claim that a blocked native call was rolled back.
        let _ = usb.baud(115200);
        return Err(if restart_canceled(cancel) {
            canceled()
        } else {
            reset.unwrap_err()
        });
    }
    Ok(())
}

#[tauri::command]
fn supports_display_test() -> bool {
    picoboot::supported()
}

#[tauri::command]
async fn prepare_display_test(app: AppHandle) -> Result<PreparedUpdate, String> {
    if !picoboot::supported() {
        return Err(
            "The standalone display test requires direct USB setup on a supported Mac.".into(),
        );
    }
    prepare_robot_update(None, app).await
}

#[tauri::command]
async fn prepare_robot_update(
    serial: Option<String>,
    app: AppHandle,
) -> Result<PreparedUpdate, String> {
    if !picoboot::supported() {
        return Err("This computer cannot verify a droid’s hardware identity before firmware recovery. Contact Droid Fun support for droid software service. App updates are available separately in Settings → App.".into());
    }
    let preparation = UsbOperationGuard::acquire()?;
    *CUSTOMER_RECOVERY.lock().map_err(|_| "Recovery state is busy")? = None;
    // An explicit serial permits restoring the diagnostic sketch, which does
    // not implement the normal app handshake. Resolve it against current USB
    // descriptors, then pass its identity through to the flash verification.
    let (requested, expected_port) = if let Some(serial) = serial {
        (serial, None)
    } else {
        let state = app.state::<AppState>();
        let c = state
            .connection
            .lock()
            .map_err(|_| "Connection state unavailable")?;
        if c.board_id.is_none() || c.firmware.is_none() {
            return Err(
                "Connect a bot that answers DroidFun App before requesting a USB update.".into(),
            );
        }
        (
            c.board_id.clone().unwrap(),
            Some(c.port.clone().ok_or("Robot USB port unavailable")?),
        )
    };
    let candidate = connection::robot_ports()?
        .into_iter()
        .find(|p| {
            p.serial.eq_ignore_ascii_case(&requested)
                && expected_port.as_ref().is_none_or(|path| &p.path == path)
        })
        .ok_or("Your bot is no longer on USB. Reconnect its cable and scan again.")?;
    let serial = candidate.serial.clone();
    let port = candidate.path.clone();
    let previous_selection = app
        .state::<AppState>()
        .engine
        .lock()
        .map_err(|_| "State unavailable")?
        .settings
        .selected_device_serial
        .clone();
    let before = list_setup_drives().await?;
    // Stop the normal link worker before opening the selected serial port.
    select_robot(None, None, app.clone(), app.state::<AppState>()).await?;
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_guard = preparation.clone();
    let job = tauri::async_runtime::spawn_blocking(move || {
        // The caller keeps its reference through ROM discovery; this reference
        // also keeps the operation locked if a native call outlives the timeout.
        let _guard = worker_guard;
        // A failed handshake may take two bounded attempts to release its port.
        std::thread::sleep(Duration::from_millis(1500));
        if restart_canceled(&worker_cancel) {
            return Err("USB restart was canceled before opening the port.".to_string());
        }
        // Do not trust a caller-supplied serial or a stale UI status: inspect
        // this exact board before sending the reset that enables a flash.
        match Link::connect(&candidate) {
            Ok(link) => onboarding::allow_customer_reflash(&link.status)?,
            Err(_) => {
                // The diagnostic sketch has no HELLO; its bounded identity-
                // checked report is the only permitted non-runtime exception.
                display_test::read(&candidate.serial, &worker_cancel)
                    .map_err(|_| "Could not verify the installed droid software. Reconnect USB or contact Droid Fun support before reinstalling.".to_string())?;
            }
        }
        if restart_canceled(&worker_cancel) { return Err("USB restart was canceled.".into()); }
        let mut usb = serialport::new(&port, 115200)
            .dtr_on_open(true)
            .timeout(Duration::from_millis(500))
            .open()
            .map_err(|_| {
                "Could not prepare the bot for its update. Reconnect USB and try again.".to_string()
            })?;
        if restart_canceled(&worker_cancel) {
            return Err("USB restart was canceled after opening the port.".to_string());
        }
        request_usb_restart(&mut usb, &worker_cancel, || {
            std::thread::sleep(Duration::from_millis(100));
        })
    });
    let result = match tokio::time::timeout(Duration::from_secs(10), job).await {
        Ok(result) => result
            .map_err(|_| "USB update preparation stopped".to_string())
            .and_then(|result| result),
        Err(_) => {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            // Keep selection paused: the native worker may still own the port.
            // Its guard prevents another restart until it has actually exited.
            return Err("USB restart timed out. The bot may still restart when the driver returns. Unplug and reconnect its cable, then scan USB before trying again.".into());
        }
    };
    if let Err(error) = result {
        let _ = mutate(&app, app.state::<AppState>(), true, |e| {
            e.settings.selected_device_serial = previous_selection.clone();
            Ok(())
        });
        return Err(error);
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Ok(found) = list_setup_drives().await {
            let new: Vec<_> = found
                .into_iter()
                .filter(|d| !before.iter().any(|old| old.path == d.path))
                .collect();
            if new.len() == 1 {
                *CUSTOMER_RECOVERY.lock().map_err(|_| "Recovery state is busy")? = Some(RecoveryTicket {
                    path: new[0].path.clone(), serial: serial.clone(), created: Instant::now()
                });
                return Ok(PreparedUpdate {
                    path: new[0].path.clone(),
                    board_id: serial,
                });
            }
            if new.len() > 1 {
                let _ = mutate(&app, app.state::<AppState>(), true, |e| {
                    e.settings.selected_device_serial = previous_selection.clone();
                    Ok(())
                });
                return Err("More than one robot is in setup mode. Use the Connection section to choose yours.".into());
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let _ = mutate(&app, app.state::<AppState>(), true, |e| {
        e.settings.selected_device_serial = previous_selection;
        Ok(())
    });
    Err("The app could not detect your bot’s update connection. Scan USB to check whether setup is available.".into())
}
#[tauri::command]
async fn install_robot_firmware(
    path: String,
    expected_board_id: Option<String>,
    bot_model: Option<String>,
    app: AppHandle,
) -> Result<Snapshot, String> {
    if bot_model.as_deref().is_some_and(|model| !["boo", "meep", "claw", "nimbus"].contains(&model)) {
        return Err("Choose Nimbus, Boo, Meep or Claw before setup.".into());
    }
    let serial = install_image(path, expected_board_id, false, app.clone()).await?;
    // Persist the newly discovered board, replacing any old robot selection.
    // The link worker performs HELLO and OLED checks before saying connected.
    select_robot(Some(serial), bot_model, app.clone(), app.state::<AppState>()).await
}

#[tauri::command]
async fn install_display_test(
    path: String,
    expected_board_id: Option<String>,
    app: AppHandle,
) -> Result<String, String> {
    install_image(path, expected_board_id, true, app).await
}

#[tauri::command]
async fn read_display_test(serial: String, app: AppHandle) -> Result<display_test::Report, String> {
    // Suppress automatic HELLO probes of the standalone diagnostic sketch,
    // including when the webview restores its unfinished-test marker.
    if !connection::robot_ports()?.iter().any(|p| p.serial.eq_ignore_ascii_case(&serial)) {
        return Err("Reconnect the droid running the screen test first.".into());
    }
    *DIAGNOSTIC_BOARD.lock().map_err(|_| "Diagnostic state is busy")? = Some(serial.clone());
    let operation = UsbOperationGuard::acquire()?;
    {
        let state = app.state::<AppState>();
        let engine = state.engine.lock().map_err(|_| "State unavailable")?;
        if engine.settings.selected_device_serial.is_some() {
            return Err(
                "Pause the normal bot connection before reading its standalone display test."
                    .into(),
            );
        }
    }
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_guard = operation.clone();
    let job = tauri::async_runtime::spawn_blocking(move || {
        let _guard = worker_guard;
        display_test::read(&serial, &worker_cancel)
    });
    match tokio::time::timeout(Duration::from_secs(7), job).await {
        Ok(result) => result.map_err(|_| {
            "The display test reader stopped. Its result was not confirmed.".to_string()
        })?,
        Err(_) => {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            Err("The display test’s USB connection stalled. Unplug and reconnect the cable before reading again. Its result was not confirmed.".into())
        }
    }
}

async fn install_image(
    path: String,
    expected_board_id: Option<String>,
    test: bool,
    app: AppHandle,
) -> Result<String, String> {
    let operation = UsbOperationGuard::acquire()?;
    if !path.starts_with("picoboot:") || !picoboot::supported() {
        return Err("Droid recovery requires a verified direct USB connection. Contact Droid Fun support for help with this droid.".into());
    }
    // A bootloader cannot report its existing firmware or wiring. Only a
    // fresh, identity-bound preflight may authorize customer recovery.
    let ticket = CUSTOMER_RECOVERY.lock().map_err(|_| "Recovery state is busy")?.take();
    if !ticket.is_some_and(|ticket| ticket.matches(&path, expected_board_id.as_deref(), Instant::now())) {
        return Err("This droid is in recovery mode, so its installed software and screen wiring cannot be verified. Unplug and reconnect it. If it stays in recovery mode, contact Droid Fun support; new boards are prepared with Workshop.".into());
    }
    {
        let state = app.state::<AppState>();
        let engine = state.engine.lock().map_err(|_| "State unavailable")?;
        if !["boo", "meep", "claw", "nimbus"].contains(&engine.settings.preferences.bot_model.as_str()) {
            return Err("Choose Nimbus, Boo, Meep or Claw before installing its software.".into());
        }
    }
    if test {
        if !path.starts_with("picoboot:") || !picoboot::supported() {
            return Err(
                "The standalone display test requires direct USB setup on a supported Mac.".into(),
            );
        }
        // Persist the paused selection across app restarts. The diagnostic
        // sketch runs independently and reports JSON rather than app packets.
        select_robot(None, None, app.clone(), app.state::<AppState>()).await?;
    }
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_guard = operation.clone();
    let progress = app.clone();
    let job = tauri::async_runtime::spawn_blocking(move || {
        let _guard = worker_guard;
        let report = |message: &str| {
            if !worker_cancel.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = progress.emit("deskbot-firmware-setup", message);
            }
        };
        if test {
            firmware_setup::install_display_test(
                &path,
                expected_board_id.as_deref(),
                &worker_cancel,
                report,
            )
        } else {
            firmware_setup::install(&path, expected_board_id.as_deref(), &worker_cancel, report)
        }
    });
    let serial = match tokio::time::timeout(Duration::from_secs(75), job).await {
        Ok(result) => result.map_err(|_| "Robot installation stopped".to_string())??,
        Err(_) => {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
            return Err(
                "USB setup has stalled. Unplug and reconnect the USB cable before trying again."
                    .into(),
            );
        }
    };
    *DIAGNOSTIC_BOARD.lock().map_err(|_| "Diagnostic state is busy")? = if test {Some(serial.clone())} else {None};
    Ok(serial)
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
            .set_file_name("DroidFun-App-diagnostics.json")
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

fn discover_droid(app: &AppHandle, probes: &mut onboarding::ProbeSchedule,
    cached: &mut Option<ConnectionSnapshot>) {
    let ports = match connection::robot_ports() {
        Ok(ports) => ports,
        Err(error) => { publish_connection(app, ConnectionSnapshot {state: "discovery_error".into(), detail: error, ..ConnectionSnapshot::default()}); return; }
    };
    probes.refresh(DISCOVERY_GENERATION.load(std::sync::atomic::Ordering::SeqCst), ports.iter().map(|p| p.serial.as_str()));
    let [candidate] = ports.as_slice() else {
        *cached = None;
        publish_connection(app, ConnectionSnapshot {
            state: if ports.is_empty() {"preview"} else {"choose_device"}.into(),
            detail: if ports.is_empty() {"Plug your droid into this computer with a USB data cable."} else {"Choose the droid you want to use. Each keeps its own screens."}.into(),
            ..ConnectionSnapshot::default()});
        return;
    };
    if DIAGNOSTIC_BOARD.lock().ok().is_some_and(|id| id.as_deref().is_some_and(|id| id.eq_ignore_ascii_case(&candidate.serial))) {
        publish_connection(app, ConnectionSnapshot {state: "diagnostic".into(), detail: "The screen test is running. Finish it to return to your normal screens.".into(), board_id: Some(candidate.serial.clone()), ..ConnectionSnapshot::default()});
        return;
    }
    if !probes.ready(&candidate.serial, Instant::now()) {
        if let Some(status) = cached.as_ref().filter(|c| c.board_id.as_deref().is_some_and(|id| id.eq_ignore_ascii_case(&candidate.serial))) {
            publish_connection(app, status.clone());
        }
        return;
    }
    let Ok(_guard) = UsbOperationGuard::acquire() else {return;};
    publish_connection(app, ConnectionSnapshot {state: "identifying".into(), detail: "Recognizing your droid…".into(), port: Some(candidate.path.clone()), board_id: Some(candidate.serial.clone()), ..ConnectionSnapshot::default()});
    match Link::connect(candidate) {
        Ok(link) => {
            probes.record_success(&candidate.serial);
            let mut status = link.status.clone();
            drop(link);
            drop(_guard);
            let first = mutate(app, app.state::<AppState>(), true, |engine| onboarding::adopt(engine, &status));
            let result = if first.is_err() && status.identity.is_none() {
                let state=app.state::<AppState>();
                let ticket=state.providers.request_ticket("droid_setup");
                if let Ok(ticket)=ticket {
                    let hint=tauri::async_runtime::block_on(droid_registry::lookup(&candidate.serial,ticket.clone()));
                    if let Some(model)=hint.filter(|_|ticket.current()) {
                        if connection::robot_ports().is_ok_and(|ports|ports.iter().any(|p|p.serial.eq_ignore_ascii_case(&candidate.serial))) {
                            mutate(app,app.state::<AppState>(),true,|engine|onboarding::adopt_hint(engine,&status,&model))
                        } else { first }
                    } else { first }
                } else { first }
            } else { first };
            match result {
                Ok(_) => { *cached = None; publish_connection(app, ConnectionSnapshot {state: "connecting".into(), detail: "Your droid was recognized. Connecting…".into(), ..status}); }
                Err(error) => {
                    // HELLO is good even for older boards without a stored model.
                    status.state = if status.identity.is_none() {"needs_model"} else {"discovery_error"}.into();
                    status.detail = error;
                    *cached = Some(status.clone()); publish_connection(app, status);
                }
            }
        }
        Err(error) => {
            probes.record_failure(&candidate.serial, Instant::now());
            let status = ConnectionSnapshot {
                state: if error == linux_usb::PERMISSION_DETAIL {"permission_needed"} else {"setup_needed"}.into(),
                detail: error, port: Some(candidate.path.clone()), board_id: Some(candidate.serial.clone()),
                ..ConnectionSnapshot::default()};
            *cached = Some(status.clone()); publish_connection(app, status);
        }
    }
}

fn connection_worker(app: AppHandle) {
    let mut backoff = 0;
    let mut probes = onboarding::ProbeSchedule::default();
    let mut discovered = None;
    loop {
        // A support pattern owns the operation guard to exclude flashing and
        // selection changes, while this worker must keep its normal link alive.
        let support_pattern_active=app.state::<AppState>().support_pattern.lock().ok()
            .is_some_and(|mut p| {if p.as_ref().is_some_and(|p|Instant::now()>=p.expires) {*p=None;} p.is_some()});
        if USB_OPERATION.load(std::sync::atomic::Ordering::SeqCst) && !support_pattern_active {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
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
                .and_then(|engine| {
                    ["boo", "meep", "claw", "nimbus"].contains(&engine.settings.preferences.bot_model.as_str())
                        .then(|| engine.settings.selected_device_serial.clone()).flatten()
                })
        };
        let Some(serial) = selected else {
            discover_droid(&app, &mut probes, &mut discovered);
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let candidate = connection::robot_ports()
            .ok()
            .and_then(|ports| ports.into_iter().find(|port| port.serial.eq_ignore_ascii_case(&serial)));
        let Some(candidate) = candidate else {
            publish_connection(
                &app,
                ConnectionSnapshot {
                    state: "reconnecting".into(),
                    detail: "Waiting for your bot. Reconnecting automatically when USB returns.".into(),
                    ..ConnectionSnapshot::default()
                },
            );
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let previous = app
            .state::<AppState>()
            .connection
            .lock()
            .ok()
            .map(|c| c.clone())
            .filter(|c| {
                c.board_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(&serial))
            })
            .unwrap_or_default();
        publish_connection(
            &app,
            ConnectionSnapshot {
                state: "connecting".into(),
                detail: "Checking the DroidFun App firmware handshake".into(),
                port: Some(candidate.path.clone()),
                ..previous
            },
        );
        match Link::connect(&candidate) {
            Ok(mut link) => {
                backoff = 0;
                if let Ok(mut metrics) = app.state::<AppState>().link_metrics.lock() { *metrics = connection::LinkMetrics::default(); }
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
                    process_standalone_request(&app, &mut link, &serial);
                    let pattern = app.state::<AppState>().support_pattern.lock().ok()
                        .and_then(|mut p| {if p.as_ref().is_some_and(|p|Instant::now()>=p.expires) {*p=None;} p.as_ref().filter(|p|p.active(&serial,Instant::now())).cloned()});
                    let frame = pattern.as_ref().map(|_|support::test_frame());
                    let prior_acks = link.metrics.frame_acks;
                    match link.update(
                        frame.as_deref().unwrap_or(&snapshot.frame),
                        pattern.is_some() || snapshot.display_on,
                        if pattern.is_some() {75} else {snapshot.settings.brightness},
                        snapshot.settings.preferences.orientation,
                    ) {
                        Ok(changed) => {
                            if let Some(pattern) = &pattern { if link.metrics.frame_acks > prior_acks { pattern.acknowledge(); } }
                            if changed {
                                publish_connection(&app, link.status.clone());
                            }
                        }
                        Err(error) => {
                            // DISPLAY/PING may have succeeded before a failed
                            // FRAME. Keep that USB evidence for support checks.
                            if let Ok(mut metrics) = app.state::<AppState>().link_metrics.lock() { *metrics = link.metrics.clone(); }
                            publish_connection(
                                &app,
                                ConnectionSnapshot {
                                    state: if error.contains("i2c_error")
                                        || error.contains("screen is not responding")
                                    {
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
                                    ..link.status.clone()
                                },
                            );
                            break;
                        }
                    }
                    if let Ok(mut metrics) = app.state::<AppState>().link_metrics.lock() {
                        *metrics = link.metrics.clone();
                    }
                    std::thread::sleep(link.next_poll_delay());
                }
            }
            Err(error) => {
                let setup = error.contains("No DroidFun App reply")
                    || error.contains("No reply from the bot after two attempts")
                    || error.contains("invalid DroidFun App handshake");
                publish_connection(
                    &app,
                    ConnectionSnapshot {
                        state: if error == linux_usb::PERMISSION_DETAIL {
                            "permission_needed"
                        } else if setup {
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
            let requests = state
                .engine
                .lock()
                .map(|e| {
                    e.settings
                        .widgets
                        .iter()
                        .filter(|w| w.enabled && w.config.mode == "live" && w.kind == kind)
                        .filter_map(|w| {
                            let ticket = state.providers.request_ticket(&w.kind).ok()?;
                            let cancel = register_request(&state, &w.id).ok()?;
                            Some((w.clone(), ticket, cancel))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for (widget, ticket, cancel) in requests {
                let _guard = ticket.enter();
                if network::canceled() {
                    continue;
                }
                let card = state.providers.fetch_cancelable(&widget, cancel.clone());
                let snapshot = if let Ok(mut engine) = state.engine.lock() {
                    // Ignore a response if the user changed configuration during its request.
                    if ticket.current()
                        && !cancel.load(std::sync::atomic::Ordering::Acquire)
                        && !engine.settings.preferences.offline_mode
                        && core::kind_allowed(&engine.settings.preferences.bot_model, &widget.kind)
                        && engine
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
            app_update::initialize(app.handle());
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            // An already enabled integration follows app updates without changing Claude settings.
            let bridge = app.path().app_config_dir()?.join("claude-bridge");
            std::thread::spawn(move || {
                let _ = claude_bridge::refresh_installed_helper(&bridge);
            });
            let settings = persistence::load(&settings_path).map_err(std::io::Error::other)?;
            let background = std::env::args().any(|a| a == "--background");
            let mut engine = Engine::new(settings, Instant::now());
            engine.set_host_display_asleep(host_display::asleep());
            engine.tick(Instant::now());
            let providers = Arc::new(
                Providers::new(app.path().app_config_dir()?.join("providers"))
                    .map_err(std::io::Error::other)?,
            );
            providers.set_policy(&engine.settings.preferences.bot_model, engine.settings.preferences.offline_mode);
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
                support_report: Mutex::new(None),
                support_pattern: Mutex::new(None),
                events: Mutex::new(std::collections::VecDeque::new()),
                pending: Mutex::new(std::collections::HashMap::new()),
                standalone: Mutex::new(None),
                save_generation: std::sync::atomic::AtomicU64::new(0),
            });
            if let Some(window) = app.get_webview_window("main") {
                window.set_size(tauri::LogicalSize::new(440.0, 760.0))?;
            }

            let open = MenuItem::with_id(app, "open", "Open DroidFun App", true, None::<&str>)?;
            let pause =
                MenuItem::with_id(app, "pause", "Pause or resume cycle", true, None::<&str>)?;
            let next = MenuItem::with_id(app, "next", "Next widget", true, None::<&str>)?;
            let face = MenuItem::with_id(app, "face", "Show face", true, None::<&str>)?;
            let display =
                MenuItem::with_id(app, "display", "Display on or off", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit DroidFun App", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&open, &pause, &next, &face, &display, &settings, &quit],
            )?;
            let tray = TrayIconBuilder::with_id("deskbot")
                .menu(&menu)
                .tooltip("DroidFun App")
                .icon(tauri::image::Image::from_bytes(if cfg!(target_os = "macos") {
                    include_bytes!("../icons/tray.png").as_slice()
                } else {
                    include_bytes!("../icons/64x64.png").as_slice()
                })?)
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
            std::thread::spawn(move || {
                let mut next_display_check = Instant::now();
                let mut display_asleep = false;
                loop {
                    std::thread::sleep(Duration::from_micros(16_667));
                    let now = Instant::now();
                    if now >= next_display_check {
                        display_asleep = host_display::asleep();
                        next_display_check = now + Duration::from_secs(1);
                    }
                    let state = handle.state::<AppState>();
                    let snapshot = {
                        let Ok(mut engine) = state.engine.lock() else { continue; };
                        let power_changed = engine.set_host_display_asleep(display_asleep);
                        let changed = engine.tick(now);
                        (power_changed || changed).then(|| engine.snapshot())
                    };
                    if let Some(snapshot) = snapshot {
                        publish(&handle, &snapshot);
                    }
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
                        let widgets: Vec<_> = s.settings.widgets.iter().enumerate().map(|(index,w)| serde_json::json!({"index":index,"kind":w.kind,"enabled":w.enabled,"mode":w.config.mode,"durationSeconds":w.duration_sec,"current":w.id==s.current_id,"dataState":s.data.get(&w.id).map(|d|d.state_at(now)),"fetchedAt":s.data.get(&w.id).map(|d|d.fetched_at),"expiresAt":s.data.get(&w.id).map(|d|d.expires_at),"allowanceState":s.data.get(&w.id).and_then(|d|d.values.get("allowanceState")).and_then(|v|v.as_str())})).collect();
                        let connection = state.connection.lock().ok().map(|c|serde_json::json!({"state":c.state,"firmware":c.firmware,"displayHealth":c.display_health}));
                        let metrics = state.link_metrics.lock().ok().map(|m|serde_json::to_value(&*m).unwrap_or_default());
                        let pending = state.pending.lock().ok().map(|p|p.values().map(|v|v.iter().filter(|w|w.strong_count()>0).count()).sum::<usize>());
                        let events = state.events.lock().ok().map(|e|e.clone());
                        let visible = handle.get_webview_window("main").is_some_and(|w|w.is_visible().unwrap_or(false));
                        let value = serde_json::json!({"formatVersion":1,"at":now,"pid":std::process::id(),"uptimeSeconds":state.started.elapsed().as_secs(),"controlsVisible":visible,"connection":connection,"link":metrics,"playback":s.playback,"displayOn":s.display_on,"displayReason":s.display_reason,"orientation":s.settings.preferences.orientation,"brightness":s.settings.brightness,"frameCrc":protocol::crc32(&s.frame),"widgets":widgets,"pendingRequests":pending,"connectionEvents":events,"claudeAccount":claude_account::diagnostics(),"faults":{"networkOffline":reliability::fault("network-offline"),"linkInterruption":reliability::fault("link-interruption")}});
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
            app_update::get_app_update_status,
            app_update::check_app_update,
            app_update::install_app_update,
            app_update::restart_app_after_update,
            set_preferences,
            set_bot_model,
            set_offline_mode,
            save_to_bot,
            preview_draft,
            platform_info,
            override_quiet,
            set_reduced_motion,
            reorder_widget,
            reset_deskbot,
            diagnostics_preview,
            run_support_diagnostics,
            show_support_pattern,
            record_support_observation,
            export_support_report,
            export_diagnostics,
            export_firmware,
            list_setup_drives,
            supports_display_test,
            prepare_display_test,
            prepare_robot_update,
            install_robot_firmware,
            install_display_test,
            read_display_test,
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
            rescan_robot,
            linux_usb_status,
            setup_robot_access,
            select_robot,
            rename_droid,
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
        .expect("DroidFun App could not start")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                // Ordinary Quit cannot stop a bundle replacement halfway
                // through. The explicit updater restart remains permitted.
                if app_update::installing(app) && code != Some(tauri::RESTART_EXIT_CODE) {
                    api.prevent_exit();
                    return;
                }
                let state = app.state::<AppState>();
                if code.is_none() && state.platform.tray_available {
                    // Last control window closed; explicit Quit uses exit(0).
                    api.prevent_exit();
                }
            }
        });
}

#[cfg(test)]
mod usb_restart_tests {
    use super::*;
    #[test]
    fn wall_power_saves_uploaded_images_but_does_not_freeze_scrolling_text() {
        let mut settings=Settings::default();settings.preferences.bot_model="meep".into();for w in &mut settings.widgets {w.enabled=false;}
        let mut e=Engine::new(settings,Instant::now());e.add_widget("image").unwrap();let image=e.settings.widgets.last_mut().unwrap();image.enabled=true;image.config.image_pixels="a5".repeat(1024);let expected=vec![0xa5;1024];
        e.add_widget("text").unwrap();e.settings.widgets.last_mut().unwrap().config.text_motion="scroll".into();
        let playlist=standalone_playlist(&e).unwrap();assert_eq!(playlist.frames.len(),1);assert_eq!(playlist.frames[0].frame,expected);assert!(playlist.encode().is_ok());
    }

    #[test]
    fn customer_recovery_requires_matching_recent_verified_board() {
        let now = Instant::now();
        let ticket = RecoveryTicket {path: "picoboot:ABC".into(), serial: "ABC".into(), created: now};
        assert!(ticket.matches("picoboot:ABC", Some("abc"), now));
        assert!(!ticket.matches("picoboot:OTHER", Some("ABC"), now));
        assert!(!ticket.matches("picoboot:ABC", Some("OTHER"), now));
        assert!(!ticket.matches("picoboot:ABC", None, now));
        assert!(!ticket.matches("picoboot:ABC", Some("ABC"), now + Duration::from_secs(300)));
    }
    use std::sync::atomic::{AtomicBool, Ordering};

    // Only tests exercising the real process-wide guard need serialization.
    static GUARD_TEST: Mutex<()> = Mutex::new(());

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Action {
        Dtr(bool),
        Baud(u32),
    }

    struct Port<'a> {
        cancel: &'a AtomicBool,
        actions: Vec<Action>,
        cancel_on: Option<Action>,
        fail_on: Option<Action>,
    }

    impl Port<'_> {
        fn act(&mut self, action: Action) -> Result<(), String> {
            self.actions.push(action);
            if self.cancel_on == Some(action) {
                self.cancel.store(true, Ordering::SeqCst);
            }
            if self.fail_on == Some(action) {
                Err("Driver call failed after it may have reached the board".into())
            } else {
                Ok(())
            }
        }
    }

    impl RestartPort for Port<'_> {
        fn dtr(&mut self, state: bool) -> Result<(), String> {
            self.act(Action::Dtr(state))
        }
        fn baud(&mut self, rate: u32) -> Result<(), String> {
            self.act(Action::Baud(rate))
        }
    }

    #[test]
    fn canceled_baud_call_is_disarmed_before_drop_without_requesting_reset() {
        let cancel = AtomicBool::new(false);
        let mut port = Port {
            cancel: &cancel,
            actions: Vec::new(),
            cancel_on: Some(Action::Baud(1200)),
            fail_on: None,
        };
        assert!(request_usb_restart(&mut port, &cancel, || {}).is_err());
        assert_eq!(
            port.actions,
            [Action::Dtr(true), Action::Baud(1200), Action::Baud(115200)]
        );
    }

    #[test]
    fn failed_baud_call_is_disarmed_even_if_driver_reports_an_error() {
        let cancel = AtomicBool::new(false);
        let mut port = Port {
            cancel: &cancel,
            actions: Vec::new(),
            cancel_on: None,
            fail_on: Some(Action::Baud(1200)),
        };
        assert!(request_usb_restart(&mut port, &cancel, || {}).is_err());
        assert_eq!(
            port.actions,
            [Action::Dtr(true), Action::Baud(1200), Action::Baud(115200)]
        );
    }

    #[test]
    fn cancellation_before_or_between_native_calls_stops_later_reset_actions() {
        let cancel = AtomicBool::new(true);
        let mut port = Port {
            cancel: &cancel,
            actions: Vec::new(),
            cancel_on: None,
            fail_on: None,
        };
        assert!(request_usb_restart(&mut port, &cancel, || {}).is_err());
        assert!(port.actions.is_empty());
        cancel.store(false, Ordering::SeqCst);
        port.cancel_on = Some(Action::Dtr(true));
        assert!(request_usb_restart(&mut port, &cancel, || {}).is_err());
        assert_eq!(port.actions, [Action::Dtr(true)]);
        cancel.store(false, Ordering::SeqCst);
        port.actions.clear();
        port.cancel_on = None;
        assert!(
            request_usb_restart(&mut port, &cancel, || cancel.store(true, Ordering::SeqCst))
                .is_err()
        );
        assert_eq!(
            port.actions,
            [Action::Dtr(true), Action::Baud(1200), Action::Baud(115200)]
        );
    }

    #[test]
    fn normal_restart_and_cancellation_during_reset_have_distinct_results() {
        let cancel = AtomicBool::new(false);
        let mut port = Port {
            cancel: &cancel,
            actions: Vec::new(),
            cancel_on: None,
            fail_on: None,
        };
        assert!(request_usb_restart(&mut port, &cancel, || {}).is_ok());
        assert_eq!(
            port.actions,
            [Action::Dtr(true), Action::Baud(1200), Action::Dtr(false)]
        );
        port.actions.clear();
        port.cancel_on = Some(Action::Dtr(false));
        assert!(request_usb_restart(&mut port, &cancel, || {}).is_err());
        assert_eq!(
            port.actions,
            [
                Action::Dtr(true),
                Action::Baud(1200),
                Action::Dtr(false),
                Action::Baud(115200)
            ]
        );
    }

    #[tokio::test]
    async fn timeout_does_not_release_usb_operation_guard_until_blocking_worker_exits() {
        let _test = GUARD_TEST.lock().unwrap();
        let caller = UsbOperationGuard::acquire().unwrap();
        let worker = caller.clone();
        let (release, wait) = std::sync::mpsc::channel();
        let (exited, done) = std::sync::mpsc::channel();
        let job = tokio::task::spawn_blocking(move || {
            wait.recv().unwrap();
            drop(worker);
            exited.send(()).unwrap();
        });
        assert!(tokio::time::timeout(Duration::from_millis(1), job)
            .await
            .is_err());
        drop(caller);
        assert!(UsbOperationGuard::acquire().is_err());
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
        let completed = UsbOperationGuard::acquire().unwrap();
        drop(completed);
    }

    #[test]
    fn queued_and_active_saves_hold_guard_until_completion_even_after_caller_leaves() {
        let _test = GUARD_TEST.lock().unwrap();
        for disconnect_caller in [false, true] {
            let (response, rx) = std::sync::mpsc::sync_channel(1);
            let mut queue = Some(StandaloneRequest {
                _operation: UsbOperationGuard::acquire().unwrap(),
                generation: 0,
                settings: Vec::new(),
                serial: "test-board".into(),
                playlist: protocol::StandalonePlaylist {
                    frames: Vec::new(),
                    brightness: 1,
                    orientation: 0,
                },
                cancel: Arc::new(AtomicBool::new(false)),
                response,
            });
            // Restart and another firmware operation must both be refused
            // before the connection worker has even dequeued this save.
            assert!(UsbOperationGuard::acquire().is_err());
            let active = queue.take().unwrap();
            assert!(UsbOperationGuard::acquire().is_err());
            if disconnect_caller {
                drop(rx);
                active.cancel.store(true, Ordering::Release);
                assert!(UsbOperationGuard::acquire().is_err());
                assert!(active.response.send(Err("Save canceled".into())).is_err());
            } else {
                active.response.send(Ok(1)).unwrap();
                assert_eq!(rx.recv().unwrap().unwrap(), 1);
            }
            // Sending a response is not the end of the native operation's
            // ownership; release only when the worker drops the request.
            assert!(UsbOperationGuard::acquire().is_err());
            drop(active);
            drop(UsbOperationGuard::acquire().unwrap());
        }
    }
}

#[cfg(test)]
mod offline_tests {
    use super::*;

    #[test]
    fn wall_power_playlist_contains_only_enabled_local_face_and_text_screens() {
        let mut settings = Settings::default();
        settings.preferences.bot_model = "boo".into();
        settings.preferences.offline_mode = true;
        settings.preferences.orientation = 180;
        settings.brightness = 47;
        // Even enabled demo/live clocks and data cards must never become stale
        // wall-power snapshots. Only intentionally static screens are saved.
        for widget in &mut settings.widgets {
            widget.enabled = true;
        }
        let mut engine = Engine::new(settings, Instant::now());
        engine.add_widget("text").unwrap();
        let text = engine
            .settings
            .widgets
            .iter_mut()
            .find(|w| w.kind == "text")
            .unwrap();
        text.enabled = true;
        text.config.custom_text = "HELLO".into();
        text.duration_sec = 17;
        let playlist = standalone_playlist(&engine).unwrap();
        assert_eq!(playlist.frames.len(), 2);
        assert_eq!(playlist.brightness, 47);
        assert_eq!(playlist.orientation, 180);
        assert_eq!(playlist.frames[1].duration_sec, 17);
        assert!(playlist.frames.iter().all(|f| f.frame.len() == 1024));
        assert!(playlist.encode().is_ok());
        engine
            .settings
            .widgets
            .iter_mut()
            .find(|w| w.kind == "text")
            .unwrap()
            .enabled = false;
        assert_eq!(standalone_playlist(&engine).unwrap().frames.len(), 1);
    }

    #[test]
    fn offline_mode_keeps_local_computer_stats_usable_for_general_bots() {
        let mut settings = Settings::default();
        settings.preferences.bot_model = "boo".into();
        settings.preferences.offline_mode = true;
        let mut engine = Engine::new(settings, Instant::now());
        engine.add_widget("cpu").unwrap();
        let widget = engine
            .settings
            .widgets
            .iter()
            .find(|w| w.kind == "cpu")
            .unwrap()
            .clone();
        engine.set_enabled(&widget.id, true).unwrap();
        let card = local_widgets::cpu_card(&widget, core::unix_now(), 25.0, &[25.0]);
        engine.set_data(&widget.id, card);
        assert!(engine.show_now(&widget.id).is_ok());
        assert_eq!(engine.snapshot().current_id, widget.id);
        assert!(engine.render_widget(&widget.id).iter().any(|b| *b != 0));
    }

    #[test]
    fn saved_face_is_stable_across_animation_and_blink_phases() {
        let mut settings = Settings::default();
        settings.preferences.bot_model = "boo".into();
        let start = Instant::now();
        let mut engine = Engine::new(settings, start);
        let original = standalone_playlist(&engine).unwrap().frames[0]
            .frame
            .clone();
        for millis in [200, 4200, 4900, 5000, 6200, 10_000] {
            engine.tick(start + Duration::from_millis(millis));
            assert_eq!(
                standalone_playlist(&engine).unwrap().frames[0].frame,
                original
            );
        }
    }

    #[test]
    fn wall_power_save_requires_a_supported_bot_and_at_least_one_static_screen() {
        let mut engine = Engine::new(Settings::default(), Instant::now());
        assert!(standalone_playlist(&engine).unwrap_err().contains("Meep"));
        engine.set_bot_model("claw").unwrap();
        assert!(standalone_playlist(&engine).unwrap_err().contains("Claw"));
        engine.set_bot_model("meep").unwrap();
        for widget in &mut engine.settings.widgets {
            widget.enabled = false;
        }
        assert!(standalone_playlist(&engine)
            .unwrap_err()
            .contains("at least one"));
    }
}
