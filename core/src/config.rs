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

    #[serde(default = "default_language")]
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

    /// What each key does. See [`KeyBindings`].
    #[serde(default)]
    pub keys: KeyBindings,

    /// What the in-game overlay shows. Published as flags on every frame, so a
    /// change here reaches the panel on the next tick without a restart.
    #[serde(default)]
    pub overlay: OverlayConfig,

    #[serde(default = "default_data_path")]
    pub data_path: PathBuf,

    /// Where Assetto Corsa is installed. Empty means auto-detect.
    ///
    /// The escape hatch for an install `ac_paths` cannot find on its own: a
    /// non-Steam copy, a library Steam's own metadata does not describe, or a
    /// Proton prefix somewhere unusual.
    #[serde(default)]
    pub ac_install_path: PathBuf,

    /// The Documents folder AC reads setups from. Empty means auto-detect.
    ///
    /// Under Proton this is inside the prefix, not the host's ~/Documents.
    #[serde(default)]
    pub ac_documents_path: PathBuf,
}

// Serde default helpers
fn default_config_version() -> u32 {
    1
}
fn default_update_rate() -> u64 {
    16
}
fn default_history_size() -> usize {
    300
}
fn default_true() -> bool {
    true
}
fn default_pressure_unit() -> PressureUnit {
    PressureUnit::Psi
}
fn default_temp_unit() -> TempUnit {
    TempUnit::Celsius
}
fn default_shift_point_offset() -> u32 {
    200
}
fn default_fuel_safety_margin() -> f32 {
    1.0
}
fn default_target_tyre_pressure() -> f32 {
    27.5
}
fn default_target_hot_pressure_front() -> f32 {
    27.5
}
fn default_target_hot_pressure_rear() -> f32 {
    27.0
}
fn default_engineer_lines() -> u8 {
    4
}
fn default_data_path() -> PathBuf {
    app_dir()
}

/// The in-game overlay's sections.
///
/// These are the application's side of the decision. The Lua app has its own
/// switches for the same sections, and both have to agree before anything is
/// drawn: this one means "there is nothing worth showing", the app's means
/// "the driver does not want to see it".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    #[serde(default = "default_true")]
    pub show_telemetry: bool,
    #[serde(default = "default_true")]
    pub show_engineer: bool,
    #[serde(default = "default_true")]
    pub show_session: bool,
    #[serde(default = "default_true")]
    pub show_timing: bool,
    #[serde(default = "default_true")]
    pub show_fuel: bool,
    /// How many engineer lines reach the overlay at once, 0 to
    /// [`crate::overlay::frame::MESSAGE_SLOTS`].
    ///
    /// The default stays at four, which is what fits in the corner of a
    /// windscreen. Anyone with the advice window on a second monitor can ask
    /// for all eight.
    #[serde(default = "default_engineer_lines")]
    pub engineer_lines: u8,
    /// Show what was found and installed when the application starts.
    #[serde(default = "default_true")]
    pub startup_card: bool,
    /// The overlay has been offered once. Nobody wants to be asked twice.
    #[serde(default)]
    pub onboarding_done: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            show_telemetry: true,
            show_engineer: true,
            show_session: true,
            show_timing: true,
            show_fuel: true,
            engineer_lines: default_engineer_lines(),
            startup_card: true,
            onboarding_done: false,
        }
    }
}

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
    /// Life left below which a tyre is "going off", as a percentage.
    #[serde(default = "default_wear_warning")]
    pub wear_warning: f32,
    /// Life left below which it is a critical problem, as a percentage.
    ///
    /// Separate from [`Self::wear_warning`] because it used to be derived from
    /// it: anything under `wear_warning - 2` was reported as WORN OUT, so with
    /// the default of 96 a tyre at 93.9 % life — which is a tyre most of the
    /// way through a first stint — came back as a critical alert. Advice that
    /// cries wolf on lap three is advice nobody reads on lap thirty.
    #[serde(default = "default_wear_critical")]
    pub wear_critical: f32,
}

