use crate::ac_structs::{AcPhysics, AcGraphics};
use serde::Serialize;
// Предполагаем, что struct TrackRecord определен в records.rs, 
// если нет - можно заменить на использование локальной структуры или placeholder.
// Для компиляции добавим упрощенное определение, если модуль records не подключен,
// но в вашем проекте лучше использовать use crate::records::TrackRecord;
use crate::records::TrackRecord; 

#[derive(Debug, Clone, Serialize)]
pub struct LapData {
    pub lap_number: i32,
    pub lap_time_ms: i32,
    pub sectors: [i32; 3], 
    pub valid: bool,
    
    // --- Базовые Метрики ---
    pub max_speed: f32,
    pub avg_speed: f32,
    pub min_corner_speed_avg: f32, 
    pub fuel_used: f32,
    pub gear_shifts: i32,          
    pub peak_lat_g: f32,           
    pub peak_brake_g: f32,
    
    // --- Расширенные Метрики (Новые) ---
    pub avg_tyre_temp: [f32; 4],
    pub max_brake_temp: [f32; 4],
    pub pressure_deviation: f32, // Среднее отклонение от идеала (27.5 psi)
    pub suspension_travel_hist: [f32; 4], // % хода подвески (средний)
    
    // --- Глубокий анализ ---
    pub throttle_smoothness: f32,
    pub steering_smoothness: f32,
    pub trail_braking_score: f32,
    pub coasting_percent: f32,
    pub pedal_overlap_percent: f32,
    pub full_throttle_percent: f32,
    pub grip_usage_percent: f32, // Насколько полно используется круг сцепления

    // Стабильность
    pub oversteer_count: i32,
    pub understeer_count: i32,
    pub lockup_count: i32,

    // Данные для графиков
    pub telemetry_trace: Vec<TelemetryPoint>,
    pub bounds_min_x: f32,
    pub bounds_max_x: f32,
    pub bounds_min_y: f32,
    pub bounds_max_y: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryPoint {
    pub distance: f32, 
    pub time_ms: i32, 
    pub speed: f32,
    pub gas: f32,
    pub brake: f32,
    pub gear: i32,
    pub steer: f32,
    pub lat_g: f32,
    pub lon_g: f32,
    pub slip_avg: f32,
    pub x: f32, 
    pub y: f32, 
}

pub struct LapComparison {
    pub time_diff: f32,
    pub sector_diffs: [f32; 3], // Дельта по секторам
    pub speed_diff_avg: f32,
    pub lost_on_straights: f32,    
    pub lost_in_corners: f32,      
    pub lost_on_braking: f32,      
    pub braking_aggression_diff: f32,
}

pub struct StandaloneAnalysis {
    pub is_perfect: bool,
    pub advices: Vec<Advice>,
}

#[derive(Debug, Clone)]
pub struct Advice {
    pub zone: String,      // Например: "Turn 1", "General", "Sector 2"
    pub problem: String,   // Например: "Early Braking", "Low Corner Speed"
    pub solution: String,  // Например: "Brake 10m later", "Trust aero more"
    pub severity: u8,      // 1 (Info) to 3 (Critical)
}

#[derive(Debug, Serialize, Clone)]
pub struct AnalysisResult {
    pub category: String,
    pub metric: String,
    pub value: f32,
    pub unit: String,
    pub recommendation: String,
}

pub struct TelemetryAnalyzer {
    pub laps: Vec<LapData>,
    pub best_lap_index: Option<usize>,
    pub best_sectors: [i32; 3], // Лучшие сектора сессии (Теоретический Best)
    pub world_record: Option<TrackRecord>, // Подгруженный мировой/расчетный рекорд
}

impl TelemetryAnalyzer {
    pub fn new() -> Self {
        Self {
            laps: Vec::new(),
            best_lap_index: None,
            best_sectors: [i32::MAX, i32::MAX, i32::MAX], // Инициализируем MAX значением
            world_record: None,
        }
    }
    
    pub fn set_world_record(&mut self, record: TrackRecord) {
        self.world_record = Some(record);
    }
    
