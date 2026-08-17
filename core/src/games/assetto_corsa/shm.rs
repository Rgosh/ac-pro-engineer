//! Reading Assetto Corsa's shared memory.
//!
//! Lifted out of the terminal, where it used to live. The user interface owned
//! the connection to the game, so anything else that wanted telemetry — a
//! broadcast to another machine, a second front end, a headless recorder — had
//! to go through a TUI to get at it. The core reads the game now and the
//! interfaces read the core.
//!
//! The names are AC's own. On Windows the game writes these mappings itself; on
//! Linux the game runs under Proton and `shm-bridge.exe` mirrors them into
//! `/dev/shm`, so the paths differ and the bytes do not.
//!
//! **They are also Competizione's**, which is the one thing to be careful
//! about here. ACC inherited this interface: the same three names, a different
//! layout, and no error of any kind when the wrong parser attaches — it reads
//! numbers, and they look like telemetry. On Linux both games mirror into the
//! same `/dev/shm` files, so a mapping left behind by one is still there when
//! the other starts. The version at the top of the static page is what tells
//! them apart, and [`page_is_ours`] is where that is checked.

use super::structs::{AcGraphics, AcPhysics, AcStatic, read_ac_string};
use crate::memory::SharedMemory;

#[cfg(target_os = "windows")]
static SHM_MEM_DIR: &str = "Local\\";
#[cfg(not(target_os = "windows"))]
static SHM_MEM_DIR: &str = "/dev/shm/";

static SHM_MEM_PHYSICS: &str = "acpmf_physics";
static SHM_MEM_GRAPHICS: &str = "acpmf_graphics";
static SHM_MEM_STATIC: &str = "acpmf_static";

/// The shared-memory contract this reader was written against.
///
/// Read off a live page rather than a header: the static capture in
/// `tests_suite/src/shm_layout_tests.rs`, taken from AC 1.16.4, reads `1.7`.
pub const SHARED_MEMORY_VERSION: &str = "1.7";

/// Whether a static page was written by a game speaking this reader's
/// contract.
///
/// `Ok(())` for Assetto Corsa and for a page nothing has published into yet;
/// `Err` with the version it found for anything else — which on this pair of
/// games means Competizione, whose graphics page inserts 964 bytes at offset
/// 252 and reads perfectly plausibly with this parser.
///
/// An **empty** version is allowed through deliberately. The mapping exists
/// before the game has written into it, and the simulator that stands in for
/// the game while developing writes no version at all; refusing a page of
/// zeros would turn "nothing published yet", which is normal, into "wrong
/// game", which is not.
pub fn page_is_ours(stat: &AcStatic) -> Result<(), String> {
    let declared = read_ac_string(&stat.sm_version);
    if declared.is_empty() || declared == SHARED_MEMORY_VERSION {
        return Ok(());
    }
    Err(format!(
        "these pages declare shared-memory version {declared}, and Assetto \
         Corsa publishes {SHARED_MEMORY_VERSION} — this is another game's \
         mapping under the same name, and reading it with this parser would \
         produce numbers rather than an error"
    ))
}

pub struct Memory {
    physics_mem: SharedMemory<AcPhysics>,
    graphics_mem: SharedMemory<AcGraphics>,
    static_mem: SharedMemory<AcStatic>,

    ac_physics: AcPhysics,
    ac_graphics: AcGraphics,
    ac_static: AcStatic,
}

impl Memory {
    pub fn try_connect() -> Result<Self, Box<dyn std::error::Error>> {
        let mut memory = Self {
            physics_mem: SharedMemory::<AcPhysics>::connect(&Self::get_mem(SHM_MEM_PHYSICS))?,
            graphics_mem: SharedMemory::<AcGraphics>::connect(&Self::get_mem(SHM_MEM_GRAPHICS))?,
            static_mem: SharedMemory::<AcStatic>::connect(&Self::get_mem(SHM_MEM_STATIC))?,
            ac_physics: AcPhysics::default(),
            ac_graphics: AcGraphics::default(),
            ac_static: AcStatic::default(),
        };
        // Refusing here rather than on the first tick means the caller is told
        // at the point it can still do something about it — try another game,
        // or say which one is actually publishing.
        memory.refresh()?;
        Ok(memory)
    }

    pub fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // The static page first, because it is the one that says which game
        // wrote these mappings. Checked every tick and not only on connecting:
        // a session of the other game can start while this connection is open,
        // and it publishes under the same name into the same file.
        let stat = self
            .static_mem
            .get()
            .map_err(|e| anyhow::format_err!("Cannot read static: {e:?}"))?;
        page_is_ours(&stat).map_err(|why| anyhow::format_err!("{why}"))?;
        self.ac_static = stat;

        // These two are rewritten by the game while we read them, so they go
        // through the tear-checking read. The static page is written once at
        // session load and does not need it.
        self.ac_physics = self
            .physics_mem
            .get_stable()
            .map_err(|e| anyhow::format_err!("Cannot read physics: {e:?}"))?;
        self.ac_graphics = self
            .graphics_mem
            .get_stable()
            .map_err(|e| anyhow::format_err!("Cannot read graphics: {e:?}"))?;
        Ok(())
    }

    fn get_mem(name: &str) -> String {
        format!("{}{}", SHM_MEM_DIR, name)
    }

    pub fn physics(&self) -> &AcPhysics {
        &self.ac_physics
    }

    pub fn graphics(&self) -> &AcGraphics {
        &self.ac_graphics
    }

    pub fn stat(&self) -> &AcStatic {
        &self.ac_static
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mapping nothing has published into yet says nothing about which game
    /// made it, and there is nothing in it to misread. The simulator that
    /// stands in for the game writes exactly this.
    #[test]
    fn an_empty_page_is_not_somebody_elses() {
        assert!(page_is_ours(&AcStatic::default()).is_ok());
    }

    #[test]
    fn our_own_version_is_accepted() {
        let mut stat = AcStatic::default();
        for (slot, unit) in stat
            .sm_version
            .iter_mut()
            .zip(SHARED_MEMORY_VERSION.encode_utf16())
        {
            *slot = unit;
        }
        assert!(page_is_ours(&stat).is_ok());
    }

    /// Competizione's contract, in Assetto Corsa's reader. It would otherwise
    /// parse: ACC's static page is longer, and the first 688 bytes of it are
    /// a perfectly valid `AcStatic` as far as `try_read_from_bytes` can tell.
    #[test]
    fn another_games_version_is_refused_by_name() {
        let mut stat = AcStatic::default();
        for (slot, unit) in stat.sm_version.iter_mut().zip("1.9".encode_utf16()) {
            *slot = unit;
        }
        let refusal = page_is_ours(&stat).expect_err("1.9 is not Assetto Corsa");
        assert!(refusal.contains("1.9"), "{refusal}");
        assert!(refusal.contains("1.7"), "{refusal}");
    }
}
