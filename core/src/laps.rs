//! Where a saved lap lives, and the one place that decides it.
//!
//! A lap is already JSON — `LapData` serialises whole — so this is not a
//! database, it is a folder with rules. The rules are what was missing.
//!
//! # What this replaces
//!
//! Both front ends wrote and read `saved_laps/`, **relative to the working
//! directory**, and each held its own half of the logic. That is fine until
//! two programs disagree about what the working directory is, which they
//! always do: the terminal is started from the folder it was unpacked into,
//! and the window is started from a desktop entry, whose working directory is
//! the user's home. Neither is wrong and neither can see the other's laps —
//! which reads, to the person driving, as laps vanishing.
//!
//! So a lap goes where the settings and the records already go:
//! `config::app_dir()/saved_laps`, one folder per user, found the same way from
//! any working directory and by any front end.
//!
//! # Three smaller things that were also wrong
//!
//! * **The write was not atomic.** `fs::write` returns when the data is with
//!   the operating system, not when it is on the disk. Everything else that
//!   saves user data here goes through [`crate::atomic_file`]; a lap is worth
//!   the same care, because the file it would leave behind is one that fails
//!   to parse on the day somebody wants it.
//! * **Two laps could share a name.** `car_track_1-23-456.json` is the same
//!   name for the same time driven twice, and the second one silently replaced
//!   the first. The date is part of the name now, and a name that is somehow
//!   still taken gets a counter rather than the other lap's contents.
//! * **The name was sanitised by replacing three characters.** Spaces and
//!   slashes were handled; colons, quotes and the rest of what Windows refuses
//!   were not, and a car or track with one in it failed to save with an error
//!   about the file system.
//!
//! # Listing does not parse
//!
//! A lap carries its whole telemetry trace and runs to megabytes.
//! [`LapStore::list`] therefore reads names and modification times and nothing
//! else — opening forty files to draw a menu would make the menu the slowest
//! thing in the program. What a listing knows about a lap is what its name
//! says; [`LapStore::load`] is where the lap itself comes from.

use std::fs;
use std::path::{Path, PathBuf};

use crate::analyzer::LapData;

/// The most a saved lap may be before this refuses to read it.
///
/// A lap is a few hundred kilobytes. Ten megabytes is far above anything this
/// program writes and far below anything that would hurt to read, which makes
/// it a guard against a wrong file rather than a limit anybody meets.
pub const MAX_LAP_BYTES: u64 = 10 * 1024 * 1024;

/// The folder holding one driver's saved laps.
#[derive(Debug, Clone)]
pub struct LapStore {
    dir: PathBuf,
}

/// One file in the store, as much as can be known without reading it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedLap {
    /// The file's own name, which is what a menu shows and what
    /// [`LapStore::load`] takes.
    pub file_name: String,
    pub path: PathBuf,
}

impl Default for LapStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LapStore {
    /// The store this machine's user has, beside their settings and records.
    pub fn new() -> Self {
        Self {
            dir: crate::config::app_dir().join("saved_laps"),
        }
    }

    /// A store somewhere else — for tests, and for reading a folder somebody
    /// was handed.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Write a lap, and say where it went.
    ///
    /// Atomic, so a crash or a power loss leaves either the previous state or
    /// the whole lap and never half of one.
    pub fn save(&self, lap: &LapData) -> Result<PathBuf, String> {
        let json = serde_json::to_string_pretty(lap)
            .map_err(|e| format!("could not serialise the lap: {e}"))?;
        let path = self.free_path(&file_name_for(lap));
        crate::atomic_file::write_atomic(&path, json.as_bytes())
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        Ok(path)
    }

