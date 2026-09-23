//! Knowing which `shm-bridge.exe` is in play, and whether it is old enough to
//! break the overlay.
//!
//! On Linux the application publishes [`OverlayFrame`] into a plain file under
//! `/dev/shm`, and `shm-bridge.exe` — a Windows binary running inside the Proton
//! prefix — wraps that file in the Win32 named mapping CSP can open. Three
//! pieces have to agree about the frame: this application, the panel, and the
//! bridge. The first two are checked already; this is the third, and it was the
//! one that could not be checked at all.
//!
//! The symptom of a bridge built before the frame grew is not an error. CSP
//! refuses to open a mapping smaller than the struct the panel declares, so the
//! panel says "waiting for Pro Engineer" while `/dev/shm` holds the file, at
//! the right size, with the application running. Two evenings went to that.
//!
//! Two ways to ask, because neither works on its own:
//!
//! * a **running** bridge writes [`BRIDGE_INFO_FILE`] next to the mappings, so
//!   [`status`] can name the version currently serving the game;
//! * a bridge sitting on disk and **not** running is read with
//!   [`version_in_executable`], which scans the file for a marker the bridge
//!   compiles into itself. The application cannot run a Windows binary to ask
//!   it, and this is the only answer that does not require Wine.
//!
//! None of this exists on Windows: there the application creates the named
//! mapping itself and no bridge is involved. [`status`] says so rather than
//! reporting a missing component.

use crate::overlay::frame::{OVERLAY_MMF_NAME, OverlayFrame};
use std::path::{Path, PathBuf};

/// Shape of [`BRIDGE_INFO_FILE`]'s contents.
///
/// **Taken from the bridge rather than restated here.** These four constants
/// used to be written down twice — once in this file and once in the bridge's
/// own source, with a comment on each saying "must match". They agreed
/// because somebody remembered. The bridge is its own crate now, so the
/// agreement is a `use` and cannot rot.
pub const BRIDGE_PROTOCOL: u32 = wineshm::announce::FORMAT;

/// What a running bridge calls itself, in `/dev/shm`.
pub const BRIDGE_INFO_FILE: &str = wineshm::announce::FILE;

/// Filename of the bridge as it is built and shipped.
pub const BRIDGE_EXE: &str = "wineshm.exe";

/// The bridge version this build was compiled against.
///
/// **It is no longer this application's version, and that is the change.**
/// The bridge used to be a crate in this workspace, so the two numbers moved
/// together and "is the bridge current" was "does it say what I say". It is
/// its own project now, on its own release cycle, and a driver whose bridge
/// reads 0.1.0 beside an application reading 0.5.0 has a matched pair.
///
/// Taken from the library this links, which is built from the same tag as the
/// `.exe` it is asking about.
pub const BRIDGE_VERSION: &str = wineshm::VERSION;

/// The prefix the bridge compiles into its own binary, ahead of its version.
pub const VERSION_MARKER_PREFIX: &str = wineshm::VERSION_MARKER_PREFIX;

/// Every block this program needs out of the prefix, and how big each is.
///
/// Four that Assetto Corsa and Competizione publish, and one that runs the
/// other way — the application writes it and the in-game panel reads it, which
/// is the same mechanism with the arrow reversed and so costs one more entry
/// rather than a second bridge.
///
/// **The one list.** It is what the bridge is started with, what the note is
/// checked against, and what a diagnostic quotes.
pub fn pages() -> Vec<wineshm::Page> {
    let mut pages = wineshm::page::preset("assetto-corsa").unwrap_or_default();
    pages.push(wineshm::Page {
        name: super::frame::OVERLAY_MMF_NAME.to_string(),
        bytes: size_of::<super::frame::OverlayFrame>(),
    });
    pages
}

/// Where the mappings live on Linux. Wine sees it as `Z:\dev\shm\…`.
const SHM_DIR: &str = "/dev/shm";

/// What a running bridge says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeInfo {
    /// [`BRIDGE_PROTOCOL`] as the bridge understands it.
    pub protocol: u32,
    /// The bridge's crate version, e.g. `0.3.3`.
    pub version: String,
    /// How many bytes it sized the overlay mapping to. The number that
    /// actually decides whether CSP will open it.
    pub frame_bytes: usize,
    /// The mapping name it created for the overlay.
    pub mmf: String,
    /// The bridge's process id — a *Wine* pid, so it is a diagnostic to quote
    /// back at the user and never a liveness check.
    pub pid: u32,
}

