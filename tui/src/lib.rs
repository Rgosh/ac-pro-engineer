pub mod platform;
pub mod ui;

use crate::ui::{UIRenderer, UIState};
use ac_core::ac_structs::{AcGraphics, AcPhysics, AcStatic};
use ac_core::analyzer::{AnalysisResult, TelemetryAnalyzer};
use ac_core::config::{AppConfig, Language};
use ac_core::content_manager::ContentManager;
use ac_core::discord::DiscordClient;
use ac_core::engineer::{Engineer, Recommendation};
use ac_core::memory::SharedMemory;
use ac_core::overlay::{OverlayManager, OverlayMode};
use ac_core::process::is_process_running;
use ac_core::records::RecordManager;
use ac_core::session_info::SessionInfo;
use ac_core::setup_manager::SetupManager;
use ac_core::updater::{UpdateStatus, Updater};

use clap::ValueEnum;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::metadata::LevelFilter;
use tracing::{error, info};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

pub fn setup_logging(
    file: Option<&PathBuf>,
    level: AppLogLevel,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = match file {
        Some(file) => file,
        None => &PathBuf::from("logs").with_file_name("ac_engineer.log"),
    };

    if let Some(parent) = file.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        error!(error = ?error, "Cannot create log directory");
    }

    let file = File::create(file)?;

    let debug_log = tracing_subscriber::fmt::layer()
        .with_writer(file)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_span_events(FmtSpan::ACTIVE)
        .with_ansi(false)
        .compact();

    tracing_subscriber::registry()
        .with(debug_log.with_filter(LevelFilter::from(level)))
        .init();

    info!("AC Pro Engineer v0.2.0 Logger Initialized");
    Ok(())
}

pub trait SafeLock<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> SafeLock<T> for Mutex<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T> {
        match self.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppTab {
    Dashboard,
    Telemetry,
    Engineer,
    Setup,
    Analysis,
    Strategy,
    Ffb,
    Settings,
    Guide,
}

impl AppTab {
    pub fn next(&self) -> Self {
        match self {
            AppTab::Dashboard => AppTab::Telemetry,
            AppTab::Telemetry => AppTab::Engineer,
            AppTab::Engineer => AppTab::Setup,
            AppTab::Setup => AppTab::Analysis,
            AppTab::Analysis => AppTab::Strategy,
            AppTab::Strategy => AppTab::Ffb,
            AppTab::Ffb => AppTab::Settings,
            AppTab::Settings => AppTab::Guide,
            AppTab::Guide => AppTab::Dashboard,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            AppTab::Dashboard => AppTab::Guide,
            AppTab::Guide => AppTab::Settings,
            AppTab::Settings => AppTab::Ffb,
            AppTab::Ffb => AppTab::Strategy,
            AppTab::Strategy => AppTab::Analysis,
            AppTab::Analysis => AppTab::Setup,
            AppTab::Setup => AppTab::Engineer,
            AppTab::Engineer => AppTab::Telemetry,
            AppTab::Telemetry => AppTab::Dashboard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppStage {
    Launcher,
    Running,
}

#[cfg(target_os = "windows")]
static SHM_MEM_DIR: &str = "Local\\";
#[cfg(not(target_os = "windows"))]
static SHM_MEM_DIR: &str = "/dev/shm/";

static SHM_MEM_PHYSICS: &str = "acpmf_physics";
static SHM_MEM_GRAPHICS: &str = "acpmf_graphics";
static SHM_MEM_STATIC: &str = "acpmf_static";

pub struct Memory {
    physics_mem: SharedMemory<AcPhysics>,
    graphics_mem: SharedMemory<AcGraphics>,
    static_mem: SharedMemory<AcStatic>,

    ac_physics: AcPhysics,
    ac_graphics: AcGraphics,
    ac_static: AcStatic,
}

impl Memory {
    pub fn try_connect() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            physics_mem: SharedMemory::<AcPhysics>::connect(&Self::get_mem(SHM_MEM_PHYSICS))?,
            graphics_mem: SharedMemory::<AcGraphics>::connect(&Self::get_mem(SHM_MEM_GRAPHICS))?,
            static_mem: SharedMemory::<AcStatic>::connect(&Self::get_mem(SHM_MEM_STATIC))?,
            ac_physics: AcPhysics::default(),
            ac_graphics: AcGraphics::default(),
            ac_static: AcStatic::default(),
        })
    }

