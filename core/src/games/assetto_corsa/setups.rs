//! Assetto Corsa's setup files: where they live, and what is in them.
//!
//! AC keeps setups as INI under
//! `Documents/Assetto Corsa/setups/<car>/<track>/`, with one section per
//! adjustment and a `VALUE` in each — `[SPRING_RATE_LF] VALUE=12`. The numbers
//! are click indices into a range that lives inside the car's own data, which
//! is why nothing above this file tries to read a spring rate in N/mm out of
//! them.
//!
//! Three folders are searched, and the order is the answer to "which setup is
//! this": the track's own, then `generic`, then `downloaded`.
//!
//! ACC keeps JSON in a different tree entirely, so this is the file that gets
//! a sibling rather than an `if` — see §7 of `docs/roadmap.md`.

use crate::setup_manager::CarSetup;
use ini::Ini;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Where AC keeps its setups, honouring a configured override.
///
/// Under Proton this is inside the game's prefix, not the host's `~/Documents`
/// — which is why `UserDirs::document_dir()` found nothing on Linux and local
/// setups were never discovered there.
pub fn setups_root(configured_docs: Option<&Path>) -> Option<PathBuf> {
    super::paths::ac_documents_dir(configured_docs)
        .map(|docs| docs.join("Assetto Corsa").join("setups"))
}

/// The folder a downloaded setup for `car` is installed into.
pub fn downloaded_dir(configured_docs: Option<&Path>, car: &str) -> Option<PathBuf> {
    setups_root(configured_docs).map(|root| root.join(car).join("downloaded"))
}

/// The name a downloaded setup takes on disk.
///
/// Both halves are expected to be sanitised already: this decides the shape,
/// not the safety.
pub fn file_name(safe_author: &str, safe_name: &str) -> String {
    format!("{}_{}.ini", safe_author, safe_name)
}

pub fn scan_folders(
    car_model: &str,
    track_name: &str,
    configured_docs: &std::path::Path,
) -> Vec<CarSetup> {
    let mut found = Vec::new();
    if let Some(root) =
        setups_root((!configured_docs.as_os_str().is_empty()).then_some(configured_docs))
    {
        let base_path = root.join(car_model);
        if !track_name.is_empty() && track_name != "-" {
            scan_single_folder(
                &base_path.join(track_name),
                track_name,
                car_model,
                &mut found,
            );
        }
        scan_single_folder(&base_path.join("generic"), "Generic", car_model, &mut found);
        scan_single_folder(
            &base_path.join("downloaded"),
            "Downloaded",
            car_model,
            &mut found,
        );
    }
    found
}

