//! Every car and circuit checked at once: what will not load, what will load
//! wrong, and what takes the room.
//!
//! **The question Content Manager's users ask a forum**, after the game
//! crashed on a car they installed a year ago: which of my eight hundred
//! mods is broken? Each check here is one the game itself trips over — a car
//! with no data, no model or no livery does not load; a circuit with no
//! surfaces has no grip anywhere; one with no AI line has no opponents — so
//! every finding is a reason, not a style note. [`preflight`] asks the same
//! of the one car and circuit about to be driven; this asks it of all of
//! them.
//!
//! **Blocking and proportional to the install**: it walks every folder for
//! the sizes. Called on a thread.

use super::preflight;
use std::path::Path;

/// How bad a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The game will not load it, or will crash on it.
    Broken,
    /// It loads, and something about it is wrong.
    Warning,
}

/// One thing found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// `cars` or `tracks`.
    pub kind: &'static str,
    pub id: String,
    pub severity: Severity,
    /// In English, for the front end to translate: the key is the sentence.
    pub what: &'static str,
}

/// The whole check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Broken first, then by kind and folder.
    pub findings: Vec<Finding>,
    /// Every car and circuit with its size in bytes, largest first.
    pub sizes: Vec<(&'static str, String, u64)>,
    pub cars: usize,
    pub tracks: usize,
}

impl Report {
    /// How many are broken, and how many only warned about.
    pub fn counts(&self) -> (usize, usize) {
        let broken = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Broken)
            .count();
        (broken, self.findings.len() - broken)
    }

    /// Everything's size together.
    pub fn total(&self) -> u64 {
        self.sizes.iter().map(|(_, _, bytes)| bytes).sum()
    }
}

/// Check every car and circuit in an install.
pub fn check(root: &Path) -> Report {
    let mut report = Report::default();
    for (kind, folder) in [("cars", "cars"), ("tracks", "tracks")] {
        let Ok(entries) = std::fs::read_dir(root.join("content").join(folder)) else {
            continue;
        };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect();
        ids.sort();
        for id in ids {
            let at = root.join("content").join(folder).join(&id);
            let mut said = |severity, what| {
                report.findings.push(Finding {
                    kind,
                    id: id.clone(),
                    severity,
                    what,
                })
            };
            match kind {
                "cars" => {
                    report.cars += 1;
                    check_car(root, &id, &at, &mut said);
                }
                _ => {
                    report.tracks += 1;
                    check_track(&at, &mut said);
                }
            }
            report.sizes.push((kind, id.clone(), size_of(&at)));
        }
    }
    report
        .findings
        .sort_by(|a, b| (a.severity, a.kind, &a.id).cmp(&(b.severity, b.kind, &b.id)));
    report.sizes.sort_by_key(|s| std::cmp::Reverse(s.2));
    report
}

fn check_car(root: &Path, id: &str, at: &Path, said: &mut impl FnMut(Severity, &'static str)) {
    if !at.join("data.acd").is_file() && !at.join("data").is_dir() {
        said(
            Severity::Broken,
            "No data — it was not installed completely",
        );
    }
    if preflight::car_ready(root, id).is_err()
        && (at.join("data.acd").is_file() || at.join("data").is_dir())
    {
        said(
            Severity::Broken,
            "No model (.kn5) — it was not installed completely",
        );
    }
    let skins = std::fs::read_dir(at.join("skins"))
        .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
        .unwrap_or(0);
    if skins == 0 {
        said(
            Severity::Broken,
            "No livery — the game cannot load a car without one",
        );
    }
    match std::fs::read_to_string(at.join("ui").join("ui_car.json")) {
        Err(_) => said(
            Severity::Warning,
            "No ui_car.json — it is listed by its folder name, with no class or specs",
        ),
        Ok(text) => {
            // The game's own leniency: control characters inside strings.
            let cleaned: String = text
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            if serde_json::from_str::<serde_json::Value>(&cleaned).is_err() {
                said(
                    Severity::Warning,
                    "ui_car.json does not read — its name and specs are lost",
                );
            }
        }
    }
}

fn check_track(at: &Path, said: &mut impl FnMut(Severity, &'static str)) {
    let has_kn5 = std::fs::read_dir(at).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("kn5"))
        })
    });
    if !has_kn5 {
        said(
            Severity::Broken,
            "No model (.kn5) — it was not installed completely",
        );
    }
    // Its layouts: folders beside `ui` with a `data` of their own, or the
    // circuit itself when it has none.
    let layouts: Vec<std::path::PathBuf> = std::fs::read_dir(at)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join("data").is_dir() && p.join("ai").is_dir())
                .collect()
        })
        .unwrap_or_default();
    let places = match layouts.is_empty() {
        true => vec![at.to_path_buf()],
        false => layouts,
    };
    if places
        .iter()
        .any(|place| !place.join("data").join("surfaces.ini").is_file())
    {
        said(
            Severity::Broken,
            "No surfaces.ini — no grip anywhere, the game will not load it",
        );
    }
    if places
        .iter()
        .any(|place| !place.join("ai").join("fast_lane.ai").is_file())
    {
        said(
            Severity::Warning,
            "No AI line — no opponents and no racing line here",
        );
    }
    if !at.join("ui").is_dir() {
        said(
            Severity::Warning,
            "No ui folder — it is listed by its folder name, with no picture",
        );
    }
}

