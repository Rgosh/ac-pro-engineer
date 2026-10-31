//! The game's driving aids, read and set without starting it.
//!
//! **One file, `cfg/assists.ini` in the game's Documents folder**, read by
//! the game at the start of every offline session. Content Manager puts
//! these on its quick-drive screen; a launcher built on this core can too.
//!
//! The rows and presets use the same shapes as [`video`](super::video):
//! a row is a key the game reads with the values it offers, and a value
//! nobody offered is shown as written and left alone.

use super::ini;
use super::video::{Kind, Preset, Setting};
use std::path::{Path, PathBuf};

/// The aids, in the order the game's own screen gives them.
pub const SETTINGS: &[Setting] = &[
    Setting {
        section: "ASSISTS",
        key: "ABS",
        name: "ABS",
        about: "Wheels that do not lock under braking.",
        kind: Kind::Choice(&[("0", "Off"), ("1", "Factory"), ("2", "On")]),
    },
    Setting {
        section: "ASSISTS",
        key: "TRACTION_CONTROL",
        name: "Traction control",
        about: "Wheels that do not spin on the throttle.",
        kind: Kind::Choice(&[("0", "Off"), ("1", "Factory"), ("2", "On")]),
    },
    Setting {
        section: "ASSISTS",
        key: "STABILITY_CONTROL",
        name: "Stability control",
        about: "The car caught before it spins.",
        kind: Kind::Choice(&[("0", "Off"), ("25", "25%"), ("50", "50%"), ("100", "100%")]),
    },
    Setting {
        section: "ASSISTS",
        key: "AUTO_CLUTCH",
        name: "Automatic clutch",
        about: "No clutch pedal needed to pull away.",
        kind: Kind::Switch,
    },
    Setting {
        section: "ASSISTS",
        key: "AUTO_BLIP",
        name: "Automatic throttle blip",
        about: "Downshifts that do not upset the car.",
        kind: Kind::Switch,
    },
    Setting {
        section: "ASSISTS",
        key: "AUTO_SHIFTER",
        name: "Automatic gearbox",
        about: "The game changes gear for you.",
        kind: Kind::Switch,
    },
    Setting {
        section: "ASSISTS",
        key: "IDEAL_LINE",
        name: "Racing line",
        about: "The line drawn on the road, with braking points.",
        kind: Kind::Switch,
    },
    Setting {
        section: "ASSISTS",
        key: "DAMAGE",
        name: "Damage",
        about: "How much a crash hurts the car.",
        kind: Kind::Choice(&[("0", "Off"), ("50", "50%"), ("100", "Real")]),
    },
    Setting {
        section: "ASSISTS",
        key: "TYRE_WEAR",
        name: "Tyre wear",
        about: "Tyres that wear as they would.",
        kind: Kind::Choice(&[("0", "Off"), ("1", "Real")]),
    },
    Setting {
        section: "ASSISTS",
        key: "FUEL_RATE",
        name: "Fuel use",
        about: "Fuel that runs down as it would.",
        kind: Kind::Choice(&[("0", "Off"), ("1", "Real")]),
    },
    Setting {
        section: "ASSISTS",
        key: "TYRE_BLANKETS",
        name: "Tyre blankets",
        about: "Tyres warm when you leave the pits.",
        kind: Kind::Switch,
    },
];

