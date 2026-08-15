//! Which games this build knows about, and which of them it can read.
//!
//! One table. A game is an entry in it, and adding one is a folder beside
//! `assetto_corsa/` plus a line here — not a search through the terminal for
//! every place that says the name of a simulator. Before this existed there
//! were ten such places.
//!
//! **A planned entry is not a stub.** It carries a name, a sentence about what
//! the game would still need, and nothing else: no guessed process name, no
//! guessed Steam appid, no guessed capabilities. Everything this project has
//! got wrong about a simulator's layout, it got wrong by writing down a number
//! somebody said rather than a number a capture proved — so a planned game
//! says what has to be *found out*, and the fields stay empty until it is.
//!
//! That is also why [`Support::Planned`] carries no [`Backend`]: a game that
//! cannot be read has no capabilities to report, and
//! `Capabilities::default()` sitting in a table would be indistinguishable
//! from a game that measures nothing.

use super::{Capabilities, CarSpecs, GameId, Source};
use crate::setup_manager::CarSetup;
use std::path::Path;

/// Read a game's installed cars, given a configured install path.
pub type ScanCars = fn(Option<&Path>) -> Vec<CarSpecs>;

/// Read the setups a game keeps for one car and track, given a configured
/// documents path.
pub type ScanSetups = fn(&str, &str, &Path) -> Vec<CarSetup>;

/// Reading and writing one game's setup files.
///
/// All four together or none: a build that can find a game's setups but not
/// write one back can offer a download button that does nothing, which is
/// worse than not offering it. `None` on a [`Backend`] is the honest state for
/// a game whose setups this program cannot read — iRacing keeps them in a
/// format of its own — and it is the same answer
/// [`Capabilities::setups`](super::Capabilities::setups) gives the screens.
pub struct SetupStore {
    pub scan: ScanSetups,
    /// The folder the game keeps setups in, honouring a configured override.
    pub root: fn(Option<&Path>) -> Option<std::path::PathBuf>,
    /// What a downloaded setup is called on disk. Both halves arrive
    /// sanitised: this decides the shape, not the safety.
    pub file_name: fn(&str, &str) -> String,
    /// A setup in the game's own format, ready to write.
    pub serialise: fn(&CarSetup) -> String,
}

/// Open a connection to a running game.
pub type Connect = fn() -> Result<Box<dyn Source + Send>, Box<dyn std::error::Error>>;

/// Everything needed to actually read a game.
///
/// Function pointers rather than a trait per concern: there is one
/// implementation of each of these today, and a trait with one implementation
/// is a guess about what the second one will need. A table of the functions
/// that already exist is not.
pub struct Backend {
    /// What the game measures. Travels onto every
    /// [`Reading`](super::Reading) and gates the advice that rests on it.
    pub capabilities: Capabilities,
    /// Process names that mean this game is up.
    pub processes: &'static [&'static str],
    /// Whether its telemetry can be reached on this machine right now — a
    /// second question from "is the process there", and one only the game can
    /// answer. Under Proton, Assetto Corsa needs its bridge before anything
    /// arrives.
    pub telemetry_is_reachable: fn() -> bool,
    pub connect: Connect,
    pub scan_cars: ScanCars,
    /// `None` where the game keeps no setups this program can read.
    pub setups: Option<SetupStore>,
}

/// How far this build has got with a game.
pub enum Support {
    /// Read today.
    Playable(Backend),
    /// Known and wanted. `needs` is the honest answer to "why not yet", in one
    /// sentence, and it is what the site lists.
    Planned { needs: &'static str },
}

pub struct Game {
    pub id: GameId,
    /// As the game calls itself, for a screen.
    pub name: &'static str,
    pub support: Support,
}

impl Game {
    pub fn backend(&self) -> Option<&Backend> {
        match &self.support {
            Support::Playable(backend) => Some(backend),
            Support::Planned { .. } => None,
        }
    }

