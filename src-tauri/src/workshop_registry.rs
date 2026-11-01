//! Private Workshop inventory client. Session credentials never enter the webview.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
const ORIGIN: &str = "https://droidfun.io";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub board_id: String,
    pub model: String,
    pub firmware: String,
    pub wiring: String,
    pub prepared: bool,
    #[serde(default)]
    pub reset_preparation: bool,
}
impl Record {
    fn valid(&self) -> bool {
        self.board_id.len() == 16
            && self.board_id.bytes().all(|b| b.is_ascii_hexdigit())
            && ["boo", "claw", "meep", "nimbus"].contains(&self.model.as_str())
            && self.firmware.len() <= 64
            && self
                .firmware
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".+_-".contains(&b))
            && ["", "standard", "mistake"].contains(&self.wiring.as_str())
    }
}
#[derive(Default, Serialize, Deserialize)]
struct Cache {
    pending: BTreeMap<String, Record>,
    #[serde(default)]
    droids: Vec<Value>,
    #[serde(default)]
    next: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    connected: bool,
    pending: usize,
    droids: Vec<Value>,
    next: Option<String>,
}
pub struct Store {
    cache: Mutex<Cache>,
    root: PathBuf,
    busy: AtomicBool,
}
fn atomic_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Inventory location is unavailable")?;
    fs::create_dir_all(parent).map_err(|_| "Could not create private Workshop storage")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Could not protect Workshop storage")?;
    }
    let tmp = parent.join(format!(
        ".registry-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Clock unavailable")?
            .as_nanos()
    ));
    let result: Result<(), String> = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&tmp)
            .map_err(|_| "Could not save private Workshop storage")?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Could not save private Workshop storage")?;
        fs::rename(&tmp, path).map_err(|_| "Could not commit Workshop storage".to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
impl Store {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let cache=match fs::read(root.join("droids.json")){Ok(bytes) if bytes.len()<=4*1024*1024=>serde_json::from_slice(&bytes).map_err(|_|"The saved droid list could not be read. Preserve it before repairing Workshop storage.")?,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Cache::default(),_=>return Err("Could not read the saved droid list".into())};
        Ok(Self {
            cache: Mutex::new(cache),
            root,
            busy: AtomicBool::new(false),
        })
    }
    fn save(&self, cache: &Cache) -> Result<(), String> {
        let bytes = serde_json::to_vec(cache).map_err(|_| "Could not prepare droid list")?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("The saved droid list is full. Sync before recording more droids.".into());
        }
        atomic_private(&self.root.join("droids.json"), &bytes)
    }
    fn token(&self) -> Option<String> {
        let bytes = fs::read(self.root.join("session.json")).ok()?;
        if bytes.len() > 2048 {
            return None;
        }
        let token = String::from_utf8(bytes).ok()?;
        (!token.is_empty()).then_some(token)
    }
    fn forget(&self) -> Result<(), String> {
        match fs::remove_file(self.root.join("session.json")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(
                "Could not remove the Workshop connection. Revoke it from the dashboard.".into(),
            ),
        }
    }
    fn view(&self) -> Result<View, String> {
        let cache = self.cache.lock().map_err(|_| "Droid list is busy")?;
        let mut rows = cache.droids.clone();
        for pending in cache.pending.values() {
            let value = serde_json::json!({"board_id":pending.board_id,"model":pending.model,"firmware":pending.firmware,"wiring":pending.wiring,"state":if pending.prepared{"prepared"}else{"registered"},"pendingSync":true});
            if let Some(row) = rows
                .iter_mut()
                .find(|row| row["board_id"].as_str() == Some(&pending.board_id))
            {
                *row = value;
            } else {
                rows.push(value);
            }
        }
        Ok(View {
            connected: self.token().is_some(),
            pending: cache.pending.len(),
            droids: rows,
            next: cache.next.clone(),
        })
    }
}
fn merge_pending(record: &mut Record, previous: &Record) {
        if previous.model == record.model
            && previous.firmware == record.firmware
            && previous.wiring == record.wiring
        {
            if !record.reset_preparation { record.prepared |= previous.prepared; }
            record.reset_preparation |= previous.reset_preparation;
        }
}

