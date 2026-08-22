//! Assetto Corsa: its shared memory, its file layout, and reading both.
//!
//! AC publishes three pages — physics, graphics and static — into named shared
//! memory, and that is the whole interface. On Windows the game writes them
//! directly; on Linux the game is a Windows process under Proton, so
//! `shm-bridge.exe` runs inside the prefix and mirrors them out to `/dev/shm`.
//! Either way what arrives here is the same bytes in the same layout.

pub mod content;
pub mod paths;
pub mod reading;
pub mod setups;
pub mod shm;
pub mod structs;
pub mod tracks;

use crate::games::{Capabilities, GameId, Reading, Source};

/// The identifier this game goes out under.
pub const GAME_ID: GameId = "assetto_corsa";

/// The processes that mean Assetto Corsa is running.
///
/// `acs.exe` is the game, on Windows and equally under Proton, where it is a
/// Windows process either way. `simulator.exe` is this project's own fake
/// telemetry source, which stands in for the game while developing — the
/// launcher treats it as the game because for every purpose above this line
/// it is one.
///
/// Which process names mean "the game is up" is a fact about a game, and the
/// only place it can be checked against is the game itself. Steam appid 244210
/// is the other half of the same fact and lives in `paths.rs`.
pub const PROCESS_NAMES: &[&str] = &["acs.exe", "simulator.exe"];

/// Whether Assetto Corsa's telemetry can actually be read on this machine.
///
/// On Linux the game is a Windows process under Proton and its pages reach us
/// only once `shm-bridge.exe` mirrors them into `/dev/shm`, so a running
/// `acs.exe` with no mapping is a game this application cannot see. On Windows
/// the game writes the pages itself and the process being up is the whole
/// answer.
pub fn telemetry_is_reachable() -> bool {
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::path::Path::new("/dev/shm/acpmf_physics").exists()
    }
}

/// What Assetto Corsa measures.
///
/// All four, and every one of them checked against a real capture rather than
/// assumed: `tests_suite/src/shm_layout_tests.rs` parses bytes taken verbatim
/// from a live `/dev/shm/acpmf_*`. AC does publish the inner and outer tyre
/// temperatures — 33.5 and 34.0 °C on a cold front-left — which is why the
/// camber advice is possible on this game at all.
///
/// A constant rather than only a method, because anything reading AC's pages
/// has to be able to say so without holding a connection: a probe reading
/// `/dev/shm` by hand is still reading Assetto Corsa, and an engineer that is
/// not told withholds every verdict that rests on a measurement.
pub const CAPABILITIES: Capabilities = Capabilities {
    wind: true,
    // Published per axle, and the bottoming and rake advice are built on it.
    ride_height: true,
    tyre_edge_temps: true,
    sectors: true,
    setups: true,
    tyre_wear: true,
    // 0.94 on a green track in the capture, so it is a measurement and the
    // cold-pressure calculator may lean on it.
    track_grip: true,
    // AC publishes neither pad nor disc thickness — its cars mostly have no
    // model for it — and it says how much tyre is left instead, which is the
    // opposite of Competizione.
    brake_wear: false,
    // AC never says whether a lap counted. Every lap is treated as valid
    // because nothing contradicts it, which is not the same as being told.
    lap_validity: false,
    // Custom Shaders Patch is an Assetto Corsa mod, so the panel is this
    // game's and no other's.
    in_game_panel: true,
};

/// A connection to a running Assetto Corsa.
///
/// Holds the three mappings and the last good reading of each. Constructing it
/// connects; if the game is not running that fails, and the caller retries —
/// which is what the application does once a second rather than treating a
/// closed game as a fatal error.
pub struct AssettoCorsa {
    memory: shm::Memory,
}

impl AssettoCorsa {
    /// Connect to the game's shared memory.
    ///
    /// Fails when the pages are not there, which on Linux most often means the
    /// bridge is not running rather than that the game is closed.
    pub fn connect() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            memory: shm::Memory::try_connect()?,
        })
    }
}

impl Source for AssettoCorsa {
    fn id(&self) -> GameId {
        GAME_ID
    }

    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn poll(&mut self) -> Option<Reading> {
        self.memory.refresh().ok()?;
        let mut reading = reading::reading_of(
            self.memory.physics(),
            self.memory.graphics(),
            self.memory.stat(),
        );
        reading.capabilities = self.capabilities();
        Some(reading)
    }
}