    pub fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.ac_physics = self
            .physics_mem
            .get()
            .map_err(|e| anyhow::format_err!("Cannot read physics: {e:?}"))?;
        self.ac_graphics = self
            .graphics_mem
            .get()
            .map_err(|e| anyhow::format_err!("Cannot read graphics: {e:?}"))?;
        self.ac_static = self
            .static_mem
            .get()
            .map_err(|e| anyhow::format_err!("Cannot read static: {e:?}"))?;
        Ok(())
    }

    fn get_mem(name: &str) -> String {
        format!("{}{}", SHM_MEM_DIR, name)
    }

    pub fn physics(&self) -> &AcPhysics {
        &self.ac_physics
    }

    pub fn graphics(&self) -> &AcGraphics {
        &self.ac_graphics
    }

    pub fn stat(&self) -> &AcStatic {
        &self.ac_static
    }
}

pub struct AppState {
    pub mem: Option<Memory>,
    pub setup_manager: SetupManager,
    pub content_manager: ContentManager,
    pub record_manager: RecordManager,
    pub updater: Updater,
    pub discord: DiscordClient,
    pub engineer: Engineer,
    pub analyzer: TelemetryAnalyzer,
    pub ui_state: UIState,
    pub overlay_manager: OverlayManager,
    pub stage: AppStage,
    pub launcher_selection: usize,
    pub is_game_running: bool,
    pub is_connected: bool,
    pub active_tab: AppTab,
    pub session_info: SessionInfo,
    pub physics_history: Vec<AcPhysics>,
    pub graphics_history: Vec<AcGraphics>,
    pub current_lap_physics: Vec<AcPhysics>,
    pub current_lap_graphics: Vec<AcGraphics>,
    pub current_lap_number: i32,
    pub recommendations: Vec<Recommendation>,
    pub analysis_results: Vec<AnalysisResult>,
    pub last_update: Instant,
    pub config: AppConfig,
    pub show_update_success: bool,
    pub show_first_run_prompt: bool,
    pub first_run_selection: usize,
    pub mock_physics: Option<AcPhysics>,
    pub mock_graphics: Option<AcGraphics>,
    pub mock_static: Option<AcStatic>,
    pub is_demo_mode: bool,
    pub demo_tick_counter: u64,
    pub show_help: bool,
    pub show_overlay_menu: bool,
    pub overlay_menu_selection: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(OverlayMode::External)
    }
}

impl AppState {
    pub fn new(overlay_mode: OverlayMode) -> Self {
        let mut config = AppConfig::load().unwrap_or_default();
        let mut show_success = false;
        let is_first_run = config.last_run_version == "0.0.0" || config.last_run_version.is_empty();

        if config.last_run_version != ac_core::updater::CURRENT_VERSION {
            if !is_first_run {
                show_success = true;
            }
            config.last_run_version = ac_core::updater::CURRENT_VERSION.to_string();
            let _res = config.save();
        }

        let overlay_manager = OverlayManager::new(overlay_mode);

        Self {
            mem: None,
            mock_physics: None,
            mock_graphics: None,
            mock_static: None,
            is_demo_mode: false,
            demo_tick_counter: 0,
            setup_manager: SetupManager::new(),
            content_manager: ContentManager::new(),
            record_manager: RecordManager::new(),
            updater: Updater::new(),
            discord: DiscordClient::new(),
            engineer: Engineer::new(&config),
            analyzer: TelemetryAnalyzer::new(),
            ui_state: UIState::new(),
            overlay_manager,
            stage: AppStage::Launcher,
            launcher_selection: 0,
            is_game_running: false,
            is_connected: false,
            active_tab: AppTab::Dashboard,
            session_info: SessionInfo::default(),
            physics_history: Vec::with_capacity(300),
            graphics_history: Vec::with_capacity(300),
            current_lap_physics: Vec::with_capacity(10000),
            current_lap_graphics: Vec::with_capacity(10000),
            current_lap_number: -1,
            recommendations: Vec::new(),
            analysis_results: Vec::new(),
            last_update: Instant::now(),
            config,
            show_update_success: show_success,
            show_first_run_prompt: is_first_run,
            first_run_selection: 0,
            show_help: false,
            show_overlay_menu: false,
            overlay_menu_selection: 0,
        }
    }

