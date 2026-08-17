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
    /// Steam's number for this game.
    ///
    /// On Linux it names the Proton prefix, which is the only thing that
    /// decides where `shm-bridge.exe` has to run: a bridge started in the
    /// wrong prefix creates mappings the game never writes into, and every
    /// symptom of that looks like the game not publishing. It used to be a
    /// constant in the launcher, which meant one game.
    pub app_id: u32,
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
    /// The same, for somewhere there is no room.
    ///
    /// The launcher's menu column is 36 cells wide with a border and a
    /// prefix on every row, so "Assetto Corsa Competizione" is cut in half —
    /// and a selector whose two options both read `< Assetto Cor` is not a
    /// selector. Written down beside the name rather than truncated at the
    /// call site, because the game is the only thing that knows which half of
    /// its name identifies it.
    pub short_name: &'static str,
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
        short_name: "Assetto Corsa",
        support: Support::Playable(Backend {
            capabilities: super::assetto_corsa::CAPABILITIES,
            processes: super::assetto_corsa::PROCESS_NAMES,
            app_id: super::assetto_corsa::paths::AC_APP_ID_NUMBER,
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
        id: super::assetto_corsa_competizione::GAME_ID,
        name: "Assetto Corsa Competizione",
        short_name: "Competizione",
        support: Support::Playable(Backend {
            capabilities: super::assetto_corsa_competizione::CAPABILITIES,
            processes: super::assetto_corsa_competizione::PROCESS_NAMES,
            app_id: super::assetto_corsa_competizione::paths::ACC_APP_ID_NUMBER,
            telemetry_is_reachable: super::assetto_corsa_competizione::telemetry_is_reachable,
            connect: || {
                Ok(
                    Box::new(super::assetto_corsa_competizione::Competizione::connect()?)
                        as Box<dyn Source + Send>,
                )
            },
            // Competizione keeps its cars inside packed Unreal assets, so
            // there is nothing on disk to read specifications from. An empty
            // list is the honest answer; see its `paths::scan_cars`.
            scan_cars: super::assetto_corsa_competizione::paths::scan_cars,
            // Its setups are JSON in a tree of its own, which nothing here
            // reads yet. `None` and `capabilities.setups: false` together are
            // a supported state, not a broken screen — the Setup tab says the
            // game keeps none this program can read.
            setups: None,
        }),
    },
    Game {
        id: "assetto_corsa_evo",
        name: "Assetto Corsa EVO",
        short_name: "AC EVO",
        support: Support::Planned {
            needs: "Still in early access, and its telemetry surface is not settled. \
                    Nothing here should be written down until a capture from a released \
                    build says what it publishes.",
        },
    },
    Game {
        id: "iracing",
        name: "iRacing",
        short_name: "iRacing",
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
        short_name: "rFactor 2",
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
        short_name: "Le Mans",
        support: Support::Planned {
            needs: "Built on rFactor 2's engine and read the same way, so it follows that \
                    one rather than standing on its own.",
        },
    },
];

/// The game this build reads unless told otherwise.
///
/// The first playable entry, which is the order the table is written in: what
/// has been supported longest, first.
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
/// **It does not decide which game is read** — `config.game` does, through
/// [`chosen`]. This answers the other question, and one worth asking: is the
/// game the driver said they were in actually running? It is what the launcher
/// draws as "WAITING FOR SIMULATOR…".
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

/// The game this program is working with: the one that was chosen.
///
/// Never `None`, because the terminal has to name something on its launcher
/// while nothing is running at all, and an unknown or empty id — every
/// configuration written before there was a second game — means the default.
///
/// **This replaced "whichever game is running", deliberately.** Detection is
/// still here and still useful for telling somebody their game is up, but it
/// is the wrong thing to *decide* with: both games publish under the same
/// three page names and, on Linux, mirror into the same `/dev/shm` files, so
/// the answer can be confidently wrong. What follows from a wrong answer is
/// expensive and quiet — the bridge started in the other game's Proton prefix,
/// an engineer running Assetto Corsa's tyre thresholds against a GT3, a stint
/// judged with capabilities the running game does not have. A driver saying
/// which game they are in costs one keypress and cannot be wrong.
pub fn chosen(id: &str) -> &'static Game {
    by_id(id)
        .filter(|game| game.is_playable())
        .unwrap_or_else(default_game)
}

/// Every game this build can actually read, in the order they are offered.
pub fn selectable() -> Vec<&'static Game> {
    playable().collect()
}

/// The game after this one in the selectable list, wrapping round.
///
/// Wrapping rather than stopping at the end: this is two entries today and a
/// driver flicking through them should not have to know which direction they
/// are at the end of.
pub fn next_to(id: &str, forwards: bool) -> &'static Game {
    let games = selectable();
    let here = games
        .iter()
        .position(|game| game.id == chosen(id).id)
        .unwrap_or(0);
    let count = games.len();
    let there = if forwards {
        (here + 1) % count
    } else {
        (here + count - 1) % count
    };
    games[there]
}

pub fn playable() -> impl Iterator<Item = &'static Game> {
    GAMES.iter().filter(|game| game.is_playable())
}

