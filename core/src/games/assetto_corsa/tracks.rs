//! Reading what Assetto Corsa knows about the circuit that is loaded.
//!
//! **Every one of these files ships with the game and sits on the driver's own
//! disk.** Nothing is downloaded and nothing is bundled: a track this program
//! has never heard of, installed from a forum yesterday, carries the same files
//! in the same places, which is the whole reason to read them rather than to
//! keep a database. It also means none of Kunos's artwork is redistributed —
//! it is read from the copy the driver already owns.
//!
//! What is here, and how each was confirmed — by decoding the real files of a
//! real installation, not from memory:
//!
//! | File | What it gives |
//! |---|---|
//! | `map.png` | the rendered outline, exactly as the game draws it |
//! | `data/map.ini` | the offsets and scale that put world coordinates on it |
//! | `data/sections.ini` | the names the corners actually have, and their bounds |
//! | `data/drs_zones.ini` | where the wing may be opened |
//! | `ai/fast_lane.ai` | the line the AI drives, as x/y/z and metres |
//!
//! **What is deliberately not here: the width of the road.** `fast_lane.ai`
//! carries it, in a block after the line whose layout this has not confirmed —
//! and a guessed layout draws a road edge in the wrong place, which is worse
//! than drawing none. It stays out until it is decoded against real files the
//! same way the rest was.

use crate::track::{Alignment, DrsZone, LinePoint, Section, TrackData};
use std::path::{Path, PathBuf};
use tracing::debug;

/// The header of an `.ai` line: version, how many points, and two fields the
/// game leaves at zero on every file checked.
const AI_HEADER_BYTES: usize = 16;
/// One point of the line: three coordinates, the distance along it, and an
/// index. Twenty bytes, confirmed against a 4 470-point file whose distances
/// rise smoothly from zero.
const AI_POINT_BYTES: usize = 20;
/// The only version seen in the wild. A different one is refused rather than
/// read as if it were this one.
const AI_VERSION: i32 = 7;

/// Everything the game holds about one track layout.
///
/// `track` is what the game reports as the track's name and `config` is the
/// layout within it, which is empty on a circuit with only one. Returns an
/// empty [`TrackData`] rather than an error when nothing is found: a track
/// with no files is a normal state, and it is the state every mod used to be
/// in before its author added them.
pub fn read(install: &Path, track: &str, config: &str) -> TrackData {
    let root = install.join("content").join("tracks").join(track);
    if !root.is_dir() {
        debug!("no track folder at {}", root.display());
        return TrackData::default();
    }

    // A layout keeps its own `data` and `map.png` in a subfolder, and falls
    // back to the track's own when it has none of its own.
    let layout = (!config.is_empty()).then(|| root.join(config));
    let find = |relative: &str| -> Option<PathBuf> {
        layout
            .as_ref()
            .map(|dir| dir.join(relative))
            .filter(|path| path.is_file())
            .or_else(|| Some(root.join(relative)).filter(|path| path.is_file()))
    };

    TrackData {
        source: root.display().to_string(),
        outline: find("map.png"),
        alignment: find("data/map.ini").and_then(|path| alignment(&path)),
        sections: find("data/sections.ini")
            .map(|path| sections(&path))
            .unwrap_or_default(),
        drs: find("data/drs_zones.ini")
            .map(|path| drs(&path))
            .unwrap_or_default(),
        ai_line: find("ai/fast_lane.ai")
            .map(|path| ai_line(&path))
            .unwrap_or_default(),
    }
}

/// The simplest possible INI reader: sections and `KEY=VALUE`.
///
/// These files are written by the game's own tools and have no quoting, no
/// continuations and no comments beyond `;`. Pulling in an INI crate for four
/// files of this shape would be a dependency to audit for no gain.
fn ini(path: &Path) -> Vec<(String, Vec<(String, String)>)> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            out.push((name.trim().to_string(), Vec::new()));
        } else if let Some((key, value)) = line.split_once('=')
            && let Some((_, entries)) = out.last_mut()
        {
            entries.push((key.trim().to_uppercase(), value.trim().to_string()));
        }
    }
    out
}

fn number(entries: &[(String, String)], key: &str) -> Option<f32> {
    entries
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| value.parse().ok())
}

