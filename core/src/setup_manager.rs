use directories_next::UserDirs;
use ini::Ini;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use walkdir::WalkDir;

const GITHUB_USER_REPO: &str = "Rgosh/ac-setups";
const GITHUB_BRANCH: &str = "main";

#[derive(Debug, Clone)]
pub struct SetupDiffItem {
    pub name: String,
    pub current: f32,
    pub reference: f32,
    pub diff: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CarSetup {
    pub name: String,
    #[serde(skip)]
    pub path: PathBuf,
    pub source: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub credits: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub car_id: String,
    #[serde(skip)]
    pub is_remote: bool,

    #[serde(default)]
    pub fuel: u32,
    #[serde(default)]
    pub brake_bias: u32,
    #[serde(default)]
    pub engine_limiter: u32,
    #[serde(default)]
    pub pressure_lf: u32,
    #[serde(default)]
    pub pressure_rf: u32,
    #[serde(default)]
    pub pressure_lr: u32,
    #[serde(default)]
    pub pressure_rr: u32,
    #[serde(default)]
    pub wing_1: u32,
    #[serde(default)]
    pub wing_2: u32,
    #[serde(default)]
    pub camber_lf: i32,
    #[serde(default)]
    pub camber_rf: i32,
    #[serde(default)]
    pub camber_lr: i32,
    #[serde(default)]
    pub camber_rr: i32,
    #[serde(default)]
    pub toe_lf: i32,
    #[serde(default)]
    pub toe_rf: i32,
    #[serde(default)]
    pub toe_lr: i32,
    #[serde(default)]
    pub toe_rr: i32,
    #[serde(default)]
    pub spring_lf: u32,
    #[serde(default)]
    pub spring_rf: u32,
    #[serde(default)]
    pub spring_lr: u32,
    #[serde(default)]
    pub spring_rr: u32,
    #[serde(default)]
    pub rod_length_lf: i32,
    #[serde(default)]
    pub rod_length_rf: i32,
    #[serde(default)]
    pub rod_length_lr: i32,
    #[serde(default)]
    pub rod_length_rr: i32,
    #[serde(default)]
    pub arb_front: u32,
    #[serde(default)]
    pub arb_rear: u32,
    #[serde(default)]
    pub damp_bump_lf: u32,
    #[serde(default)]
    pub damp_rebound_lf: u32,
    #[serde(default)]
    pub damp_bump_rf: u32,
    #[serde(default)]
    pub damp_rebound_rf: u32,
    #[serde(default)]
    pub damp_bump_lr: u32,
    #[serde(default)]
    pub damp_rebound_lr: u32,
    #[serde(default)]
    pub damp_bump_rr: u32,
    #[serde(default)]
    pub damp_rebound_rr: u32,
    #[serde(default)]
    pub diff_power: u32,
    #[serde(default)]
    pub diff_coast: u32,
    #[serde(default)]
    pub final_ratio: u32,
    #[serde(default)]
    pub gears: Vec<u32>,
}

impl CarSetup {
    pub fn match_score(
        &self,
        current_fuel: f32,
        current_bias: f32,
        current_pressures: &[f32; 4],
    ) -> u32 {
        let mut score = 0;
        if (self.fuel as f32 - current_fuel).abs() < 2.0 {
            score += 30;
        }
        let bias_file = self.brake_bias as f32 / 100.0;
        if (bias_file - current_bias).abs() < 0.05 {
            score += 25;
        }
        let avg_p_file = (self.pressure_lf + self.pressure_rf + self.pressure_lr + self.pressure_rr)
            as f32
            / 4.0;
        let avg_p_curr = current_pressures.iter().sum::<f32>() / 4.0;
        if (avg_p_file - avg_p_curr).abs() < 2.0 {
            score += 20;
        }
        score
    }

