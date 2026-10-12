#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--claude-statusline") {
        if let Some(path) = std::env::args().nth(2) {
            let _ = deskbot_lib::claude_bridge::run(std::path::Path::new(&path));
        }
        return;
    }
    deskbot_lib::run();
}