fn alignment(path: &Path) -> Option<Alignment> {
    let file = ini(path);
    let (_, entries) = file.iter().find(|(name, _)| name == "PARAMETERS")?;
    Some(Alignment {
        width: number(entries, "WIDTH")?,
        height: number(entries, "HEIGHT")?,
        margin: number(entries, "MARGIN").unwrap_or(0.0),
        scale_factor: number(entries, "SCALE_FACTOR").unwrap_or(1.0),
        x_offset: number(entries, "X_OFFSET")?,
        z_offset: number(entries, "Z_OFFSET")?,
    })
}

fn sections(path: &Path) -> Vec<Section> {
    ini(path)
        .iter()
        .filter(|(name, _)| name.starts_with("SECTION"))
        .filter_map(|(_, entries)| {
            let name = entries
                .iter()
                .find(|(key, _)| key == "TEXT")
                .map(|(_, value)| value.clone())?;
            Some(Section {
                name,
                from: number(entries, "IN")?,
                to: number(entries, "OUT")?,
            })
        })
        .collect()
}

fn drs(path: &Path) -> Vec<DrsZone> {
    ini(path)
        .iter()
        .filter(|(name, _)| name.starts_with("ZONE"))
        .filter_map(|(_, entries)| {
            Some(DrsZone {
                detection: number(entries, "DETECTION")?,
                start: number(entries, "START")?,
                end: number(entries, "END")?,
            })
        })
        .collect()
}

/// The AI line, out of the front of `fast_lane.ai`.
///
/// Refuses anything whose header does not describe the file it is in — a
/// truncated download and a format that changed both land here, and reading
/// either as if it were a line produces a shape that is not the track.
fn ai_line(path: &Path) -> Vec<LinePoint> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    if bytes.len() < AI_HEADER_BYTES {
        return Vec::new();
    }
    let word =
        |at: usize| i32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    if word(0) != AI_VERSION {
        debug!(
            "{} is version {}, not {AI_VERSION}",
            path.display(),
            word(0)
        );
        return Vec::new();
    }
    let count = word(4).max(0) as usize;
    if count == 0 || bytes.len() < AI_HEADER_BYTES + count * AI_POINT_BYTES {
        debug!(
            "{} claims {count} points and is too short for them",
            path.display()
        );
        return Vec::new();
    }

    let float =
        |at: usize| f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    (0..count)
        .map(|index| {
            let at = AI_HEADER_BYTES + index * AI_POINT_BYTES;
            LinePoint {
                x: float(at),
                y: float(at + 4),
                z: float(at + 8),
                metres: float(at + 12),
            }
        })
        .collect()
}

