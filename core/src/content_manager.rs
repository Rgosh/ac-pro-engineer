use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarSpecs {
    pub id: String,
    pub name: String,
    pub brand: String,
    pub description: String,
    pub class: String,
    pub power: String,
    pub torque: String,
    pub weight: String,
    pub year: Option<i32>,
    pub power_hp: f32,
    pub weight_kg: f32,
}

#[derive(Debug, Clone)]
pub struct ContentManager {
    pub cars: Vec<CarSpecs>,
    pub ac_root: PathBuf,
}

impl Default for ContentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentManager {
    pub fn new() -> Self {
        Self::with_root_override(None)
    }

    /// Build a manager, honouring a configured install path.
    ///
    /// Detection used to be four hardcoded Windows drive letters. That found
    /// nothing on Linux — where the game is a normal Steam install under
    /// Proton — so `cars` was always empty, `get_car_specs` always returned
    /// None, and everything downstream of it silently did nothing. It also
    /// missed any Windows library on a drive other than C through F.
    pub fn with_root_override(configured: Option<&std::path::Path>) -> Self {
        let ac_root = crate::ac_paths::ac_install_root(configured).unwrap_or_else(|| {
            info!("No Assetto Corsa installation found; car specs unavailable");
            PathBuf::new()
        });

        let mut manager = Self {
            cars: Vec::new(),
            ac_root,
        };

        manager.scan_cars();
        manager
    }

    pub fn scan_cars(&mut self) {
        let cars_dir = self.ac_root.join("content").join("cars");
        if !cars_dir.exists() {
            return;
        }

        for entry in WalkDir::new(&cars_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
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

                    self.cars.push(CarSpecs {
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
        }
    }

    pub fn get_car_specs(&self, car_id: &str) -> Option<&CarSpecs> {
        self.cars.iter().find(|c| c.id == car_id)
    }
}

fn extract_number(s: &str) -> Option<f32> {
    let num_str: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num_str.parse().ok()
}

fn find_case_insensitive(base: &std::path::Path, name: &str) -> Option<PathBuf> {
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
