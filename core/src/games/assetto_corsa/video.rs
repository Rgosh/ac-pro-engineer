//! The game's video settings, read and set without starting it.
//!
//! **Content Manager's most used page.** The game's own options screen is
//! inside the game: changing the resolution means starting it, changing it,
//! and starting it again. The settings are one file, `cfg/video.ini` in the
//! game's Documents folder, and a launcher can show and change them before
//! the game is ever loaded.
//!
//! **Only the settings this table names, and only the values it lists.** Each
//! row is a key the game is known to read, with the values the game's own
//! screen offers. A value in the file that is not among them — somebody's
//! hand edit, a mod's preset — is shown as it is and left alone until the
//! driver chooses another, never "corrected".

use super::ini;
use std::path::{Path, PathBuf};

/// What kind of answer a setting takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// On or off, written as `1` or `0`.
    Switch,
    /// One of a list: what is written, and what it is called.
    Choice(&'static [(&'static str, &'static str)]),
}

/// One setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting {
    pub section: &'static str,
    pub key: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    pub kind: Kind,
}

/// The settings, in the order the game's own screen gives them.
pub const SETTINGS: &[Setting] = &[
    Setting {
        section: "VIDEO",
        key: "FULLSCREEN",
        name: "Full screen",
        about: "The whole screen, or a window.",
        kind: Kind::Switch,
    },
    Setting {
        section: "VIDEO",
        key: "VSYNC",
        name: "Vertical sync",
        about: "No tearing, at the cost of a frame of delay.",
        kind: Kind::Switch,
    },
    Setting {
        section: "VIDEO",
        key: "FPS_CAP_MS",
        name: "Frame limit",
        about: "The most frames a second the game will draw.",
        kind: Kind::Choice(&[
            ("0", "None"),
            ("16.6667", "60"),
            ("11.1111", "90"),
            ("8.33333", "120"),
            ("6.94444", "144"),
            ("6.06061", "165"),
        ]),
    },
    Setting {
        section: "VIDEO",
        key: "AASAMPLES",
        name: "Anti-aliasing",
        about: "Smoother edges, for a share of the frame rate.",
        kind: Kind::Choice(&[("1", "Off"), ("2", "2×"), ("4", "4×"), ("8", "8×")]),
    },
    Setting {
        section: "VIDEO",
        key: "ANISOTROPIC",
        name: "Texture filtering",
        about: "Sharper tarmac into the distance.",
        kind: Kind::Choice(&[
            ("0", "Off"),
            ("2", "2×"),
            ("4", "4×"),
            ("8", "8×"),
            ("16", "16×"),
        ]),
    },
    Setting {
        section: "VIDEO",
        key: "SHADOW_MAP_SIZE",
        name: "Shadows",
        about: "How fine the shadows are.",
        kind: Kind::Choice(&[
            ("512", "Low"),
            ("1024", "Medium"),
            ("2048", "High"),
            ("4096", "Ultra"),
        ]),
    },
    Setting {
        section: "ASSETTOCORSA",
        key: "WORLD_DETAIL",
        name: "World detail",
        about: "How much of the scenery is drawn.",
        kind: Kind::Choice(&[
            ("0", "Min"),
            ("1", "Low"),
            ("2", "Medium"),
            ("3", "High"),
            ("4", "Very high"),
            ("5", "Max"),
        ]),
    },
    Setting {
        section: "POST_PROCESS",
        key: "ENABLED",
        name: "Post-processing",
        about: "Bloom, glare and the colour filter.",
        kind: Kind::Switch,
    },
    Setting {
        section: "MIRROR",
        key: "HQ",
        name: "Detailed mirrors",
        about: "Mirrors drawn at full detail.",
        kind: Kind::Switch,
    },
    Setting {
        section: "MIRROR",
        key: "SIZE",
        name: "Mirror resolution",
        about: "How sharp the mirrors are. Off saves the most frames on a busy grid.",
        kind: Kind::Choice(&[
            ("0", "Off"),
            ("256", "64×256"),
            ("512", "128×512"),
            ("1024", "256×1024"),
            ("2048", "512×2048"),
        ]),
    },
    Setting {
        section: "CUBEMAP",
        key: "SIZE",
        name: "Reflections",
        about: "How sharp the world reflected in the paint and the glass is.",
        kind: Kind::Choice(&[
            ("0", "Off"),
            ("256", "256"),
            ("512", "512"),
            ("1024", "1024"),
            ("2048", "2048"),
        ]),
    },
    Setting {
        section: "CUBEMAP",
        key: "FACES_PER_FRAME",
        name: "Reflection rate",
        about: "Faces of the reflection redrawn each frame: six is all of them, static \
                draws it once.",
        kind: Kind::Choice(&[
            ("0", "Static"),
            ("1", "1"),
            ("2", "2"),
            ("3", "3"),
            ("4", "4"),
            ("5", "5"),
            ("6", "6"),
        ]),
    },
    Setting {
        section: "EFFECTS",
        key: "SMOKE",
        name: "Smoke",
        about: "Tyre smoke and dust.",
        kind: Kind::Choice(&[
            ("0", "Off"),
            ("1", "Min"),
            ("2", "Low"),
            ("3", "Medium"),
            ("4", "High"),
            ("5", "Max"),
        ]),
    },
    Setting {
        section: "EFFECTS",
        key: "RENDER_SMOKE_IN_MIRROR",
        name: "Smoke in the mirrors",
        about: "Smoke drawn in the mirrors as well.",
        kind: Kind::Switch,
    },
    Setting {
        section: "EFFECTS",
        key: "MOTION_BLUR",
        name: "Motion blur",
        about: "Blur at speed. Off is the sharpest picture; it costs a frame or two at high \
                settings.",
        kind: Kind::Choice(&[
            ("0", "Off"),
            ("2", "2"),
            ("4", "4"),
            ("6", "6"),
            ("8", "8"),
            ("10", "10"),
            ("12", "12"),
        ]),
    },
    Setting {
        section: "POST_PROCESS",
        key: "QUALITY",
        name: "Post-processing quality",
        about: "How finely the filter, bloom and glare are drawn.",
        kind: Kind::Choice(&[
            ("0", "Min"),
            ("1", "Low"),
            ("2", "Medium"),
            ("3", "High"),
            ("4", "Very high"),
            ("5", "Max"),
        ]),
    },
    Setting {
        section: "POST_PROCESS",
        key: "GLARE",
        name: "Glare",
        about: "Bright light blooming over the picture.",
        kind: Kind::Choice(&[
            ("0", "Off"),
            ("1", "Low"),
            ("2", "Medium"),
            ("3", "High"),
            ("4", "Very high"),
            ("5", "Max"),
        ]),
    },
    Setting {
        section: "POST_PROCESS",
        key: "DOF",
        name: "Depth of field",
        about: "The distance softened where the eye would not be focused.",
        kind: Kind::Choice(&[
            ("0", "Off"),
            ("1", "Low"),
            ("2", "Medium"),
            ("3", "High"),
            ("4", "Very high"),
            ("5", "Max"),
        ]),
    },
    Setting {
        section: "POST_PROCESS",
        key: "RAYS_OF_GOD",
        name: "Sun rays",
        about: "Light shafts from the sun.",
        kind: Kind::Switch,
    },
    Setting {
        section: "POST_PROCESS",
        key: "HEAT_SHIMMER",
        name: "Heat shimmer",
        about: "Hot air wobbling over the track and behind the exhausts.",
        kind: Kind::Switch,
    },
    Setting {
        section: "POST_PROCESS",
        key: "FXAA",
        name: "FXAA",
        about: "A cheap smoothing of edges on top of anti-aliasing.",
        kind: Kind::Switch,
    },
];