    pub fn enable_demo_simulation(&mut self) {
        self.is_demo_mode = true;
        self.is_connected = true;
        self.is_game_running = true;

        let mut sess = SessionInfo::default();
        sess.car_name = "Ferrari SF70H".to_string();
        sess.track_name = "Autodromo Nazionale Monza".to_string();
        sess.track_config = "GP".to_string();
        sess.player_name = "Pro Sim Racer".to_string();
        sess.session_type = "Practice".to_string();
        sess.lap_count = 6;
        sess.session_time_left = 1800.0;
        sess.max_rpm = 12500;
        sess.max_fuel = 110.0;
        self.session_info = sess;

        let mut stat = AcStatic::default();
        stat.max_rpm = 12500;
        stat.max_fuel = 110.0;
        stat.car_model = ac_core::ac_structs::StringU16_33::from("ks_ferrari_sf70h");
        stat.track = ac_core::ac_structs::StringU16_33::from("monza");
        self.mock_static = Some(stat);

        self.update_demo_tick();
    }

    pub fn update_demo_tick(&mut self) {
        self.demo_tick_counter = self.demo_tick_counter.wrapping_add(1);
        let t = (self.demo_tick_counter as f32) * 0.05;

        let speed = 180.0 + (t * 1.5).sin() * 90.0;
        let rpm = (8500.0 + (t * 1.5).sin() * 3200.0) as i32;
        let gear = ((speed / 45.0) as i32).clamp(1, 7);
        let gas = (0.5 + (t * 1.2).cos() * 0.5).clamp(0.0, 1.0);
        let brake = if (t * 1.2).cos() < -0.4 { 0.75 } else { 0.0 };
        let steer = (t * 0.7).sin() * 0.35;
        let lat_g = (t * 0.7).sin() * 1.6;
        let lon_g = (t * 1.2).cos() * 1.3;

        let mut phys = AcPhysics::default();
        phys.speed_kmh = speed;
        phys.rpms = rpm;
        phys.gear = gear;
        phys.fuel = 34.2;
        phys.gas = gas;
        phys.brake = brake;
        phys.clutch = 0.0;
        phys.steer_angle = steer;
        phys.acc_g = [lat_g, 0.0, lon_g];
        phys.wheels_pressure = [27.4, 27.6, 27.5, 27.3];
        phys.tyre_temp_i = [89.2 + (t.sin() * 2.0), 88.0, 92.1, 90.5];
        phys.tyre_temp_m = [86.4 + (t.sin() * 2.0), 85.2, 89.0, 87.8];
        phys.tyre_temp_o = [82.1 + (t.sin() * 2.0), 81.0, 85.2, 84.0];
        phys.brake_temp = [450.0 + (t.cos() * 30.0), 442.0, 380.0, 375.0];
        phys.air_temp = 22.5;
        phys.road_temp = 34.0;
        phys.tc = 3.0;
        phys.abs = 2.0;
        self.mock_physics = Some(phys);

        let mut gfx = AcGraphics::default();
        gfx.surface_grip = 0.98;
        gfx.completed_laps = 5;
        gfx.i_current_time = ((t * 1000.0) as i32) % 81452;
        gfx.i_last_time = 81452;
        gfx.i_best_time = 81452;
        gfx.position = 2;
        gfx.fuel_x_lap = 2.85;
        self.mock_graphics = Some(gfx);

        self.physics_history.push(phys);
        if self.physics_history.len() > 300 {
            self.physics_history.remove(0);
        }
    }

