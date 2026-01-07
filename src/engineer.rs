use crate::ac_structs::{AcPhysics, AcGraphics};
use crate::setup_manager::CarSetup;
use crate::config::AppConfig;
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
    pub total_frames: u32,
    
    pub fuel_laps_remaining: f32,
    pub fuel_consumption_rate: f32,
    
    pub tyre_wear_rate: [f32; 4],
    pub tyre_temp_consistency: [f32; 4],
    
    pub brake_wear_rate: f32,
    pub brake_temp_avg: f32,
    
    pub current_delta: f32,
    pub sector_deltas: [f32; 3],
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
            if phys.wheel_slip[i] > 0.15 && phys.gas > 0.3 && phys.speed_kmh < 100.0 {
                self.stats.wheel_spin_frames += 1;
            }
        }
        
        let total_slip: f32 = phys.wheel_slip.iter().sum();
        if total_slip > 0.3 && phys.speed_kmh > 50.0 {
            self.stats.traction_loss_frames += 1;
        }
        
        let front_slip = (phys.slip_angle[0].abs() + phys.slip_angle[1].abs()) / 2.0;
        let rear_slip = (phys.slip_angle[2].abs() + phys.slip_angle[3].abs()) / 2.0;
        
        if front_slip > rear_slip + 2.0 && phys.speed_kmh > 30.0 {
            self.stats.understeer_frames += 1;
        } else if rear_slip > front_slip + 2.0 && phys.speed_kmh > 30.0 {
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
        self.driving_style.aggression = 0.9 * self.driving_style.aggression + 0.1 * lateral_g.min(2.0) / 2.0 * 100.0;
        
        if phys.brake > 0.1 && phys.steer_angle.abs() > 0.1 {
            self.driving_style.trail_braking = 0.95 * self.driving_style.trail_braking + 0.05 * 100.0;
        } else {
            self.driving_style.trail_braking *= 0.99;
        }
    }
    
    fn reset_counters(&mut self) {
        self.stats.bottoming_frames = [0; 4];
        self.stats.lockup_frames = 0;
        self.stats.wheel_spin_frames = 0;
        self.stats.traction_loss_frames = 0;
        self.stats.oversteer_frames = 0;
        self.stats.understeer_frames = 0;
        self.stats.total_frames = 0;
    }
    
    pub fn analyze_live(&mut self, phys: &AcPhysics, gfx: &AcGraphics, _setup: Option<&CarSetup>) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();
        
        self.analyze_tyre_pressure(phys, &mut recommendations);
        self.analyze_tyre_temperature(phys, &mut recommendations);
        self.analyze_brakes(phys, &mut recommendations);
        self.analyze_driving_style_rec(&mut recommendations);
        self.analyze_strategy(phys, gfx, &mut recommendations);
        
        recommendations.sort_by(|a, b| {
            b.severity.partial_cmp(&a.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
        });
        
        recommendations
    }
    
    fn analyze_tyre_pressure(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let optimal_pressure = 27.0;
        let tolerance = 1.0;
        
        for i in 0..4 {
            let pressure = phys.wheels_pressure[i];
            let name = match i {
                0 => "Front Left",
                1 => "Front Right",
                2 => "Rear Left",
                3 => "Rear Right",
                _ => continue,
            };
            
            let diff = (pressure - optimal_pressure).abs();
            if diff > tolerance {
                let severity = if diff > 2.0 { Severity::Warning } else { Severity::Info };
                let action = if pressure < optimal_pressure {
                    format!("Increase pressure by {:.1} PSI", optimal_pressure - pressure)
                } else {
                    format!("Decrease pressure by {:.1} PSI", pressure - optimal_pressure)
                };
                
                recs.push(Recommendation {
                    component: "Tyres".to_string(),
                    category: "Pressure".to_string(),
                    severity,
                    message: format!("{} tyre pressure outside optimal range: {:.1} PSI", name, pressure),
                    action,
                    parameters: vec![
                        Parameter {
                            name: "Current Pressure".to_string(),
                            current: pressure,
                            target: optimal_pressure,
                            unit: "PSI".to_string(),
                        }
                    ],
                    confidence: 0.9,
                });
            }
        }
    }
    
    fn analyze_tyre_temperature(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let optimal_temp = 85.0;
        let min_temp = 70.0;
        let max_temp = 105.0;
        
        for i in 0..4 {
            let avg_temp = phys.get_avg_tyre_temp(i);
            let gradient = phys.get_tyre_gradient(i);
            let name = match i {
                0 => "Front Left",
                1 => "Front Right",
                2 => "Rear Left",
                3 => "Rear Right",
                _ => continue,
            };
            
            if avg_temp < min_temp {
                recs.push(Recommendation {
                    component: "Tyres".to_string(),
                    category: "Temperature".to_string(),
                    severity: Severity::Warning,
                    message: format!("{} tyre too cold: {:.1}°C", name, avg_temp),
                    action: "Increase camber or add pressure".to_string(),
                    parameters: vec![
                        Parameter {
                            name: "Temperature".to_string(),
                            current: avg_temp,
                            target: optimal_temp,
                            unit: "°C".to_string(),
                        }
                    ],
                    confidence: 0.8,
                });
            } else if avg_temp > max_temp {
                recs.push(Recommendation {
                    component: "Tyres".to_string(),
                    category: "Temperature".to_string(),
                    severity: Severity::Critical,
                    message: format!("{} tyre overheating: {:.1}°C", name, avg_temp),
                    action: "Reduce camber or lower pressure".to_string(),
                    parameters: vec![
                        Parameter {
                            name: "Temperature".to_string(),
                            current: avg_temp,
                            target: optimal_temp,
                            unit: "°C".to_string(),
                        }
                    ],
                    confidence: 0.9,
                });
            }
            
            if gradient.abs() > 15.0 {
                let issue = if gradient > 0.0 { "outside overheating" } else { "inside overheating" };
                recs.push(Recommendation {
                    component: "Tyres".to_string(),
                    category: "Wear Pattern".to_string(),
                    severity: Severity::Warning,
                    message: format!("{} tyre has {} (gradient: {:.1}°C)", name, issue, gradient),
                    action: "Adjust camber or toe settings".to_string(),
                    parameters: vec![],
                    confidence: 0.7,
                });
            }
        }
    }
    
    fn analyze_brakes(&self, phys: &AcPhysics, recs: &mut Vec<Recommendation>) {
        let max_brake_temp = 800.0;
        let optimal_brake_temp = 500.0;
        
        for i in 0..4 {
            let temp = phys.brake_temp[i];
            if temp > max_brake_temp {
                recs.push(Recommendation {
                    component: "Brakes".to_string(),
                    category: "Temperature".to_string(),
                    severity: Severity::Critical,
                    message: format!("Brake {} overheating: {:.0}°C", i+1, temp),
                    action: "Move brake bias back or open cooling ducts".to_string(),
                    parameters: vec![
                        Parameter {
                            name: "Brake Temperature".to_string(),
                            current: temp,
                            target: optimal_brake_temp,
                            unit: "°C".to_string(),
                        }
                    ],
                    confidence: 0.95,
                });
            }
        }
        
        let optimal_bias = 0.52;
        let current_bias = phys.brake_bias;
        if (current_bias - optimal_bias).abs() > 0.02 {
            recs.push(Recommendation {
                component: "Brakes".to_string(),
                category: "Bias".to_string(),
                severity: Severity::Warning,
                message: format!("Brake bias suboptimal: {:.1}%", current_bias * 100.0),
                action: format!("Adjust bias to {:.1}%", optimal_bias * 100.0),
                parameters: vec![
                    Parameter {
                        name: "Brake Bias".to_string(),
                        current: current_bias,
                        target: optimal_bias,
                        unit: "%".to_string(),
                    }
                ],
                confidence: 0.8,
            });
        }
    }
    
    fn analyze_driving_style_rec(&self, recs: &mut Vec<Recommendation>) {
        if self.driving_style.aggression > 80.0 {
            recs.push(Recommendation {
                component: "Driving Style".to_string(),
                category: "Aggression".to_string(),
                severity: Severity::Warning,
                message: "Aggressive driving detected".to_string(),
                action: "Smooth inputs to preserve tyres".to_string(),
                parameters: vec![
                    Parameter {
                        name: "Aggression Level".to_string(),
                        current: self.driving_style.aggression,
                        target: 60.0,
                        unit: "%".to_string(),
                    }
                ],
                confidence: 0.75,
            });
        }
        
        if self.stats.lockup_frames > 20 {
            recs.push(Recommendation {
                component: "Driving Style".to_string(),
                category: "Braking".to_string(),
                severity: Severity::Warning,
                message: "Frequent brake lockups detected".to_string(),
                action: "Increase ABS setting or brake earlier".to_string(),
                parameters: vec![],
                confidence: 0.9,
            });
        }
    }
    
    fn analyze_strategy(&self, phys: &AcPhysics, _gfx: &AcGraphics, recs: &mut Vec<Recommendation>) {
        if self.stats.fuel_laps_remaining < 3.0 && self.stats.fuel_laps_remaining > 0.0 {
            recs.push(Recommendation {
                component: "Strategy".to_string(),
                category: "Fuel".to_string(),
                severity: Severity::Critical,
                message: format!("Low fuel: {:.1} laps remaining", self.stats.fuel_laps_remaining),
                action: "PIT THIS LAP".to_string(),
                parameters: vec![
                    Parameter {
                        name: "Fuel Laps Remaining".to_string(),
                        current: self.stats.fuel_laps_remaining,
                        target: 5.0,
                        unit: "laps".to_string(),
                    }
                ],
                confidence: 1.0,
            });
        }
        
        let avg_wear: f32 = phys.tyre_wear.iter().sum::<f32>() / 4.0;
        if avg_wear > 80.0 {
            recs.push(Recommendation {
                component: "Strategy".to_string(),
                category: "Tyres".to_string(),
                severity: Severity::Warning,
                message: format!("High tyre wear: {:.0}%", avg_wear),
                action: "Consider pit stop for fresh tyres".to_string(),
                parameters: vec![],
                confidence: 0.8,
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
            total_frames: 0,
            fuel_laps_remaining: 0.0,
            fuel_consumption_rate: 0.0,
            tyre_wear_rate: [0.0; 4],
            tyre_temp_consistency: [0.0; 4],
            brake_wear_rate: 0.0,
            brake_temp_avg: 0.0,
            current_delta: 0.0,
            sector_deltas: [0.0; 3],
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