impl BridgeInfo {
    /// Read what a running bridge published about itself.
    ///
    /// The bridge writes a note listing every block it is serving, its own
    /// version and its pid. What this program needs from it is narrower — the
    /// overlay mapping's name and size, because that is the number that
    /// decides whether CSP will open it — so the note is read by the crate
    /// that defines it and the interesting part is picked out here.
    ///
    /// `None` when the file is not a note, or is one from a format this build
    /// does not understand, or does not carry the overlay block at all: all
    /// three mean "a bridge that cannot serve this application", which is what
    /// the caller is asking about.
    pub fn parse(text: &str) -> Option<Self> {
        let note = wineshm::Note::parse(text)?;
        let (page, _) = note
            .pages
            .iter()
            .find(|(page, _)| page.name == super::frame::OVERLAY_MMF_NAME)?;
        Some(Self {
            protocol: note.format,
            version: note.version,
            frame_bytes: page.bytes,
            mmf: page.name.clone(),
            pid: note.pid,
        })
    }
}

/// Why a bridge cannot serve this application's frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Complaint {
    /// The mapping is not the size of the struct the panel declares, so CSP
    /// will refuse to open it. This is the one that presents as silence.
    FrameBytes { found: usize, expected: usize },
    /// The note itself is in a shape this application cannot read.
    Protocol { found: u32, expected: u32 },
    /// The bridge is mapping some other block.
    MappingName { found: String, expected: String },
}

impl Complaint {
    /// One sentence, for a card that has one line to spend.
    pub fn describe(&self) -> String {
        match self {
            Self::FrameBytes { found, expected } => format!(
                "maps {found} bytes, this build's frame is {expected} — CSP will not open it"
            ),
            Self::Protocol { found, expected } => {
                format!("speaks bridge protocol {found}, this build expects {expected}")
            }
            Self::MappingName { found, expected } => {
                format!("maps {found}, this build publishes {expected}")
            }
        }
    }
}

/// What the application knows about the bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeStatus {
    /// Windows: the application makes the named mapping itself, and there is no
    /// bridge to be out of date.
    NotRequired,
    /// No bridge has announced itself. Either it was never started, or it was
    /// and has exited.
    NotRunning,
    /// No announcement, but AC's own pages are mapped — so *something* made
    /// them, and a bridge too old to announce itself is the likely answer.
    ///
    /// This is the state every release up to and including v0.3.3 leaves a
    /// Linux driver in, and it is worth its own case because the remedy is the
    /// opposite of [`Self::NotRunning`]'s. That one says "start the bridge";
    /// this one has a bridge running and still no overlay, because a bridge
    /// built before the overlay existed maps AC's four pages and nothing else.
    /// Told to start it, the driver starts the same one again.
    Unannounced,
    /// A note is there but cannot be read.
    Unreadable(String),
    /// A note is there, and nothing has touched it for long enough that the
    /// bridge behind it is gone.
    ///
    /// **The state that used to read as health.** A bridge killed rather than
    /// closed leaves its note and its pages exactly as they were, so this used
    /// to come back [`Self::Current`] — and the pages beside it hold a session
    /// that ended whenever the process died. Every number in them is real and
    /// none of it is now, which is this project's worst class of fault: a
    /// Huracán at Spa reported as a Ferrari at Monza with the speed correct.
    ///
    /// The bridge touches its note every couple of seconds now, so its age is
    /// a pulse — see `wineshm::liveness`.
    Abandoned(Box<BridgeInfo>),
    /// Running, and it cannot serve this build's frames.
    Incompatible {
        info: Box<BridgeInfo>,
        complaint: Complaint,
    },
    /// Running and compatible, but built from a different release than this
    /// application. Nothing is broken; it is worth saying before something is.
    Behind {
        info: Box<BridgeInfo>,
        expected_version: String,
    },
    /// Running, compatible, same release.
    Current(Box<BridgeInfo>),
}

impl BridgeStatus {
    /// Whether the overlay can work as things stand.
    ///
    /// [`Self::Behind`] is fine: a bridge from another release that maps the
    /// right number of bytes under the right name serves frames correctly.
    pub fn is_workable(&self) -> bool {
        matches!(
            self,
            Self::NotRequired | Self::Current(_) | Self::Behind { .. }
        )
    }

    /// The version currently serving the game, if one is.
    pub fn running_version(&self) -> Option<&str> {
        match self {
            Self::Current(info) | Self::Behind { info, .. } | Self::Incompatible { info, .. } => {
                Some(info.version.as_str())
            }
            _ => None,
        }
    }
}

/// Everything needed to start the bridge inside a game's Proton prefix.
///
/// The pieces rather than a `Command`, because the two front ends build
/// different ones — one of them `tokio`'s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// What to run.
    pub program: PathBuf,
    /// Its arguments, in order.
    pub args: Vec<String>,
    /// Environment to set for it.
    pub env: Vec<(String, String)>,
    /// What to make the working directory, when it matters.
    pub working_dir: Option<PathBuf>,
    /// How this was decided, for the log line and for the settings card.
    pub how: How,
}