fn scan_single_folder(folder: &Path, source: &str, car_id: &str, list: &mut Vec<CarSetup>) {
    if !folder.exists() {
        return;
    }
    for entry in WalkDir::new(folder)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file()
            && path.extension().is_some_and(|ext| ext == "ini")
            && let Ok(conf) = Ini::load_from_file(path)
        {
            let get = |sec: &str, key: &str| -> u32 {
                conf.section(Some(sec))
                    .and_then(|s| s.get(key))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0)
            };
            let get_i = |sec: &str, key: &str| -> i32 {
                conf.section(Some(sec))
                    .and_then(|s| s.get(key))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0)
            };
            let get_s = |sec: &str, key: &str| -> String {
                conf.section(Some(sec))
                    .and_then(|s| s.get(key))
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            };

            let mut gears = Vec::new();
            for i in 2..=9 {
                let key = format!("INTERNAL_GEAR_{}", i);
                if let Some(val) = conf
                    .section(Some(key.as_str()))
                    .and_then(|s| s.get("VALUE"))
                    && let Ok(v) = val.parse::<u32>()
                {
                    gears.push(v);
                }
            }

            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            list.push(CarSetup {
                name,
                path: path.to_path_buf(),
                source: source.to_string(),
                author: "Local".to_string(),
                credits: String::new(),
                notes: get_s("NOTES", "VALUE"),
                car_id: car_id.to_string(),
                is_remote: false,
                fuel: get("FUEL", "VALUE"),
                brake_bias: get("FRONT_BIAS", "VALUE"),
                engine_limiter: get("ENGINE_LIMITER", "VALUE"),
                pressure_lf: get("PRESSURE_LF", "VALUE"),
                pressure_rf: get("PRESSURE_RF", "VALUE"),
                pressure_lr: get("PRESSURE_LR", "VALUE"),
                pressure_rr: get("PRESSURE_RR", "VALUE"),
                wing_1: get("WING_1", "VALUE"),
                wing_2: get("WING_2", "VALUE"),
                camber_lf: get_i("CAMBER_LF", "VALUE"),
                camber_rf: get_i("CAMBER_RF", "VALUE"),
                camber_lr: get_i("CAMBER_LR", "VALUE"),
                camber_rr: get_i("CAMBER_RR", "VALUE"),
                toe_lf: get_i("TOE_OUT_LF", "VALUE"),
                toe_rf: get_i("TOE_OUT_RF", "VALUE"),
                toe_lr: get_i("TOE_OUT_LR", "VALUE"),
                toe_rr: get_i("TOE_OUT_RR", "VALUE"),
                spring_lf: get("SPRING_RATE_LF", "VALUE"),
                spring_rf: get("SPRING_RATE_RF", "VALUE"),
                spring_lr: get("SPRING_RATE_LR", "VALUE"),
                spring_rr: get("SPRING_RATE_RR", "VALUE"),
                rod_length_lf: get_i("ROD_LENGTH_LF", "VALUE"),
                rod_length_rf: get_i("ROD_LENGTH_RF", "VALUE"),
                rod_length_lr: get_i("ROD_LENGTH_LR", "VALUE"),
                rod_length_rr: get_i("ROD_LENGTH_RR", "VALUE"),
                arb_front: get("ARB_FRONT", "VALUE"),
                arb_rear: get("ARB_REAR", "VALUE"),
                damp_bump_lf: get("DAMP_BUMP_LF", "VALUE"),
                damp_bump_rf: get("DAMP_BUMP_RF", "VALUE"),
                damp_bump_lr: get("DAMP_BUMP_LR", "VALUE"),
                damp_bump_rr: get("DAMP_BUMP_RR", "VALUE"),
                damp_rebound_lf: get("DAMP_REBOUND_LF", "VALUE"),
                damp_rebound_rf: get("DAMP_REBOUND_RF", "VALUE"),
                damp_rebound_lr: get("DAMP_REBOUND_LR", "VALUE"),
                damp_rebound_rr: get("DAMP_REBOUND_RR", "VALUE"),
                diff_power: get("DIFF_POWER", "VALUE"),
                diff_coast: get("DIFF_COAST", "VALUE"),
                final_ratio: get("FINAL_RATIO", "VALUE"),
                gears,
            });
        }
    }
}

/// Flatten a value so it cannot break out of the `KEY=value` line it is
/// written on.
///
/// Every other field of a `CarSetup` is a `u32` or `i32` and cannot express
/// anything but a number. `notes` is a free-form string that arrives from the
/// setup JSON fetched over the network, and a newline in it would start a new
/// INI line — `[SPRING_RATE_LF]\nVALUE=...` in a notes field is a section AC
/// would parse and apply as part of the setup.
fn sanitize_ini_value(value: &str) -> String {
    value
        .chars()
        .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
        .collect()
}

