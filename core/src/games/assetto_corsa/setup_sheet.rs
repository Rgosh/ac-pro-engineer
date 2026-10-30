//! A car's setup sheet: every adjustment it has, and the values a setup file
//! gives them.
//!
//! **Two files, and they speak different units.** The car's own `setup.ini`
//! (in `data/` or packed in `data.acd`) says what can be adjusted — a section
//! per adjustment, with `MIN`, `MAX`, `STEP`, the tab it is shown on and its
//! name. A setup somebody saved (`Documents/Assetto Corsa/setups/<car>/<track>/
//! <name>.ini`) gives each a `VALUE`. For some adjustments that value is in the
//! sheet's own units — a pressure of 27, a camber of −30 — and for others it is
//! a click counted from the bottom of the range: a spring rate of `12` on a
//! sheet whose range is 80 000 to 180 000.
//!
//! **Which one a value is, is read, never assumed.** A value inside the
//! sheet's range is a value; one inside `0..=clicks` and outside the range is
//! a click. Writing back always uses the form that was read, and an adjustment
//! the file never mentioned is never written — so nothing this module saves can
//! give the game a number in the wrong units, which would load as a car set to
//! the end of its travel and look like it worked.
//!
//! Everything in a setup file that is not an adjustment — the `[CAR]` model,
//! CSP's `[__EXT_PATCH]`, whatever a newer game version adds — is kept exactly
//! as it was.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One thing the car lets you change.
#[derive(Debug, Clone, PartialEq)]
pub struct Adjustment {
    /// The section name: `CAMBER_LF`, `SPRING_RATE_RR`, `WING_1`.
    pub key: String,
    /// What the game calls it on screen, or the key tidied when it says
    /// nothing.
    pub name: String,
    /// The tab the game shows it on — `TYRES`, `ALIGNMENT`, `SUSPENSION` — or
    /// `GENERIC` when the sheet does not say.
    pub tab: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

/// The form a value in a setup file is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// In the sheet's own units, between `min` and `max`.
    Value,
    /// A click counted from `min`, between 0 and [`Adjustment::clicks`].
    Click,
}

impl Adjustment {
    /// How many clicks the range has.
    pub fn clicks(&self) -> u32 {
        if self.step <= 0.0 {
            return 0;
        }
        ((self.max - self.min) / self.step).round().max(0.0) as u32
    }

    /// Which form `value`, read from a setup file, is in — or `None` when it
    /// fits neither, and must be left alone.
    pub fn form_of(&self, value: f64) -> Option<Form> {
        let eps = self.step.abs().max(1e-6) * 1e-3;
        if value >= self.min - eps && value <= self.max + eps {
            return Some(Form::Value);
        }
        if value >= -eps && value <= self.clicks() as f64 + eps && value.fract().abs() < 1e-6 {
            return Some(Form::Click);
        }
        None
    }

    /// The sheet's value a setup-file number stands for.
    pub fn as_value(&self, read: f64, form: Form) -> f64 {
        match form {
            Form::Value => read,
            Form::Click => self.min + read * self.step,
        }
    }

    /// The setup-file number for a sheet value, in the form it was read in —
    /// held to the range and snapped to a step.
    pub fn as_written(&self, value: f64, form: Form) -> f64 {
        let snapped = self.snap(value);
        match form {
            Form::Value => snapped,
            Form::Click if self.step > 0.0 => ((snapped - self.min) / self.step).round(),
            Form::Click => 0.0,
        }
    }

    /// Held to the range, on a step.
    pub fn snap(&self, value: f64) -> f64 {
        let clamped = value.clamp(self.min.min(self.max), self.max.max(self.min));
        if self.step <= 0.0 {
            return clamped;
        }
        let snapped = self.min + ((clamped - self.min) / self.step).round() * self.step;
        // Rounding noise off: 0.1 steps should read as 26.5, not 26.499999.
        (snapped * 1e6).round() / 1e6
    }
}

/// Every adjustment in a car's `setup.ini` text, in the order the file lists
/// them.
///
/// A section is an adjustment when it has `MIN`, `MAX` and `STEP`; the rest —
/// `[DISPLAY_METHOD]`, `[GEARS]` with its ratio files — are not sliders and
/// are left out.
pub fn read_sheet(text: &str) -> Vec<Adjustment> {
    sections(text)
        .into_iter()
        .filter_map(|(key, fields)| {
            let number = |name: &str| fields.get(name).and_then(|v| parse_number(v));
            let (min, max, step) = (number("MIN")?, number("MAX")?, number("STEP")?);
            let name = fields
                .get("NAME")
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| tidy(&key));
            let tab = fields
                .get("TAB")
                .map(|t| t.trim().to_uppercase())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "GENERIC".to_string());
            Some(Adjustment {
                key,
                name,
                tab,
                min,
                max,
                step,
            })
        })
        .collect()
}