/// Which of the three ways of reaching a Proton prefix this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// `AC_PROTON_PATH`, which the driver set themselves.
    Chosen,
    /// Steam's own Proton, found in the prefix it built.
    Steam,
    /// `protontricks-launch`, and hoping it is installed.
    Protontricks,
}

/// **Steam's own Proton first, and protontricks only if it cannot be found.**
///
/// The bridge has to run as a Windows process inside the game's prefix, and
/// for four releases the only way this offered was `protontricks-launch`.
/// That is a dependency, and on an immutable distribution — Bazzite,
/// Silverblue, the Steam Deck — it is one that usually arrives as a Flatpak.
/// A Flatpak has a `/dev/shm` of its own, so the bridge inside one creates
/// its pages in a tmpfs that exists only in that sandbox: it reports success,
/// and nothing outside can see a byte of it. Reported from Bazzite as
/// "running shm-bridge.exe does nothing", by somebody who then could not
/// install protontricks any other way without rebuilding their OS image.
///
/// Nothing needs to be installed. Steam already ships the Proton the game is
/// set to use and writes which one into the prefix it built, so the bridge
/// can be started with that directly — see [`crate::steam::proton_wine`].
///
/// `AC_PROTON_PATH` still wins where it is set: it is documented, somebody is
/// relying on it, and it takes the protontricks command line.
#[cfg(unix)]
pub fn how_to_start(app_id: u32, exe: &Path) -> Invocation {
    let chosen = std::env::var("AC_PROTON_PATH")
        .ok()
        .filter(|program| !program.is_empty());
    if let Some(program) = chosen {
        return dressed(
            wineshm::launch::Launch::Protontricks { app_id },
            PathBuf::from(program),
            exe,
            How::Chosen,
        );
    }

    // **Which Proton, and where, is the bridge crate's question now.** It was
    // answered here as well until this release, and the two answers were the
    // same only because they were written on the same afternoon. See
    // `wineshm::launch` for the layouts it covers — native, Flatpak, Snap, the
    // Deck, and libraries on other disks.
    let plan = wineshm::launch::how_to_launch(app_id);
    let how = match plan {
        wineshm::launch::Launch::Proton { .. } => How::Steam,
        wineshm::launch::Launch::Protontricks { .. } => How::Protontricks,
    };
    let (program, _) = plan.command(exe);
    dressed(plan, program, exe, how)
}

/// Turn the bridge crate's plan into this program's [`Invocation`], with the
/// blocks it wants added to the command line.
#[cfg(unix)]
fn dressed(plan: wineshm::launch::Launch, program: PathBuf, exe: &Path, how: How) -> Invocation {
    let (_, mut args) = plan.command(exe);
    for page in pages() {
        args.push("--page".to_string());
        args.push(format!("{}:{}", page.name, page.bytes));
    }
    // **The block that proves the game is still there.** Assetto Corsa
    // rewrites its physics page three hundred times a second while a session
    // is live and stops entirely when it exits — and the pages outlive it,
    // because the bridge is holding the sections. Without this the file goes
    // on holding the last frame: a car at some speed on some circuit, real
    // numbers from a session that ended, and the first thing read after the
    // game is started again. That is the Huracán at Spa, at its source.
    //
    // Named rather than guessed: `acpmf_static` is written once a session and
    // is constant afterwards, so "blank what has not changed" would wipe the
    // car and the track of a session that is still running.
    // Which block that is, is the bridge crate's knowledge — it ships the
    // preset these pages come from, so it knows which of them moves. Naming
    // it here as well would be the same fact in two places, and the wrong
    // answer is not an error anybody sees: pick the static block, which
    // carries the car and the track, and a live session gets blanked.
    if let Some(beat) = wineshm::page::preset_heartbeat("assetto-corsa") {
        args.push("--heartbeat".to_string());
        args.push(beat.to_string());
    }

    // Nothing to say on a terminal nobody is watching: the launcher card and
    // the diagnostics read the note instead.
    args.push("--quiet".to_string());

    Invocation {
        program,
        args,
        env: plan.env(),
        // **The folder the bridge is in, and it matters.** Wine resolves the
        // path against the prefix's drive mappings, and a folder that is not
        // mapped — `/tmp` is the one that bit during testing — comes back as
        // "failed to open" with the file plainly there.
        working_dir: wineshm::launch::Launch::working_dir(exe),
        how,
    }
}

/// Where a running bridge would have left its note.
pub fn info_path() -> PathBuf {
    Path::new(SHM_DIR).join(BRIDGE_INFO_FILE)
}

