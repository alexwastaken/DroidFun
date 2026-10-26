#[path = "../linux_usb.rs"]
mod linux_usb;
#[path = "../claude_account.rs"]
mod claude_account;
#[path = "../connection.rs"]
mod connection;
#[path = "../core.rs"]
mod core;
#[path = "../droids.rs"]
mod droids;
#[path = "../host_display.rs"]
mod host_display;
#[path = "../local_widgets.rs"]
mod local_widgets;
#[path = "../network.rs"]
mod network;
#[path = "../protocol.rs"]
mod protocol;
#[path = "../providers.rs"]
mod providers;
#[path = "../reliability.rs"]
mod reliability;

use std::{
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), String> {
    if std::env::args().nth(1).as_deref() == Some("--display-power") {
        println!(
            "{}",
            serde_json::json!({"hostDisplayAsleep": host_display::state()})
        );
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--face-assets") {
        println!(
            "{}",
            serde_json::to_string(&core::render(None, true)).unwrap()
        );
        return Ok(());
    }
    if matches!(
        std::env::args().nth(1).as_deref(),
        Some("--usage-assets" | "--codex-assets")
    ) {
        let mut screens = serde_json::Map::new();
        for kind in ["codex", "claude"] {
            let now = Instant::now();
            let mut engine = core::Engine::new(core::Settings::default(), now);
            engine.set_enabled(kind, true)?;
            engine.pin(kind)?;
            let mut frames = Vec::new();
            for ms in [0, 900, 1200, 1800, 2100] {
                engine.tick_at(now + Duration::from_millis(ms), 10000, 720);
                frames.push(engine.render_widget(kind));
            }
            screens.insert(kind.into(), serde_json::json!(frames));
        }
        println!("{}", serde_json::Value::Object(screens));
        return Ok(());
    }
    let serial = std::env::args()
        .nth(1)
        .ok_or("Pass the robot USB serial number")?;
    let hold_seconds: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let checker = std::env::args().nth(3).as_deref() == Some("checker");
    let port = connection::robot_ports()?
        .into_iter()
        .find(|p| p.serial == serial)
        .ok_or("Selected RP2040-Zero is not present")?;
    let mut link = connection::Link::connect(&port)?;
    println!(
        "HELLO_ACK: board={:?}, firmware={:?}, OLED={:?}",
        link.status.board_id, link.status.firmware, link.status.display_health
    );
    if std::env::args().nth(3).as_deref() == Some("benchmark") {
        let base = core::render(None, true);
        let started = Instant::now();
        let mut ack_times = Vec::new();
        let mut transfer_ms = Vec::new();
        let mut sleep_ms = Vec::new();
        while started.elapsed() < Duration::from_secs(hold_seconds) {
            let mut frame = base.clone();
            // A small moving footer leaves the face visible; no full-screen flashing.
            let x = (link.metrics.frame_acks as usize * 3) % 120;
            for col in x..x + 8 { frame[7 * 128 + col] |= 0xc0; }
            let previous = link.metrics.frame_acks;
            let transfer_started = Instant::now();
            link.update(&frame, true, 85, 0)?;
            if link.metrics.frame_acks > previous {
                ack_times.push(started.elapsed().as_secs_f64());
                transfer_ms.push(transfer_started.elapsed().as_secs_f64() * 1000.0);
            }
            let delay = link.next_poll_delay();
            sleep_ms.push(delay.as_secs_f64() * 1000.0);
            thread::sleep(delay);
        }
        let fps = if ack_times.len() > 1 {
            (ack_times.len() - 1) as f64 / (ack_times.last().unwrap() - ack_times[0])
        } else { 0.0 };
        println!("BENCHMARK {}", serde_json::json!({
            "firmware": link.status.firmware, "seconds": started.elapsed().as_secs_f64(),
            "verifiedFrames": ack_times.len(), "measuredFps": fps,
            "meanTransferMs": transfer_ms.iter().sum::<f64>() / transfer_ms.len().max(1) as f64,
            "meanSleepMs": sleep_ms.iter().sum::<f64>() / sleep_ms.len().max(1) as f64,
            "maxFrameGapMs": ack_times.windows(2).map(|w| (w[1]-w[0])*1000.0).fold(0.0f64, f64::max),
            "displayHealth": link.status.display_health,
        }));
        thread::sleep(Duration::from_millis(210));
        link.update(&base, true, 85, 0)?;
        return Ok(());
    }
    let test = core::Widget {
        id: "test".into(),
        kind: "weather".into(),
        label: "Test".into(),
        enabled: true,
        duration_sec: 8,
        headline: "HELLO".into(),
        footer: "DROID FUN TEST".into(),
        config: core::SourceConfig::default(),
    };
    let test_frame = if checker {
        (0..1024)
            .map(|i| if i % 2 == 0 { 0xAA } else { 0x55 })
            .collect()
    } else {
        core::render(Some(&test), true)
    };
    println!(
        "Host frame CRC={} nonzeroBytes={}",
        protocol::crc32(&test_frame),
        test_frame.iter().filter(|byte| **byte != 0).count()
    );
    link.update(&test_frame, true, 85, 0)?;
    println!(
        "FRAME ACK: {}, OLED={:?}",
        if checker { "checkerboard" } else { "test text" },
        link.status.display_health
    );
    let until = Instant::now() + Duration::from_secs(hold_seconds);
    while Instant::now() < until {
        thread::sleep(Duration::from_millis(250));
        link.update(&test_frame, true, 85, 0)?;
    }
    link.update(&core::render(None, true), true, 85, 0)?;
    println!("FRAME ACK: smile, OLED={:?}", link.status.display_health);
    Ok(())
}
