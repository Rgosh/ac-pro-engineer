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

use ac_core::config::AppConfig;
use ac_core::engineer::{Engineer, Severity};
use ac_core::games::assetto_corsa::structs::{AcGraphics, AcPhysics, AcStatic};
use ac_core::games::{Car, Session};
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
    let info = SessionInfo::default();

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

        // This probe reads AC's pages itself, so that a missing static page
        // still leaves something to print. Everything past this line works in
        // the neutral reading, like the application does.
        let (car, session): (Car, Session) = ((&phys).into(), (&gfx).into());

        engineer.update_config(&config);
        engineer.update(&car, &session, &info);
        let recommendations = engineer.analyze_live(&car, &session, None);

        println!("── sample {sample} ─────────────────────────────────────────────");
        println!(
            "speed {:6.1} km/h   gear {}   rpm {}   fuel {:.1} L{}",
            car.speed_kmh,
            car.gear,
            car.rpm,
            car.fuel_litres,
            stat.map(|s| format!("   max fuel {:.0} L", s.max_fuel))
                .unwrap_or_default()
        );
        println!(
            "pressure  {:5.1} {:5.1} {:5.1} {:5.1} psi",
            car.tyre_pressure_psi[0],
            car.tyre_pressure_psi[1],
            car.tyre_pressure_psi[2],
            car.tyre_pressure_psi[3]
        );
        println!(
            "tyre temp {:5.0} {:5.0} {:5.0} {:5.0} °C  (middle)",
            car.tyre_temp_middle_c[0],
            car.tyre_temp_middle_c[1],
            car.tyre_temp_middle_c[2],
            car.tyre_temp_middle_c[3]
        );
        println!(
            "brake     {:5.0} {:5.0} {:5.0} {:5.0} °C",
            car.brake_temp_c[0], car.brake_temp_c[1], car.brake_temp_c[2], car.brake_temp_c[3]
        );
        println!(
            "wear      {:5.1} {:5.1} {:5.1} {:5.1} %",
            car.tyre_wear[0], car.tyre_wear[1], car.tyre_wear[2], car.tyre_wear[3]
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
                    "  {} [{}] {:<12} {}",
                    rec.confidence_level().marker(),
                    severity_label(&rec.severity),
                    rec.component,
                    rec.message
                );
                // The reason this probe exists is to read the advice against
                // the numbers above it, and since v0.3.7 most of the advice has
                // more to say than one line. `confirm` especially: it is the
                // field that makes a rule checkable, and a rule whose check
                // reads oddly is a rule to go back to — which cannot be noticed
                // if the probe never prints it.
                if let Some(chain) = rec.chain.as_ref() {
                    println!("       why:     {}", chain.cause);
                    println!("       seen:    {}", chain.effect);
                    println!("       check:   {}", chain.confirm);
                }
            }
        }
        println!();

        sleep(Duration::from_secs(1));
    }
}
