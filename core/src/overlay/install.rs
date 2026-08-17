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

use crate::games::assetto_corsa::paths as ac_paths;
use std::io;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Folder name under `assettocorsa/apps/lua/`.
///
/// CSP finds an app's entry point by folder name, so the main script must be
/// `<APP_DIR>.lua`.
///
/// Deliberately unrelated to where the sources live. The panel sits in
/// `assets/frontends/csp-panel/` in this tree — it is the Assetto Corsa front
/// end, not "the overlay", and a second game's front end will sit beside it —
/// but what CSP loads has to be called this whatever the repository does.
pub const APP_DIR: &str = "ac_pro_engineer";

/// The app, as shipped. Embedded so it cannot drift from the struct layout
/// this build uses.
///
/// A tree, not a flat list: the panel is `ac_pro_engineer.lua` plus a dozen
/// modules under `acpe/`, and `install_into` creates the directories on the
/// way. Every `.lua` file in the source folder has to appear here or it simply
/// does not reach the game —
/// `every_lua_file_in_the_app_folder_is_shipped` fails when one is missed.
const FILES: &[(&str, &[u8])] = &[
    (
        "ac_pro_engineer.lua",
        include_bytes!("../../../assets/frontends/csp-panel/ac_pro_engineer.lua"),
    ),
    (
        "frame_layout.lua",
        include_bytes!("../../../assets/frontends/csp-panel/frame_layout.lua"),
    ),
    (
        "manifest.ini",
        include_bytes!("../../../assets/frontends/csp-panel/manifest.ini"),
    ),
    (
        "icon.png",
        include_bytes!("../../../assets/frontends/csp-panel/icon.png"),
    ),
    (
        "acpe/blocks.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/blocks.lua"),
    ),
    (
        "acpe/console.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/console.lua"),
    ),
    (
        "acpe/controls.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/controls.lua"),
    ),
    (
        "acpe/format.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/format.lua"),
    ),
    (
        "acpe/frame.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/frame.lua"),
    ),
    (
        "acpe/i18n.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/i18n.lua"),
    ),
    (
        "acpe/layout.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/layout.lua"),
    ),
    (
        "acpe/persist.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/persist.lua"),
    ),
    (
        "acpe/settings.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/settings.lua"),
    ),
    (
        "acpe/theme.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/theme.lua"),
    ),
    (
        "acpe/windows/changed.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/windows/changed.lua"),
    ),
    (
        "acpe/windows/debrief.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/windows/debrief.lua"),
    ),
    (
        "acpe/windows/dev.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/windows/dev.lua"),
    ),
    (
        "acpe/windows/engineer.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/windows/engineer.lua"),
    ),
    (
        "acpe/windows/main.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/windows/main.lua"),
    ),
    (
        "acpe/windows/settings.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/windows/settings.lua"),
    ),
    (
        "acpe/windows/status.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/windows/status.lua"),
    ),
    (
        "acpe/windows/telemetry.lua",
        include_bytes!("../../../assets/frontends/csp-panel/acpe/windows/telemetry.lua"),
    ),
];

/// How many files the panel is, for anything that offers to write or remove
/// them.
///
/// Said "four" in five places, and the panel became nineteen files the moment
/// it was split into modules. A number nobody has to remember is a number that
/// cannot be wrong.
pub fn file_count() -> usize {
    FILES.len()
}

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
    /// The *release* the installed panel came from, e.g. `0.3.3`.
    ///
    /// Most releases leave the frame alone, so [`Self::panel_version`] matching
    /// says nothing about how old the installed panel is. This does, and it is
    /// what a bug report needs: "the panel is from three releases ago" is a
    /// diagnosis, "the frame version matches" is not.
    pub panel_release: Option<String>,
}

/// The `EXPECTED_VERSION` an installed panel was written against.
fn installed_panel_version(app_path: &Path) -> Option<u32> {
    let source = std::fs::read_to_string(app_path.join("ac_pro_engineer.lua")).ok()?;
    let line = source
        .lines()
        .find(|line| line.trim_start().starts_with("local EXPECTED_VERSION"))?;
    line.split('=').nth(1)?.trim().parse().ok()
}

