//! Where Assetto Corsa Competizione is installed, and where it keeps what it
//! writes.
//!
//! Read off a machine that has the game rather than copied from a forum post.
//! A guessed appid is the kind of thing that costs an evening the day it turns
//! out to be wrong, and there is no way to tell from the outside: every path
//! below still *resolves*, it just resolves to nothing.
//!
//! Finding Steam is not a fact about this game and lives in [`crate::steam`].
//! What is here is the four things that are: the appid, the folder name, where
//! the documents sit inside the Proton prefix, and the fact that there is
//! nothing on disk to scan for car specifications.

use crate::steam;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Assetto Corsa Competizione on Steam.
///
/// Confirmed from `appmanifest_805550.acf` in a Steam library — the same
/// manifest `tools/record-session.sh` reads to find the prefix the bridge has
/// to run inside. Assetto Corsa is 244210 and belongs to its own folder.
#[cfg(not(target_os = "windows"))]
pub const ACC_APP_ID: &str = "805550";

/// The same, for the callers that need it as a number — the Proton bridge is
/// launched against this.
pub const ACC_APP_ID_NUMBER: u32 = 805550;

/// Directory name of the game inside a Steam library. Steam's `installdir`,
/// spaces and capitals included, unlike Assetto Corsa's lowercase run-together
/// one.
const ACC_DIR_NAME: &str = "Assetto Corsa Competizione";

/// The folder the game writes into, inside whichever Documents folder applies.
const ACC_DOCUMENTS_NAME: &str = "Assetto Corsa Competizione";

/// Resolve the Competizione install root.
///
/// `configured` wins if it is set and exists, so a driver with an install this
/// cannot find is never stuck.
pub fn acc_install_root(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|p| !p.as_os_str().is_empty()) {
        if path.exists() {
            debug!("Using configured ACC install path: {}", path.display());
            return Some(path.to_path_buf());
        }
        info!(
            "Configured ACC install path does not exist, falling back to auto-detection: {}",
            path.display()
        );
    }

    steam::install_dir(ACC_DIR_NAME)
}

/// Resolve the folder Competizione writes its setups, configuration and
/// results into: `Documents/Assetto Corsa Competizione`.
///
/// Under Proton that is inside the game's own prefix — 805550, not Assetto
/// Corsa's 244210 — which is the whole reason each game owns its appid.
pub fn acc_documents_dir(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|p| !p.as_os_str().is_empty()) {
        if path.exists() {
            debug!("Using configured ACC documents path: {}", path.display());
            return Some(path.to_path_buf());
        }
        info!(
            "Configured ACC documents path does not exist, falling back to auto-detection: {}",
            path.display()
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(prefix_docs) = steam::proton_documents_dir(ACC_APP_ID) {
            let inside = prefix_docs.join(ACC_DOCUMENTS_NAME);
            if inside.exists() {
                return Some(inside);
            }
        }
    }

    steam::host_documents_dir()
        .map(|docs| docs.join(ACC_DOCUMENTS_NAME))
        .filter(|path| path.exists())
}

/// What car specifications this game keeps on disk: none this program can
/// read.
///
/// Assetto Corsa ships `content/cars/<car>/ui/ui_car.json` with the power,
/// torque and weight of every car, which is where the reference data comes
/// from. Competizione keeps its cars inside packed Unreal assets, so there is
/// nothing to scan — and an empty list is the honest answer, not a bug. The
/// screens already draw "no car data" rather than an empty table.
pub fn scan_cars(_configured: Option<&Path>) -> Vec<crate::games::CarSpecs> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create tmp");
        dir
    }

    #[test]
    fn a_configured_install_path_is_preferred() {
        let configured = scratch_dir("accpaths_configured_root");
        assert_eq!(acc_install_root(Some(&configured)), Some(configured));
    }

    #[test]
    fn a_configured_path_that_does_not_exist_falls_back_to_detection() {
        let missing = PathBuf::from("/nonexistent/Assetto Corsa Competizione");
        assert_ne!(acc_install_root(Some(&missing)), Some(missing));
    }

    #[test]
    fn an_empty_configured_path_is_ignored() {
        let empty = PathBuf::new();
        assert_ne!(acc_install_root(Some(&empty)), Some(empty));
    }

    #[test]
    fn a_configured_documents_dir_is_preferred() {
        let configured = scratch_dir("accpaths_configured_docs");
        assert_eq!(acc_documents_dir(Some(&configured)), Some(configured));
    }

    /// The two games are different numbers, and mixing them up points the
    /// bridge at a prefix the game is not publishing into.
    #[test]
    fn the_appid_is_competiziones_own() {
        assert_eq!(ACC_APP_ID_NUMBER, 805550);
        assert_ne!(
            ACC_APP_ID_NUMBER,
            crate::games::assetto_corsa::paths::AC_APP_ID_NUMBER
        );
    }
}