/// The entry with this id, if this build has one.
///
/// Takes any string rather than a [`GameId`]: the ids that arrive from outside
/// — a configuration file, a broadcast message — are borrowed and may name a
/// game this build has never heard of, which is a `None` and not a failure.
pub fn by_id(id: &str) -> Option<&'static Game> {
    GAMES.iter().find(|game| game.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The launcher draws the short name inside a 36-cell column, after a
    /// prefix, a label and the two arrows that say it can be changed. Anything
    /// much longer than this is cut off, and a cut-off name is a selector that
    /// cannot be read.
    #[test]
    fn a_short_name_fits_where_it_has_to_go() {
        for game in GAMES {
            assert!(
                !game.short_name.is_empty() && game.short_name.chars().count() <= 16,
                "{} calls itself {:?} in short, which does not fit the menu",
                game.name,
                game.short_name
            );
            assert!(
                game.name.contains(game.short_name) || game.short_name.len() < game.name.len(),
                "{}'s short name should be recognisably its own",
                game.name
            );
        }
    }

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

    /// Whatever is in a configuration file, the game that comes back is one
    /// this build can actually read — including for the two ways a
    /// configuration can name a game that is not readable: an id from a
    /// build that supported more games, and a planned entry.
    #[test]
    fn the_chosen_game_is_always_a_playable_one() {
        for id in [
            "",
            "assetto_corsa",
            "assetto_corsa_competizione",
            "iracing",
            "some_game_this_build_never_heard_of",
        ] {
            let game = chosen(id);
            assert!(game.is_playable(), "{id} chose {}", game.name);
        }
        assert_eq!(chosen("").id, default_game().id, "empty means the default");
        assert_eq!(
            chosen("iracing").id,
            default_game().id,
            "and so does a game that is only planned"
        );
        assert_eq!(
            chosen("assetto_corsa_competizione").id,
            "assetto_corsa_competizione"
        );
    }

    /// Flicking through the list gets to every game and comes back, from
    /// either direction.
    #[test]
    fn the_selector_wraps_round_the_readable_games() {
        let games = selectable();
        assert_eq!(games.len(), playable().count());

        let mut id = default_game().id;
        for _ in 0..games.len() {
            id = next_to(id, true).id;
        }
        assert_eq!(id, default_game().id, "forwards, all the way round");

        for _ in 0..games.len() {
            id = next_to(id, false).id;
        }
        assert_eq!(id, default_game().id, "and backwards");

        if games.len() > 1 {
            assert_ne!(next_to(default_game().id, true).id, default_game().id);
        }
    }

    /// Detection is still asked, and it still only ever answers with a game
    /// this build can read — it just no longer decides which one is read.
    #[test]
    fn detection_only_ever_names_a_playable_game() {
        if let Some(game) = detect_running() {
            assert!(game.is_playable(), "{} is not readable", game.name);
        }
    }

    /// Competizione is the second game this build reads, and its facts come
    /// from its own folder rather than being repeated here.
    #[test]
    fn competizione_is_the_second_one_that_works() {
        let acc = by_id(super::super::assetto_corsa_competizione::GAME_ID)
            .expect("Competizione is in the table");
        let backend = acc.backend().expect("Competizione is playable");

        assert_eq!(
            backend.capabilities,
            super::super::assetto_corsa_competizione::CAPABILITIES
        );
        assert_eq!(
            backend.processes,
            super::super::assetto_corsa_competizione::PROCESS_NAMES
        );
        assert!(
            !backend.capabilities.tyre_wear && !backend.capabilities.tyre_edge_temps,
            "the game publishes neither, and the capture is what says so"
        );
        assert!(
            backend.setups.is_none() && !backend.capabilities.setups,
            "its setups are JSON in a tree nothing here reads yet"
        );

        // The default is unchanged by a second game arriving: the terminal has
        // to name something on its launcher while nothing is running, and that
        // is still the game this build has read longest.
        assert_eq!(default_game().id, super::super::assetto_corsa::GAME_ID);
    }

    /// The two games publish under the same three names and into the same
    /// `/dev/shm` files, so everything that tells them apart has to actually
    /// differ.
    ///
    /// One process name is deliberately on both lists: this project's own
    /// simulator, which stands in for whichever game it was started as. That
    /// is the case that shows why detection cannot be what *decides* — the
    /// same executable is either game — and it is why the decision is a
    /// setting.
    #[test]
    fn the_two_readable_games_are_told_apart_by_every_means_there_is() {
        let ac = by_id(super::super::assetto_corsa::GAME_ID)
            .and_then(Game::backend)
            .expect("AC is playable");
        let acc = by_id(super::super::assetto_corsa_competizione::GAME_ID)
            .and_then(Game::backend)
            .expect("Competizione is playable");

        assert_ne!(ac.app_id, acc.app_id, "different Proton prefixes");
        let shared: Vec<&&str> = ac
            .processes
            .iter()
            .filter(|process| acc.processes.contains(process))
            .collect();
        assert_eq!(
            shared,
            vec![&"simulator.exe"],
            "only the stand-in may name both games; a real executable naming \
             both would make detection ambiguous"
        );
        assert_ne!(
            ac.capabilities, acc.capabilities,
            "they do not measure the same things, and saying they do is how a \
             verdict gets made about a number nobody took"
        );
    }

    /// The list is the plan, so it has to hold the games that were actually
    /// agreed rather than whatever was easy.
    #[test]
    fn the_planned_list_is_the_one_that_was_agreed() {
        for id in [
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
        assert_eq!(playable().count(), 2);
    }
}