    pub fn ac_graphics(&self) -> Option<&AcGraphics> {
        if let Some(ref mock) = self.mock_graphics {
            Some(mock)
        } else {
            self.mem.as_ref().map(|mem| &mem.ac_graphics)
        }
    }

    pub fn ac_physics(&self) -> Option<&AcPhysics> {
        if let Some(ref mock) = self.mock_physics {
            Some(mock)
        } else {
            self.mem.as_ref().map(|mem| &mem.ac_physics)
        }
    }

    pub fn ac_static(&self) -> Option<&AcStatic> {
        if let Some(ref mock) = self.mock_static {
            Some(mock)
        } else {
            self.mem.as_ref().map(|mem| &mem.ac_static)
        }
    }

    pub fn tick(&mut self) {
        self.ui_state.update_blink();
        let delta = self.engineer.stats.current_delta;
        self.discord
            .update(self.is_connected, &self.session_info, delta);

        if self.is_demo_mode {
            self.update_demo_tick();
            return;
        }

        if self.active_tab == AppTab::Setup {
            let mut tick = self.setup_manager.loading_tick.safe_lock();
            *tick = (*tick + 1) % 100;
        }

        if self.stage != AppStage::Running {
            return;
        }

        let process_active = is_process_running("acs.exe") || is_process_running("simulator.exe");
        self.is_game_running = process_active;

        if !process_active && self.is_connected {
            self.disconnect();
        } else if process_active
            && !self.is_connected
            && let Err(error) = self.connect_memory()
        {
            error!(error = ?error, "Cannot connect to shared memory");
        }

        if !self.is_connected {
            if self.overlay_manager.mode == OverlayMode::StandaloneTest {
                let s = &mut self.overlay_manager.state;
                s.speed_kmh = (s.speed_kmh + 1) % 320;
                s.rpm = (s.rpm + 75) % 9000;
                s.gear = (s.speed_kmh / 50) + 1;

                if s.rpm > 8000 {
                    s.engineer_messages = vec!["SHIFT UP NOW!".to_string()];
                } else if s.speed_kmh > 280 {
                    s.engineer_messages = vec!["HEAVY BRAKING AHEAD".to_string()];
                } else {
                    s.engineer_messages.clear();
                }
            }
            return;
        }

        let Some(mem) = self.mem.as_mut() else {
            return;
        };

        if let Err(error) = mem.refresh() {
            error!(error = ?error, "Cannot refresh memory");
            return;
        }

        let (phys, gfx, stat) = (mem.ac_physics, mem.ac_graphics, mem.ac_static);

        self.update_live_buffers(&phys, &gfx);
        self.update_session_info(&gfx);
        self.engineer.update(&phys, &gfx, &self.session_info);

        self.overlay_manager.update(&self.session_info);
        let s = &mut self.overlay_manager.state;
        s.speed_kmh = phys.speed_kmh as i32;
        s.gear = phys.gear - 1;
        s.rpm = phys.rpms;

        let completed_laps = gfx.completed_laps;
        if self.current_lap_number == -1 {
            self.current_lap_number = completed_laps;
        }

        if completed_laps > self.current_lap_number {
            let last_lap_time = gfx.i_last_time;
            if last_lap_time > 10000 && !self.current_lap_physics.is_empty() {
                self.analyzer.process_lap(
                    self.current_lap_number,
                    last_lap_time,
                    &self.current_lap_physics,
                    &self.current_lap_graphics,
                    self.session_info.car_name.clone(),
                    self.session_info.track_name.clone(),
                );

                if let Some(car_specs) = self
                    .content_manager
                    .get_car_specs(&self.session_info.car_name)
                {
                    let track_len = stat.track_spline_length;
                    let mut rec = self.record_manager.get_or_calculate_record(
                        &self.session_info.car_name,
                        &self.session_info.track_name,
                        &self.session_info.track_config,
                        Some(car_specs),
                        track_len,
                    );

                    if last_lap_time < rec.time_ms {
                        rec.time_ms = last_lap_time;
                        rec.source = "User Best".to_string();
                        self.record_manager.update_if_faster(rec.clone());
                    }
                    self.analyzer.set_world_record(rec);
                }
            }
            self.current_lap_physics.clear();
            self.current_lap_graphics.clear();
            self.current_lap_number = completed_laps;
        }

        if gfx.status != 0 && (phys.speed_kmh > 1.0 || phys.rpms > 1000) {
            self.current_lap_physics.push(phys);
            self.current_lap_graphics.push(gfx);
        }

        if !self.session_info.car_name.is_empty() && self.session_info.car_name != "-" {
            self.setup_manager
                .set_context(&self.session_info.car_name, &self.session_info.track_name);
        }
        let active_setup = self.setup_manager.get_active_setup();
        self.recommendations = self
            .engineer
            .analyze_live(&phys, &gfx, active_setup.as_ref());

        self.overlay_manager.state.engineer_messages = self
            .recommendations
            .iter()
            .map(|rec| rec.message.clone())
            .collect();
    }

