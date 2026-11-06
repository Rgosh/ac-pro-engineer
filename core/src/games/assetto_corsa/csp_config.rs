//! Every setting of Custom Shaders Patch, as the patch itself describes it.
//!
//! **The patch documents its own settings, in its own files.** Each module
//! is an ini under `extension/config/` with an `[ABOUT]` saying what it is,
//! and most keys carry a comment after the value — the label, sometimes a
//! description in brackets, sometimes the range a value may take ("from 0 to
//! 2", "round to 0.05", "perc"). Content Manager builds its settings pages
//! from those comments; so does this. Nothing here knows what any key means:
//! it says what the patch says about it, which is why a patch update that
//! adds a key adds a setting here with no change to this file.
//!
//! **Two files per module, and the driver's wins** — the patch's defaults in
//! the game folder, the driver's own choices in `cfg/extension/` in the
//! game's Documents folder, as [`csp_settings`](super::csp_settings) explains.
//! Only the key that was changed is written there.

use std::path::{Path, PathBuf};

/// One module of the patch.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleFile {
    /// Its file name, `weather_fx.ini`.
    pub file: String,
    /// `[ABOUT] NAME`, or the file name made readable.
    pub name: String,
    /// `[ABOUT] DESCRIPTION`, when it has one.
    pub about: String,
}

/// How a value is changed.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// On or off.
    Switch,
    /// A number, inside a range when the patch gave one, stepped when it
    /// said to round, shown as a percentage when it said so.
    Number {
        range: Option<(f64, f64)>,
        step: Option<f64>,
        percent: bool,
    },
    /// Anything else — a colour, a list, a name — as text.
    Text,
}

/// One setting.
#[derive(Debug, Clone, PartialEq)]
pub struct Setting {
    pub section: String,
    pub key: String,
    /// What the patch calls it, or the key made readable.
    pub label: String,
    /// The bracketed part of its comment, when there is one.
    pub about: String,
    pub kind: Kind,
    /// The patch's own value.
    pub default: String,
    /// The driver's, when they changed it.
    pub mine: Option<String>,
}

impl Setting {
    /// What is in force: the driver's value, else the patch's.
    pub fn value(&self) -> &str {
        self.mine.as_deref().unwrap_or(&self.default)
    }
}

/// A titled group of settings: one `[SECTION]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub name: String,
    pub label: String,
    pub settings: Vec<Setting>,
}

/// The patch's modules in a game folder, by name.
pub fn modules(root: &Path) -> Vec<ModuleFile> {
    let folder = root.join("extension").join("config");
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut found: Vec<ModuleFile> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().into_owned();
            if !file.to_ascii_lowercase().ends_with(".ini") {
                return None;
            }
            let text = std::fs::read_to_string(entry.path()).ok()?;
            // **A module says what it is or has a switch**; the patch's other
            // files there (its data manifest, lists) are not settings.
            let upper = text.to_ascii_uppercase();
            if !upper.contains("[ABOUT]") && !upper.contains("[BASIC]") {
                return None;
            }
            let about = |key: &str| super::ini::value(&text, "ABOUT", key).unwrap_or_default();
            let name = Some(about("NAME"))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| readable(file.trim_end_matches(".ini")));
            Some(ModuleFile {
                about: about("DESCRIPTION"),
                name,
                file,
            })
        })
        .collect();
    found.sort_by_key(|m| m.name.to_lowercase());
    found
}

/// Where the driver's own choices for a module are written.
pub fn user_file(documents: &Path, file: &str) -> PathBuf {
    documents.join("cfg").join("extension").join(file)
}

/// Every setting of one module: the patch's file read for what there is and
/// what it says about each, the driver's file for what they changed.
pub fn read(root: &Path, documents: Option<&Path>, file: &str) -> Vec<Section> {
    let defaults = std::fs::read_to_string(root.join("extension").join("config").join(file))
        .unwrap_or_default();
    let mine = documents
        .and_then(|docs| std::fs::read_to_string(user_file(docs, file)).ok())
        .unwrap_or_default();
    sections(&defaults, &mine)
}