    pub fn process_lap(
        &mut self,
        lap_number: i32,
        lap_time_ms: i32,
        physics_log: &[AcPhysics],
        graphics_log: &[AcGraphics],
    ) {
        if physics_log.is_empty() { return; }
        
        // --- 1. Обработка Секторов ---
        let last_gfx = graphics_log.last().unwrap();
        // В AC split[] хранит время сплитов (момент времени). 
        // S1 = split[0], S2 = split[1] - split[0], S3 = Total - split[1]
        // Но иногда AC отдает 0 в сплитах, нужна проверка.
        let raw_s1 = last_gfx.split[0] as i32;
        let raw_s2 = last_gfx.split[1] as i32;
        
        let s1 = if raw_s1 > 0 { raw_s1 } else { 0 };
        let s2 = if raw_s2 > raw_s1 { raw_s2 - raw_s1 } else { 0 };
        let s3 = if lap_time_ms > raw_s2 { lap_time_ms - raw_s2 } else { 0 };
        
        let sectors = [s1, s2, s3];

        // Обновляем теоретический лучший круг (Session Theoretical Best)
        for i in 0..3 {
            if sectors[i] > 1000 && sectors[i] < self.best_sectors[i] {
                self.best_sectors[i] = sectors[i];
            }
        }

        // --- 2. Базовые вычисления ---
        let max_speed = physics_log.iter().map(|p| p.speed_kmh).fold(0.0, f32::max);
        let avg_speed = physics_log.iter().map(|p| p.speed_kmh).sum::<f32>() / physics_log.len() as f32;
        
        let start_fuel = physics_log.first().map(|p| p.fuel).unwrap_or(0.0);
        let end_fuel = physics_log.last().map(|p| p.fuel).unwrap_or(0.0);
        let fuel_used = (start_fuel - end_fuel).max(0.0);

        // --- 3. Счетчики и Накопители ---
        let mut coasting_frames = 0;
        let mut overlap_frames = 0;
        let mut full_throttle_frames = 0;
        let mut gear_shifts = 0;
        let mut prev_gear = physics_log.first().map(|p| p.gear).unwrap_or(0);
        
        let mut trail_braking_score_acc = 0.0;
        let mut trail_braking_samples = 0.0;
        let mut grip_usage_acc = 0.0;
        let mut grip_samples = 0.0;
        
        let mut oversteer_c = 0;
        let mut understeer_c = 0;
        let mut lockup_c = 0;

        let mut total_jerk = 0.0;
        let mut prev_acc = 0.0;
        let mut steer_jerk = 0.0;
        let mut prev_steer = 0.0;
        
        let mut peak_lat_g = 0.0;
        let mut peak_brake_g = 0.0;
        
        // Новые накопители
        let mut max_brake_temp = [0.0; 4];
        let mut sum_tyre_temp = [0.0; 4];
        let mut sum_susp_travel = [0.0; 4];
        let mut press_dev_acc = 0.0;

        let log_len = physics_log.len() as f32;

        for p in physics_log {
            // -- Физика движения --
            let acc = p.acc_g[2];
            total_jerk += (acc - prev_acc).abs();
            prev_acc = acc;
            
            steer_jerk += (p.steer_angle - prev_steer).abs();
            prev_steer = p.steer_angle;
            
            let lat_g = p.acc_g[0];
            let lon_g = p.acc_g[2];
            
            // G-Force пики
            if lat_g.abs() > peak_lat_g { peak_lat_g = lat_g.abs(); }
            if lon_g < peak_brake_g { peak_brake_g = lon_g; } // lon_g при торможении отрицательный

            // Grip Usage (Векторная сумма G / 2.5G эталон)
            let combined_g = (lat_g.powi(2) + lon_g.powi(2)).sqrt();
            if combined_g > 0.5 { // Считаем только активные фазы
                grip_usage_acc += combined_g;
                grip_samples += 1.0;
            }

            // Переключения передач
            if p.gear != prev_gear {
                gear_shifts += 1;
                prev_gear = p.gear;
            }

            // Педали
            if p.gas > 0.95 { full_throttle_frames += 1; }
            if p.speed_kmh > 30.0 && p.gas < 0.05 && p.brake < 0.05 { coasting_frames += 1; }
            if p.gas > 0.1 && p.brake > 0.1 { overlap_frames += 1; }

            // Trail Braking: Идеально когда тормоз падает по мере роста угла руля
            if p.brake > 0.1 && p.steer_angle.abs() > 0.05 {
                let steer_factor = p.steer_angle.abs().min(1.0);
                // Идеальный тормоз = 1.0 - steer (линейно), чем ближе к этому, тем лучше
                let brake_ideal = (1.0 - steer_factor).max(0.0); 
                let diff = (brake_ideal - p.brake).abs();
                trail_braking_score_acc += (1.0 - diff).max(0.0);
                trail_braking_samples += 1.0;
            }

            // Стабильность и Ошибки
            if p.speed_kmh > 20.0 {
                 if p.slip_ratio.iter().any(|&s| s.abs() > 0.2) && p.brake > 0.5 {
                    lockup_c += 1;
                }
                let front_slip = (p.slip_angle[0].abs() + p.slip_angle[1].abs()) / 2.0;
                let rear_slip = (p.slip_angle[2].abs() + p.slip_angle[3].abs()) / 2.0;
                if rear_slip > front_slip + 5.0 { oversteer_c += 1; } 
                else if front_slip > rear_slip + 5.0 { understeer_c += 1; }
            }
            
            // Температуры и давление
            for i in 0..4 {
                if p.brake_temp[i] > max_brake_temp[i] { max_brake_temp[i] = p.brake_temp[i]; }
                
                // Средняя темп шин (I+M+O)/3
                let t_avg = (p.tyre_temp_i[i] + p.tyre_temp_m[i] + p.tyre_temp_o[i]) / 3.0;
                sum_tyre_temp[i] += t_avg;
                
                sum_susp_travel[i] += p.suspension_travel[i];
                
                // Отклонение давления от эталона GT3 (27.5 psi). 
                // В идеале эталон брать из Setup, но тут берем хардкод для анализа
                if p.speed_kmh > 50.0 {
                    press_dev_acc += (p.wheels_pressure[i] - 27.5).abs();
                }
            }
        }
        
        // --- 4. Финализация метрик ---
        let throttle_smoothness = (100.0 - (total_jerk / log_len) * 50.0).clamp(0.0, 100.0);
        let steering_smoothness = (100.0 - (steer_jerk / log_len) * 200.0).clamp(0.0, 100.0);
        
        let trail_score = if trail_braking_samples > 0.0 {
            (trail_braking_score_acc / trail_braking_samples * 100.0).clamp(0.0, 100.0)
        } else { 50.0 };
        
        let grip_usage_percent = if grip_samples > 0.0 {
            // Средний G / 2.5G (пик для GT3 на сликах). Для дорожных авто будет меньше.
            ((grip_usage_acc / grip_samples) / 2.0 * 100.0).clamp(0.0, 100.0)
        } else { 0.0 };

        let coasting_pct = (coasting_frames as f32 / log_len) * 100.0;
        let overlap_pct = (overlap_frames as f32 / log_len) * 100.0;
        let full_throttle_pct = (full_throttle_frames as f32 / log_len) * 100.0;
        
        let avg_tyre_temp = [
            sum_tyre_temp[0] / log_len, sum_tyre_temp[1] / log_len, 
            sum_tyre_temp[2] / log_len, sum_tyre_temp[3] / log_len
        ];
        
        let suspension_travel_hist = [
            sum_susp_travel[0] / log_len, sum_susp_travel[1] / log_len,
            sum_susp_travel[2] / log_len, sum_susp_travel[3] / log_len
        ];
        
        let pressure_deviation = press_dev_acc / (log_len * 4.0); // Среднее по 4 колесам и времени

        // --- 5. Генерация трейса (прореживание для UI) ---
        let mut trace = Vec::new();
        let step = 5; 
        
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        
        for (i, p) in physics_log.iter().enumerate() {
            if i % step == 0 {
                let g = if i < graphics_log.len() { &graphics_log[i] } else { graphics_log.last().unwrap() };
                
                let x = g.car_coordinates[0][0];
                let z = g.car_coordinates[0][2];
                
                if x != 0.0 || z != 0.0 {
                    if x < min_x { min_x = x; }
                    if x > max_x { max_x = x; }
                    if z < min_y { min_y = z; }
                    if z > max_y { max_y = z; }
                }

                trace.push(TelemetryPoint {
                    distance: g.normalized_car_position,
                    time_ms: g.i_current_time, 
                    speed: p.speed_kmh,
                    gas: p.gas,
                    brake: p.brake,
                    gear: p.gear - 1,
                    steer: p.steer_angle,
                    lat_g: p.acc_g[0],
                    lon_g: p.acc_g[2],
                    slip_avg: p.wheel_slip.iter().sum::<f32>() / 4.0,
                    x,
                    y: z,
                });
            }
        }
        // Сортировка по дистанции важна для графиков
        trace.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        let corner_points: Vec<&TelemetryPoint> = trace.iter()
            .filter(|p| p.speed > 30.0 && p.lat_g.abs() > 0.5)
            .collect();
        let min_corner_speed_avg = if !corner_points.is_empty() {
             corner_points.iter().map(|p| p.speed).sum::<f32>() / corner_points.len() as f32
        } else { 0.0 };

        let lap_data = LapData {
            lap_number,
            lap_time_ms,
            sectors, 
            valid: true, 
            max_speed,
            avg_speed,
            min_corner_speed_avg,
            fuel_used,
            gear_shifts,
            peak_lat_g,
            peak_brake_g: peak_brake_g.abs(),
            avg_tyre_temp,
            max_brake_temp,
            pressure_deviation,
            suspension_travel_hist,
            throttle_smoothness,
            steering_smoothness,
            trail_braking_score: trail_score,
            coasting_percent: coasting_pct,
            pedal_overlap_percent: overlap_pct,
            full_throttle_percent: full_throttle_pct,
            grip_usage_percent,
            oversteer_count: oversteer_c / 5, // Делим на 5, чтобы убрать шум (фреймы в события)
            understeer_count: understeer_c / 5,
            lockup_count: lockup_c / 5,
            telemetry_trace: trace,
            bounds_min_x: min_x,
            bounds_max_x: max_x,
            bounds_min_y: min_y,
            bounds_max_y: max_y,
        };
        
        self.laps.push(lap_data);
        
        // Обновляем индекс лучшего круга
        if let Some(best_idx) = self.best_lap_index {
            if lap_time_ms < self.laps[best_idx].lap_time_ms && lap_time_ms > 10000 {
                self.best_lap_index = Some(self.laps.len() - 1);
            }
        } else {
            if lap_time_ms > 10000 {
                self.best_lap_index = Some(self.laps.len() - 1);
            }
        }
    }

