//! The game's numbers that are not video: sound, force feedback, the wheel,
//! the cockpit camera and the in-game display — read and set without starting
//! the game.
//!
//! **One table per screen, every row a key the game reads.** Each row says
//! which file, which section and key, what the game takes when the key is not
//! there, and how the number is shown. The keys and the defaults are the ones
//! the game and Content Manager both read: a launcher that writes a key the
//! game does not know has changed nothing, and one that shows a default the
//! game does not use shows a setting that is not the one in force.
//!
//! **One line changed, everything else left as it was** — the same rule as
//! `video` and `assists`, through `ini::set_in_file`. `controls.ini` holds the
//! wheel's axis and button bindings beside the force-feedback values; nothing
//! here touches a binding.

use super::ini;
use std::path::{Path, PathBuf};

/// The file a setting lives in, under the game's Documents `cfg` folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum File {
    Audio,
    Controls,
    CameraOnboard,
    Gameplay,
}

impl File {
    /// Its name on disk.
    pub fn name(self) -> &'static str {
        match self {
            Self::Audio => "audio.ini",
            Self::Controls => "controls.ini",
            Self::CameraOnboard => "camera_onboard.ini",
            Self::Gameplay => "gameplay.ini",
        }
    }

    /// Where it is.
    pub fn at(self, documents: &Path) -> PathBuf {
        documents.join("cfg").join(self.name())
    }
}

/// How a number is written, and how it is shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Measure {
    /// A fraction in the file (`0.75`), a percentage on screen (`75 %`), from
    /// nought to `max` — which is above 1 where the game takes more than full.
    Percent { max: f32 },
    /// A whole number in the file and on screen, with its unit.
    Whole {
        min: f32,
        max: f32,
        unit: &'static str,
    },
    /// A number with two decimals, for the curves (`2.00`).
    Decimal { min: f32, max: f32 },
    /// On or off, written as `1` or `0`.
    Switch,
}

/// One setting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    pub file: File,
    pub section: &'static str,
    pub key: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    /// What the game takes when the key is not in the file.
    pub default: f32,
    pub measure: Measure,
    /// The sign in the file is a separate switch: force feedback written as
    /// `-0.8` is 80 % reversed. The number shown and set is the size, and the
    /// sign already in the file is kept.
    pub signed: bool,
    /// A second key Content Manager keeps the same value under — its copy of
    /// the steering lock while it adjusts the lock per car. Written together,
    /// so Content Manager does not put its own copy back.
    pub twin: Option<&'static str>,
}

const fn percent(
    file: File,
    section: &'static str,
    key: &'static str,
    name: &'static str,
    about: &'static str,
    default: f32,
    max: f32,
) -> Level {
    Level {
        file,
        section,
        key,
        name,
        about,
        default,
        measure: Measure::Percent { max },
        signed: false,
        twin: None,
    }
}

const fn switch(
    file: File,
    section: &'static str,
    key: &'static str,
    name: &'static str,
    about: &'static str,
    on: bool,
) -> Level {
    Level {
        file,
        section,
        key,
        name,
        about,
        default: if on { 1.0 } else { 0.0 },
        measure: Measure::Switch,
        signed: false,
        twin: None,
    }
}

