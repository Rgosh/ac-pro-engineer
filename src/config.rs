use ratatui::style::Color as RatColor;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Language {
    English,
    Russian,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PressureUnit {
    Psi,
    Bar,
    Kpa,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TempUnit {
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub language: Language,
    pub update_rate: u64,
    pub history_size: usize,
    pub auto_save: bool,

    pub last_run_version: String,

    pub pressure_unit: PressureUnit,
    pub temp_unit: TempUnit,

    pub shift_point_offset: u32,
    pub fuel_safety_margin: f32,
    pub target_tyre_pressure: f32,
    pub enable_logging: bool,

    #[serde(default)]
    pub review_banner_hidden: bool,

    pub alerts: AlertsConfig,

    pub data_path: PathBuf,
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

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: ColorTuple {
                r: 10,
                g: 10,
                b: 15,
            },
            text: ColorTuple {
                r: 220,
                g: 220,
                b: 230,
            },
            highlight: ColorTuple {
                r: 0,
                g: 180,
                b: 255,
            },
            accent: ColorTuple {
                r: 255,
                g: 165,
                b: 0,
            },
            border: ColorTuple {
                r: 60,
                g: 70,
                b: 90,
            },
            warning: ColorTuple {
                r: 255,
                g: 220,
                b: 50,
            },
            critical: ColorTuple {
                r: 255,
                g: 50,
                b: 50,
            },
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::English,
            update_rate: 16,
            history_size: 300,
            auto_save: true,

            last_run_version: "0.0.0".to_string(),

            pressure_unit: PressureUnit::Psi,
            temp_unit: TempUnit::Celsius,

            shift_point_offset: 200,
            fuel_safety_margin: 1.0,
            target_tyre_pressure: 27.5,
            enable_logging: false,

            review_banner_hidden: false,

            alerts: AlertsConfig::default(),
            data_path: PathBuf::from("./data"),
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
            wear_warning: 96.0,
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
        let config_path = PathBuf::from("./config.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }
}
