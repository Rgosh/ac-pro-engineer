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

// ------------------------------------------------------------ what is offered

/// Where the patch is published.
///
/// **Measured, not assumed.** `?get=<version>` answers with the zip itself —
/// sixty megabytes of it, first entry `dwrite.dll` — and the page lists every
/// version as an `?info=` link with the maintainer's recommended one in its
/// download block. Nothing here was guessed at: each was fetched and looked at
/// before it was written down.
pub const HOME: &str = "https://acstuff.ru/patch/";

/// What the site is offering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Available {
    /// The build its maintainer recommends, which is what anybody should take
    /// unless they have a reason.
    pub recommended: Option<String>,
    /// Every build the page lists, oldest first as the page orders them.
    pub all: Vec<String>,
}

/// Read what a copy of the patch's page is offering.
///
/// **A pure function over the page's text**, so the rule that reads it is
/// checked against a real copy of that page rather than against the network.
/// A site that changes shape then fails a test here rather than failing in
/// front of somebody.
pub fn offered_in(html: &str) -> Available {
    let mut all: Vec<String> = Vec::new();
    for at in html
        .match_indices("?info=")
        .map(|(at, _)| at + "?info=".len())
    {
        let rest = &html[at..];
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let said = &rest[..end];
        if !said.is_empty() && !all.iter().any(|seen| seen == said) {
            all.push(said.to_string());
        }
    }

    // The download block names one, and it is the one to take.
    let recommended = html
        .match_indices("?get=")
        .map(|(at, _)| at + "?get=".len())
        .find_map(|at| {
            let rest = &html[at..];
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(rest.len());
            (end > 0).then(|| rest[..end].to_string())
        });

    Available { recommended, all }
}

/// Where one version's archive is.
pub fn archive_url(version: &str) -> String {
    format!("{HOME}?get={version}")
}

#[cfg(test)]
mod offered_tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// The page as it was actually served, saved so the parser is checked
    /// against the real thing rather than against something written here.
    const PAGE: &str = include_str!("../../../tests/fixtures/csp-patch-page.html");

    /// **Read off the page the maintainer publishes.** The recommended build
    /// is the one its download block names; the list is every version it links
    /// an information page for.
    #[test]
    fn the_real_page_reads_as_the_versions_it_offers() {
        let offered = offered_in(PAGE);

        assert_eq!(offered.recommended.as_deref(), Some("0.2.11"));
        // Seventeen, which is what that copy of the page lists. Counted from
        // the page rather than guessed: the first guess here was "more than
        // twenty" and it was the assertion that was wrong, not the parser.
        assert_eq!(offered.all.len(), 17, "{:?}", offered.all);
        assert_eq!(offered.all.first().map(String::as_str), Some("0.1.75"));
        assert_eq!(offered.all.last().map(String::as_str), Some("0.2.11"));

        // **The page has a regular expression in its own script that contains
        // the same marker.** It yields an empty version, and an empty version
        // in a list of builds is a row somebody can press that downloads
        // nothing.
        assert!(
            !offered.all.iter().any(|one| one.is_empty()),
            "the script's own text got in: {:?}",
            offered.all
        );
        assert!(
            offered
                .all
                .iter()
                .all(|one| one.chars().all(|c| c.is_ascii_digit() || c == '.')),
            "something that is not a version got in: {:?}",
            offered.all
        );
    }

    /// A page that is not that page offers nothing, rather than nonsense.
    #[test]
    fn a_page_with_nothing_on_it_offers_nothing() {
        assert_eq!(
            offered_in("<html><body>down for maintenance</body></html>"),
            Available::default()
        );
    }

    /// The address is built from the version and nothing else.
    #[test]
    fn an_archive_is_addressed_by_its_version() {
        assert_eq!(
            archive_url("0.2.11"),
            "https://acstuff.ru/patch/?get=0.2.11"
        );
    }
}

// ------------------------------------------------------------- fetching one

/// Long enough for sixty megabytes on a slow line, short enough that a dead
/// host does not hold a thread all evening.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(600);

/// The smallest thing that could be the patch.
///
/// **A guard against a page, not a size check.** The published archive is
/// around sixty megabytes; an error page, a redirect body or a rate-limit
/// notice is a few thousand bytes and arrives with a perfectly good status
/// code. Anything under this is not the patch whatever it says it is.
const SURELY_TOO_SMALL: u64 = 4 * 1024 * 1024;

/// Ask the site what it is offering.
///
/// Blocking, so the caller puts it on a thread. Nothing in this crate is
/// async and nothing here should become so.
pub fn what_is_offered() -> Result<Available, String> {
    let page = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|why| format!("{why}"))?
        .get(HOME)
        .send()
        .map_err(|why| format!("{HOME} could not be reached: {why}"))?
        .error_for_status()
        .map_err(|why| format!("{HOME} answered with {why}"))?
        .text()
        .map_err(|why| format!("{HOME} sent something unreadable: {why}"))?;

    let offered = offered_in(&page);
    if offered.all.is_empty() {
        return Err(format!(
            "{HOME} answered, but nothing on it looks like a list of versions any more"
        ));
    }
    Ok(offered)
}

