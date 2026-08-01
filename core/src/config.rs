use directories_next::ProjectDirs;

pub fn app_dir() -> PathBuf {
    if let Some(proj) = ProjectDirs::from("com", "RaceEngineer", "RaceEngineer") {
        proj.config_dir().to_path_buf()
    } else {
        PathBuf::from("./data")
    }
}

pub fn app_config_path() -> PathBuf {
    app_dir().join("config.json")
}

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{error, info, warn};

/// Current config schema version. Increment when adding fields.
const CONFIG_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Language {
    English,
    Russian,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PressureUnit {
    Psi,
    Bar,
    Kpa,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TempUnit {
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Schema version for migration. Defaults to 1 for old configs.
    #[serde(default = "default_config_version")]
    pub config_version: u32,

    pub language: Language,
    #[serde(default = "default_update_rate")]
    pub update_rate: u64,
    #[serde(default = "default_history_size")]
    pub history_size: usize,
    #[serde(default = "default_true")]
    pub auto_save: bool,

    #[serde(default)]
    pub last_run_version: String,

    #[serde(default = "default_pressure_unit")]
    pub pressure_unit: PressureUnit,
    #[serde(default = "default_temp_unit")]
    pub temp_unit: TempUnit,

    #[serde(default = "default_shift_point_offset")]
    pub shift_point_offset: u32,
    #[serde(default = "default_fuel_safety_margin")]
    pub fuel_safety_margin: f32,
    #[serde(default = "default_target_tyre_pressure")]
    pub target_tyre_pressure: f32,
    #[serde(default = "default_target_hot_pressure_front")]
    pub target_hot_pressure_front: f32,
    #[serde(default = "default_target_hot_pressure_rear")]
    pub target_hot_pressure_rear: f32,
    #[serde(default = "default_true")]
    pub show_ghost_delta: bool,

    #[serde(default)]
    pub review_banner_hidden: bool,

    #[serde(default)]
    pub alerts: AlertsConfig,

    #[serde(default = "default_data_path")]
    pub data_path: PathBuf,
}

// Serde default helpers
fn default_config_version() -> u32 { 1 }
fn default_update_rate() -> u64 { 16 }
fn default_history_size() -> usize { 300 }
fn default_true() -> bool { true }
fn default_pressure_unit() -> PressureUnit { PressureUnit::Psi }
fn default_temp_unit() -> TempUnit { TempUnit::Celsius }
fn default_shift_point_offset() -> u32 { 200 }
fn default_fuel_safety_margin() -> f32 { 1.0 }
fn default_target_tyre_pressure() -> f32 { 27.5 }
fn default_target_hot_pressure_front() -> f32 { 27.5 }
fn default_target_hot_pressure_rear() -> f32 { 27.0 }
fn default_data_path() -> PathBuf { app_dir() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsConfig {
    #[serde(default = "default_tyre_pressure_min")]
    pub tyre_pressure_min: f32,
    #[serde(default = "default_tyre_pressure_max")]
    pub tyre_pressure_max: f32,
    #[serde(default = "default_tyre_temp_min")]
    pub tyre_temp_min: f32,
    #[serde(default = "default_tyre_temp_max")]
    pub tyre_temp_max: f32,
    #[serde(default = "default_brake_temp_max")]
    pub brake_temp_max: f32,
    #[serde(default = "default_fuel_warning_laps")]
    pub fuel_warning_laps: f32,
    #[serde(default = "default_wear_warning")]
    pub wear_warning: f32,
}

fn default_tyre_pressure_min() -> f32 { 26.0 }
fn default_tyre_pressure_max() -> f32 { 28.5 }
fn default_tyre_temp_min() -> f32 { 70.0 }
fn default_tyre_temp_max() -> f32 { 105.0 }
fn default_brake_temp_max() -> f32 { 800.0 }
fn default_fuel_warning_laps() -> f32 { 3.0 }
fn default_wear_warning() -> f32 { 96.0 }

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
            config_version: CONFIG_VERSION,
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
            target_hot_pressure_front: 27.5,
            target_hot_pressure_rear: 27.0,
            show_ghost_delta: true,

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

/// Formatter for pressure and temperature units configured in AppConfig.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitFormatter {
    pub pressure_unit: PressureUnit,
    pub temp_unit: TempUnit,
}

impl UnitFormatter {
    pub fn new(pressure_unit: PressureUnit, temp_unit: TempUnit) -> Self {
        Self {
            pressure_unit,
            temp_unit,
        }
    }

    /// Convert pressure from native AC unit (PSI) to configured unit value.
    pub fn pressure_val(&self, psi: f32) -> f32 {
        match self.pressure_unit {
            PressureUnit::Psi => psi,
            PressureUnit::Bar => psi * 0.0689476,
            PressureUnit::Kpa => psi * 6.89476,
        }
    }

    pub fn pressure_symbol(&self) -> &'static str {
        match self.pressure_unit {
            PressureUnit::Psi => "psi",
            PressureUnit::Bar => "bar",
            PressureUnit::Kpa => "kPa",
        }
    }

    pub fn format_pressure(&self, psi: f32) -> String {
        match self.pressure_unit {
            PressureUnit::Psi => format!("{:.1} psi", psi),
            PressureUnit::Bar => format!("{:.2} bar", self.pressure_val(psi)),
            PressureUnit::Kpa => format!("{:.1} kPa", self.pressure_val(psi)),
        }
    }

    /// Convert user input threshold in configured pressure unit back to native PSI.
    pub fn pressure_to_psi(&self, val: f32) -> f32 {
        match self.pressure_unit {
            PressureUnit::Psi => val,
            PressureUnit::Bar => val / 0.0689476,
            PressureUnit::Kpa => val / 6.89476,
        }
    }

    /// Convert temperature from native AC unit (Celsius) to configured unit value.
    pub fn temp_val(&self, temp_c: f32) -> f32 {
        match self.temp_unit {
            TempUnit::Celsius => temp_c,
            TempUnit::Fahrenheit => temp_c * 1.8 + 32.0,
        }
    }

    pub fn temp_symbol(&self) -> &'static str {
        match self.temp_unit {
            TempUnit::Celsius => "°C",
            TempUnit::Fahrenheit => "°F",
        }
    }

    pub fn format_temp(&self, temp_c: f32) -> String {
        match self.temp_unit {
            TempUnit::Celsius => format!("{:.0}°C", temp_c),
            TempUnit::Fahrenheit => format!("{:.0}°F", self.temp_val(temp_c)),
        }
    }

    pub fn format_temp_prec(&self, temp_c: f32, precision: usize) -> String {
        match self.temp_unit {
            TempUnit::Celsius => format!("{:.1$}°C", temp_c, precision),
            TempUnit::Fahrenheit => format!("{:.1$}°F", self.temp_val(temp_c), precision),
        }
    }

    /// Convert user input threshold in configured temp unit back to native Celsius.
    pub fn temp_to_celsius(&self, val: f32) -> f32 {
        match self.temp_unit {
            TempUnit::Celsius => val,
            TempUnit::Fahrenheit => (val - 32.0) / 1.8,
        }
    }
}