/// Sound: `audio.ini`, `[LEVELS]`. The game takes more than full on these —
/// people turn the tyres up to make the grip audible — so the slider goes to
/// 200 %.
pub const AUDIO: &[Level] = &[
    percent(
        File::Audio,
        "LEVELS",
        "MASTER",
        "Master",
        "Everything, together.",
        1.0,
        2.0,
    ),
    percent(
        File::Audio,
        "LEVELS",
        "ENGINE",
        "Engine",
        "Your own engine.",
        1.0,
        2.0,
    ),
    percent(
        File::Audio,
        "LEVELS",
        "TYRES",
        "Tyres",
        "The squeal at the limit — the sound that says how much grip is left.",
        0.8,
        2.0,
    ),
    percent(
        File::Audio,
        "LEVELS",
        "BRAKES",
        "Brakes",
        "Brakes and their squeal.",
        0.8,
        2.0,
    ),
    percent(
        File::Audio,
        "LEVELS",
        "SURFACES",
        "Surfaces",
        "Kerbs, grass and gravel under the wheels.",
        1.0,
        2.0,
    ),
    percent(
        File::Audio,
        "LEVELS",
        "WIND",
        "Wind",
        "The air at speed.",
        0.9,
        2.0,
    ),
    percent(
        File::Audio,
        "LEVELS",
        "OPPONENTS",
        "Opponents",
        "Other cars. Turn it down when they drown out your own engine.",
        0.9,
        2.0,
    ),
    percent(
        File::Audio,
        "LEVELS",
        "DIRT_BOTTOM",
        "Dirt under the car",
        "Stones and dirt hitting the floor off the track.",
        1.0,
        2.0,
    ),
    percent(
        File::Audio,
        "LEVELS",
        "UISOUNDS",
        "Menus",
        "The game's own menu sounds.",
        0.7,
        2.0,
    ),
];

/// Force feedback: `controls.ini`. Values only — the bindings in the same
/// file are never touched.
pub const FORCE_FEEDBACK: &[Level] = &[
    Level {
        file: File::Controls,
        section: "STEER",
        key: "FF_GAIN",
        name: "Strength",
        about: "How hard the wheel pushes back. Too high and the strongest moments clip — \
                the wheel stops telling you anything there.",
        default: 1.0,
        measure: Measure::Percent { max: 2.0 },
        signed: true,
        twin: None,
    },
    percent(
        File::Controls,
        "STEER",
        "FILTER_FF",
        "Smoothing",
        "Takes the edge off a grainy signal, and some of the detail with it.",
        0.0,
        1.0,
    ),
    percent(
        File::Controls,
        "FF_TWEAKS",
        "MIN_FF",
        "Minimum force",
        "Lifts the weakest forces over the point where a wheel's own friction swallows them. \
         Belt and gear wheels want some; a direct-drive wheel wants none.",
        0.05,
        0.3,
    ),
    percent(
        File::Controls,
        "FF_TWEAKS",
        "CENTER_BOOST_GAIN",
        "Centre boost",
        "Extra weight around the straight-ahead, where a wheel can feel vague.",
        0.0,
        1.0,
    ),
    percent(
        File::Controls,
        "FF_TWEAKS",
        "CENTER_BOOST_RANGE",
        "Centre boost range",
        "How far either side of centre that weight reaches.",
        0.1,
        1.0,
    ),
    percent(
        File::Controls,
        "FF_ENHANCEMENT",
        "CURBS",
        "Kerbs",
        "Kerbs added to the feedback.",
        0.4,
        1.0,
    ),
    percent(
        File::Controls,
        "FF_ENHANCEMENT",
        "ROAD",
        "Road",
        "Bumps and the road surface added to the feedback.",
        0.5,
        1.0,
    ),
    percent(
        File::Controls,
        "FF_ENHANCEMENT",
        "SLIPS",
        "Slip",
        "A vibration when the tyres slide.",
        0.0,
        1.0,
    ),
    percent(
        File::Controls,
        "FF_ENHANCEMENT",
        "ABS",
        "ABS",
        "The ABS pulsing through the wheel.",
        0.25,
        1.0,
    ),
    switch(
        File::Controls,
        "FF_ENHANCEMENT_2",
        "UNDERSTEER",
        "Enhanced understeer",
        "The wheel goes light sooner as the front gives up.",
        false,
    ),
    Level {
        file: File::Controls,
        section: "FF_SKIP_STEPS",
        key: "VALUE",
        name: "Skip steps",
        about: "Sends force feedback less often. Only for a wheel whose driver cannot keep up \
                and stutters; leave it at nought otherwise.",
        default: 0.0,
        measure: Measure::Whole {
            min: 0.0,
            max: 10.0,
            unit: "",
        },
        signed: false,
        twin: None,
    },
];

