//! Assetto Corsa: its shared memory, its file layout, and reading both.
//!
//! AC publishes three pages — physics, graphics and static — into named shared
//! memory, and that is the whole interface. On Windows the game writes them
//! directly; on Linux the game is a Windows process under Proton, so
//! `shm-bridge.exe` runs inside the prefix and mirrors them out to `/dev/shm`.
//! Either way what arrives here is the same bytes in the same layout.

pub mod paths;
pub mod shm;
pub mod structs;

use crate::games::{Capabilities, GameId, Source};
use structs::{AcGraphics, AcPhysics, AcStatic};

/// The identifier this game goes out under.
pub const GAME_ID: GameId = "assetto_corsa";

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

    pub fn physics(&self) -> &AcPhysics {
        self.memory.physics()
    }

    pub fn graphics(&self) -> &AcGraphics {
        self.memory.graphics()
    }

    pub fn stat(&self) -> &AcStatic {
        self.memory.stat()
    }
}

impl Source for AssettoCorsa {
    fn id(&self) -> GameId {
        GAME_ID
    }

    fn capabilities(&self) -> Capabilities {
        // All four, and every one of them checked against a real capture rather
        // than assumed: `tests_suite/src/shm_layout_tests.rs` parses bytes taken
        // verbatim from a live `/dev/shm/acpmf_*`. AC does publish the inner and
        // outer tyre temperatures — 33.5 and 34.0 °C on a cold front-left — which
        // is why the camber advice is possible here at all.
        Capabilities {
            tyre_edge_temps: true,
            sectors: true,
            setups: true,
            tyre_wear: true,
        }
    }

    fn poll(&mut self) -> bool {
        self.memory.refresh().is_ok()
    }
}
