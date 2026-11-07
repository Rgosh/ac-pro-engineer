//! Every car and circuit checked at once: what will not load, what will load
//! wrong, and what takes the room.
//!
//! **The question Content Manager's users ask a forum**, after the game
//! crashed on a car they installed a year ago: which of my eight hundred
//! mods is broken? Each check here is one the game itself trips over — a car
//! with no data, no model or no livery does not load; a circuit with no
//! surfaces has no grip anywhere; one with no AI line has no opponents — so
//! every finding is a reason, not a style note. `preflight` asks the same
//! of the one car and circuit about to be driven; this asks it of all of
//! them.
//!
//! **Blocking and proportional to the install**: it walks every folder for
//! the sizes. Called on a thread.

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
    /// What is switched off — out of the game's sight in `content-off/`,
    /// still on the disk — with its size, largest first.
    pub off: Vec<(&'static str, String, u64)>,
    /// Folders that are not a car or circuit at all — see [`present`] —
    /// with their size. Not checked and not counted.
    pub absent: Vec<(&'static str, String, u64)>,
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

    /// How many of a kind (`cars` or `tracks`), and their size together.
    pub fn of_kind(&self, kind: &str) -> (usize, u64) {
        self.sizes
            .iter()
            .filter(|(k, _, _)| *k == kind)
            .fold((0, 0), |(n, bytes), (_, _, b)| (n + 1, bytes + b))
    }

    /// Each reason found, with how many it was found for, broken first.
    pub fn reasons(&self) -> Vec<(&'static str, Severity, usize)> {
        let mut out: Vec<(&'static str, Severity, usize)> = Vec::new();
        for finding in &self.findings {
            match out.iter_mut().find(|(what, _, _)| *what == finding.what) {
                Some((_, _, n)) => *n += 1,
                None => out.push((finding.what, finding.severity, 1)),
            }
        }
        out.sort_by_key(|a| (a.1, std::cmp::Reverse(a.2)));
        out
    }

    /// The size of one car or circuit, when it was measured.
    pub fn size_of(&self, kind: &str, id: &str) -> Option<u64> {
        self.sizes
            .iter()
            .find(|(k, i, _)| *k == kind && i == id)
            .map(|(_, _, b)| *b)
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
            if !present(kind, &at) {
                report.absent.push((kind, id.clone(), size_of(&at)));
                continue;
            }
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
                    check_car(&at, &mut said);
                }
                _ => {
                    report.tracks += 1;
                    check_track(&at, &mut said);
                }
            }
            report.sizes.push((kind, id.clone(), size_of(&at)));
        }
    }
    for (kind, id, path) in super::adding::switched_off(root) {
        let kind = if kind == "cars" { "cars" } else { "tracks" };
        report.off.push((kind, id, size_of(&path)));
    }
    report.off.sort_by_key(|o| std::cmp::Reverse(o.2));
    report
        .findings
        .sort_by(|a, b| (a.severity, a.kind, &a.id).cmp(&(b.severity, b.kind, &b.id)));
    report.sizes.sort_by_key(|s| std::cmp::Reverse(s.2));
    report
}

/// A file or folder in `dir` by name, whatever its case — a mod packed on
/// Windows writes `Data.acd` or `UI` as often as not, and the game, running
/// on Windows's file system or Wine's, finds it either way.
fn child(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    let exact = dir.join(name);
    if exact.exists() {
        return Some(exact);
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        e.file_name()
            .to_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(name))
            .then(|| e.path())
    })
}

fn has_kn5(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("kn5"))
        })
    })
}

/// Whether a folder under `content/` is a car or circuit at all, broken or
/// not, rather than a folder that only holds its sounds.
///
/// **Steam puts a folder for every DLC car in the game, owned or not**, with
/// only its `sfx` in it — the sound banks ship with the base game. Read as
/// cars, every DLC somebody does not own was "not installed completely, no
/// livery, no ui_car.json": 174 errors on a real install, every one of them
/// wrong. A folder with none of a car's or circuit's own parts is not one.
pub fn present(kind: &str, at: &Path) -> bool {
    match kind {
        // Data, a model or its description — not an empty `ui` or `skins`,
        // which the unowned DLC folders on a real install had: eighty of
        // them were still read as cars missing everything.
        "cars" => {
            child(at, "data.acd").is_some()
                || child(at, "data").is_some_and(|d| d.is_dir())
                || has_kn5(at)
                || child(at, "ui")
                    .and_then(|ui| child(&ui, "ui_car.json"))
                    .is_some()
        }
        _ => {
            let place_is = |place: &Path| {
                has_kn5(place)
                    || child(place, "ai").is_some()
                    || child(place, "data")
                        .and_then(|d| child(&d, "surfaces.ini"))
                        .is_some()
            };
            place_is(at)
                || std::fs::read_dir(at).is_ok_and(|entries| {
                    entries
                        .flatten()
                        .any(|e| e.path().is_dir() && place_is(&e.path()))
                })
        }
    }
}