/// The wheel itself: `controls.ini`, `[STEER]`.
pub const STEERING: &[Level] = &[
    Level {
        file: File::Controls,
        section: "STEER",
        key: "LOCK",
        name: "Rotation",
        about: "Degrees lock to lock the game expects of the wheel. Match the wheel's own \
                setting; the game then turns the steering wheel in the car as far as yours.",
        default: 900.0,
        measure: Measure::Whole {
            min: 180.0,
            max: 1080.0,
            unit: "°",
        },
        signed: false,
        twin: Some("__CM_ORIGINAL_LOCK"),
    },
    Level {
        file: File::Controls,
        section: "STEER",
        key: "STEER_GAMMA",
        name: "Steering gamma",
        about: "Above 1, the first part of the wheel's travel does less and the rest more — \
                calmer on the straight, the same lock at the end.",
        default: 1.0,
        measure: Measure::Decimal { min: 0.5, max: 3.0 },
        signed: false,
        twin: None,
    },
    percent(
        File::Controls,
        "STEER",
        "STEER_FILTER",
        "Steering filter",
        "Smooths the steering input itself.",
        0.0,
        1.0,
    ),
    percent(
        File::Controls,
        "STEER",
        "SPEED_SENSITIVITY",
        "Speed sensitivity",
        "Less steering at speed. For a wheel, usually nought.",
        0.0,
        1.0,
    ),
];

/// The cockpit camera: `camera_onboard.ini`.
pub const CAMERA: &[Level] = &[
    Level {
        file: File::CameraOnboard,
        section: "MODE",
        key: "FOV",
        name: "Field of view",
        about: "Vertical degrees. Right for a monitor is small — the screen's height seen from \
                where you sit — and wider looks faster and judges distance worse.",
        default: 56.0,
        measure: Measure::Whole {
            min: 10.0,
            max: 120.0,
            unit: "°",
        },
        signed: false,
        twin: None,
    },
    switch(
        File::CameraOnboard,
        "MODE",
        "IS_WORLD_ALIGNED",
        "Horizon stays level",
        "The view stays level with the world instead of rolling with the car.",
        false,
    ),
    Level {
        file: File::CameraOnboard,
        section: "ROTATION",
        key: "HEAD_MAX_DEGREES",
        name: "Look into the corner",
        about: "How far the head turns towards the apex.",
        default: 60.0,
        measure: Measure::Whole {
            min: 0.0,
            max: 90.0,
            unit: "°",
        },
        signed: false,
        twin: None,
    },
];

/// The in-game display: `gameplay.ini`.
pub const DISPLAY: &[Level] = &[
    switch(
        File::Gameplay,
        "OPTIONS",
        "USE_MPH",
        "Miles per hour",
        "Speeds in the game in mph instead of km/h.",
        false,
    ),
    switch(
        File::Gameplay,
        "TIME_DIFFERENCE",
        "IS_ACTIVE",
        "Time gap",
        "The gap to the car ahead and behind.",
        true,
    ),
    switch(
        File::Gameplay,
        "DAMAGE_DISPLAYER",
        "IS_ACTIVE",
        "Damage",
        "The car's damage as it happens.",
        true,
    ),
    switch(
        File::Gameplay,
        "OVERLAY_LEADERBOARD",
        "ACTIVE",
        "Leaderboard",
        "The order in a race.",
        true,
    ),
    switch(
        File::Gameplay,
        "VIRTUAL_MIRROR",
        "ACTIVE",
        "Virtual mirror",
        "A mirror across the top of the screen.",
        true,
    ),
    switch(
        File::Gameplay,
        "DRIVER_NAME_DISPLAYER",
        "IS_ACTIVE",
        "Driver names",
        "Names over the other cars.",
        false,
    ),
    switch(
        File::Gameplay,
        "PYTHON",
        "ENABLE_PYTHON",
        "Python apps",
        "The apps the game runs on screen — sector times, maps, the ones you installed.",
        false,
    ),
];

