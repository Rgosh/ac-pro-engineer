use crate::ac_structs::{AcGraphics, AcPhysics};
use crate::config::{AppConfig, Language};
use crate::session_info::SessionInfo;
use crate::setup_manager::CarSetup;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info};

#[derive(Debug, Serialize, Clone)]
pub struct Recommendation {
    pub component: String,
    pub category: String,
    pub severity: Severity,
    pub message: String,
    pub action: String,
    pub parameters: Vec<Parameter>,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct Parameter {
    pub name: String,
    pub current: f32,
    pub target: f32,
    pub unit: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, PartialOrd)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct DrivingStyle {
    pub smoothness: f32,
    pub aggression: f32,
    pub consistency: f32,
    pub trail_braking: f32,
    pub throttle_control: f32,
    pub prev_gas: f32,
    pub prev_brake: f32,
    pub prev_steer: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WizardPhase {
    Entry,
    Apex,
    Exit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WizardProblem {
    Understeer,
    Oversteer,
    Instability,
}

pub struct Engineer {
    config: AppConfig,
    history_size: usize,
    pub stats: EngineerStats,
    pub driving_style: DrivingStyle,
    pub wizard_phase: WizardPhase,
    pub wizard_problem: WizardProblem,
    alert_timers: HashMap<String, (Instant, Instant)>,
}

#[derive(Debug, Clone)]
pub struct EngineerStats {
    pub bottoming_frames: [u32; 4],
    pub lockup_frames_front: u32,
    pub lockup_frames_rear: u32,
    pub wheel_spin_frames: u32,
    pub traction_loss_frames: u32,
    pub oversteer_frames: u32,
    pub understeer_frames: u32,
    pub coasting_frames: u32,
    pub scrubbing_frames: u32,
    pub current_excess_steer: f32,
    pub total_frames: u32,
    pub ffb_clip_frames: u32,
    pub input_history: crate::RingBuffer<(f64, f64, f64, f64, f64)>,
    pub fuel_laps_remaining: f32,
    pub fuel_consumption_rate: f32,
    pub current_delta: f32,
    pub predicted_lap_time: f32,
    pub low_speed_rake: f32,
    pub high_speed_rake: f32,
    pub base_tyre_wear: [f32; 4],
    pub stint_laps: i32,
    pub last_lap_count: i32,
    pub tyre_laps_remaining: [f32; 4],
}

impl Default for EngineerStats {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineerStats {
    pub fn new() -> Self {
        Self {
            bottoming_frames: [0; 4],
            lockup_frames_front: 0,
            lockup_frames_rear: 0,
            wheel_spin_frames: 0,
            traction_loss_frames: 0,
            oversteer_frames: 0,
            understeer_frames: 0,
            coasting_frames: 0,
            scrubbing_frames: 0,
            current_excess_steer: 0.0,
            total_frames: 0,
            ffb_clip_frames: 0,
            input_history: crate::RingBuffer::new(300),
            fuel_laps_remaining: 0.0,
            fuel_consumption_rate: 0.0,
            current_delta: 0.0,
            predicted_lap_time: 0.0,
            low_speed_rake: 0.0,
            high_speed_rake: 0.0,
            base_tyre_wear: [100.0; 4],
            stint_laps: 0,
            last_lap_count: -1,
            tyre_laps_remaining: [99.0; 4],
        }
    }
}

impl Default for DrivingStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl DrivingStyle {
    pub fn new() -> Self {
        Self {
            smoothness: 50.0,
            aggression: 50.0,
            consistency: 50.0,
            trail_braking: 0.0,
            throttle_control: 50.0,
            prev_gas: 0.0,
            prev_brake: 0.0,
            prev_steer: 0.0,
        }
    }
}

impl Engineer {
    pub fn new(config: &AppConfig) -> Self {
        info!("Engineer module initialized.");
        Self {
            config: config.clone(),
            history_size: 600,
            stats: EngineerStats::new(),
            driving_style: DrivingStyle::new(),
            wizard_phase: WizardPhase::Entry,
            wizard_problem: WizardProblem::Understeer,
            alert_timers: HashMap::new(),
        }
    }

    pub fn update_config(&mut self, config: &AppConfig) {
        self.config = config.clone();
        self.stats.input_history.set_capacity(config.history_size);
    }

    pub fn update(&mut self, phys: &AcPhysics, gfx: &AcGraphics, _session: &SessionInfo) {
        self.update_stats(phys, gfx);
        self.analyze_driving_style(phys);

        if self.stats.total_frames > self.history_size as u32 {
            debug!("Engineer history buffer reached limit, resetting counters.");
            self.reset_counters();
        }
    }

    fn update_stats(&mut self, phys: &AcPhysics, gfx: &AcGraphics) {
        let dt_sec = (self.config.update_rate as f32 / 1000.0).clamp(0.001, 1.0);
        let ticks_norm = (dt_sec * 60.0).round().max(1.0) as u32;

        self.stats.total_frames += ticks_norm;

        if phys.final_ff.abs() > 0.98 {
            self.stats.ffb_clip_frames += ticks_norm;
        }

        if self.stats.total_frames.is_multiple_of(3) {
            let t = self.stats.total_frames as f64;
            self.stats.input_history.push((
                t,
                phys.steer_angle as f64,
                phys.gas as f64,
                phys.brake as f64,
                phys.final_ff as f64,
            ));
        }

        for i in 0..4 {
            if phys.suspension_travel[i] < 0.005 {
                self.stats.bottoming_frames[i] += ticks_norm;
            }
        }

        let rake = phys.ride_height[1] - phys.ride_height[0];
        let rake_mm = rake * 1000.0;
        if phys.speed_kmh > 50.0 && phys.speed_kmh < 90.0 {
            if self.stats.low_speed_rake == 0.0 {
                self.stats.low_speed_rake = rake_mm;
            }
            self.stats.low_speed_rake = self.stats.low_speed_rake * 0.98 + rake_mm * 0.02;
        } else if phys.speed_kmh > 160.0 {
            if self.stats.high_speed_rake == 0.0 {
                self.stats.high_speed_rake = rake_mm;
            }
            self.stats.high_speed_rake = self.stats.high_speed_rake * 0.98 + rake_mm * 0.02;
        }

        let current_laps = gfx.completed_laps;
        if current_laps != self.stats.last_lap_count {
            if self.stats.last_lap_count == -1 || current_laps == 0 || phys.speed_kmh < 10.0 {
                self.stats.base_tyre_wear = phys.tyre_wear;
                self.stats.stint_laps = 0;
            } else {
                self.stats.stint_laps += 1;
                for i in 0..4 {
                    let wear_used = self.stats.base_tyre_wear[i] - phys.tyre_wear[i];
                    if wear_used > 0.0 && self.stats.stint_laps > 0 {
                        let wear_per_lap = wear_used / self.stats.stint_laps as f32;
                        let replacement_threshold =
                            (self.config.alerts.wear_warning - 2.0).max(0.0);
                        let remaining_wear = phys.tyre_wear[i] - replacement_threshold;
                        if wear_per_lap > 0.001 {
                            let laps = (remaining_wear / wear_per_lap).max(0.0);
                            self.stats.tyre_laps_remaining[i] =
                                if laps > 500.0 { 500.0 } else { laps };
                        }
                    }
                }
            }
            self.stats.last_lap_count = current_laps;
        }

        if phys.speed_kmh > 30.0 {
            if (phys.wheel_slip[0].abs() > 0.2 || phys.wheel_slip[1].abs() > 0.2)
                && phys.brake > 0.1
            {
                self.stats.lockup_frames_front += ticks_norm;
            }
            if (phys.wheel_slip[2].abs() > 0.2 || phys.wheel_slip[3].abs() > 0.2)
                && phys.brake > 0.1
            {
                self.stats.lockup_frames_rear += ticks_norm;
            }
        }

        for i in 0..4 {
            if phys.wheel_slip[i] > 0.15 && phys.gas > 0.3 && phys.speed_kmh < 120.0 {
                self.stats.wheel_spin_frames += ticks_norm;
            }
        }

        if phys.speed_kmh > 30.0 && phys.gas < 0.05 && phys.brake < 0.05 {
            self.stats.coasting_frames += ticks_norm;
        }

        if phys.speed_kmh > 40.0 {
            let front_slip = phys.wheel_slip[0].max(phys.wheel_slip[1]);
            let rear_slip = phys.wheel_slip[2].max(phys.wheel_slip[3]);

            if front_slip > 0.15 && front_slip > rear_slip + 0.05 && phys.steer_angle.abs() > 0.15 {
                self.stats.understeer_frames += ticks_norm;
                self.stats.scrubbing_frames += ticks_norm;
                let excess = (phys.steer_angle.abs() - 0.15) * 57.2958;
                if excess > self.stats.current_excess_steer {
                    self.stats.current_excess_steer = excess;
                }
            } else if rear_slip > 0.15 && rear_slip > front_slip + 0.05 {
                self.stats.oversteer_frames += ticks_norm;
            }
        } else if self.stats.scrubbing_frames > 0 && self.stats.scrubbing_frames < 45 {
            self.stats.scrubbing_frames = 0;
            self.stats.current_excess_steer = 0.0;
        }

        if gfx.fuel_x_lap > 0.0 {
            self.stats.fuel_laps_remaining = phys.fuel / gfx.fuel_x_lap;
        }

        self.stats.current_delta = phys.performance_meter;

        if gfx.i_best_time > 0 {
            self.stats.predicted_lap_time =
                (gfx.i_best_time as f32 / 1000.0) + phys.performance_meter;
        } else if gfx.i_last_time > 0 {
            self.stats.predicted_lap_time = gfx.i_last_time as f32 / 1000.0;
        }
    }

    fn analyze_driving_style(&mut self, phys: &AcPhysics) {
        let gas_diff = (phys.gas - self.driving_style.prev_gas).abs();
        let brake_diff = (phys.brake - self.driving_style.prev_brake).abs();
        let steer_diff = (phys.steer_angle - self.driving_style.prev_steer).abs();

        let throttle_smoothness = (100.0 - (gas_diff * 1000.0)).clamp(0.0, 100.0);
        let brake_smoothness = (100.0 - (brake_diff * 1000.0)).clamp(0.0, 100.0);
        let steer_smoothness = (100.0 - (steer_diff * 500.0)).clamp(0.0, 100.0);

        self.driving_style.smoothness = 0.95 * self.driving_style.smoothness
            + 0.05 * (throttle_smoothness + brake_smoothness + steer_smoothness) / 3.0;

        self.driving_style.prev_gas = phys.gas;
        self.driving_style.prev_brake = phys.brake;
        self.driving_style.prev_steer = phys.steer_angle;

        let lateral_g = (phys.acc_g[0].powi(2) + phys.acc_g[1].powi(2)).sqrt();
        self.driving_style.aggression =
            0.9 * self.driving_style.aggression + 0.1 * lateral_g.min(2.5) / 2.5 * 100.0;

        if phys.brake > 0.1 && phys.steer_angle.abs() > 0.1 {
            self.driving_style.trail_braking =
                0.95 * self.driving_style.trail_braking + 0.05 * 100.0;
        } else {
            self.driving_style.trail_braking *= 0.98;
        }
    }

    fn reset_counters(&mut self) {
        self.stats.bottoming_frames = [0; 4];
        self.stats.lockup_frames_front = 0;
        self.stats.lockup_frames_rear = 0;
        self.stats.wheel_spin_frames = 0;
        self.stats.traction_loss_frames = 0;
        self.stats.oversteer_frames = 0;
        self.stats.understeer_frames = 0;
        self.stats.coasting_frames = 0;
        self.stats.scrubbing_frames = 0;
        self.stats.current_excess_steer = 0.0;
        self.stats.total_frames = 0;
        self.stats.ffb_clip_frames = 0;
    }

    fn check_hysteresis(&mut self, key: &str, active: bool) -> bool {
        let now = Instant::now();
        if active {
            let first_seen = if let Some(&(first, _)) = self.alert_timers.get(key) {
                first
            } else {
                now
            };
            self.alert_timers.insert(key.to_string(), (first_seen, now));
            return now.duration_since(first_seen) >= Duration::from_secs_f32(1.0);
        }

        if let Some(&(first_seen, last_seen)) = self.alert_timers.get(key)
            && now.duration_since(last_seen) < Duration::from_secs_f32(2.0)
        {
            return now.duration_since(first_seen) >= Duration::from_secs_f32(1.0);
        }

        self.alert_timers.remove(key);
        false
    }

    pub fn analyze_live(
        &mut self,
        phys: &AcPhysics,
        gfx: &AcGraphics,
        setup: Option<&CarSetup>,
    ) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        self.analyze_tyre_pressure(phys, gfx, &mut recommendations);
        self.analyze_tyre_temperature(phys, &mut recommendations);
        self.analyze_tyre_wear(phys, &mut recommendations);

        self.analyze_camber(phys, setup, &mut recommendations);
        self.analyze_suspension(phys, &mut recommendations);
        self.analyze_brakes(phys, &mut recommendations);
        self.analyze_brake_bias(setup, &mut recommendations);
        self.analyze_aero(phys, &mut recommendations);

        self.analyze_driving_errors(&mut recommendations);
        self.analyze_strategy(phys, gfx, &mut recommendations);
        self.analyze_ffb_clipping(phys, &mut recommendations);

        recommendations.sort_by(|a, b| {
            b.severity
                .partial_cmp(&a.severity)
                .unwrap_or(Ordering::Equal)
                .then(
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(Ordering::Equal),
                )
        });

        recommendations
    }

    fn analyze_suspension(&mut self, _phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        let mut bottoming_detected = false;
        for i in 0..4 {
            if self.stats.bottoming_frames[i] > 30 {
                bottoming_detected = true;
                break;
            }
        }
        if self.check_hysteresis("bottoming", bottoming_detected) && bottoming_detected {
            recs.push(Recommendation {
                component: if ru {
                    "Подвеска".to_string()
                } else {
                    "Suspension".to_string()
                },
                category: if ru {
                    "Пробой".to_string()
                } else {
                    "Bottoming".to_string()
                },
                severity: Severity::Critical,
                message: if ru {
                    "Удары днищем о трассу!".to_string()
                } else {
                    "Chassis bottoming out!".to_string()
                },
                action: if ru {
                    "Увеличьте клиренс или жесткость".to_string()
                } else {
                    "Increase ride height or stiffness".to_string()
                },
                parameters: vec![],
                confidence: 0.95,
            });
        }
    }

    fn analyze_aero(&mut self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        if self.stats.high_speed_rake != 0.0
            && self.stats.low_speed_rake != 0.0
            && phys.speed_kmh > 150.0
        {
            let rake_loss = self.stats.low_speed_rake - self.stats.high_speed_rake;
            if self.check_hysteresis("aero_rake", rake_loss > 10.0) && rake_loss > 10.0 {
                recs.push(Recommendation {
                    component: if ru {
                        "Аэродинамика".to_string()
                    } else {
                        "Aerodynamics".to_string()
                    },
                    category: "Rake Loss".to_string(),
                    severity: Severity::Warning,
                    message: if ru {
                        format!("Зад сильно проседает на скорости (-{:.1}мм)", rake_loss)
                    } else {
                        format!("Rear dropping too much at high speed (-{:.1}mm)", rake_loss)
                    },
                    action: if ru {
                        "Увеличьте жесткость задних пружин (Rear Springs) или Packer".to_string()
                    } else {
                        "Stiffen Rear Springs or add Packers".to_string()
                    },
                    parameters: vec![],
                    confidence: 0.85,
                });
            }
        }
    }

    pub fn get_wizard_advice(&self) -> Vec<String> {
        let is_ru = self.config.language == Language::Russian;
        let mut advice = Vec::new();

        match (&self.wizard_phase, &self.wizard_problem) {
            (WizardPhase::Entry, WizardProblem::Understeer) => {
                advice.push(if is_ru {
                    "Уменьшить отбой (Rebound) спереди".to_string()
                } else {
                    "Decrease Front Rebound".to_string()
                });
                advice.push(if is_ru {
                    "Увеличить клиренс сзади".to_string()
                } else {
                    "Increase Rear Ride Height".to_string()
                });
                advice.push(if is_ru {
                    "Сместить тормозной баланс назад".to_string()
                } else {
                    "Move Brake Bias Rearwards".to_string()
                });
            }
            (WizardPhase::Entry, WizardProblem::Oversteer) => {
                advice.push(if is_ru {
                    "Увеличить отбой (Rebound) спереди".to_string()
                } else {
                    "Increase Front Rebound".to_string()
                });
                advice.push(if is_ru {
                    "Сместить тормозной баланс вперед".to_string()
                } else {
                    "Move Brake Bias Forwards".to_string()
                });
                advice.push(if is_ru {
                    "Увеличить переднее антикрыло".to_string()
                } else {
                    "Increase Front Wing".to_string()
                });
            }
            (WizardPhase::Apex, WizardProblem::Understeer) => {
                advice.push(if is_ru {
                    "Мягче передние пружины".to_string()
                } else {
                    "Softer Front Springs".to_string()
                });
                advice.push(if is_ru {
                    "Мягче передний стабилизатор (ARB)".to_string()
                } else {
                    "Softer Front ARB".to_string()
                });
                advice.push(if is_ru {
                    "Больше развал (Camber) спереди".to_string()
                } else {
                    "More Front Camber".to_string()
                });
            }
            (WizardPhase::Apex, WizardProblem::Oversteer) => {
                advice.push(if is_ru {
                    "Мягче задние пружины".to_string()
                } else {
                    "Softer Rear Springs".to_string()
                });
                advice.push(if is_ru {
                    "Мягче задний стабилизатор (ARB)".to_string()
                } else {
                    "Softer Rear ARB".to_string()
                });
                advice.push(if is_ru {
                    "Выше клиренс спереди".to_string()
                } else {
                    "Increase Front Ride Height".to_string()
                });
            }
            (WizardPhase::Exit, WizardProblem::Understeer) => {
                advice.push(if is_ru {
                    "Увеличить сжатие (Bump) спереди".to_string()
                } else {
                    "Increase Front Bump".to_string()
                });
                advice.push(if is_ru {
                    "Жестче задние пружины".to_string()
                } else {
                    "Stiffer Rear Springs".to_string()
                });
                advice.push(if is_ru {
                    "Увеличить блокировку дифференциала (Power)".to_string()
                } else {
                    "Increase Diff Power".to_string()
                });
            }
            (WizardPhase::Exit, WizardProblem::Oversteer) => {
                advice.push(if is_ru {
                    "Мягче задние пружины".to_string()
                } else {
                    "Softer Rear Springs".to_string()
                });
                advice.push(if is_ru {
                    "Уменьшить сжатие (Bump) сзади".to_string()
                } else {
                    "Decrease Rear Bump".to_string()
                });
                advice.push(if is_ru {
                    "Уменьшить блокировку дифференциала (Power)".to_string()
                } else {
                    "Decrease Diff Power".to_string()
                });
                advice.push(if is_ru {
                    "Больше Traction Control".to_string()
                } else {
                    "Increase TC".to_string()
                });
            }
            (_, WizardProblem::Instability) => {
                advice.push(if is_ru {
                    "Увеличить прижимную силу (Крылья)".to_string()
                } else {
                    "Increase Downforce (Wings)".to_string()
                });
                advice.push(if is_ru {
                    "Больше схождения (Toe) сзади".to_string()
                } else {
                    "More Rear Toe-In".to_string()
                });
                advice.push(if is_ru {
                    "Жестче подвеску в целом".to_string()
                } else {
                    "Stiffer Suspension Overall".to_string()
                });
            }
        }
        advice
    }

    fn is_ru(&self) -> bool {
        self.config.language == Language::Russian
    }

    pub fn compare_setups_advice(&self, target: &CarSetup, reference: &CarSetup) -> Vec<String> {
        let mut advice = Vec::new();
        let ru = self.is_ru();

        let aero_diff =
            (target.wing_1 + target.wing_2) as i32 - (reference.wing_1 + reference.wing_2) as i32;
        if aero_diff != 0 {
            advice.push(if ru {
                format!("Аэродинамика: {:+}", aero_diff)
            } else {
                format!("Aero: {:+}", aero_diff)
            });
        }

        let camber_f_diff =
            (target.camber_lf + target.camber_rf) - (reference.camber_lf + reference.camber_rf);
        if camber_f_diff.abs() > 2 {
            advice.push(if ru {
                format!("Развал перед: {:+}", camber_f_diff)
            } else {
                format!("Front Camber: {:+}", camber_f_diff)
            });
        }

        let avg_p_target: f32 =
            (target.pressure_lf + target.pressure_rf + target.pressure_lr + target.pressure_rr)
                as f32
                / 4.0;
        let avg_p_ref: f32 = (reference.pressure_lf
            + reference.pressure_rf
            + reference.pressure_lr
            + reference.pressure_rr) as f32
            / 4.0;
        if (avg_p_target - avg_p_ref).abs() > 1.0 {
            advice.push(if ru {
                format!("Давление шин: {:+.1} PSI", avg_p_target - avg_p_ref)
            } else {
                format!("Tyre Press: {:+.1} PSI", avg_p_target - avg_p_ref)
            });
        }

        if advice.is_empty() {
            advice.push(if ru {
                "Нет существенных отличий".to_string()
            } else {
                "No major differences".to_string()
            });
        }
        advice
    }

    fn analyze_ffb_clipping(&mut self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        let clip_ratio = if self.stats.total_frames > 0 {
            self.stats.ffb_clip_frames as f32 / self.stats.total_frames as f32
        } else {
            0.0
        };

        let is_clipping = clip_ratio > 0.05 && phys.speed_kmh > 10.0;

        if self.check_hysteresis("ffb_clip", is_clipping) && is_clipping {
            recs.push(Recommendation {
                component: if ru {
                    "Руль (FFB)".to_string()
                } else {
                    "Force Feedback".to_string()
                },
                category: "Clipping".to_string(),
                severity: Severity::Warning,
                message: if ru {
                    format!("Клиппинг силы: {:.1}% времени", clip_ratio * 100.0)
                } else {
                    format!("FFB Clipping: {:.1}% of time", clip_ratio * 100.0)
                },
                action: if ru {
                    "Снизить Gain".to_string()
                } else {
                    "Lower FFB Gain".to_string()
                },
                parameters: vec![Parameter {
                    name: "Clip Ratio".to_string(),
                    current: clip_ratio * 100.0,
                    target: 0.0,
                    unit: "%".to_string(),
                }],
                confidence: 1.0,
            });
        }
    }

    fn analyze_tyre_pressure(
        &mut self,
        phys: &AcPhysics,
        gfx: &AcGraphics,
        recs: &mut Vec<Recommendation>,
    ) {
        let ru = self.is_ru();

        let compound_name = gfx.tyre_compound.to_string().to_lowercase();

        let class_name = if compound_name.contains("street")
            || compound_name.contains("sport")
            || compound_name.contains("eco")
            || compound_name.contains("semislick")
        {
            "Street"
        } else if compound_name.contains("wet") || compound_name.contains("rain") {
            "Wet"
        } else {
            "Racing"
        };
        let pressure_min = self
            .config
            .alerts
            .tyre_pressure_min
            .min(self.config.alerts.tyre_pressure_max);
        let pressure_max = self
            .config
            .alerts
            .tyre_pressure_min
            .max(self.config.alerts.tyre_pressure_max);
        let base_optimal = if self.config.target_tyre_pressure > 0.0 {
            self.config.target_tyre_pressure
        } else {
            (pressure_min + pressure_max) / 2.0
        };

        let grip_compensation = (1.0 - gfx.surface_grip.clamp(0.80, 1.0)) * 1.5;
        let optimal_pressure = base_optimal + grip_compensation;

        for i in 0..4 {
            let pressure = phys.wheels_pressure[i];
            let diff = (pressure - optimal_pressure).abs();
            let is_error = pressure < pressure_min || pressure > pressure_max;

            let key = format!("pres_{}", i);
            if self.check_hysteresis(&key, is_error) && phys.speed_kmh > 10.0 {
                if !is_error {
                    continue;
                }

                let name = match i {
                    0 => "FL",
                    1 => "FR",
                    2 => "RL",
                    3 => "RR",
                    _ => "",
                };
                let action_text = if pressure < optimal_pressure {
                    if ru { "Накачать" } else { "Inflate" }
                } else {
                    if ru { "Спустить" } else { "Deflate" }
                }
                .to_string();

                recs.push(Recommendation {
                    component: if ru {
                        format!("Шины ({})", class_name)
                    } else {
                        format!("Tyres ({})", class_name)
                    },
                    category: if ru {
                        "Давление".to_string()
                    } else {
                        "Pressure".to_string()
                    },
                    severity: if diff > 2.5 {
                        Severity::Warning
                    } else {
                        Severity::Info
                    },
                    message: if ru {
                        format!(
                            "{} Давление: {:.1} (Цель: {:.1})",
                            name, pressure, optimal_pressure
                        )
                    } else {
                        format!(
                            "{} Pressure: {:.1} (Target: {:.1})",
                            name, pressure, optimal_pressure
                        )
                    },
                    action: action_text,
                    parameters: vec![Parameter {
                        name: "Delta".to_string(),
                        current: pressure,
                        target: optimal_pressure,
                        unit: "PSI".to_string(),
                    }],
                    confidence: 0.9,
                });
            }
        }
    }

    fn analyze_tyre_wear(&mut self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();

        for i in 0..4 {
            let wear = phys.tyre_wear[i];
            let warning_threshold = self.config.alerts.wear_warning;
            let is_worn = wear < warning_threshold;

            if self.check_hysteresis(&format!("wear_{}", i), is_worn) {
                if !is_worn {
                    continue;
                }

                let name = match i {
                    0 => "FL",
                    1 => "FR",
                    2 => "RL",
                    3 => "RR",
                    _ => "",
                };
                let (severity, msg_en, msg_ru) = if wear < (warning_threshold - 2.0).max(0.0) {
                    (Severity::Critical, "WORN OUT", "ИЗНОС (Крит)")
                } else {
                    (Severity::Warning, "High Wear", "Сильный износ")
                };

                recs.push(Recommendation {
                    component: if ru {
                        "Шины".to_string()
                    } else {
                        "Tyres".to_string()
                    },
                    category: if ru {
                        "Износ".to_string()
                    } else {
                        "Wear".to_string()
                    },
                    severity,
                    message: if ru {
                        format!("{} {}: {:.1}%", name, msg_ru, wear)
                    } else {
                        format!("{} {}: {:.1}%", name, msg_en, wear)
                    },
                    action: if ru {
                        "Пит-стоп / Осторожно".to_string()
                    } else {
                        "Box / Careful".to_string()
                    },
                    parameters: vec![Parameter {
                        name: "Life".to_string(),
                        current: wear,
                        target: 100.0,
                        unit: "%".to_string(),
                    }],
                    confidence: 0.9,
                });
            }
        }
    }

    fn analyze_camber(
        &self,
        phys: &AcPhysics,
        setup: Option<&CarSetup>,
        recs: &mut Vec<Recommendation>,
    ) {
        let ru = self.is_ru();
        if phys.speed_kmh < 50.0 {
            return;
        }
        let ideal_spread = 8.0;

        for i in 0..4 {
            let temp_i = phys.tyre_temp_i[i];
            let temp_o = phys.tyre_temp_o[i];
            let spread = temp_i - temp_o;

            let name = match i {
                0 => "FL",
                1 => "FR",
                2 => "RL",
                3 => "RR",
                _ => "",
            };

            if spread < 2.0 {
                let action_text = if let Some(s) = setup {
                    let current_camber = match i {
                        0 => s.camber_lf,
                        1 => s.camber_rf,
                        2 => s.camber_lr,
                        3 => s.camber_rr,
                        _ => 0,
                    };
                    if ru {
                        format!(
                            "Увеличить развал (сейчас: {}). Если предел -> смягчите ARB",
                            current_camber
                        )
                    } else {
                        format!(
                            "Increase Camber (now: {}). If maxed -> soften ARB",
                            current_camber
                        )
                    }
                } else {
                    if ru {
                        "Увеличить отриц. развал".to_string()
                    } else {
                        "Increase Neg. Camber".to_string()
                    }
                };

                recs.push(Recommendation {
                    component: if ru {
                        "Подвеска".to_string()
                    } else {
                        "Suspension".to_string()
                    },
                    category: if ru {
                        "Развал".to_string()
                    } else {
                        "Camber".to_string()
                    },
                    severity: Severity::Info,
                    message: if ru {
                        format!(
                            "{} Пятно контакта не эффективно (I-O: {:.1}C)",
                            name, spread
                        )
                    } else {
                        format!("{} Contact patch inefficient (I-O: {:.1}C)", name, spread)
                    },
                    action: action_text,
                    parameters: vec![Parameter {
                        name: "Temp Spread".to_string(),
                        current: spread,
                        target: ideal_spread,
                        unit: self.config.formatter().temp_symbol().to_string(),
                    }],
                    confidence: 0.7,
                });
            } else if spread > 15.0 {
                let action_text = if let Some(s) = setup {
                    let current_camber = match i {
                        0 => s.camber_lf,
                        1 => s.camber_rf,
                        2 => s.camber_lr,
                        3 => s.camber_rr,
                        _ => 0,
                    };
                    if ru {
                        format!(
                            "Уменьшить развал (сейчас: {}). Если предел -> зажмите ARB",
                            current_camber
                        )
                    } else {
                        format!(
                            "Decrease Camber (now: {}). If maxed -> stiffen ARB",
                            current_camber
                        )
                    }
                } else {
                    if ru {
                        "Уменьшить отриц. развал".to_string()
                    } else {
                        "Decrease Neg. Camber".to_string()
                    }
                };

                recs.push(Recommendation {
                    component: if ru {
                        "Подвеска".to_string()
                    } else {
                        "Suspension".to_string()
                    },
                    category: if ru {
                        "Развал".to_string()
                    } else {
                        "Camber".to_string()
                    },
                    severity: Severity::Warning,
                    message: if ru {
                        format!("{} Перегрев внутренней части (I-O: {:.1}C)", name, spread)
                    } else {
                        format!("{} Inner edge overheating (I-O: {:.1}C)", name, spread)
                    },
                    action: action_text,
                    parameters: vec![Parameter {
                        name: "Temp Spread".to_string(),
                        current: spread,
                        target: ideal_spread,
                        unit: self.config.formatter().temp_symbol().to_string(),
                    }],
                    confidence: 0.8,
                });
            }
        }
    }

    fn analyze_tyre_temperature(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let min_temp = self
            .config
            .alerts
            .tyre_temp_min
            .min(self.config.alerts.tyre_temp_max);
        let max_temp = self
            .config
            .alerts
            .tyre_temp_min
            .max(self.config.alerts.tyre_temp_max);
        let ru = self.is_ru();

        if phys.speed_kmh > 100.0 {
            for i in 0..4 {
                let temp = phys.get_avg_tyre_temp(i);
                if temp < min_temp {
                    let name = match i {
                        0 => "FL",
                        1 => "FR",
                        2 => "RL",
                        3 => "RR",
                        _ => "",
                    };
                    recs.push(Recommendation {
                        component: if ru {
                            "Шины".to_string()
                        } else {
                            "Tyres".to_string()
                        },
                        category: if ru {
                            "Температура".to_string()
                        } else {
                            "Temperature".to_string()
                        },
                        severity: Severity::Warning,
                        message: if ru {
                            format!(
                                "{} ХОЛОДНАЯ: {}",
                                name,
                                self.config.formatter().format_temp(temp)
                            )
                        } else {
                            format!(
                                "{} COLD: {}",
                                name,
                                self.config.formatter().format_temp(temp)
                            )
                        },
                        action: if ru {
                            "Греть шины".to_string()
                        } else {
                            "Warm tyres".to_string()
                        },
                        parameters: vec![],
                        confidence: 0.95,
                    });
                } else if temp > max_temp {
                    let name = match i {
                        0 => "FL",
                        1 => "FR",
                        2 => "RL",
                        3 => "RR",
                        _ => "",
                    };
                    recs.push(Recommendation {
                        component: if ru {
                            "Шины".to_string()
                        } else {
                            "Tyres".to_string()
                        },
                        category: if ru {
                            "Перегрев".to_string()
                        } else {
                            "Overheat".to_string()
                        },
                        severity: Severity::Critical,
                        message: if ru {
                            format!(
                                "{} ПЕРЕГРЕВ: {}",
                                name,
                                self.config.formatter().format_temp(temp)
                            )
                        } else {
                            format!(
                                "{} OVERHEATING: {}",
                                name,
                                self.config.formatter().format_temp(temp)
                            )
                        },
                        action: if ru {
                            "Остудить шины".to_string()
                        } else {
                            "Cool tyres".to_string()
                        },
                        parameters: vec![],
                        confidence: 0.95,
                    });
                }
            }
        }
    }

    fn analyze_brakes(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let max_temp = self.config.alerts.brake_temp_max;
        let ru = self.is_ru();
        for i in 0..4 {
            if phys.brake_temp[i] > max_temp {
                recs.push(Recommendation {
                    component: if ru {
                        "Тормоза".to_string()
                    } else {
                        "Brakes".to_string()
                    },
                    category: if ru {
                        "Перегрев".to_string()
                    } else {
                        "Overheat".to_string()
                    },
                    severity: Severity::Critical,
                    message: if ru {
                        format!("Тормоз {} горит!", i + 1)
                    } else {
                        format!("Brake {} cooking!", i + 1)
                    },
                    action: if ru {
                        "Сместить баланс / Охладить".to_string()
                    } else {
                        "Move bias / Cool down".to_string()
                    },
                    parameters: vec![],
                    confidence: 1.0,
                });
            }
        }
    }

    fn analyze_brake_bias(&self, setup: Option<&CarSetup>, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        let total_lockups = self.stats.lockup_frames_front + self.stats.lockup_frames_rear;

        if total_lockups > 20 {
            let current_bias_str = if let Some(s) = setup {
                if ru {
                    format!(" (СЕЙЧАС: {}%)", s.brake_bias)
                } else {
                    format!(" (NOW: {}%)", s.brake_bias)
                }
            } else {
                "".to_string()
            };

            if self.stats.lockup_frames_front > self.stats.lockup_frames_rear * 2 {
                recs.push(Recommendation {
                    component: if ru {
                        "Тормоза".to_string()
                    } else {
                        "Brakes".to_string()
                    },
                    category: if ru {
                        "Баланс".to_string()
                    } else {
                        "Bias".to_string()
                    },
                    severity: Severity::Warning,
                    message: if ru {
                        format!("Блокировка ПЕРЕДНИХ колес{}", current_bias_str)
                    } else {
                        format!("FRONT Locking detected{}", current_bias_str)
                    },
                    action: if ru {
                        "Сместить баланс НАЗАД".to_string()
                    } else {
                        "Move Bias REARWARDS".to_string()
                    },
                    parameters: vec![],
                    confidence: 0.85,
                });
            } else if self.stats.lockup_frames_rear > self.stats.lockup_frames_front * 2 {
                recs.push(Recommendation {
                    component: if ru {
                        "Тормоза".to_string()
                    } else {
                        "Brakes".to_string()
                    },
                    category: if ru {
                        "Баланс".to_string()
                    } else {
                        "Bias".to_string()
                    },
                    severity: Severity::Critical,
                    message: if ru {
                        format!("Блокировка ЗАДНИХ колес{}", current_bias_str)
                    } else {
                        format!("REAR Locking (Danger!){}", current_bias_str)
                    },
                    action: if ru {
                        "Сместить баланс ВПЕРЕД".to_string()
                    } else {
                        "Move Bias FORWARDS".to_string()
                    },
                    parameters: vec![],
                    confidence: 0.95,
                });
            }
        }
    }

    fn analyze_driving_errors(&mut self, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();

        let is_coasting = self.stats.coasting_frames > 60;
        if self.check_hysteresis("coast", is_coasting) && is_coasting {
            recs.push(Recommendation {
                component: if ru {
                    "Пилотаж".to_string()
                } else {
                    "Driving".to_string()
                },
                category: if ru {
                    "Потеря времени".to_string()
                } else {
                    "Time Loss".to_string()
                },
                severity: Severity::Info,
                message: if ru {
                    "Много наката (Coasting)".to_string()
                } else {
                    "Excessive Coasting".to_string()
                },
                action: if ru {
                    "Держите газ или тормозите".to_string()
                } else {
                    "Keep throttle or brake".to_string()
                },
                parameters: vec![],
                confidence: 0.7,
            });
        }

        if self.stats.understeer_frames > 30 {
            recs.push(Recommendation {
                component: if ru {
                    "Баланс".to_string()
                } else {
                    "Balance".to_string()
                },
                category: "Understeer".to_string(),
                severity: Severity::Warning,
                message: if ru {
                    "Снос передней оси (High Speed)".to_string()
                } else {
                    "High Speed Understeer".to_string()
                },
                action: if ru {
                    "Больше крыла спереди / Мягче спереди".to_string()
                } else {
                    "More Front Wing / Softer Front".to_string()
                },
                parameters: vec![],
                confidence: 0.85,
            });
        }

        if self.stats.oversteer_frames > 30 {
            recs.push(Recommendation {
                component: if ru {
                    "Баланс".to_string()
                } else {
                    "Balance".to_string()
                },
                category: "Oversteer".to_string(),
                severity: Severity::Warning,
                message: if ru {
                    "Нестабильность сзади (High Speed)".to_string()
                } else {
                    "High Speed Oversteer".to_string()
                },
                action: if ru {
                    "Больше крыла сзади".to_string()
                } else {
                    "More Rear Wing".to_string()
                },
                parameters: vec![],
                confidence: 0.85,
            });
        }

        let is_scrubbing = self.stats.scrubbing_frames > 45;
        if self.check_hysteresis("scrubbing", is_scrubbing) && is_scrubbing {
            let excess = self.stats.current_excess_steer;
            recs.push(Recommendation {
                component: if ru {
                    "Пилотаж".to_string()
                } else {
                    "Driving".to_string()
                },
                category: if ru {
                    "Скраббинг".to_string()
                } else {
                    "Overdriving".to_string()
                },
                severity: Severity::Warning,
                message: if ru {
                    format!("Перекрут руля на {:.0}°! Шины скользят.", excess)
                } else {
                    format!("Steering over-rotated by {:.0}°! Tyres sliding.", excess)
                },
                action: if ru {
                    format!("Уменьши угол руля на {:.0}°", excess)
                } else {
                    format!("Reduce steering angle by {:.0}°", excess)
                },
                parameters: vec![],
                confidence: 0.95,
            });
            self.stats.scrubbing_frames = 0;
            self.stats.current_excess_steer = 0.0;
        }
    }

    fn analyze_strategy(&self, phys: &AcPhysics, gfx: &AcGraphics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();

        if self.stats.fuel_laps_remaining < self.config.alerts.fuel_warning_laps
            && self.stats.fuel_laps_remaining > 0.0
        {
            recs.push(Recommendation {
                component: if ru {
                    "Стратегия".to_string()
                } else {
                    "Strategy".to_string()
                },
                category: if ru {
                    "Топливо".to_string()
                } else {
                    "Fuel".to_string()
                },
                severity: Severity::Critical,
                message: if ru {
                    format!("ТОПЛИВО: {:.1} кр.", self.stats.fuel_laps_remaining)
                } else {
                    format!("FUEL LOW: {:.1} laps", self.stats.fuel_laps_remaining)
                },
                action: "BOX BOX BOX".to_string(),
                parameters: vec![],
                confidence: 1.0,
            });
        }

        if (gfx.session_time_left > 0.0 || gfx.number_of_laps > 0) && gfx.fuel_x_lap > 0.0 {
            let laps_remaining_in_race = crate::session_info::SessionTiming::remaining_laps(
                gfx.session_time_left,
                gfx.i_best_time,
                gfx.i_last_time,
                gfx.number_of_laps,
                gfx.completed_laps,
                gfx.normalized_car_position,
            );

            if laps_remaining_in_race > 0.0 {
                let fuel_needed =
                    (laps_remaining_in_race * gfx.fuel_x_lap) + self.config.fuel_safety_margin;
                let fuel_diff = phys.fuel - fuel_needed;

                if fuel_diff < -1.0 {
                    recs.push(Recommendation {
                        component: if ru {
                            "Стратегия".to_string()
                        } else {
                            "Strategy".to_string()
                        },
                        category: if ru {
                            "Финиш".to_string()
                        } else {
                            "Race Finish".to_string()
                        },
                        severity: Severity::Warning,
                        message: if ru {
                            format!("Не хватит {:.1} л.", fuel_diff.abs())
                        } else {
                            format!("Short {:.1} L", fuel_diff.abs())
                        },
                        action: if ru {
                            "Экономить / Пит-стоп".to_string()
                        } else {
                            "Save Fuel / Box".to_string()
                        },
                        parameters: vec![Parameter {
                            name: "Need".to_string(),
                            current: phys.fuel,
                            target: fuel_needed,
                            unit: "L".to_string(),
                        }],
                        confidence: 0.8,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Engineer;
    use crate::ac_structs::{AcGraphics, AcPhysics};
    use crate::config::AppConfig;

    #[test]
    fn tyre_pressure_alert_uses_updated_configuration() {
        let mut config = AppConfig::default();
        config.alerts.tyre_pressure_max = 31.0;
        let mut engineer = Engineer::new(&config);
        let physics = AcPhysics {
            speed_kmh: 60.0,
            wheels_pressure: [30.0; 4],
            ..Default::default()
        };
        let graphics = AcGraphics::default();

        let recommendations = engineer.analyze_live(&physics, &graphics, None);
        assert!(!recommendations.iter().any(|rec| rec.category == "Pressure"));

        config.alerts.tyre_pressure_max = 29.0;
        engineer.update_config(&config);

        let past = std::time::Instant::now() - std::time::Duration::from_secs(2);
        for i in 0..4 {
            engineer
                .alert_timers
                .insert(format!("pres_{}", i), (past, past));
        }

        let recommendations = engineer.analyze_live(&physics, &graphics, None);
        assert!(recommendations.iter().any(|rec| rec.category == "Pressure"));
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TyreCornerAdjustment {
    pub corner_name: String,
    pub current_psi: f32,
    pub recommended_delta_psi: f32,
    pub temp_spread_c: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TyrePressureOptimizer {
    pub corners: [TyreCornerAdjustment; 4],
}

impl TyrePressureOptimizer {
    pub fn calculate(phys: &AcPhysics, target_psi: f32) -> Self {
        let labels = ["FL", "FR", "RL", "RR"];
        let mut corners = Vec::with_capacity(4);

        for i in 0..4 {
            let p_psi = phys.wheels_pressure[i];
            let t_i = phys.tyre_temp_i[i];
            let t_o = phys.tyre_temp_o[i];
            let spread = t_i - t_o;
            let p_delta = target_psi - p_psi;

            let rec_delta = if p_delta.abs() > 0.5 {
                (p_delta * 10.0).round() / 10.0
            } else if spread > 12.0 {
                -0.3
            } else if spread < -8.0 {
                0.3
            } else {
                0.0
            };

            corners.push(TyreCornerAdjustment {
                corner_name: labels[i].to_string(),
                current_psi: p_psi,
                recommended_delta_psi: rec_delta,
                temp_spread_c: spread,
            });
        }

        Self {
            corners: [
                corners[0].clone(),
                corners[1].clone(),
                corners[2].clone(),
                corners[3].clone(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdPressureEstimate {
    pub target_hot_psi: f32,
    pub recommended_cold_psi: f32,
    pub delta_temp_psi: f32,
    pub delta_grip_psi: f32,
}

pub struct ColdPressureCalculator;

impl ColdPressureCalculator {
    pub fn calculate(
        target_hot_psi: f32,
        ambient_temp_c: f32,
        track_grip: f32,
    ) -> ColdPressureEstimate {
        let temp_diff = (85.0 - ambient_temp_c.clamp(0.0, 50.0)).max(0.0);
        let delta_temp_psi = temp_diff * 0.08;
        let grip_norm = track_grip.clamp(0.80, 1.0);
        let delta_grip_psi = (1.0 - grip_norm) * 1.2;

        let recommended_cold_psi = (target_hot_psi - delta_temp_psi - delta_grip_psi).max(15.0);

        ColdPressureEstimate {
            target_hot_psi,
            recommended_cold_psi: (recommended_cold_psi * 10.0).round() / 10.0,
            delta_temp_psi: (delta_temp_psi * 10.0).round() / 10.0,
            delta_grip_psi: (delta_grip_psi * 10.0).round() / 10.0,
        }
    }
}