fn check_car(at: &Path, said: &mut impl FnMut(Severity, &'static str)) {
    let has_data = child(at, "data.acd").is_some_and(|p| p.is_file())
        || child(at, "data").is_some_and(|p| p.is_dir());
    if !has_data {
        said(
            Severity::Broken,
            "No data — it was not installed completely",
        );
    }
    if has_data && !has_kn5(at) {
        said(
            Severity::Broken,
            "No model (.kn5) — it was not installed completely",
        );
    }
    let skins = child(at, "skins")
        .and_then(|skins| std::fs::read_dir(skins).ok())
        .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
        .unwrap_or(0);
    if skins == 0 {
        said(
            Severity::Broken,
            "No livery — the game cannot load a car without one",
        );
    }
    let ui_car = child(at, "ui").and_then(|ui| child(&ui, "ui_car.json"));
    match ui_car
        .map(std::fs::read_to_string)
        .unwrap_or_else(|| Err(std::io::Error::from(std::io::ErrorKind::NotFound)))
    {
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
    if !has_kn5(at) {
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
    if places.iter().any(|place| {
        child(place, "data")
            .and_then(|data| child(&data, "surfaces.ini"))
            .is_none()
    }) {
        said(
            Severity::Broken,
            "No surfaces.ini — no grip anywhere, the game will not load it",
        );
    }
    if places.iter().any(|place| {
        child(place, "ai")
            .and_then(|ai| child(&ai, "fast_lane.ai"))
            .is_none()
    }) {
        said(
            Severity::Warning,
            "No AI line — no opponents and no racing line here",
        );
    }
    if child(at, "ui").is_none() {
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
        // Half a car: its description and a livery, no data, no model.
        file(&cars.join("mod_half"), "skins/a/x.jpg", "p");
        file(
            &cars.join("mod_half"),
            "ui/ui_car.json",
            r#"{"name":"Half"}"#,
        );
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

        // A DLC nobody here owns: Steam's folder with its sounds alone.
        file(
            &cars.join("ks_porsche_919_hybrid_2016"),
            "sfx/GUIDs.txt",
            "x",
        );
        // As the owner's install has them: with an empty `ui` and `skins`.
        file(&cars.join("ks_audi_r8_lms"), "sfx/GUIDs.txt", "x");
        file(&cars.join("ks_audi_r8_lms"), "ui/badge.png", "b");
        std::fs::create_dir_all(cars.join("ks_audi_r8_lms/skins")).unwrap_or_default();
        file(&tracks.join("ks_highlands"), "ui/preview.png", "p");
        // A mod packed on Windows, its parts in other cases: whole.
        file(&cars.join("mod_caps"), "Data.acd", "x");
        file(&cars.join("mod_caps"), "caps.KN5", "m");
        file(&cars.join("mod_caps"), "Skins/one/preview.jpg", "p");
        file(
            &cars.join("mod_caps"),
            "UI/UI_car.json",
            r#"{"name":"Caps"}"#,
        );

        let report = check(&root);
        assert_eq!(
            report
                .absent
                .iter()
                .map(|a| a.1.as_str())
                .collect::<Vec<_>>(),
            vec![
                "ks_audi_r8_lms",
                "ks_porsche_919_hybrid_2016",
                "ks_highlands"
            ],
            "a folder with only its sounds is not a broken car"
        );
        assert!(
            report.findings.iter().all(|f| f.id != "mod_caps"),
            "{:?}",
            report.findings
        );
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
                    "mod_ring",
                    "No AI line — no opponents and no racing line here",
                    Severity::Warning
                ),
            ]
        );
        assert_eq!(report.counts(), (3, 2));
        assert_eq!((report.cars, report.tracks), (4, 2));
        assert_eq!(report.sizes.first().map(|s| s.1.as_str()), Some("ks_good"));
        assert_eq!(
            report.total(),
            report.sizes.iter().map(|s| s.2).sum::<u64>()
        );
        assert_eq!(report.of_kind("cars").0, 4);
        assert_eq!(report.of_kind("tracks").0, 2);
        let reasons = report.reasons();
        assert_eq!(reasons.len(), 5);
        assert!(reasons.first().is_some_and(|r| r.1 == Severity::Broken));
        assert!(report.size_of("cars", "ks_good").is_some_and(|b| b >= 5000));
        // Something switched off is counted apart, with its size.
        file(
            &root.join("content-off/cars/mod_old"),
            "data.acd",
            &"x".repeat(300),
        );
        let again = check(&root);
        assert_eq!(
            again.off.first().map(|o| (o.0, o.1.as_str())),
            Some(("cars", "mod_old"))
        );
        assert_eq!(again.cars, 4, "what is off is not checked as installed");
        let _ = std::fs::remove_dir_all(&root);
    }
}
