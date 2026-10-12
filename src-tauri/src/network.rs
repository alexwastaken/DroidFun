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
thread_local! { static CANCEL:RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) }; }
pub struct RequestGuard(Option<Arc<AtomicBool>>);
impl RequestGuard {
    pub fn new(cancel: Arc<AtomicBool>) -> Self {
        Self(CANCEL.with(|c| c.replace(Some(cancel))))
    }
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        CANCEL.with(|c| {
            c.replace(self.0.take());
        });
    }
}
pub fn canceled() -> bool {
    CANCEL.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|f| f.load(Ordering::Acquire))
    })
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
    let key = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(url, query, bearer, headers, limit))
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
    let cancel = CANCEL.with(|c| c.borrow().clone());
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
            loop { if cancel.as_ref().is_some_and(|f|f.load(Ordering::Acquire)) {break;} tokio::time::sleep(Duration::from_millis(20)).await; }
        };
        tokio::select! { result=operation=>result, _=cancellation=>Err("Request canceled because the source changed".into()) }
    });
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
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buffer = [0; 2048];
            let _ = socket.read(&mut buffer);
            std::thread::sleep(Duration::from_millis(300));
            let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        });
        let flag = Arc::new(AtomicBool::new(false));
        let cancel = flag.clone();
        let trigger = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            cancel.store(true, Ordering::Release);
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let _guard = RequestGuard::new(flag);
        let start = Instant::now();
        assert!(
            json(&client, &format!("http://{address}"), &[], None, &[], 1024)
                .unwrap_err()
                .contains("canceled")
        );
        assert!(start.elapsed() < Duration::from_millis(250));
        trigger.join().unwrap();
        server.join().unwrap();
    }
}