/// A folder's size, every file under it. Links are not followed.
pub fn size_of(at: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![at.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(at: &Path, name: &str, body: &str) {
        let path = at.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_default();
        }
        std::fs::write(path, body).unwrap_or_default();
    }

    /// **Each broken thing is named for the reason the game trips over
    /// it**, a whole car and a whole circuit have nothing said about them,
    /// and the sizes add up largest first.
    #[test]
    fn broken_cars_and_circuits_are_found_and_whole_ones_are_not() {
        let root = std::env::temp_dir().join(format!("ac-health-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cars = root.join("content/cars");
        let tracks = root.join("content/tracks");
        // Whole.
        file(&cars.join("ks_good"), "data.acd", "x");
        file(&cars.join("ks_good"), "good.kn5", &"m".repeat(5000));
        file(&cars.join("ks_good"), "skins/red/preview.jpg", "p");
        file(
            &cars.join("ks_good"),
            "ui/ui_car.json",
            r#"{"name":"Good"}"#,
        );
        // No livery, and a ui_car.json that does not read.
        file(&cars.join("mod_bare"), "data.acd", "x");
        file(&cars.join("mod_bare"), "bare.kn5", "m");
        file(&cars.join("mod_bare"), "ui/ui_car.json", "{ not json");
        // Half a car: no data, no model.
        file(&cars.join("mod_half"), "skins/a/x.jpg", "p");
        // A whole circuit with two layouts.
        for layout in ["gp", "short"] {
            file(
                &tracks.join("spa"),
                &format!("{layout}/data/surfaces.ini"),
                "s",
            );
            file(
                &tracks.join("spa"),
                &format!("{layout}/ai/fast_lane.ai"),
                "a",
            );
        }
        file(&tracks.join("spa"), "spa.kn5", "m");
        file(&tracks.join("spa"), "ui/gp/ui_track.json", "{}");
        // A circuit with no AI line and no surfaces.
        file(&tracks.join("mod_ring"), "ring.kn5", "m");
        file(&tracks.join("mod_ring"), "ui/ui_track.json", "{}");

        let report = check(&root);
        let found: Vec<(&str, &str, Severity)> = report
            .findings
            .iter()
            .map(|f| (f.id.as_str(), f.what, f.severity))
            .collect();
        assert_eq!(
            found,
            vec![
                (
                    "mod_bare",
                    "No livery — the game cannot load a car without one",
                    Severity::Broken
                ),
                (
                    "mod_half",
                    "No data — it was not installed completely",
                    Severity::Broken
                ),
                (
                    "mod_ring",
                    "No surfaces.ini — no grip anywhere, the game will not load it",
                    Severity::Broken
                ),
                (
                    "mod_bare",
                    "ui_car.json does not read — its name and specs are lost",
                    Severity::Warning
                ),
                (
                    "mod_half",
                    "No ui_car.json — it is listed by its folder name, with no class or specs",
                    Severity::Warning
                ),
                (
                    "mod_ring",
                    "No AI line — no opponents and no racing line here",
                    Severity::Warning
                ),
            ]
        );
        assert_eq!(report.counts(), (3, 3));
        assert_eq!((report.cars, report.tracks), (3, 2));
        assert_eq!(report.sizes.first().map(|s| s.1.as_str()), Some("ks_good"));
        assert_eq!(
            report.total(),
            report.sizes.iter().map(|s| s.2).sum::<u64>()
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