    pub fn generate_diff(&self, reference: &CarSetup) -> Vec<SetupDiffItem> {
        let mut diffs = Vec::new();
        let mut check = |name: &str, cur: f32, ref_val: f32| {
            if (cur - ref_val).abs() > 0.001 {
                diffs.push(SetupDiffItem {
                    name: name.to_string(),
                    current: cur,
                    reference: ref_val,
                    diff: cur - ref_val,
                });
            }
        };

        check("Front Wing", self.wing_1 as f32, reference.wing_1 as f32);
        check("Rear Wing", self.wing_2 as f32, reference.wing_2 as f32);
        check(
            "Front ARB",
            self.arb_front as f32,
            reference.arb_front as f32,
        );
        check("Rear ARB", self.arb_rear as f32, reference.arb_rear as f32);
        check("Fuel (L)", self.fuel as f32, reference.fuel as f32);
        check(
            "Brake Bias (%)",
            self.brake_bias as f32,
            reference.brake_bias as f32,
        );
        check(
            "Camber FL",
            self.camber_lf as f32 / 10.0,
            reference.camber_lf as f32 / 10.0,
        );
        check(
            "Camber FR",
            self.camber_rf as f32 / 10.0,
            reference.camber_rf as f32 / 10.0,
        );
        check(
            "Camber RL",
            self.camber_lr as f32 / 10.0,
            reference.camber_lr as f32 / 10.0,
        );
        check(
            "Camber RR",
            self.camber_rr as f32 / 10.0,
            reference.camber_rr as f32 / 10.0,
        );
        check(
            "Spring FL",
            self.spring_lf as f32,
            reference.spring_lf as f32,
        );
        check(
            "Spring RL",
            self.spring_lr as f32,
            reference.spring_lr as f32,
        );
        check(
            "Pressure FL",
            self.pressure_lf as f32,
            reference.pressure_lf as f32,
        );
        check(
            "Pressure RL",
            self.pressure_lr as f32,
            reference.pressure_lr as f32,
        );

        diffs
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestItem {
    pub id: String,
    pub count: usize,
    pub authors: Vec<String>,
}

#[derive(Clone)]
pub struct SetupManager {
    pub setups: Arc<Mutex<Vec<CarSetup>>>,
    pub current_car: Arc<Mutex<String>>,
    pub current_track: Arc<Mutex<String>>,
    pub active_setup_index: Arc<Mutex<Option<usize>>>,

    pub browser_active: Arc<Mutex<bool>>,
    pub manifest: Arc<Mutex<Vec<ManifestItem>>>,
    pub browser_car_idx: Arc<Mutex<usize>>,
    pub browser_setup_idx: Arc<Mutex<usize>>,
    pub browser_focus_col: Arc<Mutex<u8>>,
    pub browser_setups: Arc<Mutex<Vec<CarSetup>>>,

    pub details_scroll: Arc<Mutex<usize>>,
    pub loading_tick: Arc<Mutex<usize>>,

    pub server_fetch_done: Arc<Mutex<bool>>,
    pub last_status: Arc<Mutex<String>>,
}

trait SafeLock<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> SafeLock<T> for Mutex<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl SetupManager {
    pub fn new() -> Self {
        let manager = Self {
            setups: Arc::new(Mutex::new(Vec::new())),
            current_car: Arc::new(Mutex::new(String::new())),
            current_track: Arc::new(Mutex::new(String::new())),
            active_setup_index: Arc::new(Mutex::new(None)),

            browser_active: Arc::new(Mutex::new(false)),
            manifest: Arc::new(Mutex::new(Vec::new())),
            browser_car_idx: Arc::new(Mutex::new(0)),
            browser_setup_idx: Arc::new(Mutex::new(0)),
            browser_focus_col: Arc::new(Mutex::new(0)),
            browser_setups: Arc::new(Mutex::new(Vec::new())),

            details_scroll: Arc::new(Mutex::new(0)),
            loading_tick: Arc::new(Mutex::new(0)),

            server_fetch_done: Arc::new(Mutex::new(false)),
            last_status: Arc::new(Mutex::new(String::new())),
        };

        let setups_clone = manager.setups.clone();
        let car_clone = manager.current_car.clone();
        let track_clone = manager.current_track.clone();
        let fetch_flag = manager.server_fetch_done.clone();
        let manifest_clone = manager.manifest.clone();

        thread::spawn(move || {
            let mut last_car = String::new();
            let mut last_track = String::new();

            if let Some(m) = fetch_manifest() {
                *manifest_clone.safe_lock() = m;
            }

            loop {
                let is_empty = manifest_clone.safe_lock().is_empty();

                if is_empty {
                    if let Some(m) = fetch_manifest() {
                        *manifest_clone.safe_lock() = m;
                    }
                }

                let car = car_clone.safe_lock().clone();
                let track = track_clone.safe_lock().clone();

                if !car.is_empty() {
                    if car != last_car || track != last_track {
                        *fetch_flag.safe_lock() = false;
                        last_car = car.clone();
                        last_track = track.clone();
                        setups_clone.safe_lock().clear();
                    }

                    let mut all_setups = scan_folders(&car, &track);

                    let mut needs_fetch = false;
                    {
                        if !*fetch_flag.safe_lock() {
                            needs_fetch = true;
                        }
                    }

                    if needs_fetch {
                        if let Some(mut server_setups) = fetch_server_setups(&car) {
                            for s in &mut server_setups {
                                if s.car_id.is_empty() {
                                    s.car_id = car.clone();
                                }
                            }
                            all_setups.append(&mut server_setups);
                        }
                        *fetch_flag.safe_lock() = true;
                    } else {
                        let existing = setups_clone.safe_lock();
                        let remotes: Vec<CarSetup> = existing
                            .iter()
                            .filter(|s| s.is_remote && s.car_id == car)
                            .cloned()
                            .collect();
                        drop(existing);
                        all_setups.extend(remotes);
                    }

                    *setups_clone.safe_lock() = all_setups;
                }
                thread::sleep(Duration::from_secs(5));
            }
        });

        manager
    }

    pub fn scroll_details(&self, delta: i32) {
        let mut scroll = self.details_scroll.safe_lock();
        if delta < 0 {
            if *scroll > 0 {
                *scroll -= 1;
            }
        } else {
            *scroll += 1;
        }
    }

    pub fn is_installed(&self, setup: &CarSetup, target_car: &str) -> bool {
        if let Some(user_dirs) = UserDirs::new() {
            if let Some(docs) = user_dirs.document_dir() {
                let safe_name = setup
                    .name
                    .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
                let file_name = format!("{}_{}.ini", setup.author, safe_name);
                let path = docs
                    .join("Assetto Corsa")
                    .join("setups")
                    .join(target_car)
                    .join("downloaded")
                    .join(file_name);
                return path.exists();
            }
        }
        false
    }

    pub fn get_manifest(&self) -> Vec<ManifestItem> {
        self.manifest.safe_lock().clone()
    }

    pub fn get_browser_setups(&self) -> Vec<CarSetup> {
        self.browser_setups.safe_lock().clone()
    }

    pub fn load_browser_car(&self) {
        let idx = *self.browser_car_idx.safe_lock();
        let manifest = self.manifest.safe_lock();
        if idx < manifest.len() {
            let car_id = &manifest[idx].id;

            let car_id_clone = car_id.clone();
            drop(manifest);

            if let Some(mut setups) = fetch_server_setups(&car_id_clone) {
                for s in &mut setups {
                    s.car_id = car_id_clone.clone();
                }
                *self.browser_setups.safe_lock() = setups;
                *self.browser_setup_idx.safe_lock() = 0;
                *self.details_scroll.safe_lock() = 0;
            }
        }
    }

    pub fn get_browser_selected_setup(&self) -> Option<CarSetup> {
        let idx = *self.browser_setup_idx.safe_lock();
        let setups = self.browser_setups.safe_lock();
        setups.get(idx).cloned()
    }

    pub fn get_browser_target_car(&self) -> String {
        let idx = *self.browser_car_idx.safe_lock();
        let manifest = self.manifest.safe_lock();
        if idx < manifest.len() {
            manifest[idx].id.clone()
        } else {
            "unknown".to_string()
        }
    }

    pub fn set_context(&self, car: &str, track: &str) {
        let mut c = self.current_car.safe_lock();
        let mut t = self.current_track.safe_lock();
        if *c != car || *t != track {
            *c = car.to_string();
            *t = track.to_string();
            *self.server_fetch_done.safe_lock() = false;
            *self.details_scroll.safe_lock() = 0;
        }
    }

    pub fn get_setups(&self) -> Vec<CarSetup> {
        self.setups.safe_lock().clone()
    }

    pub fn get_best_match_index(&self) -> Option<usize> {
        let setups = self.setups.safe_lock();
        let track_name = self.current_track.safe_lock();
        if setups.is_empty() {
            return None;
        }
        if let Some(idx) = setups
            .iter()
            .position(|s| s.source == *track_name && !s.is_remote)
        {
            return Some(idx);
        }
        Some(0)
    }

    pub fn get_setup_by_index(&self, index: usize) -> Option<CarSetup> {
        let setups = self.setups.safe_lock();
        setups.get(index).cloned()
    }

    pub fn download_setup(&self, setup: &CarSetup, target_car: &str) -> bool {
        if !setup.is_remote {
            return false;
        }

        let mut status_lock = self.last_status.safe_lock();

        if !setup.car_id.is_empty() && setup.car_id != target_car {
            *status_lock = format!("Err: Car mismatch! ({} != {})", setup.car_id, target_car);
            return false;
        }

        if let Some(user_dirs) = UserDirs::new() {
            let docs = match user_dirs.document_dir() {
                Some(d) => d,
                None => {
                    *status_lock = "Err: No document directory found".to_string();
                    return false;
                }
            };

            let target_dir = docs
                .join("Assetto Corsa")
                .join("setups")
                .join(target_car)
                .join("downloaded");

            if fs::create_dir_all(&target_dir).is_err() {
                *status_lock = "Err: Could not create directory".to_string();
                return false;
            }

            let safe_name = setup
                .name
                .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
            let file_name = format!("{}_{}.ini", setup.author, safe_name);
            let file_path = target_dir.join(file_name);
            let content = generate_ini_content(setup);

            match fs::write(&file_path, content) {
                Ok(_) => {
                    *status_lock = format!("✅ SAVED to {}!", target_car);
                    drop(status_lock);
                    *self.server_fetch_done.safe_lock() = false;
                    return true;
                }
                Err(e) => {
                    *status_lock = format!("Err: {}", e);
                }
            }
        } else {
            *status_lock = "Err: Could not determine user dirs".to_string();
        }
        false
    }

    pub fn get_status_message(&self) -> String {
        self.last_status.safe_lock().clone()
    }

    pub fn detect_current(&self, fuel: f32, bias: f32, pressures: &[f32; 4], _temps: &[f32; 4]) {
        let setups = self.setups.safe_lock();
        let mut best_score = 0;
        let mut best_idx = None;
        for (i, setup) in setups.iter().enumerate() {
            if setup.is_remote {
                continue;
            }
            let score = setup.match_score(fuel, bias, pressures);
            if score > best_score && score > 60 {
                best_score = score;
                best_idx = Some(i);
            }
        }
        let mut active_idx = self.active_setup_index.safe_lock();
        *active_idx = best_idx;
    }

    pub fn get_active_setup(&self) -> Option<CarSetup> {
        let idx = *self.active_setup_index.safe_lock();
        let setups = self.setups.safe_lock();
        if let Some(i) = idx {
            if i < setups.len() {
                return Some(setups[i].clone());
            }
        }
        None
    }
}

fn fetch_manifest() -> Option<Vec<ManifestItem>> {
    let client = reqwest::blocking::Client::new();
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/manifest.json",
        GITHUB_USER_REPO, GITHUB_BRANCH
    );
    if let Ok(resp) = client.get(&url).timeout(Duration::from_secs(5)).send() {
        if resp.status().is_success() {
            return resp.json().ok();
        }
    }
    None
}

fn fetch_server_setups(car: &str) -> Option<Vec<CarSetup>> {
    let client = reqwest::blocking::Client::new();
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}.json",
        GITHUB_USER_REPO, GITHUB_BRANCH, car
    );
    if let Ok(resp) = client.get(&url).timeout(Duration::from_secs(5)).send() {
        if resp.status().is_success() {
            if let Ok(mut setups) = resp.json::<Vec<CarSetup>>() {
                for s in &mut setups {
                    s.is_remote = true;
                    s.car_id = car.to_string();
                    if s.author.is_empty() {
                        s.author = "Server".to_string();
                    }
                }
                return Some(setups);
            }
        }
    }
    None
}

fn scan_folders(car_model: &str, track_name: &str) -> Vec<CarSetup> {
    let mut found = Vec::new();
    if let Some(user_dirs) = UserDirs::new() {
        if let Some(docs) = user_dirs.document_dir() {
            let base_path = docs.join("Assetto Corsa").join("setups").join(car_model);
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
    }
    found
}

fn scan_single_folder(
    folder: &std::path::Path,
    source: &str,
    car_id: &str,
    list: &mut Vec<CarSetup>,
) {
    if !folder.exists() {
        return;
    }
    for entry in WalkDir::new(folder)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "ini") {
            if let Ok(conf) = Ini::load_from_file(path) {
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
                    {
                        if let Ok(v) = val.parse::<u32>() {
                            gears.push(v);
                        }
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
}

fn generate_ini_content(s: &CarSetup) -> String {
    let mut out = String::new();
    if !s.notes.is_empty() {
        out.push_str(&format!("[NOTES]\nVALUE={}\n\n", s.notes));
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