    pub fn is_playable(&self) -> bool {
        self.backend().is_some()
    }
}

/// Every game this build knows about, playable or not.
///
/// The order is the order a list is drawn in: what works first, then what is
/// wanted, roughly by how close it is.
pub static GAMES: &[Game] = &[
    Game {
        id: super::assetto_corsa::GAME_ID,
        name: "Assetto Corsa",
        support: Support::Playable(Backend {
            capabilities: super::assetto_corsa::CAPABILITIES,
            processes: super::assetto_corsa::PROCESS_NAMES,
            telemetry_is_reachable: super::assetto_corsa::telemetry_is_reachable,
            connect: || {
                Ok(Box::new(super::assetto_corsa::AssettoCorsa::connect()?)
                    as Box<dyn Source + Send>)
            },
            scan_cars: |configured| {
                super::assetto_corsa::paths::ac_install_root(configured)
                    .map(|root| super::assetto_corsa::content::scan_cars(&root))
                    .unwrap_or_default()
            },
            setups: Some(SetupStore {
                scan: super::assetto_corsa::setups::scan_folders,
                root: super::assetto_corsa::setups::setups_root,
                file_name: super::assetto_corsa::setups::file_name,
                serialise: super::assetto_corsa::setups::generate_ini_content,
            }),
        }),
    },
    Game {
        id: "assetto_corsa_competizione",
        name: "Assetto Corsa Competizione",
        support: Support::Planned {
            needs: "Its shared memory uses the same page names as Assetto Corsa with a \
                    longer layout, so the structs have to be pinned from a real capture and \
                    the wrong parser has to be unable to attach. It publishes no inner or \
                    outer tyre temperature, so the camber and tread-temperature advice stay \
                    withheld. Setups are JSON in a different tree. There is no in-game \
                    panel: Custom Shaders Patch is an Assetto Corsa mod.",
        },
    },
    Game {
        id: "assetto_corsa_evo",
        name: "Assetto Corsa EVO",
        support: Support::Planned {
            needs: "Still in early access, and its telemetry surface is not settled. \
                    Nothing here should be written down until a capture from a released \
                    build says what it publishes.",
        },
    },
    Game {
        id: "iracing",
        name: "iRacing",
        support: Support::Planned {
            needs: "Publishes through a memory-mapped file of its own with a header and a \
                    session string, not three fixed pages — the first game that will not \
                    fit the shape Assetto Corsa suggested. Its setups are stored in a \
                    format this program cannot read, so that capability stays false.",
        },
    },
    Game {
        id: "rfactor2",
        name: "rFactor 2",
        support: Support::Planned {
            needs: "Telemetry needs a shared-memory plugin the driver installs into the \
                    game, so \"the game is running\" and \"the game can be read\" come \
                    apart further than under Proton — the launcher has to be able to say \
                    which one is missing.",
        },
    },
    Game {
        id: "le_mans_ultimate",
        name: "Le Mans Ultimate",
        support: Support::Planned {
            needs: "Built on rFactor 2's engine and read the same way, so it follows that \
                    one rather than standing on its own.",
        },
    },
];

/// The game this build reads unless told otherwise.
///
/// One playable entry today, so this cannot fail.
pub fn default_game() -> &'static Game {
    playable()
        .next()
        .expect("this build has at least one playable game")
}

/// Which playable game is up right now, if any.
///
/// Both halves have to agree: the process is there *and* the game's telemetry
/// can actually be reached. They are different questions and they come apart
/// in practice — under Proton, Assetto Corsa runs long before its bridge
/// mirrors anything, and rFactor 2 will not publish at all until the driver
/// installs a plugin. A launcher that treats "running" as "readable" tells
/// somebody to wait when it should be telling them what is missing.
///
/// With one playable entry this answers "is Assetto Corsa up", which is what
/// the terminal already asked. It becomes the selector unchanged when there is
/// a second.
pub fn detect_running() -> Option<&'static Game> {
    playable().find(|game| {
        game.backend().is_some_and(|backend| {
            backend
                .processes
                .iter()
                .any(|name| crate::process::is_process_running(name))
                && (backend.telemetry_is_reachable)()
        })
    })
}

