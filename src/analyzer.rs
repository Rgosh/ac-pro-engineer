use crate::ac_structs::{AcPhysics, AcGraphics};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LapData {
    pub lap_number: i32,
    pub lap_time_ms: i32,
    pub sectors: [i32; 3], 
    pub valid: bool,
    
    // --- Базовые Метрики ---
    pub max_speed: f32,
    pub avg_speed: f32,
    pub min_corner_speed_avg: f32, // Средняя скорость в апексах
    pub fuel_used: f32,
    pub gear_shifts: i32,          // Количество переключений
    pub peak_lat_g: f32,           // Макс G
    pub peak_brake_g: f32,
    
    // --- Глубокий анализ (0-100%) ---
    pub throttle_smoothness: f32,
    pub steering_smoothness: f32,
    pub trail_braking_score: f32,
    pub coasting_percent: f32,
    pub pedal_overlap_percent: f32,
    pub full_throttle_percent: f32,
    
    // --- Стабильность ---
    pub oversteer_count: i32,
    pub understeer_count: i32,
    pub lockup_count: i32,

    // График (дистанция -> данные)
    pub telemetry_trace: Vec<TelemetryPoint>,
    
    // Границы трека для масштабирования карты
    pub bounds_min_x: f32,
    pub bounds_max_x: f32,
    pub bounds_min_y: f32,
    pub bounds_max_y: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryPoint {
    pub distance: f32, 
    pub time_ms: i32, // Важно для расчета дельты по времени
    pub speed: f32,
    pub gas: f32,
    pub brake: f32,
    pub gear: i32,
    pub steer: f32,
    pub slip_avg: f32,
    pub x: f32, // Координата для карты
    pub y: f32, // Координата для карты
}

pub struct LapComparison {
    pub time_diff: f32,
    pub speed_diff_avg: f32,
    pub lost_on_straights: f32,    
    pub lost_in_corners: f32,      
    pub lost_on_braking: f32,      
    pub braking_aggression_diff: f32,
}

// Структура для анализа самого себя (без сравнения)
pub struct StandaloneAnalysis {
    pub is_perfect: bool,
    pub advices: Vec<String>,
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
}

impl TelemetryAnalyzer {
    pub fn new() -> Self {
        Self {
            laps: Vec::new(),
            best_lap_index: None,
        }
    }
    
    pub fn process_lap(
        &mut self,
        lap_number: i32,
        lap_time_ms: i32,
        physics_log: &[AcPhysics],
        graphics_log: &[AcGraphics],
    ) {
        if physics_log.is_empty() { return; }

        let max_speed = physics_log.iter().map(|p| p.speed_kmh).fold(0.0, f32::max);
        let avg_speed = physics_log.iter().map(|p| p.speed_kmh).sum::<f32>() / physics_log.len() as f32;
        
        let corner_points: Vec<&AcPhysics> = physics_log.iter()
            .filter(|p| p.speed_kmh > 30.0 && p.acc_g[0].abs() > 0.5)
            .collect();
        let min_corner_speed_avg = if !corner_points.is_empty() {
             corner_points.iter().map(|p| p.speed_kmh).sum::<f32>() / corner_points.len() as f32
        } else { 0.0 };

        let start_fuel = physics_log.first().map(|p| p.fuel).unwrap_or(0.0);
        let end_fuel = physics_log.last().map(|p| p.fuel).unwrap_or(0.0);
        let fuel_used = (start_fuel - end_fuel).max(0.0);

        let mut coasting_frames = 0;
        let mut overlap_frames = 0;
        let mut full_throttle_frames = 0;
        let mut gear_shifts = 0;
        let mut prev_gear = physics_log.first().map(|p| p.gear).unwrap_or(0);
        
        let mut trail_braking_score_acc = 0.0;
        let mut trail_braking_samples = 0.0;
        
        let mut oversteer_c = 0;
        let mut understeer_c = 0;
        let mut lockup_c = 0;

        let mut total_jerk = 0.0;
        let mut prev_acc = 0.0;
        let mut steer_jerk = 0.0;
        let mut prev_steer = 0.0;
        
        let mut peak_lat_g = 0.0;
        let mut peak_brake_g = 0.0;

        for p in physics_log {
            let acc = p.acc_g[2];
            total_jerk += (acc - prev_acc).abs();
            prev_acc = acc;
            
            steer_jerk += (p.steer_angle - prev_steer).abs();
            prev_steer = p.steer_angle;
            
            if p.acc_g[0].abs() > peak_lat_g { peak_lat_g = p.acc_g[0].abs(); }
            if p.acc_g[2] < peak_brake_g { peak_brake_g = p.acc_g[2]; }

            if p.gear != prev_gear {
                gear_shifts += 1;
                prev_gear = p.gear;
            }

            if p.gas > 0.95 { full_throttle_frames += 1; }
            if p.speed_kmh > 30.0 && p.gas < 0.05 && p.brake < 0.05 { coasting_frames += 1; }
            if p.gas > 0.1 && p.brake > 0.1 { overlap_frames += 1; }

            if p.brake > 0.1 && p.steer_angle.abs() > 0.05 {
                let steer_factor = p.steer_angle.abs().min(1.0);
                let brake_ideal = 1.0 - steer_factor * 0.8; 
                let diff = (brake_ideal - p.brake).abs();
                trail_braking_score_acc += (1.0 - diff).max(0.0);
                trail_braking_samples += 1.0;
            }

            if p.speed_kmh > 20.0 {
                 if p.slip_ratio.iter().any(|&s| s.abs() > 0.2) && p.brake > 0.5 {
                    lockup_c += 1;
                }
                let front_slip = (p.slip_angle[0].abs() + p.slip_angle[1].abs()) / 2.0;
                let rear_slip = (p.slip_angle[2].abs() + p.slip_angle[3].abs()) / 2.0;
                if rear_slip > front_slip + 5.0 { oversteer_c += 1; } 
                else if front_slip > rear_slip + 5.0 { understeer_c += 1; }
            }
        }
        
        let throttle_smoothness = (100.0 - (total_jerk / physics_log.len() as f32) * 50.0).clamp(0.0, 100.0);
        let steering_smoothness = (100.0 - (steer_jerk / physics_log.len() as f32) * 200.0).clamp(0.0, 100.0);
        
        let trail_score = if trail_braking_samples > 0.0 {
            (trail_braking_score_acc / trail_braking_samples * 100.0).clamp(0.0, 100.0)
        } else { 50.0 };

        let coasting_pct = (coasting_frames as f32 / physics_log.len() as f32) * 100.0;
        let overlap_pct = (overlap_frames as f32 / physics_log.len() as f32) * 100.0;
        let full_throttle_pct = (full_throttle_frames as f32 / physics_log.len() as f32) * 100.0;

        // 3. Формирование трека с координатами для карты
        let mut trace = Vec::new();
        let step = 5; 
        
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        
        for (i, p) in physics_log.iter().enumerate() {
            if i % step == 0 {
                let g = if i < graphics_log.len() { &graphics_log[i] } else { graphics_log.last().unwrap() };
                
                // Координаты для карты (X, Z из игры -> X, Y для 2D)
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
                    time_ms: g.i_current_time, // Важно!
                    speed: p.speed_kmh,
                    gas: p.gas,
                    brake: p.brake,
                    gear: p.gear - 1,
                    steer: p.steer_angle,
                    slip_avg: p.wheel_slip.iter().sum::<f32>() / 4.0,
                    x,
                    y: z,
                });
            }
        }
        trace.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        let lap_data = LapData {
            lap_number,
            lap_time_ms,
            sectors: [0, 0, 0], 
            valid: true, 
            max_speed,
            avg_speed,
            min_corner_speed_avg,
            fuel_used,
            gear_shifts,
            peak_lat_g,
            peak_brake_g: peak_brake_g.abs(),
            throttle_smoothness,
            steering_smoothness,
            trail_braking_score: trail_score,
            coasting_percent: coasting_pct,
            pedal_overlap_percent: overlap_pct,
            full_throttle_percent: full_throttle_pct,
            oversteer_count: oversteer_c / 5,
            understeer_count: understeer_c / 5,
            lockup_count: lockup_c / 5,
            telemetry_trace: trace,
            bounds_min_x: min_x,
            bounds_max_x: max_x,
            bounds_min_y: min_y,
            bounds_max_y: max_y,
        };
        
        self.laps.push(lap_data);
        
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
            speed_diff_avg: speed_diff,
            lost_on_straights: lost_straight * norm,
            lost_in_corners: lost_corner * norm,
            lost_on_braking: lost_brake * norm,
            braking_aggression_diff: current.peak_brake_g - reference.peak_brake_g,
        }
    }
    
    // АНАЛИЗ ОДИНОЧНОГО (ЛУЧШЕГО) КРУГА
    pub fn analyze_standalone(&self, lap: &LapData) -> StandaloneAnalysis {
        let mut advices = Vec::new();
        
        if lap.coasting_percent > 4.0 { advices.push("advice_coast".to_string()); }
        if lap.pedal_overlap_percent > 4.0 { advices.push("advice_overlap".to_string()); }
        if lap.lockup_count > 2 { advices.push("advice_lock".to_string()); }
        if lap.throttle_smoothness < 75.0 { advices.push("advice_smooth".to_string()); }
        
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