//! Print what the engineer says, next to the numbers it said it about.
//!
//! The advice is only worth as much as its agreement with the telemetry, and
//! that agreement is hard to judge inside a running TUI where both scroll past.
//! This reads the same shared memory the application reads, runs the same
//! `analyze_live`, and puts the inputs and the output on screen together.
//!
//! ```text
//! cargo run --bin simulator                 # in one terminal
//! cargo run -p ac_core --example engineer_probe
//!
//! cargo run --bin simulator acc             # or the other game
//! cargo run -p ac_core --example engineer_probe -- 8 assetto_corsa_competizione
//! ```
//!
//! **Which game is an argument**, because the advice is the thing being
//! judged and the advice depends on what the game measures: Competizione
//! publishes no tyre wear and no tread temperatures, so a probe that read its
//! pages while claiming Assetto Corsa's capabilities would print exactly the
//! confident nonsense this whole layer exists to prevent.
//!
//! It connects the way the application does — through the registry — so a game
//! whose pages are not there, or whose pages were written by the *other* game,
//! is reported rather than read.

use ac_core::config::AppConfig;
use ac_core::engineer::{Engineer, Severity};
use ac_core::games::registry;
use ac_core::session_info::SessionInfo;
use std::thread::sleep;
use std::time::Duration;

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
    let wanted = std::env::args().nth(2).unwrap_or_default();

    let game = registry::chosen(&wanted);
    let Some(backend) = game.backend() else {
        println!("{} is not a game this build can read", game.name);
        return;
    };

    let config = AppConfig::default();
    let mut engineer = Engineer::new(&config);
    let info = SessionInfo::default();

    println!(
        "reading {} — {samples} samples, one per second\n",
        game.name
    );

    let mut source = match (backend.connect)() {
        Ok(source) => source,
        Err(error) => {
            println!("could not connect: {error}");
            println!("is the game or the simulator running? (`simulator acc` for Competizione)");
            return;
        }
    };

    for sample in 1..=samples {
        let Some(reading) = source.poll() else {
            println!("no telemetry this tick");
            sleep(Duration::from_secs(1));
            continue;
        };
        let (car, session) = (reading.car, reading.session);

        engineer.update_config(&config);
        // The engineer is told what *this* game measures, off the same reading
        // rather than from a constant chosen here. Without it every verdict
        // resting on a tyre measurement is withheld — correctly, since an
        // engineer that has not been told which game it is reading cannot know
        // a default from a reading.
        engineer.update_capabilities(reading.capabilities);
        // And what kind of car it is, which is what decides whether 520 °C at
        // the front is a working GT3 or a road car with boiled fluid.
        let class = ac_core::games::CarClass::identify(&reading.fixed.car_model, &[]);
        engineer.update_car_class(class);
        engineer.update(&car, &session, &info);
        let recommendations = engineer.analyze_live(&car, &session, None);

        println!("── sample {sample} ─────────────────────────────────────────────");
        println!(
            "{} — read as {} (tyres {:.0}–{:.0} °C, brakes {:.0}/{:.0} °C)",
            if reading.fixed.car_model.is_empty() {
                "no car"
            } else {
                &reading.fixed.car_model
            },
            class.label(),
            class.window().tyre_c.0,
            class.window().tyre_c.1,
            class.window().brake_front_max_c,
            class.window().brake_rear_max_c,
        );
        println!(
            "speed {:6.1} km/h   gear {}   rpm {}   fuel {:.1} L   max fuel {:.0} L",
            car.speed_kmh, car.gear, car.rpm, car.fuel_litres, reading.fixed.max_fuel_litres
        );
        println!(
            "pressure  {:5.1} {:5.1} {:5.1} {:5.1} psi",
            car.tyre_pressure_psi[0],
            car.tyre_pressure_psi[1],
            car.tyre_pressure_psi[2],
            car.tyre_pressure_psi[3]
        );
        println!(
            "tyre temp {:5.0} {:5.0} {:5.0} {:5.0} °C  ({})",
            car.avg_tyre_temp_c(0),
            car.avg_tyre_temp_c(1),
            car.avg_tyre_temp_c(2),
            car.avg_tyre_temp_c(3),
            if reading.capabilities.tyre_edge_temps {
                "tread, mean of three"
            } else {
                "core — this game measures no tread"
            }
        );
        // **The three numbers the camber advice is actually made of.** The
        // mean above hides them, and the mean is not what any camber verdict
        // reads: inner minus outer is, and its *sign* is the whole verdict.
        // A driver reported the advice contradicting his car in v0.4.2, and
        // there was no way to ask this program what it had read — only what it
        // had concluded. Printed with the camber the wheel is running, since
        // the two together are what makes a verdict believable or not.
        if reading.capabilities.tyre_edge_temps {
            println!("tread     inner / middle / outer      I-O    camber");
            for (corner, name) in ["FL", "FR", "RL", "RR"].iter().enumerate() {
                println!(
                    "  {name}      {:5.1} {:5.1} {:5.1} °C   {:+6.1}   {:+6.2}°",
                    car.tyre_temp_inner_c[corner],
                    car.tyre_temp_middle_c[corner],
                    car.tyre_temp_outer_c[corner],
                    car.tyre_temp_inner_c[corner] - car.tyre_temp_outer_c[corner],
                    Engineer::camber_degrees(&car, corner)
                );
            }
        }
        println!(
            "brake     {:5.0} {:5.0} {:5.0} {:5.0} °C",
            car.brake_temp_c[0], car.brake_temp_c[1], car.brake_temp_c[2], car.brake_temp_c[3]
        );
        if reading.capabilities.tyre_wear {
            println!(
                "wear      {:5.1} {:5.1} {:5.1} {:5.1} %",
                car.tyre_wear[0], car.tyre_wear[1], car.tyre_wear[2], car.tyre_wear[3]
            );
        } else {
            println!("wear      not measured by this game");
        }
        if !reading.capabilities.tyre_edge_temps {
            println!("tread     not measured by this game (no camber advice)");
        }
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
