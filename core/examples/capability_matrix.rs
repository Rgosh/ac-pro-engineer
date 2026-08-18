//! What every game this build knows about can and cannot measure, as a table.
//!
//! ```text
//! cargo run -p ac_core --example capability_matrix
//! ```
//!
//! There are two copies of this table: this one, which is read out of
//! `games::registry` and therefore cannot be wrong, and the published one at
//! `/acc/`-style `/games/` on the site, which is a list in `site/build.py`.
//! **The site's copy is the one that can drift**, because nothing compiles it —
//! so this exists to be run beside it and read.
//!
//! It is also the fastest answer to "why has the advice gone quiet on this
//! game", which is a question the capability flags are designed to provoke: a
//! rule resting on a measurement a game does not make says nothing at all, and
//! the only way to tell that from a bug is to look at what the game reports.

use ac_core::games::registry::{self, Support};

/// One row: what the flag is called on screen, and how to read it off a game.
///
/// A `fn` per row rather than a match on a string, so adding a capability is a
/// compile error here until it is named — the table cannot silently stop
/// covering a flag.
type Row = (&'static str, fn(&ac_core::games::Capabilities) -> bool);

const ROWS: &[Row] = &[
    ("tyre wear", |c| c.tyre_wear),
    ("tread temperatures", |c| c.tyre_edge_temps),
    ("ride height", |c| c.ride_height),
    ("wind", |c| c.wind),
    ("brake pad and disc wear", |c| c.brake_wear),
    ("track grip", |c| c.track_grip),
    ("track limits", |c| c.lap_validity),
    ("sector times", |c| c.sectors),
    ("setups on disk", |c| c.setups),
    ("in-game panel", |c| c.in_game_panel),
];

fn main() {
    let playable: Vec<_> = registry::playable().collect();

    let widest = ROWS
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0)
        .max("what the game reports".len());

    print!("{:widest$}", "what the game reports");
    for game in &playable {
        print!("  {:>14}", game.short_name);
    }
    println!();
    println!("{}", "─".repeat(widest + playable.len() * 16));

    for (label, read) in ROWS {
        print!("{label:widest$}");
        for game in &playable {
            let yes = game.backend().is_some_and(|b| read(&b.capabilities));
            print!("  {:>14}", if yes { "yes" } else { "—" });
        }
        println!();
    }

    println!();
    println!("Steam appid, and the process that means the game is up:");
    for game in &playable {
        if let Some(backend) = game.backend() {
            println!(
                "  {:<28} {:>7}   {}",
                game.name,
                backend.app_id,
                backend.processes.join(", ")
            );
        }
    }

    // The ones with no folder yet say what stands in the way, in their own
    // words. A planned game carries no capabilities at all — a table of
    // defaults would read exactly like a game that measures nothing.
    let planned: Vec<_> = registry::GAMES
        .iter()
        .filter(|game| matches!(game.support, Support::Planned { .. }))
        .collect();
    if !planned.is_empty() {
        println!();
        println!("Not built yet, and why:");
        for game in planned {
            if let Support::Planned { needs } = &game.support {
                println!("\n  {}\n    {}", game.name, needs);
            }
        }
    }
}
