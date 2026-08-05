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

/// What the application knows about the overlay's installation.
///
/// Gathered in one place so the launcher can show it without asking four
/// separate questions and drawing whatever the answers happen to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    /// The Assetto Corsa root, if one was found.
    pub game_root: Option<PathBuf>,
    /// Where the app goes inside it.
    pub app_path: Option<PathBuf>,
    /// Every embedded file is present and identical.
    pub current: bool,
    /// Custom Shaders Patch is installed. Without it the game has no Lua apps
    /// at all, and a perfectly installed overlay will never appear.
    pub csp_present: bool,
    /// The frame version the installed panel expects, if one is installed.
    ///
    /// Three pieces have to agree — this application, the bridge, and the panel
    /// — and a panel left over from an older install reads every field after
    /// the change at the wrong offset. Better to say so here than to let the
    /// driver work it out from nonsense on the windscreen.
    pub panel_version: Option<u32>,
}

/// The `EXPECTED_VERSION` an installed panel was written against.
fn installed_panel_version(app_path: &Path) -> Option<u32> {
    let source = std::fs::read_to_string(app_path.join("ac_pro_engineer.lua")).ok()?;
    let line = source
        .lines()
        .find(|line| line.trim_start().starts_with("local EXPECTED_VERSION"))?;
    line.split('=').nth(1)?.trim().parse().ok()
}

/// Look at the game folder and report what is there.
pub fn describe(configured_install: Option<&Path>) -> InstallReport {
    let game_root = ac_paths::ac_install_root(configured_install);
    let app_path = game_root
        .as_ref()
        .map(|root| root.join("apps").join("lua").join(APP_DIR));

    let current = app_path.as_ref().is_some_and(|path| {
        FILES.iter().all(|(name, contents)| {
            std::fs::read(path.join(name)).is_ok_and(|existing| existing == *contents)
        })
    });

    // The SDK is written by CSP itself, so its presence is the honest test —
    // an extension folder can survive an uninstall.
    let csp_present = game_root.as_ref().is_some_and(|root| {
        root.join("extension")
            .join("internal")
            .join("lua-sdk")
            .exists()
    });

    let panel_version = app_path.as_deref().and_then(installed_panel_version);

    InstallReport {
        game_root,
        app_path,
        current,
        csp_present,
        panel_version,
    }
}

/// Take the overlay back out of the game folder.
///
/// Only the files this installer wrote, and the folder itself if nothing else
/// ended up in it. Anything a driver put there stays, and so does everything
/// CSP keeps elsewhere — the panel's own settings live in CSP's storage, not
/// here, so installing again brings them back exactly as they were.
pub fn uninstall(configured_install: Option<&Path>) -> io::Result<usize> {
    let Some(target) = install_path(configured_install) else {
        return Ok(0);
    };

    let mut removed = 0;
    for (name, _) in FILES {
        let path = target.join(name);
        match std::fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    // Only if it is empty: a folder with someone else's file in it is not ours
    // to delete.
    let _ = std::fs::remove_dir(&target);

    Ok(removed)
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

    /// Run the overlay app under a real LuaJIT with the CSP API stubbed.
    ///
    /// The launcher's card is only as good as this: it tells people whether
    /// the panel is in the game folder, and being wrong about that sends them
    /// looking for the problem inside the game.
    #[test]
    fn uninstalling_removes_what_was_installed_and_nothing_else() {
        let temp = std::env::temp_dir().join("acpe-uninstall-test");
        let app = temp.join("apps").join("lua").join(APP_DIR);
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&app).expect("temp app folder");

        install_into(&app).expect("install");
        let theirs = app.join("driver-notes.txt");
        std::fs::write(&theirs, b"mine").expect("a file the installer did not write");

        let removed = uninstall(Some(&temp)).expect("uninstall");
        assert_eq!(removed, FILES.len(), "every installed file goes");
        assert!(
            theirs.exists(),
            "a file we did not write stays, and so does its folder"
        );
        assert!(!describe(Some(&temp)).current);

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn describing_a_folder_with_the_app_in_it_reports_it_current() {
        let temp = std::env::temp_dir().join("acpe-describe-test");
        let app = temp.join("apps").join("lua").join(APP_DIR);
        let _ = std::fs::remove_dir_all(&temp);
        // The override is only honoured if it exists — an absent one falls back
        // to whatever real install is on this machine, which would make the
        // test agree with itself for the wrong reason.
        std::fs::create_dir_all(&temp).expect("temp game folder");

        let report = describe(Some(&temp));
        assert!(
            !report.current,
            "an empty game folder cannot be up to date: {report:?}"
        );

        install_into(&app).expect("install into the temp folder");
        let report = describe(Some(&temp));
        assert!(
            report.current,
            "freshly written files are current: {report:?}"
        );
        assert_eq!(report.app_path.as_deref(), Some(app.as_path()));
        assert!(!report.csp_present, "no CSP was put in the temp folder");

        let _ = std::fs::remove_dir_all(&temp);
    }

    /// Syntax checks and the SDK conformance tests prove the calls exist;
    /// this proves the script actually runs — that no field is nil, no
    /// arithmetic lands on a string, and every draw path completes. The only
    /// alternative is launching the game.
    ///
    /// Skipped where luajit is not installed, so CI is unaffected. Linux only:
    /// the harness reads the published frame back through its backing file,
    /// and only the Linux writer has one.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn the_overlay_app_runs_under_luajit() {
        if std::process::Command::new("luajit")
            .arg("-v")
            .output()
            .is_err()
        {
            eprintln!("luajit not installed; skipping overlay runtime check");
            return;
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");

        // The harness reads a published frame; write one somewhere private so
        // it cannot collide with a running application.
        let frame_path = std::env::temp_dir().join("acpe-luajit-harness");
        let mut writer = crate::overlay::shared_writer::OverlayWriter::open_named(
            frame_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("acpe-luajit-harness"),
        )
        .expect("publish a frame for the harness");
        let mut frame = crate::overlay::frame::OverlayFrame::empty();
        frame.speed_kmh = 214.0;
        frame.gear = 4;
        frame.max_rpm = 8000;
        frame.rpm = 6000;
        writer.publish(&frame);

        let output = std::process::Command::new("luajit")
            .arg(root.join("apps/lua/tests/run_overlay.lua"))
            .env("ACPE_FRAME", writer.backing_path())
            .output()
            .expect("run the harness");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "the overlay app failed under luajit:\n{stdout}\n{stderr}"
        );
        assert!(
            stdout.contains("windowMain: OK"),
            "draw path ran:\n{stdout}"
        );
        assert!(
            stdout.contains("214"),
            "the published speed reached the draw path:\n{stdout}"
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
