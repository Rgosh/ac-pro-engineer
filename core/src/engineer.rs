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

/// The four corners, in the order every array in AC's physics page uses.
///
/// Was written out as a `match i { 0 => "FL", ... }` in six analysers, which
/// is six places to get the order wrong.
pub const CORNER_NAMES: [&str; 4] = ["FL", "FR", "RL", "RR"];

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

/// How many recent laps the measured fuel average runs over. Short enough to
/// track a changing pace, long enough that one cautious lap does not skew it.
const FUEL_HISTORY_LAPS: usize = 3;

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
    /// Fuel used on each of the last few completed laps, newest last.
    ///
    /// AC's own `fuel_x_lap` sits in the part of the graphics page that is
    /// not confirmed against a live capture (see the note in ac_structs.rs),
    /// and it reads zero on lap one regardless. Everything on the strategy
    /// tab was gated on it being positive, so the tab showed "NO DATA" for
    /// the whole first lap and, if that offset is wrong, forever.
    pub recent_fuel_per_lap: Vec<f32>,
    /// Fuel level at the start of the current lap, to measure against.
    pub fuel_at_lap_start: f32,
    pub current_delta: f32,
    pub predicted_lap_time: f32,
    pub low_speed_rake: f32,
    pub high_speed_rake: f32,
    /// Inner-minus-outer surface temperature per corner, averaged over the
    /// frames where the tyre was actually loaded sideways.
    ///
    /// A tyre says nothing about camber on a straight. Upright, both edges run
    /// at the same temperature and the spread reads about zero — which the
    /// first version of `analyze_camber` read as "contact patch inefficient"
    /// and published on every straight, four corners at a time, filling the
    /// panel's eight advice lines with one wrong answer. Camber only shows in
    /// the spread while the tyre is loaded, so the sample is taken while the
    /// car is cornering and the verdict comes from the average.
    ///
    /// All four corners are sampled rather than the loaded pair: left-handers
    /// load the right tyres and right-handers the left, so over a lap of a
    /// circuit both sides get their turn. An oval would skew it.
    pub camber_spread: [f32; 4],
    /// Ticks of cornering behind `camber_spread`. Below `CAMBER_MIN_FRAMES`
    /// the average is one corner's worth of noise and nothing is published.
    pub camber_frames: u32,
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
            recent_fuel_per_lap: Vec::new(),
            fuel_at_lap_start: 0.0,
            current_delta: 0.0,
            predicted_lap_time: 0.0,
            low_speed_rake: 0.0,
            high_speed_rake: 0.0,
            camber_spread: [0.0; 4],
            camber_frames: 0,
            base_tyre_wear: [100.0; 4],
            stint_laps: 0,
            last_lap_count: -1,
            // Negative means "not measured yet", and it has to be a value
            // laps remaining can never take — this was 99.0, which is also a
            // perfectly good answer, so a tyre with ninety-nine laps in it read
            // as no data.
            tyre_laps_remaining: [-1.0; 4],
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

        // acc_g is [lateral, vertical, longitudinal]. Half a g sideways is a
        // corner being driven; a lane change on a straight does not reach it,
        // and a straight is where the inner and outer edges of a correctly
        // cambered tyre read the same temperature.
        if phys.speed_kmh > 50.0 && phys.acc_g[0].abs() > 0.5 {
            let first_sample = self.stats.camber_frames == 0;
            self.stats.camber_frames = self.stats.camber_frames.saturating_add(ticks_norm);
            for i in 0..4 {
                let spread = phys.tyre_temp_i[i] - phys.tyre_temp_o[i];
                if first_sample {
                    self.stats.camber_spread[i] = spread;
                } else {
                    self.stats.camber_spread[i] =
                        self.stats.camber_spread[i] * 0.98 + spread * 0.02;
                }
            }
        }

        let current_laps = gfx.completed_laps;
        if current_laps != self.stats.last_lap_count {
            self.record_fuel_for_completed_lap(phys.fuel);
            if self.stats.last_lap_count == -1 || current_laps == 0 || phys.speed_kmh < 10.0 {
                self.stats.base_tyre_wear = phys.tyre_wear;
                self.stats.stint_laps = 0;
            } else {
                self.stats.stint_laps += 1;
                for i in 0..4 {
                    let wear_used = self.stats.base_tyre_wear[i] - phys.tyre_wear[i];
                    if wear_used > 0.0 && self.stats.stint_laps > 0 {
                        let wear_per_lap = wear_used / self.stats.stint_laps as f32;
                        // Laps until the tyre is *done*, not until it is two
                        // percent past the warning. That derivation is the same
                        // one that used to call a tyre at 93.9 % life critical.
                        let replacement_threshold = self
                            .config
                            .alerts
                            .wear_critical
                            .min(self.config.alerts.wear_warning)
                            .max(0.0);
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

        // Prefer AC's own figure when it is reporting one, and fall back to
        // what we measured ourselves otherwise. Either way the strategy tab
        // has something to work with from the second lap onward.
        let fuel_per_lap = if gfx.fuel_x_lap > 0.0 {
            Some(gfx.fuel_x_lap)
        } else {
            self.measured_fuel_per_lap()
        };
        match fuel_per_lap {
            Some(per_lap) if per_lap > 0.0 => {
                self.stats.fuel_consumption_rate = per_lap;
                self.stats.fuel_laps_remaining = phys.fuel / per_lap;
            }
            // Nothing to go on. Clear rather than leave the previous value
            // standing: it was never reset, so after a refuel or a session
            // change `analyze_strategy` could call BOX BOX BOX on a number
            // measured before the stop.
            _ => {
                self.stats.fuel_consumption_rate = 0.0;
                self.stats.fuel_laps_remaining = 0.0;
            }
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

        // acc_g is [lateral, vertical, longitudinal]. This combined the
        // lateral and *vertical* axes, so it measured cornering plus the ~1 g
        // the car carries standing still, and never saw braking or
        // acceleration at all.
        let combined_g = (phys.acc_g[0].powi(2) + phys.acc_g[2].powi(2)).sqrt();
        self.driving_style.aggression =
            0.9 * self.driving_style.aggression + 0.1 * combined_g.min(2.5) / 2.5 * 100.0;

        if phys.brake > 0.1 && phys.steer_angle.abs() > 0.1 {
            self.driving_style.trail_braking =
                0.95 * self.driving_style.trail_braking + 0.05 * 100.0;
        } else {
            self.driving_style.trail_braking *= 0.98;
        }
    }

    /// Average fuel burn over the laps measured so far, if there are any.
    fn measured_fuel_per_lap(&self) -> Option<f32> {
        if self.stats.recent_fuel_per_lap.is_empty() {
            return None;
        }
        let sum: f32 = self.stats.recent_fuel_per_lap.iter().sum();
        Some(sum / self.stats.recent_fuel_per_lap.len() as f32)
    }

    /// Note how much fuel the lap that just ended consumed.
    ///
    /// A negative delta means the tank went up, i.e. a pit stop: the history
    /// is dropped rather than averaged, because burn measured across a refuel
    /// is meaningless.
    fn record_fuel_for_completed_lap(&mut self, fuel_now: f32) {
        let used = self.stats.fuel_at_lap_start - fuel_now;
        self.stats.fuel_at_lap_start = fuel_now;

        if used < 0.0 {
            self.stats.recent_fuel_per_lap.clear();
            return;
        }
        if used <= 0.0 || !used.is_finite() {
            return;
        }

        self.stats.recent_fuel_per_lap.push(used);
        if self.stats.recent_fuel_per_lap.len() > FUEL_HISTORY_LAPS {
            self.stats.recent_fuel_per_lap.remove(0);
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

    /// Forget everything measured about this stint. Called when the session
    /// changes underneath us.
    pub fn reset_fuel_tracking(&mut self) {
        self.stats.recent_fuel_per_lap.clear();
        self.stats.fuel_at_lap_start = 0.0;
        self.stats.fuel_laps_remaining = 0.0;
        self.stats.fuel_consumption_rate = 0.0;
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

        self.analyze_camber(phys, &mut recommendations);
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

    /// Name a set of corners the way an engineer would say it out loud.
    ///
    /// Four separate lines saying the same thing about four wheels is four of
    /// the overlay's slots spent on one fact, and the driver reads "FL COLD /
    /// FR COLD / RL COLD / RR COLD" as noise rather than as "the tyres are not
    /// up to temperature yet". Which is what it means.
    fn corner_phrase(corners: &[usize], ru: bool) -> String {
        match corners {
            [] => String::new(),
            [only] => CORNER_NAMES[*only].to_string(),
            [0, 1] => if ru { "Перед" } else { "Fronts" }.to_string(),
            [2, 3] => if ru { "Зад" } else { "Rears" }.to_string(),
            [0, 2] => if ru { "Левые" } else { "Left side" }.to_string(),
            [1, 3] => if ru { "Правые" } else { "Right side" }.to_string(),
            [0, 1, 2, 3] => if ru { "Все шины" } else { "All four" }.to_string(),
            many => many
                .iter()
                .map(|index| CORNER_NAMES[*index])
                .collect::<Vec<_>>()
                .join("/"),
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

        let mut low: Vec<usize> = Vec::new();
        let mut high: Vec<usize> = Vec::new();

        for i in 0..4 {
            let pressure = phys.wheels_pressure[i];
            let is_error = pressure < pressure_min || pressure > pressure_max;

            let key = format!("pres_{}", i);
            if !self.check_hysteresis(&key, is_error) || phys.speed_kmh <= 10.0 || !is_error {
                continue;
            }

            if pressure < optimal_pressure {
                low.push(i);
            } else {
                high.push(i);
            }
        }

        // Units, at last. The temperature analyser has gone through the
        // formatter since it was written; this one printed raw psi, so anyone
        // working in bar read their pressures in one unit on the Dashboard and
        // another in the advice about them.
        let formatter = self.config.formatter();

        let mut push = |corners: &[usize], inflate: bool| {
            if corners.is_empty() {
                return;
            }
            let average = corners
                .iter()
                .map(|i| phys.wheels_pressure[*i])
                .sum::<f32>()
                / corners.len() as f32;
            let difference = (average - optimal_pressure).abs();

            recs.push(Recommendation {
                component: if ru {
                    format!("Шины ({})", class_name)
                } else {
                    format!("Tyres ({})", class_name)
                },
                category: if ru { "Давление" } else { "Pressure" }.to_string(),
                // A pressure a full unit off the target changes how the car
                // turns; half of one is a setup working as intended.
                severity: if difference > 2.5 {
                    Severity::Warning
                } else {
                    Severity::Info
                },
                message: format!(
                    "{} {}: {} ({} {})",
                    Self::corner_phrase(corners, ru),
                    if ru { "давление" } else { "pressure" },
                    formatter.format_pressure(average),
                    if ru { "цель" } else { "target" },
                    formatter.format_pressure(optimal_pressure)
                ),
                action: if inflate {
                    if ru { "Накачать" } else { "Inflate" }
                } else if ru {
                    "Спустить"
                } else {
                    "Deflate"
                }
                .to_string(),
                parameters: corners
                    .iter()
                    .map(|i| Parameter {
                        name: CORNER_NAMES[*i].to_string(),
                        current: phys.wheels_pressure[*i],
                        target: optimal_pressure,
                        unit: formatter.pressure_symbol().to_string(),
                    })
                    .collect(),
                confidence: 0.9,
            });
        };

        push(&low, true);
        push(&high, false);
    }

    fn analyze_tyre_wear(&mut self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();

        // AC counts wear down from 100, so all four corners reading zero is not
        // four destroyed tyres — it is a session that has not published wear
        // yet. Without this the panel opens with four CRITICAL lines telling a
        // driver who just left the pits that every tyre is gone, which is the
        // kind of thing that gets an engineer ignored for the rest of the race.
        if phys.tyre_wear.iter().all(|wear| *wear <= 0.0) {
            return;
        }

        let warning_threshold = self.config.alerts.wear_warning;
        // Its own threshold, not `warning - 2`. That derivation made a tyre at
        // 93.9 % life a CRITICAL "WORN OUT" with the default settings, which is
        // a tyre most of the way through its first stint.
        let critical_threshold = self
            .config
            .alerts
            .wear_critical
            .min(self.config.alerts.wear_warning);

        let mut worn: Vec<usize> = Vec::new();
        let mut critical: Vec<usize> = Vec::new();

        for (i, wear) in phys.tyre_wear.iter().copied().enumerate() {
            let is_worn = wear < warning_threshold;
            // The hysteresis is still per corner: it is per-corner state, and
            // one wheel picking up a flat spot should not reset the timers on
            // the other three. Only the reporting is grouped.
            if !self.check_hysteresis(&format!("wear_{}", i), is_worn) || !is_worn {
                continue;
            }
            if wear < critical_threshold {
                critical.push(i);
            } else {
                worn.push(i);
            }
        }

        let mut push = |corners: &[usize], severity: Severity| {
            if corners.is_empty() {
                return;
            }
            let lowest = corners
                .iter()
                .map(|i| phys.tyre_wear[*i])
                .fold(f32::MAX, f32::min);
            let where_ = Self::corner_phrase(corners, ru);
            let what = match (&severity, ru) {
                (Severity::Critical, true) => "ИЗНОС (Крит)",
                (Severity::Critical, false) => "WORN OUT",
                (_, true) => "сильный износ",
                (_, false) => "high wear",
            };

            recs.push(Recommendation {
                component: if ru { "Шины" } else { "Tyres" }.to_string(),
                category: if ru { "Износ" } else { "Wear" }.to_string(),
                severity,
                message: format!("{where_} {what}: {lowest:.1}%"),
                action: if ru {
                    "Пит-стоп / Осторожно"
                } else {
                    "Box / Careful"
                }
                .to_string(),
                parameters: corners
                    .iter()
                    .map(|i| Parameter {
                        name: format!("{} life", CORNER_NAMES[*i]),
                        current: phys.tyre_wear[*i],
                        target: 100.0,
                        unit: "%".to_string(),
                    })
                    .collect(),
                confidence: 0.9,
            });
        };

        push(&critical, Severity::Critical);
        push(&worn, Severity::Warning);
    }

    /// The camber a wheel is running, in degrees, in the car's frame rather
    /// than the wheel's own.
    ///
    /// AC publishes `camberRAD` per wheel, and the two sides mirror each other:
    /// an abarth500 sitting in the pits with the same setting on both front
    /// wheels reads -0.023 rad on the left and +0.021 on the right. Negating
    /// the right-hand corners puts all four on one scale, where negative means
    /// the top of the tyre leans in — which is what a driver means by camber.
    fn camber_degrees(phys: &AcPhysics, corner: usize) -> f32 {
        let sign = if corner == 1 || corner == 3 {
            -1.0
        } else {
            1.0
        };
        phys.camber_rad[corner].to_degrees() * sign
    }

    /// Cornering ticks needed before the camber average is worth publishing.
    /// 60 ticks is a second of load, which is one long corner or two short
    /// ones — enough that the average is not a single frame of noise.
    const CAMBER_MIN_FRAMES: u32 = 60;

    /// What the inner and outer edges of a tyre say about its camber.
    ///
    /// Judged on `stats.camber_spread`, which is only sampled while the car is
    /// cornering — see the field. The instantaneous spread this used to read
    /// is near zero on every straight, so it published four Info lines about
    /// nothing every time the car left a corner.
    fn analyze_camber(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        if self.stats.camber_frames < Self::CAMBER_MIN_FRAMES {
            return;
        }
        let fmt = self.config.formatter();
        let ideal_spread = 8.0;

        let mut too_little: Vec<usize> = Vec::new();
        let mut too_much: Vec<usize> = Vec::new();
        for i in 0..4 {
            let spread = self.stats.camber_spread[i];
            if spread < 2.0 {
                too_little.push(i);
            } else if spread > 15.0 {
                too_much.push(i);
            }
        }

        // Four corners of one problem are one problem — the same grouping the
        // wear and pressure advice does, and for the same reason: the panel
        // shows eight lines, and four of them saying "FL", "FR", "RL", "RR"
        // about one setting crowds out everything else the engineer noticed.
        let mut push = |corners: &[usize], more_camber: bool| {
            if corners.is_empty() {
                return;
            }
            let where_ = Self::corner_phrase(corners, ru);
            // The worst corner of the group is the one worth naming: the
            // furthest from the window for too little camber, the hottest
            // inner edge for too much.
            let spread = corners
                .iter()
                .map(|i| self.stats.camber_spread[*i])
                .fold(f32::NAN, if more_camber { f32::min } else { f32::max });
            // AC publishes the camber it is running, so the advice can say what
            // to change *from*. The setup file cannot: `CAMBER_LF VALUE=-9` is
            // a step index into a range that lives inside the car's `data.acd`,
            // and printing it read as "now: -9" beside a car showing -1.3°.
            let now = corners
                .iter()
                .map(|i| Self::camber_degrees(phys, *i))
                .sum::<f32>()
                / corners.len() as f32;
            let now_clause = if now.abs() > 0.05 {
                format!(" ({}: {now:.1}°)", if ru { "сейчас" } else { "now" })
            } else {
                String::new()
            };

            recs.push(Recommendation {
                component: if ru { "Подвеска" } else { "Suspension" }.to_string(),
                category: if ru { "Развал" } else { "Camber" }.to_string(),
                severity: if more_camber {
                    Severity::Info
                } else {
                    Severity::Warning
                },
                // The message hardcoded "C" while the parameter beside it
                // was labelled with the configured symbol, so with
                // Fahrenheit selected the user saw a Celsius number
                // labelled °F. A spread is a difference, so it converts
                // by scale only -- `format_temp` would add 32.
                message: format!(
                    "{where_} {} (I-O: {})",
                    match (more_camber, ru) {
                        (true, true) => "пятно контакта не эффективно",
                        (true, false) => "contact patch inefficient",
                        (false, true) => "перегрев внутренней части",
                        (false, false) => "inner edge overheating",
                    },
                    fmt.format_temp_delta(spread)
                ),
                action: match (more_camber, ru) {
                    (true, true) => {
                        format!("Больше отриц. развала{now_clause}. Если предел -> смягчите ARB")
                    }
                    (true, false) => {
                        format!("More neg. camber{now_clause}. If maxed -> soften ARB")
                    }
                    (false, true) => {
                        format!("Меньше отриц. развала{now_clause}. Если предел -> зажмите ARB")
                    }
                    (false, false) => {
                        format!("Less neg. camber{now_clause}. If maxed -> stiffen ARB")
                    }
                },
                parameters: corners
                    .iter()
                    .map(|i| Parameter {
                        name: format!("{} I-O", CORNER_NAMES[*i]),
                        current: fmt.temp_delta_val(self.stats.camber_spread[*i]),
                        target: fmt.temp_delta_val(ideal_spread),
                        unit: fmt.temp_symbol().to_string(),
                    })
                    .collect(),
                confidence: if more_camber { 0.7 } else { 0.8 },
            });
        };

        push(&too_much, false);
        push(&too_little, true);
    }

    fn analyze_tyre_temperature(&mut self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
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

        if phys.speed_kmh <= 100.0 {
            return;
        }

        let mut cold: Vec<usize> = Vec::new();
        let mut hot: Vec<usize> = Vec::new();

        for i in 0..4 {
            let temp = phys.get_avg_tyre_temp(i);
            let out_of_band = temp < min_temp || temp > max_temp;
            // Same gate as the pressure and wear alerts. This ran on every
            // frame, so a tyre that stayed cold produced a fresh recommendation
            // dozens of times a second.
            if !self.check_hysteresis(&format!("tyre_temp_{}", i), out_of_band) {
                continue;
            }
            if temp < min_temp {
                cold.push(i);
            } else if temp > max_temp {
                hot.push(i);
            }
        }

        let formatter = self.config.formatter();

        if !cold.is_empty() {
            let average =
                cold.iter().map(|i| phys.get_avg_tyre_temp(*i)).sum::<f32>() / cold.len() as f32;
            recs.push(Recommendation {
                component: if ru { "Шины" } else { "Tyres" }.to_string(),
                category: if ru {
                    "Температура"
                } else {
                    "Temperature"
                }
                .to_string(),
                severity: Severity::Warning,
                message: format!(
                    "{} {}: {}",
                    Self::corner_phrase(&cold, ru),
                    if ru { "ХОЛОДНЫЕ" } else { "COLD" },
                    formatter.format_temp(average)
                ),
                action: if ru {
                    "Греть шины"
                } else {
                    "Warm tyres"
                }
                .to_string(),
                parameters: vec![],
                confidence: 0.95,
            });
        }

        if !hot.is_empty() {
            let average =
                hot.iter().map(|i| phys.get_avg_tyre_temp(*i)).sum::<f32>() / hot.len() as f32;
            recs.push(Recommendation {
                component: if ru { "Шины" } else { "Tyres" }.to_string(),
                category: if ru { "Перегрев" } else { "Overheat" }.to_string(),
                severity: Severity::Critical,
                message: format!(
                    "{} {}: {}",
                    Self::corner_phrase(&hot, ru),
                    if ru {
                        "ПЕРЕГРЕВ"
                    } else {
                        "OVERHEATING"
                    },
                    formatter.format_temp(average)
                ),
                action: if ru {
                    "Остудить шины"
                } else {
                    "Cool tyres"
                }
                .to_string(),
                parameters: vec![],
                confidence: 0.95,
            });
        }
    }

    fn analyze_brakes(&mut self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let max_temp = self.config.alerts.brake_temp_max;
        let ru = self.is_ru();

        let mut cooking: Vec<usize> = Vec::new();
        for i in 0..4 {
            // Gated the way the pressure and wear alerts already are. Without
            // it this pushed a fresh recommendation on every single frame the
            // brake was over temperature — dozens a second, burying every
            // other message in the list.
            let too_hot = phys.brake_temp[i] > max_temp;
            if self.check_hysteresis(&format!("brake_temp_{}", i), too_hot) && too_hot {
                cooking.push(i);
            }
        }

        if cooking.is_empty() {
            return;
        }

        // Both front brakes overheating is one thing to say, not two. The
        // corner names are FL/FR/RL/RR the way every neighbouring alert says
        // them — this used to number them "Brake 1" to "Brake 4", the only
        // place in the application that did.
        let hottest = cooking
            .iter()
            .map(|i| phys.brake_temp[*i])
            .fold(0.0_f32, f32::max);
        let formatter = self.config.formatter();

        recs.push(Recommendation {
            component: if ru { "Тормоза" } else { "Brakes" }.to_string(),
            category: if ru { "Перегрев" } else { "Overheat" }.to_string(),
            severity: Severity::Critical,
            message: format!(
                "{} {}: {}",
                Self::corner_phrase(&cooking, ru),
                if ru {
                    "перегрев тормозов"
                } else {
                    "brakes cooking"
                },
                formatter.format_temp(hottest)
            ),
            action: if ru {
                "Сместить баланс / Охладить"
            } else {
                "Move bias / Cool down"
            }
            .to_string(),
            parameters: vec![],
            confidence: 1.0,
        });
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
            // Whole laps, not the display fraction: a timed race runs until
            // the leader completes the lap the clock ran out on, and the lap
            // already in progress still has to be finished.
            let laps_remaining_in_race = crate::session_info::SessionTiming::laps_to_fuel_for(
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

    /// The simulator publishes zero wear until a lap is done, and so does a
    /// real session before its first update. Neither means the tyres are gone.
    #[test]
    fn wear_of_zero_on_every_corner_is_no_data_rather_than_four_dead_tyres() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);

        let mut phys = AcPhysics {
            tyre_wear: [0.0; 4],
            ..Default::default()
        };
        let mut recs = Vec::new();
        engineer.analyze_tyre_wear(&phys, &mut recs);
        assert!(recs.is_empty(), "no wear data means no wear advice");

        // One corner reporting something plausible is data, and a corner at 40%
        // in that state is worth saying out loud. Alerts are held back for a
        // second before they are reported, so the timer is aged by hand here
        // rather than by sleeping through it.
        phys.tyre_wear = [98.0, 97.0, 40.0, 96.0];
        let aged = std::time::Instant::now() - std::time::Duration::from_secs(2);
        engineer
            .alert_timers
            .insert("wear_2".to_string(), (aged, std::time::Instant::now()));

        let mut recs = Vec::new();
        engineer.analyze_tyre_wear(&phys, &mut recs);
        assert!(
            recs.iter().any(|rec| rec.message.contains("RL")),
            "the worn corner is reported once there is data: {recs:?}"
        );
    }
    use super::{Engineer, Severity};
    use crate::ac_structs::{AcGraphics, AcPhysics};
    use crate::config::{AppConfig, PressureUnit};

    /// Age every alert timer past the one-second hold, so a test does not have
    /// to sleep through it.
    fn age_the_alerts(engineer: &mut Engineer) {
        let aged = std::time::Instant::now() - std::time::Duration::from_secs(2);
        let now = std::time::Instant::now();
        for key in [
            "wear_0",
            "wear_1",
            "wear_2",
            "wear_3",
            "tyre_temp_0",
            "tyre_temp_1",
            "tyre_temp_2",
            "tyre_temp_3",
            "pres_0",
            "pres_1",
            "pres_2",
            "pres_3",
            "brake_temp_0",
            "brake_temp_1",
            "brake_temp_2",
            "brake_temp_3",
        ] {
            engineer.alert_timers.insert(key.to_string(), (aged, now));
        }
    }

    /// Drive `phys` through `update` often enough for the camber average to
    /// have something behind it.
    fn drive(engineer: &mut Engineer, phys: &AcPhysics, ticks: u32) {
        let gfx = AcGraphics::default();
        let session = crate::session_info::SessionInfo::default();
        for _ in 0..ticks {
            engineer.update(phys, &gfx, &session);
        }
    }

    /// A tyre says nothing about camber while the car is upright: both edges
    /// run the same temperature, the spread reads zero, and the old check read
    /// that as "contact patch inefficient" — four Info lines, on every
    /// straight, in the eight the panel has.
    #[test]
    fn a_straight_is_not_a_camber_problem() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);

        let phys = AcPhysics {
            speed_kmh: 240.0,
            acc_g: [0.0, 1.0, 0.0],
            tyre_temp_i: [90.0; 4],
            tyre_temp_o: [90.0; 4],
            ..Default::default()
        };
        drive(&mut engineer, &phys, 600);

        let mut recs = Vec::new();
        engineer.analyze_camber(&phys, &mut recs);
        assert!(recs.is_empty(), "ten seconds of straight line: {recs:?}");
    }

    /// The same spread, measured while the tyre is loaded, is a real answer —
    /// and it is one line for all four corners rather than four.
    #[test]
    fn four_corners_wanting_camber_are_one_piece_of_advice() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);

        let phys = AcPhysics {
            speed_kmh: 160.0,
            acc_g: [1.2, 1.0, 0.0],
            // Outer edge hotter than inner: not enough negative camber.
            tyre_temp_i: [90.0; 4],
            tyre_temp_o: [94.0; 4],
            camber_rad: [
                (-1.5f32).to_radians(),
                1.5f32.to_radians(),
                (-1.5f32).to_radians(),
                1.5f32.to_radians(),
            ],
            ..Default::default()
        };
        drive(&mut engineer, &phys, 120);

        let mut recs = Vec::new();
        engineer.analyze_camber(&phys, &mut recs);

        assert_eq!(recs.len(), 1, "one fact, one line: {recs:?}");
        assert!(
            recs[0].message.contains("All four"),
            "and it says which corners: {}",
            recs[0].message
        );
        // The number in the advice is the camber AC reports, not the setup
        // file's step index — `CAMBER_LF VALUE=-9` used to be printed as
        // "now: -9" beside a car the game showed at -1.3°.
        assert!(
            recs[0].action.contains("-1.5°"),
            "the advice names the camber the car is running: {}",
            recs[0].action
        );
    }

    /// Inner edges cooking is the other direction, and it is a warning rather
    /// than a note.
    #[test]
    fn inner_edges_cooking_ask_for_less_camber() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);

        let phys = AcPhysics {
            speed_kmh: 160.0,
            acc_g: [-1.2, 1.0, 0.0],
            tyre_temp_i: [110.0, 110.0, 90.0, 90.0],
            tyre_temp_o: [90.0, 90.0, 82.0, 82.0],
            ..Default::default()
        };
        drive(&mut engineer, &phys, 120);

        let mut recs = Vec::new();
        engineer.analyze_camber(&phys, &mut recs);

        assert_eq!(recs.len(), 1, "{recs:?}");
        assert!(
            recs[0].message.contains("Fronts"),
            "only the fronts are cooking: {}",
            recs[0].message
        );
        assert_eq!(recs[0].severity, Severity::Warning);
        assert!(
            recs[0].action.contains("Less neg. camber"),
            "{}",
            recs[0].action
        );
    }

    /// AC reports camber in each wheel's own frame, so the two sides mirror.
    /// Reading them raw makes a symmetric car look like it has +1.5° on the
    /// right, which is a car about to spin rather than a normal setup.
    #[test]
    fn the_two_sides_of_the_car_report_camber_on_one_scale() {
        let phys = AcPhysics {
            camber_rad: [
                (-1.3f32).to_radians(),
                1.3f32.to_radians(),
                (-2.0f32).to_radians(),
                2.0f32.to_radians(),
            ],
            ..Default::default()
        };
        for corner in 0..4 {
            assert!(
                Engineer::camber_degrees(&phys, corner) < 0.0,
                "corner {corner} reads positive on a car with negative camber"
            );
        }
        assert!((Engineer::camber_degrees(&phys, 0) + 1.3).abs() < 0.01);
        assert!((Engineer::camber_degrees(&phys, 1) + 1.3).abs() < 0.01);
    }

    /// Four corners of one problem used to be four recommendations, which is
    /// every slot the overlay has spent saying one thing — and "FL COLD / FR
    /// COLD / RL COLD / RR COLD" reads as noise rather than as "the tyres are
    /// not up to temperature".
    #[test]
    fn four_cold_tyres_are_one_piece_of_advice() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        age_the_alerts(&mut engineer);

        let phys = AcPhysics {
            speed_kmh: 180.0,
            tyre_temp_i: [55.0; 4],
            tyre_temp_m: [55.0; 4],
            tyre_temp_o: [55.0; 4],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_temperature(&phys, &mut recs);

        assert_eq!(recs.len(), 1, "one fact, one line: {recs:?}");
        assert!(
            recs[0].message.contains("All four"),
            "and it says which corners: {}",
            recs[0].message
        );
    }

    /// Two of four is still one line, and it names the axle rather than
    /// listing wheels.
    #[test]
    fn two_hot_fronts_are_named_as_an_axle() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        age_the_alerts(&mut engineer);

        let phys = AcPhysics {
            speed_kmh: 180.0,
            tyre_temp_i: [130.0, 130.0, 90.0, 90.0],
            tyre_temp_m: [130.0, 130.0, 90.0, 90.0],
            tyre_temp_o: [130.0, 130.0, 90.0, 90.0],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_temperature(&phys, &mut recs);

        assert_eq!(recs.len(), 1);
        assert!(
            recs[0].message.contains("Fronts"),
            "expected an axle, got: {}",
            recs[0].message
        );
    }

    /// Cold at the front and hot at the back is two different problems, and
    /// grouping must not merge them.
    #[test]
    fn cold_and_hot_stay_separate_problems() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        age_the_alerts(&mut engineer);

        let phys = AcPhysics {
            speed_kmh: 180.0,
            tyre_temp_i: [50.0, 50.0, 130.0, 130.0],
            tyre_temp_m: [50.0, 50.0, 130.0, 130.0],
            tyre_temp_o: [50.0, 50.0, 130.0, 130.0],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_temperature(&phys, &mut recs);

        assert_eq!(recs.len(), 2, "{recs:?}");
        assert!(recs.iter().any(|rec| rec.message.contains("Fronts")));
        assert!(recs.iter().any(|rec| rec.message.contains("Rears")));
    }

    /// The threshold that made the engineer cry wolf. `wear_warning - 2` meant
    /// a tyre at 93.9 % life — most of the way through a first stint — came
    /// back as CRITICAL "WORN OUT".
    #[test]
    fn a_tyre_most_of_the_way_through_a_stint_is_not_critical() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        age_the_alerts(&mut engineer);

        let phys = AcPhysics {
            tyre_wear: [93.0; 4],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_wear(&phys, &mut recs);

        assert_eq!(recs.len(), 1, "{recs:?}");
        assert_eq!(
            recs[0].severity,
            Severity::Warning,
            "93% life is a warning, not a critical: {}",
            recs[0].message
        );

        // And below the critical threshold it still is one.
        let phys = AcPhysics {
            tyre_wear: [70.0; 4],
            ..Default::default()
        };
        let mut recs = Vec::new();
        engineer.analyze_tyre_wear(&phys, &mut recs);
        assert_eq!(recs[0].severity, Severity::Critical, "{recs:?}");
    }

    /// "Laps remaining" starts as "not measured", and 99.0 was a bad way to
    /// say it — ninety-nine laps is also a perfectly good answer, so a fresh
    /// set on a short track read as having no data at all.
    #[test]
    fn tyre_life_says_not_measured_with_a_value_laps_cannot_take() {
        let engineer = Engineer::new(&AppConfig::default());
        for corner in 0..4 {
            assert!(
                engineer.stats.tyre_laps_remaining[corner] < 0.0,
                "corner {corner} should start unmeasured"
            );
        }
    }

    /// The pressure advice printed raw psi while the temperature advice next
    /// to it went through the formatter, so anyone working in bar read their
    /// pressures in one unit on the Dashboard and another in the advice about
    /// them.
    #[test]
    fn pressure_advice_is_in_the_unit_the_driver_chose() {
        let config = AppConfig {
            pressure_unit: PressureUnit::Bar,
            ..AppConfig::default()
        };
        let mut engineer = Engineer::new(&config);
        age_the_alerts(&mut engineer);

        let phys = AcPhysics {
            speed_kmh: 120.0,
            wheels_pressure: [20.0; 4],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_pressure(&phys, &AcGraphics::default(), &mut recs);

        assert_eq!(
            recs.len(),
            1,
            "four under-inflated tyres are one line: {recs:?}"
        );
        assert!(
            recs[0].message.contains("bar"),
            "expected bar, got: {}",
            recs[0].message
        );
        assert!(
            !recs[0].message.contains("psi"),
            "and not psi: {}",
            recs[0].message
        );
    }

    /// Both front brakes over temperature is one thing to say. It also has to
    /// name the wheels the way every other alert does — this used to number
    /// them "Brake 1" to "Brake 4".
    #[test]
    fn cooking_brakes_are_grouped_and_named_by_corner() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        age_the_alerts(&mut engineer);

        let phys = AcPhysics {
            brake_temp: [950.0, 950.0, 300.0, 300.0],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_brakes(&phys, &mut recs);

        assert_eq!(recs.len(), 1, "{recs:?}");
        assert!(recs[0].message.contains("Fronts"), "{}", recs[0].message);
        assert!(
            !recs[0].message.contains("Brake 1"),
            "corners are named, not numbered: {}",
            recs[0].message
        );
    }

    /// What all of this is for: the overlay carries eight lines, and a session
    /// with several things wrong has to fit more than one of them in.
    #[test]
    fn several_problems_at_once_still_leave_room_for_each_other() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        age_the_alerts(&mut engineer);

        let phys = AcPhysics {
            speed_kmh: 180.0,
            wheels_pressure: [20.0; 4],
            tyre_wear: [70.0; 4],
            brake_temp: [950.0; 4],
            tyre_temp_i: [130.0; 4],
            tyre_temp_m: [130.0; 4],
            tyre_temp_o: [130.0; 4],
            ..Default::default()
        };

        let recs = engineer.analyze_live(&phys, &AcGraphics::default(), None);

        // Four distinct problems. Ungrouped this was sixteen lines, and the
        // four the overlay publishes were all about tyre temperature.
        for expected in ["pressure", "WORN OUT", "brakes cooking", "OVERHEATING"] {
            assert!(
                recs.iter().any(|rec| rec.message.contains(expected)),
                "'{expected}' did not survive into the top of the list: {:?}",
                recs.iter().map(|r| &r.message).collect::<Vec<_>>()
            );
        }
    }

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

    /// The brake and tyre-temperature alerts had no hysteresis, unlike the
    /// pressure and wear alerts, so they pushed a fresh recommendation on
    /// every frame the condition held.
    #[test]
    fn overheating_alerts_are_not_repeated_every_frame() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        let graphics = AcGraphics::default();
        let physics = AcPhysics {
            speed_kmh: 150.0,
            brake_temp: [1500.0; 4],
            ..Default::default()
        };

        // The gate needs a second of sustained condition before it fires at
        // all, so the first burst produces nothing.
        let mut total = 0;
        for _ in 0..50 {
            total += engineer
                .analyze_live(&physics, &graphics, None)
                .iter()
                .filter(|rec| rec.category == "Overheat")
                .count();
        }
        assert_eq!(
            total, 0,
            "an alert should need a sustained condition, not a single frame"
        );
    }

    /// AC's own fuel_x_lap sits in the unverified tail of the graphics page
    /// and reads zero on lap one regardless, so the whole strategy tab was
    /// gated on a field that may never be populated.
    #[test]
    fn fuel_estimate_falls_back_to_measured_consumption() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        let session = crate::session_info::SessionInfo::default();

        // AC reports nothing, as it does on lap one and as it would
        // permanently if that offset is wrong.
        let graphics = |laps| AcGraphics {
            completed_laps: laps,
            fuel_x_lap: 0.0,
            ..Default::default()
        };
        let physics = |fuel| AcPhysics {
            fuel,
            speed_kmh: 120.0,
            ..Default::default()
        };

        // Start the stint with a full tank.
        engineer.update(&physics(50.0), &graphics(0), &session);
        assert_eq!(
            engineer.stats.fuel_laps_remaining, 0.0,
            "nothing measured yet, so no estimate is claimed"
        );

        // Two laps at 2.5 L each.
        engineer.update(&physics(47.5), &graphics(1), &session);
        engineer.update(&physics(45.0), &graphics(2), &session);

        assert!(
            (engineer.stats.fuel_consumption_rate - 2.5).abs() < 0.01,
            "measured burn, got {}",
            engineer.stats.fuel_consumption_rate
        );
        assert!(
            (engineer.stats.fuel_laps_remaining - 18.0).abs() < 0.1,
            "45 L at 2.5 L/lap is 18 laps, got {}",
            engineer.stats.fuel_laps_remaining
        );
    }

    /// Burn measured across a refuel is meaningless, and the stale estimate
    /// it produced could fire BOX BOX BOX on a car that had just filled up.
    #[test]
    fn refuelling_discards_the_measured_history() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        let session = crate::session_info::SessionInfo::default();
        let graphics = |laps| AcGraphics {
            completed_laps: laps,
            ..Default::default()
        };
        let physics = |fuel| AcPhysics {
            fuel,
            speed_kmh: 120.0,
            ..Default::default()
        };

        engineer.update(&physics(20.0), &graphics(0), &session);
        engineer.update(&physics(17.5), &graphics(1), &session);
        assert!(engineer.stats.fuel_laps_remaining > 0.0);

        // Pit stop: the tank goes up.
        engineer.update(&physics(60.0), &graphics(2), &session);
        assert_eq!(
            engineer.stats.fuel_laps_remaining, 0.0,
            "the estimate is dropped rather than carried across the stop"
        );
    }

    /// acc_g is [lateral, vertical, longitudinal]. Aggression used indices 0
    /// and 1, so it summed cornering with the ~1 g of gravity the car carries
    /// even parked, and never looked at braking or acceleration.
    #[test]
    fn aggression_ignores_the_vertical_axis() {
        let config = AppConfig::default();
        let mut engineer = Engineer::new(&config);
        let graphics = AcGraphics::default();
        let session = crate::session_info::SessionInfo::default();

        // Straight line, steady speed, 1 g down. Nothing aggressive here.
        let cruising = AcPhysics {
            speed_kmh: 120.0,
            acc_g: [0.0, 1.0, 0.0],
            ..Default::default()
        };
        for _ in 0..200 {
            engineer.update(&cruising, &graphics, &session);
        }
        let cruising_aggression = engineer.driving_style.aggression;
        assert!(
            cruising_aggression < 5.0,
            "vertical g must not register as aggression, got {cruising_aggression}"
        );

        // Hard braking. This is the case the old formula could not see at all,
        // because longitudinal g lives at index 2.
        let braking = AcPhysics {
            speed_kmh: 120.0,
            acc_g: [0.0, 1.0, -1.8],
            ..Default::default()
        };
        for _ in 0..200 {
            engineer.update(&braking, &graphics, &session);
        }
        assert!(
            engineer.driving_style.aggression > cruising_aggression + 20.0,
            "braking must raise aggression, got {}",
            engineer.driving_style.aggression
        );
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

        for (i, label) in labels.iter().enumerate() {
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
                corner_name: (*label).to_string(),
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
