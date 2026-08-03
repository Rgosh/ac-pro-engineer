//! Publish one known frame, for the Lua conformance check.
use ac_core::overlay::frame::{OverlayFrame, flags};
use ac_core::overlay::shared_writer::OverlayWriter;

fn main() {
    let mut w = OverlayWriter::open_named("acpe-luacheck").expect("open");
    let mut f = OverlayFrame::empty();
    f.speed_kmh = 213.5;
    f.rpm = 7400;
    f.max_rpm = 8500;
    f.gear = 4;
    f.fuel_litres = 42.25;
    f.fuel_laps_remaining = 12.5;
    f.delta_seconds = -0.372;
    f.tyre_pressure_psi = [27.1, 27.3, 26.8, 26.9];
    f.tyre_temp_c = [88.0, 91.5, 84.25, 85.75];
    f.tyre_wear_percent = [98.5, 98.25, 99.0, 98.75];
    f.set_flag(flags::CONNECTED, true);
    f.set_flag(flags::SHOW_TELEMETRY, true);
    f.set_flag(flags::PIT_LIMITER, true);
    w.publish(&f);
    println!("published to /dev/shm/acpe-luacheck");
}