/// What the game knows about one car.
///
/// `ui/ui_car.json` ships beside every car in the game, including any mod:
/// the name a person would use, the brand, the class the author filed it
/// under, the headline specifications, and the engine's torque and power
/// against revs.
///
/// **The file is not always valid JSON.** Several of the cars that ship with
/// the game have raw newlines inside string values, which no parser accepts —
/// so control characters are replaced before parsing rather than the car being
/// reported as absent. That is a real property of the data and not a shortcut.
pub fn read_car(install: &Path, car: &str) -> crate::track::CarData {
    let root = install.join("content").join("cars").join(car);
    let ui = root.join("ui");
    let Ok(raw) = std::fs::read_to_string(ui.join("ui_car.json")) else {
        return crate::track::CarData::default();
    };
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&cleaned) else {
        debug!("{} is not readable as JSON", ui.display());
        return crate::track::CarData::default();
    };

    let text = |key: &str| {
        value
            .get(key)
            .and_then(|found| found.as_str())
            .unwrap_or_default()
            .to_string()
    };
    // Both curves are arrays of two-element arrays of *strings*, which is how
    // the game writes them.
    let curve = |key: &str| -> Vec<(f32, f32)> {
        value
            .get(key)
            .and_then(|found| found.as_array())
            .map(|points| {
                points
                    .iter()
                    .filter_map(|pair| {
                        let pair = pair.as_array()?;
                        let read = |at: usize| -> Option<f32> {
                            let entry = pair.get(at)?;
                            entry
                                .as_str()
                                .and_then(|text| text.trim().parse().ok())
                                .or_else(|| entry.as_f64().map(|number| number as f32))
                        };
                        Some((read(0)?, read(1)?))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    crate::track::CarData {
        name: text("name"),
        brand: text("brand"),
        class: text("class"),
        tags: value
            .get("tags")
            .and_then(|found| found.as_array())
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| tag.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        specs: value
            .get("specs")
            .and_then(|found| found.as_object())
            .map(|specs| {
                specs
                    .iter()
                    .filter_map(|(key, entry)| Some((key.clone(), entry.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default(),
        torque: curve("torqueCurve"),
        power: curve("powerCurve"),
        badge: Some(ui.join("badge.png")).filter(|path| path.is_file()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parser, against a file written here rather than against a real
    /// install — so it runs on a machine with no game on it.
    #[test]
    fn the_sections_of_a_track_are_read_with_their_names() {
        let dir = std::env::temp_dir().join("acpe-track-sections");
        let data = dir.join("content/tracks/spa/data");
        std::fs::create_dir_all(&data).expect("a temporary track folder");
        std::fs::write(
            data.join("sections.ini"),
            "[SECTION_0]\nIN=0.038\nOUT=0.068\nTEXT=La Source\n\n\
             [SECTION_1]\nIN=0.137\nOUT=0.154\nTEXT=Eau Rouge\n",
        )
        .expect("the file must be writable");

        let track = read(&dir, "spa", "");
        assert_eq!(track.sections.len(), 2);
        assert_eq!(track.name_at(0.05), Some("La Source"));
        assert_eq!(track.name_at(0.14), Some("Eau Rouge"));
        assert_eq!(track.name_at(0.5), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A track nobody has surveyed is a normal state and not an error.
    #[test]
    fn a_track_that_is_not_installed_reads_as_nothing() {
        let nothing = read(Path::new("/nonexistent"), "no_such_track", "");
        assert!(nothing.is_empty());
    }

    /// An `.ai` file whose header does not match its length is refused rather
    /// than read as a shape that is not the track.
    #[test]
    fn a_truncated_line_is_refused() {
        let dir = std::env::temp_dir().join("acpe-track-ai");
        let ai = dir.join("content/tracks/spa/ai");
        std::fs::create_dir_all(&ai).expect("a temporary track folder");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&7i32.to_le_bytes());
        bytes.extend_from_slice(&4_470i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        // and then nothing, where four thousand points should be
        std::fs::write(ai.join("fast_lane.ai"), &bytes).expect("the file must be writable");

        assert!(read(&dir, "spa", "").ai_line.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// **The file is not always valid JSON**, and several cars that ship with
    /// the game are the reason: raw newlines inside a string value, which no
    /// parser accepts. Reporting those cars as absent would be wrong, so the
    /// control characters are replaced — and this is the test that says so.
    #[test]
    fn a_car_whose_description_has_a_raw_newline_is_still_read() {
        let dir = std::env::temp_dir().join("acpe-car-newline");
        let ui = dir.join("content/cars/ks_test/ui");
        std::fs::create_dir_all(&ui).expect("a temporary car folder");
        std::fs::write(
            ui.join("ui_car.json"),
            "{\"name\": \"Test Car\", \"brand\": \"Brand\",\n \
             \"description\": \"one\nline break inside a string\",\n \
             \"specs\": {\"bhp\": \"130bhp\"},\n \
             \"torqueCurve\": [[\"0\", \"50\"], [\"5000\", \"152\"]],\n \
             \"powerCurve\": [[\"0\", \"0\"], [\"5000\", \"130\"]]}",
        )
        .expect("the file must be writable");

        let car = read_car(&dir, "ks_test");
        assert_eq!(car.name, "Test Car");
        assert_eq!(car.specs, vec![("bhp".to_string(), "130bhp".to_string())]);
        assert_eq!(car.torque_peak(), Some((5_000.0, 152.0)));
        assert_eq!(car.power_peak(), Some((5_000.0, 130.0)));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A car with no `ui` folder is a normal state — a mod may ship none.
    #[test]
    fn a_car_with_no_metadata_reads_as_nothing() {
        assert!(read_car(Path::new("/nonexistent"), "nobody").is_empty());
    }
}