/// The car's sheet, out of its folder — `data/setup.ini`, or the packed
/// `data.acd` the shipped cars keep it in.
pub fn sheet(car_folder: &Path) -> Option<Vec<Adjustment>> {
    let text = super::car_data::data_file(car_folder, "setup.ini")?;
    let sheet = read_sheet(&text);
    (!sheet.is_empty()).then_some(sheet)
}

/// The `VALUE` of every section in a setup file's text.
pub fn read_values(text: &str) -> BTreeMap<String, f64> {
    sections(text)
        .into_iter()
        .filter_map(|(key, fields)| Some((key, parse_number(fields.get("VALUE")?)?)))
        .collect()
}

/// `original` with the adjustments in `values` changed — in the form each was
/// read in — and everything else exactly as it was.
///
/// `values` are sheet values (see [`Adjustment::as_value`]). An adjustment
/// that `original` does not mention is not added, and one whose present value
/// fits neither form is not touched: see the note at the top of this file.
pub fn with_values(original: &str, sheet: &[Adjustment], values: &BTreeMap<String, f64>) -> String {
    let read = read_values(original);
    let by_key: BTreeMap<&str, &Adjustment> = sheet.iter().map(|a| (a.key.as_str(), a)).collect();
    let mut section = String::new();
    let mut out = String::with_capacity(original.len());
    for line in original.split_inclusive('\n') {
        let bare = line.trim();
        if bare.starts_with('[') && bare.ends_with(']') {
            section = bare[1..bare.len() - 1].trim().to_string();
            out.push_str(line);
            continue;
        }
        let replaced = bare
            .split_once('=')
            .filter(|(name, _)| name.trim().eq_ignore_ascii_case("VALUE"))
            .and_then(|_| {
                let adjustment = by_key.get(section.as_str())?;
                let wanted = values.get(&section)?;
                let form = adjustment.form_of(*read.get(&section)?)?;
                let written = adjustment.as_written(*wanted, form);
                let ending = if line.ends_with("\r\n") {
                    "\r\n"
                } else if line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                Some(format!("VALUE={}{ending}", number_text(written)))
            });
        match replaced {
            Some(new) => out.push_str(&new),
            None => out.push_str(line),
        }
    }
    out
}

/// One saved setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSetup {
    pub path: PathBuf,
    /// The file's name without `.ini`.
    pub name: String,
    /// The folder it is in under the car: a track id, `generic`, `downloaded`.
    pub folder: String,
    /// When it was last written, seconds since the epoch.
    pub modified: u64,
}

/// Every setup saved for `car`, in every folder under it, newest first.
pub fn saved(configured_docs: Option<&Path>, car: &str) -> Vec<SavedSetup> {
    let Some(root) = super::setups::setups_root(configured_docs) else {
        return Vec::new();
    };
    saved_under(&root.join(car))
}

/// [`saved`], given the car's own setups folder.
pub fn saved_under(car_dir: &Path) -> Vec<SavedSetup> {
    let mut found = Vec::new();
    let Ok(folders) = std::fs::read_dir(car_dir) else {
        return found;
    };
    for folder in folders.flatten() {
        let Ok(files) = std::fs::read_dir(folder.path()) else {
            continue;
        };
        let folder_name = folder.file_name().to_string_lossy().into_owned();
        for file in files.flatten() {
            let path = file.path();
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
            {
                continue;
            }
            let modified = file
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_secs())
                .unwrap_or(0);
            found.push(SavedSetup {
                name: path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                folder: folder_name.clone(),
                path,
                modified,
            });
        }
    }
    found.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    found
}

/// Where a new setup called `name` for `track` is written — beside the ones
/// the game itself saves for that circuit — never over one that exists.
pub fn new_path(car_dir: &Path, track_folder: &str, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(
            |c| match c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.') {
                true => c,
                false => '_',
            },
        )
        .collect();
    let safe = safe.trim().trim_matches('.').to_string();
    let safe = if safe.is_empty() {
        "setup".to_string()
    } else {
        safe
    };
    let folder = car_dir.join(if track_folder.is_empty() {
        "generic"
    } else {
        track_folder
    });
    let mut candidate = folder.join(format!("{safe}.ini"));
    let mut n = 2;
    while candidate.exists() {
        candidate = folder.join(format!("{safe} ({n}).ini"));
        n += 1;
    }
    candidate
}

/// One change the engineer asked for, as a change to a setup.
#[derive(Debug, Clone, PartialEq)]
pub struct Advice {
    /// The adjustment it changes.
    pub key: String,
    /// How far, in the sheet's units.
    pub by: f64,
    /// What the engineer said, as it said it.
    pub said: String,
}

