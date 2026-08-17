//! Reading Assetto Corsa Competizione's shared memory, and refusing to read
//! anybody else's.
//!
//! ACC publishes under **the same three names Assetto Corsa uses**, because it
//! inherited the interface: `acpmf_physics`, `acpmf_graphics`, `acpmf_static`.
//! The names match and the layouts do not, which is the most dangerous
//! property this pair of games has — a reader that attaches to the wrong one
//! gets numbers, not an error, and then says confident things about them.
//!
//! On Linux the two games mirror into the *same* `/dev/shm` files, so the
//! mapping left behind by a session of one game is still sitting there when
//! the other starts. Running both at once is not a supported state; this file
//! is what stops it being a silent one.
//!
//! # How the two are told apart
//!
//! Both games write a version at the top of the static page saying which
//! shared-memory contract they implement: Assetto Corsa writes `1.7`,
//! Competizione writes `1.9`. That is what a version field is for, and it is
//! the one byte range both games write at the same offset in the same
//! encoding, so it can be read before anything else is trusted.
//!
//! A page that is **all zeros declares nothing**, and is allowed through: the
//! mapping exists before the game has published into it, and there is nothing
//! there to misread yet.

use super::structs::{AccGraphics, AccPhysics, AccStatic, read_acc_string};
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
/// Read off a live ACC page rather than a header: the static page of the
/// recording in the repository root reads `1.9`.
pub const SHARED_MEMORY_VERSION: &str = "1.9";

/// Whether a static page was written by a game speaking this reader's
/// contract.
///
/// `Ok(())` for ACC and for a page nothing has published into yet; `Err` with
/// the version it found for anything else, which on this pair of games means
/// Assetto Corsa.
pub fn page_is_ours(stat: &AccStatic) -> Result<(), String> {
    let declared = read_acc_string(&stat.sm_version);
    if declared.is_empty() || declared == SHARED_MEMORY_VERSION {
        return Ok(());
    }
    Err(format!(
        "these pages declare shared-memory version {declared}, and Assetto \
         Corsa Competizione publishes {SHARED_MEMORY_VERSION} — this is \
         another game's mapping under the same name, and reading it with this \
         parser would produce numbers rather than an error"
    ))
}

pub struct Memory {
    physics_mem: SharedMemory<AccPhysics>,
    graphics_mem: SharedMemory<AccGraphics>,
    static_mem: SharedMemory<AccStatic>,

    acc_physics: AccPhysics,
    acc_graphics: AccGraphics,
    acc_static: AccStatic,
}

impl Memory {
    pub fn try_connect() -> Result<Self, Box<dyn std::error::Error>> {
        let mut memory = Self {
            physics_mem: SharedMemory::<AccPhysics>::connect(&Self::get_mem(SHM_MEM_PHYSICS))?,
            graphics_mem: SharedMemory::<AccGraphics>::connect(&Self::get_mem(SHM_MEM_GRAPHICS))?,
            static_mem: SharedMemory::<AccStatic>::connect(&Self::get_mem(SHM_MEM_STATIC))?,
            acc_physics: AccPhysics::default(),
            acc_graphics: AccGraphics::default(),
            acc_static: AccStatic::default(),
        };
        // Refusing here rather than on the first tick means the caller is told
        // at the point it can still do something about it — pick another game,
        // or say which one is actually publishing.
        memory.refresh()?;
        Ok(memory)
    }

    pub fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Checked every tick and not only on connecting: a session of the
        // other game can start while this connection is open, and the mapping
        // it writes has the same name and this one's file handle still points
        // at it.
        let stat = self
            .static_mem
            .get()
            .map_err(|e| anyhow::format_err!("Cannot read static: {e:?}"))?;
        page_is_ours(&stat).map_err(|why| anyhow::format_err!("{why}"))?;
        self.acc_static = stat;

        // These two are rewritten by the game while we read them, so they go
        // through the tear-checking read. The static page is written once at
        // session load and does not need it.
        self.acc_physics = self
            .physics_mem
            .get_stable()
            .map_err(|e| anyhow::format_err!("Cannot read physics: {e:?}"))?;
        self.acc_graphics = self
            .graphics_mem
            .get_stable()
            .map_err(|e| anyhow::format_err!("Cannot read graphics: {e:?}"))?;
        Ok(())
    }

    fn get_mem(name: &str) -> String {
        format!("{}{}", SHM_MEM_DIR, name)
    }

    pub fn physics(&self) -> &AccPhysics {
        &self.acc_physics
    }

    pub fn graphics(&self) -> &AccGraphics {
        &self.acc_graphics
    }

    pub fn stat(&self) -> &AccStatic {
        &self.acc_static
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mapping nothing has published into yet says nothing about which game
    /// made it, and there is nothing in it to misread.
    #[test]
    fn an_empty_page_is_not_somebody_elses() {
        assert!(page_is_ours(&AccStatic::default()).is_ok());
    }

    #[test]
    fn our_own_version_is_accepted() {
        let mut stat = AccStatic::default();
        for (slot, unit) in stat
            .sm_version
            .iter_mut()
            .zip(SHARED_MEMORY_VERSION.encode_utf16())
        {
            *slot = unit;
        }
        assert!(page_is_ours(&stat).is_ok());
    }

    /// Assetto Corsa's contract, in Competizione's reader. The layouts differ
    /// from offset 252 of the graphics page onward, so this is the check that
    /// keeps the difference from being read as telemetry.
    #[test]
    fn another_games_version_is_refused_by_name() {
        let mut stat = AccStatic::default();
        for (slot, unit) in stat.sm_version.iter_mut().zip("1.7".encode_utf16()) {
            *slot = unit;
        }
        let refusal = page_is_ours(&stat).expect_err("1.7 is not Competizione");
        assert!(refusal.contains("1.7"), "{refusal}");
        assert!(refusal.contains("1.9"), "{refusal}");
    }
}