impl AppConfig {
    pub fn resolve_data_path(&self) -> PathBuf {
        if self.data_path.as_os_str().is_empty() || self.data_path == PathBuf::from("./data") {
            app_dir()
        } else {
            self.data_path.clone()
        }
    }

    pub fn formatter(&self) -> UnitFormatter {
        UnitFormatter::new(self.pressure_unit.clone(), self.temp_unit.clone())
    }

    /// Load config from disk with migration and backup support.
    pub fn load() -> Result<Self, anyhow::Error> {
        let config_path = app_config_path();
        if let Some(parent) = config_path.parent() { fs::create_dir_all(parent).ok(); }

        if !config_path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&config_path)?;

        // Try parsing with serde(default) — handles missing fields gracefully
        let mut config: AppConfig = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                // Config is corrupted — save backup and create default
                error!("Config parse error: {}. Creating backup.", e);
                let backup_path = config_path.with_extension("json.bak");
                if let Err(backup_err) = fs::copy(&config_path, &backup_path) {
                    warn!("Failed to create config backup: {}", backup_err);
                } else {
                    info!("Corrupted config backed up to {:?}", backup_path);
                }
                let default_config = Self::default();
                default_config.save()?;
                return Ok(default_config);
            }
        };

        // Run migrations
        config.migrate();

        // Save if migration bumped the version
        if config.config_version != CONFIG_VERSION
            || content != serde_json::to_string_pretty(&config).unwrap_or_default()
        {
            config.config_version = CONFIG_VERSION;
            config.save()?;
        }

        Ok(config)
    }

    /// Apply any necessary migrations from older config versions.
    fn migrate(&mut self) {
        if self.config_version < 2 {
            // v1 → v2: ensure alerts have reasonable defaults if they were 0
            if self.alerts.tyre_pressure_min <= 0.0 {
                self.alerts.tyre_pressure_min = 26.0;
            }
            if self.alerts.tyre_pressure_max <= 0.0 {
                self.alerts.tyre_pressure_max = 28.5;
            }
            info!("Config migrated from v{} to v{}", self.config_version, CONFIG_VERSION);
        }
        self.config_version = CONFIG_VERSION;
    }

    /// Validate config values, clamping any out-of-range values.
    pub fn validate(&mut self) {
        self.update_rate = self.update_rate.clamp(5, 1000);
        self.history_size = self.history_size.clamp(50, 10000);
        self.fuel_safety_margin = self.fuel_safety_margin.clamp(0.0, 10.0);
        self.alerts.fuel_warning_laps = self.alerts.fuel_warning_laps.clamp(0.5, 20.0);
        self.alerts.wear_warning = self.alerts.wear_warning.clamp(50.0, 100.0);
        self.alerts.brake_temp_max = self.alerts.brake_temp_max.clamp(200.0, 1200.0);
    }

    /// Save config atomically: write to .tmp then rename.
    pub fn save(&self) -> Result<(), anyhow::Error> {
        let config_path = app_config_path();
        if let Some(parent) = config_path.parent() { fs::create_dir_all(parent).ok(); }
        let temp_path = config_path.with_extension("json.tmp");

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&temp_path, &content)?;
        fs::rename(&temp_path, &config_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn config_loads_old_version_json() {
        let tmp = std::env::temp_dir().join("test_config_old");
        let _ = fs::create_dir_all(&tmp);
        let path = tmp.join("config.json");

        // Old v1 config without config_version, missing some fields
        let old_json = r#"{
            "language": "English",
            "update_rate": 16,
            "history_size": 300,
            "auto_save": true,
            "last_run_version": "0.1.0",
            "pressure_unit": "Psi",
            "temp_unit": "Celsius",
            "shift_point_offset": 200,
            "fuel_safety_margin": 1.0,
            "target_tyre_pressure": 27.5,
            "enable_logging": false,
            "alerts": {
                "tyre_pressure_min": 26.0,
                "tyre_pressure_max": 28.5,
                "tyre_temp_min": 70.0,
                "tyre_temp_max": 105.0,
                "brake_temp_max": 800.0,
                "fuel_warning_laps": 3.0,
                "wear_warning": 96.0
            },
            "data_path": "./data"
        }"#;

        let mut file = File::create(&path).expect("create");
        file.write_all(old_json.as_bytes()).expect("write");
        drop(file);

        let config: AppConfig = serde_json::from_str(old_json).expect("parse");
        assert_eq!(config.config_version, 1); // default for old config
        assert_eq!(config.language, Language::English);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn config_handles_unknown_fields() {
        let json_with_extra = r#"{
            "config_version": 2,
            "language": "English",
            "update_rate": 16,
            "history_size": 300,
            "auto_save": true,
            "last_run_version": "0.2.0",
            "pressure_unit": "Psi",
            "temp_unit": "Celsius",
            "shift_point_offset": 200,
            "fuel_safety_margin": 1.0,
            "target_tyre_pressure": 27.5,
            "enable_logging": false,
            "review_banner_hidden": false,
            "UNKNOWN_FUTURE_FIELD": 42,
            "another_future_field": "hello",
            "alerts": {
                "tyre_pressure_min": 26.0,
                "tyre_pressure_max": 28.5,
                "tyre_temp_min": 70.0,
                "tyre_temp_max": 105.0,
                "brake_temp_max": 800.0,
                "fuel_warning_laps": 3.0,
                "wear_warning": 96.0
            },
            "data_path": "./data"
        }"#;

        // serde by default ignores unknown fields — this should succeed
        let result: Result<AppConfig, _> = serde_json::from_str(json_with_extra);
        assert!(result.is_ok(), "Should handle unknown fields: {:?}", result.err());
    }

    #[test]
    fn config_handles_missing_fields() {
        // Minimal config — everything should default
        let minimal = r#"{"language": "Russian"}"#;
        let config: AppConfig = serde_json::from_str(minimal).expect("parse minimal");
        assert_eq!(config.language, Language::Russian);
        assert_eq!(config.update_rate, 16);
        assert_eq!(config.history_size, 300);
        assert!(config.auto_save);
    }

    #[test]
    fn config_corrupted_json_returns_error() {
        let bad_json = r#"{ this is not json }"#;
        let result: Result<AppConfig, _> = serde_json::from_str(bad_json);
        assert!(result.is_err());
    }

    #[test]
    fn config_validate_clamps_values() {
        let mut config = AppConfig::default();
        config.update_rate = 0;
        config.history_size = 999999;
        config.fuel_safety_margin = -5.0;
        config.alerts.fuel_warning_laps = 100.0;

        config.validate();

        assert_eq!(config.update_rate, 5);
        assert_eq!(config.history_size, 10000);
        assert_eq!(config.fuel_safety_margin, 0.0);
        assert_eq!(config.alerts.fuel_warning_laps, 20.0);
    }

    use std::fs::File;

    #[test]
    fn config_migration_v1_to_v2() {
        let mut config = AppConfig::default();
        config.config_version = 1;
        config.alerts.tyre_pressure_min = 0.0;

        config.migrate();

        assert_eq!(config.config_version, CONFIG_VERSION);
        assert!(config.alerts.tyre_pressure_min > 0.0);
    }

    #[test]
    fn unit_formatter_pressure_conversions() {
        let fmt_psi = UnitFormatter::new(PressureUnit::Psi, TempUnit::Celsius);
        assert_eq!(fmt_psi.pressure_val(27.5), 27.5);
        assert_eq!(fmt_psi.pressure_symbol(), "psi");
        assert_eq!(fmt_psi.format_pressure(27.5), "27.5 psi");
        assert_eq!(fmt_psi.pressure_to_psi(27.5), 27.5);

        let fmt_bar = UnitFormatter::new(PressureUnit::Bar, TempUnit::Celsius);
        let bar_val = fmt_bar.pressure_val(27.5);
        assert!((bar_val - 1.896).abs() < 0.01, "Expected ~1.896 bar, got {}", bar_val);
        assert_eq!(fmt_bar.pressure_symbol(), "bar");
        assert_eq!(fmt_bar.format_pressure(27.5), "1.90 bar");
        assert!((fmt_bar.pressure_to_psi(bar_val) - 27.5).abs() < 0.001);

        let fmt_kpa = UnitFormatter::new(PressureUnit::Kpa, TempUnit::Celsius);
        let kpa_val = fmt_kpa.pressure_val(27.5);
        assert!((kpa_val - 189.6).abs() < 0.1, "Expected ~189.6 kPa, got {}", kpa_val);
        assert_eq!(fmt_kpa.pressure_symbol(), "kPa");
        assert_eq!(fmt_kpa.format_pressure(27.5), "189.6 kPa");
        assert!((fmt_kpa.pressure_to_psi(kpa_val) - 27.5).abs() < 0.001);
    }

    #[test]
    fn unit_formatter_temp_conversions() {
        let fmt_c = UnitFormatter::new(PressureUnit::Psi, TempUnit::Celsius);
        assert_eq!(fmt_c.temp_val(100.0), 100.0);
        assert_eq!(fmt_c.temp_symbol(), "°C");
        assert_eq!(fmt_c.format_temp(100.0), "100°C");
        assert_eq!(fmt_c.temp_to_celsius(100.0), 100.0);

        let fmt_f = UnitFormatter::new(PressureUnit::Psi, TempUnit::Fahrenheit);
        assert_eq!(fmt_f.temp_val(100.0), 212.0);
        assert_eq!(fmt_f.temp_symbol(), "°F");
        assert_eq!(fmt_f.format_temp(100.0), "212°F");
        assert_eq!(fmt_f.format_temp_prec(100.0, 1), "212.0°F");
        assert_eq!(fmt_f.temp_to_celsius(212.0), 100.0);
    }

    #[test]
    fn config_unit_settings_roundtrip() {
        let mut config = AppConfig::default();
        config.pressure_unit = PressureUnit::Bar;
        config.temp_unit = TempUnit::Fahrenheit;

        let json = serde_json::to_string(&config).expect("serialize");
        let restored: AppConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.pressure_unit, PressureUnit::Bar);
        assert_eq!(restored.temp_unit, TempUnit::Fahrenheit);

        let fmt = restored.formatter();
        assert_eq!(fmt.format_pressure(27.5), "1.90 bar");
        assert_eq!(fmt.format_temp(100.0), "212°F");
    }
}
