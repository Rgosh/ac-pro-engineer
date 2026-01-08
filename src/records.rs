use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::content_manager::CarSpecs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRecord {
    pub car_id: String,
    pub track_name: String,
    pub track_config: String, // "" если основная конфигурация
    pub time_ms: i32,
    pub driver_name: String,
    pub date: String,
    pub source: String, // "User Best", "World Record (DB)", "Physics Est."
}

pub struct RecordManager {
    pub records: HashMap<String, TrackRecord>, // Локальные рекорды пользователя
    pub static_db: HashMap<String, i32>,       // Встроенная база реальных рекордов
    pub db_path: PathBuf,
}

impl RecordManager {
    pub fn new() -> Self {
        let db_path = PathBuf::from("./data/records.json");
        let mut manager = Self {
            records: HashMap::new(),
            static_db: HashMap::new(),
            db_path,
        };
        manager.init_static_db(); // Загружаем базу WR
        manager.load();
        manager
    }

    // --- БАЗА РЕАЛЬНЫХ РЕКОРДОВ (Hardcoded Gold Tier Times) ---
    // Данные собраны с архивов RSR, SRO и SimracingGP
    // Ключ: "car_id|track_name|track_config"
    fn init_static_db(&mut self) {
        let mut db = HashMap::new();

        // ==========================================
        // CLASS: GT3 (McLaren, BMW Z4, SLS, etc.)
        // ==========================================
        
        // --- SPA FRANCORCHAMPS (spa) ---
        // Top Aliens: ~2:16.xxx, Good Pace: 2:18.xxx
        db.insert("mclaren_mp412c_gt3|spa|".into(), 136800); // 2:16.8
        db.insert("bmw_z4_gt3|spa|".into(), 137200);         // 2:17.2
        db.insert("mercedes_sls_gt3|spa|".into(), 137500);   // 2:17.5
        db.insert("ferrari_488_gt3|spa|".into(), 136500);    // 2:16.5
        db.insert("ks_nissan_gtr_gt3|spa|".into(), 137000);  // 2:17.0

        // --- MONZA (monza) ---
        // Top Aliens: ~1:47.xxx
        db.insert("mclaren_mp412c_gt3|monza|".into(), 107500); // 1:47.5
        db.insert("bmw_z4_gt3|monza|".into(), 108100);         // 1:48.1
        db.insert("mercedes_sls_gt3|monza|".into(), 107900);   // 1:47.9
        db.insert("ks_nissan_gtr_gt3|monza|".into(), 107800);  // 1:47.8

        // --- IMOLA (imola) ---
        // Top Aliens: ~1:41.xxx - 1:42.xxx
        db.insert("mclaren_mp412c_gt3|imola|".into(), 102100); // 1:42.1
        db.insert("bmw_z4_gt3|imola|".into(), 102400);         // 1:42.4
        db.insert("mercedes_sls_gt3|imola|".into(), 102600);   // 1:42.6

        // --- NURBURGRING GP (nurburgring) ---
        // Top Aliens: ~1:55.xxx
        db.insert("mclaren_mp412c_gt3|nurburgring|".into(), 115200); // 1:55.2
        db.insert("bmw_z4_gt3|nurburgring|".into(), 115500);         // 1:55.5
        db.insert("mercedes_sls_gt3|nurburgring|".into(), 115800);   // 1:55.8

        // ==========================================
        // CLASS: GT2 (BMW M3, Ferrari 458, P4/5)
        // ==========================================
        
        // --- IMOLA ---
        db.insert("bmw_m3_gt2|imola|".into(), 103100);        // 1:43.1
        db.insert("ferrari_458_gt2|imola|".into(), 102500);   // 1:42.5
        db.insert("p4-5_2011|imola|".into(), 103500);         // 1:43.5 (P4/5 Competizione)

        // --- MONZA ---
        db.insert("bmw_m3_gt2|monza|".into(), 108500);        // 1:48.5
        db.insert("ferrari_458_gt2|monza|".into(), 107800);   // 1:47.8
        db.insert("p4-5_2011|monza|".into(), 109000);         // 1:49.0

        // --- SPA ---
        db.insert("bmw_m3_gt2|spa|".into(), 138200);          // 2:18.2
        db.insert("ferrari_458_gt2|spa|".into(), 137500);     // 2:17.5

        // ==========================================
        // CLASS: OPEN WHEEL / SPECIAL (Lotus Exos)
        // ==========================================

        // Lotus Exos 125 Stage 1 (Ультра-быстрая)
        db.insert("lotus_exos_125_s1|imola|".into(), 81500);       // 1:21.5
        db.insert("lotus_exos_125_s1|monza|".into(), 78500);       // 1:18.5
        db.insert("lotus_exos_125_s1|spa|".into(), 109500);        // 1:49.5
        db.insert("lotus_exos_125_s1|nurburgring|".into(), 93000); // 1:33.0
        db.insert("lotus_exos_125_s1|mugello|".into(), 83000);     // 1:23.0

        // ==========================================
        // CLASS: ROAD / TRACK DAY
        // ==========================================
        
        // KTM X-Bow R
        db.insert("ktm_xbow_r|magione|".into(), 78800);       // 1:18.8
        db.insert("ktm_xbow_r|imola|".into(), 113500);        // 1:53.5

        // BMW M3 E30
        db.insert("bmw_m3_e30|magione|".into(), 87100);       // 1:27.1
        
        // Ferrari 458 Italia (Road)
        db.insert("ferrari_458|imola|".into(), 114500);       // 1:54.5
        db.insert("ferrari_458|monza|".into(), 118200);       // 1:58.2

        self.static_db = db;
    }