    /// Every saved lap, newest first.
    ///
    /// By modification time rather than by name: a name sorts by car before it
    /// sorts by date, and the lap somebody wants is nearly always the one they
    /// just drove. Missing metadata sorts last rather than failing the listing.
    pub fn list(&self) -> Vec<SavedLap> {
        let mut found: Vec<(std::time::SystemTime, SavedLap)> = Vec::new();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let when = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            found.push((
                when,
                SavedLap {
                    file_name: file_name.to_string(),
                    path: path.clone(),
                },
            ));
        }
        found.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.file_name.cmp(&b.1.file_name))
        });
        found.into_iter().map(|(_, lap)| lap).collect()
    }

    /// Read one lap back, by the name a listing gave.
    ///
    /// `from_file` is set here rather than by every caller: a lap that came off
    /// the disk is not a lap of this session, and everything that ranks laps
    /// asks that question.
    pub fn load(&self, file_name: &str) -> Result<LapData, String> {
        // A name from a listing, not from a user — but this is the one place
        // that turns a string into a path, so it is the place to refuse a
        // string that walks out of the folder.
        if Path::new(file_name).components().count() != 1 {
            return Err(format!("{file_name} is not a name in this folder"));
        }
        self.load_path(&self.dir.join(file_name))
    }

    /// Read a lap from anywhere — a file somebody was sent, or one still in an
    /// old folder.
    pub fn load_path(&self, path: &Path) -> Result<LapData, String> {
        let metadata = fs::metadata(path).map_err(|e| format!("could not read: {e}"))?;
        if metadata.len() > MAX_LAP_BYTES {
            return Err(format!(
                "{} is larger than a lap should ever be",
                path.display()
            ));
        }
        let text = fs::read_to_string(path).map_err(|e| format!("could not read: {e}"))?;
        let mut lap: LapData =
            serde_json::from_str(&text).map_err(|e| format!("not a saved lap: {e}"))?;
        lap.from_file = true;
        Ok(lap)
    }

    pub fn delete(&self, file_name: &str) -> Result<(), String> {
        if Path::new(file_name).components().count() != 1 {
            return Err(format!("{file_name} is not a name in this folder"));
        }
        fs::remove_file(self.dir.join(file_name)).map_err(|e| format!("could not delete: {e}"))
    }

    /// Move an older `saved_laps/` folder's contents into this store.
    ///
    /// Called once at startup with the folders the previous releases wrote to.
    /// Moving rather than copying, so it cannot be done twice and so the laps
    /// stop being in a place that will disappear the next time the program is
    /// started from somewhere else. A file whose name is already taken here is
    /// left alone: the copy in the store is the one the driver has been using.
    ///
    /// Returns how many were adopted. Failures are skipped rather than fatal —
    /// a folder that cannot be read is a folder somebody else owns, and losing
    /// a lap to a migration would be worse than the problem being fixed.
    pub fn adopt(&self, legacy: &Path) -> usize {
        if !legacy.is_dir() || legacy == self.dir {
            return 0;
        }
        let Ok(entries) = fs::read_dir(legacy) else {
            return 0;
        };
        let mut moved = 0;
        for entry in entries.flatten() {
            let from = entry.path();
            if from.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(name) = from.file_name() else {
                continue;
            };
            let to = self.dir.join(name);
            if to.exists() {
                continue;
            }
            if fs::create_dir_all(&self.dir).is_err() {
                return moved;
            }
            // Rename first: same filesystem, and it is instant. A copy is the
            // fallback for a home directory that spans two mounts.
            let ok = fs::rename(&from, &to).is_ok()
                || (fs::copy(&from, &to).is_ok() && fs::remove_file(&from).is_ok());
            if ok {
                moved += 1;
            }
        }
        moved
    }

    /// The path this name should take, with a counter if it is taken.
    fn free_path(&self, stem: &str) -> PathBuf {
        let first = self.dir.join(format!("{stem}.json"));
        if !first.exists() {
            return first;
        }
        // Two laps saved in the same second, which the driver did on purpose.
        for n in 2..1000 {
            let candidate = self.dir.join(format!("{stem}-{n}.json"));
            if !candidate.exists() {
                return candidate;
            }
        }
        first
    }
}

/// `mazda_mx5_spa_1-58-431_20260824-014233` — car, track, lap time, and when.
///
/// The lap time is in the name because that is what a driver scans a list for.
/// The stamp is there because two laps to the same thousandth is a thing that
/// happens, and the previous scheme answered it by overwriting one of them.
fn file_name_for(lap: &LapData) -> String {
    let minutes = lap.lap_time_ms / 60_000;
    let seconds = (lap.lap_time_ms % 60_000) / 1000;
    let thousandths = lap.lap_time_ms % 1000;
    format!(
        "{}_{}_{}-{:02}-{:03}_{}",
        safe(&lap.car_model),
        safe(&lap.track_name),
        minutes,
        seconds,
        thousandths,
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
    )
}

