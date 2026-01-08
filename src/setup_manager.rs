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
    pub source: String, // "Generic" или название папки (трассы)
    
    // --- Basic ---
    pub fuel: u32,
    pub brake_bias: u32,
    pub engine_limiter: u32,
    
    // --- Tyres (Pressure) ---
    pub pressure_lf: u32,
    pub pressure_rf: u32,
    pub pressure_lr: u32,
    pub pressure_rr: u32,
    
    // --- Aero ---
    pub wing_1: u32,
    pub wing_2: u32,
    
    // --- Alignment ---
    pub camber_lf: i32,
    pub camber_rf: i32,
    pub camber_lr: i32,
    pub camber_rr: i32,
    pub toe_lf: i32,
    pub toe_rf: i32,
    pub toe_lr: i32,
    pub toe_rr: i32,
    
    // --- Suspension ---
    pub spring_lf: u32,
    pub spring_rf: u32,
    pub spring_lr: u32,
    pub spring_rr: u32,
    pub rod_length_lf: i32, // Ride Height
    pub rod_length_rf: i32,
    pub rod_length_lr: i32,
    pub rod_length_rr: i32,
    pub arb_front: u32,
    pub arb_rear: u32,
    pub bump_stop_rate_lf: u32,
    pub bump_stop_rate_rf: u32,
    pub bump_stop_rate_lr: u32,
    pub bump_stop_rate_rr: u32,
    pub packer_range_lf: u32,
    pub packer_range_rf: u32,
    pub packer_range_lr: u32,
    pub packer_range_rr: u32,

    // --- Dampers (Standard) ---
    pub damp_bump_lf: u32,
    pub damp_rebound_lf: u32,
    pub damp_bump_rf: u32,
    pub damp_rebound_rf: u32,
    pub damp_bump_lr: u32,
    pub damp_rebound_lr: u32,
    pub damp_bump_rr: u32,
    pub damp_rebound_rr: u32,
    
    // --- Dampers (Fast) ---
    pub damp_fast_bump_lf: u32,
    pub damp_fast_rebound_lf: u32,
    pub damp_fast_bump_rf: u32,
    pub damp_fast_rebound_rf: u32,
    pub damp_fast_bump_lr: u32,
    pub damp_fast_rebound_lr: u32,
    pub damp_fast_bump_rr: u32,
    pub damp_fast_rebound_rr: u32,

    // --- Drivetrain ---
    pub diff_power: u32,
    pub diff_coast: u32,
    pub final_ratio: u32,
    pub gears: Vec<u32>, // Передаточные числа передач 2, 3, 4...
}

impl CarSetup {
    /// Оценка соответствия сетапа текущему состоянию автомобиля (для авто-детектирования)
    pub fn match_score(&self, current_fuel: f32, current_bias: f32, current_pressures: &[f32; 4]) -> u32 {
        let mut score = 0;
        
        // Топливо (допуск 2 литра)
        if (self.fuel as f32 - current_fuel).abs() < 2.0 {
            score += 30;
        }
        
        // Баланс тормозов (в INI обычно 0-100, в физике 0.0-1.0)
        let bias_file = self.brake_bias as f32 / 100.0;
        if (bias_file - current_bias).abs() < 0.05 {
            score += 25;
        }
        
        // Давление (среднее)
        let avg_p_file = (self.pressure_lf + self.pressure_rf + self.pressure_lr + self.pressure_rr) as f32 / 4.0;
        let avg_p_curr = current_pressures.iter().sum::<f32>() / 4.0;
        
        // В Setup файлах давление часто в PSI, но AC Physics дает его в PSI. Проверяем грубо.
        if (avg_p_file - avg_p_curr).abs() < 2.0 {
            score += 20;
        }
        
        score
    }
}

#[derive(Clone)]
pub struct SetupManager {
    pub setups: Arc<Mutex<Vec<CarSetup>>>,
    pub current_car: Arc<Mutex<String>>,
    pub current_track: Arc<Mutex<String>>,
    pub active_setup_index: Arc<Mutex<Option<usize>>>, // Индекс активного сетапа в векторе
}

