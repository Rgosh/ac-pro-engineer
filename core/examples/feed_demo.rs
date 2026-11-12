//! Answer subscriptions with made-up frames, for writing a dashboard without
//! a game.
//!
//! ```bash
//! cargo run -p ac_core --example feed_demo
//! ```
//!
//! Listens where the real program does, on `broadcast::subscribe::PORT`, and
//! sends a car lapping an imaginary circuit: speed and gear rising and
//! falling, fuel going down, tyres warming, a line of advice now and then.
//! **The driver is called `DEMO — MADE-UP DATA`**, so nothing drawn from it
//! can be mistaken for a measurement. Do not run it beside the real program:
//! the port is one or the other's.

use ac_core::broadcast::Broadcaster;
use ac_core::broadcast::subscribe::PORT;
use ac_core::engineer::{Recommendation, Severity};
use ac_core::overlay::frame::{OverlayFrame, flags};
use std::time::{Duration, Instant};

fn main() -> std::io::Result<()> {
    let mut broadcaster = Broadcaster::new();
    let at =
        broadcaster.accept_subscriptions(PORT, false, "assetto_corsa", "DEMO — MADE-UP DATA")?;
    println!("answering subscriptions on {at} with made-up frames; Ctrl+C to stop");

    let start = Instant::now();
    let mut frame = OverlayFrame::empty();
    frame.set_flag(flags::CONNECTED, true);
    frame.max_rpm = 8500;
    let lap = 92.0_f32;
    let mut last_lap = 0;
    let mut subscribers = 0;
    loop {
        let t = start.elapsed().as_secs_f32();
        let into = t % lap;
        let wave = (into / lap * std::f32::consts::TAU * 4.0).sin();
        frame.sequence = frame.sequence.wrapping_add(2);
        frame.speed_kmh = 170.0 + 80.0 * wave;
        frame.gear = 3 + ((wave + 1.0) * 1.5) as i32;
        frame.rpm = 5200 + (2800.0 * ((wave + 1.0) / 2.0)) as i32;
        frame.lap_count = (t / lap) as i32;
        frame.current_lap_ms = (into * 1000.0) as i32;
        frame.fuel_per_lap = 2.6;
        frame.fuel_litres = (40.0 - t / lap * 2.6).max(0.0);
        frame.fuel_laps_remaining = frame.fuel_litres / 2.6;
        frame.delta_seconds = 0.4 * (t / 7.0).sin();
        let warm = (t / 120.0).min(1.0);
        frame.tyre_temp_c = [60.0 + 30.0 * warm; 4];
        frame.tyre_pressure_psi = [24.5 + 3.0 * warm; 4];
        frame.tyre_wear_percent = [100.0 - t / 60.0; 4];
        frame.brake_temp_c = [300.0 + 200.0 * wave.abs(); 4];
        if frame.lap_count != last_lap {
            last_lap = frame.lap_count;
            frame.last_lap_ms = 92_000 + (frame.lap_count % 3) * 350;
            if frame.best_lap_ms == 0 || frame.last_lap_ms < frame.best_lap_ms {
                frame.best_lap_ms = frame.last_lap_ms;
            }
            frame.stint_laps = frame.lap_count as u32;
        }
        let advice = match warm < 1.0 {
            true => "Made up: tyres still coming in — wait a lap before judging pressures",
            false => "Made up: fronts 0.6 psi over target — take it off at the next stop",
        };
        frame.set_messages(&[Recommendation {
            component: String::new(),
            category: String::new(),
            severity: Severity::Info,
            message: advice.to_string(),
            action: String::new(),
            parameters: Vec::new(),
            confidence: 1.0,
            chain: None,
        }]);
        broadcaster.publish(&frame);
        if let Some(now) = broadcaster.subscribers()
            && now != subscribers
        {
            subscribers = now;
            println!("{now} subscribed");
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}