/// The setup changes the engineer's findings ask for, on this car's sheet.
///
/// **Only what it gave a number for.** Most of what the engineer says is a
/// direction — "more front wing", "brake later" — and turning a direction into
/// a click count would be this module inventing advice the engineer did not
/// give. What it does give a number for is tyre pressure, per corner: the hot
/// pressure it measured and the hot pressure the car wants. The cold setting
/// in a setup moves the hot one by about as much, so the change is that
/// difference — which is the rule of thumb every engineer starts from, and
/// the follow-up on the next session says whether it was enough.
///
/// Pressures are kept in psi whatever the driver reads them in, and AC's
/// setup files are in psi too.
pub fn advice_for(sheet: &[Adjustment], findings: &[crate::followup::Said]) -> Vec<Advice> {
    let key_of = |corner: &str| match corner {
        "FL" => Some("PRESSURE_LF"),
        "FR" => Some("PRESSURE_RF"),
        "RL" => Some("PRESSURE_LR"),
        "RR" => Some("PRESSURE_RR"),
        _ => None,
    };
    let mut out: Vec<Advice> = Vec::new();
    for finding in findings {
        for parameter in &finding.parameters {
            let Some(key) = key_of(parameter.name.as_str()) else {
                continue;
            };
            if !sheet.iter().any(|a| a.key == key) || out.iter().any(|a| a.key == key) {
                continue;
            }
            let by = (parameter.target - parameter.current) as f64;
            if by.abs() < 0.05 {
                continue;
            }
            out.push(Advice {
                key: key.to_string(),
                by,
                said: finding.message.clone(),
            });
        }
    }
    out
}

/// The sections of an INI text and their fields, keys upper-cased, in order.
///
/// **Lenient, like the game.** AC's own files have comments after values,
/// spaces around `=`, and the odd duplicate section; the last one wins, which
/// is what the game does.
fn sections(text: &str) -> Vec<(String, BTreeMap<String, String>)> {
    let mut out: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.starts_with('[') && line.ends_with(']') {
            let name = line[1..line.len() - 1].trim().to_uppercase();
            if let Some(at) = out.iter().position(|(k, _)| *k == name) {
                let again = out.remove(at);
                out.push(again);
            } else {
                out.push((name, BTreeMap::new()));
            }
            continue;
        }
        if let (Some((_, fields)), Some((key, value))) = (out.last_mut(), line.split_once('=')) {
            fields.insert(key.trim().to_uppercase(), value.trim().to_string());
        }
    }
    out
}

fn parse_number(text: &str) -> Option<f64> {
    text.split_whitespace().next()?.parse().ok()
}