/// Three ways to drive, from the car as built to every aid there is.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Pro",
        about: "The car as it was built, and everything real.",
        values: &[
            ("ASSISTS", "ABS", "1"),
            ("ASSISTS", "TRACTION_CONTROL", "1"),
            ("ASSISTS", "STABILITY_CONTROL", "0"),
            ("ASSISTS", "AUTO_CLUTCH", "0"),
            ("ASSISTS", "AUTO_BLIP", "0"),
            ("ASSISTS", "AUTO_SHIFTER", "0"),
            ("ASSISTS", "IDEAL_LINE", "0"),
            ("ASSISTS", "DAMAGE", "100"),
            ("ASSISTS", "TYRE_WEAR", "1"),
            ("ASSISTS", "FUEL_RATE", "1"),
        ],
    },
    Preset {
        name: "Club",
        about: "The car's own aids, and the fiddly parts done for you.",
        values: &[
            ("ASSISTS", "ABS", "1"),
            ("ASSISTS", "TRACTION_CONTROL", "1"),
            ("ASSISTS", "STABILITY_CONTROL", "0"),
            ("ASSISTS", "AUTO_CLUTCH", "1"),
            ("ASSISTS", "AUTO_BLIP", "1"),
            ("ASSISTS", "AUTO_SHIFTER", "0"),
            ("ASSISTS", "IDEAL_LINE", "0"),
            ("ASSISTS", "DAMAGE", "50"),
            ("ASSISTS", "TYRE_WEAR", "1"),
            ("ASSISTS", "FUEL_RATE", "1"),
        ],
    },
    Preset {
        name: "Learning",
        about: "Every aid on, and the line to follow.",
        values: &[
            ("ASSISTS", "ABS", "2"),
            ("ASSISTS", "TRACTION_CONTROL", "2"),
            ("ASSISTS", "STABILITY_CONTROL", "50"),
            ("ASSISTS", "AUTO_CLUTCH", "1"),
            ("ASSISTS", "AUTO_BLIP", "1"),
            ("ASSISTS", "AUTO_SHIFTER", "1"),
            ("ASSISTS", "IDEAL_LINE", "1"),
            ("ASSISTS", "DAMAGE", "0"),
            ("ASSISTS", "TYRE_WEAR", "0"),
            ("ASSISTS", "FUEL_RATE", "0"),
        ],
    },
];

/// Where the file is, under the game's own Documents folder.
pub fn file(documents: &Path) -> PathBuf {
    documents.join("cfg").join("assists.ini")
}

/// Read the file, or nothing when it is not there yet.
pub fn read(documents: &Path) -> Option<String> {
    std::fs::read_to_string(file(documents)).ok()
}

/// Set one aid, every other line left as it was.
pub fn set(documents: &Path, setting: &Setting, value: &str) -> Result<PathBuf, String> {
    ini::set_in_file(&file(documents), setting.section, setting.key, value)
}

/// Apply a preset, every other line left as it was.
pub fn apply(documents: &Path, preset: &Preset) -> Result<PathBuf, String> {
    let mut at = file(documents);
    for (section, key, value) in preset.values {
        at = ini::set_in_file(&file(documents), section, key, value)?;
    }
    Ok(at)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::games::assetto_corsa::video::{is_preset, same_value};

    #[test]
    fn every_preset_sets_only_rows_of_the_table_to_values_they_offer() {
        for preset in PRESETS {
            for (section, key, value) in preset.values {
                let row = SETTINGS
                    .iter()
                    .find(|s| s.section == *section && s.key == *key);
                assert!(row.is_some(), "{} sets {section}/{key}", preset.name);
                let Some(row) = row else { continue };
                match row.kind {
                    Kind::Switch => assert!(["0", "1"].contains(value)),
                    Kind::Choice(options) => assert!(
                        options.iter().any(|(v, _)| same_value(v, value)),
                        "{} sets {key} to {value}, which the game does not offer",
                        preset.name
                    ),
                }
            }
        }
    }

    #[test]
    fn the_presets_are_told_apart_and_applying_one_makes_the_file_it() {
        let docs = std::env::temp_dir().join(format!("acpe-assists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&docs);
        std::fs::create_dir_all(docs.join("cfg")).unwrap();
        std::fs::write(file(&docs), "[ASSISTS]\nSLIPSTREAM=1\n").unwrap();
        for preset in PRESETS {
            apply(&docs, preset).unwrap();
            let text = read(&docs).unwrap();
            let matching: Vec<&str> = PRESETS
                .iter()
                .filter(|p| is_preset(&text, p))
                .map(|p| p.name)
                .collect();
            assert_eq!(matching, vec![preset.name]);
            assert!(text.contains("SLIPSTREAM=1"), "an unknown key was lost");
        }
        let _ = std::fs::remove_dir_all(&docs);
    }
}