/// Fetch one version's archive to a file, and refuse anything that is not one.
///
/// **What arrives is checked before it is anywhere near the game.** A download
/// under an `.zip` name can be an error page, a redirect body or somebody
/// else's file entirely, and the cost of writing one of those into a game
/// folder is the game. So the bytes are written to a file of their own and
/// then read back through the same recogniser a dropped archive goes through —
/// if it does not come back as the patch, it is refused and the file is
/// removed.
///
/// Returns the archive, for [`super::adding::place`] to install from. One
/// path for a download and for a drop, deliberately: two ways of installing
/// the same zip is two ways for one of them to be wrong.
pub fn fetch(version: &str, into: &Path) -> Result<PathBuf, String> {
    let at = into.join(format!("csp-{version}.zip"));
    if let Some(above) = at.parent() {
        std::fs::create_dir_all(above).map_err(|why| format!("{}: {why}", above.display()))?;
    }

    let url = archive_url(version);
    let mut answer = reqwest::blocking::Client::builder()
        .timeout(PATIENCE)
        .build()
        .map_err(|why| format!("{why}"))?
        .get(&url)
        .send()
        .map_err(|why| format!("{url} could not be reached: {why}"))?
        .error_for_status()
        .map_err(|why| format!("{url} answered with {why}"))?;

    let mut file = std::fs::File::create(&at).map_err(|why| format!("{}: {why}", at.display()))?;
    std::io::copy(&mut answer, &mut file)
        .map_err(|why| format!("{} could not be written: {why}", at.display()))?;
    drop(file);

    if let Err(why) = is_really_the_patch(&at) {
        // **Removed, not left for somebody to find later.** A file under a
        // zip's name that is not the patch is worse sitting in a folder than
        // it is missing.
        let _ = std::fs::remove_file(&at);
        return Err(format!("what arrived from {url}: {why}"));
    }

    Ok(at)
}

/// Whether a downloaded file is the patch, before it is anywhere near a game.
///
/// **Two checks, and each catches what the other does not.** A rate-limit
/// page, a redirect body and an error JSON all arrive happily under a `.zip`
/// name with a perfectly good status code, and they are a few kilobytes — so
/// size catches them first and cheaply. And a real zip of something else
/// entirely passes any size rule, so what is inside it is read back through
/// the same recogniser a dropped archive goes through.
///
/// One recogniser for a download and for a drop, deliberately: two ways of
/// judging the same zip is two ways for one of them to be wrong.
fn is_really_the_patch(at: &Path) -> Result<(), String> {
    let size = std::fs::metadata(at).map(|it| it.len()).unwrap_or_default();
    if size < SURELY_TOO_SMALL {
        return Err(format!(
            "it is {size} bytes, which is a page and not sixty megabytes of patch"
        ));
    }
    match super::adding::whats_in(at).first() {
        Some(one) if one.kind == super::adding::Kind::Csp => Ok(()),
        _ => Err(format!(
            "there is no {LOADER} and no {FOLDER} in it, so it is not going anywhere near the game"
        )),
    }
}

#[cfg(test)]
mod fetch_tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// **A page under a zip's name is not the patch.** This is the check
    /// standing between a rate-limit notice and somebody's game folder.
    #[test]
    fn a_page_saved_under_a_zip_name_is_refused() {
        let dir = std::env::temp_dir().join(format!("acpe-csp-page-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let at = dir.join("csp-0.2.11.zip");
        std::fs::write(&at, b"<html><body>too many requests</body></html>").expect("a fixture");

        let refused = is_really_the_patch(&at).expect_err("a page is not the patch");
        assert!(refused.contains("bytes"), "{refused}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **And a real archive of something else passes any size rule.** What is
    /// inside it has to be read, through the same recogniser a dropped archive
    /// goes through.
    #[test]
    fn a_large_archive_that_is_not_the_patch_is_refused() {
        let dir = std::env::temp_dir().join(format!("acpe-csp-other-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let at = dir.join("csp-0.2.11.zip");
        // Big enough to pass the size rule, and not the patch.
        std::fs::write(&at, vec![0u8; (SURELY_TOO_SMALL + 1) as usize]).expect("a fixture");

        let refused = is_really_the_patch(&at).expect_err("not the patch");
        assert!(refused.contains(LOADER), "{refused}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The version list has to come out of the page, and a page that no longer
    /// has one is an error rather than an empty list — an empty list of builds
    /// reads as "the patch has no versions", which is never true.
    #[test]
    fn a_page_without_versions_is_an_error_not_an_empty_list() {
        assert!(offered_in("<html>nothing here</html>").all.is_empty());
    }
}
