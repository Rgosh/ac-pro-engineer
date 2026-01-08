use crate::ac_structs::{AcPhysics, AcGraphics};
use crate::setup_manager::CarSetup;
use crate::config::{AppConfig, Language};
use crate::session_info::SessionInfo;
use serde::Serialize;

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

pub struct Engineer {
    config: AppConfig,
    history_size: usize,
    
    pub stats: EngineerStats,
    pub driving_style: DrivingStyle,
}

#[derive(Debug, Clone)]
pub struct EngineerStats {
    pub bottoming_frames: [u32; 4],
    pub lockup_frames: u32,
    pub wheel_spin_frames: u32,
    pub traction_loss_frames: u32,
    pub oversteer_frames: u32,
    pub understeer_frames: u32,
    pub coasting_frames: u32, 
    pub total_frames: u32,
    
    pub fuel_laps_remaining: f32,
    pub fuel_consumption_rate: f32,
    
    pub current_delta: f32,
    pub predicted_lap_time: f32,
}

impl Engineer {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            config: config.clone(),
            history_size: 600,
            
            stats: EngineerStats::new(),
            driving_style: DrivingStyle::new(),
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
        
        for i in 0..4 {
            if phys.suspension_travel[i] < 0.005 {
                self.stats.bottoming_frames[i] += 1;
            }
        }
        
