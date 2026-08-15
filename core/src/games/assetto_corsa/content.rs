//! Reading Assetto Corsa's car catalogue off disk.
//!
//! Everything here is AC's own file layout: `content/cars/<id>/ui/ui_car.json`,
//! its spelling of "bhp", and the fact that a modded car folder may capitalise
//! `UI` however it likes. The [`CarSpecs`] that come out are the neutral shape
//! the rest of the program works in — the same split as the telemetry, for the
//! same reason.
//!
//! ACC keeps none of this: no `content/cars`, no `ui_car.json`, and its car
//! list is baked into the executable. Whatever it does keep goes in its own
//! folder beside this one, and nothing above has to learn a second layout.

use crate::games::catalogue::CarSpecs;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Every car installed under an Assetto Corsa root.
///
/// An empty list is the normal answer for a machine with no game installed and
/// is not an error: the car specs sharpen a reference lap time and nothing
/// depends on them existing.
pub fn scan_cars(ac_root: &Path) -> Vec<CarSpecs> {
    let cars_dir = ac_root.join("content").join("cars");
    if !cars_dir.exists() {
        return Vec::new();
    }

    let mut cars = Vec::new();
    for entry in WalkDir::new(&cars_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let car_id = entry.file_name().to_string_lossy().to_string();
        let ui_dir = find_case_insensitive(entry.path(), "ui");
        let ui_path = ui_dir.and_then(|d| find_case_insensitive(&d, "ui_car.json"));

        if let Some(p) = ui_path
            && let Ok(content) = fs::read_to_string(p)
            && let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content)
        {
            let name = json_val["name"].as_str().unwrap_or("Unknown").to_string();
            let brand = json_val["brand"].as_str().unwrap_or("Unknown").to_string();
            let desc = json_val["description"].as_str().unwrap_or("").to_string();
            let class = json_val["class"].as_str().unwrap_or("street").to_string();

            let (power_s, torque_s, weight_s) = if let Some(specs) = json_val.get("specs") {
                (
                    specs["bhp"].as_str().unwrap_or("0").to_string(),
                    specs["torque"].as_str().unwrap_or("0").to_string(),
                    specs["weight"].as_str().unwrap_or("1000").to_string(),
                )
            } else {
                ("0".to_string(), "0".to_string(), "1000".to_string())
            };

            let power_clean = extract_number(&power_s).unwrap_or(100.0);
            let weight_clean = extract_number(&weight_s).unwrap_or(1000.0);

            cars.push(CarSpecs {
                id: car_id,
                name,
                brand,
                description: desc,
                class,
                power: power_s,
                torque: torque_s,
                weight: weight_s,
                year: json_val["year"].as_i64().map(|y| y as i32),
                power_hp: power_clean,
                weight_kg: weight_clean,
            });
        }
    }
    cars
}

/// AC writes its specs as human strings — "552bhp", "1 245 kg" — so the number
/// is dug out of whatever the car's author typed.
fn extract_number(s: &str) -> Option<f32> {
    let num_str: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num_str.parse().ok()
}

/// Mod folders capitalise `UI` and `ui_car.json` inconsistently, and Linux
/// filesystems care where Windows does not.
fn find_case_insensitive(base: &Path, name: &str) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
            {
                return Some(entry.path());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_dug_out_of_whatever_the_author_typed() {
        assert_eq!(extract_number("552bhp"), Some(552.0));
        assert_eq!(extract_number("1245 kg"), Some(1245.0));
        assert_eq!(extract_number("N/A"), None);
    }

    /// No game installed is an empty catalogue, not a failure.
    #[test]
    fn a_root_with_no_cars_scans_to_nothing() {
        assert!(scan_cars(Path::new("/nonexistent/assettocorsa")).is_empty());
    }
}
