use crate::ac_structs::{AcPhysics, AcGraphics};
use crate::setup_manager::CarSetup;
use crate::session_info::SessionInfo;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct AnalysisResult {
    pub category: String,
    pub metric: String,
    pub value: f32,
    pub unit: String,
    pub trend: Trend,
    pub recommendation: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, PartialOrd)]
pub enum Trend {
    Improving,
    Stable,
    Declining,
    Critical,
}

pub struct TelemetryAnalyzer {
    max_laps_stored: usize,
}

impl TelemetryAnalyzer {
    pub fn new() -> Self {
        Self {
            max_laps_stored: 20,
        }
    }
    
    pub fn analyze_session(
        &mut self,
        physics_history: &[AcPhysics],
        _graphics_history: &[AcGraphics],
        _session_info: &SessionInfo,
        _setup: Option<&CarSetup>,
    ) -> Vec<AnalysisResult> {
        let mut results = Vec::new();
        
        // Analyze tyre performance
        self.analyze_tyres(physics_history, &mut results);
        
        // Analyze braking performance
        self.analyze_braking(physics_history, &mut results);
        
        // Analyze cornering performance
        self.analyze_cornering(physics_history, &mut results);
        
        results
    }
    
    fn analyze_tyres(&self, physics_history: &[AcPhysics], results: &mut Vec<AnalysisResult>) {
        if physics_history.len() < 10 {
            return;
        }
        
        // Calculate average tyre temperatures
        let mut avg_temps = [0.0; 4];
        let mut temp_std_dev = [0.0; 4];
        
        for phys in physics_history.iter().rev().take(60) {
            for i in 0..4 {
                let temp = (phys.tyre_temp_i[i] + phys.tyre_temp_m[i] + phys.tyre_temp_o[i]) / 3.0;
                avg_temps[i] += temp;
            }
        }
        
        for i in 0..4 {
            avg_temps[i] /= physics_history.len().min(60) as f32;
        }
        
        // Calculate standard deviation
        for phys in physics_history.iter().rev().take(60) {
            for i in 0..4 {
                let temp = (phys.tyre_temp_i[i] + phys.tyre_temp_m[i] + phys.tyre_temp_o[i]) / 3.0;
                temp_std_dev[i] += (temp - avg_temps[i]).powi(2);
            }
        }
        
        for i in 0..4 {
            temp_std_dev[i] = (temp_std_dev[i] / physics_history.len().min(60) as f32).sqrt();
            
            let wheel_name = match i {
                0 => "Front Left",
                1 => "Front Right",
                2 => "Rear Left",
                3 => "Rear Right",
                _ => continue,
            };
            
            results.push(AnalysisResult {
                category: "Tyres".to_string(),
                metric: format!("{} Temp Consistency", wheel_name),
                value: temp_std_dev[i],
                unit: "°C".to_string(),
                trend: if temp_std_dev[i] < 3.0 { Trend::Stable } else { Trend::Declining },
                recommendation: if temp_std_dev[i] < 3.0 {
                    "Good temperature consistency".to_string()
                } else {
                    "Temperature fluctuating, check setup".to_string()
                },
            });
        }
    }
    
    fn analyze_braking(&self, physics_history: &[AcPhysics], results: &mut Vec<AnalysisResult>) {
        let mut brake_points = Vec::new();
        let mut in_braking_zone = false;
        
        for (index, phys) in physics_history.iter().enumerate() {
            if phys.brake > 0.5 && !in_braking_zone {
                in_braking_zone = true;
                brake_points.push(index);
            } else if phys.brake < 0.1 && in_braking_zone {
                in_braking_zone = false;
            }
        }
        
        if brake_points.len() >= 3 {
            let braking_consistency = self.calculate_braking_consistency(physics_history, &brake_points);
            
            results.push(AnalysisResult {
                category: "Braking".to_string(),
                metric: "Braking Consistency".to_string(),
                value: braking_consistency,
                unit: "%".to_string(),
                trend: if braking_consistency > 90.0 { Trend::Improving } else { Trend::Declining },
                recommendation: if braking_consistency > 90.0 {
                    "Consistent braking points".to_string()
                } else {
                    "Work on brake marker consistency".to_string()
                },
            });
        }
    }
    
    fn calculate_braking_consistency(&self, physics: &[AcPhysics], brake_points: &[usize]) -> f32 {
        if brake_points.len() < 2 {
            return 0.0;
        }
        
        let mut decelerations = Vec::new();
        
        for &point in brake_points.iter().take(5) {
            if point + 10 < physics.len() {
                let speed_start = physics[point].speed_kmh;
                let speed_end = physics[point + 10].speed_kmh;
                let decel = (speed_start - speed_end) / 0.166;
                decelerations.push(decel);
            }
        }
        
        if decelerations.is_empty() {
            return 0.0;
        }
        
        let avg: f32 = decelerations.iter().sum::<f32>() / decelerations.len() as f32;
        let variance: f32 = decelerations.iter().map(|&d| (d - avg).powi(2)).sum::<f32>() / decelerations.len() as f32;
        
        (1.0 / (1.0 + variance.sqrt()) * 100.0).min(100.0)
    }
    
    fn analyze_cornering(&self, physics_history: &[AcPhysics], results: &mut Vec<AnalysisResult>) {
        let mut cornering_effort = Vec::new();
        
        for phys in physics_history.iter().rev().take(180) {
            if phys.steer_angle.abs() > 0.3 {
                let lateral_g = (phys.acc_g[0].powi(2) + phys.acc_g[1].powi(2)).sqrt();
                cornering_effort.push(lateral_g);
            }
        }
        
        if !cornering_effort.is_empty() {
            let avg_lateral_g: f32 = cornering_effort.iter().sum::<f32>() / cornering_effort.len() as f32;
            let max_lateral_g = cornering_effort.iter().fold(0.0_f32, |max, &g| if g > max { g } else { max });
            
            results.push(AnalysisResult {
                category: "Cornering".to_string(),
                metric: "Average Lateral G".to_string(),
                value: avg_lateral_g,
                unit: "G".to_string(),
                trend: if avg_lateral_g > 1.2 { Trend::Improving } else { Trend::Stable },
                recommendation: if avg_lateral_g > 1.2 {
                    "Good cornering speed".to_string()
                } else {
                    "Can carry more speed through corners".to_string()
                },
            });
            
            results.push(AnalysisResult {
                category: "Cornering".to_string(),
                metric: "Peak Lateral G".to_string(),
                value: max_lateral_g,
                unit: "G".to_string(),
                trend: Trend::Stable,
                recommendation: format!("Peak grip: {:.2}G", max_lateral_g),
            });
        }
    }
}