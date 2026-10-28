//! Bounded HTTP reads with cooperative cancellation on source changes.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};
use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};
/// Each Providers instance owns its policy. Tests and secondary instances do not
/// change one another's connectivity. A generation never becomes valid again.
#[derive(Clone)]
pub struct Policy(Arc<Mutex<PolicyState>>);
struct PolicyState {
    bot_model: String,
    offline: bool,
    generation: u64,
    cancel: Arc<AtomicBool>,
}
static NEXT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
#[derive(Clone)]
pub struct Ticket {
    policy: Policy,
    generation: u64,
    cancel: Arc<AtomicBool>,
}
impl Policy {
    pub fn new(bot_model: &str, offline: bool) -> Self {
        Self(Arc::new(Mutex::new(PolicyState {
            bot_model: bot_model.into(),
            offline,
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
            cancel: Arc::new(AtomicBool::new(false)),
        })))
    }
    pub fn update(&self, bot_model: &str, offline: bool) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.bot_model == bot_model && state.offline == offline {
            return;
        }
        state.cancel.store(true, Ordering::Release);
        state.bot_model = bot_model.into();
        state.offline = offline;
        state.generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        state.cancel = Arc::new(AtomicBool::new(false));
    }
    pub fn ticket(&self, kind: &str) -> Result<Ticket, String> {
        let state = self.0.lock().map_err(|_| "Network policy unavailable")?;
        if state.offline {
            return Err(
                "Offline mode is on. Turn it off to use online data or connect an account.".into(),
            );
        }
        if kind != "droid_setup" && state.bot_model.is_empty() {
            return Err("Choose your bot before connecting an online source.".into());
        }
        if kind != "droid_setup" && !crate::core::kind_allowed(&state.bot_model, kind) {
            return Err("This source is not available for the selected bot.".into());
        }
        Ok(Ticket {
            policy: self.clone(),
            generation: state.generation,
            cancel: state.cancel.clone(),
        })
    }
}
impl Ticket {
    pub fn current(&self) -> bool {
        !self.cancel.load(Ordering::Acquire)
    }
    pub fn enter(&self) -> RequestGuard {
        let mut next = CONTEXT.with(|c| c.borrow().clone());
        next.cancels.push(self.cancel.clone());
        next.ticket = Some(self.clone());
        RequestGuard(CONTEXT.with(|c| c.replace(next)))
    }
}
#[derive(Clone, Default)]
struct Context {
    cancels: Vec<Arc<AtomicBool>>,
    ticket: Option<Ticket>,
}
thread_local! { static CONTEXT:RefCell<Context> = RefCell::new(Context::default()); }
pub struct RequestGuard(Context);
impl RequestGuard {
    pub fn new(cancel: Arc<AtomicBool>) -> Self {
        let mut next = CONTEXT.with(|c| c.borrow().clone());
        next.cancels.push(cancel);
        Self(CONTEXT.with(|c| c.replace(next)))
    }
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        CONTEXT.with(|c| {
            c.replace(std::mem::take(&mut self.0));
        });
    }
}
pub fn canceled() -> bool {
    CONTEXT.with(|c| c.borrow().cancels.iter().any(|f| f.load(Ordering::Acquire)))
}
pub fn ensure_allowed() -> Result<(), String> {
    if canceled() {
        Err("Request canceled because the bot, offline mode, or source changed".into())
    } else {
        Ok(())
    }
}
pub fn generation() -> u64 {
    CONTEXT.with(|c| c.borrow().ticket.as_ref().map_or(0, |t| t.generation))
}
/// Serialize subprocess launch with policy changes. The process can never be
/// launched after an offline transition has already completed.
pub fn authorized<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    ensure_allowed()?;
    let ticket = CONTEXT.with(|c| c.borrow().ticket.clone());
    if let Some(ticket) = ticket {
        let state = ticket
            .policy
            .0
            .lock()
            .map_err(|_| "Network policy unavailable")?;
        if state.offline || state.generation != ticket.generation || canceled() {
            return Err("Request canceled because the bot or offline mode changed".into());
        }
        operation()
    } else {
        operation()
    }
}
pub fn lock_cancellable<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    loop {
        if canceled() {
            return Err("Request canceled because the source changed".into());
        }
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(e)) => return Ok(e.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(20))
            }
        }
    }
}
#[derive(Default)]
struct Responses {
    values: HashMap<String, (u64, Value)>,
    locks: HashMap<String, Arc<Mutex<()>>>,
    blocked: HashMap<String, u64>,
}
fn responses() -> &'static Mutex<Responses> {
    static RESPONSES: OnceLock<Mutex<Responses>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(Responses::default()))
}
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("HTTP runtime unavailable")
    })
}
pub fn json(
    client: &reqwest::Client,
    url: &str,
    query: &[(&str, String)],
    bearer: Option<&str>,
    headers: &[(&str, &str)],
    limit: usize,
) -> Result<Value, String> {
    ensure_allowed()?;
    if crate::reliability::fault("network-offline") {
        return Err("The provider could not be reached. Try again later.".into());
    }
    let key = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(generation(), url, query, bearer, headers, limit))
                .map_err(|_| "Invalid request")?
        )
    );
    let host = reqwest::Url::parse(url)
        .ok()
        .map(|u| u.origin().ascii_serialization())
        .unwrap_or_default();
    let scope = format!(
        "{}:{:x}",
        host,
        Sha256::digest(bearer.unwrap_or("").as_bytes())
    );
    let lock = {
        let mut cache = responses().lock().unwrap_or_else(|e| e.into_inner());
        if cache.locks.len() >= 64 {
            cache.locks.retain(|_, v| Arc::strong_count(v) > 1);
        }
        cache
            .locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock_cancellable(&lock)?;
    let now = crate::core::unix_now();
    {
        let cache = responses().lock().unwrap_or_else(|e| e.into_inner());
        if let Some((expires, value)) = cache.values.get(&key) {
            if now < *expires {
                return Ok(value.clone());
            }
        }
        if let Some(until) = cache.blocked.get(&scope) {
            if now < *until {
                return Err(format!(
                    "Provider returned HTTP 429; retry after {}s",
                    until - now
                ));
            }
        }
    }
    let mut request = client.get(url).query(query);
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    ensure_allowed()?;
    let cancels = CONTEXT.with(|c| c.borrow().cancels.clone());
    let result=runtime().block_on(async {
        let operation=async {
            let mut response=request.send().await.map_err(|_|"The provider could not be reached. Try again later.".to_string())?;
            if !response.status().is_success() {
                let retry=response.headers().get("retry-after").and_then(|h|h.to_str().ok()).and_then(|s|s.parse::<u64>().ok().or_else(||chrono::DateTime::parse_from_rfc2822(s).ok().map(|t|(t.timestamp()-crate::core::unix_now() as i64).max(0) as u64)));
                return Err(format!("Provider returned HTTP {}{}",response.status().as_u16(),retry.map(|n|format!("; retry after {n}s")).unwrap_or_default()));
            }
            let mut bytes=Vec::new();
            while let Some(chunk)=response.chunk().await.map_err(|_|"Provider response could not be read")? {
                if bytes.len()+chunk.len()>limit {return Err("Provider response exceeded the size limit".into());}
                bytes.extend_from_slice(&chunk);
            }
            serde_json::from_slice::<Value>(&bytes).map_err(|_|"Provider returned invalid JSON".into())
        };
        let cancellation=async {
            loop { if cancels.iter().any(|f|f.load(Ordering::Acquire)) {break;} tokio::time::sleep(Duration::from_millis(20)).await; }
        };
        tokio::select! { biased; _=cancellation=>Err("Request canceled because the source changed".into()), result=operation=>result }
    });
    ensure_allowed()?;
    if let Ok(value) = &result {
        let mut cache = responses().lock().unwrap_or_else(|e| e.into_inner());
        let now = crate::core::unix_now();
        cache.values.retain(|_, (expires, _)| *expires > now);
        cache.blocked.retain(|_, until| *until > now);
        if cache.values.len() >= 48 {
            if let Some(key) = cache
                .values
                .iter()
                .min_by_key(|(_, v)| v.0)
                .map(|(k, _)| k.clone())
            {
                cache.values.remove(&key);
            }
        }
        cache.values.insert(key, (now + 30, value.clone()));
    } else if let Err(error) = &result {
        if error.starts_with("Provider returned HTTP 429") {
            let delay = error
                .split("retry after ")
                .nth(1)
                .and_then(|s| s.trim_end_matches('s').parse::<u64>().ok())
                .unwrap_or(600)
                .max(600);
            let mut cache = responses().lock().unwrap_or_else(|e| e.into_inner());
            cache.blocked.retain(|_, until| *until > now);
            if cache.blocked.len() >= 128 {
                if let Some(key) = cache
                    .blocked
                    .iter()
                    .min_by_key(|(_, until)| **until)
                    .map(|(k, _)| k.clone())
                {
                    cache.blocked.remove(&key);
                }
            }
            cache.blocked.insert(scope, now.saturating_add(delay));
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Instant,
    };
    #[test]
    fn offline_transition_blocks_even_an_already_captured_http_request() {
        let policy = Policy::new("boo", false);
        let ticket = policy.ticket("weather").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/never-requested", listener.local_addr().unwrap());
        policy.update("boo", true);
        assert!(policy.ticket("weather").err().unwrap().contains("Offline"));
        let _guard = ticket.enter();
        let client = reqwest::Client::new();
        assert!(json(&client, &url, &[], None, &[], 1024)
            .unwrap_err()
            .contains("canceled"));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        let mut launched = false;
        assert!(authorized(|| {
            launched = true;
            Ok(())
        })
        .is_err());
        assert!(
            !launched,
            "an offline request must not launch an authentication subprocess"
        );
    }
    #[test]
    fn policies_are_isolated_and_generations_never_revive_after_mode_or_profile_changes() {
        let first = Policy::new("claw", false);
        let second = Policy::new("boo", false);
        let old = first.ticket("claude").unwrap();
        assert!(first.ticket("weather").is_err());
        assert!(second.ticket("codex").is_err());
        assert!(Policy::new("", false).ticket("weather").is_err());
        first.update("claw", true);
        first.update("claw", false);
        assert!(
            !old.current(),
            "off/on must never resurrect an in-flight result"
        );
        assert!(second.ticket("weather").unwrap().current());
        let current = first.ticket("codex").unwrap();
        first.update("boo", false);
        first.update("claw", false);
        assert!(
            !current.current(),
            "bot switches must invalidate the entire old generation"
        );
        let guard = old.enter();
        let nested = first.ticket("claude").unwrap().enter();
        assert!(
            ensure_allowed().is_err(),
            "nested provider calls must retain parent cancellation"
        );
        drop(nested);
        drop(guard);
        assert!(
            !canceled(),
            "request context must be restored for the next operation"
        );
    }
    #[test]
    fn offline_cancels_an_inflight_http_request_and_rejects_its_late_response() {
        let policy = Policy::new("boo", false);
        let ticket = policy.ticket("weather").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0; 2048];
            socket.read(&mut bytes).unwrap();
            tx.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(300));
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\n{\"value\":42}",
            );
        });
        let captured = ticket.clone();
        let request = std::thread::spawn(move || {
            let _guard = captured.enter();
            json(
                &reqwest::Client::new(),
                &format!("http://{address}/slow"),
                &[],
                None,
                &[],
                1024,
            )
        });
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let start = Instant::now();
        policy.update("boo", true);
        policy.update("boo", false);
        assert!(request.join().unwrap().unwrap_err().contains("canceled"));
        assert!(start.elapsed() < Duration::from_millis(250));
        assert!(!ticket.current());
        server.join().unwrap();
    }
    #[test]
    fn shared_requests_fetch_once_and_quota_blocks_other_queries() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = count.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let server = std::thread::spawn(move || {
            while !done.load(Ordering::Acquire) {
                if let Ok((mut socket, _)) = listener.accept() {
                    // Windows can inherit the listener's nonblocking mode. The mock
                    // server must wait for the request instead of racing its sender.
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut bytes = [0; 2048];
                    let mut n = 0;
                    while !bytes[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                        assert!(n < bytes.len(), "Mock HTTP request headers are too large");
                        let received = socket.read(&mut bytes[n..]).unwrap();
                        assert!(received > 0, "Mock HTTP request ended before its headers");
                        n += received;
                    }
                    seen.fetch_add(1, Ordering::SeqCst);
                    let reply: &[u8] = if String::from_utf8_lossy(&bytes[..n]).contains("/quota") {
                        b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 900\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                    } else {
                        b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\n{\"value\":42}"
                    };
                    socket.write_all(reply).unwrap();
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let url = format!("http://{address}/data");
        let second_client = client.clone();
        let second_url = url.clone();
        let concurrent = std::thread::spawn(move || {
            json(&second_client, &second_url, &[], None, &[], 1024).unwrap()
        });
        assert_eq!(
            json(&client, &url, &[], None, &[], 1024).unwrap()["value"],
            42
        );
        assert_eq!(concurrent.join().unwrap()["value"], 42);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(json(
            &client,
            &format!("http://{address}/quota"),
            &[],
            None,
            &[],
            1024
        )
        .unwrap_err()
        .contains("429"));
        assert!(json(
            &client,
            &format!("http://{address}/other"),
            &[],
            None,
            &[],
            1024
        )
        .unwrap_err()
        .contains("retry after"));
        assert_eq!(count.load(Ordering::SeqCst), 2);
        stop.store(true, Ordering::Release);
        server.join().unwrap();
    }
    #[test]
    fn canceled_http_is_dropped_without_waiting_for_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let cancel = flag.clone();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("The cancellation fixture did not receive a request: {error}"),
                }
            };
            socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut buffer = [0; 2048];
            assert!(socket.read(&mut buffer).unwrap() > 0);
            // Cancel only after the request is truly in flight. Starting a
            // timer before connecting could strand accept() on a busy host.
            std::thread::sleep(Duration::from_millis(80));
            cancel.store(true, Ordering::Release);
            std::thread::sleep(Duration::from_millis(300));
            let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let _guard = RequestGuard::new(flag);
        let start = Instant::now();
        assert!(
            json(&client, &format!("http://{address}"), &[], None, &[], 1024)
                .unwrap_err()
                .contains("canceled")
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        server.join().unwrap();
    }
}