/// The same, from the two files' text — split out to be tested.
pub fn sections(defaults: &str, mine: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for line in defaults.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let (body, comment) = match line.split_once(';') {
            Some((body, comment)) => (body.trim(), comment.trim()),
            None => (line, ""),
        };
        if let Some(name) = body.strip_prefix('[').and_then(|b| b.strip_suffix(']')) {
            let name = name.trim().to_string();
            let label = match comment.is_empty() {
                true => readable(&name),
                false => comment.to_string(),
            };
            out.push(Section {
                name,
                label,
                settings: Vec::new(),
            });
            continue;
        }
        let Some((key, value)) = body.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let Some(section) = out.last_mut() else {
            continue;
        };
        // What the patch keeps for itself: its own description, keys marked
        // as internal with a leading underscore.
        if section.name.eq_ignore_ascii_case("ABOUT") || key.starts_with('_') {
            continue;
        }
        let (label, about, kind) = described(key, value, comment);
        let theirs = super::ini::value(mine, &section.name, key);
        section.settings.push(Setting {
            section: section.name.clone(),
            key: key.to_string(),
            label,
            about,
            kind,
            default: value.to_string(),
            mine: theirs,
        });
    }
    out.retain(|s| !s.settings.is_empty() && !s.name.starts_with('_'));
    out
}

/// A label, a description and a way to change it, from the comment the
/// patch put after the value.
fn described(key: &str, value: &str, comment: &str) -> (String, String, Kind) {
    let lower = comment.to_ascii_lowercase();
    // The label is the comment up to its first bracket or comma.
    let label_end = comment.find(['(', ',']).unwrap_or(comment.len());
    let label = comment.get(..label_end).unwrap_or("").trim();
    let label = match label.is_empty() {
        true => readable(key),
        false => label.to_string(),
    };
    let about = comment
        .find('(')
        .and_then(|open| {
            let close = comment.get(open..)?.find(')')? + open;
            comment.get(open + 1..close).map(str::trim)
        })
        .unwrap_or("")
        .to_string();
    let number_after = |word: &str| -> Option<f64> {
        let at = lower.find(word)? + word.len();
        lower
            .get(at..)?
            .split_whitespace()
            .next()?
            .trim_end_matches([',', ';', ')'])
            .parse()
            .ok()
    };
    let range = number_after("from ").zip(number_after(" to "));
    let step = number_after("round to ");
    let percent = lower.contains("perc") || lower.contains('%');
    let switch_word = matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "false" | "on" | "off" | "yes" | "no"
    );
    let numeric = value.parse::<f64>().ok();
    let kind = match (numeric, range) {
        _ if switch_word => Kind::Switch,
        (Some(v), None) if (v == 0.0 || v == 1.0) && !value.contains('.') => Kind::Switch,
        (Some(_), _) => Kind::Number {
            range,
            step,
            percent,
        },
        (None, _) => Kind::Text,
    };
    (label, about, kind)
}

