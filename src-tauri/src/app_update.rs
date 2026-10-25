//! Manual, signed application updates. The checked release and verified package
//! stay in Rust; the webview cannot supply a URL, signature, version, or path.
use crate::{AppState, UsbOperationGuard};
use serde::Serialize;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Config, Update, UpdaterExt};

const EVENT: &str = "app-update-progress";
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_NOTES_CHARS: usize = 8_000;
const OFFLINE: &str =
    "App updates are paused in offline mode. Turn off offline mode to check or download an update.";
const UNCONFIGURED: &str = "Signed app updates are not configured in this build. Download the latest app from DroidFun’s releases page.";
const MANUAL: &str = "Download the latest Linux package from DroidFun’s releases page and install it with your package manager.";
const BUSY: &str = "An app update is already running. Wait for it to finish.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    Idle,
    Unconfigured,
    Unsupported,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Ready,
    Restarting,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppUpdateStatus {
    current_version: String,
    platform: &'static str,
    supported: bool,
    phase: Phase,
    version: Option<String>,
    date: Option<String>,
    notes: Option<String>,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    ready_to_restart: bool,
    message: Option<String>,
}

// Keeping transitions independent of Tauri makes stale/repeated commands and
// failed verification testable without a webview, network, or installer.
struct Session<U> {
    status: AppUpdateStatus,
    checked: Option<U>,
    verified_package: Option<Vec<u8>>,
}

impl<U: Clone> Session<U> {
    fn new(version: String, platform: &'static str, configured: bool) -> Self {
        let supported = matches!(platform, "macos" | "windows");
        let (phase, message) = if !supported {
            (Phase::Unsupported, Some(MANUAL.into()))
        } else if !configured {
            (Phase::Unconfigured, Some(UNCONFIGURED.into()))
        } else {
            (Phase::Idle, None)
        };
        Self {
            status: AppUpdateStatus {
                current_version: version,
                platform,
                supported,
                phase,
                version: None,
                date: None,
                notes: None,
                downloaded_bytes: 0,
                total_bytes: None,
                ready_to_restart: false,
                message,
            },
            checked: None,
            verified_package: None,
        }
    }

    fn begin_check(&mut self) -> Result<(), String> {
        match self.status.phase {
            Phase::Checking | Phase::Downloading | Phase::Restarting => return Err(BUSY.into()),
            Phase::Ready => {
                return Err("An update is ready. Restart to install the downloaded version.".into())
            }
            Phase::Unconfigured => return Err(UNCONFIGURED.into()),
            Phase::Unsupported => return Err(MANUAL.into()),
            _ => {}
        }
        // A new check invalidates the previous selection even if it fails.
        self.checked = None;
        self.verified_package = None;
        self.status.phase = Phase::Checking;
        self.status.version = None;
        self.status.date = None;
        self.status.notes = None;
        self.status.downloaded_bytes = 0;
        self.status.total_bytes = None;
        self.status.ready_to_restart = false;
        self.status.message = None;
        Ok(())
    }

    fn begin_download(&mut self) -> Result<U, String> {
        if self.status.phase != Phase::Available {
            return Err(
                if matches!(
                    self.status.phase,
                    Phase::Checking | Phase::Downloading | Phase::Restarting
                ) {
                    BUSY.into()
                } else {
                    "Check for an available app update before downloading it.".into()
                },
            );
        }
        let update = self
            .checked
            .clone()
            .ok_or("Check for an app update again.")?;
        self.status.phase = Phase::Downloading;
        self.status.message = None;
        self.status.downloaded_bytes = 0;
        self.status.total_bytes = None;
        Ok(update)
    }

    fn downloaded(&mut self, verified_package: Vec<u8>) {
        self.status.downloaded_bytes = verified_package.len() as u64;
        self.status.total_bytes = Some(verified_package.len() as u64);
        self.verified_package = Some(verified_package);
        self.status.phase = Phase::Ready;
        self.status.ready_to_restart = true;
        self.status.message =
            Some("The update is verified and ready to install. Restart when you’re ready.".into());
    }

