use crate::ac_structs::{read_ac_string, AcGraphics, AcPhysics};
use crate::config::{AppConfig, Language};
use crate::session_info::SessionInfo;
use crate::setup_manager::CarSetup;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::{Duration, Instant};

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

    alert_timers: HashMap<String, Instant>,
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
    pub total_frames: u32,

    pub ffb_clip_frames: u32,

    pub input_history: Vec<(f64, f64, f64, f64, f64)>,

    pub fuel_laps_remaining: f32,
    pub fuel_consumption_rate: f32,

    pub current_delta: f32,
    pub predicted_lap_time: f32,
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
            total_frames: 0,

            ffb_clip_frames: 0,
            input_history: Vec::with_capacity(200),

            fuel_laps_remaining: 0.0,
            fuel_consumption_rate: 0.0,
            current_delta: 0.0,
            predicted_lap_time: 0.0,
        }
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
        }
    }
}

impl Engineer {
    pub fn new(config: &AppConfig) -> Self {
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

    pub fn update(&mut self, phys: &AcPhysics, gfx: &AcGraphics, _session: &SessionInfo) {
        self.update_stats(phys, gfx);
        self.analyze_driving_style(phys);

        if self.stats.total_frames > self.history_size as u32 {
            self.reset_counters();
        }
    }

    fn update_stats(&mut self, phys: &AcPhysics, gfx: &AcGraphics) {
        self.stats.total_frames += 1;

        if phys.final_ff.abs() > 0.98 {
            self.stats.ffb_clip_frames += 1;
        }

        if self.stats.total_frames % 3 == 0 {
            let t = self.stats.total_frames as f64;
            self.stats.input_history.push((
                t,
                phys.steer_angle as f64,
                phys.gas as f64,
                phys.brake as f64,
                phys.final_ff as f64,
            ));
            if self.stats.input_history.len() > 200 {
                self.stats.input_history.remove(0);
            }
        }

        for i in 0..4 {
            if phys.suspension_travel[i] < 0.005 {
                self.stats.bottoming_frames[i] += 1;
            }
        }

        if phys.speed_kmh > 30.0 {
            if (phys.wheel_slip[0].abs() > 0.2 || phys.wheel_slip[1].abs() > 0.2)
                && phys.brake > 0.1
            {
                self.stats.lockup_frames_front += 1;
            }
            if (phys.wheel_slip[2].abs() > 0.2 || phys.wheel_slip[3].abs() > 0.2)
                && phys.brake > 0.1
            {
                self.stats.lockup_frames_rear += 1;
            }
        }

        for i in 0..4 {
            if phys.wheel_slip[i] > 0.15 && phys.gas > 0.3 && phys.speed_kmh < 120.0 {
                self.stats.wheel_spin_frames += 1;
            }
        }

        if phys.speed_kmh > 30.0 && phys.gas < 0.05 && phys.brake < 0.05 {
            self.stats.coasting_frames += 1;
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
        let throttle_smoothness = 100.0 - (phys.gas * 100.0).abs();
        let brake_smoothness = 100.0 - (phys.brake * 100.0).abs();

        self.driving_style.smoothness = 0.7 * self.driving_style.smoothness
            + 0.3 * (throttle_smoothness + brake_smoothness) / 2.0;

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
        self.stats.total_frames = 0;
        self.stats.ffb_clip_frames = 0;
    }

    fn check_hysteresis(&mut self, key: &str, active: bool) -> bool {
        let now = Instant::now();
        if active {
            self.alert_timers.insert(key.to_string(), now);
            return true;
        }

        if let Some(last_seen) = self.alert_timers.get(key) {
            if now.duration_since(*last_seen) < Duration::from_secs_f32(2.0) {
                return true;
            }
        }
        false
    }

    pub fn analyze_live(
        &mut self,
        phys: &AcPhysics,
        gfx: &AcGraphics,
        _setup: Option<&CarSetup>,
    ) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        self.analyze_tyre_pressure(phys, gfx, &mut recommendations);
        self.analyze_tyre_temperature(phys, &mut recommendations);
        self.analyze_tyre_wear(phys, &mut recommendations);
        self.analyze_camber(phys, &mut recommendations);
        self.analyze_brakes(phys, &mut recommendations);
        self.analyze_brake_bias(&mut recommendations);
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

    pub fn get_wizard_advice(&self) -> Vec<String> {
        let is_ru = self.config.language == Language::Russian;
        let mut advice = Vec::new();

        match (&self.wizard_phase, &self.wizard_problem) {
            (WizardPhase::Entry, WizardProblem::Understeer) => {
                advice.push(
                    if is_ru {
                        "Уменьшить отбой (Rebound) спереди"
                    } else {
                        "Decrease Front Rebound"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Увеличить клиренс сзади"
                    } else {
                        "Increase Rear Ride Height"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Сместить тормозной баланс назад"
                    } else {
                        "Move Brake Bias Rearwards"
                    }
                    .to_string(),
                );
            }
            (WizardPhase::Entry, WizardProblem::Oversteer) => {
                advice.push(
                    if is_ru {
                        "Увеличить отбой (Rebound) спереди"
                    } else {
                        "Increase Front Rebound"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Сместить тормозной баланс вперед"
                    } else {
                        "Move Brake Bias Forwards"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Увеличить переднее антикрыло"
                    } else {
                        "Increase Front Wing"
                    }
                    .to_string(),
                );
            }
            (WizardPhase::Apex, WizardProblem::Understeer) => {
                advice.push(
                    if is_ru {
                        "Мягче передние пружины"
                    } else {
                        "Softer Front Springs"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Мягче передний стабилизатор (ARB)"
                    } else {
                        "Softer Front ARB"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Больше развал (Camber) спереди"
                    } else {
                        "More Front Camber"
                    }
                    .to_string(),
                );
            }
            (WizardPhase::Apex, WizardProblem::Oversteer) => {
                advice.push(
                    if is_ru {
                        "Мягче задние пружины"
                    } else {
                        "Softer Rear Springs"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Мягче задний стабилизатор (ARB)"
                    } else {
                        "Softer Rear ARB"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Выше клиренс спереди"
                    } else {
                        "Increase Front Ride Height"
                    }
                    .to_string(),
                );
            }
            (WizardPhase::Exit, WizardProblem::Understeer) => {
                advice.push(
                    if is_ru {
                        "Увеличить сжатие (Bump) спереди"
                    } else {
                        "Increase Front Bump"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Жестче задние пружины"
                    } else {
                        "Stiffer Rear Springs"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Увеличить блокировку дифференциала (Power)"
                    } else {
                        "Increase Diff Power"
                    }
                    .to_string(),
                );
            }
            (WizardPhase::Exit, WizardProblem::Oversteer) => {
                advice.push(
                    if is_ru {
                        "Мягче задние пружины"
                    } else {
                        "Softer Rear Springs"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Уменьшить сжатие (Bump) сзади"
                    } else {
                        "Decrease Rear Bump"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Уменьшить блокировку дифференциала (Power)"
                    } else {
                        "Decrease Diff Power"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Больше Traction Control"
                    } else {
                        "Increase TC"
                    }
                    .to_string(),
                );
            }
            (_, WizardProblem::Instability) => {
                advice.push(
                    if is_ru {
                        "Увеличить прижимную силу (Крылья)"
                    } else {
                        "Increase Downforce (Wings)"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Больше схождения (Toe) сзади"
                    } else {
                        "More Rear Toe-In"
                    }
                    .to_string(),
                );
                advice.push(
                    if is_ru {
                        "Жестче подвеску в целом"
                    } else {
                        "Stiffer Suspension Overall"
                    }
                    .to_string(),
                );
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
                    "Руль (FFB)"
                } else {
                    "Force Feedback"
                }
                .to_string(),
                category: "Clipping".to_string(),
                severity: Severity::Warning,
                message: if ru {
                    format!("Клиппинг силы: {:.1}% времени", clip_ratio * 100.0)
                } else {
                    format!("FFB Clipping: {:.1}% of time", clip_ratio * 100.0)
                },
                action: if ru {
                    "Снизить Gain"
                } else {
                    "Lower FFB Gain"
                }
                .to_string(),
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

        let compound_name = read_ac_string(&gfx.tyre_compound).to_lowercase();

        let (optimal_pressure, tolerance, class_name) = if compound_name.contains("street")
            || compound_name.contains("sport")
            || compound_name.contains("eco")
            || compound_name.contains("semislick")
        {
            (32.0, 2.0, "Street")
        } else if compound_name.contains("wet") || compound_name.contains("rain") {
            (30.0, 1.5, "Wet")
        } else {
            (27.5, 1.2, "Racing")
        };

        for i in 0..4 {
            let pressure = phys.wheels_pressure[i];
            let diff = (pressure - optimal_pressure).abs();
            let is_error = diff > tolerance;

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
                    if ru {
                        "Накачать"
                    } else {
                        "Inflate"
                    }
                } else {
                    if ru {
                        "Спустить"
                    } else {
                        "Deflate"
                    }
                }
                .to_string();

                recs.push(Recommendation {
                    component: if ru {
                        format!("Шины ({})", class_name)
                    } else {
                        format!("Tyres ({})", class_name)
                    }
                    .to_string(),
                    category: if ru { "Давление" } else { "Pressure" }.to_string(),
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
            let is_worn = wear < 96.0;

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
                let (severity, msg_en, msg_ru) = if wear < 94.0 {
                    (Severity::Critical, "WORN OUT", "ИЗНОС (Крит)")
                } else {
                    (Severity::Warning, "High Wear", "Сильный износ")
                };

                recs.push(Recommendation {
                    component: if ru { "Шины" } else { "Tyres" }.to_string(),
                    category: if ru { "Износ" } else { "Wear" }.to_string(),
                    severity,
                    message: if ru {
                        format!("{} {}: {:.1}%", name, msg_ru, wear)
                    } else {
                        format!("{} {}: {:.1}%", name, msg_en, wear)
                    },
                    action: if ru {
                        "Пит-стоп / Осторожно"
                    } else {
                        "Box / Careful"
                    }
                    .to_string(),
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

    fn analyze_camber(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        if phys.speed_kmh < 50.0 {
            return;
        }
        let ideal_spread = 8.0;

        for i in 0..4 {
            let temp_i = phys.tyre_temp_i[i];
            let temp_o = phys.tyre_temp_o[i];
            let spread = temp_i - temp_o;

            if spread < 2.0 {
                let name = match i {
                    0 => "FL",
                    1 => "FR",
                    2 => "RL",
                    3 => "RR",
                    _ => "",
                };
                recs.push(Recommendation {
                    component: if ru { "Подвеска" } else { "Suspension" }.to_string(),
                    category: if ru { "Развал" } else { "Camber" }.to_string(),
                    severity: Severity::Info,
                    message: if ru {
                        format!(
                            "{} Пятно контакта не эффективно (I-O: {:.1}C)",
                            name, spread
                        )
                    } else {
                        format!("{} Contact patch inefficient (I-O: {:.1}C)", name, spread)
                    },
                    action: if ru {
                        "Увеличить отриц. развал"
                    } else {
                        "Increase Neg. Camber"
                    }
                    .to_string(),
                    parameters: vec![Parameter {
                        name: "Temp Spread".to_string(),
                        current: spread,
                        target: ideal_spread,
                        unit: "°C".to_string(),
                    }],
                    confidence: 0.7,
                });
            } else if spread > 15.0 {
                let name = match i {
                    0 => "FL",
                    1 => "FR",
                    2 => "RL",
                    3 => "RR",
                    _ => "",
                };
                recs.push(Recommendation {
                    component: if ru { "Подвеска" } else { "Suspension" }.to_string(),
                    category: if ru { "Развал" } else { "Camber" }.to_string(),
                    severity: Severity::Warning,
                    message: if ru {
                        format!("{} Перегрев внутренней части (I-O: {:.1}C)", name, spread)
                    } else {
                        format!("{} Inner edge overheating (I-O: {:.1}C)", name, spread)
                    },
                    action: if ru {
                        "Уменьшить отриц. развал"
                    } else {
                        "Decrease Neg. Camber"
                    }
                    .to_string(),
                    parameters: vec![Parameter {
                        name: "Temp Spread".to_string(),
                        current: spread,
                        target: ideal_spread,
                        unit: "°C".to_string(),
                    }],
                    confidence: 0.8,
                });
            }
        }
    }

    fn analyze_tyre_temperature(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let min_temp = 70.0;
        let ru = self.is_ru();

        if phys.speed_kmh > 150.0 {
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
                        component: if ru { "Шины" } else { "Tyres" }.to_string(),
                        category: if ru {
                            "Температура"
                        } else {
                            "Temperature"
                        }
                        .to_string(),
                        severity: Severity::Critical,
                        message: if ru {
                            format!("{} ХОЛОДНАЯ: {:.0}°C", name, temp)
                        } else {
                            format!("{} COLD: {:.0}°C", name, temp)
                        },
                        action: if ru {
                            "Греть шины / Аккуратнее"
                        } else {
                            "Warm tyres / Careful"
                        }
                        .to_string(),
                        parameters: vec![],
                        confidence: 0.95,
                    });
                }
            }
        }
    }

    fn analyze_brakes(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let max_temp = 750.0;
        let ru = self.is_ru();
        for i in 0..4 {
            if phys.brake_temp[i] > max_temp {
                recs.push(Recommendation {
                    component: if ru { "Тормоза" } else { "Brakes" }.to_string(),
                    category: if ru { "Перегрев" } else { "Overheat" }.to_string(),
                    severity: Severity::Critical,
                    message: if ru {
                        format!("Тормоз {} горит!", i + 1)
                    } else {
                        format!("Brake {} cooking!", i + 1)
                    },
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
        }
    }

    fn analyze_brake_bias(&self, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        let total_lockups = self.stats.lockup_frames_front + self.stats.lockup_frames_rear;

        if total_lockups > 20 {
            if self.stats.lockup_frames_front > self.stats.lockup_frames_rear * 2 {
                recs.push(Recommendation {
                    component: if ru { "Тормоза" } else { "Brakes" }.to_string(),
                    category: if ru { "Баланс" } else { "Bias" }.to_string(),
                    severity: Severity::Warning,
                    message: if ru {
                        "Блокировка ПЕРЕДНИХ колес"
                    } else {
                        "FRONT Locking detected"
                    }
                    .to_string(),
                    action: if ru {
                        "Сместить баланс НАЗАД"
                    } else {
                        "Move Bias REARWARDS"
                    }
                    .to_string(),
                    parameters: vec![],
                    confidence: 0.85,
                });
            } else if self.stats.lockup_frames_rear > self.stats.lockup_frames_front * 2 {
                recs.push(Recommendation {
                    component: if ru { "Тормоза" } else { "Brakes" }.to_string(),
                    category: if ru { "Баланс" } else { "Bias" }.to_string(),
                    severity: Severity::Critical,
                    message: if ru {
                        "Блокировка ЗАДНИХ колес (Опасно!)"
                    } else {
                        "REAR Locking (Danger!)"
                    }
                    .to_string(),
                    action: if ru {
                        "Сместить баланс ВПЕРЕД"
                    } else {
                        "Move Bias FORWARDS"
                    }
                    .to_string(),
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
                component: if ru { "Пилотаж" } else { "Driving" }.to_string(),
                category: if ru {
                    "Потеря времени"
                } else {
                    "Time Loss"
                }
                .to_string(),
                severity: Severity::Info,
                message: if ru {
                    "Много наката (Coasting)"
                } else {
                    "Excessive Coasting"
                }
                .to_string(),
                action: if ru {
                    "Держите газ или тормозите"
                } else {
                    "Keep throttle or brake"
                }
                .to_string(),
                parameters: vec![],
                confidence: 0.7,
            });
        }

        if self.stats.understeer_frames > 30 {
            recs.push(Recommendation {
                component: if ru { "Баланс" } else { "Balance" }.to_string(),
                category: "Understeer".to_string(),
                severity: Severity::Warning,
                message: if ru {
                    "Снос передней оси (High Speed)"
                } else {
                    "High Speed Understeer"
                }
                .to_string(),
                action: if ru {
                    "Больше крыла спереди / Мягче спереди"
                } else {
                    "More Front Wing / Softer Front"
                }
                .to_string(),
                parameters: vec![],
                confidence: 0.85,
            });
        }

        if self.stats.oversteer_frames > 30 {
            recs.push(Recommendation {
                component: if ru { "Баланс" } else { "Balance" }.to_string(),
                category: "Oversteer".to_string(),
                severity: Severity::Warning,
                message: if ru {
                    "Нестабильность сзади (High Speed)"
                } else {
                    "High Speed Oversteer"
                }
                .to_string(),
                action: if ru {
                    "Больше крыла сзади"
                } else {
                    "More Rear Wing"
                }
                .to_string(),
                parameters: vec![],
                confidence: 0.85,
            });
        }
    }

    fn analyze_strategy(&self, phys: &AcPhysics, gfx: &AcGraphics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();

        if self.stats.fuel_laps_remaining < 2.5 && self.stats.fuel_laps_remaining > 0.0 {
            recs.push(Recommendation {
                component: if ru { "Стратегия" } else { "Strategy" }.to_string(),
                category: if ru { "Топливо" } else { "Fuel" }.to_string(),
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

        if gfx.session_time_left > 0.0 && gfx.fuel_x_lap > 0.0 {
            let time_left_sec = gfx.session_time_left / 1000.0;
            let lap_time_sec = if self.stats.predicted_lap_time > 0.0 {
                self.stats.predicted_lap_time
            } else {
                0.0
            };

            if lap_time_sec > 30.0 {
                let laps_remaining_in_race = time_left_sec / lap_time_sec;
                let fuel_needed = laps_remaining_in_race * gfx.fuel_x_lap;
                let fuel_diff = phys.fuel - fuel_needed;

                if fuel_diff < -1.0 {
                    recs.push(Recommendation {
                        component: if ru { "Стратегия" } else { "Strategy" }.to_string(),
                        category: if ru { "Финиш" } else { "Race Finish" }.to_string(),
                        severity: Severity::Warning,
                        message: if ru {
                            format!("Не хватит {:.1} л.", fuel_diff.abs())
                        } else {
                            format!("Short {:.1} L", fuel_diff.abs())
                        },
                        action: if ru {
                            "Экономить / Пит-стоп"
                        } else {
                            "Save Fuel / Box"
                        }
                        .to_string(),
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
