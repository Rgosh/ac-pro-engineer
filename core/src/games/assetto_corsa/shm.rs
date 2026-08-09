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

use super::structs::{AcGraphics, AcPhysics, AcStatic};
use crate::memory::SharedMemory;

#[cfg(target_os = "windows")]
static SHM_MEM_DIR: &str = "Local\\";
#[cfg(not(target_os = "windows"))]
static SHM_MEM_DIR: &str = "/dev/shm/";

static SHM_MEM_PHYSICS: &str = "acpmf_physics";
static SHM_MEM_GRAPHICS: &str = "acpmf_graphics";
static SHM_MEM_STATIC: &str = "acpmf_static";

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
        Ok(Self {
            physics_mem: SharedMemory::<AcPhysics>::connect(&Self::get_mem(SHM_MEM_PHYSICS))?,
            graphics_mem: SharedMemory::<AcGraphics>::connect(&Self::get_mem(SHM_MEM_GRAPHICS))?,
            static_mem: SharedMemory::<AcStatic>::connect(&Self::get_mem(SHM_MEM_STATIC))?,
            ac_physics: AcPhysics::default(),
            ac_graphics: AcGraphics::default(),
            ac_static: AcStatic::default(),
        })
    }

    pub fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
        self.ac_static = self
            .static_mem
            .get()
            .map_err(|e| anyhow::format_err!("Cannot read static: {e:?}"))?;
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