/// Every table, with what its group is called — for whatever lists them.
pub const GROUPS: &[(&str, &[Level])] = &[
    ("SOUND", AUDIO),
    ("FORCE FEEDBACK", FORCE_FEEDBACK),
    ("THE WHEEL", STEERING),
    ("COCKPIT CAMERA", CAMERA),
    ("IN-GAME DISPLAY", DISPLAY),
];

/// Read a file, or nothing when it is not there yet — every setting then
/// shows the game's own default.
pub fn read(documents: &Path, file: File) -> Option<String> {
    std::fs::read_to_string(file.at(documents)).ok()
}

/// What a setting is, as the game would take it: the number in the file, or
/// the default when the key is missing or is not a number. The size only for
/// a signed setting; see [`reversed`].
pub fn value(text: &str, level: &Level) -> f32 {
    let said = ini::value(text, level.section, level.key)
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(level.default);
    match level.signed {
        true => said.abs(),
        false => said,
    }
}

/// Whether a signed setting is written negative — force feedback reversed.
pub fn reversed(text: &str, level: &Level) -> bool {
    level.signed
        && ini::value(text, level.section, level.key)
            .and_then(|v| v.parse::<f32>().ok())
            .is_some_and(|v| v < 0.0)
}

/// The range a slider for this setting covers, in what the file holds.
pub fn range(level: &Level) -> (f32, f32) {
    match level.measure {
        Measure::Percent { max } => (0.0, max),
        Measure::Whole { min, max, .. } | Measure::Decimal { min, max } => (min, max),
        Measure::Switch => (0.0, 1.0),
    }
}

/// How the number reads on screen.
pub fn shown(level: &Level, value: f32) -> String {
    match level.measure {
        Measure::Percent { .. } => format!("{:.0} %", value * 100.0),
        Measure::Whole { unit, .. } => format!("{:.0}{unit}", value),
        Measure::Decimal { .. } => format!("{value:.2}"),
        Measure::Switch => match value >= 0.5 {
            true => "on".to_string(),
            false => "off".to_string(),
        },
    }
}

/// How the number is written in the file: kept inside the setting's range,
/// whole where the game reads a whole number, and no trailing zeros.
pub fn written(level: &Level, value: f32) -> String {
    let (low, high) = range(level);
    let value = if value.is_finite() {
        value.clamp(low, high)
    } else {
        level.default
    };
    match level.measure {
        Measure::Whole { .. } => format!("{:.0}", value.round()),
        Measure::Switch => match value >= 0.5 {
            true => "1".to_string(),
            false => "0".to_string(),
        },
        Measure::Percent { .. } | Measure::Decimal { .. } => {
            let text = format!("{value:.3}");
            let text = text.trim_end_matches('0').trim_end_matches('.');
            match text {
                "" | "-" => "0".to_string(),
                _ => text.to_string(),
            }
        }
    }
}

/// Set one setting, every other line in its file left as it was. A signed
/// setting keeps the sign already in the file; a setting with a twin writes
/// it too where the file has it.
pub fn set(documents: &Path, level: &Level, value: f32) -> Result<PathBuf, String> {
    let at = level.file.at(documents);
    let before = std::fs::read_to_string(&at).unwrap_or_default();
    let mut text = written(level, value);
    if reversed(&before, level) && text != "0" {
        text = format!("-{text}");
    }
    let mut after = ini::with_value(&before, level.section, level.key, &text);
    if let Some(twin) = level.twin
        && ini::value(&before, level.section, twin).is_some()
    {
        after = ini::with_value(&after, level.section, twin, &text);
    }
    write(&at, &after)
}

