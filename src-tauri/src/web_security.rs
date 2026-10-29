//! Keep the privileged controls on bundled pages. Public help opens in the
//! system browser, using fixed destinations rather than arbitrary URL input.
use std::{process::{Command, Stdio}, sync::atomic::{AtomicBool, Ordering}};
use tauri::Url;

pub fn local_navigation(url: &Url, development: bool) -> bool {
    if !url.username().is_empty() || url.password().is_some() { return false; }
    if development && url.scheme() == "http" && url.host_str() == Some("127.0.0.1")
        && url.port() == Some(1420) { return true; }
    url.port().is_none() && match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => url.host_str() == Some("tauri.localhost"),
        _ => false,
    }
}

fn public_destination(url: &Url) -> Option<&'static str> {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some()
        || url.port().is_some() || url.query().is_some() { return None; }
    match (url.host_str(), url.path(), url.fragment()) {
        (Some("droidfun.io"), "/downloads", None) => Some("https://droidfun.io/downloads"),
        (Some("droidfun.io"), "/downloads", Some("droid-support")) => Some("https://droidfun.io/downloads#droid-support"),
        (Some("open-meteo.com"), "/", None) => Some("https://open-meteo.com/"),
        _ => None,
    }
}

pub fn open_public_link(url: &Url) -> Result<(), String> {
    let destination = public_destination(url).ok_or("This link is not an approved public page.")?;
    static OPENING: AtomicBool = AtomicBool::new(false);
    if OPENING.swap(true, Ordering::SeqCst) { return Ok(()); }
    #[cfg(target_os = "macos")]
    let mut command = { let mut c = Command::new("/usr/bin/open"); c.arg(destination); c };
    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", destination]).creation_flags(0x08000000); c
    };
    #[cfg(target_os = "linux")]
    let mut command = { let mut c = Command::new("xdg-open"); c.arg(destination); c };
    match command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || { let _ = child.wait(); OPENING.store(false, Ordering::SeqCst); });
            Ok(())
        }
        Err(_) => {
            OPENING.store(false, Ordering::SeqCst);
            Err("Could not open your browser. Open droidfun.io/downloads in your browser for help.".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_stay_on_exact_local_origins() {
        for value in ["tauri://localhost/", "tauri://localhost/index.html#settings", "http://tauri.localhost/", "https://tauri.localhost/index.html"] {
            assert!(local_navigation(&value.parse().unwrap(), false), "{value}");
        }
        for value in ["https://droidfun.io/", "http://tauri.evil.com/", "http://tauri.localhost.evil.com/", "http://evil.tauri.localhost/", "tauri://elsewhere/", "tauri://user@localhost/", "http://tauri.localhost:1234/", "http://localhost:1420/", "http://127.0.0.1:1420/", "file:///etc/passwd", "data:text/html,test", "about:blank"] {
            assert!(!local_navigation(&value.parse().unwrap(), false), "{value}");
        }
        assert!(local_navigation(&"http://127.0.0.1:1420/".parse().unwrap(), true));
        assert!(!local_navigation(&"http://127.0.0.1:1421/".parse().unwrap(), true));
    }
    #[test]
    fn only_fixed_public_links_can_leave_the_app() {
        for value in ["https://droidfun.io/downloads", "https://droidfun.io/downloads#droid-support", "https://open-meteo.com/"] {
            assert_eq!(public_destination(&value.parse().unwrap()), Some(value));
        }
        for value in ["http://droidfun.io/downloads", "https://droidfun.io.evil.com/downloads", "https://evil.com/", "https://user@droidfun.io/downloads", "https://droidfun.io:8443/downloads", "https://droidfun.io/admin", "https://droidfun.io/downloads?boardId=private", "https://droidfun.io/downloads#private", "https://open-meteo.com/?token=private", "file:///tmp/app", "javascript:alert(1)"] {
            assert_eq!(public_destination(&value.parse().unwrap()), None, "{value}");
        }
    }
}
