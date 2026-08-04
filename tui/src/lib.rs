pub mod platform;
pub mod ui;

use crate::ui::UIState;
use ac_core::RingBuffer;
use ac_core::ac_structs::{AcGraphics, AcPhysics, AcStatic};
use ac_core::analyzer::{AnalysisResult, TelemetryAnalyzer};
use ac_core::config::AppConfig;
use ac_core::content_manager::ContentManager;
use ac_core::discord::DiscordClient;
use ac_core::engineer::{Engineer, Recommendation};
use ac_core::memory::SharedMemory;
use ac_core::overlay::{OverlayManager, OverlayMode};
use ac_core::process::ProcessWatcher;
use ac_core::records::RecordManager;
use ac_core::session_info::SessionInfo;
use ac_core::setup_manager::SetupManager;
use ac_core::updater::Updater;

use clap::ValueEnum;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tracing::metadata::LevelFilter;
use tracing::{error, info};
use tracing_subscriber::Layer;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Create a log file, making its directory first.
fn open_log_file(path: &PathBuf) -> Result<File, std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    File::create(path)
}

pub fn setup_logging(
    file: Option<&PathBuf>,
    level: AppLogLevel,
) -> Result<(), Box<dyn std::error::Error>> {
    // The app data directory first: "logs" relative to the working directory
    // is not writable when the app is launched from a shortcut or installed
    // under Program Files, and a failure here used to abort startup before
    // the TUI was ever drawn.
    let default_path = ac_core::config::app_dir()
        .join("logs")
        .join("ac_engineer.log");
    let fallback_path = PathBuf::from("logs").join("ac_engineer.log");

    let file = match file {
        Some(explicit) => open_log_file(explicit)?,
        None => match open_log_file(&default_path) {
            Ok(file) => file,
            Err(error) => {
                eprintln!(
                    "Could not open {}: {error}. Falling back to {}.",
                    default_path.display(),
                    fallback_path.display()
                );
                open_log_file(&fallback_path)?
            }
        },
    };

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

    info!(
        "AC Pro Engineer v{} Logger Initialized",
        ac_core::updater::CURRENT_VERSION
    );
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
        // These two are rewritten by the game while we read them, so they go
        // through the tear-checking read. The static page is written once at
        // session load and does not need it.
        self.ac_physics = self
            .physics_mem
            .get_stable()
            .map_err(|e| anyhow::format_err!("Cannot read physics: {e:?}"))?;
        self.ac_graphics = self
            .graphics_mem
            .get_stable()
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

/// Sector count to assume until AcStatic says otherwise. AC's own tracks are
/// almost all three-sector.
pub const DEFAULT_SECTOR_COUNT: i32 = 3;

/// Frame and tick timing, for the footer readout.
#[derive(Debug, Clone, Copy)]
pub struct PerfStats {
    /// Duration of the last completed render.
    pub frame_time: std::time::Duration,
    /// When the background tick last finished.
    pub last_tick: Instant,
}

impl Default for PerfStats {
    fn default() -> Self {
        Self {
            frame_time: std::time::Duration::ZERO,
            last_tick: Instant::now(),
        }
    }
}

impl PerfStats {
    /// Frames per second implied by the last frame's duration.
    ///
    /// Zero while nothing has been drawn yet, rather than dividing by zero and
    /// reporting infinity.
    pub fn fps(&self) -> f32 {
        let secs = self.frame_time.as_secs_f32();
        if secs > 0.0 { 1.0 / secs } else { 0.0 }
    }

    /// How long since the background tick last completed.
    pub fn tick_age(&self) -> std::time::Duration {
        self.last_tick.elapsed()
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
    /// Publishes frames to the in-game Lua overlay, when the shared block
    /// could be opened. `None` is not worth stopping for — the overlay simply
    /// never appears, and everything else works.
    pub overlay_writer: Option<ac_core::overlay::shared_writer::OverlayWriter>,
    pub stage: AppStage,
    pub launcher_selection: usize,
    pub is_game_running: bool,
    /// Cached "is AC (or the simulator) running" check. The uncached scan
    /// reads every process on the system, and the launcher used to ask twice
    /// per frame.
    pub game_watcher: ProcessWatcher,
    pub is_connected: bool,
    pub active_tab: AppTab,
    pub session_info: SessionInfo,
    pub physics_history: RingBuffer<AcPhysics>,
    pub graphics_history: RingBuffer<AcGraphics>,
    pub current_lap_physics: Vec<AcPhysics>,
    pub current_lap_graphics: Vec<AcGraphics>,
    pub current_lap_number: i32,
    pub current_lap_sectors: [i32; 3],
    pub last_sector_index: i32,
    /// How many sectors this track publishes, from AcStatic. Not every track
    /// runs three — mods use two or four — and assuming three left the extra
    /// slots permanently zero, which `theoretical_best_lap_ms` reads as "no
    /// data" and so never produced a result on those tracks.
    pub track_sector_count: i32,
    pub recommendations: Vec<Recommendation>,
    pub analysis_results: Vec<AnalysisResult>,
    pub last_update: Instant,
    pub config: AppConfig,
    pub show_update_success: bool,
    pub show_first_run_prompt: bool,
    pub first_run_selection: usize,

    /// The overlay install card shown at startup: what was found in the game
    /// folder, and what the last install attempt did.
    pub show_overlay_card: bool,
    pub overlay_card_selection: usize,
    pub overlay_report: ac_core::overlay::install::InstallReport,
    pub overlay_install_status: String,
    pub mock_physics: Option<AcPhysics>,
    pub mock_graphics: Option<AcGraphics>,
    pub mock_static: Option<AcStatic>,
    pub is_demo_mode: bool,
    pub demo_tick_counter: u64,
    pub show_help: bool,
    pub show_overlay_menu: bool,
    /// How long the last rendered frame took, and how long ago the background
    /// tick last completed.
    ///
    /// The render loop and the tick thread contend for the same state mutex,
    /// so when one stalls it is usually because the other is holding the lock.
    /// Neither was observable, which made that a guess.
    pub perf: PerfStats,
    pub overlay_menu_selection: usize,
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

        let setup_manager = SetupManager::new();
        setup_manager.set_documents_override(&config.ac_documents_path);

        let mut state = Self {
            mem: None,
            mock_physics: None,
            mock_graphics: None,
            mock_static: None,
            is_demo_mode: false,
            demo_tick_counter: 0,
            setup_manager,
            content_manager: ContentManager::with_root_override(config.ac_install_override()),
            record_manager: RecordManager::new(),
            updater: Updater::new(),
            discord: DiscordClient::new(),
            engineer: Engineer::new(&config),
            analyzer: TelemetryAnalyzer::new(),
            ui_state: UIState::new(),
            overlay_manager,
            // Installed from the binary that writes the struct it reads, so
            // the two can never be out of step. Cheap and idempotent: it only
            // writes when the files differ.
            overlay_writer: {
                ac_core::overlay::install::install_on_startup(config.ac_install_override());
                match ac_core::overlay::shared_writer::OverlayWriter::open() {
                    Ok(writer) => Some(writer),
                    Err(error) => {
                        info!(error = ?error, "In-game overlay unavailable");
                        None
                    }
                }
            },
            stage: AppStage::Launcher,
            launcher_selection: 0,
            is_game_running: false,
            game_watcher: ProcessWatcher::new(&["acs.exe", "simulator.exe"]),
            is_connected: false,
            active_tab: AppTab::Dashboard,
            session_info: SessionInfo::default(),
            physics_history: RingBuffer::new(config.history_size),
            graphics_history: RingBuffer::new(config.history_size),
            current_lap_physics: Vec::with_capacity(36000),
            current_lap_graphics: Vec::with_capacity(36000),
            current_lap_number: -1,
            current_lap_sectors: [0; 3],
            last_sector_index: 0,
            track_sector_count: DEFAULT_SECTOR_COUNT,
            recommendations: Vec::new(),
            analysis_results: Vec::new(),
            last_update: Instant::now(),
            config,
            show_update_success: show_success,
            show_first_run_prompt: is_first_run,
            first_run_selection: 0,
            show_overlay_card: false,
            overlay_card_selection: 0,
            overlay_report: ac_core::overlay::install::InstallReport {
                game_root: None,
                app_path: None,
                current: false,
                csp_present: false,
            },
            overlay_install_status: String::new(),
            show_help: false,
            show_overlay_menu: false,
            perf: PerfStats::default(),
            overlay_menu_selection: 0,
        };

        state.refresh_overlay_report();
        state.show_overlay_card = state.config.overlay.startup_card;
        state
    }

    /// Look at the game folder again and remember what is there.
    pub fn refresh_overlay_report(&mut self) {
        self.overlay_report =
            ac_core::overlay::install::describe(self.config.ac_install_override());
    }

    /// Write the overlay app into the game folder now, and say what happened.
    ///
    /// The application already does this at startup; this is the button for
    /// when the game moved, the files were deleted, or the panel is being
    /// installed for a copy of AC that was not there at launch.
    pub fn install_overlay_now(&mut self) {
        use ac_core::overlay::install::{InstallOutcome, install};

        self.overlay_install_status = match install(self.config.ac_install_override()) {
            Ok(InstallOutcome::Installed { updated }) => {
                format!("installed, {updated} file(s) written")
            }
            Ok(InstallOutcome::AlreadyCurrent) => "already up to date".to_string(),
            Ok(InstallOutcome::NoGameFound) => {
                "no Assetto Corsa found — set the path in Settings".to_string()
            }
            Err(error) => format!("failed: {error}"),
        };
        self.refresh_overlay_report();
    }

    pub fn enable_demo_simulation(&mut self) {
        self.is_demo_mode = true;
        self.is_connected = true;
        self.is_game_running = true;

        self.session_info = SessionInfo {
            car_name: "Ferrari SF70H".to_string(),
            track_name: "Autodromo Nazionale Monza".to_string(),
            track_config: "GP".to_string(),
            player_name: "Pro Sim Racer".to_string(),
            session_type: "Practice".to_string(),
            lap_count: 6,
            session_time_left: 1_800_000.0,
            max_rpm: 12500,
            max_fuel: 110.0,
        };

        self.mock_static = Some(AcStatic {
            max_rpm: 12500,
            max_fuel: 110.0,
            car_model: ac_core::ac_structs::StringU16_33::from("ks_ferrari_sf70h"),
            track: ac_core::ac_structs::StringU16_33::from("monza"),
            ..Default::default()
        });

        if self.analyzer.laps.is_empty() {
            let mut trace_points = Vec::with_capacity(300);
            for i in 0..300 {
                let t = i as f32 * 0.05;
                let spd = 180.0 + (t * 2.0).sin() * 70.0;
                let px = (t * 0.8).cos() * 150.0;
                let py = (t * 0.8).sin() * 80.0;
                trace_points.push(ac_core::analyzer::TelemetryPoint {
                    rpms: 4000,
                    time_ms: i * 50,
                    distance: i as f32 * 10.0,
                    speed: spd,
                    gas: 0.9,
                    brake: 0.0,
                    steer: (t * 0.8).sin() * 0.4,
                    gear: 6,
                    lat_g: (t * 0.8).sin() * 1.5,
                    lon_g: (t * 1.5).cos() * 1.2,
                    x: px,
                    y: py,
                    slip_avg: 0.02,
                });
            }

            let mock_lap = ac_core::analyzer::LapData {
                lap_number: 5,
                lap_time_ms: 81452,
                sectors: [24120, 28350, 28982],
                valid: true,
                car_model: "Ferrari SF70H".to_string(),
                track_name: "Autodromo Nazionale Monza".to_string(),
                save_date: "2026-07-31".to_string(),
                from_file: false,
                air_temp: 22.5,
                road_temp: 34.0,
                track_grip: 98.0,
                timestamp: "14:32:05".to_string(),
                max_speed: 342.5,
                avg_speed: 254.2,
                avg_pressure: Some(27.4),
                min_corner_speed_avg: 78.5,
                fuel_used: 2.85,
                gear_shifts: 42,
                peak_lat_g: 2.45,
                peak_brake_g: 4.85,
                avg_tyre_temp: [88.5, 87.2, 91.0, 89.4],
                max_brake_temp: [580.0, 565.0, 490.0, 485.0],
                pressure_deviation: Some(0.15),
                suspension_travel_hist: [12.4, 11.8, 14.2, 13.9],
                avg_wheels_pressure: [27.4, 27.6, 27.5, 27.3],
                avg_tyre_temp_i: [89.2, 88.0, 92.1, 90.5],
                avg_tyre_temp_m: [86.4, 85.2, 89.0, 87.8],
                avg_tyre_temp_o: [82.1, 81.0, 85.2, 84.0],
                avg_brake_temp: [450.0, 442.0, 380.0, 375.0],
                // Metres, as AC publishes them. The renderer scales to mm.
                avg_ride_height: [0.025, 0.055],
                damper_histograms: [[25.0, 35.0, 20.0, 20.0]; 4],
                throttle_smoothness: 94.2,
                steering_smoothness: 91.8,
                trail_braking_score: 88.4,
                coasting_percent: 4.2,
                pedal_overlap_percent: 1.1,
                full_throttle_percent: 68.5,
                grip_usage_percent: 94.8,
                oversteer_count: 1,
                understeer_count: 2,
                lockup_count: 0,
                car_control_score: 95.0,
                scrubbing_incidents: 0,
                max_steering_over_rotation: 0.0,
                radar_stats: ac_core::analyzer::RadarStats {
                    consistency: 94.0,
                    car_control: 95.0,
                    aggression: 88.0,
                    smoothness: 93.0,
                    tyre_mgmt: 91.0,
                },
                telemetry_trace: trace_points,
                bounds_min_x: -160.0,
                bounds_max_x: 160.0,
                bounds_min_y: -90.0,
                bounds_max_y: 90.0,
            };

            self.analyzer.laps.push(mock_lap);
            self.analyzer.best_lap_index = Some(0);
        }

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

        self.mock_physics = Some(AcPhysics {
            speed_kmh: speed,
            rpms: rpm,
            gear,
            fuel: 34.2,
            gas,
            brake,
            clutch: 0.0,
            steer_angle: steer,
            acc_g: [lat_g, 0.0, lon_g],
            wheels_pressure: [27.4, 27.6, 27.5, 27.3],
            tyre_temp_i: [89.2 + (t.sin() * 2.0), 88.0, 92.1, 90.5],
            tyre_temp_m: [86.4 + (t.sin() * 2.0), 85.2, 89.0, 87.8],
            tyre_temp_o: [82.1 + (t.sin() * 2.0), 81.0, 85.2, 84.0],
            brake_temp: [450.0 + (t.cos() * 30.0), 442.0, 380.0, 375.0],
            air_temp: 22.5,
            road_temp: 34.0,
            tc: 3.0,
            abs: 2.0,
            ..Default::default()
        });

        self.mock_graphics = Some(AcGraphics {
            surface_grip: 0.98,
            completed_laps: 5,
            i_current_time: ((t * 1000.0) as i32) % 81452,
            i_last_time: 81452,
            i_best_time: 81452,
            position: 2,
            fuel_x_lap: 2.85,
            ..Default::default()
        });
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

    pub fn process_tick_logic(&mut self, phys: AcPhysics, gfx: AcGraphics, stat: AcStatic) {
        let stat_spline_length = stat.track_spline_length;
        // AcStatic::sector_count was read by nothing, so every track was
        // treated as three-sector.
        if stat.sector_count > 0 && stat.sector_count as usize <= self.current_lap_sectors.len() {
            self.track_sector_count = stat.sector_count;
        }

        self.update_live_buffers(&phys, &gfx);
        self.update_session_info(&gfx);
        self.engineer.update_config(&self.config);
        self.engineer.update(&phys, &gfx, &self.session_info);

        // The engineer sets `current_delta` from AC's own performance meter,
        // which is measured against whatever reference the game picked. With
        // the ghost delta enabled, compare against our own recorded best lap
        // instead — `calculate_ghost_delta` existed for this and had no caller
        // outside its unit test, which is also why the Settings toggle did
        // nothing at all.
        if self.config.show_ghost_delta
            && let Some(best) = self
                .analyzer
                .best_lap_index
                .and_then(|i| self.analyzer.laps.get(i))
            && let Some(delta) = ac_core::analyzer::calculate_ghost_delta(
                best,
                gfx.normalized_car_position,
                gfx.i_current_time as f32 / 1000.0,
            )
        {
            self.engineer.stats.current_delta = delta;
        }

        self.overlay_manager.update(&self.session_info);
        self.publish_overlay_frame(&phys, &gfx);

        // Sector splits are captured on the transition *out* of a sector,
        // when AC publishes the one just finished in `last_sector_time`. The
        // final sector is the exception: its transition is the lap rollover,
        // which races the `completed_laps` increment handled below. Whichever
        // AC publishes first decides whether the last split lands in this lap
        // or the next one, so it is derived from the lap time at lap close
        // instead — see `close_current_lap_sectors`.
        let current_sector = gfx.current_sector_index;
        if current_sector != self.last_sector_index {
            let finished = self.last_sector_index;
            let is_final_sector = finished == self.track_sector_count - 1;
            if finished >= 0
                && (finished as usize) < self.current_lap_sectors.len()
                && !is_final_sector
            {
                self.current_lap_sectors[finished as usize] = gfx.last_sector_time;
            }
            self.last_sector_index = current_sector;
        }

        let s = &mut self.overlay_manager.state;
        s.speed_kmh = phys.speed_kmh as i32;
        s.gear = (phys.gear - 1).max(0);
        s.rpm = phys.rpms;

        let completed_laps = gfx.completed_laps;
        if self.current_lap_number == -1 {
            self.current_lap_number = completed_laps;
        }

        if completed_laps != self.current_lap_number {
            if completed_laps == self.current_lap_number + 1 {
                let last_lap_time = gfx.i_last_time;
                if last_lap_time > 10000 && !self.current_lap_physics.is_empty() {
                    self.close_current_lap_sectors(last_lap_time);
                    self.analyzer.process_lap(
                        self.current_lap_number,
                        last_lap_time,
                        &self.current_lap_physics,
                        &self.current_lap_graphics,
                        self.current_lap_sectors,
                        self.session_info.car_name.clone(),
                        self.session_info.track_name.clone(),
                        self.config.target_tyre_pressure,
                        self.config.update_rate,
                    );

                    // Car specs sharpen the *estimated* reference time, but
                    // they are an enrichment, not a precondition. This whole
                    // block used to be nested inside `if let Some(car_specs)`,
                    // so on any machine where the AC install could not be
                    // found — every Linux machine, before ac_paths — no
                    // record was ever created, compared or saved, and the
                    // analyzer's world record stayed None, silently disabling
                    // the off-pace advice as well.
                    let car_specs = self
                        .content_manager
                        .get_car_specs(&self.session_info.car_name);
                    let reference = self.record_manager.get_or_calculate_record(
                        &self.session_info.car_name,
                        &self.session_info.track_name,
                        &self.session_info.track_config,
                        car_specs,
                        stat_spline_length,
                    );

                    // The driver's own best is tracked against their own
                    // history, not against the world record. Comparing to the
                    // WR meant `records.json` only ever gained an entry from
                    // someone who had beaten it, so for every normal driver
                    // the personal best was never saved at all.
                    let mut personal = reference.clone();
                    personal.time_ms = last_lap_time;
                    personal.source = "User Best".to_string();
                    self.record_manager.update_if_faster(personal);

                    self.analyzer.set_world_record(reference);
                }
            }
            self.current_lap_physics.clear();
            self.current_lap_graphics.clear();
            self.current_lap_sectors = [0; 3];
            self.current_lap_number = completed_laps;
        }

        if (gfx.status != 0 || self.is_demo_mode)
            && (phys.speed_kmh > 1.0 || phys.rpms > 1000)
            && self.current_lap_physics.len() < 36000
        {
            self.current_lap_physics.push(phys);
            self.current_lap_graphics.push(gfx);
        }

        if !self.session_info.car_name.is_empty() && self.session_info.car_name != "-" {
            self.setup_manager
                .set_context(&self.session_info.car_name, &self.session_info.track_name);
            self.setup_manager.detect_current(
                phys.fuel,
                phys.brake_bias / 100.0,
                &phys.wheels_pressure,
                &phys.tyre_temp_m,
            );
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

    pub fn tick(&mut self) {
        self.ui_state.update_blink();
        self.ui_state.analysis.tick_status();
        let delta = self.engineer.stats.current_delta;
        self.discord
            .update(self.is_connected, &self.session_info, delta);

        if self.is_demo_mode {
            self.update_demo_tick();
            if let (Some(phys), Some(gfx), Some(stat)) =
                (self.mock_physics, self.mock_graphics, self.mock_static)
            {
                self.process_tick_logic(phys, gfx, stat);
            }
            return;
        }

        if self.active_tab == AppTab::Setup {
            let mut tick = self.setup_manager.loading_tick.safe_lock();
            *tick = (*tick + 1) % 100;
        }

        // Kept above the early return so the launcher can read
        // `is_game_running` rather than running its own scan on every frame.
        let process_active = self.game_watcher.is_running();
        self.is_game_running = process_active;

        if self.stage != AppStage::Running {
            return;
        }

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

        self.process_tick_logic(phys, gfx, stat);
    }

    /// Fill in the final sector split from the lap time.
    ///
    /// The earlier splits come from AC's `last_sector_time` on each sector
    /// transition, but the final one's transition *is* the lap rollover — the
    /// same tick that increments `completed_laps`. Which of the two AC
    /// publishes first is not guaranteed, and when the lap count won the race
    /// the split was written after this lap had already been processed and
    /// its array cleared, so it landed in the *next* lap instead. The symptom
    /// was an occasional lap with a zero final sector and a following lap
    /// whose splits did not add up.
    ///
    /// The lap time minus the sectors already known is not subject to that
    /// ordering at all, so it is used instead. Left at zero if the earlier
    /// splits are missing or do not leave a plausible remainder — a wrong
    /// split is worse than a missing one, since `theoretical_best_lap_ms`
    /// would take it as a personal best.
    fn close_current_lap_sectors(&mut self, lap_time_ms: i32) {
        let final_idx = (self.track_sector_count - 1).max(0) as usize;
        if final_idx == 0 || final_idx >= self.current_lap_sectors.len() {
            return;
        }

        let earlier: i32 = self.current_lap_sectors[..final_idx].iter().sum();
        if self.current_lap_sectors[..final_idx]
            .iter()
            .any(|s| *s <= 0)
        {
            return;
        }

        let remainder = lap_time_ms - earlier;
        if remainder > ac_core::analyzer::MIN_VALID_SECTOR_MS && remainder < lap_time_ms {
            self.current_lap_sectors[final_idx] = remainder;
        }
    }

    /// Pack the current state into an overlay frame and publish it.
    ///
    /// Everything here is a copy of an already-computed value: the overlay
    /// draws on AC's render thread, so no work that can be done on this side
    /// belongs on that one.
    fn publish_overlay_frame(&mut self, phys: &AcPhysics, gfx: &AcGraphics) {
        use ac_core::overlay::frame::{OverlayFrame, flags};

        let Some(writer) = self.overlay_writer.as_mut() else {
            return;
        };

        let mut frame = OverlayFrame::empty();

        frame.speed_kmh = phys.speed_kmh;
        frame.rpm = phys.rpms;
        // AC encodes reverse as 0 and neutral as 1. Translated here so the
        // overlay does not have to know that.
        frame.gear = phys.gear - 1;
        frame.fuel_litres = phys.fuel;
        frame.air_temp_c = phys.air_temp;
        frame.road_temp_c = phys.road_temp;
        frame.surface_grip = gfx.surface_grip;

        frame.tyre_pressure_psi = phys.wheels_pressure;
        frame.tyre_wear_percent = phys.tyre_wear;
        frame.brake_temp_c = phys.brake_temp;
        for i in 0..4 {
            frame.tyre_temp_c[i] =
                (phys.tyre_temp_i[i] + phys.tyre_temp_m[i] + phys.tyre_temp_o[i]) / 3.0;
        }

        frame.last_lap_ms = gfx.i_last_time;
        frame.best_lap_ms = gfx.i_best_time;
        frame.current_lap_ms = gfx.i_current_time;
        frame.position = gfx.position;

        frame.fuel_laps_remaining = self.engineer.stats.fuel_laps_remaining;
        frame.fuel_per_lap = self.engineer.stats.fuel_consumption_rate;
        frame.delta_seconds = self.engineer.stats.current_delta;

        frame.apply_session(&self.session_info);

        frame.set_flag(flags::PIT_LIMITER, phys.pit_limiter_on != 0);
        frame.set_flag(flags::CONNECTED, self.is_connected);
        // Both sides have to agree: the overlay manager knows whether there is
        // anything to show right now, the config carries what the driver asked
        // for in the Settings tab.
        frame.set_flag(
            flags::SHOW_TELEMETRY,
            self.overlay_manager.state.show_telemetry && self.config.overlay.show_telemetry,
        );
        frame.set_flag(
            flags::SHOW_ENGINEER,
            self.overlay_manager.state.show_engineer && self.config.overlay.show_engineer,
        );
        frame.set_flag(flags::SHOW_SESSION, self.config.overlay.show_session);
        frame.set_flag(flags::SHOW_TIMING, self.config.overlay.show_timing);
        frame.set_flag(
            flags::RUSSIAN,
            self.config.language == ac_core::config::Language::Russian,
        );
        frame.set_flag(flags::SHOW_FUEL, self.config.overlay.show_fuel);
        frame.set_flag(
            flags::FUEL_WARNING,
            self.engineer.stats.fuel_laps_remaining > 0.0
                && self.engineer.stats.fuel_laps_remaining < self.config.alerts.fuel_warning_laps,
        );

        // Capped here rather than in the panel: the number is a setting on
        // this side, and publishing four lines to draw one is work the render
        // thread does not need to do.
        frame.set_messages_capped(
            &self.recommendations,
            self.config.overlay.engineer_lines as usize,
        );

        writer.publish(&frame);
    }

    /// Tell the overlay we are going away, so it hides at once rather than
    /// holding the last frame until its liveness timeout expires.
    pub fn shutdown_overlay(&mut self) {
        if let Some(writer) = self.overlay_writer.as_mut() {
            writer.publish_shutdown();
        }
    }

    pub fn disconnect(&mut self) {
        self.mem = None;
        self.is_connected = false;
        self.session_info = SessionInfo::default();
        self.recommendations.clear();
        self.physics_history.clear();
        self.graphics_history.clear();
        self.current_lap_physics.clear();
        self.current_lap_graphics.clear();
        self.current_lap_number = -1;
        // These were left behind on disconnect, so the first sector
        // transition after reconnecting was measured against the previous
        // session's state.
        self.current_lap_sectors = [0; 3];
        self.last_sector_index = -1;
        self.track_sector_count = DEFAULT_SECTOR_COUNT;
        // Fuel burn measured in the previous session says nothing about the
        // next one.
        self.engineer.reset_fuel_tracking();
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

    pub fn apply_config(&mut self) {
        let cap = self.config.history_size;
        self.physics_history.set_capacity(cap);
        self.graphics_history.set_capacity(cap);
        self.engineer.update_config(&self.config);
    }

    pub fn update_live_buffers(&mut self, phys: &AcPhysics, gfx: &AcGraphics) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_mode_executes_full_telemetry_pipeline() {
        let mut app = AppState::new(OverlayMode::External);
        app.is_demo_mode = true;
        app.mock_static = Some(ac_core::ac_structs::AcStatic::default());
        app.stage = AppStage::Running;

        assert_eq!(app.physics_history.len(), 0);

        for _ in 0..10 {
            app.tick();
        }

        assert_eq!(app.physics_history.len(), 10);
        assert_eq!(app.graphics_history.len(), 10);
        assert!(app.overlay_manager.state.speed_kmh > 0);
        assert_ne!(app.session_info.car_name, "");
        assert_ne!(app.session_info.track_name, "");
    }

    /// The final split is derived from the lap time rather than read off the
    /// sector transition, because that transition races the lap-count
    /// increment and could land the split in the following lap.
    #[test]
    fn final_sector_is_derived_from_the_lap_time() {
        let mut app = AppState::new(OverlayMode::External);
        app.track_sector_count = 3;
        app.current_lap_sectors = [30_000, 35_000, 0];

        app.close_current_lap_sectors(95_500);

        assert_eq!(
            app.current_lap_sectors[2], 30_500,
            "the remainder of the lap time is the final sector"
        );
        assert_eq!(
            app.current_lap_sectors.iter().sum::<i32>(),
            95_500,
            "the splits add up to the lap time"
        );
    }

    /// A wrong split is worse than a missing one: theoretical_best_lap_ms
    /// would take it as a personal best and never let go of it.
    #[test]
    fn final_sector_stays_empty_when_earlier_splits_are_missing() {
        let mut app = AppState::new(OverlayMode::External);
        app.track_sector_count = 3;
        app.current_lap_sectors = [30_000, 0, 0];

        app.close_current_lap_sectors(95_500);

        assert_eq!(app.current_lap_sectors[2], 0);
    }

    /// An implausible remainder means the earlier splits came from a
    /// different lap, so it is discarded rather than recorded.
    #[test]
    fn final_sector_rejects_an_implausible_remainder() {
        let mut app = AppState::new(OverlayMode::External);
        app.track_sector_count = 3;
        // The two known splits already exceed the lap time.
        app.current_lap_sectors = [60_000, 50_000, 0];

        app.close_current_lap_sectors(95_500);

        assert_eq!(app.current_lap_sectors[2], 0);
    }

    /// Two-sector mod tracks exist, and AcStatic says so.
    #[test]
    fn final_sector_honours_a_two_sector_track() {
        let mut app = AppState::new(OverlayMode::External);
        app.track_sector_count = 2;
        app.current_lap_sectors = [45_000, 0, 0];

        app.close_current_lap_sectors(95_500);

        assert_eq!(app.current_lap_sectors[1], 50_500);
        assert_eq!(app.current_lap_sectors[2], 0, "the third slot is unused");
    }

    #[test]
    fn fps_is_derived_from_the_frame_time() {
        let perf = PerfStats {
            frame_time: std::time::Duration::from_millis(16),
            ..Default::default()
        };
        assert!(
            (perf.fps() - 62.5).abs() < 0.5,
            "16ms is about 62fps, got {}",
            perf.fps()
        );
    }

    /// Before the first frame there is no duration to divide by, and
    /// reporting infinity in the footer would be worse than reporting nothing.
    #[test]
    fn fps_is_zero_before_anything_is_drawn() {
        assert_eq!(PerfStats::default().fps(), 0.0);
    }
}