/// Reverse a signed setting, or put it back, keeping its size.
pub fn set_reversed(
    documents: &Path,
    level: &Level,
    reversed_now: bool,
) -> Result<PathBuf, String> {
    let at = level.file.at(documents);
    let before = std::fs::read_to_string(&at).unwrap_or_default();
    let size = written(level, value(&before, level));
    let text = match reversed_now && size != "0" {
        true => format!("-{size}"),
        false => size,
    };
    write(
        &at,
        &ini::with_value(&before, level.section, level.key, &text),
    )
}

fn write(at: &Path, text: &str) -> Result<PathBuf, String> {
    if let Some(folder) = at.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    std::fs::write(at, text).map_err(|e| format!("{}: {e}", at.display()))?;
    Ok(at.to_path_buf())
}

// ── the bindings, read and kept safe ─────────────────────────────────────

/// The axes the game binds, each a section of `controls.ini`.
pub const AXES: &[(&str, &str)] = &[
    ("STEER", "steering"),
    ("THROTTLE", "throttle"),
    ("BRAKES", "brake"),
    ("CLUTCH", "clutch"),
    ("HANDBRAKE", "handbrake"),
];

/// Two controls bound to the same axis of the same device — a pedal that
/// brakes and declutches at once. Each pair is said by what the game calls
/// them, in the order of [`AXES`].
pub fn bound_twice(text: &str) -> Vec<(&'static str, &'static str)> {
    let bound: Vec<(&'static str, i64, i64)> = AXES
        .iter()
        .filter_map(|(section, name)| {
            let joy = ini::value(text, section, "JOY")?.parse::<i64>().ok()?;
            let axle = ini::value(text, section, "AXLE")?.parse::<i64>().ok()?;
            (joy >= 0 && axle >= 0).then_some((*name, joy, axle))
        })
        .collect();
    let mut twice = Vec::new();
    for (at, (name, joy, axle)) in bound.iter().enumerate() {
        for (other, other_joy, other_axle) in bound.iter().skip(at + 1) {
            if joy == other_joy && axle == other_axle {
                twice.push((*name, *other));
            }
        }
    }
    twice
}

/// Copy `controls.ini` aside before it is changed, into `into`, under a name
/// that sorts by when it was taken. Keeps the newest `keep` and removes the
/// rest. Nothing to copy when the game has not written the file yet.
pub fn back_up_controls(
    documents: &Path,
    into: &Path,
    stamp: u64,
    keep: usize,
) -> Result<Option<PathBuf>, String> {
    let from = File::Controls.at(documents);
    if !from.is_file() {
        return Ok(None);
    }
    std::fs::create_dir_all(into).map_err(|e| format!("{}: {e}", into.display()))?;
    let to = into.join(format!("controls-{stamp:012}.ini"));
    std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut kept = backups(into);
    while kept.len() > keep.max(1) {
        let oldest = kept.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
    Ok(Some(to))
}

/// The copies taken, oldest first.
pub fn backups(into: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(into)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("controls-") && n.ends_with(".ini"))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// Put a copy back as the game's `controls.ini`.
pub fn restore_controls(documents: &Path, copy: &Path) -> Result<PathBuf, String> {
    let to = File::Controls.at(documents);
    let text = std::fs::read_to_string(copy).map_err(|e| format!("{}: {e}", copy.display()))?;
    write(&to, &text)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("ac-levels-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(at.join("cfg")).unwrap();
        at
    }

    /// Every row is a key in a section, with a default inside its own range
    /// and a name and a reason to show.
    #[test]
    fn every_row_reads_and_its_default_is_inside_its_range() {
        for (group, levels) in GROUPS {
            assert!(!levels.is_empty(), "{group}");
            for level in *levels {
                let (low, high) = range(level);
                assert!(
                    (low..=high).contains(&level.default),
                    "{group}: {} default {} is outside {low}..{high}",
                    level.key,
                    level.default
                );
                assert!(!level.name.is_empty() && !level.about.is_empty());
                assert!(
                    level
                        .key
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_')
                );
            }
        }
    }

    /// No two rows write the same key in the same file.
    #[test]
    fn no_two_rows_share_a_key() {
        let all: Vec<&Level> = GROUPS.iter().flat_map(|(_, l)| l.iter()).collect();
        for (at, a) in all.iter().enumerate() {
            for b in all.iter().skip(at + 1) {
                assert!(
                    !(a.file == b.file && a.section == b.section && a.key == b.key),
                    "{} {} {} twice",
                    a.file.name(),
                    a.section,
                    a.key
                );
            }
        }
    }

    /// **What is written is what the game reads back** — the same section and
    /// key, in the number format the game parses, and the rest of the file
    /// untouched, comments and unknown keys included.
    #[test]
    fn a_setting_written_reads_back_and_leaves_the_rest_of_the_file_alone() {
        let documents = scratch("written");
        let at = File::Audio.at(&documents);
        std::fs::write(
            &at,
            "; written by the game\n[LEVELS]\nMASTER=1\nMUSIC=1 ; mine\n[SETTINGS]\nDRIVER_NAME=x\n",
        )
        .unwrap();
        let tyres = AUDIO.iter().find(|l| l.key == "TYRES").unwrap();
        set(&documents, tyres, 1.25).unwrap();
        let text = std::fs::read_to_string(&at).unwrap();
        assert_eq!(
            ini::value(&text, "LEVELS", "TYRES").as_deref(),
            Some("1.25")
        );
        assert!((value(&text, tyres) - 1.25).abs() < 1e-6);
        assert!(text.contains("; written by the game"));
        assert!(text.contains("MUSIC=1 ; mine"));
        assert_eq!(
            ini::value(&text, "SETTINGS", "DRIVER_NAME").as_deref(),
            Some("x")
        );
        std::fs::remove_dir_all(&documents).ok();
    }

    /// A key that is not in the file reads as the game's default, and an
    /// out-of-range value is kept inside the range rather than written.
    #[test]
    fn missing_keys_read_as_the_default_and_values_stay_in_range() {
        let master = AUDIO.iter().find(|l| l.key == "MASTER").unwrap();
        assert!((value("", master) - 1.0).abs() < 1e-6);
        assert!((value("[LEVELS]\nMASTER=nonsense\n", master) - 1.0).abs() < 1e-6);
        assert_eq!(written(master, 9.0), "2");
        assert_eq!(written(master, -1.0), "0");
        assert_eq!(written(master, 0.5), "0.5");
        assert_eq!(written(master, f32::NAN), "1");
        let lock = STEERING.iter().find(|l| l.key == "LOCK").unwrap();
        assert_eq!(written(lock, 540.4), "540");
        assert_eq!(shown(lock, 540.0), "540°");
        let under = FORCE_FEEDBACK
            .iter()
            .find(|l| l.key == "UNDERSTEER")
            .unwrap();
        assert_eq!(written(under, 1.0), "1");
        assert_eq!(shown(under, 0.0), "off");
    }

    /// **Reversed force feedback stays reversed** when its strength changes,
    /// and can be switched on its own.
    #[test]
    fn force_feedback_keeps_its_direction() {
        let documents = scratch("signed");
        let gain = FORCE_FEEDBACK.iter().find(|l| l.key == "FF_GAIN").unwrap();
        std::fs::write(
            File::Controls.at(&documents),
            "[STEER]\nFF_GAIN=-0.8\nJOY=0\nAXLE=0\n",
        )
        .unwrap();
        let text = read(&documents, File::Controls).unwrap();
        assert!(reversed(&text, gain));
        assert!((value(&text, gain) - 0.8).abs() < 1e-6);
        set(&documents, gain, 0.6).unwrap();
        let text = read(&documents, File::Controls).unwrap();
        assert_eq!(
            ini::value(&text, "STEER", "FF_GAIN").as_deref(),
            Some("-0.6")
        );
        set_reversed(&documents, gain, false).unwrap();
        let text = read(&documents, File::Controls).unwrap();
        assert_eq!(
            ini::value(&text, "STEER", "FF_GAIN").as_deref(),
            Some("0.6")
        );
        // The binding beside it is untouched.
        assert_eq!(ini::value(&text, "STEER", "JOY").as_deref(), Some("0"));
        assert_eq!(ini::value(&text, "STEER", "AXLE").as_deref(), Some("0"));
        std::fs::remove_dir_all(&documents).ok();
    }

    /// The steering lock is written to Content Manager's copy as well where
    /// that copy exists, so Content Manager does not put its own back.
    #[test]
    fn the_lock_is_written_to_content_managers_copy_too() {
        let documents = scratch("twin");
        let lock = STEERING.iter().find(|l| l.key == "LOCK").unwrap();
        std::fs::write(
            File::Controls.at(&documents),
            "[STEER]\nLOCK=540\n__CM_ORIGINAL_LOCK=900\n",
        )
        .unwrap();
        set(&documents, lock, 720.0).unwrap();
        let text = read(&documents, File::Controls).unwrap();
        assert_eq!(ini::value(&text, "STEER", "LOCK").as_deref(), Some("720"));
        assert_eq!(
            ini::value(&text, "STEER", "__CM_ORIGINAL_LOCK").as_deref(),
            Some("720")
        );
        // And a file without it does not grow one.
        std::fs::write(File::Controls.at(&documents), "[STEER]\nLOCK=540\n").unwrap();
        set(&documents, lock, 720.0).unwrap();
        let text = read(&documents, File::Controls).unwrap();
        assert_eq!(ini::value(&text, "STEER", "__CM_ORIGINAL_LOCK"), None);
        std::fs::remove_dir_all(&documents).ok();
    }

    /// A pedal bound to the same axis as another is found, and an unbound
    /// axis (`-1`) is not a clash.
    #[test]
    fn an_axis_bound_twice_is_found() {
        let text = "[STEER]\nJOY=0\nAXLE=0\n[THROTTLE]\nJOY=1\nAXLE=1\n[BRAKES]\nJOY=1\nAXLE=2\n\
                    [CLUTCH]\nJOY=1\nAXLE=2\n[HANDBRAKE]\nJOY=-1\nAXLE=-1\n";
        assert_eq!(bound_twice(text), vec![("brake", "clutch")]);
        assert!(bound_twice("[CLUTCH]\nJOY=-1\nAXLE=-1\n[BRAKES]\nJOY=-1\nAXLE=-1\n").is_empty());
    }

    /// A copy is taken before a change, the oldest go past the limit, and a
    /// copy put back is the file as it was.
    #[test]
    fn controls_are_backed_up_and_put_back() {
        let documents = scratch("backup");
        let into = documents.join("kept");
        assert_eq!(back_up_controls(&documents, &into, 1, 3).unwrap(), None);
        std::fs::write(File::Controls.at(&documents), "[STEER]\nLOCK=900\n").unwrap();
        for stamp in 1..=5 {
            back_up_controls(&documents, &into, stamp, 3).unwrap();
        }
        let kept = backups(&into);
        assert_eq!(kept.len(), 3);
        assert!(kept.first().unwrap().ends_with("controls-000000000003.ini"));
        let lock = STEERING.iter().find(|l| l.key == "LOCK").unwrap();
        set(&documents, lock, 540.0).unwrap();
        restore_controls(&documents, kept.last().unwrap()).unwrap();
        let text = read(&documents, File::Controls).unwrap();
        assert_eq!(ini::value(&text, "STEER", "LOCK").as_deref(), Some("900"));
        std::fs::remove_dir_all(&documents).ok();
    }
}
