//! Custom Shaders Patch: whether it is there, which one, and putting it there.
//!
//! CSP is a third-party patch that most of what people do with this game in
//! 2020s now assumes — the weather scripts, the Lua apps, the in-game panel
//! this project's own overlay draws into. It is not part of the game and it is
//! not part of this project, and both of those matter to how it is treated
//! here.
//!
//! **It is two things in the game's folder and nothing else.** `dwrite.dll` at
//! the root, which Windows loads because the game asks for a system library of
//! that name, and `extension/`, which is everything the patch actually is.
//! That is the whole of its footprint, and it is what makes both installing
//! and removing it something a program can do honestly.
//!
//! **What is deliberately not here is downloading it.** Fetching somebody
//! else's binary and choosing which build of it a driver runs is a second
//! updater with its own ways of going wrong, and it is a decision about what
//! this project is rather than a feature. What is here is: say whether it is
//! installed and which version, put one in place from an archive somebody
//! already has, and take it out again.
//!
//! **The version is read, never inferred.** A number that came from a file is
//! worth saying; a number worked out from a folder name or a file size is a
//! plausible number nobody measured, which is this project's worst class of
//! bug.

use std::path::{Path, PathBuf};

/// The loader, at the game's root. Windows loads it because the game asks for
/// a system library of that name.
pub const LOADER: &str = "dwrite.dll";

/// Everything the patch is, beside the loader.
pub const FOLDER: &str = "extension";

/// Where the patch writes down what it is.
///
/// **Two places, because it has used both.** Nothing here guesses when neither
/// answers: it says it is installed and that it did not say which version,
/// which is true and useful, rather than inventing a number.
const MANIFESTS: &[&str] = &[
    "extension/config/data_manifest.ini",
    "extension/config/data_manifest_default.ini",
];

/// The keys a manifest states the version under.
const KEYS: &[&str] = &["SHADERS_PATCH", "SHADERS_PATCH_VERSION", "VERSION"];

/// What is installed, if anything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    /// The loader is in place.
    pub loader: bool,
    /// The patch's own folder is in place.
    pub folder: bool,
    /// What it says it is, when it says.
    pub version: Option<String>,
}

impl Installed {
    /// Whether the game will actually load it.
    ///
    /// **Both halves, and that is not pedantry.** Removing only the loader is
    /// how people turn CSP off without losing its configuration, so a folder
    /// on its own is a real and deliberate state — and reporting it as
    /// installed would tell somebody their patch is running when it is not.
    pub fn is_working(&self) -> bool {
        self.loader && self.folder
    }

    /// Whether anything of it is on disk at all.
    pub fn is_anywhere(&self) -> bool {
        self.loader || self.folder
    }

    /// What to say about it in one line.
    pub fn describe(&self) -> String {
        match (self.is_working(), &self.version) {
            (true, Some(version)) => version.clone(),
            (true, None) => "installed — it does not say which version".to_string(),
            (false, _) if self.folder => {
                format!("its files are there but {LOADER} is not, so the game does not load it")
            }
            (false, _) if self.loader => {
                format!(
                    "{LOADER} is there but {FOLDER} is not, which is a patch with nothing in it"
                )
            }
            _ => "not installed — the game has no Lua apps without it".to_string(),
        }
    }
}

/// Look at an install root and say what is there.
pub fn look(root: &Path) -> Installed {
    let folder = root.join(FOLDER);
    Installed {
        loader: root.join(LOADER).is_file(),
        folder: folder.is_dir(),
        version: version(root),
    }
}

/// What the patch says its version is, out of its own manifest.
fn version(root: &Path) -> Option<String> {
    for manifest in MANIFESTS {
        let at = manifest
            .split('/')
            .fold(root.to_path_buf(), |at, part| at.join(part));
        let Ok(text) = std::fs::read_to_string(&at) else {
            continue;
        };
        if let Some(said) = stated_in(&text) {
            return Some(said);
        }
    }
    None
}

/// The version out of a manifest's text.
///
/// **Only a value a key stated.** An ini this does not recognise gives
/// nothing, and nothing is the honest answer — "installed, and it did not say"
/// is information, and a made-up number is not.
pub(super) fn stated_in(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(key, value)| {
            let key = key.trim();
            KEYS.iter()
                .any(|wanted| key.eq_ignore_ascii_case(wanted))
                .then(|| value.trim().to_string())
        })
        .filter(|said| !said.is_empty() && said.len() < 64)
}