/// The colour filters the game offers: `system/cfg/ppfilters/*.ini` in the
/// install, by the name the game writes as `[POST_PROCESS] FILTER`. Sorted,
/// with `default` first when it is there, which is what the game uses when
/// none is set.
pub fn filters(root: &Path) -> Vec<String> {
    let folder = root.join("system").join("cfg").join("ppfilters");
    let mut found: Vec<String> = std::fs::read_dir(&folder)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    let is_ini = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("ini"));
                    if !is_ini {
                        return None;
                    }
                    path.file_stem()
                        .and_then(|n| n.to_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort_by_key(|name| (name != "default", name.to_ascii_lowercase()));
    found
}

/// The filter the file names, or the game's default when it names none.
pub fn filter(text: &str) -> String {
    ini::value(text, "POST_PROCESS", "FILTER")
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

/// Choose a filter, every other line left as it was.
pub fn set_filter(documents: &Path, name: &str) -> Result<PathBuf, String> {
    set_raw(documents, "POST_PROCESS", "FILTER", name)
}

/// A whole look in one press: a name, a line about it, and the value it
/// gives each setting it touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    pub about: &'static str,
    /// `(section, key, value)`; every one is a row of [`SETTINGS`] and one of
    /// the values that row offers — a test holds both.
    pub values: &'static [(&'static str, &'static str, &'static str)],
}