    pub fn load(&mut self) {
        if self.db_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.db_path) {
                if let Ok(list) = serde_json::from_str::<Vec<TrackRecord>>(&content) {
                    for rec in list {
                        let key = format!("{}|{}|{}", rec.car_id, rec.track_name, rec.track_config);
                        self.records.insert(key, rec);
                    }
                }
            }
        }
    }

    pub fn save(&self) {
        if let Some(parent) = self.db_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let list: Vec<&TrackRecord> = self.records.values().collect();
        if let Ok(content) = serde_json::to_string_pretty(&list) {
            fs::write(&self.db_path, content).ok();
        }
    }

    pub fn get_or_calculate_record(
        &mut self, 
        car_id: &str, 
        track_name: &str, 
        track_config: &str, 
        specs: Option<&CarSpecs>,
        track_len_m: f32
    ) -> TrackRecord {
        let key = format!("{}|{}|{}", car_id, track_name, track_config);
        
        // 1. Проверяем ЛИЧНЫЙ рекорд пользователя.
        // Если он быстрее базы данных, то используем его (пользователь - Alien!)
        if let Some(user_rec) = self.records.get(&key) {
            if let Some(static_time) = self.static_db.get(&key) {
                if user_rec.time_ms < *static_time {
                    return user_rec.clone();
                }
            } else {
                return user_rec.clone();
            }
        }

        // 2. Ищем в STATIC DB (Наша база реальных рекордов)
        if let Some(static_time) = self.static_db.get(&key) {
            return TrackRecord {
                car_id: car_id.to_string(),
                track_name: track_name.to_string(),
                track_config: track_config.to_string(),
                time_ms: *static_time,
                driver_name: "World Record".to_string(),
                date: "Global DB".to_string(),
                source: "World Record (DB)".to_string(),
            };
        }

        // 3. FALLBACK: Если машины/трассы нет в базе, рассчитываем физически
        // Улучшенная формула с поправкой на класс машины
        let calculated_time = if let Some(s) = specs {
            self.calculate_theoretical_time(s, track_len_m, car_id)
        } else {
            // Если нет данных о машине, берем среднюю 130 км/ч
            if track_len_m > 0.0 { (track_len_m / 36.0 * 1000.0) as i32 } else { 120000 }
        };

        TrackRecord {
            car_id: car_id.to_string(),
            track_name: track_name.to_string(),
            track_config: track_config.to_string(),
            time_ms: calculated_time,
            driver_name: "AI Calculation".to_string(),
            date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            source: "Physics Est.".to_string(),
        }
    }

    fn calculate_theoretical_time(&self, specs: &CarSpecs, track_len: f32, car_id: &str) -> i32 {
        if specs.power_hp == 0.0 || specs.weight_kg == 0.0 || track_len == 0.0 {
            return 120000;
        }

        // Вес на мощность (кг/лс)
        let pwr = specs.weight_kg / specs.power_hp; 
        
        // Базовая скорость для "идеальной" трассы типа Монцы
        // GT3 (~2.5 kg/hp) -> ~180 km/h avg (50 m/s)
        // Road Car (~6.0 kg/hp) -> ~120 km/h avg (33 m/s)
        // Formula (~1.0 kg/hp) -> ~230 km/h avg (64 m/s)
        
        let mut base_speed_ms = 68.0 - (pwr * 4.2);
        
        // Коррекция по ID машины (если известно что это GT3, но нет в базе рекордов)
        if car_id.contains("gt3") {
            base_speed_ms = base_speed_ms.max(48.0); // Мин. средняя для GT3
        } else if car_id.contains("f1") || car_id.contains("formula") || car_id.contains("exos") {
            base_speed_ms = base_speed_ms.max(60.0); // Мин. для формул
        }

        // Коррекция по длине трассы 
        // Короткие трассы (Magione) медленнее из-за затычных поворотов
        // Длинные (Spa) быстрее
        if track_len < 3000.0 {
            base_speed_ms *= 0.82; 
        } else if track_len > 5500.0 {
            base_speed_ms *= 1.08; 
        }

        let speed_ms = base_speed_ms.clamp(20.0, 95.0);
        let time_sec = track_len / speed_ms;
        
        (time_sec * 1000.0) as i32
    }
    
    pub fn update_if_faster(&mut self, record: TrackRecord) {
        let key = format!("{}|{}|{}", record.car_id, record.track_name, record.track_config);
        
        let should_update = if let Some(existing) = self.records.get(&key) {
            record.time_ms < existing.time_ms
        } else {
            true
        };

        if should_update {
            self.records.insert(key, record);
            self.save();
        }
    }
}