/// Take it out of the game.
///
/// **Exactly the two things it put there, and nothing under `content`.** This
/// deletes from the middle of somebody's game folder, so what it is allowed to
/// touch is written down here and the test holds it: the loader, and the
/// patch's own folder.
///
/// Returns what was removed.
pub fn remove(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut gone = Vec::new();

    let loader = root.join(LOADER);
    if loader.is_file() {
        std::fs::remove_file(&loader).map_err(|e| format!("{}: {e}", loader.display()))?;
        gone.push(loader);
    }

    let folder = root.join(FOLDER);
    if folder.is_dir() {
        std::fs::remove_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        gone.push(folder);
    }

    Ok(gone)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("acpe-csp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    /// A game with the patch in it, optionally saying which version.
    fn with_csp(root: &Path, version: Option<&str>) {
        std::fs::create_dir_all(root.join("content/cars/a_car")).expect("a fixture");
        std::fs::write(root.join(LOADER), b"MZ").expect("a fixture");
        std::fs::create_dir_all(root.join("extension/config")).expect("a fixture");
        std::fs::write(root.join("extension/lua/whatever.lua"), "").ok();
        if let Some(version) = version {
            std::fs::create_dir_all(root.join("extension/config")).expect("a fixture");
            std::fs::write(
                root.join("extension/config/data_manifest.ini"),
                format!("[VERSION]\nSHADERS_PATCH={version}\nSOMETHING_ELSE=9\n"),
            )
            .expect("a fixture");
        }
    }

    #[test]
    fn a_patched_game_says_which_version_it_has() {
        let root = scratch("version");
        with_csp(&root, Some("0.2.7-preview1"));

        let found = look(&root);
        assert!(found.is_working());
        assert_eq!(found.version.as_deref(), Some("0.2.7-preview1"));
        assert_eq!(found.describe(), "0.2.7-preview1");
    }

    /// **Installed and silent is a state, not a number to invent.** A manifest
    /// this does not recognise gives nothing, and saying so is information.
    #[test]
    fn a_patch_that_does_not_state_a_version_is_not_given_one() {
        let root = scratch("silent");
        with_csp(&root, None);

        let found = look(&root);
        assert!(found.is_working());
        assert_eq!(found.version, None);
        assert!(
            found.describe().contains("does not say"),
            "{}",
            found.describe()
        );
    }

    /// **Removing the loader is how people turn it off.** Reporting that as
    /// installed would tell somebody their patch is running when the game is
    /// not loading a line of it.
    #[test]
    fn the_files_without_the_loader_are_not_a_working_patch() {
        let root = scratch("no-loader");
        with_csp(&root, Some("0.2.0"));
        std::fs::remove_file(root.join(LOADER)).expect("turn it off");

        let found = look(&root);
        assert!(!found.is_working());
        assert!(found.is_anywhere());
        assert!(found.describe().contains(LOADER), "{}", found.describe());
    }

    /// A plain game is not a broken patch.
    #[test]
    fn a_game_without_it_says_so_plainly() {
        let root = scratch("plain");
        std::fs::create_dir_all(root.join("content/cars/a_car")).expect("a fixture");

        let found = look(&root);
        assert!(!found.is_working());
        assert!(!found.is_anywhere());
        assert!(found.describe().contains("not installed"));
    }

    /// **It deletes from the middle of somebody's game folder**, so what it is
    /// allowed to touch is exactly two things. A test rather than a comment,
    /// because the failure here is somebody's cars.
    #[test]
    fn removing_it_touches_the_patch_and_nothing_else() {
        let root = scratch("remove");
        with_csp(&root, Some("0.2.0"));
        std::fs::write(root.join("acs.exe"), b"MZ").expect("a fixture");

        let gone = remove(&root).expect("it should come out");
        assert_eq!(gone.len(), 2);
        assert!(!root.join(LOADER).exists());
        assert!(!root.join(FOLDER).exists());
        assert!(root.join("acs.exe").is_file(), "it removed the game");
        assert!(
            root.join("content/cars/a_car").is_dir(),
            "it removed the cars"
        );
        assert_eq!(look(&root), Installed::default());
    }

    /// Taking out what is not there is not an error.
    #[test]
    fn removing_nothing_is_not_a_failure() {
        let root = scratch("remove-nothing");
        std::fs::create_dir_all(root.join("content")).expect("a fixture");
        assert_eq!(
            remove(&root).expect("no patch is fine"),
            Vec::<PathBuf>::new()
        );
    }
}