impl SetupManager {
    pub fn new() -> Self {
        let manager = Self {
            setups: Arc::new(Mutex::new(Vec::new())),
            current_car: Arc::new(Mutex::new(String::new())),
            current_track: Arc::new(Mutex::new(String::new())),
            active_setup_index: Arc::new(Mutex::new(None)),
        };
        
        let setups_clone = manager.setups.clone();
        let car_clone = manager.current_car.clone();
        let track_clone = manager.current_track.clone();
        
        // Фоновый поток, который следит за изменениями папок (раз в 5 сек)
        thread::spawn(move || {
            loop {
                let car = { car_clone.lock().unwrap().clone() };
                let track = { track_clone.lock().unwrap().clone() };
                
                if !car.is_empty() {
                    let new_setups = scan_folders(&car, &track);
                    let mut lock = setups_clone.lock().unwrap();
                    // Сохраняем, стараясь не ломать порядок если список не сильно изменился
                    *lock = new_setups;
                }
                thread::sleep(Duration::from_secs(5));
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
    
    pub fn get_setups(&self) -> Vec<CarSetup> {
        self.setups.lock().unwrap().clone()
    }

    /// Находит индекс сетапа, который лучше всего подходит для текущей трассы
    pub fn get_best_match_index(&self) -> Option<usize> {
        let setups = self.setups.lock().unwrap();
        let track_name = self.current_track.lock().unwrap();
        
        if setups.is_empty() { return None; }
        
        // 1. Ищем точное совпадение папки с именем трассы
        if let Some(idx) = setups.iter().position(|s| s.source == *track_name) {
            return Some(idx);
        }
        
        // 2. Если нет, ищем файлы, в названии которых есть имя трассы
        if !track_name.is_empty() && *track_name != "-" {
            if let Some(idx) = setups.iter().position(|s| s.name.to_lowercase().contains(&track_name.to_lowercase())) {
                return Some(idx);
            }
        }
        
        // 3. Если совсем ничего нет, возвращаем первый (обычно Generic)
        Some(0)
    }

    pub fn get_setup_by_index(&self, index: usize) -> Option<CarSetup> {
        let setups = self.setups.lock().unwrap();
        setups.get(index).cloned()
    }
    
    pub fn detect_current(&self, fuel: f32, bias: f32, pressures: &[f32; 4], _temps: &[f32; 4]) {
        let setups = self.setups.lock().unwrap();
        let mut best_score = 0;
        let mut best_idx = None;
        
        for (i, setup) in setups.iter().enumerate() {
            let score = setup.match_score(fuel, bias, pressures);
            if score > best_score && score > 60 { // Порог уверенности
                best_score = score;
                best_idx = Some(i);
            }
        }
        
        let mut active_idx = self.active_setup_index.lock().unwrap();
        *active_idx = best_idx;
    }
    
    pub fn get_active_setup(&self) -> Option<CarSetup> {
        let idx = *self.active_setup_index.lock().unwrap();
        let setups = self.setups.lock().unwrap();
        if let Some(i) = idx {
            if i < setups.len() {
                return Some(setups[i].clone());
            }
        }
        None
    }
}

fn scan_folders(car_model: &str, track_name: &str) -> Vec<CarSetup> {
    let mut found = Vec::new();
    
    if let Some(user_dirs) = UserDirs::new() {
        let docs = user_dirs.document_dir().unwrap();
        // Путь: Documents/Assetto Corsa/setups/<car_model>
        let base_path = docs.join("Assetto Corsa").join("setups").join(car_model);
        
        // 1. Приоритет: Папка текущей трассы
        if !track_name.is_empty() && track_name != "-" {
            scan_single_folder(&base_path.join(track_name), track_name, &mut found);
        }
        
        // 2. Generic сетапы
        scan_single_folder(&base_path.join("generic"), "Generic", &mut found);
        
        // 3. Можно добавить сканирование всех остальных папок как "Other", если нужно
    }
    
    found
}

fn scan_single_folder(folder: &std::path::Path, source: &str, list: &mut Vec<CarSetup>) {
    if !folder.exists() { return; }
    
    for entry in WalkDir::new(folder).max_depth(1).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "ini") {
            if let Ok(conf) = Ini::load_from_file(path) {
                // Хелперы для безопасного чтения
                let get = |sec: &str, key: &str| -> u32 {
                    conf.section(Some(sec)).and_then(|s| s.get(key)).and_then(|v| v.parse().ok()).unwrap_or(0)
                };
                let get_i = |sec: &str, key: &str| -> i32 {
                    conf.section(Some(sec)).and_then(|s| s.get(key)).and_then(|v| v.parse().ok()).unwrap_or(0)
                };

                // Читаем передачи динамически (INTERNAL_GEAR_2...8)
                let mut gears = Vec::new();
                for i in 2..=8 {
                    let key = format!("INTERNAL_GEAR_{}", i);
                    if let Some(val) = conf.section(Some(key.as_str())).and_then(|s| s.get("VALUE")) {
                         if let Ok(v) = val.parse::<u32>() {
                             gears.push(v);
                         }
                    }
                }

                list.push(CarSetup {
                    name: path.file_stem().unwrap().to_string_lossy().to_string(),
                    path: path.to_path_buf(),
                    source: source.to_string(),
                    
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
                    
                    bump_stop_rate_lf: get("BUMP_STOP_RATE_LF", "VALUE"),
                    bump_stop_rate_rf: get("BUMP_STOP_RATE_RF", "VALUE"),
                    bump_stop_rate_lr: get("BUMP_STOP_RATE_LR", "VALUE"),
                    bump_stop_rate_rr: get("BUMP_STOP_RATE_RR", "VALUE"),
                    packer_range_lf: get("PACKER_RANGE_LF", "VALUE"),
                    packer_range_rf: get("PACKER_RANGE_RF", "VALUE"),
                    packer_range_lr: get("PACKER_RANGE_LR", "VALUE"),
                    packer_range_rr: get("PACKER_RANGE_RR", "VALUE"),
                    
                    damp_bump_lf: get("DAMP_BUMP_LF", "VALUE"),
                    damp_bump_rf: get("DAMP_BUMP_RF", "VALUE"),
                    damp_bump_lr: get("DAMP_BUMP_LR", "VALUE"),
                    damp_bump_rr: get("DAMP_BUMP_RR", "VALUE"),
                    
                    damp_rebound_lf: get("DAMP_REBOUND_LF", "VALUE"),
                    damp_rebound_rf: get("DAMP_REBOUND_RF", "VALUE"),
                    damp_rebound_lr: get("DAMP_REBOUND_LR", "VALUE"),
                    damp_rebound_rr: get("DAMP_REBOUND_RR", "VALUE"),
                    
                    damp_fast_bump_lf: get("DAMP_FAST_BUMP_LF", "VALUE"),
                    damp_fast_bump_rf: get("DAMP_FAST_BUMP_RF", "VALUE"),
                    damp_fast_bump_lr: get("DAMP_FAST_BUMP_LR", "VALUE"),
                    damp_fast_bump_rr: get("DAMP_FAST_BUMP_RR", "VALUE"),
                    
                    damp_fast_rebound_lf: get("DAMP_FAST_REBOUND_LF", "VALUE"),
                    damp_fast_rebound_rf: get("DAMP_FAST_REBOUND_RF", "VALUE"),
                    damp_fast_rebound_lr: get("DAMP_FAST_REBOUND_LR", "VALUE"),
                    damp_fast_rebound_rr: get("DAMP_FAST_REBOUND_RR", "VALUE"),
                    
                    diff_power: get("DIFF_POWER", "VALUE"),
                    diff_coast: get("DIFF_COAST", "VALUE"),
                    final_ratio: get("FINAL_RATIO", "VALUE"),
                    gears,
                });
            }
        }
    }
}