pub fn observe(app: &AppHandle, mut record: Record) -> Result<(), String> {
    if !record.valid() {
        return Err("This board does not have a supported registry identity".into());
    }
    record.board_id = record.board_id.to_uppercase();
    let store = app.state::<Store>();
    let mut cache = store.cache.lock().map_err(|_| "Droid list is busy")?;
    if let Some(previous) = cache.pending.get(&record.board_id) {
        merge_pending(&mut record, previous);
    }
    let old = cache
        .pending
        .insert(record.board_id.clone(), record.clone());
    if let Err(e) = store.save(&cache) {
        if let Some(previous) = old {
            cache.pending.insert(record.board_id, previous);
        } else {
            cache.pending.remove(&record.board_id);
        }
        return Err(e);
    }
    drop(cache);
    let _ = app.emit("workshop-registry-changed", ());
    Ok(())
}
async fn request(path: &str, token: Option<&str>, data: Option<&Value>) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| "Could not prepare Workshop sync")?;
    let mut request = if data.is_some() {
        client.post(format!("{ORIGIN}{path}"))
    } else {
        client.get(format!("{ORIGIN}{path}"))
    };
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(data) = data {
        request = request.json(data);
    }
    let mut response = request
        .send()
        .await
        .map_err(|_| "Could not reach Droid Fun. Your droids are saved locally; try Sync again.")?;
    if response.status() == 401 {
        return Err(
            "Workshop connection expired. Create a new code in your owner dashboard.".into(),
        );
    }
    if !response.status().is_success() {
        return Err(if response.status() == 429 {
            "Sync paused. Wait a few minutes, then try again."
        } else {
            "Workshop sync could not finish. Your pending droids are saved locally."
        }
        .into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not read Workshop response")?
    {
        if bytes.len() + chunk.len() > 256 * 1024 {
            return Err("Workshop response was too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Workshop returned an invalid response".into())
}
struct Busy<'a>(&'a AtomicBool);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn enter(store: &Store) -> Result<Busy<'_>, String> {
    store
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "A Workshop sync is already running")?;
    Ok(Busy(&store.busy))
}
#[tauri::command]
pub fn workshop_registry_state(app: AppHandle) -> Result<View, String> {
    app.state::<Store>().view()
}
#[tauri::command]
pub fn workshop_registry_open_dashboard() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .arg("https://droidfun.io/admin#droids")
            .spawn()
            .map_err(|_| "Open droidfun.io/admin in your browser")?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32.exe").args(["url.dll,FileProtocolHandler","https://droidfun.io/admin#droids"]).creation_flags(0x08000000).spawn().map_err(|_|"Open droidfun.io/admin in your browser")?;
        Ok(())
    }
    #[cfg(not(any(target_os = "macos",target_os = "windows")))]
    { Err("Open https://droidfun.io/admin#droids in your browser".into()) }
}
#[tauri::command]
pub async fn workshop_registry_connect(app: AppHandle, code: String) -> Result<View, String> {
    if code.len() != 64 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Paste the connection code from your owner dashboard.".into());
    }
    let store = app.state::<Store>();
    let _busy = enter(&store)?;
    let result = request(
        "/api/workshop/pair",
        None,
        Some(&serde_json::json!({"code":code.to_lowercase(),"label":"DroidFun Workshop on Mac"})),
    )
    .await?;
    let token = result["token"]
        .as_str()
        .filter(|s| {
            s.len() <= 1000
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
        .ok_or("Workshop returned an invalid connection")?;
    atomic_private(&store.root.join("session.json"), token.as_bytes())?;
    store.view()
}
#[tauri::command]
pub async fn workshop_registry_sync(
    app: AppHandle,
    query: Option<String>,
    after: Option<String>,
) -> Result<View, String> {
    let store = app.state::<Store>();
    let _busy = enter(&store)?;
    let token = store
        .token()
        .ok_or("Connect Workshop using a code from your owner dashboard.")?;
    let pending = store
        .cache
        .lock()
        .map_err(|_| "Droid list is busy")?
        .pending
        .values()
        .take(25)
        .cloned()
        .collect::<Vec<_>>();
    for row in pending {
        request(
            "/api/workshop/droids",
            Some(&token),
            Some(&serde_json::to_value(&row).map_err(|_| "Invalid droid record")?),
        )
        .await?;
        let mut cache = store.cache.lock().map_err(|_| "Droid list is busy")?;
        if cache.pending.get(&row.board_id) == Some(&row) {
            let old = cache.pending.remove(&row.board_id).unwrap();
            if let Err(e) = store.save(&cache) {
                cache.pending.insert(row.board_id, old);
                return Err(e);
            }
        }
    }
    let mut url = reqwest::Url::parse(&format!("{ORIGIN}/api/workshop/droids"))
        .map_err(|_| "Invalid sync address")?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair(
            "q",
            &query
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect::<String>(),
        );
        if let Some(after) = after {
            params.append_pair("after", &after);
        }
    }
    let response = request(
        &format!("{}?{}", url.path(), url.query().unwrap_or("")),
        Some(&token),
        None,
    )
    .await?;
    let rows = response["droids"]
        .as_array()
        .filter(|rows| rows.len() <= 100)
        .ok_or("Invalid droid list")?
        .clone();
    {
        let mut cache = store.cache.lock().map_err(|_| "Droid list is busy")?;
        cache.droids = rows;
        cache.next = response["next"].as_str().map(str::to_owned);
        store.save(&cache)?;
    }
    store.view()
}
#[tauri::command]
pub async fn workshop_registry_disconnect(app: AppHandle) -> Result<View, String> {
    let store = app.state::<Store>();
    let _busy = enter(&store)?;
    let token = store.token();
    store.forget()?;
    if let Some(token) = token {
        let _ = request(
            "/api/workshop/disconnect",
            Some(&token),
            Some(&serde_json::json!({})),
        )
        .await;
    }
    store.view()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wipe_clears_prepared_and_reset_survives_offline_inspection_until_fresh_checks() {
        let prepared = Record { board_id: "0123456789ABCDEF".into(), model: "boo".into(), firmware: "1.3.2".into(), wiring: "standard".into(), prepared: true, reset_preparation: false };
        let mut wipe = Record { prepared: false, reset_preparation: true, ..prepared.clone() };
        merge_pending(&mut wipe, &prepared);
        assert!(!wipe.prepared);
        let mut inspected = Record { prepared: false, ..prepared.clone() };
        merge_pending(&mut inspected, &wipe);
        assert!(!inspected.prepared);
        assert!(inspected.reset_preparation);
        let mut checked = prepared.clone();
        merge_pending(&mut checked, &inspected);
        assert!(checked.prepared);
        assert!(checked.reset_preparation);
    }

    #[test]
    fn pending_records_are_persisted_without_a_session_and_tokens_never_enter_views() {
        let root = std::env::temp_dir().join(format!(
            "workshop-registry-{}-{}",
            std::process::id(),
            crate::core::unix_now()
        ));
        let store = Store::new(root.clone()).unwrap();
        {
            let mut c = store.cache.lock().unwrap();
            c.pending.insert(
                "0123456789ABCDEF".into(),
                Record {
                    board_id: "0123456789ABCDEF".into(),
                    model: "meep".into(),
                    firmware: "1.3.2".into(),
                    wiring: "standard".into(),
                    prepared: false,
                    reset_preparation: false,
                },
            );
            store.save(&c).unwrap();
        }
        atomic_private(&root.join("session.json"), b"secret-session-not-for-ui").unwrap();
        let restored = Store::new(root.clone()).unwrap();
        let view = serde_json::to_string(&restored.view().unwrap()).unwrap();
        assert!(!view.contains("secret-session"));
        assert!(view.contains("pendingSync"));
        assert_eq!(restored.view().unwrap().pending, 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(root.join("session.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        restored.forget().unwrap();
        assert!(!restored.view().unwrap().connected);
        assert_eq!(restored.view().unwrap().pending, 1);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn invalid_registry_files_are_preserved() {
        let root =
            std::env::temp_dir().join(format!("workshop-registry-invalid-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("droids.json"), b"bad-data").unwrap();
        assert!(Store::new(root.clone()).is_err());
        assert_eq!(fs::read(root.join("droids.json")).unwrap(), b"bad-data");
        let _ = fs::remove_dir_all(root);
    }
}
