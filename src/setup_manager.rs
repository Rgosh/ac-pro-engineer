use directories_next::UserDirs;
use std::path::PathBuf;
use walkdir::WalkDir;
use ini::Ini;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct CarSetup {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
    
    // Basic
    pub fuel: u32,
    pub brake_bias: u32,
    
    // Pressures
    pub pressure_lf: u32,
    pub pressure_rf: u32,
    pub pressure_lr: u32,
    pub pressure_rr: u32,
    
    // Aero
    pub wing_1: u32,
    pub wing_2: u32,
    
    // Camber
    pub camber_lf: i32,
    pub camber_rf: i32,
    pub camber_lr: i32,
    pub camber_rr: i32,
    
    // Toe
    pub toe_lf: i32,
    pub toe_rf: i32,
    pub toe_lr: i32,
    pub toe_rr: i32,
    
    // Springs
    pub spring_lf: u32,
    pub spring_rf: u32,
    pub spring_lr: u32,
    pub spring_rr: u32,
    pub arb_front: u32,
    pub arb_rear: u32,
    
    // Dampers
    pub damp_bump_lf: u32,
    pub damp_rebound_lf: u32,
    pub damp_bump_lr: u32,
    pub damp_rebound_lr: u32,
    
    // Differential
    pub diff_power: u32,
    pub diff_coast: u32,
}

impl CarSetup {
    pub fn match_score(&self, current_fuel: f32, current_bias: f32, current_pressures: &[f32; 4], _current_temps: &[f32; 4]) -> u32 {
        let mut score = 0;
        
        // Fuel match (within 2kg)
        if (self.fuel as f32 - current_fuel).abs() < 2.0 {
            score += 30;
        }
        
        // Brake bias match (within 2%)
        let bias_file = self.brake_bias as f32 / 100.0;
        if (bias_file - current_bias).abs() < 0.02 {
            score += 25;
        }
        
        // Pressure match (average within 1 PSI)
        let avg_pressure_file = (self.pressure_lf + self.pressure_rf + self.pressure_lr + self.pressure_rr) as f32 / 4.0;
        let avg_pressure_current = current_pressures.iter().sum::<f32>() / 4.0;
        if (avg_pressure_file - avg_pressure_current).abs() < 1.5 {
            score += 20;
        }
        
        // Individual pressure match
        let pressures_file = [
            self.pressure_lf as f32,
            self.pressure_rf as f32,
            self.pressure_lr as f32,
            self.pressure_rr as f32,
        ];
        
        for i in 0..4 {
            if (pressures_file[i] - current_pressures[i]).abs() < 2.0 {
                score += 5;
            }
        }
        
        score
    }
    
    pub fn get_recommended_adjustments(&self, current_pressures: &[f32; 4], _current_temps: &[f32; 4]) -> Vec<String> {
        let mut adjustments = Vec::new();
        
        // Pressure adjustments
        let target_pressures = [
            self.pressure_lf as f32,
            self.pressure_rf as f32,
            self.pressure_lr as f32,
            self.pressure_rr as f32,
        ];
        
        for i in 0..4 {
            let diff = target_pressures[i] - current_pressures[i];
            if diff.abs() > 0.5 {
                let wheel = match i {
                    0 => "FL",
                    1 => "FR",
                    2 => "RL",
                    3 => "RR",
                    _ => continue,
                };
                adjustments.push(format!("{}: {} {:.1} PSI", wheel, if diff > 0.0 { "Add" } else { "Remove" }, diff.abs()));
            }
        }
        
        adjustments
    }
}

#[derive(Clone)]
pub struct SetupManager {
    pub setups: Arc<Mutex<Vec<CarSetup>>>,
    pub current_car: Arc<Mutex<String>>,
    pub current_track: Arc<Mutex<String>>,
    pub active_setup: Arc<Mutex<Option<CarSetup>>>,
}

impl SetupManager {
    pub fn new() -> Self {
        let manager = Self {
            setups: Arc::new(Mutex::new(Vec::new())),
            current_car: Arc::new(Mutex::new(String::new())),
            current_track: Arc::new(Mutex::new(String::new())),
            active_setup: Arc::new(Mutex::new(None)),
        };
        
        let setups_clone = manager.setups.clone();
        let car_clone = manager.current_car.clone();
        let track_clone = manager.current_track.clone();
        
        thread::spawn(move || {
            loop {
                let car = { car_clone.lock().unwrap().clone() };
                let track = { track_clone.lock().unwrap().clone() };
                
                if !car.is_empty() && !track.is_empty() {
                    let new_setups = scan_folders(&car, &track);
                    let mut lock = setups_clone.lock().unwrap();
                    *lock = new_setups;
                }
                thread::sleep(Duration::from_secs(10));
            }
        });
        
        manager
    }
    
