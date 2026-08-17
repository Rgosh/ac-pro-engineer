//! Finding the Assetto Corsa installation and its Documents folder.
//!
//! Two directories matter and they are not in the same place:
//!
//! * the **install root**, which holds `content/cars` — where car specs and
//!   the reference data come from
//! * the **Documents folder**, which holds `Assetto Corsa/setups` — where the
//!   game reads and writes car setups
//!
//! On Windows these are a Steam library folder and the user's Documents. On
//! Linux the game runs under Proton, so the install root is a normal Steam
//! library path but Documents lives *inside the Proton prefix* — the game is a
//! Windows process and writes to what it believes is `C:\Users\steamuser\
//! Documents`. Resolving that to `~/Documents` finds nothing, which is why
//! local setups were never discovered on Linux.
//!
//! Both can be overridden from the config, which is the escape hatch for a
//! non-Steam install, a second library, or a prefix somewhere unusual.
//!
//! Finding Steam itself is not a fact about Assetto Corsa and lives in
//! [`crate::steam`]. What is left here is the four things that *are* facts
//! about this game: its appid, its folder name, where its documents sit inside
//! that folder, and what an install looks like when it is not in a library at
//! all.

use crate::steam;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Steam's app id for Assetto Corsa. Names the Proton prefix.
///
/// Only the Proton lookup needs it, and that is Linux-only — on Windows the
/// game writes to the real Documents folder and there is no prefix to find.
#[cfg(not(target_os = "windows"))]
/// Assetto Corsa on Steam.
///
/// Written down once: the Proton bridge is launched against this appid and the
/// install is found under it, and the two disagreeing is a class of bug that
/// costs an evening. ACC is 805550 and belongs to its own folder.
pub const AC_APP_ID: &str = "244210";

/// The same, for the callers that need it as a number.
pub const AC_APP_ID_NUMBER: u32 = 244210;

/// Directory name of the game inside a Steam library.
const AC_DIR_NAME: &str = "assettocorsa";

/// Resolve the Assetto Corsa install root.
///
/// `configured` wins if it is set and exists, so a user with an install this
/// cannot find is never stuck.
pub fn ac_install_root(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|p| !p.as_os_str().is_empty()) {
        if path.exists() {
            debug!("Using configured AC install path: {}", path.display());
            return Some(path.to_path_buf());
        }
        info!(
            "Configured AC install path does not exist, falling back to auto-detection: {}",
            path.display()
        );
    }

    if let Some(from_steam) = steam::install_dir(AC_DIR_NAME) {
        return Some(from_steam);
    }

    // A copy that is not in a Steam library at all: moved to another disk by
    // hand, or installed from somewhere that is not Steam. Only reached when
    // every library has been looked in, so it costs nothing in the normal case.
    #[cfg(target_os = "windows")]
    {
        for drive in steam::drive_roots() {
            for candidate in [
                drive.join(AC_DIR_NAME),
                drive.join("Games").join(AC_DIR_NAME),
                drive.join("Games").join("Assetto Corsa"),
            ] {
                if candidate.join("content").join("cars").is_dir() {
                    info!(
                        "Found Assetto Corsa outside Steam at {}",
                        candidate.display()
                    );
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Resolve the Documents folder Assetto Corsa reads setups from.
pub fn ac_documents_dir(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|p| !p.as_os_str().is_empty()) {
        if path.exists() {
            debug!("Using configured AC documents path: {}", path.display());
            return Some(path.to_path_buf());
        }
        info!(
            "Configured AC documents path does not exist, falling back to auto-detection: {}",
            path.display()
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Under Proton the game writes to the prefix, not to the host's
        // ~/Documents. Prefer the prefix and only fall back to the host
        // directory for a native or Wine-less setup.
        if let Some(prefix_docs) = steam::proton_documents_dir(AC_APP_ID) {
            return Some(prefix_docs);
        }
    }

    steam::host_documents_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory of its own per test, so the suite stays parallel.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create tmp");
        dir
    }

    /// The configured path is the escape hatch for an install this module
    /// cannot find, so it has to win over detection.
    #[test]
    fn a_configured_install_path_is_preferred() {
        let configured = scratch_dir("acpaths_configured_root");
        assert_eq!(ac_install_root(Some(&configured)), Some(configured));
    }

    /// ...but only when it exists, or a stale config would silently disable
    /// detection with no way to tell why.
    #[test]
    fn a_configured_path_that_does_not_exist_falls_back_to_detection() {
        let missing = PathBuf::from("/nonexistent/assettocorsa");
        assert_ne!(ac_install_root(Some(&missing)), Some(missing));
    }

    #[test]
    fn an_empty_configured_path_is_ignored() {
        let empty = PathBuf::new();
        assert_ne!(ac_install_root(Some(&empty)), Some(empty));
    }

    #[test]
    fn a_configured_documents_dir_is_preferred() {
        let configured = scratch_dir("acpaths_configured_docs");
        assert_eq!(ac_documents_dir(Some(&configured)), Some(configured));
    }
}
