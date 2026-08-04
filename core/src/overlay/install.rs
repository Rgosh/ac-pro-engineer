//! Installing the in-game Lua overlay into Assetto Corsa.
//!
//! The app files are embedded in this binary rather than shipped alongside it,
//! and that is the whole point. The Lua declares the shared struct field by
//! field; if its copy is older or newer than the `OverlayFrame` this build
//! writes, every read past the first difference is silently misaligned. Baking
//! them into the same artifact makes that impossible — the binary that writes
//! the struct is the binary that installs the declaration for reading it.
//!
//! Installation happens at startup, is idempotent, and rewrites the files
//! whenever they differ from what is embedded. So updating the application
//! updates the overlay, with no step for the user to forget.

use crate::ac_paths;
use std::io;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Folder name under `assettocorsa/apps/lua/`.
///
/// CSP finds an app's entry point by folder name, so the main script must be
/// `<APP_DIR>.lua`.
pub const APP_DIR: &str = "ac_pro_engineer";

/// The app, as shipped. Embedded so it cannot drift from the struct layout
/// this build uses.
const FILES: &[(&str, &[u8])] = &[
    (
        "ac_pro_engineer.lua",
        include_bytes!("../../../apps/lua/ac_pro_engineer/ac_pro_engineer.lua"),
    ),
    (
        "frame_layout.lua",
        include_bytes!("../../../apps/lua/ac_pro_engineer/frame_layout.lua"),
    ),
    (
        "manifest.ini",
        include_bytes!("../../../apps/lua/ac_pro_engineer/manifest.ini"),
    ),
    (
        "icon.png",
        include_bytes!("../../../apps/lua/ac_pro_engineer/icon.png"),
    ),
];

/// What an install attempt did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallOutcome {
    /// Files were written. Carries how many changed.
    Installed { updated: usize },
    /// Everything already matched.
    AlreadyCurrent,
    /// No Assetto Corsa installation was found to install into.
    NoGameFound,
}

/// Install or refresh the overlay app inside Assetto Corsa.
///
/// `configured_install` is the user's override from the config, if any.
/// Failure is reported but is never fatal: an unwritable game directory means
/// no overlay, not a broken application.
pub fn install(configured_install: Option<&Path>) -> io::Result<InstallOutcome> {
    let Some(root) = ac_paths::ac_install_root(configured_install) else {
        return Ok(InstallOutcome::NoGameFound);
    };

    let target = root.join("apps").join("lua").join(APP_DIR);
    install_into(&target)
}

/// Write the embedded app into `target`, creating it if needed.
///
/// Split out so tests can install somewhere harmless.
pub fn install_into(target: &Path) -> io::Result<InstallOutcome> {
    let mut updated = 0;

    for (name, contents) in FILES {
        let path = target.join(name);

        // Only write when the content differs. Rewriting identical files on
        // every launch would churn the game folder for nothing, and CSP
        // watches these for changes.
        if let Ok(existing) = std::fs::read(&path)
            && existing == *contents
        {
            continue;
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
        updated += 1;
    }

    if updated == 0 {
        Ok(InstallOutcome::AlreadyCurrent)
    } else {
        Ok(InstallOutcome::Installed { updated })
    }
}

/// Where the overlay app would be installed, for diagnostics.
pub fn install_path(configured_install: Option<&Path>) -> Option<PathBuf> {
    ac_paths::ac_install_root(configured_install)
        .map(|root| root.join("apps").join("lua").join(APP_DIR))
}

/// Install at startup, logging the result.
///
/// Never returns an error: this is a convenience the application offers, and
/// nothing about it should stop the app running.
pub fn install_on_startup(configured_install: Option<&Path>) {
    match install(configured_install) {
        Ok(InstallOutcome::Installed { updated }) => {
            info!("Installed the in-game overlay ({updated} file(s) written)");
        }
        Ok(InstallOutcome::AlreadyCurrent) => {
            info!("In-game overlay is up to date");
        }
        Ok(InstallOutcome::NoGameFound) => {
            info!("No Assetto Corsa installation found; in-game overlay not installed");
        }
        Err(error) => {
            warn!(error = ?error, "Could not install the in-game overlay");
        }
    }
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
    fn installing_writes_every_file() {
        let target = scratch_dir("acpe-install-fresh").join(APP_DIR);
        let outcome = install_into(&target).expect("install");

        assert_eq!(
            outcome,
            InstallOutcome::Installed {
                updated: FILES.len()
            }
        );
        for (name, _) in FILES {
            assert!(target.join(name).exists(), "{name} was written");
        }
    }

    /// Rewriting identical files on every launch would churn the game folder,
    /// and CSP watches them for changes.
    #[test]
    fn installing_twice_writes_nothing_the_second_time() {
        let target = scratch_dir("acpe-install-twice").join(APP_DIR);
        install_into(&target).expect("first install");

        let outcome = install_into(&target).expect("second install");
        assert_eq!(outcome, InstallOutcome::AlreadyCurrent);
    }

    /// The case that matters: an old copy of the app in the game folder must
    /// be replaced, or its struct declaration disagrees with this binary's.
    #[test]
    fn a_stale_file_is_replaced() {
        let target = scratch_dir("acpe-install-stale").join(APP_DIR);
        install_into(&target).expect("install");

        std::fs::write(target.join("frame_layout.lua"), b"-- from an older build\n")
            .expect("stale write");

        let outcome = install_into(&target).expect("reinstall");
        assert_eq!(outcome, InstallOutcome::Installed { updated: 1 });

        let restored = std::fs::read(target.join("frame_layout.lua")).expect("read back");
        assert!(
            restored.starts_with(b"-- GENERATED"),
            "the embedded copy wins over whatever was there"
        );
    }

    /// CSP locates an app's entry point by folder name, so this pairing is
    /// load-bearing rather than a convention.
    #[test]
    fn the_main_script_is_named_after_the_folder() {
        let expected = format!("{APP_DIR}.lua");
        assert!(
            FILES.iter().any(|(name, _)| *name == expected),
            "the app must ship {expected}"
        );
    }

    /// The embedded layout must be the one this build's struct produces —
    /// that is the entire reason for embedding rather than shipping loose.
    #[test]
    fn the_embedded_layout_matches_this_builds_struct() {
        let embedded = FILES
            .iter()
            .find(|(name, _)| *name == "frame_layout.lua")
            .map(|(_, bytes)| *bytes)
            .expect("frame_layout.lua is embedded");

        let text = std::str::from_utf8(embedded).expect("valid UTF-8");
        assert_eq!(
            text,
            crate::overlay::frame::lua_struct_declaration(),
            "the embedded Lua declaration is stale; regenerate it with \
             `cargo run -p ac_core --example gen_lua_layout > \
             apps/lua/ac_pro_engineer/frame_layout.lua`"
        );
    }

    #[test]
    fn nothing_is_installed_when_no_game_is_found() {
        // A path that cannot exist stands in for "no installation".
        let outcome = install(Some(Path::new("/nonexistent/assettocorsa")));
        // Either it found the real one on this machine, or it found nothing;
        // both are valid, but it must not have used the bogus path.
        match outcome.expect("install") {
            InstallOutcome::NoGameFound => {}
            other => {
                let path = install_path(Some(Path::new("/nonexistent/assettocorsa")));
                assert!(
                    path.is_some_and(|p| !p.starts_with("/nonexistent")),
                    "a configured path that does not exist must not be used: {other:?}"
                );
            }
        }
    }
}