        if phys.speed_kmh > 30.0 {
            for i in 0..4 {
                if phys.slip_ratio[i].abs() > 0.2 && phys.brake > 0.1 {
                    self.stats.lockup_frames += 1;
                }
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

        let front_slip = (phys.slip_angle[0].abs() + phys.slip_angle[1].abs()) / 2.0;
        let rear_slip = (phys.slip_angle[2].abs() + phys.slip_angle[3].abs()) / 2.0;
        
        if front_slip > rear_slip + 3.0 && phys.speed_kmh > 40.0 && phys.steer_angle.abs() > 0.1 {
            self.stats.understeer_frames += 1;
        } else if rear_slip > front_slip + 3.0 && phys.speed_kmh > 40.0 {
            self.stats.oversteer_frames += 1;
        }
        
        if gfx.fuel_x_lap > 0.0 {
            self.stats.fuel_laps_remaining = phys.fuel / gfx.fuel_x_lap;
        }
        
        self.stats.current_delta = phys.performance_meter;
    }
    
    fn analyze_driving_style(&mut self, phys: &AcPhysics) {
        let throttle_smoothness = 100.0 - (phys.gas * 100.0).abs();
        let brake_smoothness = 100.0 - (phys.brake * 100.0).abs();
        
        self.driving_style.smoothness = 0.7 * self.driving_style.smoothness + 0.3 * (throttle_smoothness + brake_smoothness) / 2.0;
        
        let lateral_g = (phys.acc_g[0].powi(2) + phys.acc_g[1].powi(2)).sqrt();
        self.driving_style.aggression = 0.9 * self.driving_style.aggression + 0.1 * lateral_g.min(2.5) / 2.5 * 100.0;
        
        if phys.brake > 0.1 && phys.steer_angle.abs() > 0.1 {
            self.driving_style.trail_braking = 0.95 * self.driving_style.trail_braking + 0.05 * 100.0;
        } else {
            self.driving_style.trail_braking *= 0.98;
        }
    }
    
    fn reset_counters(&mut self) {
        self.stats.bottoming_frames = [0; 4];
        self.stats.lockup_frames = 0;
        self.stats.wheel_spin_frames = 0;
        self.stats.traction_loss_frames = 0;
        self.stats.oversteer_frames = 0;
        self.stats.understeer_frames = 0;
        self.stats.coasting_frames = 0;
        self.stats.total_frames = 0;
    }
    
    pub fn analyze_live(&mut self, phys: &AcPhysics, gfx: &AcGraphics, _setup: Option<&CarSetup>) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();
        
        self.analyze_tyre_pressure(phys, &mut recommendations);
        self.analyze_tyre_temperature(phys, &mut recommendations);
        self.analyze_brakes(phys, &mut recommendations);
        self.analyze_driving_errors(&mut recommendations);
        self.analyze_strategy(phys, gfx, &mut recommendations);
        
        recommendations.sort_by(|a, b| {
            b.severity.partial_cmp(&a.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
        });
        
        recommendations
    }
    
    fn is_ru(&self) -> bool {
        self.config.language == Language::Russian
    }

    pub fn compare_setups_advice(&self, target: &CarSetup, reference: &CarSetup) -> Vec<String> {
        let mut advice = Vec::new();
        let ru = self.is_ru();
        
        let aero_diff = (target.wing_1 + target.wing_2) as i32 - (reference.wing_1 + reference.wing_2) as i32;
        if aero_diff != 0 {
            advice.push(if ru { format!("Аэродинамика: {:+}", aero_diff) } else { format!("Aero: {:+}", aero_diff) });
        }
        
        let avg_p_target: f32 = (target.pressure_lf + target.pressure_rf + target.pressure_lr + target.pressure_rr) as f32 / 4.0;
        let avg_p_ref: f32 = (reference.pressure_lf + reference.pressure_rf + reference.pressure_lr + reference.pressure_rr) as f32 / 4.0;
        if (avg_p_target - avg_p_ref).abs() > 1.0 {
             advice.push(if ru { format!("Давление шин: {:+.1} PSI", avg_p_target - avg_p_ref) } else { format!("Tyre Press: {:+.1} PSI", avg_p_target - avg_p_ref) });
        }
        
        if advice.is_empty() {
            advice.push(if ru { "Нет существенных отличий".to_string() } else { "No major differences".to_string() });
        }
        advice
    }
    
    fn analyze_tyre_pressure(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let optimal_pressure = 27.5; 
        let tolerance = 1.5;
        let ru = self.is_ru();
        
        for i in 0..4 {
            let pressure = phys.wheels_pressure[i];
            let diff = (pressure - optimal_pressure).abs();
            
            if diff > tolerance {
                if phys.speed_kmh < 10.0 { continue; }

                let name = match i { 0=>"FL", 1=>"FR", 2=>"RL", 3=>"RR", _=>"" };
                
                recs.push(Recommendation {
                    component: if ru { "Шины".to_string() } else { "Tyres".to_string() },
                    category: if ru { "Давление".to_string() } else { "Pressure".to_string() },
                    severity: Severity::Warning,
                    message: if ru { format!("{} Давление вне нормы: {:.1}", name, pressure) } else { format!("{} Pressure bad: {:.1}", name, pressure) },
                    action: if pressure < optimal_pressure {
                        if ru { "Накачать".to_string() } else { "Inflate".to_string() }
                    } else {
                        if ru { "Спустить".to_string() } else { "Deflate".to_string() }
                    },
                    parameters: vec![
                        Parameter { name: "Target".to_string(), current: pressure, target: optimal_pressure, unit: "PSI".to_string() }
                    ],
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
                    let name = match i { 0=>"FL", 1=>"FR", 2=>"RL", 3=>"RR", _=>"" };
                    recs.push(Recommendation {
                        component: if ru { "Шины".to_string() } else { "Tyres".to_string() },
                        category: if ru { "Опасность".to_string() } else { "Danger".to_string() },
                        severity: Severity::Critical,
                        message: if ru { format!("{} ХОЛОДНАЯ: {:.0}°C", name, temp) } else { format!("{} COLD: {:.0}°C", name, temp) },
                        action: if ru { "Греть шины / Аккуратнее".to_string() } else { "Warm tyres / Careful".to_string() },
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
                    component: if ru { "Тормоза".to_string() } else { "Brakes".to_string() },
                    category: if ru { "Перегрев".to_string() } else { "Overheat".to_string() },
                    severity: Severity::Critical,
                    message: if ru { format!("Тормоз {} горит!", i+1) } else { format!("Brake {} cooking!", i+1) },
                    action: if ru { "Сместить баланс / Охладить".to_string() } else { "Move bias / Cool down".to_string() },
                    parameters: vec![],
                    confidence: 1.0,
                });
            }
        }
    }
    
    fn analyze_driving_errors(&self, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        
        if self.stats.coasting_frames > 60 { 
            recs.push(Recommendation {
                component: if ru { "Пилотаж".to_string() } else { "Driving".to_string() },
                category: if ru { "Потеря времени".to_string() } else { "Time Loss".to_string() },
                severity: Severity::Info,
                message: if ru { "Много наката (Coasting)".to_string() } else { "Excessive Coasting".to_string() },
                action: if ru { "Держите газ или тормозите позже".to_string() } else { "Keep throttle or brake later".to_string() },
                parameters: vec![],
                confidence: 0.7,
            });
        }

        if self.stats.understeer_frames > 30 {
            recs.push(Recommendation {
                component: if ru { "Баланс".to_string() } else { "Balance".to_string() },
                category: "Understeer".to_string(),
                severity: Severity::Warning,
                message: if ru { "Сильный снос передней оси".to_string() } else { "Heavy Understeer detected".to_string() },
                action: if ru { "Мягче на входе / Ждите зацепа".to_string() } else { "Smooth entry / Wait for grip".to_string() },
                parameters: vec![],
                confidence: 0.85,
            });
        }

        if self.stats.oversteer_frames > 30 {
            recs.push(Recommendation {
                component: if ru { "Баланс".to_string() } else { "Balance".to_string() },
                category: "Oversteer".to_string(),
                severity: Severity::Warning,
                message: if ru { "Потеря задней оси".to_string() } else { "Rear end loose".to_string() },
                action: if ru { "Аккуратнее с газом на выходе".to_string() } else { "Be gentle on throttle exit".to_string() },
                parameters: vec![],
                confidence: 0.85,
            });
        }
        
        if self.stats.lockup_frames > 15 {
             recs.push(Recommendation {
                component: if ru { "Торможение".to_string() } else { "Braking".to_string() },
                category: "Lockup".to_string(),
                severity: Severity::Warning,
                message: if ru { "Блокировка колес".to_string() } else { "Tyre Lockups".to_string() },
                action: if ru { "Уменьшите давление на тормоз".to_string() } else { "Reduce brake pressure".to_string() },
                parameters: vec![],
                confidence: 0.9,
            });
        }
    }
    
    // FIX: phys -> _phys
    fn analyze_strategy(&self, _phys: &AcPhysics, _gfx: &AcGraphics, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        if self.stats.fuel_laps_remaining < 2.5 && self.stats.fuel_laps_remaining > 0.0 {
            recs.push(Recommendation {
                component: if ru { "Стратегия".to_string() } else { "Strategy".to_string() },
                category: if ru { "Топливо".to_string() } else { "Fuel".to_string() },
                severity: Severity::Critical,
                message: if ru { format!("ТОПЛИВО: {:.1} кр.", self.stats.fuel_laps_remaining) } else { format!("FUEL LOW: {:.1} laps", self.stats.fuel_laps_remaining) },
                action: "BOX BOX BOX".to_string(),
                parameters: vec![],
                confidence: 1.0,
            });
        }
    }
}

impl EngineerStats {
    pub fn new() -> Self {
        Self {
            bottoming_frames: [0; 4],
            lockup_frames: 0,
            wheel_spin_frames: 0,
            traction_loss_frames: 0,
            oversteer_frames: 0,
            understeer_frames: 0,
            coasting_frames: 0,
            total_frames: 0,
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