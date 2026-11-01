//! Private live-text monitor and USB display bridge. Queue order and SMS stay on the backend.
use crate::{
    connection::{self, Link, RobotPort},
    core::{self, SourceConfig},
    UsbOperationGuard,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct View {
    running: bool,
    released: bool,
    connection: String,
    error: String,
    ports: Vec<RobotPort>,
    target: Option<String>,
    frame: Vec<u8>,
    current: Option<Current>,
    backend: Option<Value>,
}
#[derive(Clone, Serialize)]
pub(crate) struct Current {
    ticket: u64,
    text: String,
}
struct Inner {
    view: View,
    cancel: Arc<AtomicBool>,
    last_fetch: Instant,
}
pub(crate) struct LiveState {
    inner: Mutex<Inner>,
    control: AtomicBool,
    fetching: AtomicBool,
}
pub(crate) fn initialize(app: &mut tauri::App) {
    app.manage(LiveState {
        inner: Mutex::new(Inner {
            view: View {
                running: false,
                released: true,
                connection: "Display stopped".into(),
                error: String::new(),
                ports: Vec::new(),
                target: None,
                frame: core::render(None, true),
                current: None,
                backend: None,
            },
            cancel: Arc::new(AtomicBool::new(false)),
            last_fetch: Instant::now() - Duration::from_secs(10),
        }),
        control: AtomicBool::new(false),
        fetching: AtomicBool::new(false),
    });
}
fn change(app: &AppHandle, f: impl FnOnce(&mut View)) {
    let state = app.state::<LiveState>();
    if let Ok(mut s) = state.inner.lock() {
        f(&mut s.view);
        let _ = app.emit("workshop-live-state", s.view.clone());
    };
}
fn snapshot(app: &AppHandle) -> Result<View, String> {
    app.state::<LiveState>()
        .inner
        .lock()
        .map(|s| s.view.clone())
        .map_err(|_| "Live text controls are unavailable".into())
}
fn token(app: &AppHandle) -> Result<String, String> {
    app.state::<crate::workshop_registry::Store>()
        .token()
        .ok_or("Connect Workshop to your dashboard in Droid list first.".into())
}
async fn backend(
    app: &AppHandle,
    action: Option<&str>,
    data: Option<&Value>,
) -> Result<Value, String> {
    let token = token(app)?;
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| "Could not prepare live connection")?;
    let path = action.map(|s| format!("/{s}")).unwrap_or_default();
    let url = format!("https://droidfun.io/api/workshop/live-text{path}");
    let mut request = if data.is_some() {
        client.post(url)
    } else {
        client.get(url)
    }
    .bearer_auth(token);
    if let Some(data) = data {
        request = request.json(data);
    }
    let mut response = request
        .send()
        .await
        .map_err(|_| "Could not reach the live backend. Check your internet connection.")?;
    let status = response.status();
    let mut bytes = Vec::new();
    while let Some(part) = response
        .chunk()
        .await
        .map_err(|_| "Could not read live status")?
    {
        if bytes.len() + part.len() > 64 * 1024 {
            return Err("Live status was too large".into());
        }
        bytes.extend_from_slice(&part);
    }
    let result: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "The live backend returned an invalid response")?;
    if !status.is_success() {
        return Err(if status == 401 {
            "Connect Workshop using a new dashboard code in Droid list.".into()
        } else {
            result["error"]
                .as_str()
                .filter(|s| s.len() <= 250)
                .unwrap_or("The live request could not finish.")
                .into()
        });
    }
    Ok(result)
}
fn remember(app: &AppHandle, value: Value) {
    let state = app.state::<LiveState>();
    if let Ok(mut s) = state.inner.lock() {
        s.view.backend = Some(value);
        s.last_fetch = Instant::now();
        s.view.error.clear();
    };
}
struct Flag<'a>(&'a AtomicBool);
impl Drop for Flag<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
#[tauri::command]
pub(crate) async fn workshop_live_state(app: AppHandle) -> Result<View, String> {
    let state = app.state::<LiveState>();
    let due = state
        .inner
        .lock()
        .is_ok_and(|s| !s.view.running && s.last_fetch.elapsed() > Duration::from_secs(4));
    if due
        && state
            .fetching
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    {
        let _fetch = Flag(&state.fetching);
        let ports = connection::robot_ports().unwrap_or_default();
        change(&app, |s| s.ports = ports);
        match backend(&app, None, None).await {
            Ok(value) => remember(&app, value),
            Err(error) => change(&app, |s| s.error = error),
        }
    }
    snapshot(&app)
}
async fn stop(app: &AppHandle) -> Result<(), String> {
    app.state::<LiveState>()
        .inner
        .lock()
        .map_err(|_| "Live controls are unavailable")?
        .cancel
        .store(true, Ordering::SeqCst);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            if snapshot(&handle)?.released {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(
                    "The live display is still closing. Keep Workshop open and try again.".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    })
    .await
    .map_err(|_| "Could not stop the live display".to_string())?
}
#[tauri::command]
pub(crate) async fn workshop_live_action(
    action: String,
    value: Option<Value>,
    app: AppHandle,
) -> Result<Value, String> {
    let state = app.state::<LiveState>();
    state
        .control
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "Wait for the current live action to finish")?;
    let _control = Flag(&state.control);
    let value = value.unwrap_or(Value::Null);
    match action.as_str() {
        "start" => {
            if snapshot(&app)?.running {
                return Ok(json!({"view":snapshot(&app)?}));
            }
            token(&app)?;
            let guard = UsbOperationGuard::acquire()?;
            crate::showcase::require_board_mode(&app)?;
            if !crate::workshop::can_show_screens(&app) {
                return Err(
                    "Finish the board’s preparation checks before starting a live display.".into(),
                );
            }
            let serial = value["serial"]
                .as_str()
                .ok_or("Choose a connected USB droid")?;
            let bridge = value["bridge"]
                .as_str()
                .filter(|s| valid_bridge(s))
                .ok_or("Invalid display connection")?
                .to_string();
            let port = connection::robot_ports()?
                .into_iter()
                .find(|p| p.serial == serial)
                .ok_or("That droid is no longer connected")?;
            let cancel = Arc::new(AtomicBool::new(false));
            {
                let mut s = state
                    .inner
                    .lock()
                    .map_err(|_| "Live controls are unavailable")?;
                s.cancel = cancel.clone();
                s.view.running = true;
                s.view.released = false;
                s.view.target = Some(serial.into());
                s.view.connection = "Connecting USB…".into();
                s.view.error.clear();
            }
            let handle = app.clone();
            std::thread::spawn(move || run_display(handle, port, bridge, cancel, guard));
        }
        "stop" => {
            stop(&app).await?;
        }
        "numbers" => return backend(&app, Some("numbers"), Some(&value)).await,
        "connect" | "disconnect" | "session" | "pause" | "skip" => {
            let response = backend(&app, Some(&action), Some(&value)).await?;
            remember(&app, response);
        }
        _ => return Err("Unknown live action".into()),
    }
    Ok(json!({"view":snapshot(&app)?}))
}
fn valid_bridge(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
#[derive(Clone)]
struct Item {
    ticket: u64,
    text: String,
    lease: String,
    seconds: u64,
    started: Instant,
}
fn parse_item(value: &Value) -> Result<Option<Item>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let ticket = value["ticket"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("Invalid live ticket")?;
    let text = value["text"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 120 && s.bytes().all(|b| (32..=126).contains(&b)))
        .ok_or("Invalid live display text")?
        .to_string();
    let lease = value["lease"]
        .as_str()
        .filter(|s| valid_bridge(s))
        .ok_or("Invalid live display lease")?
        .to_string();
    let seconds = value["duration"]
        .as_u64()
        .filter(|s| (6..=32).contains(s))
        .ok_or("Invalid live display duration")?;
    Ok(Some(Item {
        ticket,
        text,
        lease,
        seconds,
        started: Instant::now(),
    }))
}
fn live_frame(text: &str, phase: u64) -> Result<Vec<u8>, String> {
    let config = SourceConfig {
        custom_text: text.into(),
        text_size: 1,
        text_motion: "scroll".into(),
        scroll_speed: "normal".into(),
        text_valign: "center".into(),
        text_border: false,
        ..Default::default()
    };
    core::render_marquee(&config, phase, false)
}
fn run_display(
    app: AppHandle,
    port: RobotPort,
    bridge: String,
    cancel: Arc<AtomicBool>,
    guard: Arc<UsbOperationGuard>,
) {
    let mut link = None;
    let result: Result<(), String> = (|| {
        let connected = Link::connect(&port)?;
        link = Some(connected);
        change(&app, |s| {
            s.connection = "USB connected · waiting for texts".into()
        });
        let mut item: Option<Item> = None;
        let mut poll = Instant::now() - Duration::from_secs(5);
        let mut emitted = Instant::now();
        while !cancel.load(Ordering::SeqCst) {
            if poll.elapsed() >= Duration::from_secs(2) {
                let status = tauri::async_runtime::block_on(backend(
                    &app,
                    Some("heartbeat"),
                    Some(&json!({"bridge":bridge})),
                ))?;
                let paused = status["paused"].as_bool().unwrap_or(true);
                if item.as_ref().is_some_and(|i| {
                    paused || status["playing"]["ticket"].as_u64() != Some(i.ticket)
                }) {
                    item = None;
                }
                remember(&app, status);
                if item.is_none() && !paused {
                    let claim = tauri::async_runtime::block_on(backend(
                        &app,
                        Some("claim"),
                        Some(&json!({"bridge":bridge})),
                    ))?;
                    item = parse_item(&claim["item"])?;
                }
                poll = Instant::now();
            }
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            if item
                .as_ref()
                .is_some_and(|i| i.started.elapsed() >= Duration::from_secs(i.seconds))
            {
                let shown = item.take().unwrap();
                // An acknowledged idle frame ends this display before the backend advances FIFO.
                link.as_mut()
                    .unwrap()
                    .update(&core::render(None, true), true, 80, 0)?;
                tauri::async_runtime::block_on(backend(
                    &app,
                    Some("complete"),
                    Some(&json!({"bridge":bridge,"ticket":shown.ticket,"lease":shown.lease})),
                ))?;
                poll = Instant::now() - Duration::from_secs(3);
            }
            let frame = match &item {
                Some(i) => live_frame(&i.text, i.started.elapsed().as_millis() as u64)?,
                None => core::render(None, true),
            };
            link.as_mut().unwrap().update(&frame, true, 80, 0)?;
            if emitted.elapsed() > Duration::from_millis(200) {
                let current = item.as_ref().map(|i| Current {
                    ticket: i.ticket,
                    text: i.text.clone(),
                });
                change(&app, |s| {
                    s.frame = frame;
                    s.current = current;
                    s.connection = if s.current.is_some() {
                        "Showing live text"
                    } else {
                        "USB connected · waiting for texts"
                    }
                    .into();
                });
                emitted = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    })();
    // Never leave a stale sentence on camera after an internet/USB failure or explicit stop.
    if let Some(link) = link.as_mut() {
        let _ = link.update(&core::render(None, true), true, 80, 0);
    }
    drop(link);
    drop(guard);
    let _ = tauri::async_runtime::block_on(backend(
        &app,
        Some("release"),
        Some(&json!({"bridge":bridge})),
    ));
    change(&app, |s| {
        s.running = false;
        s.released = true;
        s.current = None;
        s.frame = core::render(None, true);
        s.connection = "Display stopped".into();
        if let Err(e) = result {
            s.error = e;
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_frames_scroll_with_no_provider_or_sender_metadata() {
        let a = live_frame("Hello from the livestream!", 1000).unwrap();
        let b = live_frame("Hello from the livestream!", 1500).unwrap();
        assert_eq!(a.len(), 1024);
        assert_ne!(a, b);
        assert!(a.iter().any(|b| *b != 0));
    }
    #[test]
    fn remote_claims_are_bounded_and_never_accept_invalid_text_or_durations() {
        let valid = json!({"ticket":1,"text":"One sentence.","lease":"01234567-89ab-cdef-0123-456789abcdef","duration":8});
        assert!(parse_item(&valid).is_ok());
        for key in ["ticket", "text", "lease", "duration"] {
            let mut bad = valid.clone();
            bad[key] = Value::Null;
            assert!(parse_item(&bad).is_err());
        }
        for text in [
            "a".repeat(121),
            "text\nwith newline".into(),
            "hello 👋".into(),
        ] {
            let mut bad = valid.clone();
            bad["text"] = json!(text);
            assert!(parse_item(&bad).is_err());
        }
        assert!(parse_item(&Value::Null).unwrap().is_none());
    }
}