/// The game to read: whichever is running, or the default to wait for.
///
/// Never `None`, because the terminal has to have something to name on its
/// launcher while nothing is running at all.
pub fn game_to_read() -> &'static Game {
    detect_running().unwrap_or_else(default_game)
}

pub fn playable() -> impl Iterator<Item = &'static Game> {
    GAMES.iter().filter(|game| game.is_playable())
}

pub fn by_id(id: GameId) -> Option<&'static Game> {
    GAMES.iter().find(|game| game.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_game_has_its_own_id() {
        let mut ids: Vec<GameId> = GAMES.iter().map(|game| game.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "two games share an id");
    }

    /// A planned game has no backend, and that is the whole safety property:
    /// there is no table of default capabilities for something nobody has
    /// captured, because a default reads exactly like a measurement.
    #[test]
    fn a_planned_game_carries_no_capabilities_to_be_believed() {
        for game in GAMES {
            match &game.support {
                Support::Planned { needs } => {
                    assert!(game.backend().is_none(), "{} is planned", game.name);
                    assert!(
                        needs.len() > 40,
                        "{} has to say what it still needs",
                        game.name
                    );
                }
                Support::Playable(backend) => {
                    assert!(
                        !backend.processes.is_empty(),
                        "{} must be recognisable when it runs",
                        game.name
                    );
                    // The flag the screens read and the functions that do the
                    // reading have to agree, or a tab says setups exist and
                    // finds nothing, or hides them while they are right there.
                    assert_eq!(
                        backend.capabilities.setups,
                        backend.setups.is_some(),
                        "{}: the setups capability and the setup store disagree",
                        game.name
                    );
                }
            }
        }
    }

    /// Assetto Corsa is playable, and its facts come from its own folder
    /// rather than being repeated here.
    #[test]
    fn assetto_corsa_is_the_one_that_works() {
        let ac = by_id(super::super::assetto_corsa::GAME_ID).expect("AC is in the table");
        let backend = ac.backend().expect("AC is playable");
        assert_eq!(
            backend.capabilities,
            super::super::assetto_corsa::CAPABILITIES
        );
        assert_eq!(
            backend.processes,
            super::super::assetto_corsa::PROCESS_NAMES
        );
        assert!(backend.setups.is_some(), "AC keeps setups this build reads");
        assert!(
            backend.capabilities.setups,
            "and the capability flag agrees with the store"
        );
        assert_eq!(default_game().id, ac.id);
    }

    /// Detection agrees with the table: with one playable entry it can only
    /// ever answer Assetto Corsa or nothing, and the fallback is that same
    /// entry either way.
    #[test]
    fn the_game_to_read_is_a_playable_one() {
        let chosen = game_to_read();
        assert!(chosen.is_playable(), "{} is not readable", chosen.name);
        if let Some(running) = detect_running() {
            assert_eq!(
                running.id, chosen.id,
                "a running game is the one that gets read"
            );
        } else {
            assert_eq!(chosen.id, default_game().id);
        }
    }

    /// The list is the plan, so it has to hold the games that were actually
    /// agreed rather than whatever was easy.
    #[test]
    fn the_planned_list_is_the_one_that_was_agreed() {
        for id in [
            "assetto_corsa_competizione",
            "assetto_corsa_evo",
            "iracing",
            "rfactor2",
            "le_mans_ultimate",
        ] {
            let game = by_id(id);
            assert!(game.is_some(), "{id} is in the table");
            assert!(
                game.is_some_and(|game| !game.is_playable()),
                "{id} is not built yet"
            );
        }
        assert_eq!(playable().count(), 1);
    }
}