/// A file name component that every filesystem this runs on will accept.
///
/// Allowing a set rather than replacing a list: the previous version replaced
/// spaces and both slashes, which left `:`, `?`, `*`, `"`, `<`, `>` and `|` —
/// all of which Windows refuses — to fail the save with an error about the
/// file system rather than about the car's name.
fn safe(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_underscore = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '.' {
            out.push(ch);
            last_was_underscore = false;
        } else if !last_was_underscore {
            out.push('_');
            last_was_underscore = true;
        }
    }
    let trimmed = out.trim_matches(['_', '.']).to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lap(car: &str, track: &str, time_ms: i32) -> LapData {
        LapData {
            car_model: car.to_string(),
            track_name: track.to_string(),
            lap_time_ms: time_ms,
            ..Default::default()
        }
    }

    /// The saved file's own name. `expect` rather than `unwrap` so a failure
    /// here reads as what it is instead of as a line number.
    fn file_name_of(path: &Path) -> String {
        path.file_name()
            .and_then(|name| name.to_str())
            .expect("a saved lap has a readable name")
            .to_string()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("acpe-laps-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("make a scratch folder");
        dir
    }

    #[test]
    fn a_saved_lap_reads_back_as_the_lap_that_was_saved() {
        let store = LapStore::with_dir(scratch("roundtrip"));
        let path = store.save(&lap("Mazda MX5", "Spa", 118_431)).expect("save");
        let name = file_name_of(&path);

        let back = store.load(&name).expect("load");
        assert_eq!(back.car_model, "Mazda MX5");
        assert_eq!(back.lap_time_ms, 118_431);
        assert!(
            back.from_file,
            "a lap off the disk is not a lap of this session, and the ranking asks"
        );
    }

    /// The bug this module exists for, in its smallest form: the old scheme
    /// named a file after the car, the track and the time, so the same lap time
    /// driven twice replaced the first one without a word.
    #[test]
    fn the_same_lap_time_twice_keeps_both_laps() {
        let store = LapStore::with_dir(scratch("collide"));
        let first = store.save(&lap("MX5", "Spa", 118_431)).expect("first");
        let second = store.save(&lap("MX5", "Spa", 118_431)).expect("second");

        assert_ne!(first, second, "the second save overwrote the first");
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn a_car_named_with_what_windows_refuses_still_saves() {
        let store = LapStore::with_dir(scratch("naming"));
        let path = store
            .save(&lap("Ferrari 488 GT3 \"Evo\": #51/2", "Spa/GP*", 90_000))
            .expect("save a car with a hostile name");
        let name = file_name_of(&path);
        for bad in [':', '*', '?', '"', '<', '>', '|', '/', '\\'] {
            assert!(!name.contains(bad), "{name} still contains {bad}");
        }
    }

    #[test]
    fn the_newest_lap_is_first_in_the_list() {
        let store = LapStore::with_dir(scratch("order"));
        store.save(&lap("MX5", "Spa", 118_431)).expect("older");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let newer = store.save(&lap("MX5", "Spa", 117_002)).expect("newer");

        let listed = store.list();
        assert_eq!(listed.first().map(|l| l.path.clone()), Some(newer));
    }

    #[test]
    fn a_lap_in_an_old_folder_is_adopted_once() {
        let legacy = scratch("legacy");
        let store = LapStore::with_dir(scratch("adopting"));
        let stray = LapStore::with_dir(&legacy);
        stray.save(&lap("MX5", "Spa", 118_431)).expect("save");

        assert_eq!(store.adopt(&legacy), 1);
        assert_eq!(store.list().len(), 1);
        assert_eq!(
            store.adopt(&legacy),
            0,
            "the folder was emptied, so a second run has nothing to move"
        );
    }

    #[test]
    fn a_name_that_walks_out_of_the_folder_is_refused() {
        let store = LapStore::with_dir(scratch("escape"));
        assert!(store.load("../config.json").is_err());
        assert!(store.delete("../records.json").is_err());
    }

    /// **A lap saved by an older release still loads.**
    ///
    /// `tests/fixtures/lap-v0.3.json` is a real lap off the author's disk with
    /// its trace cut to one sample — twelve keys per sample, no `rpms` and no
    /// `detail`. It stopped loading when `rpms` was added to the struct and
    /// not to the format's rules, and the symptom was a driver selecting his
    /// own reference lap and being told it was "not a saved lap".
    ///
    /// A lap is a file that outlives every release, so this is the test to
    /// *extend* rather than rewrite the next time a field is added: put the
    /// older shape in and check it still comes back.
    #[test]
    fn a_lap_saved_by_an_older_release_still_loads() {
        let dir = scratch("older-release");
        fs::write(
            dir.join("older.json"),
            include_str!("../tests/fixtures/lap-v0.3.json"),
        )
        .expect("write");

        let store = LapStore::with_dir(&dir);
        let lap = store
            .load("older.json")
            .expect("a lap saved before `rpms` existed is still a lap");

        assert_eq!(lap.lap_time_ms, 29207);
        assert_eq!(lap.car_model, "kunos_ferrari_488_gt3");
        assert_eq!(lap.telemetry_trace.len(), 1);

        let sample = &lap.telemetry_trace[0];
        assert_eq!(sample.speed, 295.0);
        // Absent, so zero — and `detail.measured` is false, which is how every
        // screen tells "the lap never carried this" from "it was measured as
        // zero".
        assert_eq!(sample.rpms, 0);
        assert!(!sample.detail.measured);
    }

    #[test]
    fn something_that_is_not_a_lap_says_so_rather_than_panicking() {
        let dir = scratch("garbage");
        fs::write(dir.join("not_a_lap.json"), b"{\"hello\":1}").expect("write");
        let store = LapStore::with_dir(&dir);
        let error = store.load("not_a_lap.json").expect_err("should refuse");
        assert!(error.contains("not a saved lap"), "{error}");
    }
}