/// `RAIN_FX` and `rain_fx` as "Rain fx".
fn readable(name: &str) -> String {
    let words = name.replace(['_', '-'], " ").to_lowercase();
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Set one value in the driver's own file for a module.
pub fn set(
    documents: &Path,
    file: &str,
    section: &str,
    key: &str,
    value: &str,
) -> Result<PathBuf, String> {
    super::ini::set_in_file(&user_file(documents, file), section, key, value)
}

/// What a switch's value is written as, matching how the patch wrote it.
pub fn switch_value(default: &str, on: bool) -> String {
    let lower = default.to_ascii_lowercase();
    let (yes, no) = match lower.as_str() {
        "true" | "false" => ("true", "false"),
        "on" | "off" => ("on", "off"),
        "yes" | "no" => ("yes", "no"),
        _ => ("1", "0"),
    };
    match on {
        true => yes,
        false => no,
    }
    .to_string()
}

/// Whether a switch's value is on.
pub fn is_on(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULTS: &str = "\
[ABOUT]
NAME = Weather FX
DESCRIPTION = Dynamic weather and the sky.

[BASIC]
ENABLED = 1 ; Active

[CLOUDS] ; Clouds and their shadows
SHADOWS = 1 ; Cloud shadows (on the ground, not only the sky)
QUALITY = 0.75 ; Quality, from 0.25 to 1, round to 0.05, perc
DENSITY = 2.5
_INTERNAL = 3
COLOR = 255, 240, 230 ; Tint

[__HIDDEN]
X = 1
";

    /// **Every setting the patch describes, as it describes it**: the
    /// module's own sections, labels and ranges from its comments, a switch
    /// where the value is one, a number with its range and step and shown as
    /// a percentage where the comment says so, and text for the rest — with
    /// what the patch keeps to itself left out, and the driver's own value
    /// over the patch's.
    #[test]
    fn the_patch_describes_its_own_settings() {
        let mine = "[CLOUDS]\nQUALITY=0.5\n";
        let sections = sections(DEFAULTS, mine);
        let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["BASIC", "CLOUDS"]);
        let clouds = sections
            .get(1)
            .map(|s| s.settings.clone())
            .unwrap_or_default();
        assert_eq!(
            sections.get(1).map(|s| s.label.as_str()),
            Some("Clouds and their shadows")
        );
        let find = |key: &str| clouds.iter().find(|s| s.key == key).cloned();
        let shadows = find("SHADOWS").unwrap_or_else(|| unreachable!());
        assert_eq!(shadows.label, "Cloud shadows");
        assert_eq!(shadows.about, "on the ground, not only the sky");
        assert_eq!(shadows.kind, Kind::Switch);
        let quality = find("QUALITY").unwrap_or_else(|| unreachable!());
        assert_eq!(quality.label, "Quality");
        assert_eq!(
            quality.kind,
            Kind::Number {
                range: Some((0.25, 1.0)),
                step: Some(0.05),
                percent: true
            }
        );
        assert_eq!(quality.value(), "0.5", "the driver's value wins");
        assert_eq!(quality.default, "0.75");
        let density = find("DENSITY").unwrap_or_else(|| unreachable!());
        assert_eq!(density.label, "Density");
        assert!(matches!(density.kind, Kind::Number { range: None, .. }));
        assert_eq!(find("COLOR").map(|s| s.kind), Some(Kind::Text));
        assert!(find("_INTERNAL").is_none());
    }

    /// The modules are found with their own names, and a change is written
    /// to the driver's file and read back as theirs.
    #[test]
    fn a_module_is_found_and_a_change_is_read_back() {
        let root = std::env::temp_dir().join(format!("csp-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let config = root.join("extension/config");
        std::fs::create_dir_all(&config).unwrap_or_default();
        std::fs::write(config.join("weather_fx.ini"), DEFAULTS).unwrap_or_default();
        std::fs::write(config.join("rain_fx.ini"), "[BASIC]\nENABLED = 0\n").unwrap_or_default();
        std::fs::write(config.join("data_manifest.ini"), "[VERSION]\nX=1\n").unwrap_or_default();
        let found = modules(&root);
        let names: Vec<&str> = found.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["Rain fx", "Weather FX"]);
        assert_eq!(
            found.get(1).map(|m| m.about.as_str()),
            Some("Dynamic weather and the sky.")
        );
        let documents = root.join("Documents");
        let written = set(&documents, "weather_fx.ini", "CLOUDS", "SHADOWS", "0");
        assert!(written.is_ok());
        let read_back = read(&root, Some(&documents), "weather_fx.ini");
        let shadows = read_back
            .iter()
            .flat_map(|s| s.settings.iter())
            .find(|s| s.key == "SHADOWS")
            .map(|s| (s.value().to_string(), s.default.clone()));
        assert_eq!(shadows, Some(("0".to_string(), "1".to_string())));
        assert_eq!(switch_value("true", false), "false");
        assert_eq!(switch_value("1", true), "1");
        assert!(is_on("TRUE") && !is_on("0"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