/// Three looks, from the frame rate first to the picture first. Full screen,
/// vsync and the frame limit are left alone: they are about the screen, not
/// the picture.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Performance",
        about: "Every frame the machine can give.",
        values: &[
            ("VIDEO", "AASAMPLES", "1"),
            ("VIDEO", "ANISOTROPIC", "4"),
            ("VIDEO", "SHADOW_MAP_SIZE", "1024"),
            ("ASSETTOCORSA", "WORLD_DETAIL", "3"),
            ("POST_PROCESS", "ENABLED", "0"),
            ("MIRROR", "HQ", "0"),
        ],
    },
    Preset {
        name: "Balanced",
        about: "A good picture, and a steady frame rate.",
        values: &[
            ("VIDEO", "AASAMPLES", "2"),
            ("VIDEO", "ANISOTROPIC", "8"),
            ("VIDEO", "SHADOW_MAP_SIZE", "2048"),
            ("ASSETTOCORSA", "WORLD_DETAIL", "4"),
            ("POST_PROCESS", "ENABLED", "1"),
            ("MIRROR", "HQ", "0"),
        ],
    },
    Preset {
        name: "Quality",
        about: "The best the game can look.",
        values: &[
            ("VIDEO", "AASAMPLES", "4"),
            ("VIDEO", "ANISOTROPIC", "16"),
            ("VIDEO", "SHADOW_MAP_SIZE", "4096"),
            ("ASSETTOCORSA", "WORLD_DETAIL", "5"),
            ("POST_PROCESS", "ENABLED", "1"),
            ("MIRROR", "HQ", "1"),
        ],
    },
];

/// Whether the file's text is exactly a preset.
pub fn is_preset(text: &str, preset: &Preset) -> bool {
    preset.values.iter().all(|(section, key, value)| {
        ini::value(text, section, key).is_some_and(|now| same_value(&now, value))
    })
}

/// Apply a preset, every other line left as it was.
pub fn apply(documents: &Path, preset: &Preset) -> Result<PathBuf, String> {
    let mut at = file(documents);
    for (section, key, value) in preset.values {
        at = set_raw(documents, section, key, value)?;
    }
    Ok(at)
}

/// Where the file is, under the game's own Documents folder.
pub fn file(documents: &Path) -> PathBuf {
    documents.join("cfg").join("video.ini")
}

/// The resolution the game is set to, when the file says.
pub fn resolution(text: &str) -> Option<(u32, u32)> {
    let width = ini::value(text, "VIDEO", "WIDTH")?.parse().ok()?;
    let height = ini::value(text, "VIDEO", "HEIGHT")?.parse().ok()?;
    Some((width, height))
}

/// What a setting is set to in the file's text: the raw value, when it is
/// there.
pub fn current(text: &str, setting: &Setting) -> Option<String> {
    ini::value(text, setting.section, setting.key)
}

/// What to call a raw value of a setting: its label when it is one the game
/// offers, the value itself when it is not.
pub fn label_of(setting: &Setting, raw: &str) -> String {
    match setting.kind {
        Kind::Switch => match raw.trim() {
            "1" => "On".to_string(),
            "0" => "Off".to_string(),
            other => other.to_string(),
        },
        Kind::Choice(options) => options
            .iter()
            .find(|(value, _)| same_value(value, raw))
            .map(|(_, label)| (*label).to_string())
            .unwrap_or_else(|| raw.trim().to_string()),
    }
}

/// Whether two values are the same setting: as numbers when both are, so
/// `16.666700` written by the game is the `16.6667` the list offers.
pub fn same_value(a: &str, b: &str) -> bool {
    match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        (Ok(x), Ok(y)) => (x - y).abs() < 0.01,
        _ => a.trim().eq_ignore_ascii_case(b.trim()),
    }
}

/// Read the file, or nothing when it is not there yet.
pub fn read(documents: &Path) -> Option<String> {
    std::fs::read_to_string(file(documents)).ok()
}

/// Set one setting in the file, every other line left as it was.
pub fn set(documents: &Path, setting: &Setting, value: &str) -> Result<PathBuf, String> {
    set_raw(documents, setting.section, setting.key, value)
}

/// Set the resolution.
pub fn set_resolution(documents: &Path, width: u32, height: u32) -> Result<PathBuf, String> {
    set_raw(documents, "VIDEO", "WIDTH", &width.to_string())?;
    set_raw(documents, "VIDEO", "HEIGHT", &height.to_string())
}

fn set_raw(documents: &Path, section: &str, key: &str, value: &str) -> Result<PathBuf, String> {
    ini::set_in_file(&file(documents), section, key, value)
}

