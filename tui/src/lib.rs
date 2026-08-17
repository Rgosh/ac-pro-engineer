pub mod keys;
pub mod platform;
pub mod ui;

use crate::ui::UIState;
use ac_core::RingBuffer;
use ac_core::analyzer::{AnalysisResult, TelemetryAnalyzer};
use ac_core::config::AppConfig;
use ac_core::content_manager::ContentManager;
use ac_core::engineer::{Engineer, Recommendation};
use ac_core::games::{Capabilities, Car, Fixed, Game, Reading, Session, Source, Status};
use ac_core::process::ProcessWatcher;
use ac_core::records::RecordManager;
use ac_core::session_info::SessionInfo;
use ac_core::setup_manager::SetupManager;
use ac_core::updater::Updater;

use clap::ValueEnum;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use tracing::metadata::LevelFilter;
use tracing::{error, info, warn};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// What a confirmation is about to do to the game folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    Install,
    Uninstall,
}

/// The first-run overlay offer: ask once, install if wanted, then say how to
/// use it. Anything past that is the ordinary status card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayOnboarding {
    Offer,
    Tips,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppStage {
    Launcher,
    Running,
}

/// Sector count to assume until the reading says otherwise. AC's own tracks are
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

/// The car catalogue of the game this build reads.
///
/// The game is asked for its own scan rather than named here. Detection used
/// to be four hardcoded Windows drive letters, which found nothing on Linux —
/// so the catalogue was always empty, every lookup returned None, and
/// everything downstream silently did nothing. An empty catalogue is still a
/// normal state, so it is logged rather than treated as a failure.
fn scan_installed_cars(game: &Game, configured: Option<&std::path::Path>) -> ContentManager {
    let Some(backend) = game.backend() else {
        return ContentManager::new();
    };
    let cars = (backend.scan_cars)(configured);
    if cars.is_empty() {
        tracing::info!(
            game = game.name,
            "No installation found; car specs unavailable"
        );
    }
    ContentManager::from_cars(cars)
}

pub struct AppState {
    /// Which game this run is reading, out of the registry.
    ///
    /// The driver's choice, made on the launcher and kept in the
    /// configuration — not a guess from what is running. A field rather than
    /// a call so that it is decided in one place instead of being re-decided
    /// by every screen that needs to know, and changed only through
    /// [`select_game`](Self::select_game), which rebuilds everything hanging
    /// off it.
    pub game: &'static Game,
    /// The live connection to it, behind the trait rather than in front of it.
    ///
    /// This was an `AssettoCorsa` until v0.3.7, and every screen reached
    /// through it into AC's own shared-memory structs — which is how a folder
    /// per game ended up carrying no data across its own boundary.
    pub source: Option<Box<dyn Source + Send>>,
    /// The most recent reading, and the only thing the screens draw from.
    ///
    /// A demo run and a screenshot run put a made-up one here, which is why
    /// there is no separate mock: above this field a reading somebody invented
    /// and a reading the game published are the same thing.
    pub reading: Option<Reading>,
    pub setup_manager: SetupManager,
    pub content_manager: ContentManager,
    pub record_manager: RecordManager,
    pub updater: Updater,
    pub engineer: Engineer,
    pub analyzer: TelemetryAnalyzer,
    pub ui_state: UIState,
    /// Publishes frames to the in-game Lua overlay, when the shared block
    /// could be opened. `None` is not worth stopping for — the overlay simply
    /// never appears, and everything else works.
    pub overlay_writer: Option<ac_core::overlay::shared_writer::OverlayWriter>,
    /// Everywhere else the computed frame goes.
    ///
    /// Beside the writer rather than replacing it for now: the mapping is on a
    /// path with a great deal of behaviour hanging off it — the launcher card,
    /// the diagnostics screen, the backing-file probe — and moving that into a
    /// sink is its own change. What this adds is everything that is *not* the
    /// in-game panel, which is what had no way in at all.
    pub broadcast: ac_core::broadcast::Broadcaster,
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
    pub car_history: RingBuffer<Car>,
    pub session_history: RingBuffer<Session>,
    pub current_lap_cars: Vec<Car>,
    pub current_lap_sessions: Vec<Session>,
    pub current_lap_number: i32,
    pub current_lap_sectors: [i32; 3],
    pub last_sector_index: i32,
    /// How many sectors this track publishes, from the reading. Not every track
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
    /// Where the first-run overlay offer has got to.
    pub onboarding: OverlayOnboarding,
    pub show_overlay_card: bool,
    pub overlay_card_selection: usize,
    pub overlay_report: ac_core::overlay::install::InstallReport,
    /// Which `shm-bridge.exe` is serving the mapping, and whether it can.
    ///
    /// The third of the three pieces that have to agree about a frame, and the
    /// one that used to be unknowable: a bridge older than the struct maps too
    /// few bytes, CSP silently refuses to open it, and the panel sits saying
    /// "waiting for AC Pro Engineer" beside a mapping that is right there.
    pub bridge_status: ac_core::overlay::bridge::BridgeStatus,
    /// What the last fetch-a-newer-bridge attempt said. Empty until one is
    /// asked for.
    pub bridge_fetch_status: String,
    /// A bridge on the release page worth taking, found by the startup check.
    ///
    /// Filled by a background thread and never acted on by one: the check is
    /// automatic, the download is not. Pressing [B] is what spends bandwidth
    /// and replaces a binary, and doing that unasked to a file the user cannot
    /// rebuild is not a thing to do quietly.
    pub bridge_offer: Arc<Mutex<Option<ac_core::overlay::bridge_update::RemoteBridge>>>,
    pub overlay_install_status: String,
    /// Listening for another machine's frames, when `receive_from` is set.
    ///
    /// `None` is the ordinary case: this is off unless a viewer asks for it.
    pub receiver: Option<ac_core::broadcast::receiver::FrameReceiver>,
    /// Who is being watched, for the status line. `None` until one arrives.
    pub remote_sender: Option<String>,
    /// The last few finished laps and what the engineer made of each, ready
    /// for the frame.
    ///
    /// Kept rather than computed per frame: a debrief is a whole lap's worth of
    /// averages and it changes once a lap, while the frame goes out sixty times
    /// a second. Newest first, which is the order the panel draws them in.
    pub overlay_debrief: Vec<ac_core::overlay::frame::DebriefLap>,
    /// A result worth showing: the install or removal was asked for from the
    /// Settings tab, where a status line at the bottom of a card nobody is
    /// looking at is the same as no answer at all.
    pub overlay_result_popup: bool,
    /// The bridge report, and whether it is on screen.
    ///
    /// Held rather than computed per frame: it reads a file in `/dev/shm` and
    /// scans a binary for a version marker, which is not work to do sixty
    /// times a second behind a screen nobody has opened.
    pub show_overlay_diagnosis: bool,
    pub overlay_diagnosis: ac_core::overlay::diagnosis::Report,
    /// Asked for, not yet done. Writing into someone's game folder is worth a
    /// second keystroke — [U] and [I] are neighbours on the keyboard, and one
    /// of them deletes.
    pub overlay_confirm: Option<OverlayAction>,
    pub overlay_confirm_selection: usize,
    pub is_demo_mode: bool,
    pub demo_tick_counter: u64,
    pub show_help: bool,
    /// How long the last rendered frame took, and how long ago the background
    /// tick last completed.
    ///
    /// The render loop and the tick thread contend for the same state mutex,
    /// so when one stalls it is usually because the other is holding the lock.
    /// Neither was observable, which made that a guess.
    pub perf: PerfStats,
}