/// The `PANEL_VERSION` an installed panel announces.
///
/// Read from the script rather than from the manifest: the script is the file
/// the installer overwrites and the one CSP actually loads, so it cannot be the
/// stale half of a partial install.
fn installed_panel_release(app_path: &Path) -> Option<String> {
    let source = std::fs::read_to_string(app_path.join("ac_pro_engineer.lua")).ok()?;
    let line = source
        .lines()
        .find(|line| line.trim_start().starts_with("local PANEL_VERSION"))?;
    Some(
        line.split('=')
            .nth(1)?
            .trim()
            .trim_matches('\'')
            .to_string(),
    )
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
    let panel_release = app_path.as_deref().and_then(installed_panel_release);

    InstallReport {
        game_root,
        app_path,
        current,
        csp_present,
        panel_version,
        panel_release,
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

    // The panel is a tree now, so the folders it created have to go too —
    // deepest first, because a parent is only empty once its children are.
    // Only if empty, throughout: a folder with someone else's file in it is not
    // ours to delete, and neither is its parent.
    let mut directories: Vec<&str> = FILES
        .iter()
        .filter_map(|(name, _)| name.rsplit_once('/').map(|(dir, _)| dir))
        .collect();
    directories.sort_unstable();
    directories.dedup();
    directories.sort_by_key(|dir| std::cmp::Reverse(dir.matches('/').count()));
    for dir in directories {
        let _ = std::fs::remove_dir(target.join(dir));
    }

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

    /// The panel has to say which release it is, and say the right one.
    ///
    /// It is embedded in this binary, so the two ship together and the number
    /// is trivially knowable — but only if someone changes it. Nothing else
    /// fails when they forget: the frame version stays valid, the panel draws,
    /// and every bug report afterwards names a version that is not the one
    /// installed. The manifest sat at 1.0 for eleven releases that way.
    #[test]
    fn the_panel_announces_this_builds_version() {
        let source = embedded("ac_pro_engineer.lua");
        let declared = source
            .lines()
            .find(|line| line.trim_start().starts_with("local PANEL_VERSION"))
            .and_then(|line| line.split('=').nth(1))
            .map(|value| value.trim().trim_matches('\'').to_string())
            .expect("the panel declares a PANEL_VERSION");

        assert_eq!(
            declared,
            env!("CARGO_PKG_VERSION"),
            "PANEL_VERSION in assets/frontends/csp-panel/ac_pro_engineer.lua is stale; \
             set it to {}",
            env!("CARGO_PKG_VERSION")
        );
    }

    /// A module that is not embedded is a module that does not reach the game.
    ///
    /// The panel is a tree of a dozen files now, and `FILES` lists them one by
    /// one because `include_bytes!` takes a literal path. Adding a module and
    /// forgetting this list produces an install that is missing one `require`
    /// target — which fails at load, in the game, with every window drawing the
    /// error instead of the panel.
    #[test]
    fn every_lua_file_in_the_app_folder_is_shipped() {
        let root = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../assets/frontends/csp-panel"
        ));

        let mut found = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "lua") {
                    let relative = path
                        .strip_prefix(root)
                        .expect("under the app root")
                        .to_string_lossy()
                        .replace('\\', "/");
                    found.push(relative);
                }
            }
        }
        found.sort();
        assert!(!found.is_empty(), "the app folder has Lua in it");

        let shipped: Vec<&str> = FILES.iter().map(|(name, _)| *name).collect();
        for name in &found {
            assert!(
                shipped.contains(&name.as_str()),
                "{name} is in the app folder and not in FILES, so it would not \
                 be installed"
            );
        }
    }

    /// The panel has to read the frame this build writes.
    ///
    /// `frame_layout.lua` is generated and checked, and `PANEL_VERSION` is
    /// checked, but the one number that decides whether the panel draws at all
    /// was not: `EXPECTED_VERSION` is written by hand in the panel and compared
    /// against the frame's `version` field at runtime. Leaving it behind after
    /// a layout change gives a panel that loads, reads the right offsets, and
    /// refuses to draw anything but "Version mismatch" — which reads as the
    /// install being broken rather than as a number nobody bumped.
    #[test]
    fn the_panel_reads_the_frame_this_build_writes() {
        let source = embedded("ac_pro_engineer.lua");
        let declared = source
            .lines()
            .find(|line| line.trim_start().starts_with("local EXPECTED_VERSION"))
            .and_then(|line| line.split('=').nth(1))
            .and_then(|value| value.trim().parse::<u32>().ok())
            .expect("the panel declares an EXPECTED_VERSION");

        assert_eq!(
            declared,
            crate::overlay::frame::OVERLAY_VERSION,
            "EXPECTED_VERSION in assets/frontends/csp-panel/ac_pro_engineer.lua is stale; \
             set it to {}",
            crate::overlay::frame::OVERLAY_VERSION
        );
    }

    /// And it has to read every advice slot the frame carries.
    ///
    /// The panel names the message fields one by one, because CSP hands back
    /// raw cdata for an array of strings. A list one entry short is not an
    /// error anywhere: the panel simply stops at the second-to-last line, and
    /// the setting that asks for all of them quietly does nothing.
    #[test]
    fn the_panel_names_every_advice_slot() {
        // Across every embedded file, not just the entry point: MESSAGE_KEYS
        // lives in acpe/frame.lua, and a check pinned to one filename is a
        // check that stops working the next time something moves.
        let source: String = FILES
            .iter()
            .filter(|(name, _)| name.ends_with(".lua"))
            .filter_map(|(_, bytes)| std::str::from_utf8(bytes).ok())
            .collect();

        for slot in 0..crate::overlay::frame::MESSAGE_SLOTS {
            let field = format!("'message_{slot}'");
            assert!(
                source.contains(&field),
                "the panel's MESSAGE_KEYS is missing {field}"
            );
        }
    }

    /// CSP shows the manifest's VERSION in its apps list, which makes it the
    /// version a driver can read without opening a file.
    #[test]
    fn the_manifest_announces_this_builds_version() {
        let manifest = embedded("manifest.ini");
        let declared = manifest
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with(';'))
            .find_map(|line| line.strip_prefix("VERSION"))
            .map(|value| value.trim_start_matches([' ', '=']).trim().to_string())
            .expect("the manifest declares a VERSION");

        assert_eq!(
            declared,
            env!("CARGO_PKG_VERSION"),
            "VERSION in assets/frontends/csp-panel/manifest.ini is stale; set it to {}",
            env!("CARGO_PKG_VERSION")
        );
    }

    /// `README.txt` is the first file anyone opens in a downloaded archive,
    /// and it announces a version in its banner. It said v0.2.2 for thirteen
    /// releases — including in a build handed to someone to test — alongside
    /// install directions for a bundle layout that had stopped existing.
    ///
    /// Every other version in this project is pinned by a test; this is the
    /// one a user reads first and it was the one nothing checked.
    #[test]
    fn the_readme_announces_this_builds_version() {
        let readme = include_str!("../../../README.txt");
        let expected = format!("PRO ENGINEER v{}", env!("CARGO_PKG_VERSION"));
        assert!(
            readme.contains(&expected),
            "README.txt's banner is stale; it should read {expected}"
        );
    }

    /// The report is what the launcher card draws, so reading the release out
    /// of an installed panel has to work on a panel this installer wrote.
    #[test]
    fn an_installed_panel_reports_the_release_it_came_from() {
        let temp = std::env::temp_dir().join("acpe-release-report");
        let app = temp.join("apps").join("lua").join(APP_DIR);
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).expect("temp game folder");
        install_into(&app).expect("install");

        let report = describe(Some(&temp));
        assert_eq!(
            report.panel_release.as_deref(),
            Some(env!("CARGO_PKG_VERSION")),
            "a freshly installed panel reports this build's release"
        );
        assert_eq!(
            report.panel_version,
            Some(crate::overlay::frame::OVERLAY_VERSION)
        );

        let _ = std::fs::remove_dir_all(&temp);
    }

    /// The text of an embedded file, so a test reads what actually ships
    /// rather than what happens to be on disk beside it.
    fn embedded(name: &str) -> String {
        let bytes = FILES
            .iter()
            .find(|(file, _)| *file == name)
            .map(|(_, bytes)| *bytes);
        assert!(bytes.is_some(), "{name} is one of the embedded files");
        String::from_utf8(bytes.unwrap_or_default().to_vec()).expect("valid UTF-8")
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
             assets/frontends/csp-panel/frame_layout.lua`"
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

        use crate::engineer::{Recommendation, Severity};

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
        // A frame from a car on track, which is what this check is for. Without
        // CONNECTED the panel correctly draws "waiting for the car" instead of
        // the readouts, and the draw path this test exists to exercise is the
        // one that never runs.
        frame.flags = crate::overlay::frame::flags::CONNECTED
            | crate::overlay::frame::flags::SHOW_TELEMETRY
            | crate::overlay::frame::flags::SHOW_TIMING
            | crate::overlay::frame::flags::SHOW_FUEL
            | crate::overlay::frame::flags::SHOW_SESSION
            | crate::overlay::frame::flags::SHOW_ENGINEER;
        // A finished lap, so the debrief window has something to draw and the
        // new frame fields are checked across the language boundary rather than
        // only inside Rust. Without this the harness reads a real frame with no
        // laps in it and the debrief correctly draws "no finished laps yet" —
        // which is the one path this check is not for.
        frame.set_debrief(
            &[
                crate::overlay::frame::DebriefLap {
                    lap_number: 12,
                    lap_time_ms: 91_234,
                    sectors: [0; crate::overlay::frame::SECTORS],
                    advice: vec![Recommendation {
                        component: "Tyres".to_string(),
                        category: "Pressure".to_string(),
                        severity: Severity::Warning,
                        message: "Fronts over 28.4 psi (target 27.5)".to_string(),
                        action: String::new(),
                        parameters: Vec::new(),
                        confidence: 1.0,
                        chain: None,
                    }],
                },
                crate::overlay::frame::DebriefLap {
                    lap_number: 11,
                    lap_time_ms: 92_871,
                    sectors: [0; crate::overlay::frame::SECTORS],
                    advice: vec![Recommendation {
                        component: "Tyres".to_string(),
                        category: "Temperature".to_string(),
                        severity: Severity::Info,
                        message: "All four cold 62C".to_string(),
                        action: String::new(),
                        parameters: Vec::new(),
                        confidence: 1.0,
                        chain: None,
                    }],
                },
            ],
            crate::overlay::frame::DEBRIEF_LINES,
        );

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

    /// Run the panel under LÖVE as well as under LuaJIT.
    ///
    /// Not redundant: the two harnesses stub CSP differently, and the
    /// difference is the point. The LuaJIT stub answers every unknown name with
    /// something both callable and indexable, which is forgiving by design —
    /// and forgiving enough to hide `ui.Icons.Settings` succeeding against a
    /// value that is not a table. LÖVE's stub hands back a bare function, the
    /// index throws, and because that line sits at file scope the whole script
    /// fails to load and every window draws the error.
    ///
    /// That is the failure this catches and the other cannot, and it reached a
    /// tagged release before anyone looked at a screenshot.
    ///
    /// Skipped where love is not installed, so CI is unaffected.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn the_overlay_app_runs_under_love() {
        if std::process::Command::new("love")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("love not installed; skipping the LÖVE harness");
            return;
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let output = std::process::Command::new("love")
            .arg(root.join("apps/lua/love"))
            .arg("--test")
            .arg("--settings")
            .output()
            .expect("run the LÖVE harness");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "the overlay app failed under LÖVE:\n{stdout}\n{stderr}"
        );
        assert!(
            stdout.contains("0 errors"),
            "the harness reported errors:\n{stdout}\n{stderr}"
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