/// A number as a setup file writes it: no trailing `.0` on a whole one.
fn number_text(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{}", value.round() as i64)
    } else {
        let text = format!("{value:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// `CAMBER_LF` → `Camber LF`.
fn tidy(key: &str) -> String {
    key.split('_')
        .map(|part| match part.len() <= 2 {
            true => part.to_string(),
            false => {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first
                        .to_uppercase()
                        .chain(chars.flat_map(|c| c.to_lowercase()))
                        .collect(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of a real sheet: pressures and camber in their own units,
    /// springs as a range in N/m, a section that is not a slider at all.
    const SHEET: &str = "\
[DISPLAY_METHOD]
SHOW_CLICKS=0

[PRESSURE_LF]
SHOW_CLICKS=0
TAB=TYRES
NAME=Pressure LF
MIN=15
MAX=40
STEP=1

[CAMBER_LF]
TAB=ALIGNMENT
NAME=Camber LF
MIN=-50   ; tenths of a degree
MAX=0
STEP=1

[SPRING_RATE_LF]
TAB=SUSPENSION
MIN=80000
MAX=180000
STEP=5000

[GEARS]
USE_GEARSET=1
";

    const SAVED: &str = "\
[CAR]
MODEL=ks_test

[PRESSURE_LF]
VALUE=27

[CAMBER_LF]
VALUE=-30

[SPRING_RATE_LF]
VALUE=8

[__EXT_PATCH]
VERSION=0.2.11
";

    #[test]
    fn a_sheet_lists_what_can_be_adjusted_and_nothing_else() {
        let sheet = read_sheet(SHEET);
        let keys: Vec<&str> = sheet.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys, ["PRESSURE_LF", "CAMBER_LF", "SPRING_RATE_LF"]);
        assert_eq!(
            sheet[1].min, -50.0,
            "a comment after a value is not part of it"
        );
        assert_eq!(
            sheet[2].name, "Spring Rate LF",
            "a missing name is the key tidied"
        );
        assert_eq!(sheet[2].tab, "SUSPENSION");
        assert_eq!(sheet[2].clicks(), 20);
    }

    #[test]
    fn a_value_inside_the_range_is_a_value_and_one_outside_is_a_click() {
        let sheet = read_sheet(SHEET);
        assert_eq!(sheet[0].form_of(27.0), Some(Form::Value));
        assert_eq!(sheet[1].form_of(-30.0), Some(Form::Value));
        assert_eq!(sheet[2].form_of(8.0), Some(Form::Click));
        assert_eq!(sheet[2].as_value(8.0, Form::Click), 120000.0);
        // Neither: past the clicks and outside the range.
        assert_eq!(sheet[2].form_of(55.0), None);
    }

    #[test]
    fn writing_keeps_each_value_in_the_form_it_was_read() {
        let sheet = read_sheet(SHEET);
        let mut wanted = BTreeMap::new();
        wanted.insert("PRESSURE_LF".to_string(), 26.0);
        wanted.insert("CAMBER_LF".to_string(), -27.4);
        wanted.insert("SPRING_RATE_LF".to_string(), 131000.0);
        let out = with_values(SAVED, &sheet, &wanted);
        let back = read_values(&out);
        assert_eq!(back["PRESSURE_LF"], 26.0);
        assert_eq!(back["CAMBER_LF"], -27.0, "snapped to a step");
        assert_eq!(back["SPRING_RATE_LF"], 10.0, "130 000 is the tenth click");
        // Everything that is not an adjustment is exactly as it was.
        assert!(out.contains("[CAR]\nMODEL=ks_test\n"));
        assert!(out.contains("[__EXT_PATCH]\nVERSION=0.2.11\n"));
    }

    #[test]
    fn an_adjustment_the_file_does_not_mention_is_not_added() {
        let sheet = read_sheet(SHEET);
        let mut wanted = BTreeMap::new();
        wanted.insert("PRESSURE_LF".to_string(), 30.0);
        let original = "[CAMBER_LF]\nVALUE=-10\n";
        assert_eq!(with_values(original, &sheet, &wanted), original);
    }

    #[test]
    fn a_value_in_neither_form_is_left_alone() {
        let sheet = read_sheet(SHEET);
        let mut wanted = BTreeMap::new();
        wanted.insert("SPRING_RATE_LF".to_string(), 100000.0);
        let original = "[SPRING_RATE_LF]\nVALUE=55\n";
        assert_eq!(with_values(original, &sheet, &wanted), original);
    }

    #[test]
    fn windows_line_endings_survive() {
        let sheet = read_sheet(SHEET);
        let mut wanted = BTreeMap::new();
        wanted.insert("PRESSURE_LF".to_string(), 25.0);
        let out = with_values("[PRESSURE_LF]\r\nVALUE=27\r\n", &sheet, &wanted);
        assert_eq!(out, "[PRESSURE_LF]\r\nVALUE=25\r\n");
    }

    #[test]
    fn the_engineer_s_pressures_become_changes_to_the_cold_setting() {
        use crate::engineer::{Parameter, Severity};
        use crate::followup::Said;
        let sheet = read_sheet(SHEET);
        let findings = vec![
            Said {
                component: "Tyres".into(),
                category: "Pressure".into(),
                severity: Severity::Warning,
                message: "Front left pressure: 29.4 (target 27.5)".into(),
                parameters: vec![
                    Parameter {
                        name: "FL".into(),
                        current: 29.4,
                        target: 27.5,
                        unit: "psi".into(),
                    },
                    // A corner this sheet cannot set is not advice for it.
                    Parameter {
                        name: "RR".into(),
                        current: 25.0,
                        target: 26.0,
                        unit: "psi".into(),
                    },
                ],
            },
            // A direction with no number is not turned into one.
            Said {
                component: "Aero".into(),
                category: "Balance".into(),
                severity: Severity::Info,
                message: "More front wing".into(),
                parameters: Vec::new(),
            },
        ];
        let advice = advice_for(&sheet, &findings);
        assert_eq!(advice.len(), 1, "{advice:?}");
        assert_eq!(advice[0].key, "PRESSURE_LF");
        assert!((advice[0].by + 1.9).abs() < 1e-5);
    }

    #[test]
    fn a_new_setup_never_lands_on_an_old_one() {
        let dir = std::env::temp_dir().join(format!("acpe-sheet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("ks_spa")).expect("a scratch folder");
        let first = new_path(&dir, "ks_spa", "quali / dry");
        assert_eq!(first, dir.join("ks_spa").join("quali _ dry.ini"));
        std::fs::write(&first, "").expect("the first setup");
        let second = new_path(&dir, "ks_spa", "quali / dry");
        assert_eq!(second, dir.join("ks_spa").join("quali _ dry (2).ini"));
        let saved = saved_under(&dir);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].folder, "ks_spa");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
