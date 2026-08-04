//! Print what the engineer says, next to the numbers it said it about.
//!
//! The advice is only worth as much as its agreement with the telemetry, and
//! that agreement is hard to judge inside a running TUI where both scroll past.
//! This reads the same shared memory the application reads, runs the same
//! `analyze_live`, and puts the inputs and the output on screen together.
//!
//! ```text
//! cargo run --bin simulator          # in one terminal
//! cargo run -p ac_core --example engineer_probe
//! ```
//!
//! Linux only: it reads `/dev/shm` directly, which is where Proton and the
//! simulator both put AC's maps.

use ac_core::ac_structs::{AcGraphics, AcPhysics, AcStatic};
use ac_core::config::AppConfig;
use ac_core::engineer::{Engineer, Severity};
use ac_core::session_info::SessionInfo;
use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;

/// Read one of AC's maps out of `/dev/shm`.
///
/// The maps are plain files there, written by the game (through Proton) or by
/// `cargo run --bin simulator`. A short file means the writer has not finished
/// its first update, which is worth waiting out rather than reporting as an
/// error.
fn read_map<T: Copy>(name: &str) -> Option<T> {
    let bytes = std::fs::read(format!("/dev/shm/{name}")).ok()?;
    if bytes.len() < size_of::<T>() {
        return None;
    }
    // SAFETY: the file is at least as large as the struct, and AC's maps are
    // `#[repr(C)]` images of exactly these types. `read_unaligned` because
    // nothing guarantees the buffer's alignment.
    Some(unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const T) })
}

fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical => "CRITICAL",
        Severity::Warning => "WARNING ",
        Severity::Info => "INFO    ",
    }
}

fn main() {
    let samples: usize = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(8);

    let config = AppConfig::default();
    let mut engineer = Engineer::new(&config);
    let session = SessionInfo::default();

    println!("reading /dev/shm/acpmf_* — {samples} samples, one per second\n");

    for sample in 1..=samples {
        let (Some(phys), Some(gfx)) = (
            read_map::<AcPhysics>("acpmf_physics"),
            read_map::<AcGraphics>("acpmf_graphics"),
        ) else {
            println!("no telemetry: is the game or the simulator running?");
            sleep(Duration::from_secs(1));
            continue;
        };
        let stat = read_map::<AcStatic>("acpmf_static");

        engineer.update_config(&config);
        engineer.update(&phys, &gfx, &session);
        let recommendations = engineer.analyze_live(&phys, &gfx, None);

        println!("── sample {sample} ─────────────────────────────────────────────");
        println!(
            "speed {:6.1} km/h   gear {}   rpm {}   fuel {:.1} L{}",
            phys.speed_kmh,
            phys.gear - 1,
            phys.rpms,
            phys.fuel,
            stat.map(|s| format!("   max fuel {:.0} L", s.max_fuel))
                .unwrap_or_default()
        );
        println!(
            "pressure  {:5.1} {:5.1} {:5.1} {:5.1} psi",
            phys.wheels_pressure[0],
            phys.wheels_pressure[1],
            phys.wheels_pressure[2],
            phys.wheels_pressure[3]
        );
        println!(
            "tyre temp {:5.0} {:5.0} {:5.0} {:5.0} °C  (middle)",
            phys.tyre_temp_m[0], phys.tyre_temp_m[1], phys.tyre_temp_m[2], phys.tyre_temp_m[3]
        );
        println!(
            "brake     {:5.0} {:5.0} {:5.0} {:5.0} °C",
            phys.brake_temp[0], phys.brake_temp[1], phys.brake_temp[2], phys.brake_temp[3]
        );
        println!(
            "wear      {:5.1} {:5.1} {:5.1} {:5.1} %",
            phys.tyre_wear[0], phys.tyre_wear[1], phys.tyre_wear[2], phys.tyre_wear[3]
        );
        println!(
            "fuel/lap {:.2} L   laps left {:.1}   delta {:+.3} s",
            engineer.stats.fuel_consumption_rate,
            engineer.stats.fuel_laps_remaining,
            engineer.stats.current_delta
        );

        if recommendations.is_empty() {
            println!("advice: none");
        } else {
            println!("advice ({}):", recommendations.len());
            for rec in &recommendations {
                println!(
                    "  [{}] {:<12} {}",
                    severity_label(&rec.severity),
                    rec.component,
                    rec.message
                );
            }
        }
        println!();

        sleep(Duration::from_secs(1));
    }
}