    fn begin_restart(&mut self) -> Result<(U, Vec<u8>), String> {
        if self.status.phase != Phase::Ready || !self.status.ready_to_restart {
            return Err("Download and verify an app update before restarting.".into());
        }
        let update = self
            .checked
            .clone()
            .ok_or("Check for an app update again.")?;
        let bytes = self
            .verified_package
            .take()
            .ok_or("Download the app update again.")?;
        self.status.phase = Phase::Restarting;
        self.status.ready_to_restart = false;
        self.status.message = Some("Installing the verified update. DroidFun will restart.".into());
        Ok((update, bytes))
    }

    fn fail(&mut self, message: &str) {
        self.checked = None;
        self.verified_package = None;
        self.status.phase = Phase::Error;
        self.status.ready_to_restart = false;
        self.status.message = Some(message.chars().take(400).collect());
    }
}

pub(crate) struct AppUpdates(Mutex<Session<Update>>);

fn configured(value: Option<&serde_json::Value>) -> bool {
    let Some(value) = value else { return false };
    let Ok(config) = serde_json::from_value::<Config>(value.clone()) else {
        return false;
    };
    !config.pubkey.trim().is_empty()
        && config.endpoints.len() == 1
        && config.endpoints.iter().all(|url| secure_url(url.as_str()))
        && config.require_signed_version
        && !config.allow_downgrades
        && !config.dangerous_insecure_transport_protocol
        && !config.dangerous_accept_invalid_certs
        && !config.dangerous_accept_invalid_hostnames
}

fn secure_url(value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

pub(crate) fn initialize(app: &AppHandle) {
    let configured = configured(app.config().plugins.0.get("updater"));
    // Registering an incomplete plugin can fail app startup. Until a public key
    // and feed are configured, the custom commands expose a useful status only.
    let enabled = configured
        && matches!(std::env::consts::OS, "macos" | "windows")
        && app
            .plugin(tauri_plugin_updater::Builder::new().build())
            .is_ok();
    app.manage(AppUpdates(Mutex::new(Session::new(
        app.package_info().version.to_string(),
        std::env::consts::OS,
        enabled,
    ))));
}

fn online(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let engine = state
        .engine
        .lock()
        .map_err(|_| "App settings are busy. Try again.".to_string())?;
    if engine.settings.preferences.offline_mode {
        Err(OFFLINE.into())
    } else {
        Ok(())
    }
}

async fn while_online<T>(
    request: impl std::future::Future<Output = Result<T, String>>,
    policy: impl Fn() -> Result<(), String>,
    timeout: Duration,
    timeout_message: &str,
) -> Result<T, String> {
    // Check before the network future is ever polled, cancel it when offline
    // mode changes, and reject a result that arrives after that change.
    policy()?;
    let response = tokio::select! {
        result = tokio::time::timeout(timeout, request) => result.map_err(|_| timeout_message.to_string())?,
        error = async {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if let Err(error) = policy() { break error; }
            }
        } => Err(error),
    }?;
    policy()?;
    Ok(response)
}

pub(crate) fn installing(app: &AppHandle) -> bool {
    app.try_state::<AppUpdates>().is_some_and(|state| {
        state
            .0
            .lock()
            .is_ok_and(|s| s.status.phase == Phase::Restarting)
    })
}

fn status(app: &AppHandle) -> Result<AppUpdateStatus, String> {
    app.state::<AppUpdates>()
        .0
        .lock()
        .map(|s| s.status.clone())
        .map_err(|_| "App update state is unavailable. Restart DroidFun and try again.".into())
}

fn emit(app: &AppHandle) {
    if let Ok(status) = status(app) {
        let _ = app.emit(EVENT, status);
    }
}

fn failed(app: &AppHandle, message: String) -> String {
    if let Ok(mut session) = app.state::<AppUpdates>().0.lock() {
        session.fail(&message);
    }
    emit(app);
    message
}