/// So `AppState::default()` and `AppState::new()` mean the same thing, which
/// is what a reader assumes and what clippy insists on.
impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
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

        // Which game this build reads: the one the driver chose, out of the
        // registry, so everything below asks the entry rather than naming a
        // simulator. Not "whichever is running" — two games publish under the
        // same three page names, and a confident wrong answer there costs a
        // bridge in the wrong Proton prefix and an engineer running the other
        // game's thresholds.
        let game = ac_core::games::registry::chosen(&config.game);

        let setup_manager = SetupManager::new(game.backend().and_then(|b| b.setups.as_ref()));
        setup_manager.set_documents_override(&config.ac_documents_path);

        // Built before the struct literal, where `config` is still ours to
        // read: it is moved into the state below.
        let broadcast = {
            let mut broadcaster = ac_core::broadcast::Broadcaster::new();
            // Configured, and off unless it is. This is telemetry about a
            // person; it leaves the machine because they said so.
            let target = config.overlay.broadcast_to.trim();
            if !target.is_empty() {
                match target.parse() {
                    Ok(address) => match ac_core::broadcast::udp::UdpSink::new(
                        address,
                        game.id,
                        config.overlay.broadcast_name.clone(),
                        config.overlay.broadcast_hz,
                    ) {
                        Ok(sink) => broadcaster.add(Box::new(sink)),
                        Err(error) => {
                            warn!(error = ?error, "Could not open the broadcast socket")
                        }
                    },
                    Err(error) => warn!(
                        target,
                        error = ?error,
                        "broadcast_to is not a host:port address"
                    ),
                }
            }
            broadcaster
        };

        // The other end of it. Off unless asked for, and a port that cannot be
        // bound is a warning rather than a failure to start: something else is
        // already on it, and the rest of the application still works.
        let receiver = {
            let listen = config.overlay.receive_from.trim();
            if listen.is_empty() {
                None
            } else {
                match listen.parse() {
                    Ok(address) => {
                        match ac_core::broadcast::receiver::FrameReceiver::bind(address) {
                            Ok(receiver) => {
                                info!(%listen, "Listening for another machine's frames");
                                Some(receiver)
                            }
                            Err(error) => {
                                warn!(error = ?error, %listen, "Could not listen there");
                                None
                            }
                        }
                    }
                    Err(error) => {
                        warn!(error = ?error, %listen, "receive_from is not an ip:port address");
                        None
                    }
                }
            }
        };

        let mut state = Self {
            game,
            source: None,
            reading: None,
            is_demo_mode: false,
            demo_tick_counter: 0,
            setup_manager,
            content_manager: scan_installed_cars(game, config.ac_install_override()),
            record_manager: RecordManager::new(),
            updater: Updater::new(),
            engineer: Engineer::new(&config),
            analyzer: TelemetryAnalyzer::new(),
            ui_state: UIState::new(),
            // Installed from the binary that writes the struct it reads, so
            // the two can never be out of step. Cheap and idempotent: it only
            // writes when the files differ.
            //
            // This is no longer the only attempt — see `ensure_overlay_installed`.
            // It used to be, and a first run that could not reach the game
            // folder for any reason left the panel uninstalled for the whole
            // session with nothing on screen to say so.
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
            game_watcher: match game.backend() {
                Some(backend) => ProcessWatcher::new(backend.processes)
                    .corroborated_by(backend.telemetry_is_reachable),
                None => ProcessWatcher::new(&[]),
            },
            is_connected: false,
            active_tab: AppTab::Dashboard,
            session_info: SessionInfo::default(),
            car_history: RingBuffer::new(config.history_size),
            session_history: RingBuffer::new(config.history_size),
            current_lap_cars: Vec::with_capacity(36000),
            current_lap_sessions: Vec::with_capacity(36000),
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
            onboarding: OverlayOnboarding::Done,
            show_overlay_card: false,
            overlay_card_selection: 0,
            overlay_report: ac_core::overlay::install::InstallReport {
                game_root: None,
                app_path: None,
                current: false,
                csp_present: false,
                panel_version: None,
                panel_release: None,
            },
            bridge_status: ac_core::overlay::bridge::BridgeStatus::NotRunning,
            bridge_fetch_status: String::new(),
            bridge_offer: Arc::new(Mutex::new(None)),
            overlay_install_status: String::new(),
            overlay_debrief: Vec::new(),
            receiver,
            remote_sender: None,
            broadcast,
            overlay_result_popup: false,
            show_overlay_diagnosis: false,
            overlay_diagnosis: ac_core::overlay::diagnosis::report(),
            overlay_confirm: None,
            overlay_confirm_selection: 1,
            show_help: false,
            perf: PerfStats::default(),
        };

        state.refresh_overlay_report();
        state.check_for_bridge_update();

        // A new install gets the offer; everyone else gets the status card, if
        // they have left it on. Neither happens on a game with no panel —
        // there is nothing to offer and nothing to report on.
        if !state.game_has_a_panel() {
            state.onboarding = OverlayOnboarding::Done;
        } else if state.config.overlay.onboarding_done {
            state.show_overlay_card = state.config.overlay.startup_card;
        } else {
            state.onboarding = OverlayOnboarding::Offer;
        }

        // A bridge that cannot serve the overlay overrides "do not show this at
        // startup". That preference means "stop telling me things are fine";
        // it cannot reasonably mean "stay quiet while the panel is broken", and
        // the alternative is a driver hunting through the game for a fault that
        // is not there.
        if state.game_has_a_panel()
            && !state.bridge_status.is_workable()
            && state.overlay_report.current
            && state.onboarding == OverlayOnboarding::Done
        {
            state.show_overlay_card = true;
        }
        state
    }

    /// What kind of car is being driven, for the thresholds that depend on it.
    ///
    /// The catalogue's tags first — Assetto Corsa publishes them per car and
    /// they are the game's own answer — then the car id, which Competizione
    /// names exhaustively and Assetto Corsa names well enough. An unrecognised
    /// car comes back `Unknown`, and the engineer then keeps the driver's own
    /// thresholds rather than pressing a mod into a class it may not be in.
    pub fn car_class(&self) -> ac_core::games::CarClass {
        let id = self
            .reading
            .as_ref()
            .map(|reading| reading.fixed.car_model.clone())
            .unwrap_or_default();
        let tags = self
            .content_manager
            .get_car_specs(&id)
            .map(|specs| vec![specs.class.clone()])
            .unwrap_or_default();
        ac_core::games::CarClass::identify(&id, &tags)
    }

    /// Whether the chosen game can run the in-game panel at all.
    ///
    /// It is a Custom Shaders Patch app and CSP is an Assetto Corsa mod, so on
    /// any other game the offer to install it, the status card and the install
    /// itself are all offering something that cannot work — and the card would
    /// then report the panel as missing for ever, in a game that has nowhere
    /// to put it.
    pub fn game_has_a_panel(&self) -> bool {
        self.game
            .backend()
            .is_some_and(|backend| backend.capabilities.in_game_panel)
    }

    /// Look at the game folder again and remember what is there.
    pub fn refresh_overlay_report(&mut self) {
        self.overlay_report =
            ac_core::overlay::install::describe(self.config.ac_install_override());
        self.bridge_status = ac_core::overlay::bridge::status(ac_core::updater::CURRENT_VERSION);
    }

    /// Put the panel in the game folder if it is not already there.
    ///
    /// The install used to happen once, while the application was being
    /// constructed, and never again. Every reason that attempt can fail — the
    /// game not installed yet, Steam not unpacked, a folder that briefly could
    /// not be written — therefore meant no panel for the whole session, with
    /// the failure recorded in a log file and nowhere a user would look. The
    /// report on screen said the panel was missing without ever saying that
    /// putting it there had been tried and had not worked.
    ///
    /// So: try again whenever the game folder is looked at afresh, and keep
    /// what happened. `install` compares the files first, so the normal case
    /// is a read of nineteen small files and no write at all.
    pub fn ensure_overlay_installed(&mut self) {
        use ac_core::overlay::install::{InstallOutcome, install};

        if !self.game_has_a_panel() {
            return;
        }
        self.refresh_overlay_report();
        if self.overlay_report.game_root.is_none() || self.overlay_report.current {
            return;
        }

        match install(self.config.ac_install_override()) {
            Ok(InstallOutcome::Installed { updated }) => {
                info!("Installed the in-game overlay ({updated} file(s) written)");
                self.overlay_install_status = format!("installed, {updated} file(s) written");
                // The report above was taken before the files were written, so
                // leaving it would have the card still reading "not installed"
                // over a panel that is now there.
                self.refresh_overlay_report();
            }
            Ok(InstallOutcome::AlreadyCurrent) => {}
            Ok(InstallOutcome::NoGameFound) => {}
            // The one that used to be silent. A game folder that cannot be
            // written to is the likeliest cause and the least guessable: it
            // looks exactly like the panel never having been installed.
            Err(error) => {
                warn!(error = ?error, "Could not install the in-game overlay");
                self.overlay_install_status = format!("could not install: {error}");
            }
        }
    }

    /// Recompute the debrief the panel is shown, newest lap first.
    ///
    /// `DEBRIEF_LAPS` of them, because that is what the frame carries — the
    /// panel cannot ask for a lap that was not published, so what it can show
    /// is decided here.
    pub fn rebuild_overlay_debrief(&mut self) {
        use ac_core::overlay::frame::{DEBRIEF_LAPS, DebriefLap};

        self.overlay_debrief = self
            .analyzer
            .laps
            .iter()
            .rev()
            .take(DEBRIEF_LAPS)
            .map(|lap| DebriefLap {
                lap_number: lap.lap_number.max(0) as u32,
                lap_time_ms: lap.lap_time_ms.max(0) as u32,
                sectors: [
                    lap.sectors[0].max(0) as u32,
                    lap.sectors[1].max(0) as u32,
                    lap.sectors[2].max(0) as u32,
                ],
                advice: ac_core::debrief::debrief(lap, &self.config),
            })
            .collect();
    }

    /// The best each sector has been this session.
    ///
    /// Theoretical: the best first sector and the best third need not have come
    /// from the same lap, and that is the comparison a driver makes anyway.
    fn best_sectors(&self) -> [u32; ac_core::overlay::frame::SECTORS] {
        let mut best = [0u32; ac_core::overlay::frame::SECTORS];
        for lap in &self.analyzer.laps {
            for (sector, time) in lap.sectors.iter().enumerate() {
                if *time <= 0 {
                    continue;
                }
                let time = *time as u32;
                if best[sector] == 0 || time < best[sector] {
                    best[sector] = time;
                }
            }
        }
        best
    }

    /// Ask the release page whether there is a bridge worth taking.
    ///
    /// Runs once at startup, on its own thread, and only *looks*. The overlay
    /// is dead on Linux without a bridge that matches the frame, and expecting
    /// people to know that and to check by hand is how a beta produces reports
    /// about a panel that never appears. Nothing is downloaded here — the card
    /// says what it found and [B] is what acts on it.
    ///
    /// Skipped on Windows, where there is no bridge, and skipped when the one
    /// already running is current — the common case should cost nothing.
    /// Ask all three pieces again. What `[R]` on the diagnostics screen does.
    pub fn refresh_overlay_diagnosis(&mut self) {
        self.overlay_diagnosis = ac_core::overlay::diagnosis::report();
    }

    pub fn check_for_bridge_update(&self) {
        use ac_core::overlay::bridge::{self, BridgeStatus};
        use ac_core::overlay::bridge_update;

        if cfg!(target_os = "windows") || matches!(self.bridge_status, BridgeStatus::NotRequired) {
            return;
        }

        // A bridge from this release, running and serving: nothing to offer,
        // and no reason to reach the network for it.
        if matches!(&self.bridge_status, BridgeStatus::Current(_)) {
            return;
        }

        let offer = self.bridge_offer.clone();
        let wanted = ac_core::updater::CURRENT_VERSION;
        thread::spawn(move || {
            // This release's own bridge if it is published, and only otherwise
            // the newest. Asking for "the newest" offered a v0.3.4 bridge to a
            // v0.3.5 application, which is the version that cannot serve it.
            let Ok(remote) = bridge_update::best_for(wanted) else {
                // Offline, rate-limited, no release with a bridge in it. None
                // of those is worth a message on a card about the overlay.
                return;
            };

            let local = bridge::installed_executable()
                .as_deref()
                .and_then(bridge::version_in_executable);

            if bridge_update::should_fetch(&remote.version, local.as_deref(), wanted) {
                info!(
                    "A published shm-bridge v{} is worth taking over the one here ({})",
                    remote.version,
                    local.as_deref().unwrap_or("unknown")
                );
                *offer.lock().unwrap_or_else(|e| e.into_inner()) = Some(remote);
            }
        });
    }

    /// The bridge the startup check found worth taking, if it found one.
    pub fn bridge_offer(&self) -> Option<ac_core::overlay::bridge_update::RemoteBridge> {
        self.bridge_offer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Fetch the published `shm-bridge.exe` and put it where this application
    /// looks for one.
    ///
    /// Blocking, and deliberately: it is one small file behind an explicit
    /// keystroke, and a spinner on a card that exists for ten seconds buys
    /// nothing. The card says what happened when it returns.
    ///
    /// Nothing here starts the bridge. Replacing a binary that is running is
    /// how you get a half-written executable, and the user has to restart it
    /// through protontricks anyway.
    pub fn fetch_bridge_now(&mut self) {
        use ac_core::overlay::bridge;
        use ac_core::overlay::bridge_update;

        if cfg!(target_os = "windows") {
            self.bridge_fetch_status =
                "Windows makes the mapping itself — there is no bridge to fetch".to_string();
            return;
        }

        // The bridge published with *this* release, not merely the newest one
        // there is. The two are usually different, because the bridge is not
        // republished every time.
        let wanted = ac_core::updater::CURRENT_VERSION;
        let remote = match bridge_update::best_for(wanted) {
            Ok(remote) => remote,
            Err(error) => {
                self.bridge_fetch_status = format!("could not check GitHub: {error}");
                return;
            }
        };

        // Where the one already here is, or where one would go: beside the
        // running executable, which is where the release bundle puts it.
        let destination = bridge::installed_executable().or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|dir| dir.join(bridge::BRIDGE_EXE)))
        });
        let Some(destination) = destination else {
            self.bridge_fetch_status = "could not work out where to put the bridge".to_string();
            return;
        };

        let local = bridge::version_in_executable(&destination);
        if !bridge_update::should_fetch(&remote.version, local.as_deref(), wanted) {
            let here = local.as_deref().unwrap_or("?");
            // Naming which of the two situations it is. "Nothing to fetch"
            // used to be the answer both when everything was fine and when
            // the bridge was too old and no replacement existed — and in the
            // second case the panel does not work, so the message read as the
            // application refusing to help.
            self.bridge_fetch_status = if here == wanted {
                format!("the bridge here is v{here}, the same release as this application")
            } else {
                format!(
                    "the bridge here is v{here} and the newest published is v{} —                      no bridge for v{wanted} has been published yet, so build one:                      cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu",
                    remote.version
                )
            };
            return;
        }

        self.bridge_fetch_status = match bridge_update::download_to(&remote, &destination) {
            Ok(path) => format!(
                "fetched shm-bridge v{}{} into {} — restart it to pick it up",
                remote.version,
                if remote.version == wanted {
                    ""
                } else {
                    " (not this release's own; no bridge for it is published yet)"
                },
                path.display()
            ),
            Err(error) => format!("could not fetch v{}: {error}", remote.version),
        };
        self.refresh_overlay_report();
    }

    /// Remember that the offer has been made, whichever way it was answered.
    pub fn finish_onboarding(&mut self) {
        self.onboarding = OverlayOnboarding::Done;
        self.config.overlay.onboarding_done = true;
        let _ = self.config.save();
    }

    /// Take the overlay back out of the game folder.
    ///
    /// The panel's settings are CSP's, kept outside the app folder, so this is
    /// reversible: installing again finds them where they were.
    pub fn uninstall_overlay_now(&mut self) {
        self.overlay_install_status =
            match ac_core::overlay::install::uninstall(self.config.ac_install_override()) {
                Ok(0) => "nothing to remove".to_string(),
                Ok(removed) => format!("removed, {removed} file(s) — settings kept"),
                Err(error) => format!("could not remove: {error}"),
            };
        self.refresh_overlay_report();
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
                "no Assetto Corsa found — set ac_install_path in config.json".to_string()
            }
            Err(error) => format!("failed: {error}"),
        };
        self.refresh_overlay_report();
    }

    /// Work with another game from now on.
    ///
    /// Everything that was built from the old entry is rebuilt or dropped:
    /// the live connection, because it is attached to the other game's pages
    /// and would keep reading them; the process watcher, because it is
    /// looking for the other game's executable; the setup store, because the
    /// two games keep setups in different places and one of them keeps none
    /// this program can read.
    ///
    /// The reading goes too. It is the last frame of a car in another
    /// simulator, and leaving it on screen under a new game's name is exactly
    /// the kind of quietly wrong that this project spends its tests on.
    pub fn select_game(&mut self, game: &'static Game) {
        if game.id == self.game.id {
            return;
        }
        self.config.game = game.id.to_string();
        let _res = self.config.save();
        self.apply_game(game);
    }

    /// The same, without writing the choice down.
    ///
    /// Split out because saving is a side effect on the *user's* configuration
    /// file, and a test that exercised `select_game` wrote a game into it and
    /// then every other test in the process started up as that game. Tests use
    /// this; the launcher uses the one above.
    fn apply_game(&mut self, game: &'static Game) {
        info!("Working with {} from now on", game.name);
        self.game = game;

        self.source = None;
        self.reading = None;
        self.is_connected = false;
        self.is_game_running = false;
        self.game_watcher = match game.backend() {
            Some(backend) => ProcessWatcher::new(backend.processes)
                .corroborated_by(backend.telemetry_is_reachable),
            None => ProcessWatcher::new(&[]),
        };

        self.setup_manager = SetupManager::new(game.backend().and_then(|b| b.setups.as_ref()));
        self.setup_manager
            .set_documents_override(&self.config.ac_documents_path);

        // The panel belongs to one game. Switching to a game that cannot run
        // it takes the card and the offer away rather than leaving a driver
        // looking at the state of an overlay their simulator has nowhere to
        // load.
        if !self.game_has_a_panel() {
            self.show_overlay_card = false;
            self.onboarding = OverlayOnboarding::Done;
        }
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

        // The half of the demo reading that does not move. `update_demo_tick`
        // fills in the car and the session sixty times a second on top of it.
        self.reading = Some(Reading {
            // The demo stands in for a running Assetto Corsa, so it reports
            // what one reports.
            capabilities: Capabilities::all(),
            fixed: Fixed {
                max_rpm: 12500,
                max_fuel_litres: 110.0,
                car_model: "ks_ferrari_sf70h".to_string(),
                track: "monza".to_string(),
                ..Default::default()
            },
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
                // Monza, so the corner report has real metres to work in.
                track_length_m: 5793.0,
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
        let gear = ((speed / 45.0) as i32).clamp(0, 6);
        let gas = (0.5 + (t * 1.2).cos() * 0.5).clamp(0.0, 1.0);
        let brake = if (t * 1.2).cos() < -0.4 { 0.75 } else { 0.0 };
        let steer = (t * 0.7).sin() * 0.35;
        let lat_g = (t * 0.7).sin() * 1.6;
        let lon_g = (t * 1.2).cos() * 1.3;

        let car = Car {
            speed_kmh: speed,
            rpm,
            gear,
            fuel_litres: 34.2,
            throttle: gas,
            brake,
            clutch: 0.0,
            steer_angle: steer,
            acc_g: [lat_g, 0.0, lon_g],
            tyre_pressure_psi: [27.4, 27.6, 27.5, 27.3],
            tyre_temp_inner_c: [89.2 + (t.sin() * 2.0), 88.0, 92.1, 90.5],
            tyre_temp_middle_c: [86.4 + (t.sin() * 2.0), 85.2, 89.0, 87.8],
            tyre_temp_outer_c: [82.1 + (t.sin() * 2.0), 81.0, 85.2, 84.0],
            brake_temp_c: [450.0 + (t.cos() * 30.0), 442.0, 380.0, 375.0],
            air_temp_c: 22.5,
            road_temp_c: 34.0,
            tc: 3.0,
            abs: 2.0,
            ..Default::default()
        };

        let session = Session {
            status: Status::Live,
            surface_grip: 0.98,
            completed_laps: 5,
            current_lap_ms: ((t * 1000.0) as i32) % 81452,
            last_lap_ms: 81452,
            best_lap_ms: 81452,
            position: 2,
            fuel_per_lap: 2.85,
            ..Default::default()
        };

        let reading = self.reading.get_or_insert_with(Reading::default);
        reading.car = car;
        reading.session = session;
    }

    pub fn car(&self) -> Option<&Car> {
        self.reading.as_ref().map(|reading| &reading.car)
    }

    pub fn session(&self) -> Option<&Session> {
        self.reading.as_ref().map(|reading| &reading.session)
    }

    pub fn fixed(&self) -> Option<&Fixed> {
        self.reading.as_ref().map(|reading| &reading.fixed)
    }

    /// What the game being read can measure, if one is being read at all.
    ///
    /// `None` is **not** "nothing measured": it is "no game", which answers a
    /// different question. A lap loaded from a file has sector times in it
    /// whether or not a simulator is running, so a screen drawing saved data
    /// must not be told the game does not publish them.
    pub fn capabilities(&self) -> Option<ac_core::games::Capabilities> {
        self.reading.as_ref().map(|reading| reading.capabilities)
    }

    pub fn process_tick_logic(&mut self, reading: Reading) {
        // Both are `Copy`, so the reading can be kept whole for the screens
        // while the tick works from its two halves.
        let (car, session) = (reading.car, reading.session);
        let track_length_m = reading.fixed.track_length_m;
        // The sector count was read by nothing, so every track was treated as
        // three-sector.
        let sector_count = reading.fixed.sector_count;
        if sector_count > 0 && sector_count as usize <= self.current_lap_sectors.len() {
            self.track_sector_count = sector_count;
        }
        let capabilities = reading.capabilities;
        self.reading = Some(reading);

        self.update_live_buffers(&car, &session);
        self.update_session_info(&session);
        self.engineer.update_config(&self.config);
        // Beside the config, and for the same reason: it is a property of the
        // run rather than of the tick, and the engineer withholds everything
        // until it is told. A tick that forgot this would produce an engineer
        // with nothing to say, which is loud — the alternative default is a
        // wrong verdict, which is not.
        self.engineer.update_capabilities(capabilities);
        // And what kind of car it is, which decides what those measurements
        // are supposed to look like. The game's own tags where there are any —
        // Assetto Corsa ships them beside each car — and the car's id
        // otherwise, which is descriptive in both games.
        self.engineer.update_car_class(self.car_class());
        self.engineer.update(&car, &session, &self.session_info);

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
                session.track_position,
                session.current_lap_ms as f32 / 1000.0,
            )
        {
            self.engineer.stats.current_delta = delta;
        }

        self.publish_overlay_frame(&car, &session);

        // Sector splits are captured on the transition *out* of a sector,
        // when AC publishes the one just finished in `last_sector_time`. The
        // final sector is the exception: its transition is the lap rollover,
        // which races the `completed_laps` increment handled below. Whichever
        // AC publishes first decides whether the last split lands in this lap
        // or the next one, so it is derived from the lap time at lap close
        // instead — see `close_current_lap_sectors`.
        let current_sector = session.current_sector;
        if current_sector != self.last_sector_index {
            let finished = self.last_sector_index;
            let is_final_sector = finished == self.track_sector_count - 1;
            if finished >= 0
                && (finished as usize) < self.current_lap_sectors.len()
                && !is_final_sector
            {
                self.current_lap_sectors[finished as usize] = session.last_sector_ms;
            }
            self.last_sector_index = current_sector;
        }

        let completed_laps = session.completed_laps;
        if self.current_lap_number == -1 {
            self.current_lap_number = completed_laps;
        }

        if completed_laps != self.current_lap_number {
            if completed_laps == self.current_lap_number + 1 {
                let last_lap_time = session.last_lap_ms;
                if last_lap_time > 10000 && !self.current_lap_cars.is_empty() {
                    self.close_current_lap_sectors(last_lap_time);
                    self.analyzer.process_lap(
                        self.current_lap_number,
                        last_lap_time,
                        &self.current_lap_cars,
                        &self.current_lap_sessions,
                        self.current_lap_sectors,
                        self.session_info.car_name.clone(),
                        self.session_info.track_name.clone(),
                        self.config.target_tyre_pressure,
                        self.config.update_rate,
                    );
                    // The lap the analyser has just closed is the one the panel
                    // wants to hear about, so the debrief is built here and not
                    // in the publisher: once a lap rather than sixty times a
                    // second, and the sentences are identical every frame in
                    // between.
                    self.rebuild_overlay_debrief();

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
                        track_length_m,
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
            self.current_lap_cars.clear();
            self.current_lap_sessions.clear();
            self.current_lap_sectors = [0; 3];
            self.current_lap_number = completed_laps;
        }

        if (session.status.is_on_track() || self.is_demo_mode)
            && (car.speed_kmh > 1.0 || car.rpm > 1000)
            && self.current_lap_cars.len() < 36000
        {
            self.current_lap_cars.push(car);
            self.current_lap_sessions.push(session);
        }

        if !self.session_info.car_name.is_empty() && self.session_info.car_name != "-" {
            self.setup_manager
                .set_context(&self.session_info.car_name, &self.session_info.track_name);
            self.setup_manager.detect_current(
                car.fuel_litres,
                car.brake_bias / 100.0,
                &car.tyre_pressure_psi,
                &car.tyre_temp_middle_c,
            );
        }

        let active_setup = self.setup_manager.get_active_setup();
        self.recommendations = self
            .engineer
            .analyze_live(&car, &session, active_setup.as_ref());
    }

    pub fn tick(&mut self) {
        self.ui_state.update_blink();
        self.ui_state.analysis.tick_status();

        // Somebody else driving takes the panel over entirely. Before the game
        // is read, not after: the point of watching a friend is that the
        // numbers on screen are theirs, and letting the local tick run
        // underneath would have the two fighting for the same mapping.
        if self.pump_received_frame() {
            return;
        }
        if self.is_demo_mode {
            self.update_demo_tick();
            if let Some(reading) = self.reading.clone() {
                self.process_tick_logic(reading);
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
        let game_just_started = process_active && !self.is_game_running;
        self.is_game_running = process_active;

        // A second piece of evidence, because the process name is evidence and
        // not proof. Under Proton the command line is a Windows path handed
        // through a launcher — Competizione's `acc.exe` starts
        // `AC2-Win64-Shipping.exe` — and a name this build has not been told
        // about means a driver sitting in the car watching an application that
        // says the game is not running.
        //
        // Telemetry being *reachable* is not the same claim: on Linux the
        // bridge creates those files whether or not a game ever writes to
        // them. So this only opens the connection; what decides that a game is
        // there is a reading with a car in it, below.
        let telemetry_reachable = self
            .game
            .backend()
            .is_some_and(|backend| (backend.telemetry_is_reachable)());

        // Assetto Corsa starting is the one moment worth looking again: it is
        // when a game installed since this application opened has certainly
        // finished unpacking, and it is the last point at which writing the
        // panel still helps — AC reads `apps/lua` while it loads, so a panel
        // written now is in the list the *next* time the game starts rather
        // than never. Once per start, not per frame.
        if game_just_started {
            self.ensure_overlay_installed();
        }

        if self.stage != AppStage::Running {
            // The launcher is where the application spends the minutes before
            // a race, and it published nothing from here — so a driver who
            // opened the panel in the garage was told the application was not
            // running while looking at it.
            self.publish_overlay_idle();
            return;
        }

        if !process_active && !telemetry_reachable && self.is_connected {
            self.disconnect();
        } else if (process_active || telemetry_reachable)
            && !self.is_connected
            && let Err(error) = self.connect_memory()
        {
            // Every tick, and deliberately quiet about it in the normal case:
            // "the pages are not there yet" is what the seconds before a
            // session look like. The connection refusing because they belong
            // to the *other* game is the one worth reading, and it says so.
            error!(error = ?error, "Cannot connect to shared memory");
        }

        if !self.is_connected {
            // AC running with nothing in its shared memory yet: the menus, the
            // loading screen, the seconds in the garage before the session
            // starts. The application is fine, and now says so.
            self.publish_overlay_idle();
            return;
        }

        let Some(source) = self.source.as_mut() else {
            self.publish_overlay_idle();
            return;
        };

        // A tick that reads nothing is the game being closed or between
        // sessions, which is a state and not a failure — the panel is told the
        // application is alive and has no car, which is the distinction v0.3.5
        // added.
        let Some(reading) = source.poll() else {
            self.publish_overlay_idle();
            return;
        };

        // A car on track is proof the game is up, whatever the process table
        // was asked. This is the half that makes the connection above safe to
        // open on telemetry alone: an empty mapping left by the bridge reads
        // as `Status::Off`, and nothing here claims a game from that.
        if reading.session.status.is_on_track() {
            self.is_game_running = true;
        }

        self.process_tick_logic(reading);
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

    /// The part of a frame that does not come from the car.
    ///
    /// Versions, which sections the driver asked for, which language to speak,
    /// and whether there is a car at all. Split out because this is the whole
    /// frame when there is no session: see [`Self::publish_overlay_idle`].
    fn overlay_frame_shell(&self) -> ac_core::overlay::frame::OverlayFrame {
        use ac_core::overlay::frame::{OverlayFrame, flags};

        let mut frame = OverlayFrame::empty();

        frame.target_pressure_front = self.config.target_hot_pressure_front;
        frame.target_pressure_rear = self.config.target_hot_pressure_rear;

        frame.set_flag(flags::CONNECTED, self.is_connected);
        // What the driver asked for in the Settings tab, and nothing else.
        // These used to be ANDed with a second pair of switches on the old
        // desktop-overlay manager, which nothing ever set to false — so a
        // block the driver had switched off in the config could still be
        // published, and the reason lived two structs away.
        frame.set_flag(flags::SHOW_TELEMETRY, self.config.overlay.show_telemetry);
        frame.set_flag(flags::SHOW_ENGINEER, self.config.overlay.show_engineer);
        frame.set_flag(flags::SHOW_SESSION, self.config.overlay.show_session);
        frame.set_flag(flags::SHOW_TIMING, self.config.overlay.show_timing);
        frame.set_flag(
            flags::RUSSIAN,
            self.config.language == ac_core::config::Language::Russian,
        );
        frame.set_flag(flags::SHOW_FUEL, self.config.overlay.show_fuel);

        frame
    }

    /// Draw whatever a remote sender published, if this build is listening.
    ///
    /// Returns whether a frame was taken over, so the caller can stop: while
    /// somebody else's telemetry is on screen the local game is not what the
    /// panel is about.
    ///
    /// The receiver is the other half of `broadcast::udp` and does no analysis:
    /// what arrives is the finished frame, sentences and all, so the viewer
    /// sees exactly what the driver's own engineer is saying.
    fn pump_received_frame(&mut self) -> bool {
        use ac_core::broadcast::receiver::Received;

        let Some(receiver) = self.receiver.as_mut() else {
            return false;
        };

        match receiver.poll() {
            Received::Frame(frame) => {
                self.remote_sender = receiver.sender().map(|(from, name)| {
                    if name.is_empty() {
                        from.to_string()
                    } else {
                        format!("{name} ({from})")
                    }
                });
                if let Some(writer) = self.overlay_writer.as_mut() {
                    writer.publish(&frame);
                }
                true
            }
            // A datagram that was not ours, or nothing at all. Neither is a
            // reason to stop drawing what is already on screen — a receiver
            // polled sixty times a second sees `Idle` on almost every tick,
            // and blanking the panel between two ten-a-second frames would
            // flicker the whole session.
            Received::Rejected(_) | Received::Idle => self.remote_sender.is_some(),
        }
    }

    /// Publish a frame with no car in it.
    ///
    /// The panel used to go dead in three situations that are not failures:
    /// the application sitting on its launcher screen, AC running with nothing
    /// in shared memory yet, and the driver in the pit garage before a session
    /// starts. In every one of them the panel showed "AC Pro Engineer is not
    /// running" — which is both wrong and the exact message that sends someone
    /// hunting through the bridge, the install and the Proton prefix for a
    /// problem that is not there.
    ///
    /// The application is running, so it says so. The sequence keeps advancing,
    /// which is how the panel knows; `CONNECTED` stays clear, which is how it
    /// knows not to draw zeroes as telemetry. Settings, versions and the link
    /// state are all reachable in that state, which is when they are most
    /// likely to be wanted.
    pub fn publish_overlay_idle(&mut self) {
        let frame = self.overlay_frame_shell();
        if let Some(writer) = self.overlay_writer.as_mut() {
            writer.publish(&frame);
        }
        self.broadcast.publish(&frame);
    }

    /// Pack the current state into an overlay frame and publish it.
    ///
    /// Everything here is a copy of an already-computed value: the overlay
    /// draws on AC's render thread, so no work that can be done on this side
    /// belongs on that one.
    fn publish_overlay_frame(&mut self, car: &Car, session: &Session) {
        use ac_core::overlay::frame::flags;

        // Nowhere for it to go. Both are usually present, and checking is
        // cheaper than filling in a frame that is dropped on the next line.
        if self.overlay_writer.is_none() && self.broadcast.is_empty() {
            return;
        }

        let mut frame = self.overlay_frame_shell();

        let best_sectors = self.best_sectors();

        frame.speed_kmh = car.speed_kmh;
        frame.rpm = car.rpm;
        // AC encodes reverse as 0 and neutral as 1. Translated here so the
        // overlay does not have to know that.
        frame.gear = car.gear - 1;
        frame.fuel_litres = car.fuel_litres;
        frame.air_temp_c = car.air_temp_c;
        frame.road_temp_c = car.road_temp_c;
        frame.surface_grip = session.surface_grip;

        frame.tyre_pressure_psi = car.tyre_pressure_psi;
        frame.tyre_wear_percent = car.tyre_wear;
        frame.brake_temp_c = car.brake_temp_c;
        for i in 0..4 {
            frame.tyre_temp_c[i] = car.avg_tyre_temp_c(i);
        }

        frame.last_lap_ms = session.last_lap_ms;
        frame.best_lap_ms = session.best_lap_ms;
        frame.current_lap_ms = session.current_lap_ms;
        frame.position = session.position;

        frame.fuel_laps_remaining = self.engineer.stats.fuel_laps_remaining;
        frame.fuel_per_lap = self.engineer.stats.fuel_consumption_rate;
        frame.delta_seconds = self.engineer.stats.current_delta;

        frame.apply_session(&self.session_info);

        frame.set_flag(flags::PIT_LIMITER, car.pit_limiter);
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
        frame.set_debrief(
            &self.overlay_debrief,
            self.config.overlay.debrief_lines as usize,
        );
        frame.set_sectors(&self.overlay_debrief, best_sectors);

        // The tyre's edges, which the panel needs to show whether a tyre is
        // leaning the right way rather than only how hot it is — the middle one
        // has been going across since the first frame.
        frame.tyre_temp_inner_c = car.tyre_temp_inner_c;
        frame.tyre_temp_outer_c = car.tyre_temp_outer_c;
        frame.tyre_laps_remaining = self.engineer.stats.tyre_laps_remaining;
        frame.stint_laps = self.engineer.stats.stint_laps.max(0) as u32;

        // The mapping is one place a frame goes and not the only one, so a
        // missing writer skips the writer rather than the whole publish — the
        // same shape as `publish_overlay_idle`. Written the other way round,
        // an absent mapping silenced the UDP feed as well, which is exactly
        // the case of somebody who wants the feed and not the in-game panel.
        if let Some(writer) = self.overlay_writer.as_mut() {
            writer.publish(&frame);
        }
        // Everything that is not the in-game panel gets the same frame: a
        // second front end here, a friend watching from another machine, a
        // relay for a championship. The core computes once and hands it out.
        self.broadcast.publish(&frame);
    }

    /// Tell the overlay we are going away, so it hides at once rather than
    /// holding the last frame until its liveness timeout expires.
    pub fn shutdown_overlay(&mut self) {
        if let Some(writer) = self.overlay_writer.as_mut() {
            writer.publish_shutdown();
        }
        self.broadcast.shutdown();
    }

    pub fn disconnect(&mut self) {
        self.source = None;
        // Left behind, the screens would keep drawing the last numbers of a
        // session that has ended as though the car were still on track.
        self.reading = None;
        self.is_connected = false;
        self.session_info = SessionInfo::default();
        self.recommendations.clear();
        self.car_history.clear();
        self.session_history.clear();
        self.current_lap_cars.clear();
        self.current_lap_sessions.clear();
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
        if self.source.is_none() {
            let backend = self
                .game
                .backend()
                .ok_or("this build cannot read the selected game")?;
            let mut source = (backend.connect)()?;
            // One reading before anything is believed: connecting only proves
            // the mappings exist, and on Linux they can exist with nothing in
            // them yet — the bridge creates them before the game has published.
            let Some(reading) = source.poll() else {
                return Err(format!("connected to {} but read nothing", self.game.name).into());
            };

            let fixed = &reading.fixed;
            self.session_info.car_name = fixed.car_model.clone();
            self.session_info.track_name = fixed.track.clone();
            self.session_info.track_config = fixed.track_config.clone();
            self.session_info.player_name = fixed.driver_name.clone();
            self.session_info.max_rpm = fixed.max_rpm;
            self.session_info.max_fuel = fixed.max_fuel_litres;

            let specs = self
                .content_manager
                .get_car_specs(&self.session_info.car_name)
                .cloned();
            let rec = self.record_manager.get_or_calculate_record(
                &self.session_info.car_name,
                &self.session_info.track_name,
                &self.session_info.track_config,
                specs.as_ref(),
                fixed.track_length_m,
            );
            self.analyzer.set_world_record(rec);
            // Stamped onto every lap from here, so a corner report can say
            // "14 m later on the brakes" rather than a fraction of a lap.
            self.analyzer.set_track_length(fixed.track_length_m);
            self.is_connected = true;

            self.reading = Some(reading);
            self.source = Some(source);
        }
        Ok(())
    }

    pub fn apply_config(&mut self) {
        let cap = self.config.history_size;
        self.car_history.set_capacity(cap);
        self.session_history.set_capacity(cap);
        self.engineer.update_config(&self.config);
    }

    pub fn update_live_buffers(&mut self, car: &Car, session: &Session) {
        self.car_history.push(*car);
        self.session_history.push(*session);
    }

    pub fn update_session_info(&mut self, session: &Session) {
        self.session_info.lap_count = session.completed_laps;
        self.session_info.session_time_left = session.session_time_left_ms;
        // The table this used to hold moved into the game folder, which is
        // where knowing that a race is session number three belongs.
        self.session_info.session_type = session.kind.label().to_string();
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

    /// The panel goes into the game folder without anyone asking, and it has
    /// to go in on a game that appeared *after* the application started.
    ///
    /// The install used to run once, while `AppState` was being built, and
    /// never again — so a first attempt that found nothing, or could not
    /// write, left the panel missing for the whole session however long the
    /// application then sat there with the game running. This is the retry,
    /// driven the way the running application drives it.
    #[test]
    fn a_game_that_appears_later_still_gets_the_panel() {
        let game = std::env::temp_dir().join("acpe-late-game");
        let _ = std::fs::remove_dir_all(&game);
        std::fs::create_dir_all(game.join("apps").join("lua")).expect("game folder");

        let mut app = AppState::new();
        app.config.ac_install_path = game.clone();
        // The panel is one game's, so this test is about that game — said
        // here rather than inherited from whatever this machine's
        // configuration happens to select, which is a state a test must not
        // depend on.
        app.game = ac_core::games::registry::chosen("assetto_corsa");

        // Nothing there yet, which is what the first attempt saw.
        app.refresh_overlay_report();
        assert!(
            !app.overlay_report.current,
            "the folder was just created empty"
        );

        app.ensure_overlay_installed();

        let panel = game.join("apps").join("lua").join("ac_pro_engineer");
        assert!(
            panel.join("ac_pro_engineer.lua").is_file(),
            "the entry point is what CSP looks for"
        );
        assert!(
            panel.join("acpe").join("frame.lua").is_file(),
            "and the modules it requires"
        );
        assert!(app.overlay_report.current, "the report agrees afterwards");

        // And it is idempotent: a second pass writes nothing and says nothing.
        app.overlay_install_status.clear();
        app.ensure_overlay_installed();
        assert!(
            app.overlay_install_status.is_empty(),
            "an up-to-date panel is not news: {}",
            app.overlay_install_status
        );

        let _ = std::fs::remove_dir_all(&game);
    }

    #[test]
    fn demo_mode_executes_full_telemetry_pipeline() {
        let mut app = AppState::new();
        app.is_demo_mode = true;
        // The demo tick fills the car and the session in; what it needs to
        // find already there is a reading to fill them into.
        app.reading = Some(Reading {
            capabilities: Capabilities::all(),
            ..Default::default()
        });
        app.stage = AppStage::Running;

        assert_eq!(app.car_history.len(), 0);

        for _ in 0..10 {
            app.tick();
        }

        assert_eq!(app.car_history.len(), 10);
        assert_eq!(app.session_history.len(), 10);
        assert!(app.car_history.last().is_some_and(|p| p.speed_kmh > 0.0));
        assert_ne!(app.session_info.car_name, "");
        assert_ne!(app.session_info.track_name, "");
    }

    /// The tick is what tells the engineer which game it is reading, and
    /// nothing else does.
    ///
    /// Everything else about the capability flags is tested where the rules
    /// are; this is the wire between the game's answer and the engineer's ear.
    /// It is the one link whose failure is silent in the other direction — an
    /// engineer never told anything withholds every tyre verdict, which looks
    /// like a quiet session rather than like a bug.
    #[test]
    fn the_tick_tells_the_engineer_what_the_game_measures() {
        let mut app = AppState::new();

        let complete = Reading {
            capabilities: Capabilities::all(),
            ..Default::default()
        };
        app.process_tick_logic(complete);
        assert_eq!(app.engineer.capabilities(), Capabilities::all());

        // And it is the *reading* that decides, not a value latched once: a
        // game swapped underneath — reconnecting to another simulator — has to
        // narrow what the engineer will say.
        let partial = Capabilities {
            tyre_wear: false,
            ..Capabilities::all()
        };
        app.process_tick_logic(Reading {
            capabilities: partial,
            ..Default::default()
        });
        assert_eq!(app.engineer.capabilities(), partial);
    }

    /// The panel has to be reachable before a session starts.
    ///
    /// `tick` used to return before publishing anything whenever the
    /// application was on its launcher screen — which is where it sits for the
    /// minutes before a race — so a driver who opened the panel in the garage
    /// was told the application was not running while looking at it. The frame
    /// says the opposite of that now: the sequence advances, and CONNECTED
    /// stays clear so the panel knows not to draw zeroes as telemetry.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn the_launcher_still_publishes_a_frame() {
        use ac_core::overlay::frame::{OverlayFrame, flags};

        let mut app = AppState::new();
        app.overlay_writer =
            ac_core::overlay::shared_writer::OverlayWriter::open_named("acpe-test-idle").ok();
        let Some(path) = app.overlay_writer.as_ref().map(|w| w.backing_path()) else {
            eprintln!("no shared memory here; skipping");
            return;
        };
        app.stage = AppStage::Launcher;

        app.tick();
        app.tick();

        let bytes = std::fs::read(&path).expect("read the published frame");
        assert!(bytes.len() >= size_of::<OverlayFrame>());
        // SAFETY: the file is at least struct-sized and was written by
        // OverlayWriter from exactly this type. Unaligned because nothing
        // guarantees the buffer's alignment.
        let frame: OverlayFrame =
            unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const OverlayFrame) };

        assert_ne!(
            frame.sequence, 0,
            "a sequence of zero reads as never written, which is what the panel \
             was seeing"
        );
        assert!(
            !frame.has_flag(flags::CONNECTED),
            "there is no car on the launcher screen"
        );
        assert_eq!(frame.speed_kmh, 0.0, "and no telemetry to go with it");
    }

    /// The final split is derived from the lap time rather than read off the
    /// sector transition, because that transition races the lap-count
    /// increment and could land the split in the following lap.
    #[test]
    fn final_sector_is_derived_from_the_lap_time() {
        let mut app = AppState::new();
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
        let mut app = AppState::new();
        app.track_sector_count = 3;
        app.current_lap_sectors = [30_000, 0, 0];

        app.close_current_lap_sectors(95_500);

        assert_eq!(app.current_lap_sectors[2], 0);
    }

    /// An implausible remainder means the earlier splits came from a
    /// different lap, so it is discarded rather than recorded.
    #[test]
    fn final_sector_rejects_an_implausible_remainder() {
        let mut app = AppState::new();
        app.track_sector_count = 3;
        // The two known splits already exceed the lap time.
        app.current_lap_sectors = [60_000, 50_000, 0];

        app.close_current_lap_sectors(95_500);

        assert_eq!(app.current_lap_sectors[2], 0);
    }

    /// Two-sector mod tracks exist, and the reading says so.
    #[test]
    fn final_sector_honours_a_two_sector_track() {
        let mut app = AppState::new();
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

    /// The connection opens on telemetry that is there, even when the process
    /// table says nothing.
    ///
    /// Ignored because it needs a stand-in publishing pages, which no test can
    /// start for itself. Run it against one:
    ///
    /// ```text
    /// cp target/release/simulator /tmp/notagame && /tmp/notagame acc &
    /// cargo test -p ac_tui --lib reads_a_game_the_process_table_cannot_see -- --ignored --nocapture
    /// ```
    ///
    /// The name matters: `/tmp/notagame` is deliberately not one of the
    /// processes the registry watches for, which is the case this guards. Under
    /// Proton a game arrives as a Windows path through a launcher — ACC's
    /// `acc.exe` starts `AC2-Win64-Shipping.exe` — and a name this build has
    /// not been told about used to mean a driver sitting in the car reading
    /// "the game is not running".
    #[test]
    #[ignore = "needs a stand-in publishing shared memory; see the doc comment"]
    fn reads_a_game_the_process_table_cannot_see() {
        let mut app = AppState::new();
        app.apply_game(ac_core::games::registry::chosen(
            "assetto_corsa_competizione",
        ));
        app.stage = AppStage::Running;

        // Three ticks: one to connect, two to read. The watcher caches its
        // answer, and the point is that its answer is "no".
        for _ in 0..3 {
            app.tick();
        }

        assert!(
            app.is_connected,
            "the pages are there and parse as this game, so the connection opens"
        );
        let reading = app.reading.as_ref().expect("a reading arrived");
        assert!(
            reading.car.speed_kmh > 0.0,
            "and it is telemetry, not an empty mapping: {} km/h",
            reading.car.speed_kmh
        );
        assert!(
            app.is_game_running,
            "a car on track is proof the game is up, whatever the process table said"
        );
        println!(
            "connected: {} km/h, gear {}, {:.0} °C tyres",
            reading.car.speed_kmh,
            reading.car.gear,
            reading.car.avg_tyre_temp_c(0)
        );
    }

    /// The panel is one game's, and the launcher stops offering it on the
    /// others.
    ///
    /// Without this an ACC driver is asked "Assetto Corsa and CSP are both
    /// here, shall I install it?" about a game that is Unreal Engine and has
    /// nothing to load a Custom Shaders Patch app with — and then gets a card
    /// reporting the panel as missing for the rest of the session.
    #[test]
    fn a_game_with_no_panel_is_not_offered_one() {
        let with_panel = ac_core::games::registry::chosen("assetto_corsa");
        let without = ac_core::games::registry::chosen("assetto_corsa_competizione");
        assert!(
            with_panel
                .backend()
                .is_some_and(|b| b.capabilities.in_game_panel),
            "Assetto Corsa is the game the panel is written for"
        );
        assert!(
            without
                .backend()
                .is_some_and(|b| !b.capabilities.in_game_panel),
            "Competizione has no Custom Shaders Patch"
        );

        let mut app = AppState::new();
        app.game = without;
        app.show_overlay_card = true;
        app.onboarding = OverlayOnboarding::Offer;
        assert!(!app.game_has_a_panel());

        // Switching to it puts both away; switching back does not force them
        // on, because that is the driver's setting rather than ours.
        //
        // `apply_game` rather than `select_game`: the latter writes the choice
        // to the configuration file, and a test has no business changing what
        // game somebody's application starts up as.
        app.game = with_panel;
        app.apply_game(without);
        assert!(!app.show_overlay_card, "the card went with the game");
        assert_eq!(app.onboarding, OverlayOnboarding::Done);

        // And the install is a no-op rather than a write into a game folder
        // that cannot use it.
        app.overlay_install_status.clear();
        app.ensure_overlay_installed();
        assert!(
            app.overlay_install_status.is_empty(),
            "nothing was installed into a game with no panel: {}",
            app.overlay_install_status
        );
    }
}
