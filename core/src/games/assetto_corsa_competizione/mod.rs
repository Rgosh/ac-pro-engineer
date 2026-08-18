//! Assetto Corsa Competizione: its shared memory, its file layout, and reading
//! both.
//!
//! ACC publishes three pages under the same names Assetto Corsa uses —
//! `acpmf_physics`, `acpmf_graphics`, `acpmf_static` — and with different
//! layouts. That pair of facts is the whole reason this folder is careful:
//! the names match, so the wrong parser attaches without any error, and the
//! layouts do not, so it then reads plausible-looking nonsense. `shm::Memory`
//! refuses to attach to a page that declares the other game's contract.
//!
//! On Windows the game writes the pages itself; on Linux it is a Windows
//! process under Proton, so `shm-bridge.exe` runs inside **ACC's own prefix**
//! — appid 805550, not Assetto Corsa's — and mirrors them into `/dev/shm`.

pub mod paths;
pub mod reading;
pub mod shm;
pub mod structs;

use crate::games::{Capabilities, GameId, Reading, Source};

/// The identifier this game goes out under.
pub const GAME_ID: GameId = "assetto_corsa_competizione";

/// The processes that mean Assetto Corsa Competizione is running.
///
/// `AC2-Win64-Shipping.exe` is the game itself — ACC is an Unreal Engine title
/// and that is what Unreal calls the binary. `acc.exe` is the launcher Steam
/// starts, which then starts the game; it is listed because it is up first and
/// stays up, and because "the process is there" is only half of detection.
/// Telemetry has to be reachable as well, which is the other half.
///
/// `simulator.exe` is this project's own fake telemetry source — run it as
/// `simulator acc` and it publishes Competizione's pages instead of Assetto
/// Corsa's. It is listed for the same reason it is listed under Assetto Corsa:
/// for every purpose above this line it is the game.
///
/// Read off a machine that has the game rather than guessed, the same way
/// appid 805550 in `paths.rs` was.
pub const PROCESS_NAMES: &[&str] = &["AC2-Win64-Shipping.exe", "acc.exe", "simulator.exe"];

/// Whether ACC's telemetry can actually be read on this machine.
///
/// Same shape as Assetto Corsa's, and for the same reason: under Proton the
/// pages reach us only once the bridge mirrors them into `/dev/shm`, so a
/// running game with no mapping is a game this application cannot see.
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

/// What Assetto Corsa Competizione measures, each flag traced to the
/// recording that decided it.
///
/// This is not Assetto Corsa's list with a name changed. ACC measures a
/// different set, and four of the seven differ from Assetto Corsa's — three of
/// them false where AC's are true, and one true where AC's is false:
///
/// * `tyre_edge_temps` — the tread triplet at 368/384/400 is zero for the
///   whole session. This withholds the camber advice and the tread-temperature
///   band, which is the entire reason the flags exist.
/// * `tyre_wear` — offsets 120–132 are zero too. ACC publishes brake pad and
///   disc life instead, and a game that does not measure wear must not be
///   reported as four destroyed tyres.
/// * `sectors` — `current_sector_index` reached 2 and `last_sector_time`
///   117595 ms, so both move.
/// * `setups` — ACC keeps setups as JSON in a tree of its own, which nothing
///   here reads yet. False is the honest answer, and the Setup tab already
///   says so rather than showing an empty list.
pub const CAPABILITIES: Capabilities = Capabilities {
    // Both wind fields are zero for the whole recording.
    wind: false,
    // Zero for the whole recording — see `AccPhysics::ride_height`, which is
    // marked not published for the same reason. Reported as measured, four
    // zeros read as a car sitting on the tarmac.
    ride_height: false,
    tyre_edge_temps: false,
    sectors: true,
    setups: false,
    tyre_wear: false,
    // Offset 1240 is zero for the whole recording. ACC says how the track is
    // through `track_grip_status` instead — a named state, and turning that
    // into a fraction would be inventing a measurement. Left false, the
    // cold-pressure calculator stops adding 0.3 psi for a green track it was
    // never told about.
    track_grip: false,
    // 29 mm of pad and 32 mm of disc, per corner. This is what ACC measures
    // in place of tyre wear.
    brake_wear: true,
    // `is_valid_lap` at offset 1408, which moved during the session.
    lap_validity: true,
    // ACC is Unreal Engine and has no Custom Shaders Patch, so there is
    // nothing to load the panel. Offering to install it here would be
    // offering something that cannot work.
    in_game_panel: false,
};

/// A connection to a running Assetto Corsa Competizione.
pub struct Competizione {
    memory: shm::Memory,
}

impl Competizione {
    /// Connect to the game's shared memory.
    ///
    /// Fails when the pages are not there — on Linux most often because the
    /// bridge is not running in ACC's prefix — and also when they are there
    /// but were written by Assetto Corsa, which is a different problem with
    /// the same symptom and has to be said differently.
    pub fn connect() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            memory: shm::Memory::try_connect()?,
        })
    }
}

impl Source for Competizione {
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