/// One of AC's own pages, which every bridge ever built maps.
///
/// Its presence without an announcement is what separates "no bridge" from "a
/// bridge older than the announcement".
#[cfg(not(target_os = "windows"))]
const AC_PAGE: &str = "acpmf_physics";

/// Has *something* mapped AC's pages?
///
/// Deliberately a hint and not a verdict. The simulator writes these too, and a
/// bridge killed outright leaves them behind, so this only chooses which of two
/// sentences to show — never whether the overlay works.
#[cfg(not(target_os = "windows"))]
fn ac_pages_present() -> bool {
    Path::new(SHM_DIR).join(AC_PAGE).exists()
}

/// Judge a note against what this build needs.
///
/// Split from [`status`] so it can be tested without a bridge, a Wine prefix or
/// a `/dev/shm` to write into.
pub fn judge(info: BridgeInfo, expected_version: &str) -> BridgeStatus {
    // Size first. It is the only mismatch that presents as nothing happening
    // at all, so it is the one worth naming before the others.
    let expected_bytes = size_of::<OverlayFrame>();
    if info.frame_bytes < expected_bytes {
        let complaint = Complaint::FrameBytes {
            found: info.frame_bytes,
            expected: expected_bytes,
        };
        return BridgeStatus::Incompatible {
            info: Box::new(info),
            complaint,
        };
    }

    if info.protocol != BRIDGE_PROTOCOL {
        let complaint = Complaint::Protocol {
            found: info.protocol,
            expected: BRIDGE_PROTOCOL,
        };
        return BridgeStatus::Incompatible {
            info: Box::new(info),
            complaint,
        };
    }

    if info.mmf != OVERLAY_MMF_NAME {
        let complaint = Complaint::MappingName {
            found: info.mmf.clone(),
            expected: OVERLAY_MMF_NAME.to_string(),
        };
        return BridgeStatus::Incompatible {
            info: Box::new(info),
            complaint,
        };
    }

    if info.version != expected_version {
        return BridgeStatus::Behind {
            expected_version: expected_version.to_string(),
            info: Box::new(info),
        };
    }

    BridgeStatus::Current(Box::new(info))
}

/// Ask the bridge who it is.
///
/// The note is removed on a clean exit, so its absence means "not running".
/// A bridge killed outright leaves it behind and this reports a bridge that is
/// no longer there — which is why the panel's liveness comes from the frame's
/// sequence and never from here.
#[cfg(not(target_os = "windows"))]
pub fn status(expected_version: &str) -> BridgeStatus {
    let path = info_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // No note. Whether that means "no bridge" or "a bridge from before
            // notes existed" decides which remedy to offer, and offering the
            // wrong one sends the driver to start the bridge that is already
            // running and already cannot serve the overlay.
            return if ac_pages_present() {
                BridgeStatus::Unannounced
            } else {
                BridgeStatus::NotRunning
            };
        }
        Err(error) => return BridgeStatus::Unreadable(error.to_string()),
    };

    match BridgeInfo::parse(&text) {
        Some(info) => {
            // Before anything else about it: is the bridge that wrote this
            // still there? A note outlives the process that made it, and the
            // pages beside it outlive the session.
            let store = wineshm::Store::at(SHM_DIR);
            if wineshm::reader::pulse(&store).is_some_and(|pulse| !pulse.is_worth_reading()) {
                return BridgeStatus::Abandoned(Box::new(info));
            }
            judge(info, expected_version)
        }
        None => BridgeStatus::Unreadable(format!(
            "{} is missing a key this build needs",
            path.display()
        )),
    }
}

/// On Windows the application creates the named mapping itself.
#[cfg(target_os = "windows")]
pub fn status(_expected_version: &str) -> BridgeStatus {
    BridgeStatus::NotRequired
}

/// The version compiled into a `shm-bridge.exe` on disk.
///
/// Scanned out of the file rather than asked for: this is a Windows binary and
/// the application asking may well be a Linux process, so there is no running
/// it to find out. Returns `None` for a bridge built before the marker existed,
/// which is itself the answer — it is older than this check.
pub fn version_in_executable(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    version_in_bytes(&bytes)
}

/// [`version_in_executable`] against bytes already in hand.
pub fn version_in_bytes(bytes: &[u8]) -> Option<String> {
    // The marker is the bridge's, so reading it is the bridge crate's job.
    // This used to be a second implementation of the same scan, which is one
    // more place for the terminator or the cap to be got subtly wrong.
    wineshm::version_in_binary(bytes)
}

