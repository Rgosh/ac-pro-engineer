use crate::ac_structs::{AcGraphics, AcPhysics};
use crate::config::Language;
use crate::records::TrackRecord;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapData {
    pub lap_number: i32,
    pub lap_time_ms: i32,
    pub sectors: [i32; 3],
    pub valid: bool,

    pub car_model: String,
    pub track_name: String,
    pub save_date: String,
    #[serde(default)]
    pub from_file: bool,

    pub air_temp: f32,
    pub road_temp: f32,
    pub track_grip: f32,
    pub timestamp: String,

    pub max_speed: f32,
    pub avg_speed: f32,
    pub avg_pressure: f32,
    pub min_corner_speed_avg: f32,
    pub fuel_used: f32,
    pub gear_shifts: i32,
    pub peak_lat_g: f32,
    pub peak_brake_g: f32,

    pub avg_tyre_temp: [f32; 4],
    pub max_brake_temp: [f32; 4],
    pub pressure_deviation: f32,
    pub suspension_travel_hist: [f32; 4],

    #[serde(default)]
    pub avg_wheels_pressure: [f32; 4],
    #[serde(default)]
    pub avg_tyre_temp_i: [f32; 4],
    #[serde(default)]
    pub avg_tyre_temp_m: [f32; 4],
    #[serde(default)]
    pub avg_tyre_temp_o: [f32; 4],
    #[serde(default)]
    pub avg_brake_temp: [f32; 4],
    #[serde(default)]
    pub avg_ride_height: [f32; 2],

    #[serde(default)]
    pub damper_histograms: [[f32; 4]; 4],

    pub throttle_smoothness: f32,
    pub steering_smoothness: f32,
    pub trail_braking_score: f32,
    pub coasting_percent: f32,
    pub pedal_overlap_percent: f32,
    pub full_throttle_percent: f32,
    pub grip_usage_percent: f32,

    pub oversteer_count: i32,
    pub understeer_count: i32,
    pub lockup_count: i32,
    pub car_control_score: f32,

    pub radar_stats: RadarStats,

    pub telemetry_trace: Vec<TelemetryPoint>,

    pub bounds_min_x: f32,
    pub bounds_max_x: f32,
    pub bounds_min_y: f32,
    pub bounds_max_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RadarStats {
    pub smoothness: f32,
    pub aggression: f32,
    pub consistency: f32,
    pub car_control: f32,
    pub tyre_mgmt: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub struct StandaloneAnalysis {
    pub is_perfect: bool,
    pub advices: Vec<Advice>,
}

#[derive(Debug, Clone)]
pub struct Advice {
    pub zone: String,
    pub problem: String,
    pub solution: String,
    pub severity: u8,
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
    pub best_sectors: [i32; 3],
    pub world_record: Option<TrackRecord>,
    pub reference_lap: Option<LapData>,
}

pub type Analyzer = TelemetryAnalyzer;

impl TelemetryAnalyzer {
    pub fn new() -> Self {
        Self {
            laps: Vec::new(),
            best_lap_index: None,
            best_sectors: [i32::MAX, i32::MAX, i32::MAX],
            world_record: None,
            reference_lap: None,
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
        car_name: String,
        track_name: String,
    ) {
        if physics_log.is_empty() {
            return;
        }

        info!(
            "Processing Lap {} | Time: {}ms | Car: {}",
            lap_number, lap_time_ms, car_name
        );

        let last_gfx = match graphics_log.last() {
            Some(gfx) => gfx,
            None => return,
        };

        let raw_s1 = last_gfx.split[0] as i32;
        let raw_s2 = last_gfx.split[1] as i32;

        let s1 = if raw_s1 > 0 { raw_s1 } else { 0 };
        let s2 = if raw_s2 > raw_s1 { raw_s2 - raw_s1 } else { 0 };
        let s3 = if lap_time_ms > raw_s2 {
            lap_time_ms - raw_s2
        } else {
            0
        };

        let sectors = [s1, s2, s3];

        for (i, sector) in sectors.iter().enumerate() {
            if *sector > 1000 && *sector < self.best_sectors[i] {
                self.best_sectors[i] = *sector;
            }
        }

        let air_temp = physics_log.first().map(|p| p.air_temp).unwrap_or(20.0);
        let road_temp = physics_log.first().map(|p| p.road_temp).unwrap_or(20.0);
        let track_grip = graphics_log.first().map(|g| g.surface_grip).unwrap_or(1.0) * 100.0;
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        let save_date = chrono::Local::now().format("%Y-%m-%d").to_string();

        let max_speed = physics_log.iter().map(|p| p.speed_kmh).fold(0.0, f32::max);
        let avg_speed = if !physics_log.is_empty() {
            physics_log.iter().map(|p| p.speed_kmh).sum::<f32>() / physics_log.len() as f32
        } else {
            0.0
        };

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
        let mut grip_usage_acc = 0.0;
        let mut grip_samples = 0.0;

        let mut oversteer_c = 0;
        let mut understeer_c = 0;
        let mut lockup_c = 0;

        let mut total_jerk = 0.0;
        let mut prev_acc = 0.0;
        let mut steer_jerk = 0.0;
        let mut prev_steer = 0.0;

        let mut peak_lat_g: f32 = 0.0;
        let mut peak_brake_g: f32 = 0.0;

        let mut max_brake_temp = [0.0; 4];
        let mut sum_tyre_temp = [0.0; 4];
        let mut sum_susp_travel = [0.0; 4];
        let mut press_sum = 0.0;
        let mut press_dev_acc = 0.0;

        let mut sum_wheels_pressure = [0.0; 4];
        let mut sum_tyre_temp_i = [0.0; 4];
        let mut sum_tyre_temp_m = [0.0; 4];
        let mut sum_tyre_temp_o = [0.0; 4];
        let mut sum_brake_temp_avg = [0.0; 4];
        let mut sum_ride_height = [0.0; 2];

        let mut prev_susp_travel = physics_log
            .first()
            .map(|p| p.suspension_travel)
            .unwrap_or([0.0; 4]);
        let mut damper_counts = [[0.0_f32; 4]; 4];
        let mut damper_total_moves = [0.0_f32; 4];

        let log_len = physics_log.len() as f32;

        for p in physics_log {
            let acc = p.acc_g[2];
            total_jerk += (acc - prev_acc).abs();
            prev_acc = acc;

            steer_jerk += (p.steer_angle - prev_steer).abs();
            prev_steer = p.steer_angle;

            let lat_g = p.acc_g[0];
            let lon_g = p.acc_g[2];

            if lat_g.abs() > peak_lat_g {
                peak_lat_g = lat_g.abs();
            }
            if lon_g < peak_brake_g {
                peak_brake_g = lon_g;
            }

            let combined_g = (lat_g.powi(2) + lon_g.powi(2)).sqrt();
            if combined_g > 0.5 {
                grip_usage_acc += combined_g;
                grip_samples += 1.0;
            }

            if p.gear != prev_gear {
                gear_shifts += 1;
                prev_gear = p.gear;
            }

            if p.gas > 0.95 {
                full_throttle_frames += 1;
            }
            if p.speed_kmh > 30.0 && p.gas < 0.05 && p.brake < 0.05 {
                coasting_frames += 1;
            }
            if p.gas > 0.1 && p.brake > 0.1 {
                overlap_frames += 1;
            }

            if p.brake > 0.1 && p.steer_angle.abs() > 0.05 {
                let steer_factor = p.steer_angle.abs().min(1.0);
                let brake_ideal = (1.0 - steer_factor).max(0.0);
                let diff = (brake_ideal - p.brake).abs();
                trail_braking_score_acc += (1.0 - diff).max(0.0);
                trail_braking_samples += 1.0;
            }

            if p.speed_kmh > 20.0 {
                let slip_vals = p.wheel_slip;

                if slip_vals.iter().any(|&s| s.abs() > 0.2) && p.brake > 0.5 {
                    lockup_c += 1;
                }

                if slip_vals[2].abs() > 0.3 || slip_vals[3].abs() > 0.3 {
                    oversteer_c += 1;
                }
                if slip_vals[0].abs() > 0.3 || slip_vals[1].abs() > 0.3 {
                    understeer_c += 1;
                }
            }

            for i in 0..4 {
                if p.brake_temp[i] > max_brake_temp[i] {
                    max_brake_temp[i] = p.brake_temp[i];
                }
                let t_avg = (p.tyre_temp_i[i] + p.tyre_temp_m[i] + p.tyre_temp_o[i]) / 3.0;
                sum_tyre_temp[i] += t_avg;
                sum_susp_travel[i] += p.suspension_travel[i];

                if p.speed_kmh > 50.0 {
                    press_sum += p.wheels_pressure[i];
                    press_dev_acc += (p.wheels_pressure[i] - 27.5).abs();
                }

                sum_wheels_pressure[i] += p.wheels_pressure[i];
                sum_tyre_temp_i[i] += p.tyre_temp_i[i];
                sum_tyre_temp_m[i] += p.tyre_temp_m[i];
                sum_tyre_temp_o[i] += p.tyre_temp_o[i];
                sum_brake_temp_avg[i] += p.brake_temp[i];

                let delta_travel = p.suspension_travel[i] - prev_susp_travel[i];

                let vel_mm_s = (delta_travel / 0.003) * 1000.0;

                if vel_mm_s.abs() > 2.0 {
                    damper_total_moves[i] += 1.0;
                    if vel_mm_s > 30.0 {
                        damper_counts[i][1] += 1.0;
                    } else if vel_mm_s > 2.0 {
                        damper_counts[i][0] += 1.0;
                    } else if vel_mm_s < -30.0 {
                        damper_counts[i][3] += 1.0;
                    } else if vel_mm_s < -2.0 {
                        damper_counts[i][2] += 1.0;
                    }
                }
                prev_susp_travel[i] = p.suspension_travel[i];
            }

            sum_ride_height[0] += p.ride_height[0];
            sum_ride_height[1] += p.ride_height[1];
        }

        let mut damper_histograms = [[0.0; 4]; 4];
        for i in 0..4 {
            let total = if damper_total_moves[i] > 0.0 {
                damper_total_moves[i]
            } else {
                1.0
            };
            damper_histograms[i][0] = (damper_counts[i][0] / total) * 100.0;
            damper_histograms[i][1] = (damper_counts[i][1] / total) * 100.0;
            damper_histograms[i][2] = (damper_counts[i][2] / total) * 100.0;
            damper_histograms[i][3] = (damper_counts[i][3] / total) * 100.0;
        }

        debug!("Damper Histograms calculated successfully");

        let throttle_smoothness = if log_len > 0.0 {
            (100.0 - (total_jerk / log_len) * 50.0).clamp(0.0, 100.0)
        } else {
            100.0
        };

        let steering_smoothness = if log_len > 0.0 {
            (100.0 - (steer_jerk / log_len) * 200.0).clamp(0.0, 100.0)
        } else {
            100.0
        };

        let trail_score = if trail_braking_samples > 0.0 {
            (trail_braking_score_acc / trail_braking_samples * 100.0).clamp(0.0, 100.0)
        } else {
            50.0
        };

        let grip_usage_percent = if grip_samples > 0.0 {
            ((grip_usage_acc / grip_samples) / 2.0 * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        let coasting_pct = if log_len > 0.0 {
            (coasting_frames as f32 / log_len) * 100.0
        } else {
            0.0
        };
        let overlap_pct = if log_len > 0.0 {
            (overlap_frames as f32 / log_len) * 100.0
        } else {
            0.0
        };
        let full_throttle_pct = if log_len > 0.0 {
            (full_throttle_frames as f32 / log_len) * 100.0
        } else {
            0.0
        };

        let safe_div_len = if log_len > 0.0 { log_len } else { 1.0 };

        let avg_tyre_temp = [
            sum_tyre_temp[0] / safe_div_len,
            sum_tyre_temp[1] / safe_div_len,
            sum_tyre_temp[2] / safe_div_len,
            sum_tyre_temp[3] / safe_div_len,
        ];

        let suspension_travel_hist = [
            sum_susp_travel[0] / safe_div_len,
            sum_susp_travel[1] / safe_div_len,
            sum_susp_travel[2] / safe_div_len,
            sum_susp_travel[3] / safe_div_len,
        ];

        let pressure_deviation = press_dev_acc / (safe_div_len * 4.0);
        let avg_pressure = press_sum / (safe_div_len * 4.0);

        let avg_wheels_pressure = [
            sum_wheels_pressure[0] / safe_div_len,
            sum_wheels_pressure[1] / safe_div_len,
            sum_wheels_pressure[2] / safe_div_len,
            sum_wheels_pressure[3] / safe_div_len,
        ];
        let avg_tyre_temp_i = [
            sum_tyre_temp_i[0] / safe_div_len,
            sum_tyre_temp_i[1] / safe_div_len,
            sum_tyre_temp_i[2] / safe_div_len,
            sum_tyre_temp_i[3] / safe_div_len,
        ];
        let avg_tyre_temp_m = [
            sum_tyre_temp_m[0] / safe_div_len,
            sum_tyre_temp_m[1] / safe_div_len,
            sum_tyre_temp_m[2] / safe_div_len,
            sum_tyre_temp_m[3] / safe_div_len,
        ];
        let avg_tyre_temp_o = [
            sum_tyre_temp_o[0] / safe_div_len,
            sum_tyre_temp_o[1] / safe_div_len,
            sum_tyre_temp_o[2] / safe_div_len,
            sum_tyre_temp_o[3] / safe_div_len,
        ];
        let avg_brake_temp = [
            sum_brake_temp_avg[0] / safe_div_len,
            sum_brake_temp_avg[1] / safe_div_len,
            sum_brake_temp_avg[2] / safe_div_len,
            sum_brake_temp_avg[3] / safe_div_len,
        ];
        let avg_ride_height = [
            sum_ride_height[0] / safe_div_len,
            sum_ride_height[1] / safe_div_len,
        ];

        let mistakes = (oversteer_c + understeer_c + lockup_c) as f32;
        let control_score = (100.0 - (mistakes / 10.0)).clamp(0.0, 100.0);
        let aggro_score = (grip_usage_percent + full_throttle_pct) / 2.0;

        let consistency_score = if let Some(best_idx) = self.best_lap_index {
            if best_idx < self.laps.len() {
                let diff = (lap_time_ms - self.laps[best_idx].lap_time_ms).abs();
                (100.0 - (diff as f32 / 500.0) * 10.0).clamp(0.0, 100.0)
            } else {
                100.0
            }
        } else {
            100.0
        };

        let tyre_score = (100.0 - pressure_deviation * 20.0).clamp(0.0, 100.0);

        let radar = RadarStats {
            smoothness: (throttle_smoothness + steering_smoothness) / 2.0 / 100.0,
            aggression: aggro_score / 100.0,
            consistency: consistency_score / 100.0,
            car_control: control_score / 100.0,
            tyre_mgmt: tyre_score / 100.0,
        };

        let mut trace = Vec::new();

        let step = 5;

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for (i, p) in physics_log.iter().enumerate() {
            if i % step == 0 {
                let g = if i < graphics_log.len() {
                    &graphics_log[i]
                } else {
                    match graphics_log.last() {
                        Some(last) => last,
                        None => continue,
                    }
                };

                let x = g.car_coordinates[0][0];
                let z = g.car_coordinates[0][2];

                if x.abs() > 0.1 || z.abs() > 0.1 {
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if z < min_y {
                        min_y = z;
                    }
                    if z > max_y {
                        max_y = z;
                    }
                }

                let wheel_slip = p.wheel_slip;
                let slip_avg = if !wheel_slip.is_empty() {
                    wheel_slip.iter().sum::<f32>() / wheel_slip.len() as f32
                } else {
                    0.0
                };

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
                    slip_avg,
                    x,
                    y: z,
                });
            }
        }

        trace.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(Ordering::Equal)
        });

        let corner_points: Vec<&TelemetryPoint> = trace
            .iter()
            .filter(|p| p.speed > 30.0 && p.lat_g.abs() > 0.5)
            .collect();

        let min_corner_speed_avg = if !corner_points.is_empty() {
            corner_points.iter().map(|p| p.speed).sum::<f32>() / corner_points.len() as f32
        } else {
            0.0
        };

        let lap_data = LapData {
            lap_number,
            lap_time_ms,
            sectors,
            valid: true,
            car_model: car_name,
            track_name,
            save_date,
            from_file: false,
            air_temp,
            road_temp,
            track_grip,
            timestamp,
            max_speed,
            avg_speed,
            avg_pressure,
            min_corner_speed_avg,
            fuel_used,
            gear_shifts,
            peak_lat_g,
            peak_brake_g: peak_brake_g.abs(),
            avg_tyre_temp,
            max_brake_temp,
            pressure_deviation,
            suspension_travel_hist,
            avg_wheels_pressure,
            avg_tyre_temp_i,
            avg_tyre_temp_m,
            avg_tyre_temp_o,
            avg_brake_temp,
            avg_ride_height,
            damper_histograms,
            throttle_smoothness,
            steering_smoothness,
            trail_braking_score: trail_score,
            coasting_percent: coasting_pct,
            pedal_overlap_percent: overlap_pct,
            full_throttle_percent: full_throttle_pct,
            grip_usage_percent,
            oversteer_count: oversteer_c / 5,
            understeer_count: understeer_c / 5,
            lockup_count: lockup_c / 5,
            car_control_score: control_score,
            radar_stats: radar,
            telemetry_trace: trace,
            bounds_min_x: min_x,
            bounds_max_x: max_x,
            bounds_min_y: min_y,
            bounds_max_y: max_y,
        };

        self.laps.push(lap_data);
        info!("Lap {} successfully added to telemetry stack.", lap_number);

        if let Some(best_idx) = self.best_lap_index {
            if best_idx < self.laps.len() {
                if lap_time_ms < self.laps[best_idx].lap_time_ms && lap_time_ms > 10000 {
                    self.best_lap_index = Some(self.laps.len() - 1);
                }
            } else {
                self.best_lap_index = Some(self.laps.len() - 1);
            }
        } else if lap_time_ms > 10000 {
            self.best_lap_index = Some(self.laps.len() - 1);
        }
    }

    pub fn analyze_standalone(&self, lap: &LapData, _lang: &Language) -> StandaloneAnalysis {
        let mut advices = Vec::new();

        if lap.pressure_deviation > 0.5 {
            let target = 27.5;
            let diff = lap.avg_pressure - target;

            if diff > 0.5 {
                advices.push(Advice {
                    zone: "Tyres".into(),
                    problem: format!("Pressure High: {:.1} psi", lap.avg_pressure),
                    solution: format!("Deflate tyres by {:.1} psi.", diff),
                    severity: 3,
                });
            } else if diff < -0.5 {
                advices.push(Advice {
                    zone: "Tyres".into(),
                    problem: format!("Pressure Low: {:.1} psi", lap.avg_pressure),
                    solution: format!("Inflate tyres by {:.1} psi.", diff.abs()),
                    severity: 3,
                });
            }
        }

        if let Some(wr) = &self.world_record {
            let diff = (lap.lap_time_ms - wr.time_ms) as f32 / 1000.0;
            if diff > 5.0 {
                advices.push(Advice {
                    zone: "Pace".into(),
                    problem: format!("Off WR Pace by +{:.1}s", diff),
                    solution: "Focus on corner exit speed.".into(),
                    severity: 1,
                });
            }
        }

        if lap.track_grip < 96.0 {
            advices.push(Advice {
                zone: "Track".into(),
                problem: format!("Low Grip: {:.1}%", lap.track_grip),
                solution: "Brake earlier, smooth throttle.".into(),
                severity: 2,
            });
        }

        let max_b = lap.max_brake_temp.iter().cloned().fold(f32::MIN, f32::max);
        if max_b > 750.0 {
            let diff = max_b - 750.0;
            advices.push(Advice {
                zone: "Brakes".into(),
                problem: format!("Overheating: {:.0}°C (+{:.0})", max_b, diff),
                solution: "Open ducts or increase ABS.".into(),
                severity: 3,
            });
        }

        if lap.lockup_count > 0 {
            advices.push(Advice {
                zone: "Lockup".into(),
                problem: format!("{} Lockups detected", lap.lockup_count),
                solution: "Reduce peak pressure or bias rear.".into(),
                severity: 3,
            });
        }

        let front_fast_bump = (lap.damper_histograms[0][1] + lap.damper_histograms[1][1]) / 2.0;
        if front_fast_bump > 35.0 {
            advices.push(Advice {
                zone: "Suspension".into(),
                problem: format!("High Front Fast Bump ({:.0}%)", front_fast_bump),
                solution: "Suspension bottoming out over kerbs. Stiffen front Fast Bump.".into(),
                severity: 2,
            });
        }

        StandaloneAnalysis {
            is_perfect: advices.is_empty(),
            advices,
        }
    }
}
