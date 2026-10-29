use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub tray_available: bool,
    pub explanation: String,
}

pub fn info() -> PlatformInfo {
    #[cfg(target_os = "linux")]
    {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        // NameHasOwner succeeds only when the actual session indicator host is present.
        // A missing tool, bus, timeout, or uncertain result keeps a normal window.
        let available = (|| {
            let mut child = Command::new("gdbus")
                .args([
                    "call",
                    "--session",
                    "--dest",
                    "org.freedesktop.DBus",
                    "--object-path",
                    "/org/freedesktop/DBus",
                    "--method",
                    "org.freedesktop.DBus.NameHasOwner",
                    "org.kde.StatusNotifierWatcher",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if child.try_wait().ok()?.is_some() {
                    let output = child.wait_with_output().ok()?;
                    return Some(
                        output.status.success()
                            && String::from_utf8_lossy(&output.stdout).contains("true"),
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            let _ = child.wait();
            None
        })()
        .unwrap_or(false);
        return PlatformInfo { os:"linux".into(),tray_available:available,explanation:if available { "A status indicator host is available. Open DroidFun App from its menu." } else { "Window mode keeps DroidFun App reachable. GNOME users can add the AppIndicator and KStatusNotifierItem Support extension for a tray icon." }.into() };
    }
    #[cfg(not(target_os = "linux"))]
    PlatformInfo {
        os: std::env::consts::OS.into(),
        tray_available: true,
        explanation: if cfg!(target_os = "macos") {
            "DroidFun keeps your droid running when the controls close. Open controls, Settings or Quit from its menu-bar icon."
        } else {
            "Open DroidFun App from the notification area or application launcher."
        }
        .into(),
    }
}

pub fn apply_window_mode(app: &AppHandle, mode: &str) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(if mode == "window" {
        tauri::ActivationPolicy::Regular
    } else {
        tauri::ActivationPolicy::Accessory
    })?;
    #[cfg(not(target_os = "macos"))]
    let _ = (app, mode);
    Ok(())
}

pub fn clamp_panel(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    area: tauri::PhysicalRect<i32, u32>,
) -> (i32, i32) {
    (
        x.clamp(
            area.position.x,
            area.position
                .x
                .saturating_add(area.size.width.saturating_sub(width) as i32),
        ),
        y.clamp(
            area.position.y,
            area.position
                .y
                .saturating_add(area.size.height.saturating_sub(height) as i32),
        ),
    )
}

pub fn show(app: &AppHandle, anchor: bool) {
    show_panel(app, anchor, false);
}

pub fn show_settings(app: &AppHandle) {
    show_panel(app, false, true);
}

fn show_panel(app: &AppHandle, anchor: bool, open_settings: bool) {
    let existed = app.get_webview_window("main").is_some();
    // Tray mode releases the webview when controls close. Recreate it from
    // the same configuration; frontend startup reads current Rust state.
    let window = app.get_webview_window("main").or_else(|| {
        let config = app
            .config()
            .app
            .windows
            .iter()
            .find(|w| w.label == "main")?;
        tauri::WebviewWindowBuilder::from_config(app, config)
            .ok()?
            .on_navigation(|url| crate::web_security::local_navigation(url, cfg!(dev)))
            .on_new_window(|url, _| {
                let _ = crate::web_security::open_public_link(&url);
                tauri::webview::NewWindowResponse::Deny
            })
            .initialization_script(if open_settings {
                "window.__deskbotOpenSettings = true;"
            } else {
                "window.__deskbotOpenSettings = false;"
            })
            .build()
            .ok()
    });
    if let Some(window) = window {
        let _ = window.unminimize();
        let rect = if anchor {
            app.tray_by_id("deskbot")
                .and_then(|tray| tray.rect().ok().flatten())
        } else {
            None
        };
        let mut target = None;
        if let Some(rect) = rect {
            let scale = window.scale_factor().unwrap_or(1.0);
            let pos = rect.position.to_physical::<i32>(scale);
            let size = rect.size.to_physical::<u32>(scale);
            target = Some((pos.x, pos.y, size.height));
        }
        let monitor = target
            .and_then(|(x, y, _)| window.monitor_from_point(x as f64, y as f64).ok().flatten())
            .or_else(|| window.current_monitor().ok().flatten())
            .or_else(|| window.primary_monitor().ok().flatten());
        if let Some(m) = monitor {
            let area = *m.work_area();
            let scale = m.scale_factor();
            let current = window.outer_size().unwrap_or(tauri::PhysicalSize::new(
                (440.0 * scale) as u32,
                (760.0 * scale) as u32,
            ));
            let width = current.width.min(area.size.width);
            let height = current.height.min(area.size.height);
            let _ = window.set_size(tauri::PhysicalSize::new(width, height));
            let saved = window.outer_position().unwrap_or(area.position);
            let (x, y) = if let Some((x, y, tray_height)) = target {
                (
                    x - width as i32 / 2,
                    if y < area.position.y + area.size.height as i32 / 2 {
                        y + tray_height as i32 + 6
                    } else {
                        y - height as i32 - 6
                    },
                )
            } else {
                (saved.x, saved.y)
            };
            let (x, y) = clamp_panel(x, y, width, height, area);
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        }
        let _ = window.show();
        let _ = window.set_focus();
        if open_settings && existed {
            use tauri::Emitter;
            let _ = window.emit("deskbot-open-settings", ());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_monitor_and_taskbar_work_area_are_respected() {
        let area = tauri::PhysicalRect {
            position: tauri::PhysicalPosition::new(-1920, 32),
            size: tauri::PhysicalSize::new(1920, 1008),
        };
        assert_eq!(clamp_panel(-2000, -5, 440, 760, area), (-1920, 32));
        assert_eq!(clamp_panel(-20, 900, 440, 760, area), (-440, 280));
    }
}