/// Every place a bridge might be, in the order they are preferred
/// when none of them is the right version.
///
/// Beside the application first, because that is where the release bundle puts
/// it and where the README tells people to keep it; then the working directory,
/// for a run straight out of a checkout.
fn candidate_executables() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(BRIDGE_EXE));
        // The bundle ships the Linux binary and the bridge in one folder, but
        // a checkout has the bridge under its build target.
        candidates.push(dir.join("Linux").join(BRIDGE_EXE));
    }

    // Relative to the *executable*, not just to the working directory.
    //
    // Running a checkout means `target/release/ac_pro_engineer`, and the
    // cross-compiled bridge is its sibling at
    // `target/x86_64-pc-windows-gnu/release/shm-bridge.exe`. Searching only
    // `cwd/target/...` finds that when the shell happens to be at the root of
    // the repository and finds nothing at all when it is not — which is what
    // "I ran it out of the target folder and it does not see the bridge" is.
    // Where the binary is does not depend on where you were standing when you
    // started it.
    if let Ok(exe) = std::env::current_exe() {
        candidates.extend(cross_build_candidates(&exe));
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(BRIDGE_EXE));
        for profile in ["release", "debug"] {
            candidates.push(
                cwd.join("target")
                    .join("x86_64-pc-windows-gnu")
                    .join(profile)
                    .join(BRIDGE_EXE),
            );
        }
    }

    candidates.retain(|path| path.is_file());
    // The same file can be reached by more than one of the routes above, and a
    // duplicate would be probed and reported twice.
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|path| {
        let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        seen.insert(key)
    });
    candidates
}

/// Where a cross-compiled bridge sits relative to a binary in a checkout.
///
/// Pure, and separate from the search above, so it can be checked against made
/// up paths rather than against whatever this machine happens to have.
fn cross_build_candidates(exe: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for ancestor in exe.ancestors().take(4) {
        for profile in ["release", "debug"] {
            out.push(
                ancestor
                    .join("x86_64-pc-windows-gnu")
                    .join(profile)
                    .join(BRIDGE_EXE),
            );
            out.push(
                ancestor
                    .join("target")
                    .join("x86_64-pc-windows-gnu")
                    .join(profile)
                    .join(BRIDGE_EXE),
            );
        }
    }
    out
}

/// Find the `shm-bridge.exe` this installation would run.
///
/// **A bridge carrying this build's version wins, wherever it is.** The order
/// above decides only between copies that are all the wrong version.
///
/// Without that rule a checkout is very hard to test in: the working directory
/// is searched before the build target, so one stale `shm-bridge.exe` left at
/// the root of the repository shadows the one you just cross-compiled, and the
/// application spawns it, reports it as out of date, and offers to download a
/// third. Deleting the stale copy is not obvious, because nothing on screen
/// says which of the three files it is talking about.
///
/// Matching on the version rather than on the path also does the right thing
/// for a user with an old bridge next to the application and a current one
/// somewhere else, which is the same situation with different directories.
pub fn installed_executable() -> Option<PathBuf> {
    choose_executable(&candidate_executables(), crate::updater::CURRENT_VERSION)
}

/// The rule above, with the search and the version handed in so it can be
/// tested against real files rather than against whatever is on this machine.
fn choose_executable(candidates: &[PathBuf], wanted: &str) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| version_in_executable(path).as_deref() == Some(wanted))
        .or_else(|| candidates.first())
        .cloned()
}

#[cfg(test)]
mod tests {

    /// **Steam's own Proton, and what it is instead of.**
    #[cfg(unix)]
    ///
    /// The shapes rather than the discovery: whether a prefix exists on the
    /// machine running the test is not something a test may depend on.
    #[test]
    fn the_bridge_is_started_through_steams_own_proton_when_there_is_one() {
        let exe = Path::new("/home/someone/pro-engineer/wineshm.exe");
        let through = dressed(
            wineshm::launch::Launch::Proton {
                wine: PathBuf::from("/steam/Proton - Experimental/files/bin/wine"),
                prefix: PathBuf::from("/steam/steamapps/compatdata/244210/pfx"),
            },
            PathBuf::from("/steam/Proton - Experimental/files/bin/wine"),
            exe,
            How::Steam,
        );

        assert_eq!(through.how, How::Steam);
        assert!(through.program.ends_with("bin/wine"));
        assert!(
            through.env.iter().any(|(key, value)| key == "WINEPREFIX"
                && value == "/steam/steamapps/compatdata/244210/pfx"),
            "the prefix is how wine knows which game it is joining: {:?}",
            through.env
        );
        // The folder, because wine resolves a path against the prefix's drive
        // mappings and a folder that is not mapped reads as "file not found".
        assert_eq!(
            through.working_dir.as_deref(),
            Some(Path::new("/home/someone/pro-engineer"))
        );

        // And the fallback still speaks protontricks' command line.
        let tricks = dressed(
            wineshm::launch::Launch::Protontricks { app_id: 244210 },
            PathBuf::from("protontricks-launch"),
            exe,
            How::Protontricks,
        );
        assert!(
            tricks.args.starts_with(&[
                "--appid".to_string(),
                "244210".to_string(),
                exe.to_string_lossy().into_owned()
            ]),
            "{:?}",
            tricks.args
        );
        assert!(
            !tricks.env.iter().any(|(k, _)| k == "WINEPREFIX"),
            "protontricks finds the prefix itself"
        );
    }