fn updater_error(error: &tauri_plugin_updater::Error, installing: bool) -> String {
    use tauri_plugin_updater::Error;
    // Never expose untrusted server bodies, signature text, or filesystem paths
    // as UI errors. These messages also stay bounded for progress events.
    match error {
        Error::Minisign(_) | Error::Base64(_) | Error::SignatureUtf8(_)
        | Error::SignedVersionMismatch { .. } | Error::MissingSignedVersion =>
            "The update’s signature or signed version could not be verified. Nothing was installed. Check for updates again later.",
        Error::TargetsNotFound(_) | Error::TargetNotFound(_) | Error::UnsupportedArch | Error::UnsupportedOs =>
            "There is no app update package for this computer. Download the latest compatible app from DroidFun’s releases page.",
        Error::Reqwest(_) | Error::Network(_) | Error::ReleaseNotFound =>
            "Could not reach the app update service or download its package. Check your connection and try again.",
        Error::EmptyEndpoints | Error::InsecureTransportProtocol => UNCONFIGURED,
        Error::PreviousAppNotRestored(_) =>
            "The update could not be installed or rolled back. Download and reinstall DroidFun from its releases page.",
        _ if installing =>
            "The app update could not be installed. Keep DroidFun in your Applications or installed apps folder, then check for updates again.",
        _ => "The app update service returned an invalid update. Nothing was installed. Try again later.",
    }.into()
}

#[tauri::command]
pub(crate) fn get_app_update_status(
    state: State<'_, AppUpdates>,
) -> Result<AppUpdateStatus, String> {
    state
        .0
        .lock()
        .map(|s| s.status.clone())
        .map_err(|_| "App update state is unavailable.".into())
}

#[tauri::command]
pub(crate) async fn check_app_update(app: AppHandle) -> Result<AppUpdateStatus, String> {
    online(&app)?;
    {
        let state = app.state::<AppUpdates>();
        let mut session = state
            .0
            .lock()
            .map_err(|_| "App update state is unavailable.")?;
        session.begin_check()?;
    }
    emit(&app);
    let updater = app
        .updater_builder()
        .timeout(CHECK_TIMEOUT)
        .version_comparator(|current, release| release.version > current)
        .configure_client(|client| {
            client
                .https_only(true)
                .connect_timeout(Duration::from_secs(10))
        })
        .build()
        .map_err(|e| failed(&app, updater_error(&e, false)))?;
    let response = while_online(
        async { updater.check().await.map_err(|e| updater_error(&e, false)) },
        || online(&app),
        CHECK_TIMEOUT,
        "The app update check timed out. Check your connection and try again.",
    )
    .await
    .map_err(|message| failed(&app, message))?;
    if let Some(update) = &response {
        if !secure_url(update.download_url.as_str()) || update.signature.trim().is_empty() {
            return Err(failed(&app, "The update service did not provide a secure, signed package. Nothing was installed.".into()));
        }
    }
    {
        let state = app.state::<AppUpdates>();
        let mut session = state
            .0
            .lock()
            .map_err(|_| "App update state is unavailable.")?;
        match response {
            Some(mut update) => {
                update.timeout = Some(DOWNLOAD_TIMEOUT);
                session.status.phase = Phase::Available;
                session.status.version = Some(update.version.chars().take(128).collect());
                session.status.date = update
                    .date
                    .and_then(|date| chrono::DateTime::from_timestamp(date.unix_timestamp(), 0))
                    .map(|date| date.to_rfc3339());
                session.status.notes = update
                    .body
                    .as_ref()
                    .map(|notes| notes.chars().take(MAX_NOTES_CHARS).collect());
                session.checked = Some(update);
            }
            None => {
                session.status.phase = Phase::UpToDate;
            }
        }
    }
    emit(&app);
    status(&app)
}

