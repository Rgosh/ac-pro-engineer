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
/// Bumped when the file gains or loses a key, not when the bridge changes.
/// Must match `BRIDGE_PROTOCOL` in `shm-bridge/src/main.rs`.
pub const BRIDGE_PROTOCOL: u32 = 1;

/// What a running bridge calls itself, in `/dev/shm`.
///
/// Must match `BRIDGE_INFO_FILE` in `shm-bridge/src/main.rs`.
pub const BRIDGE_INFO_FILE: &str = "acpe-bridge.info";

/// Filename of the bridge as it is built and shipped.
pub const BRIDGE_EXE: &str = "shm-bridge.exe";

/// The prefix the bridge compiles into its own binary, ahead of its version.
///
/// Must match `VERSION_MARKER` in `shm-bridge/src/main.rs`.
pub const VERSION_MARKER_PREFIX: &str = "ACPE-SHM-BRIDGE-VERSION=";

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
    /// Read the `key=value` lines a bridge writes.
    ///
    /// Unknown keys are ignored rather than rejected: a newer bridge adding one
    /// must stay readable by an older application, or the version check breaks
    /// in exactly the situation it exists to report.
    pub fn parse(text: &str) -> Option<Self> {
        let mut protocol = None;
        let mut version = None;
        let mut frame_bytes = None;
        let mut mmf = None;
        let mut pid = 0;

        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "protocol" => protocol = value.parse().ok(),
                "version" => version = Some(value.to_string()),
                "frame_bytes" => frame_bytes = value.parse().ok(),
                "mmf" => mmf = Some(value.to_string()),
                "pid" => pid = value.parse().unwrap_or(0),
                _ => {}
            }
        }

        Some(Self {
            protocol: protocol?,
            version: version?,
            frame_bytes: frame_bytes?,
            mmf: mmf?,
            pid,
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
        Some(info) => judge(info, expected_version),
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
    let marker = VERSION_MARKER_PREFIX.as_bytes();
    let start = bytes
        .windows(marker.len())
        .position(|window| window == marker)?
        + marker.len();

    // Terminated by the `;` the bridge writes. Capped so a marker that lost its
    // terminator reads a version rather than the rest of the binary.
    let rest = &bytes[start..bytes.len().min(start + 32)];
    let end = rest.iter().position(|b| *b == b';')?;
    std::str::from_utf8(&rest[..end]).ok().map(str::to_string)
}

/// Every place a `shm-bridge.exe` might be, in the order they are preferred
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
                "/home/someone/project/target/x86_64-pc-windows-gnu/release/shm-bridge.exe"
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
            "/w/target/x86_64-pc-windows-gnu/debug/shm-bridge.exe"
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

    #[test]
    fn a_note_from_the_bridge_parses_field_for_field() {
        let text = format!(
            "protocol=1\nversion=0.3.3\nframe_bytes={}\nmmf={OVERLAY_MMF_NAME}\npid=1234\n",
            size_of::<OverlayFrame>()
        );
        let info = BridgeInfo::parse(&text).expect("a complete note parses");
        assert_eq!(info.protocol, 1);
        assert_eq!(info.version, "0.3.3");
        assert_eq!(info.frame_bytes, size_of::<OverlayFrame>());
        assert_eq!(info.mmf, OVERLAY_MMF_NAME);
        assert_eq!(info.pid, 1234);
    }

    /// A newer bridge adding a key must stay readable here, or the check
    /// breaks in the one situation it exists for.
    #[test]
    fn an_unknown_key_is_ignored_rather_than_fatal() {
        let text = format!(
            "protocol=1\nversion=0.9.0\nframe_bytes={}\nmmf={OVERLAY_MMF_NAME}\npid=1\n\
             something_new=yes\n",
            size_of::<OverlayFrame>()
        );
        assert!(BridgeInfo::parse(&text).is_some());
    }

    #[test]
    fn a_note_missing_a_required_key_is_not_a_bridge_report() {
        assert!(BridgeInfo::parse("protocol=1\nversion=0.3.3\n").is_none());
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
        bytes.extend_from_slice(b"ACPE-SHM-BRIDGE-VERSION=1.2.3;");
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
        let mut bytes = b"ACPE-SHM-BRIDGE-VERSION=".to_vec();
        bytes.extend_from_slice(&[b'9'; 4096]);
        assert_eq!(version_in_bytes(&bytes), None);
    }

    /// The bridge writes this file, and it does not depend on ac_core — so the
    /// two constants are kept in step by reading its source, as the frame size
    /// already is.
    #[test]
    fn the_bridge_agrees_about_the_note_it_writes() {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../shm-bridge/src/main.rs"
        ))
        .expect("shm-bridge source");

        assert!(
            source.contains(&format!("BRIDGE_PROTOCOL: u32 = {BRIDGE_PROTOCOL};")),
            "shm-bridge declares a different bridge protocol than ac_core expects"
        );
        assert!(
            source.contains(&format!("BRIDGE_INFO_FILE: &str = \"{BRIDGE_INFO_FILE}\";")),
            "shm-bridge writes its note somewhere ac_core does not look"
        );
        assert!(
            source.contains(VERSION_MARKER_PREFIX),
            "shm-bridge must compile in the marker version_in_executable scans for"
        );

        // Every key `parse` requires has to be one the bridge actually writes.
        for key in ["protocol=", "version=", "frame_bytes=", "mmf=", "pid="] {
            assert!(
                source.contains(key),
                "shm-bridge does not write {key}, which this build requires"
            );
        }
    }

    /// A bridge built for this release wins over one that merely sits in a
    /// more-preferred directory.
    ///
    /// This is what makes a checkout testable. The working directory is
    /// searched before the build target, so a stale `shm-bridge.exe` at the
    /// root of the repository used to shadow the one just cross-compiled — the
    /// application spawned the stale one, called it out of date, and offered to
    /// download a third.
    #[test]
    fn the_bridge_built_for_this_release_wins_wherever_it_is() {
        let dir = std::env::temp_dir().join("acpe_bridge_choice");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create tmp");

        let marked = |name: &str, version: &str| {
            let path = dir.join(name);
            let body = format!("MZ padding {VERSION_MARKER_PREFIX}{version}; more padding");
            std::fs::write(&path, body).expect("write");
            path
        };

        let stale = marked("stale.exe", "0.3.1");
        let current = marked("current.exe", crate::updater::CURRENT_VERSION);

        // Stale first, which is the order the directories actually produce.
        let candidates = vec![stale.clone(), current.clone()];
        assert_eq!(
            choose_executable(&candidates, crate::updater::CURRENT_VERSION),
            Some(current),
            "the one carrying this build's version is the one to run"
        );

        // With nothing matching, the search order decides, as it always did.
        assert_eq!(
            choose_executable(
                std::slice::from_ref(&stale),
                crate::updater::CURRENT_VERSION
            ),
            Some(stale.clone()),
            "an old bridge is still better than no bridge, and the card says so"
        );

        assert_eq!(
            choose_executable(&[], crate::updater::CURRENT_VERSION),
            None,
            "and nothing found is still nothing found"
        );
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