    /// **A bridge that was killed is not a bridge.**
    ///
    /// It leaves its note and its pages exactly as they were, so this used to
    /// come back `Current` and the pages beside it held a session that ended
    /// whenever the process did. Checked here on a directory of its own, with
    /// the note's age set by hand, because a test may not wait six seconds and
    /// may not touch `/dev/shm`.
    #[cfg(unix)]
    #[test]
    fn a_note_nothing_is_maintaining_is_not_a_running_bridge() {
        use wineshm::liveness::{Pulse, pulse};

        // The rule the status depends on, stated where it can be seen: a note
        // older than the window is abandoned, and one touched now is not.
        assert_eq!(
            pulse(
                std::time::Duration::from_secs(60),
                false,
                wineshm::liveness::STALE
            ),
            Pulse::Abandoned
        );
        assert_eq!(
            pulse(std::time::Duration::ZERO, false, wineshm::liveness::STALE),
            Pulse::Beating
        );
        assert!(!Pulse::Abandoned.is_worth_reading());

        // And the status this crate builds on it says so rather than judging
        // the version of something that is gone.
        let info = BridgeInfo {
            protocol: BRIDGE_PROTOCOL,
            version: BRIDGE_VERSION.to_string(),
            frame_bytes: size_of::<OverlayFrame>(),
            mmf: OVERLAY_MMF_NAME.to_string(),
            pid: 7,
        };
        let abandoned = BridgeStatus::Abandoned(Box::new(info.clone()));
        assert!(!abandoned.is_workable(), "its pages are not a live session");
        // The same bridge, still beating, is the one that works.
        assert!(judge(info, BRIDGE_VERSION).is_workable());
    }

    /// **Every block this program needs is on the command line.**
    #[cfg(unix)]
    ///
    /// The bridge publishes what it is asked for and nothing else, so a block
    /// missing here is a mapping that never exists — and the failure is a
    /// panel waiting for ever rather than an error.
    #[test]
    fn the_blocks_this_program_needs_are_asked_for_by_name_and_size() {
        let exe = Path::new("/somewhere/wineshm.exe");
        let plan = dressed(
            wineshm::launch::Launch::Protontricks { app_id: 244210 },
            PathBuf::from("protontricks-launch"),
            exe,
            How::Protontricks,
        );
        let said = plan.args.join(" ");

        for page in pages() {
            assert!(
                said.contains(&format!("--page {}:{}", page.name, page.bytes)),
                "{} is not asked for: {said}",
                page.name
            );
        }
        // The overlay block above all: it is the one this program writes and
        // the in-game panel reads, and its size is what CSP checks.
        assert!(
            said.contains(&format!(
                "--page {}:{}",
                super::super::frame::OVERLAY_MMF_NAME,
                size_of::<super::super::frame::OverlayFrame>()
            )),
            "{said}"
        );
        assert!(said.contains("--quiet"), "nobody is watching its terminal");
        // The heartbeat has to name a block that is actually published, and
        // one that moves — the bridge refuses a name it is not serving, and a
        // static block would have it blank a live session.
        assert!(
            said.contains("--heartbeat acpmf_physics"),
            "without this the pages outlive the game: {said}"
        );
        assert!(
            pages().iter().any(|page| page.name == "acpmf_physics"),
            "the heartbeat must be one of the blocks being published"
        );
    }

    /// The five are the four the games publish and the one that runs back.
    #[test]
    fn the_page_list_is_the_games_four_and_the_panel_one() {
        let pages = pages();
        assert_eq!(pages.len(), 5, "{pages:?}");
        for wanted in [
            "acpmf_physics",
            "acpmf_graphics",
            "acpmf_static",
            "acpmf_crewchief",
        ] {
            assert!(pages.iter().any(|page| page.name == wanted), "{wanted}");
        }
        assert!(
            pages
                .iter()
                .any(|page| page.name == super::super::frame::OVERLAY_MMF_NAME)
        );
    }
    use super::*;

