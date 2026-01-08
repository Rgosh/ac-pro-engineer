use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
// use directories_next::UserDirs; // <-- УДАЛЕНО

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

impl ContentManager {
    pub fn new() -> Self {
        // Попытка найти папку игры. Можно расширить список путей.
        let ac_root = Self::detect_ac_root().unwrap_or(PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa"));
        
        let mut manager = Self {
            cars: Vec::new(),
            ac_root,
        };
        
        // Запускаем сканирование (в идеале асинхронно, но тут для простоты сразу)
        manager.scan_cars();
        manager
    }

    fn detect_ac_root() -> Option<PathBuf> {
        let paths = [
            r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa",
            r"D:\SteamLibrary\steamapps\common\assettocorsa",
            r"E:\SteamLibrary\steamapps\common\assettocorsa",
            r"F:\SteamLibrary\steamapps\common\assettocorsa",
        ];

        for p in paths {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    pub fn scan_cars(&mut self) {
        let cars_dir = self.ac_root.join("content").join("cars");
        if !cars_dir.exists() { return; }

        // Сканируем только глубину 1 (папки машин)
        for entry in WalkDir::new(&cars_dir).min_depth(1).max_depth(1).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_dir() {
                let car_id = entry.file_name().to_string_lossy().to_string();
                let ui_path = entry.path().join("ui").join("ui_car.json");
                
                if ui_path.exists() {
                    if let Ok(content) = fs::read_to_string(ui_path) {
                        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                            let name = json_val["name"].as_str().unwrap_or("Unknown").to_string();
                            let brand = json_val["brand"].as_str().unwrap_or("Unknown").to_string();
                            let desc = json_val["description"].as_str().unwrap_or("").to_string();
                            let class = json_val["class"].as_str().unwrap_or("street").to_string();
                            
                            let (power_s, torque_s, weight_s) = if let Some(specs) = json_val.get("specs") {
                                (
                                    specs["bhp"].as_str().unwrap_or("0").to_string(),
                                    specs["torque"].as_str().unwrap_or("0").to_string(),
                                    specs["weight"].as_str().unwrap_or("1000").to_string()
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
        }
    }
    
    pub fn get_car_specs(&self, car_id: &str) -> Option<&CarSpecs> {
        self.cars.iter().find(|c| c.id == car_id)
    }
}

fn extract_number(s: &str) -> Option<f32> {
    let num_str: String = s.chars()
        .filter(|c| c.is_digit(10) || *c == '.')
        .collect();
    num_str.parse().ok()
}