#[tauri::command]
pub(crate) async fn install_app_update(app: AppHandle) -> Result<AppUpdateStatus, String> {
    online(&app)?;
    let _operation = UsbOperationGuard::acquire()?;
    let update = {
        let state = app.state::<AppUpdates>();
        let mut session = state
            .0
            .lock()
            .map_err(|_| "App update state is unavailable.")?;
        session.begin_download()?
    };
    emit(&app);
    let mut last_event = Instant::now();
    let (size_tx, size_rx) = tokio::sync::oneshot::channel::<()>();
    let mut size_tx = Some(size_tx);
    let download = update.download(
        |chunk, total| {
            let state = app.state::<AppUpdates>();
            if let Ok(mut session) = state.0.lock() {
                session.status.downloaded_bytes =
                    session.status.downloaded_bytes.saturating_add(chunk as u64);
                session.status.total_bytes = total;
                if session.status.downloaded_bytes > MAX_PACKAGE_BYTES
                    || total.is_some_and(|n| n > MAX_PACKAGE_BYTES)
                {
                    if let Some(tx) = size_tx.take() {
                        let _ = tx.send(());
                    }
                }
            }
            if last_event.elapsed() >= Duration::from_millis(250) {
                emit(&app);
                last_event = Instant::now();
            }
        },
        || {},
    );
    let bytes = while_online(async {
        tokio::select! {
            biased;
            _ = size_rx => Err("The app update package is too large. Nothing was installed.".into()),
            result = download => match result {
                Ok(bytes) if bytes.len() as u64 <= MAX_PACKAGE_BYTES => Ok(bytes),
                Ok(_) => Err("The app update package is too large. Nothing was installed.".into()),
                Err(error) => Err(updater_error(&error, false)),
            },
        }
    }, || online(&app), DOWNLOAD_TIMEOUT,
        "The app update download timed out. Check your connection and try again.",
    ).await.map_err(|message| failed(&app, message))?;
    // Update::download returns bytes only after signature + signed-version
    // verification. Never mark ready in its pre-verification finish callback.
    {
        let state = app.state::<AppUpdates>();
        state
            .0
            .lock()
            .map_err(|_| "App update state is unavailable.")?
            .downloaded(bytes);
    }
    emit(&app);
    status(&app)
}