    /// "I built it and ran it out of the target folder and it does not see the
    /// bridge."
    ///
    /// The binary is `target/release/ac_pro_engineer` and the cross-compiled
    /// bridge is its sibling at `target/x86_64-pc-windows-gnu/release/`. The
    /// search used to look under the *working directory*, which finds that only
    /// when the shell happens to be at the root of the repository — and a
    /// checkout is very often run from somewhere else. Where the binary is does
    /// not depend on where you were standing when you started it.
    #[test]
    fn the_cross_compiled_bridge_is_found_from_the_binary() {
        let exe = Path::new("/home/someone/project/target/release/ac_pro_engineer");
        let candidates = cross_build_candidates(exe);

        assert!(
            candidates.contains(&PathBuf::from(
                "/home/someone/project/target/x86_64-pc-windows-gnu/release/wineshm.exe"
            )),
            "the sibling of the binary it was built beside: {candidates:?}"
        );
    }

    /// A debug build finds its own bridge too, rather than only a release one.
    #[test]
    fn a_debug_build_looks_for_a_debug_bridge() {
        let exe = Path::new("/w/target/debug/ac_pro_engineer");
        let candidates = cross_build_candidates(exe);
        assert!(candidates.contains(&PathBuf::from(
            "/w/target/x86_64-pc-windows-gnu/debug/wineshm.exe"
        )));
    }

    fn good_info() -> BridgeInfo {
        BridgeInfo {
            protocol: BRIDGE_PROTOCOL,
            version: "0.3.3".to_string(),
            frame_bytes: size_of::<OverlayFrame>(),
            mmf: OVERLAY_MMF_NAME.to_string(),
            pid: 42,
        }
    }

    /// A note the bridge actually writes, rendered by the crate that writes
    /// it rather than typed out here — which is the point of the two sharing
    /// a definition.
    fn a_real_note() -> String {
        wineshm::Note {
            version: "0.1.0".to_string(),
            format: BRIDGE_PROTOCOL,
            pid: 1234,
            pages: pages()
                .into_iter()
                .map(|page| (page, wineshm::Mode::Owned))
                .collect(),
        }
        .render()
    }

