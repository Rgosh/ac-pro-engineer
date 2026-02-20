use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::content_manager::CarSpecs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRecord {
    pub car_id: String,
    pub track_name: String,
    pub track_config: String, 
    pub time_ms: i32,
    pub driver_name: String,
    pub date: String,
    pub source: String, 
}

pub struct RecordManager {
    pub records: HashMap<String, TrackRecord>, 
    pub static_db: HashMap<String, i32>,       
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
        manager.init_static_db(); 
        manager.load();
        manager
    }

    fn init_static_db(&mut self) {
        let mut db = HashMap::new();


        // Aliens do 2:15-2:16, Good lap 2:17-2:18
        db.insert("mclaren_mp412c_gt3|spa|".to_string(), 136800); 
        db.insert("bmw_z4_gt3|spa|".to_string(), 137200);         
        db.insert("mercedes_sls_gt3|spa|".to_string(), 137500);  
        db.insert("ferrari_488_gt3|spa|".to_string(), 136500);   
        db.insert("ks_nissan_gtr_gt3|spa|".to_string(), 137000);  
        db.insert("ks_lamborghini_huracan_gt3|spa|".to_string(), 136900);
        db.insert("ks_porsche_911_gt3_r_2016|spa|".to_string(), 136400); 
        db.insert("ks_audi_r8_lms_2016|spa|".to_string(), 136600);


        // Aliens 1:47.xxx
        db.insert("mclaren_mp412c_gt3|monza|".to_string(), 107500); 
        db.insert("bmw_z4_gt3|monza|".to_string(), 108100);         
        db.insert("ks_nissan_gtr_gt3|monza|".to_string(), 107800);  
        db.insert("ks_lamborghini_huracan_gt3|monza|".to_string(), 107600);
        db.insert("ks_ferrari_488_gt3|monza|".to_string(), 107400);



        db.insert("mclaren_mp412c_gt3|imola|".to_string(), 102100); 
        db.insert("bmw_z4_gt3|imola|".to_string(), 102400);         
        db.insert("mercedes_sls_gt3|imola|".to_string(), 102600);
        db.insert("ks_porsche_911_gt3_r_2016|imola|".to_string(), 101800);


        // ~1:55 - 1:56
        db.insert("mclaren_mp412c_gt3|ks_nurburgring|layout_gp".to_string(), 115200);
        db.insert("bmw_z4_gt3|ks_nurburgring|layout_gp".to_string(), 115500);
        db.insert("ks_audi_r8_lms_2016|ks_nurburgring|layout_gp".to_string(), 114900);

        // --- GT2 CLASS ---
        db.insert("bmw_m3_gt2|imola|".to_string(), 103100);        
        db.insert("ferrari_458_gt2|imola|".to_string(), 102500); 
        db.insert("p4-5_2011|imola|".to_string(), 103500);         
        db.insert("bmw_m3_gt2|monza|".to_string(), 108500);        
        db.insert("ferrari_458_gt2|monza|".to_string(), 107800);   
        db.insert("bmw_m3_gt2|spa|".to_string(), 138500);          

        // Lotus Exos 125 S1
        db.insert("lotus_exos_125_s1|imola|".to_string(), 81500);       
        db.insert("lotus_exos_125_s1|monza|".to_string(), 78500);      
        db.insert("lotus_exos_125_s1|spa|".to_string(), 108000);        
        
        // Ferrari F2004 (F1)
        db.insert("ks_ferrari_f2004|monza|".to_string(), 79500);       
        db.insert("ks_ferrari_f2004|spa|".to_string(), 102500);         
        db.insert("ks_ferrari_f2004|imola|".to_string(), 73000);       

        // Porsche 919 Hybrid (LMP1)
        db.insert("ks_porsche_919_hybrid_2016|spa|".to_string(), 115000); 
        db.insert("ks_porsche_919_hybrid_2016|monza|".to_string(), 85000);

        // --- ROAD CARS (TRACK DAY) ---
        // KTM X-Bow R
        db.insert("ktm_xbow_r|magione|".to_string(), 78800);     
        db.insert("ktm_xbow_r|imola|".to_string(), 113500);        
        
        // BMW M3 E30
        db.insert("bmw_m3_e30|magione|".to_string(), 87100);       
        db.insert("bmw_m3_e30|imola|".to_string(), 125000);        

        // Ferrari 458 Italia (Road)
        db.insert("ferrari_458|imola|".to_string(), 114500);       
        db.insert("ferrari_458|monza|".to_string(), 118200);      
        db.insert("ferrari_458|spa|".to_string(), 155000);         

        // Mazda MX5 Cup
        db.insert("ks_mazda_mx5_cup|imola|".to_string(), 122000);  
        db.insert("ks_mazda_mx5_cup|magione|".to_string(), 84000); 


        db.insert("ks_porsche_911_gt2_rs|ks_nordschleife|tourist".to_string(), 405000); 
        db.insert("ks_nissan_gtr|ks_nordschleife|tourist".to_string(), 440000); 
        db.insert("bmw_m3_e30|ks_nordschleife|tourist".to_string(), 510000); 
        
      
        db.insert("bmw_z4_gt3|ks_nordschleife|endurance".to_string(), 495000); 
        db.insert("mclaren_mp412c_gt3|ks_nordschleife|endurance".to_string(), 492000); 

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
      
        let key_exact = format!("{}|{}|{}", car_id, track_name, track_config);
        let key_base = format!("{}|{}|", car_id, track_name);
        
        
        if let Some(user_rec) = self.records.get(&key_exact).or_else(|| self.records.get(&key_base)) {
           
            if let Some(static_time) = self.static_db.get(&key_exact).or_else(|| self.static_db.get(&key_base)) {
                if user_rec.time_ms < *static_time {
                    return user_rec.clone();
                }
            } else {
                return user_rec.clone();
            }
        }

      
        if let Some(static_time) = self.static_db.get(&key_exact).or_else(|| self.static_db.get(&key_base)) {
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


        let calculated_time = if let Some(s) = specs {
            self.calculate_theoretical_time_specs(s, track_len_m, car_id)
        } else {
            self.calculate_theoretical_time_guess(car_id, track_len_m)
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

  
    fn calculate_theoretical_time_specs(&self, specs: &CarSpecs, track_len: f32, car_id: &str) -> i32 {
        if specs.power_hp == 0.0 || specs.weight_kg == 0.0 || track_len == 0.0 {
            return 120000;
        }

        let pwr = specs.weight_kg / specs.power_hp; 
        

        let mut base_speed_ms = 58.0 - (pwr * 3.0); 
        
        let id_lower = car_id.to_lowercase();
        
       
        if id_lower.contains("f1") || id_lower.contains("formula") || id_lower.contains("exos") || id_lower.contains("919") {
            base_speed_ms = base_speed_ms.max(65.0); 
        } else if id_lower.contains("gt3") {
           
            base_speed_ms = base_speed_ms.clamp(48.0, 52.0); 
        } else if id_lower.contains("gt4") {
            base_speed_ms = base_speed_ms.clamp(44.0, 47.0);
        }

        
        if track_len > 5500.0 { base_speed_ms *= 1.06; }
        
        if track_len < 3000.0 { base_speed_ms *= 0.85; }

        let speed_ms = base_speed_ms.clamp(20.0, 110.0);
        let time_sec = track_len / speed_ms;
        (time_sec * 1000.0) as i32
    }


    fn calculate_theoretical_time_guess(&self, car_id: &str, track_len: f32) -> i32 {
        if track_len == 0.0 { return 120000; }

        let id = car_id.to_lowercase();
        
        
        let mut avg_speed_kph = 120.0; 

        if id.contains("f1") || id.contains("formula") || id.contains("exos") || id.contains("919") {
            avg_speed_kph = 225.0; 
        } else if id.contains("gt3") || id.contains("gte") {
            avg_speed_kph = 178.0; 
        } else if id.contains("gt2") {
            avg_speed_kph = 175.0; 
        } else if id.contains("gt4") || id.contains("cup") {
            avg_speed_kph = 158.0; 
        } else if id.contains("ferrari") || id.contains("mclaren") || id.contains("lamborghini") || id.contains("porsche") {
            avg_speed_kph = 155.0; 
        } else if id.contains("drift") || id.contains("e30") || id.contains("jdm") {
            avg_speed_kph = 105.0; 
        } else if id.contains("abarth") || id.contains("mx5") || id.contains("mito") {
            avg_speed_kph = 95.0; 
        }

        
        if track_len > 6000.0 { avg_speed_kph *= 1.08; }
        
        if track_len < 2500.0 { avg_speed_kph *= 0.82; }

        let speed_ms = avg_speed_kph / 3.6;
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