#[tauri::command]
pub(crate) async fn restart_app_after_update(app: AppHandle) -> Result<(), String> {
    let operation = UsbOperationGuard::acquire()?;
    let (update, bytes) = {
        let state = app.state::<AppUpdates>();
        let mut session = state
            .0
            .lock()
            .map_err(|_| "App update state is unavailable.")?;
        session.begin_restart()?
    };
    emit(&app);
    // This is the only installation path. Windows' installer exits the app;
    // macOS returns after replacing the bundle, then explicitly restarts below.
    // The guard lives inside the worker even if the invoking webview goes away.
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        match update.install(bytes) {
            Ok(()) => worker_app.restart(),
            Err(error) => Err(failed(&worker_app, updater_error(&error, true))),
        }
    })
    .await
    .map_err(|_| {
        failed(
            &app,
            "The app installer stopped unexpectedly. Check for updates again.".into(),
        )
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> serde_json::Value {
        json!({"pubkey":"public-test-key", "endpoints":["https://example.com/latest.json"], "requireSignedVersion":true})
    }

    fn available() -> Session<u8> {
        let mut s = Session::new("0.4.2".into(), "macos", true);
        s.begin_check().unwrap();
        s.status.phase = Phase::Available;
        s.status.version = Some("0.4.3".into());
        s.checked = Some(43);
        s
    }

    #[test]
    fn absent_or_unsafe_config_never_enables_updates() {
        assert!(!configured(None));
        assert!(!configured(Some(&json!({}))));
        assert!(configured(Some(&config())));
        for (key, value) in [
            ("requireSignedVersion", false),
            ("allowDowngrades", true),
            ("dangerousInsecureTransportProtocol", true),
            ("dangerousAcceptInvalidCerts", true),
            ("dangerousAcceptInvalidHostnames", true),
        ] {
            let mut c = config();
            c[key] = json!(value);
            assert!(!configured(Some(&c)), "{key}");
        }
        for endpoint in [
            "http://example.com/latest.json",
            "file:///tmp/update",
            "https://user:pass@example.com/update",
        ] {
            let mut c = config();
            c["endpoints"] = json!([endpoint]);
            assert!(!configured(Some(&c)));
        }
        let mut c = config();
        c["pubkey"] = json!("");
        assert!(!configured(Some(&c)));
    }

    #[test]
    fn manual_and_unconfigured_states_do_not_start_checks() {
        let mut linux: Session<u8> = Session::new("0.4.2".into(), "linux", true);
        assert_eq!(linux.status.phase, Phase::Unsupported);
        assert!(!linux.status.supported);
        assert!(linux.begin_check().is_err());
        let mut mac: Session<u8> = Session::new("0.4.2".into(), "macos", false);
        assert_eq!(mac.status.phase, Phase::Unconfigured);
        assert!(mac.begin_check().is_err());
    }

    #[test]
    fn failed_new_check_cannot_install_a_stale_release() {
        let mut s = available();
        s.begin_check().unwrap();
        assert!(s.checked.is_none());
        s.fail("Network failed");
        assert!(s.begin_download().is_err());
        assert!(!s.status.ready_to_restart);
    }

    #[test]
    fn failed_verification_never_becomes_restart_ready() {
        let mut s = available();
        assert_eq!(s.begin_download().unwrap(), 43);
        assert!(s.begin_restart().is_err());
        assert!(s.begin_check().is_err());
        assert!(s.begin_download().is_err());
        s.fail("Signature did not verify");
        assert!(s.begin_restart().is_err());
        assert!(s.verified_package.is_none());
    }

    #[test]
    fn explicit_restart_consumes_only_the_checked_verified_package_once() {
        let mut s = available();
        s.begin_download().unwrap();
        s.downloaded(vec![1, 2, 3]);
        assert_eq!(s.status.phase, Phase::Ready);
        assert!(s.status.ready_to_restart);
        assert!(s.begin_check().is_err());
        assert_eq!(s.begin_restart().unwrap(), (43, vec![1, 2, 3]));
        assert!(s.begin_restart().is_err());
        assert!(!s.status.ready_to_restart);
    }

    #[test]
    fn error_messages_do_not_echo_untrusted_server_or_signature_content() {
        let secret = "untrusted-url-or-token";
        for error in [
            tauri_plugin_updater::Error::Network(secret.into()),
            tauri_plugin_updater::Error::SignatureUtf8(secret.into()),
            tauri_plugin_updater::Error::SignedVersionMismatch {
                signed: secret.into(),
                announced: secret.into(),
            },
        ] {
            let message = updater_error(&error, false);
            assert!(!message.contains(secret));
            assert!(message.len() < 400);
        }
    }

    #[tokio::test]
    async fn offline_policy_prevents_network_polling_and_discards_late_results() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let polled = AtomicBool::new(false);
        let offline = AtomicBool::new(true);
        let policy = || {
            if offline.load(Ordering::SeqCst) {
                Err(OFFLINE.into())
            } else {
                Ok(())
            }
        };
        let request = async {
            polled.store(true, Ordering::SeqCst);
            Ok(7)
        };
        assert_eq!(
            while_online(request, policy, CHECK_TIMEOUT, "Timeout").await,
            Err(OFFLINE.into())
        );
        assert!(!polled.load(Ordering::SeqCst));
        offline.store(false, Ordering::SeqCst);
        let request = async {
            offline.store(true, Ordering::SeqCst);
            Ok(7)
        };
        assert_eq!(
            while_online(request, policy, CHECK_TIMEOUT, "Timeout").await,
            Err(OFFLINE.into())
        );
    }

    #[tokio::test]
    async fn offline_change_cancels_inflight_work_and_timeout_drops_request() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        struct DropFlag(Arc<AtomicBool>);
        impl Drop for DropFlag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let started = AtomicBool::new(false);
        let flag = DropFlag(dropped.clone());
        let request = async {
            let _flag = flag;
            started.store(true, Ordering::SeqCst);
            std::future::pending::<Result<(), String>>().await
        };
        let policy = || {
            if started.load(Ordering::SeqCst) {
                Err(OFFLINE.into())
            } else {
                Ok(())
            }
        };
        assert_eq!(
            while_online(request, policy, CHECK_TIMEOUT, "Timeout").await,
            Err(OFFLINE.into())
        );
        assert!(dropped.load(Ordering::SeqCst));
        dropped.store(false, Ordering::SeqCst);
        let flag = DropFlag(dropped.clone());
        let request = async {
            let _flag = flag;
            std::future::pending::<Result<(), String>>().await
        };
        assert_eq!(
            while_online(request, || Ok(()), Duration::from_millis(1), "Timeout").await,
            Err("Timeout".into())
        );
        assert!(dropped.load(Ordering::SeqCst));
    }
}