    pub fn set_context(&self, car: &str, track: &str) {
        let mut c = self.current_car.lock().unwrap();
        let mut t = self.current_track.lock().unwrap();
        if *c != car || *t != track {
            *c = car.to_string();
            *t = track.to_string();
        }
    }
    
    pub fn detect_current(&self, fuel: f32, bias: f32, pressures: &[f32; 4], temps: &[f32; 4]) {
        let setups = self.setups.lock().unwrap().clone();
        let mut best_score = 0;
        let mut best_setup = None;
        
        for setup in setups {
            let score = setup.match_score(fuel, bias, pressures, temps);
            if score > best_score && score > 70 {
                best_score = score;
                best_setup = Some(setup);
            }
        }
        
        let mut active_lock = self.active_setup.lock().unwrap();
        *active_lock = best_setup;
    }
    
    pub fn get_active_setup(&self) -> Option<CarSetup> {
        self.active_setup.lock().unwrap().clone()
    }
}

fn scan_folders(car_model: &str, track_name: &str) -> Vec<CarSetup> {
    let mut found = Vec::new();
    
    if let Some(user_dirs) = UserDirs::new() {
        let docs = user_dirs.document_dir().unwrap();
        let base_path = docs.join("Assetto Corsa").join("setups").join(car_model);
        
        // Scan track-specific setups
        scan_single_folder(&base_path.join(track_name), "Track", &mut found);
        
        // Scan generic setups
        scan_single_folder(&base_path.join("generic"), "Generic", &mut found);
    }
    
    found
}

fn scan_single_folder(folder: &std::path::Path, source: &str, list: &mut Vec<CarSetup>) {
    if !folder.exists() {
        return;
    }
    
    for entry in WalkDir::new(folder)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "ini") {
            if let Ok(conf) = Ini::load_from_file(path) {
                let get_u32 = |sec: &str, key: &str| -> u32 {
                    conf.section(Some(sec))
                        .and_then(|s| s.get(key))
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0)
                };
                
                let get_i32 = |sec: &str, key: &str| -> i32 {
                    conf.section(Some(sec))
                        .and_then(|s| s.get(key))
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0)
                };
                
                list.push(CarSetup {
                    name: path.file_stem().unwrap().to_string_lossy().to_string(),
                    path: path.to_path_buf(),
                    source: source.to_string(),
                    
                    fuel: get_u32("FUEL", "VALUE"),
                    brake_bias: get_u32("FRONT_BIAS", "VALUE"),
                    
                    pressure_lf: get_u32("PRESSURE_LF", "VALUE"),
                    pressure_rf: get_u32("PRESSURE_RF", "VALUE"),
                    pressure_lr: get_u32("PRESSURE_LR", "VALUE"),
                    pressure_rr: get_u32("PRESSURE_RR", "VALUE"),
                    
                    wing_1: get_u32("WING_1", "VALUE"),
                    wing_2: get_u32("WING_2", "VALUE"),
                    
                    camber_lf: get_i32("CAMBER_LF", "VALUE"),
                    camber_rf: get_i32("CAMBER_RF", "VALUE"),
                    camber_lr: get_i32("CAMBER_LR", "VALUE"),
                    camber_rr: get_i32("CAMBER_RR", "VALUE"),
                    
                    toe_lf: get_i32("TOE_OUT_LF", "VALUE"),
                    toe_rf: get_i32("TOE_OUT_RF", "VALUE"),
                    toe_lr: get_i32("TOE_OUT_LR", "VALUE"),
                    toe_rr: get_i32("TOE_OUT_RR", "VALUE"),
                    
                    spring_lf: get_u32("SPRING_RATE_LF", "VALUE"),
                    spring_rf: get_u32("SPRING_RATE_RF", "VALUE"),
                    spring_lr: get_u32("SPRING_RATE_LR", "VALUE"),
                    spring_rr: get_u32("SPRING_RATE_RR", "VALUE"),
                    arb_front: get_u32("ARB_FRONT", "VALUE"),
                    arb_rear: get_u32("ARB_REAR", "VALUE"),
                    
                    damp_bump_lf: get_u32("DAMP_BUMP_LF", "VALUE"),
                    damp_rebound_lf: get_u32("DAMP_REBOUND_LF", "VALUE"),
                    damp_bump_lr: get_u32("DAMP_BUMP_LR", "VALUE"),
                    damp_rebound_lr: get_u32("DAMP_REBOUND_LR", "VALUE"),
                    
                    diff_power: get_u32("DIFF_POWER", "VALUE"),
                    diff_coast: get_u32("DIFF_COAST", "VALUE"),
                });
            }
        }
    }
}