//! The stream overlay with a made-up frame, for setting OBS up or for a
//! picture of it without a game:
//!
//!     cargo run -p ac_core --example obs_demo
//!
//! then a browser source in OBS at http://127.0.0.1:7878/. Ctrl+C to stop.

use ac_core::broadcast::Sink;
use ac_core::broadcast::obs::{DEFAULT_PORT, ObsSink};
use ac_core::overlay::frame::{OverlayFrame, flags};

fn main() -> anyhow::Result<()> {
    let mut sink = ObsSink::start(DEFAULT_PORT, "assetto_corsa", "demo")?;
    println!("OBS browser source: {}", sink.url());
    let lines = [
        (
            "Brakes are cooking. More heat's going in than they can get rid of. Open the ducts a bit, or ease off the big stops.",
            "FL brakes 780 °C",
            false,
        ),
        (
            "Rear's locking, careful! Too much braking at the back. Move the bias forward a click.",
            "12 frames of rear lock into T1",
            true,
        ),
        (
            "Good lap, mate. That's a new personal best.",
            "1:47.312",
            false,
        ),
    ];
    let mut tick = 0u32;
    loop {
        let mut frame = OverlayFrame::empty();
        frame.set_flag(flags::CONNECTED, true);
        frame.speed_kmh = 180.0 + (tick % 60) as f32;
        frame.gear = 5;
        frame.delta_seconds = -0.214;
        let (said, detail, critical) = lines[(tick / 50) as usize % lines.len()];
        if tick % 50 < 40 {
            frame.set_radio(Some((said, detail, critical)));
        }
        sink.publish(&frame)?;
        tick += 1;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