    pub fn compare_laps(&self, current: &LapData, reference: &LapData) -> LapComparison {
        let time_diff = (current.lap_time_ms - reference.lap_time_ms) as f32 / 1000.0;
        let speed_diff = current.avg_speed - reference.avg_speed;
        
        // Сравнение по секторам
        let mut sector_diffs = [0.0; 3];
        for i in 0..3 {
            if current.sectors[i] > 0 && reference.sectors[i] > 0 {
                sector_diffs[i] = (current.sectors[i] - reference.sectors[i]) as f32 / 1000.0;
            }
        }

        let mut lost_brake = 0.0;
        let mut lost_corner = 0.0;
        let mut lost_straight = 0.0;
        
        let sample_size = 50; 
        for i in 0..sample_size {
            let dist_target = i as f32 / sample_size as f32;
            if let (Some(p1), Some(p2)) = (
                self.find_point_at(&current.telemetry_trace, dist_target),
                self.find_point_at(&reference.telemetry_trace, dist_target)
            ) {
                let v_diff = p2.speed - p1.speed; 
                // Если референс быстрее на 2+ км/ч, анализируем причину
                if v_diff > 2.0 { 
                    if p1.brake > 0.1 || p2.brake > 0.1 { lost_brake += 1.0; } 
                    else if p1.steer.abs() > 0.05 { lost_corner += 1.0; } 
                    else { lost_straight += 1.0; }
                }
            }
        }
        
        let total_lost_samples = lost_brake + lost_corner + lost_straight;
        let norm = if total_lost_samples > 0.0 { time_diff.max(0.0) / total_lost_samples } else { 0.0 };

        LapComparison {
            time_diff,
            sector_diffs,
            speed_diff_avg: speed_diff,
            lost_on_straights: lost_straight * norm,
            lost_in_corners: lost_corner * norm,
            lost_on_braking: lost_brake * norm,
            braking_aggression_diff: current.peak_brake_g - reference.peak_brake_g,
        }
    }
    