fn default_language() -> Language {
    Language::English
}
fn default_tyre_pressure_min() -> f32 {
    26.0
}
fn default_tyre_pressure_max() -> f32 {
    28.5
}
fn default_tyre_temp_min() -> f32 {
    70.0
}
fn default_tyre_temp_max() -> f32 {
    105.0
}
fn default_brake_temp_max() -> f32 {
    800.0
}
fn default_fuel_warning_laps() -> f32 {
    3.0
}
fn default_wear_warning() -> f32 {
    96.0
}
fn default_wear_critical() -> f32 {
    85.0
}

/// What each key in the terminal application does.
///
/// Strings rather than key codes, and in this crate rather than in `ac_tui`,
/// for two reasons. This crate does not depend on crossterm and should not
/// start — a config file is data, and turning that data into a `KeyCode` is
/// the terminal's business. And a config full of `"ctrl+s"` is a config
/// someone can edit in a text editor when the key they bound turns out to be
/// the one their terminal swallows.
///
/// The spelling is what `ac_tui::keys` parses and prints back: an optional
/// `ctrl+`, `shift+` or `alt+`, then a name (`f1`, `esc`, `tab`, `enter`,
/// `space`, `up`, `pgup`, `del`, …) or a single character. Case does not
/// matter. Anything unparseable leaves that action unbound rather than
/// crashing, and the Settings screen says so.
///
/// Every field is `#[serde(default)]`, so a config written before this existed
/// gains the defaults rather than failing to load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    #[serde(default = "key_help")]
    pub help: String,
    #[serde(default = "key_quit")]
    pub quit: String,
    #[serde(default = "key_overlay_toggle")]
    pub overlay_toggle: String,
    #[serde(default = "key_overlay_menu")]
    pub overlay_menu: String,
    #[serde(default = "key_screenshot")]
    pub screenshot: String,
    #[serde(default = "key_language")]
    pub language: String,
    #[serde(default = "key_next_tab")]
    pub next_tab: String,
    #[serde(default = "key_prev_tab")]
    pub prev_tab: String,

    #[serde(default = "key_tab_1")]
    pub tab_dashboard: String,
    #[serde(default = "key_tab_2")]
    pub tab_telemetry: String,
    #[serde(default = "key_tab_3")]
    pub tab_engineer: String,
    #[serde(default = "key_tab_4")]
    pub tab_setup: String,
    #[serde(default = "key_tab_5")]
    pub tab_analysis: String,
    #[serde(default = "key_tab_6")]
    pub tab_strategy: String,
    #[serde(default = "key_tab_7")]
    pub tab_ffb: String,
    #[serde(default = "key_tab_8")]
    pub tab_settings: String,
    #[serde(default = "key_tab_9")]
    pub tab_guide: String,

    #[serde(default = "key_analysis_save")]
    pub analysis_save: String,
    #[serde(default = "key_analysis_load")]
    pub analysis_load: String,
    #[serde(default = "key_analysis_compare")]
    pub analysis_compare: String,
    #[serde(default = "key_analysis_export")]
    pub analysis_export: String,

    #[serde(default = "key_setup_browser")]
    pub setup_browser: String,
    #[serde(default = "key_setup_download")]
    pub setup_download: String,
}