    pub fn disconnect(&mut self) {
        self.mem = None;
        self.is_connected = false;
        self.session_info = SessionInfo::default();
        self.recommendations.clear();
    }

    pub fn connect_memory(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.mem.is_none() {
            let mut mem = Memory::try_connect()?;
            mem.refresh()?;

            let st = &mem.ac_static;
            self.session_info.car_name = st.car_model.to_string();
            self.session_info.track_name = st.track.to_string();
            self.session_info.track_config = st.track_configuration.to_string();
            self.session_info.player_name = st.player_nick.to_string();
            self.session_info.max_rpm = st.max_rpm;
            self.session_info.max_fuel = st.max_fuel;

            let specs = self
                .content_manager
                .get_car_specs(&self.session_info.car_name)
                .cloned();
            let rec = self.record_manager.get_or_calculate_record(
                &self.session_info.car_name,
                &self.session_info.track_name,
                &self.session_info.track_config,
                specs.as_ref(),
                st.track_spline_length,
            );
            self.analyzer.set_world_record(rec);
            self.is_connected = true;

            self.mem = Some(mem);
        }
        Ok(())
    }

    pub fn update_live_buffers(&mut self, phys: &AcPhysics, gfx: &AcGraphics) {
        if self.physics_history.len() >= 300 {
            self.physics_history.remove(0);
        }
        if self.graphics_history.len() >= 300 {
            self.graphics_history.remove(0);
        }
        self.physics_history.push(*phys);
        self.graphics_history.push(*gfx);
    }

    pub fn update_session_info(&mut self, gfx: &AcGraphics) {
        self.session_info.lap_count = gfx.completed_laps;
        self.session_info.session_time_left = gfx.session_time_left;
        self.session_info.session_type = match gfx.session {
            0 => "Booking".to_string(),
            1 => "Practice".to_string(),
            2 => "Qualifying".to_string(),
            3 => "Race".to_string(),
            4 => "Hotlap".to_string(),
            5 => "Time Attack".to_string(),
            6 => "Drift".to_string(),
            7 => "Drag".to_string(),
            _ => "Unknown".to_string(),
        };
    }
}

#[derive(Debug, Default, Clone, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum AppLogLevel {
    Trace,
    #[cfg_attr(debug_assertions, default)]
    Debug,
    #[cfg_attr(not(debug_assertions), default)]
    Info,
    Warn,
    Error,
}

impl From<AppLogLevel> for tracing::metadata::LevelFilter {
    fn from(value: AppLogLevel) -> Self {
        match value {
            AppLogLevel::Trace => Self::TRACE,
            AppLogLevel::Debug => Self::DEBUG,
            AppLogLevel::Info => Self::INFO,
            AppLogLevel::Warn => Self::WARN,
            AppLogLevel::Error => Self::ERROR,
        }
    }
}