    #[test]
    fn a_note_from_the_bridge_parses_field_for_field() {
        let info = BridgeInfo::parse(&a_real_note()).expect("a complete note parses");
        assert_eq!(info.protocol, BRIDGE_PROTOCOL);
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.frame_bytes, size_of::<OverlayFrame>());
        assert_eq!(info.mmf, OVERLAY_MMF_NAME);
        assert_eq!(info.pid, 1234);
    }

    /// A newer bridge adding a line must stay readable here, or the check
    /// breaks in the one situation it exists for.
    #[test]
    fn an_unknown_key_is_ignored_rather_than_fatal() {
        let text = a_real_note() + "something_new=yes\n";
        assert!(BridgeInfo::parse(&text).is_some());
    }

    #[test]
    fn a_note_missing_a_required_key_is_not_a_bridge_report() {
        assert!(BridgeInfo::parse("format=1\nversion=0.1.0\n").is_none());
        assert!(BridgeInfo::parse("").is_none());
    }

    /// **A bridge serving everything except the block the panel reads.**
    ///
    /// It is running, its note is valid, every other mapping is there — and
    /// the panel waits for ever. That has to read as "no usable bridge"
    /// rather than as a healthy one.
    #[test]
    fn a_note_without_the_overlay_block_is_not_a_bridge_this_can_use() {
        let text = wineshm::Note {
            version: "0.1.0".to_string(),
            format: BRIDGE_PROTOCOL,
            pid: 1,
            pages: wineshm::page::preset("assetto-corsa")
                .unwrap_or_default()
                .into_iter()
                .map(|page| (page, wineshm::Mode::Owned))
                .collect(),
        }
        .render();
        assert!(BridgeInfo::parse(&text).is_none());
    }

    /// The failure that presents as silence: CSP will not open a mapping
    /// smaller than the struct the panel declares.
    #[test]
    fn a_bridge_that_maps_too_few_bytes_is_incompatible() {
        let mut info = good_info();
        info.frame_bytes = 256;

        let status = judge(info, "0.3.3");
        let complaint = match &status {
            BridgeStatus::Incompatible { complaint, .. } => Some(complaint.clone()),
            _ => None,
        };

        assert_eq!(
            complaint,
            Some(Complaint::FrameBytes {
                found: 256,
                expected: size_of::<OverlayFrame>()
            }),
            "a bridge mapping too few bytes must be called incompatible, got {status:?}"
        );
        assert!(
            complaint.is_some_and(|complaint| complaint.describe().contains("will not open")),
            "the complaint has to say what happens, not just that it differs"
        );
    }

    /// A bridge built after the frame shrank maps more than it needs to, and
    /// that is harmless — the panel reads the first 424 bytes either way.
    #[test]
    fn a_bridge_that_maps_more_than_enough_is_accepted() {
        let mut info = good_info();
        info.frame_bytes = size_of::<OverlayFrame>() + 1024;
        assert!(matches!(judge(info, "0.3.3"), BridgeStatus::Current(_)));
    }

    #[test]
    fn a_bridge_mapping_another_block_is_incompatible() {
        let mut info = good_info();
        info.mmf = "AcTools.CSP.Limited.SomethingElse".to_string();
        assert!(matches!(
            judge(info, "0.3.3"),
            BridgeStatus::Incompatible { .. }
        ));
    }

    /// A different release that still maps the right bytes under the right
    /// name works. Saying so is a warning, not an error, and the difference
    /// matters: telling people to rebuild when they do not have to is how a
    /// check stops being read.
    #[test]
    fn a_bridge_from_another_release_still_works() {
        let mut info = good_info();
        info.version = "0.3.1".to_string();

        let status = judge(info, "0.3.3");
        assert!(status.is_workable(), "an older compatible bridge serves");

        let expected = match &status {
            BridgeStatus::Behind {
                expected_version, ..
            } => Some(expected_version.as_str()),
            _ => None,
        };
        assert_eq!(
            expected,
            Some("0.3.3"),
            "a working bridge from another release is Behind, not broken: {status:?}"
        );
        assert_eq!(status.running_version(), Some("0.3.1"));
    }

    #[test]
    fn the_same_release_reports_current() {
        let status = judge(good_info(), "0.3.3");
        assert!(status.is_workable());
        assert_eq!(status.running_version(), Some("0.3.3"));
        assert!(matches!(status, BridgeStatus::Current(_)));
    }

    #[test]
    fn an_incompatible_bridge_is_not_workable() {
        let mut info = good_info();
        info.protocol = BRIDGE_PROTOCOL + 1;
        assert!(!judge(info, "0.3.3").is_workable());
    }

    #[test]
    fn the_marker_is_read_back_out_of_surrounding_noise() {
        let mut bytes = vec![0xAB; 4096];
        bytes.extend_from_slice(format!("{VERSION_MARKER_PREFIX}1.2.3;").as_bytes());
        bytes.extend_from_slice(&[0xCD; 4096]);

        assert_eq!(version_in_bytes(&bytes).as_deref(), Some("1.2.3"));
    }

    /// A binary from before the marker existed is older than this check, and
    /// saying nothing is the honest answer.
    #[test]
    fn a_binary_without_the_marker_reports_nothing() {
        assert_eq!(version_in_bytes(&[0u8; 8192]), None);
    }

    /// Without the cap, a marker whose terminator was stripped would read the
    /// rest of the executable as a version string.
    #[test]
    fn a_marker_without_its_terminator_is_refused_not_run_away_with() {
        let mut bytes = VERSION_MARKER_PREFIX.as_bytes().to_vec();
        bytes.extend_from_slice(&[b'9'; 4096]);
        assert_eq!(version_in_bytes(&bytes), None);
    }

    /// Windows has no bridge, and reporting a missing one there would send
    /// people looking for a component that does not apply to them.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_needs_no_bridge() {
        assert_eq!(status("0.3.3"), BridgeStatus::NotRequired);
        assert!(BridgeStatus::NotRequired.is_workable());
    }

    /// The marker has to survive the toolchain that actually builds the bridge
    /// — release LTO, `strip = "debuginfo"`, and a linker that is free to drop
    /// a static nothing reads. `#[used]` is what keeps it, and asserting that
    /// on a hand-made byte array proves nothing about the real thing.
    ///
    /// Skipped where the bridge has not been cross-built, so a checkout without
    /// a mingw toolchain is unaffected:
    ///
    /// ```text
    /// cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu
    /// ```
    #[test]
    fn the_marker_survives_a_real_release_build_of_the_bridge() {
        let built = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
            .join("target/x86_64-pc-windows-gnu/release")
            .join(BRIDGE_EXE);

        if !built.is_file() {
            eprintln!(
                "{} has not been cross-built; skipping the marker check",
                built.display()
            );
            return;
        }

        assert_eq!(
            version_in_executable(&built).as_deref(),
            Some(env!("CARGO_PKG_VERSION")),
            "{} does not announce this build's version. `None` means the linker \
             dropped the marker despite `#[used]`, and the application can no \
             longer tell how old a bridge on disk is",
            built.display()
        );
    }

    /// Whatever a bridge left behind, an empty file is not a report.
    #[test]
    fn an_empty_note_is_not_a_bridge_report() {
        assert!(BridgeInfo::parse("").is_none());
        assert!(BridgeInfo::parse("garbage without any equals sign").is_none());
    }
}