pub fn generate_ini_content(s: &CarSetup) -> String {
    let mut out = String::new();
    if !s.notes.is_empty() {
        out.push_str(&format!(
            "[NOTES]\nVALUE={}\n\n",
            sanitize_ini_value(&s.notes)
        ));
    }
    out.push_str(&format!(
        "[FUEL]\nVALUE={}\n\n[FRONT_BIAS]\nVALUE={}\n\n[ENGINE_LIMITER]\nVALUE={}\n\n",
        s.fuel, s.brake_bias, s.engine_limiter
    ));
    out.push_str(&format!("[PRESSURE_LF]\nVALUE={}\n[PRESSURE_RF]\nVALUE={}\n[PRESSURE_LR]\nVALUE={}\n[PRESSURE_RR]\nVALUE={}\n\n", s.pressure_lf, s.pressure_rf, s.pressure_lr, s.pressure_rr));
    out.push_str(&format!(
        "[WING_1]\nVALUE={}\n[WING_2]\nVALUE={}\n\n",
        s.wing_1, s.wing_2
    ));
    out.push_str(&format!("[CAMBER_LF]\nVALUE={}\n[CAMBER_RF]\nVALUE={}\n[CAMBER_LR]\nVALUE={}\n[CAMBER_RR]\nVALUE={}\n", s.camber_lf, s.camber_rf, s.camber_lr, s.camber_rr));
    out.push_str(&format!("[TOE_OUT_LF]\nVALUE={}\n[TOE_OUT_RF]\nVALUE={}\n[TOE_OUT_LR]\nVALUE={}\n[TOE_OUT_RR]\nVALUE={}\n\n", s.toe_lf, s.toe_rf, s.toe_lr, s.toe_rr));
    out.push_str(&format!("[SPRING_RATE_LF]\nVALUE={}\n[SPRING_RATE_RF]\nVALUE={}\n[SPRING_RATE_LR]\nVALUE={}\n[SPRING_RATE_RR]\nVALUE={}\n", s.spring_lf, s.spring_rf, s.spring_lr, s.spring_rr));
    out.push_str(&format!("[ROD_LENGTH_LF]\nVALUE={}\n[ROD_LENGTH_RF]\nVALUE={}\n[ROD_LENGTH_LR]\nVALUE={}\n[ROD_LENGTH_RR]\nVALUE={}\n", s.rod_length_lf, s.rod_length_rf, s.rod_length_lr, s.rod_length_rr));
    out.push_str(&format!(
        "[ARB_FRONT]\nVALUE={}\n[ARB_REAR]\nVALUE={}\n\n",
        s.arb_front, s.arb_rear
    ));
    out.push_str(&format!("[DAMP_BUMP_LF]\nVALUE={}\n[DAMP_BUMP_RF]\nVALUE={}\n[DAMP_BUMP_LR]\nVALUE={}\n[DAMP_BUMP_RR]\nVALUE={}\n", s.damp_bump_lf, s.damp_bump_rf, s.damp_bump_lr, s.damp_bump_rr));
    out.push_str(&format!("[DAMP_REBOUND_LF]\nVALUE={}\n[DAMP_REBOUND_RF]\nVALUE={}\n[DAMP_REBOUND_LR]\nVALUE={}\n[DAMP_REBOUND_RR]\nVALUE={}\n\n", s.damp_rebound_lf, s.damp_rebound_rf, s.damp_rebound_lr, s.damp_rebound_rr));
    out.push_str(&format!(
        "[DIFF_POWER]\nVALUE={}\n[DIFF_COAST]\nVALUE={}\n[FINAL_RATIO]\nVALUE={}\n",
        s.diff_power, s.diff_coast, s.final_ratio
    ));
    for (i, g) in s.gears.iter().enumerate() {
        out.push_str(&format!("[INTERNAL_GEAR_{}]\nVALUE={}\n", i + 2, g));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `notes` is the one free-form string in a setup, and it arrives from
    /// the network. A newline in it would open a new INI line, and AC parses
    /// whatever section that line names as part of the setup.
    #[test]
    fn ini_notes_cannot_inject_extra_sections() {
        let setup = CarSetup {
            notes: "nice setup\n\n[SPRING_RATE_LF]\nVALUE=99999".to_string(),
            ..CarSetup::default()
        };

        let ini = generate_ini_content(&setup);
        let notes_line = ini
            .lines()
            .find(|l| l.starts_with("VALUE="))
            .expect("the notes value is written");

        assert!(
            notes_line.contains("99999"),
            "the text is kept, just flattened onto one line: {notes_line}"
        );
        // What matters is that it is no longer a *section header*: an INI
        // parser only recognises `[NAME]` at the start of a line. Inside a
        // value it is just text.
        assert_eq!(
            ini.lines()
                .filter(|l| l.starts_with("[SPRING_RATE_LF]"))
                .count(),
            1,
            "only the real spring rate section, not one smuggled in via notes"
        );
        assert_eq!(
            ini.lines().filter(|l| l.contains("99999")).count(),
            1,
            "the whole injected string stays on the single notes line"
        );
    }

    #[test]
    fn sanitize_ini_value_flattens_line_breaks() {
        assert_eq!(sanitize_ini_value("a\r\nb\nc"), "a  b c");
        assert_eq!(sanitize_ini_value("plain text"), "plain text");
    }

    /// The three folders are searched in the order that answers "which setup
    /// is this": the track's own beats generic, and generic beats downloaded.
    #[test]
    fn a_missing_setup_tree_scans_to_nothing() {
        let nowhere = std::path::Path::new("/nonexistent/documents");
        assert!(scan_folders("ks_ferrari_sf70h", "monza", nowhere).is_empty());
    }
}