    // Глубокий анализ с ТОЧНЫМИ данными
    pub fn analyze_standalone(&self, lap: &LapData) -> StandaloneAnalysis {
        let mut advices = Vec::new();
        
        // 1. Накат (Coasting)
        if lap.coasting_percent > 3.0 { 
            advices.push(Advice {
                zone: "General".into(),
                problem: format!("Coasting detected: {:.1}% of lap", lap.coasting_percent),
                solution: "Transition Gas->Brake faster. Don't roll.".into(),
                severity: 2,
            });
        }
        
        // 2. Использование потенциала (Grip)
        // Для GT3 на сликах норма ~1.8-2.2G. Если меньше 1.4G, значит недожимаешь.
        if lap.peak_lat_g < 1.4 {
             advices.push(Advice {
                zone: "Cornering".into(),
                problem: format!("Low Peak G: {:.2}G (Target >1.8G)", lap.peak_lat_g),
                solution: "Tyres have more grip. Carry higher entry speed.".into(),
                severity: 2,
            });
        }

        // 3. Trail Braking
        if lap.trail_braking_score < 40.0 {
            advices.push(Advice {
                zone: "Entry Phase".into(),
                problem: format!("Poor Trail Braking: {:.0}%", lap.trail_braking_score),
                solution: "Keep brake pressure while turning in, release slowly.".into(),
                severity: 2,
            });
        }
        
        // 4. Блокировки (Lockups)
        if lap.lockup_count > 0 {
             advices.push(Advice {
                zone: "Braking".into(),
                problem: format!("{} Lockups detected", lap.lockup_count),
                solution: "Reduce peak brake pressure or move Bias Rearwards.".into(),
                severity: 3,
            });
        }
        
        // 5. Недостаточная поворачиваемость (Understeer)
        if lap.understeer_count > 5 {
             advices.push(Advice {
                zone: "Mid-Corner".into(),
                problem: "Excessive Understeer".into(),
                solution: "Wait for front grip before throttle. Soften Front ARB.".into(),
                severity: 2,
            });
        }

        // 6. Давление шин (Точность)
        if lap.pressure_deviation > 1.0 {
            advices.push(Advice {
                zone: "Tyres".into(),
                problem: format!("Pressure deviation: +/- {:.1} psi", lap.pressure_deviation),
                solution: "Adjust pressures to aim for 27.5 psi (GT3) hot.".into(),
                severity: 3,
            });
        }
        
        // 7. Температура тормозов
        let max_b = lap.max_brake_temp.iter().cloned().fold(0./0., f32::max);
        if max_b > 750.0 {
            advices.push(Advice {
                zone: "Brakes".into(),
                problem: format!("Overheating: {:.0}°C", max_b),
                solution: "Open brake ducts or increase bias to cooler axle.".into(),
                severity: 3,
            });
        }
        
        // 8. Сравнение с теоретическим сектором (если есть данные)
        for i in 0..3 {
            if self.best_sectors[i] < i32::MAX && lap.sectors[i] > 0 {
                let diff = (lap.sectors[i] - self.best_sectors[i]) as f32 / 1000.0;
                if diff > 0.5 {
                     advices.push(Advice {
                        zone: format!("Sector {}", i+1),
                        problem: format!("Pace Loss: +{:.2}s vs Ideal", diff),
                        solution: "Check racing line comparison.".into(),
                        severity: 1,
                    });
                }
            }
        }
        
        StandaloneAnalysis {
            is_perfect: advices.is_empty(),
            advices,
        }
    }
    
    fn find_point_at<'a>(&self, trace: &'a [TelemetryPoint], dist: f32) -> Option<&'a TelemetryPoint> {
        trace.iter().min_by(|a, b| 
            (a.distance - dist).abs().partial_cmp(&(b.distance - dist).abs()).unwrap()
        )
    }
}