pub mod keys;
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
    pub mem: Option<ac_core::games::assetto_corsa::AssettoCorsa>,
    pub setup_manager: SetupManager,
    pub content_manager: ContentManager,
    pub record_manager: RecordManager,
    pub updater: Updater,
    pub discord: DiscordClient,
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
    pub mock_physics: Option<AcPhysics>,
    pub mock_graphics: Option<AcGraphics>,
    pub mock_static: Option<AcStatic>,
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

        let setup_manager = SetupManager::new();
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
                        ac_core::games::assetto_corsa::GAME_ID,
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
        // they have left it on.
        if state.config.overlay.onboarding_done {
            state.show_overlay_card = state.config.overlay.startup_card;
        } else {
            state.onboarding = OverlayOnboarding::Offer;
        }

        // A bridge that cannot serve the overlay overrides "do not show this at
        // startup". That preference means "stop telling me things are fine";
        // it cannot reasonably mean "stay quiet while the panel is broken", and
        // the alternative is a driver hunting through the game for a fault that
        // is not there.
        if !state.bridge_status.is_workable()
            && state.overlay_report.current
            && state.onboarding == OverlayOnboarding::Done
        {
            state.show_overlay_card = true;
        }
        state
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
            self.mem.as_ref().map(|mem| mem.graphics())
        }
    }

    pub fn ac_physics(&self) -> Option<&AcPhysics> {
        if let Some(ref mock) = self.mock_physics {
            Some(mock)
        } else {
            self.mem.as_ref().map(|mem| mem.physics())
        }
    }

    pub fn ac_static(&self) -> Option<&AcStatic> {
        if let Some(ref mock) = self.mock_static {
            Some(mock)
        } else {
            self.mem.as_ref().map(|mem| mem.stat())
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
        let game_just_started = process_active && !self.is_game_running;
        self.is_game_running = process_active;

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

        if !process_active && self.is_connected {
            self.disconnect();
        } else if process_active
            && !self.is_connected
            && let Err(error) = self.connect_memory()
        {
            error!(error = ?error, "Cannot connect to shared memory");
        }

        if !self.is_connected {
            // AC running with nothing in its shared memory yet: the menus, the
            // loading screen, the seconds in the garage before the session
            // starts. The application is fine, and now says so.
            self.publish_overlay_idle();
            return;
        }

        let Some(mem) = self.mem.as_mut() else {
            self.publish_overlay_idle();
            return;
        };

        // A tick that reads nothing is the game being closed or between
        // sessions, which is a state and not a failure — the panel is told the
        // application is alive and has no car, which is the distinction v0.3.5
        // added.
        if !ac_core::games::Source::poll(mem) {
            self.publish_overlay_idle();
            return;
        }

        let (phys, gfx, stat) = (*mem.physics(), *mem.graphics(), *mem.stat());

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
    fn publish_overlay_frame(&mut self, phys: &AcPhysics, gfx: &AcGraphics) {
        use ac_core::overlay::frame::flags;

        let mut frame = self.overlay_frame_shell();

        // Computed before the writer is borrowed: `best_sectors` reads the
        // analyser, and the writer borrow is mutable and covers the rest of
        // this function.
        let best_sectors = self.best_sectors();

        let Some(writer) = self.overlay_writer.as_mut() else {
            return;
        };

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
        frame.tyre_temp_inner_c = phys.tyre_temp_i;
        frame.tyre_temp_outer_c = phys.tyre_temp_o;
        frame.tyre_laps_remaining = self.engineer.stats.tyre_laps_remaining;
        frame.stint_laps = self.engineer.stats.stint_laps.max(0) as u32;

        writer.publish(&frame);
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
            let mut mem = ac_core::games::assetto_corsa::AssettoCorsa::connect()?;
            // One reading before anything is believed: connecting only proves
            // the mappings exist, and on Linux they can exist with nothing in
            // them yet — the bridge creates them before the game has published.
            if !ac_core::games::Source::poll(&mut mem) {
                return Err("connected to Assetto Corsa but read nothing".into());
            }

            let st = mem.stat();
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
        app.mock_static = Some(ac_core::ac_structs::AcStatic::default());
        app.stage = AppStage::Running;

        assert_eq!(app.physics_history.len(), 0);

        for _ in 0..10 {
            app.tick();
        }

        assert_eq!(app.physics_history.len(), 10);
        assert_eq!(app.graphics_history.len(), 10);
        assert!(
            app.physics_history
                .last()
                .is_some_and(|p| p.speed_kmh > 0.0)
        );
        assert_ne!(app.session_info.car_name, "");
        assert_ne!(app.session_info.track_name, "");
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

    /// Two-sector mod tracks exist, and AcStatic says so.
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
}