fn key_help() -> String {
    "f1".to_string()
}
fn key_quit() -> String {
    "esc".to_string()
}
fn key_overlay_toggle() -> String {
    "f10".to_string()
}
fn key_overlay_menu() -> String {
    "f11".to_string()
}
fn key_screenshot() -> String {
    "ctrl+s".to_string()
}
fn key_language() -> String {
    "ctrl+l".to_string()
}
fn key_next_tab() -> String {
    "tab".to_string()
}
fn key_prev_tab() -> String {
    "shift+tab".to_string()
}
fn key_tab_1() -> String {
    "1".to_string()
}
fn key_tab_2() -> String {
    "2".to_string()
}
fn key_tab_3() -> String {
    "3".to_string()
}
fn key_tab_4() -> String {
    "4".to_string()
}
fn key_tab_5() -> String {
    "5".to_string()
}
fn key_tab_6() -> String {
    "6".to_string()
}
fn key_tab_7() -> String {
    "7".to_string()
}
fn key_tab_8() -> String {
    "8".to_string()
}
fn key_tab_9() -> String {
    "9".to_string()
}
fn key_analysis_save() -> String {
    "s".to_string()
}
fn key_analysis_load() -> String {
    "l".to_string()
}
fn key_analysis_compare() -> String {
    "c".to_string()
}
fn key_analysis_export() -> String {
    "e".to_string()
}
fn key_setup_browser() -> String {
    "b".to_string()
}
fn key_setup_download() -> String {
    "d".to_string()
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            help: key_help(),
            quit: key_quit(),
            overlay_toggle: key_overlay_toggle(),
            overlay_menu: key_overlay_menu(),
            screenshot: key_screenshot(),
            language: key_language(),
            next_tab: key_next_tab(),
            prev_tab: key_prev_tab(),
            tab_dashboard: key_tab_1(),
            tab_telemetry: key_tab_2(),
            tab_engineer: key_tab_3(),
            tab_setup: key_tab_4(),
            tab_analysis: key_tab_5(),
            tab_strategy: key_tab_6(),
            tab_ffb: key_tab_7(),
            tab_settings: key_tab_8(),
            tab_guide: key_tab_9(),
            analysis_save: key_analysis_save(),
            analysis_load: key_analysis_load(),
            analysis_compare: key_analysis_compare(),
            analysis_export: key_analysis_export(),
            setup_browser: key_setup_browser(),
            setup_download: key_setup_download(),
        }
    }
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
            keys: KeyBindings::default(),
            data_path: PathBuf::from("./data"),
            ac_install_path: PathBuf::new(),
            ac_documents_path: PathBuf::new(),
            overlay: OverlayConfig::default(),
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
            wear_critical: 85.0,
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

    /// Convert a temperature *difference*.
    ///
    /// A delta is not a temperature: only the scale factor applies, never the
    /// +32 offset. A 10 °C spread is an 18 °F spread, not a 50 °F one. Passing
    /// a difference through [`Self::temp_val`] would be wrong by 32 degrees
    /// every time.
    pub fn temp_delta_val(&self, delta_c: f32) -> f32 {
        match self.temp_unit {
            TempUnit::Celsius => delta_c,
            TempUnit::Fahrenheit => delta_c * 1.8,
        }
    }

    /// Format a temperature difference in the configured unit.
    pub fn format_temp_delta(&self, delta_c: f32) -> String {
        format!("{:.0}{}", self.temp_delta_val(delta_c), self.temp_symbol())
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
        if self.data_path.as_os_str().is_empty() || self.data_path == *"./data" {
            app_dir()
        } else {
            self.data_path.clone()
        }
    }

    pub fn formatter(&self) -> UnitFormatter {
        UnitFormatter::new(self.pressure_unit, self.temp_unit)
    }

    /// Clamp a float into a plausible range, substituting `fallback` when it
    /// is not a number at all.
    ///
    /// `clamp` on its own returns NaN unchanged and panics if the bounds are
    /// themselves NaN, so a `null`-turned-NaN in the JSON would survive it.
    fn sane_value(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
        if value.is_finite() {
            value.clamp(min, max)
        } else {
            fallback
        }
    }

    /// Whether two configs would behave identically.
    ///
    /// `AppConfig` cannot derive `PartialEq` usefully — it is mostly floats,
    /// and the question here is not bit equality but whether a rewrite would
    /// change anything. Serialising both and comparing the text answers that
    /// without listing thirty fields by hand, and without caring how the file
    /// on disk happened to be formatted.
    fn matches(&self, other: &Self) -> bool {
        match (serde_json::to_string(self), serde_json::to_string(other)) {
            (Ok(a), Ok(b)) => a == b,
            // If either will not serialise, do not claim they match: the
            // safe direction is to rewrite.
            _ => false,
        }
    }

    /// The configured AC install path, or `None` when it is unset.
    pub fn ac_install_override(&self) -> Option<&std::path::Path> {
        (!self.ac_install_path.as_os_str().is_empty()).then_some(self.ac_install_path.as_path())
    }

    /// The configured AC Documents path, or `None` when it is unset.
    pub fn ac_documents_override(&self) -> Option<&std::path::Path> {
        (!self.ac_documents_path.as_os_str().is_empty()).then_some(self.ac_documents_path.as_path())
    }

    /// Load config from disk with migration and backup support.
    pub fn load() -> Result<Self, anyhow::Error> {
        let config_path = app_config_path();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).ok();
        }

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
        // Then clamp. `validate` existed but had no caller outside its own
        // unit test, so nothing on the load path ever checked the file's
        // numbers — including `update_rate`, which the loops sleep and poll
        // on.
        config.validate();

        // Write back only when this load actually changed something.
        //
        // The old test compared the file text against a re-serialisation and
        // rewrote on any difference — including whitespace, key order, and the
        // `unwrap_or_default()` empty string a serialisation failure produces,
        // which never equals the file. So the user's settings were rewritten
        // on essentially every launch, each one a fresh opportunity for a
        // half-written file.
        //
        // Comparing the parsed value against a re-parse of the file ignores
        // formatting and asks the only question that matters: would loading
        // this file again produce something different from what we hold?
        let needs_write = config.config_version != CONFIG_VERSION
            || serde_json::from_str::<AppConfig>(&content)
                .is_ok_and(|on_disk| !config.matches(&on_disk));

        if needs_write {
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
            info!(
                "Config migrated from v{} to v{}",
                self.config_version, CONFIG_VERSION
            );
        }
        self.config_version = CONFIG_VERSION;
    }

    /// Validate config values, clamping any out-of-range values.
    ///
    /// The settings UI clamps as it edits, but the file on disk is plain JSON
    /// that anyone can hand-edit, and a partial write can leave any field at
    /// zero. `update_rate` is the dangerous one: the render loop passes it
    /// straight to `event::poll` and the tick thread to `thread::sleep`, so a
    /// zero there spins two cores at 100%.
    pub fn validate(&mut self) {
        self.update_rate = self.update_rate.clamp(5, 1000);
        self.history_size = self.history_size.clamp(50, 10000);
        self.fuel_safety_margin = self.fuel_safety_margin.clamp(0.0, 10.0);
        self.alerts.fuel_warning_laps = self.alerts.fuel_warning_laps.clamp(0.5, 20.0);
        self.alerts.wear_warning = self.alerts.wear_warning.clamp(50.0, 100.0);
        self.alerts.brake_temp_max = self.alerts.brake_temp_max.clamp(200.0, 1200.0);

        // Pressure and temperature targets feed the engineer's recommendation
        // maths directly. A zero target makes every suggestion a nonsense
        // delta away from it; NaN propagates through the whole advice chain.
        self.shift_point_offset = self.shift_point_offset.clamp(0, 3000);
        self.target_tyre_pressure = Self::sane_value(self.target_tyre_pressure, 15.0, 45.0, 27.5);
        self.target_hot_pressure_front =
            Self::sane_value(self.target_hot_pressure_front, 15.0, 45.0, 27.5);
        self.target_hot_pressure_rear =
            Self::sane_value(self.target_hot_pressure_rear, 15.0, 45.0, 27.0);
        self.alerts.tyre_pressure_min =
            Self::sane_value(self.alerts.tyre_pressure_min, 15.0, 45.0, 26.0);
        self.alerts.tyre_pressure_max =
            Self::sane_value(self.alerts.tyre_pressure_max, 15.0, 45.0, 28.5);
        self.alerts.tyre_temp_min = Self::sane_value(self.alerts.tyre_temp_min, 0.0, 200.0, 70.0);
        self.alerts.tyre_temp_max = Self::sane_value(self.alerts.tyre_temp_max, 0.0, 200.0, 105.0);

        // An inverted band would make both the "too low" and "too high"
        // alerts fire on every reading at once.
        if self.alerts.tyre_pressure_min > self.alerts.tyre_pressure_max {
            std::mem::swap(
                &mut self.alerts.tyre_pressure_min,
                &mut self.alerts.tyre_pressure_max,
            );
        }
        if self.alerts.tyre_temp_min > self.alerts.tyre_temp_max {
            std::mem::swap(
                &mut self.alerts.tyre_temp_min,
                &mut self.alerts.tyre_temp_max,
            );
        }
    }

    /// Save config atomically: write to .tmp then rename.
    pub fn save(&self) -> Result<(), anyhow::Error> {
        let config_path = app_config_path();
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let content = serde_json::to_string_pretty(self)?;
        crate::atomic_file::write_atomic(&config_path, content.as_bytes())?;
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
        assert!(
            result.is_ok(),
            "Should handle unknown fields: {:?}",
            result.err()
        );
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

    /// The old test compared the file text against a re-serialisation, so any
    /// difference in whitespace or key order rewrote the user's settings at
    /// every launch.
    #[test]
    fn a_reformatted_config_is_not_rewritten() {
        let config = AppConfig::default();
        let pretty = serde_json::to_string_pretty(&config).expect("serialise");

        // Same values, different formatting: compact instead of pretty.
        let compact = serde_json::to_string(&config).expect("serialise");
        assert_ne!(pretty, compact, "the two encodings do differ textually");

        let from_pretty: AppConfig = serde_json::from_str(&pretty).expect("parse");
        let from_compact: AppConfig = serde_json::from_str(&compact).expect("parse");
        assert!(
            from_pretty.matches(&from_compact),
            "but they describe the same config, so no rewrite is needed"
        );
    }

    #[test]
    fn a_config_with_different_values_does_not_match() {
        let config = AppConfig::default();
        let changed = AppConfig {
            update_rate: 33,
            ..AppConfig::default()
        };
        assert!(!config.matches(&changed));
    }

    /// The case that *must* still write: a file whose values `validate`
    /// changed. Leaving that unwritten means clamping the same bad value on
    /// every launch and never telling the user their setting was rejected.
    #[test]
    fn a_config_that_validation_changed_is_rewritten() {
        let on_disk: AppConfig = serde_json::from_str(r#"{"update_rate": 0}"#).expect("parse");
        let mut validated = on_disk.clone();
        validated.validate();

        assert_eq!(validated.update_rate, 5, "clamped up from zero");
        assert!(
            !validated.matches(&on_disk),
            "so it differs from the file and gets written back"
        );
    }

    /// ...and the case that must not: a file already within range parses,
    /// validates to itself, and needs no write.
    #[test]
    fn a_valid_config_needs_no_write() {
        let on_disk: AppConfig = serde_json::from_str(r#"{"update_rate": 20}"#).expect("parse");
        let mut validated = on_disk.clone();
        validated.validate();

        assert!(validated.matches(&on_disk));
    }

    #[test]
    fn config_validate_clamps_values() {
        let mut config = AppConfig {
            update_rate: 0,
            history_size: 999999,
            fuel_safety_margin: -5.0,
            ..Default::default()
        };
        config.alerts.fuel_warning_laps = 100.0;

        config.validate();

        assert_eq!(config.update_rate, 5);
        assert_eq!(config.history_size, 10000);
        assert_eq!(config.fuel_safety_margin, 0.0);
        assert_eq!(config.alerts.fuel_warning_laps, 20.0);
    }

    /// A temperature difference converts by scale only. Passing it through
    /// `temp_val` would add 32 and report a 10 degree spread as 50.
    #[test]
    fn a_temperature_delta_converts_without_the_offset() {
        let fahrenheit = UnitFormatter::new(PressureUnit::Psi, TempUnit::Fahrenheit);
        assert_eq!(fahrenheit.temp_delta_val(10.0), 18.0);
        assert_eq!(fahrenheit.format_temp_delta(10.0), "18°F");
        // For contrast, the same number read as an absolute temperature.
        assert_eq!(fahrenheit.temp_val(10.0), 50.0);

        let celsius = UnitFormatter::new(PressureUnit::Psi, TempUnit::Celsius);
        assert_eq!(celsius.temp_delta_val(10.0), 10.0);
        assert_eq!(celsius.format_temp_delta(10.0), "10°C");
    }

    #[test]
    fn config_validate_clamps_pressure_and_temperature_targets() {
        let mut config = AppConfig {
            shift_point_offset: 50_000,
            target_tyre_pressure: 0.0,
            target_hot_pressure_front: 900.0,
            ..Default::default()
        };
        // Below the floor, and below the min it is paired with — so the
        // un-inverting swap below picks it up as well.
        config.alerts.tyre_temp_max = -40.0;

        config.validate();

        assert_eq!(config.shift_point_offset, 3000);
        assert_eq!(config.target_tyre_pressure, 15.0);
        assert_eq!(config.target_hot_pressure_front, 45.0);
        assert_eq!(
            config.alerts.tyre_temp_min, 0.0,
            "-40 clamps to the 0 floor, then swaps into the min slot"
        );
        assert_eq!(config.alerts.tyre_temp_max, 70.0);
    }

    /// A `null` in the JSON, or a float that came back from a partial write,
    /// arrives as NaN. `clamp` passes NaN straight through, so it needs its
    /// own branch.
    #[test]
    fn config_validate_replaces_nan_targets_with_defaults() {
        let mut config = AppConfig {
            target_tyre_pressure: f32::NAN,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.target_tyre_pressure, 27.5);
    }

    /// Both alert bands would otherwise fire at once on every reading.
    #[test]
    fn config_validate_uninverts_alert_bands() {
        let mut config = AppConfig::default();
        config.alerts.tyre_pressure_min = 30.0;
        config.alerts.tyre_pressure_max = 20.0;
        config.alerts.tyre_temp_min = 120.0;
        config.alerts.tyre_temp_max = 60.0;

        config.validate();

        assert!(config.alerts.tyre_pressure_min < config.alerts.tyre_pressure_max);
        assert!(config.alerts.tyre_temp_min < config.alerts.tyre_temp_max);
    }

    use std::fs::File;

    #[test]
    fn config_migration_v1_to_v2() {
        let mut config = AppConfig {
            config_version: 1,
            ..Default::default()
        };
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
        assert!(
            (bar_val - 1.896).abs() < 0.01,
            "Expected ~1.896 bar, got {}",
            bar_val
        );
        assert_eq!(fmt_bar.pressure_symbol(), "bar");
        assert_eq!(fmt_bar.format_pressure(27.5), "1.90 bar");
        assert!((fmt_bar.pressure_to_psi(bar_val) - 27.5).abs() < 0.001);

        let fmt_kpa = UnitFormatter::new(PressureUnit::Kpa, TempUnit::Celsius);
        let kpa_val = fmt_kpa.pressure_val(27.5);
        assert!(
            (kpa_val - 189.6).abs() < 0.1,
            "Expected ~189.6 kPa, got {}",
            kpa_val
        );
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
        let config = AppConfig {
            pressure_unit: PressureUnit::Bar,
            temp_unit: TempUnit::Fahrenheit,
            ..Default::default()
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let restored: AppConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.pressure_unit, PressureUnit::Bar);
        assert_eq!(restored.temp_unit, TempUnit::Fahrenheit);

        let fmt = restored.formatter();
        assert_eq!(fmt.format_pressure(27.5), "1.90 bar");
        assert_eq!(fmt.format_temp(100.0), "212°F");
    }
}
