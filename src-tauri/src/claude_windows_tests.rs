//! Native Windows ConPTY + credential-owner + HTTP + cycle acceptance fixtures.
use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{Arc, OnceLock},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, WAIT_TIMEOUT},
    System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    },
};

struct Fixture {
    root: PathBuf,
    scratch: PathBuf,
    profile: PathBuf,
    executable: PathBuf,
}
impl Fixture {
    fn new(name: &str, mode: &str) -> Self {
        static PROGRAM: OnceLock<PathBuf> = OnceLock::new();
        let program = PROGRAM.get_or_init(|| {
            let dir = std::env::temp_dir()
                .join(format!("DroidFun fixture compiler {}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            let exe = dir.join("renewal-fixture.exe");
            let source =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/windows-renewal-fixture.rs");
            let mut command = Command::new("rustc");
            command
                .arg("--edition=2021")
                .arg("-O")
                .arg(source)
                .arg("-o")
                .arg(&exe);
            if let Ok(target) = std::env::var("DESKBOT_TARGET") {
                command.arg("--target").arg(target);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "fixture compile failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            exe
        });
        let root = std::env::temp_dir().join(format!(
            "DroidFun renewal Ω {name} {} {}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let scratch = root.join("empty workspace");
        let profile = root.join("profile spaces Ω");
        fs::create_dir(&scratch).unwrap();
        fs::create_dir(&profile).unwrap();
        fs::write(profile.join("fixture-profile-only"), "").unwrap();
        fs::write(scratch.join("scenario"), mode).unwrap();
        let executable = root.join("credential owner Ω.exe");
        fs::copy(program, &executable).unwrap();
        Self {
            root,
            scratch,
            profile,
            executable,
        }
    }
    fn credentials(&self, token: &str, expires: u64) {
        fs::write(
            self.profile.join(".credentials.json"),
            json!({"claudeAiOauth":{"accessToken":token,"expiresAt":expires}}).to_string(),
        )
        .unwrap();
    }
    fn stored(&self) -> Result<String, String> {
        stored_token(self.profile.to_str().unwrap())
    }
    fn renew(&self, rejected: Option<&str>, timeout: Duration) -> Result<(), String> {
        refresh_client_terminal(
            self.profile.to_str().unwrap(),
            &self.scratch,
            &self.executable,
            |_| {
                self.stored()
                    .is_ok_and(|token| rejected != Some(token.as_str()))
            },
            timeout,
        )
    }
    fn assert_owned_stopped(&self) {
        let pids = fs::read_to_string(self.scratch.join("owned-pids"))
            .expect("fixture must have run with a real console");
        for pid in pids.split_whitespace().map(|p| p.parse().unwrap()) {
            let deadline = Instant::now() + Duration::from_secs(2);
            while alive(pid) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(!alive(pid), "owned fixture process survived cleanup");
        }
        assert!(
            !self.scratch.join(".claude.json").exists(),
            "no trust decision may be persisted"
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn alive(pid: u32) -> bool {
    unsafe {
        let h = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        );
        if h.is_null() {
            return false;
        }
        let alive = WaitForSingleObject(h, 0) == WAIT_TIMEOUT;
        CloseHandle(h);
        alive
    }
}
fn server(reject_first: bool) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/usage", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        for index in 0..if reject_first { 2 } else { 1 } {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let mut b = [0; 2048];
                let n = stream.read(&mut b).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&b[..n]);
            }
            let reject = reject_first && index == 0;
            assert!(String::from_utf8_lossy(&request).contains(if reject {
                "Bearer fixture-rejected"
            } else {
                "Bearer fixture-renewed"
            }));
            let status = if reject { "401 Unauthorized" } else { "200 OK" };
            let body = if reject {
                "{}"
            } else {
                r#"{"five_hour":{"utilization":0,"resets_at":null}}"#
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    (url, thread)
}
fn assert_cycle(card: DataCard) {
    assert!(card.usable(unix_now()));
    assert_eq!(card.state, "fresh");
    assert_eq!(card.values["allowanceState"], "inactive");
    assert_eq!(card.headline, "100%");
    let mut settings = crate::core::Settings::default();
    let mut id = String::new();
    for w in &mut settings.widgets {
        w.enabled = w.kind == "claude";
        if w.enabled {
            w.config.mode = "live".into();
            id = w.id.clone();
        }
    }
    assert!(!id.is_empty());
    let mut engine = crate::core::Engine::new(settings, Instant::now());
    engine.set_data(&id, card);
    engine.next();
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.current_id, id);
    assert!(snapshot.frame.iter().any(|b| *b != 0));
}

#[test]
fn windows_expired_sign_in_conpty_owner_http_card_and_cycle_with_scoped_cleanup() {
    let fixture = Fixture::new("expired", "success");
    fixture.credentials("fixture-expired", 1);
    let unrelated = Command::new(&fixture.executable)
        .arg("--unrelated-decoy")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut unrelated = ChildGuard(unrelated);
    let (url, server) = server(false);
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let card = read_with_token(&client, &url, "five_hour", || {
        token_with_refresh(
            || fixture.stored(),
            || fixture.renew(None, Duration::from_secs(10)),
        )
    })
    .unwrap();
    server.join().unwrap();
    assert_cycle(card);
    fixture.assert_owned_stopped();
    assert!(
        unrelated.0.try_wait().unwrap().is_none(),
        "unrelated process must survive"
    );
}
#[test]
fn windows_401_conpty_owner_rotates_then_retries_once_and_cycles() {
    let fixture = Fixture::new("rejected", "success");
    fixture.credentials("fixture-rejected", (unix_now() + 3600) * 1000);
    let (url, server) = server(true);
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let mut calls = 0;
    let card = read_with_recovery(
        &client,
        &url,
        "five_hour",
        || fixture.stored(),
        |rejected| {
            calls += 1;
            fixture.renew(Some(rejected), Duration::from_secs(10))
        },
    )
    .unwrap();
    server.join().unwrap();
    assert_eq!(calls, 1);
    assert_cycle(card);
    fixture.assert_owned_stopped();
}
#[test]
fn windows_conpty_timeout_under_output_flood_reaps_owned_tree() {
    let fixture = Fixture::new("timeout", "flood");
    let start = Instant::now();
    assert!(fixture.renew(None, Duration::from_secs(2)).is_err());
    assert!(start.elapsed() < Duration::from_secs(6));
    fixture.assert_owned_stopped();
    assert!(fixture.stored().is_err());
}
#[test]
fn windows_conpty_client_exit_reaps_descendant_without_fabricating_sign_in() {
    let fixture = Fixture::new("failure", "failure");
    assert!(fixture.renew(None, Duration::from_secs(10)).is_err());
    fixture.assert_owned_stopped();
    assert!(fixture.stored().is_err());
}
#[test]
fn windows_conpty_source_cancellation_reaps_owned_tree() {
    let fixture = Fixture::new("cancellation", "flood");
    let cancel = Arc::new(AtomicBool::new(false));
    let _guard = crate::network::RequestGuard::new(cancel.clone());
    let result = refresh_client_terminal(
        fixture.profile.to_str().unwrap(),
        &fixture.scratch,
        &fixture.executable,
        |_| {
            if fixture.scratch.join("owned-pids").exists() {
                cancel.store(true, Ordering::Release);
            }
            false
        },
        Duration::from_secs(10),
    );
    assert!(result.unwrap_err().contains("canceled"));
    fixture.assert_owned_stopped();
}
#[test]
fn windows_conpty_missing_executable_is_an_explicit_bounded_error() {
    let fixture = Fixture::new("missing", "success");
    let start = Instant::now();
    assert!(refresh_client_terminal(
        fixture.profile.to_str().unwrap(),
        &fixture.scratch,
        &fixture.root.join("missing.exe"),
        |_| false,
        Duration::from_secs(2)
    )
    .is_err());
    assert!(start.elapsed() < Duration::from_secs(6));
    assert!(!fixture.scratch.join("owned-pids").exists());
}