/// Common resolutions, for the picker; whatever the file says is offered
/// beside them when it is not one of these.
pub const RESOLUTIONS: &[(u32, u32)] = &[
    (1280, 720),
    (1600, 900),
    (1920, 1080),
    (2560, 1080),
    (2560, 1440),
    (3440, 1440),
    (3840, 2160),
    (5760, 1080),
];

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    const GAME_WROTE: &str = "\
[VIDEO]
FULLSCREEN=1
WIDTH=2560
HEIGHT=1440
VSYNC=0
AASAMPLES=4
ANISOTROPIC=16
SHADOW_MAP_SIZE=2048
FPS_CAP_MS=16.666700

[ASSETTOCORSA]
WORLD_DETAIL=5
";

    /// The filters are the install's own files, `default` first.
    #[test]
    fn the_filters_are_read_from_the_install() {
        let root = std::env::temp_dir().join(format!("ac-ppfilters-{}", std::process::id()));
        let folder = root.join("system").join("cfg").join("ppfilters");
        std::fs::create_dir_all(&folder).unwrap();
        for name in ["natural.ini", "default.ini", "Bright.INI", "notes.txt"] {
            std::fs::write(folder.join(name), "").unwrap();
        }
        assert_eq!(filters(&root), vec!["default", "Bright", "natural"]);
        assert_eq!(filter(""), "default");
        assert_eq!(filter("[POST_PROCESS]\nFILTER=natural\n"), "natural");
        std::fs::remove_dir_all(&root).ok();
    }

    fn setting(key: &str) -> &'static Setting {
        SETTINGS.iter().find(|s| s.key == key).unwrap()
    }

    #[test]
    fn what_the_game_wrote_reads_as_the_names_its_screen_uses() {
        assert_eq!(resolution(GAME_WROTE), Some((2560, 1440)));
        let cap = current(GAME_WROTE, setting("FPS_CAP_MS")).unwrap();
        assert_eq!(label_of(setting("FPS_CAP_MS"), &cap), "60");
        let shadows = current(GAME_WROTE, setting("SHADOW_MAP_SIZE")).unwrap();
        assert_eq!(label_of(setting("SHADOW_MAP_SIZE"), &shadows), "High");
        assert_eq!(label_of(setting("FULLSCREEN"), "1"), "On");
        assert_eq!(current(GAME_WROTE, setting("HQ")), None, "not stated");
    }

    #[test]
    fn a_value_nobody_offered_is_shown_as_it_is() {
        assert_eq!(label_of(setting("SHADOW_MAP_SIZE"), "3000"), "3000");
    }

    #[test]
    fn setting_one_leaves_the_rest_of_the_file_alone() {
        let docs = std::env::temp_dir().join(format!("acpe-video-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&docs);
        std::fs::create_dir_all(docs.join("cfg")).unwrap();
        std::fs::write(file(&docs), GAME_WROTE).unwrap();

        set(&docs, setting("VSYNC"), "1").unwrap();
        set_resolution(&docs, 1920, 1080).unwrap();
        let after = read(&docs).unwrap();
        assert_eq!(current(&after, setting("VSYNC")).as_deref(), Some("1"));
        assert_eq!(resolution(&after), Some((1920, 1080)));
        assert_eq!(
            current(&after, setting("WORLD_DETAIL")).as_deref(),
            Some("5")
        );
        assert_eq!(after.lines().count(), GAME_WROTE.lines().count());
        let _ = std::fs::remove_dir_all(&docs);
    }

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
    fn applying_a_preset_makes_the_file_that_preset_and_keeps_the_rest() {
        let docs = std::env::temp_dir().join(format!("acpe-preset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&docs);
        std::fs::create_dir_all(docs.join("cfg")).unwrap();
        std::fs::write(file(&docs), GAME_WROTE).unwrap();
        let quality = PRESETS.iter().find(|p| p.name == "Quality").unwrap();
        assert!(!is_preset(GAME_WROTE, quality));
        apply(&docs, quality).unwrap();
        let after = read(&docs).unwrap();
        assert!(is_preset(&after, quality));
        assert!(PRESETS.iter().filter(|p| is_preset(&after, p)).count() == 1);
        assert_eq!(resolution(&after), Some((2560, 1440)));
        let _ = std::fs::remove_dir_all(&docs);
    }

    #[test]
    fn every_choice_offers_distinct_values() {
        for one in SETTINGS {
            if let Kind::Choice(options) = one.kind {
                for (k, (a, _)) in options.iter().enumerate() {
                    for (b, _) in options.iter().skip(k + 1) {
                        assert!(!same_value(a, b), "{} offers {a} twice", one.key);
                    }
                }
            }
        }
    }
}
