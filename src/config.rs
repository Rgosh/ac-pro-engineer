use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use ratatui::style::Color as RatColor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: Theme,
    pub update_rate: u64,
    pub history_size: usize,
    pub alerts: AlertsConfig,
    pub auto_save: bool,
    pub data_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub background: ColorTuple,
    pub text: ColorTuple,
    pub highlight: ColorTuple,
    pub accent: ColorTuple,
    pub border: ColorTuple,
    pub warning: ColorTuple,
    pub critical: ColorTuple,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorTuple {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorTuple {
    pub fn to_color(&self) -> RatColor {
        RatColor::Rgb(self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsConfig {
    pub tyre_pressure_min: f32,
    pub tyre_pressure_max: f32,
    pub tyre_temp_min: f32,
    pub tyre_temp_max: f32,
    pub brake_temp_max: f32,
    pub fuel_warning_laps: f32,
    pub wear_warning: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            update_rate: 16,
            history_size: 300,
            alerts: AlertsConfig::default(),
            auto_save: true,
            data_path: PathBuf::from("./data/ac_pro_engineer"),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: ColorTuple { r: 10, g: 10, b: 20 },
            text: ColorTuple { r: 200, g: 200, b: 220 },
            highlight: ColorTuple { r: 100, g: 200, b: 255 },
            accent: ColorTuple { r: 255, g: 150, b: 50 },
            border: ColorTuple { r: 60, g: 60, b: 80 },
            warning: ColorTuple { r: 255, g: 200, b: 50 },
            critical: ColorTuple { r: 255, g: 50, b: 50 },
        }
    }
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            tyre_pressure_min: 26.0,
            tyre_pressure_max: 28.5,
            tyre_temp_min: 70.0,
            tyre_temp_max: 105.0,
            brake_temp_max: 800.0,
            fuel_warning_laps: 3.0,
            wear_warning: 80.0,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, anyhow::Error> {
        let config_path = PathBuf::from("./config.json");
        
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }
    
    pub fn save(&self) -> Result<(), anyhow::Error> {
        let config_dir = PathBuf::from(".");
        
        let config_path = config_dir.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        
        Ok(())
    }
}