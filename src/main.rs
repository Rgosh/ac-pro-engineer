mod ac_structs;
mod analyzer;
mod config;
mod content_manager;
mod discord;
mod engineer;
mod memory;
mod process;
mod records;
mod session_info;
mod setup_manager;
mod ui;
mod updater;

use crate::ac_structs::{read_ac_string, AcGraphics, AcPhysics, AcStatic};
use crate::analyzer::{AnalysisResult, TelemetryAnalyzer};
use crate::config::{AppConfig, Language};
use crate::content_manager::ContentManager;
use crate::discord::DiscordClient;
use crate::engineer::{Engineer, Recommendation};
use crate::memory::SharedMemory;
use crate::process::is_process_running;
use crate::records::RecordManager;
use crate::session_info::SessionInfo;
use crate::setup_manager::SetupManager;
use crate::ui::{UIRenderer, UIState};
use crate::updater::{UpdateStatus, Updater};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetSize,
    },
};
use ratatui::prelude::*;
use std::{
    io,
    sync::Mutex,
    time::{Duration, Instant},
};

trait SafeLock<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> SafeLock<T> for Mutex<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
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

pub struct AppState {
    pub physics_mem: Option<SharedMemory<AcPhysics>>,
    pub graphics_mem: Option<SharedMemory<AcGraphics>>,
    pub static_mem: Option<SharedMemory<AcStatic>>,

    pub setup_manager: SetupManager,
    pub content_manager: ContentManager,
    pub record_manager: RecordManager,
    pub updater: Updater,
    pub discord: DiscordClient,

    pub engineer: Engineer,
    pub analyzer: TelemetryAnalyzer,
    pub ui_state: UIState,

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
    pub show_help: bool,
}

impl AppState {
    fn new() -> Self {
        let mut config = AppConfig::load().unwrap_or_default();
        let mut show_success = false;
        let is_first_run = config.last_run_version == "0.0.0" || config.last_run_version.is_empty();

        if config.last_run_version != crate::updater::CURRENT_VERSION {
            if !is_first_run {
                show_success = true;
            }
            config.last_run_version = crate::updater::CURRENT_VERSION.to_string();

            let _res = config.save();
        }

        Self {
            physics_mem: None,
            graphics_mem: None,
            static_mem: None,
            setup_manager: SetupManager::new(),
            content_manager: ContentManager::new(),
            record_manager: RecordManager::new(),
            updater: Updater::new(),
            discord: DiscordClient::new(),

            engineer: Engineer::new(&config),
            analyzer: TelemetryAnalyzer::new(),
            ui_state: UIState::new(),
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
        }
    }

    fn tick(&mut self) {
        self.ui_state.update_blink();
        let delta = self.engineer.stats.current_delta;
        self.discord
            .update(self.is_connected, &self.session_info, delta);

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
        } else if process_active && !self.is_connected {
            self.connect_memory();
        }

        if !self.is_connected {
            return;
        }

        let (phys, gfx) =
            if let (Some(phys_mem), Some(gfx_mem)) = (&self.physics_mem, &self.graphics_mem) {
                (*phys_mem.get(), *gfx_mem.get())
            } else {
                return;
            };

        self.update_live_buffers(&phys, &gfx);
        self.update_session_info(&gfx);
        self.engineer.update(&phys, &gfx, &self.session_info);

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
                    let track_len = self
                        .static_mem
                        .as_ref()
                        .map(|m| m.get().track_spline_length)
                        .unwrap_or(0.0);
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
    }

    fn disconnect(&mut self) {
        self.physics_mem = None;
        self.graphics_mem = None;
        self.static_mem = None;
        self.is_connected = false;
        self.session_info = SessionInfo::default();
        self.recommendations.clear();
    }

    fn connect_memory(&mut self) {
        if self.physics_mem.is_none() {
            if let Some(mem) = SharedMemory::<AcPhysics>::connect("Local\\acpmf_physics") {
                self.physics_mem = Some(mem);
            }
        }
        if self.physics_mem.is_some() && self.graphics_mem.is_none() {
            if let Some(mem) = SharedMemory::<AcGraphics>::connect("Local\\acpmf_graphics") {
                self.graphics_mem = Some(mem);
            }
        }
        if self.physics_mem.is_some() && self.static_mem.is_none() {
            if let Some(mem) = SharedMemory::<AcStatic>::connect("Local\\acpmf_static") {
                let st = mem.get();
                self.session_info.car_name = read_ac_string(&st.car_model);
                self.session_info.track_name = read_ac_string(&st.track);
                self.session_info.track_config = read_ac_string(&st.track_configuration);
                self.session_info.player_name = read_ac_string(&st.player_nick);
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

                self.static_mem = Some(mem);
                self.is_connected = true;
            }
        }
    }

    fn update_live_buffers(&mut self, phys: &AcPhysics, gfx: &AcGraphics) {
        if self.physics_history.len() >= 300 {
            self.physics_history.remove(0);
        }
        if self.graphics_history.len() >= 300 {
            self.graphics_history.remove(0);
        }
        self.physics_history.push(*phys);
        self.graphics_history.push(*gfx);
    }

    fn update_session_info(&mut self, gfx: &AcGraphics) {
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

#[cfg(target_os = "windows")]
fn set_console_icon() {
    use windows::core::PCWSTR;
    use windows::Win32::System::Console::GetConsoleWindow;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        LoadImageW, SendMessageW, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE,
        WM_SETICON,
    };

    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd.0 != 0 {
            if let Ok(hinstance) = GetModuleHandleW(None) {
                if let Ok(hicon) = LoadImageW(
                    hinstance,
                    PCWSTR(1 as *const u16),
                    IMAGE_ICON,
                    0,
                    0,
                    LR_DEFAULTSIZE,
                ) {
                    let icon_handle = HICON(hicon.0);
                    SendMessageW(
                        hwnd,
                        WM_SETICON,
                        windows::Win32::Foundation::WPARAM(ICON_SMALL as usize),
                        windows::Win32::Foundation::LPARAM(icon_handle.0),
                    );
                    SendMessageW(
                        hwnd,
                        WM_SETICON,
                        windows::Win32::Foundation::WPARAM(ICON_BIG as usize),
                        windows::Win32::Foundation::LPARAM(icon_handle.0),
                    );
                }
            }
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    #[cfg(target_os = "windows")]
    set_console_icon();

    enable_raw_mode()?;
    let mut stdout = io::stdout();

    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        SetSize(140, 40)
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppState::new();
    let renderer = UIRenderer::new();

    let mut last_tick = Instant::now();

    loop {
        let target_frame_time = Duration::from_millis(app.config.update_rate);
        let elapsed = last_tick.elapsed();
        if elapsed < target_frame_time {
            std::thread::sleep(target_frame_time - elapsed);
        }
        last_tick = Instant::now();

        app.tick();
        terminal.draw(|f| renderer.render(f, &app))?;

        if event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    if app.show_update_success {
                        if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                            app.show_update_success = false;
                        }
                        continue;
                    }

                    if app.show_first_run_prompt {
                        match key.code {
                            KeyCode::Left => app.first_run_selection = 0,
                            KeyCode::Right => app.first_run_selection = 1,
                            KeyCode::Enter => {
                                app.show_first_run_prompt = false;
                                if app.first_run_selection == 0 {
                                    app.stage = AppStage::Running;
                                    app.active_tab = AppTab::Guide;
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if key.code == KeyCode::Char('h') || key.code == KeyCode::Char('H') {
                        if app.stage == AppStage::Running {
                            app.show_help = !app.show_help;
                            continue;
                        }
                    }

                    if app.show_help {
                        app.show_help = false;
                        continue;
                    }

                    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                        break;
                    }

                    if key.code == KeyCode::F(10) {
                        app.ui_state.overlay_mode = !app.ui_state.overlay_mode;
                        continue;
                    }

                    if app.stage == AppStage::Launcher {
                        match key.code {
                            KeyCode::Up => {
                                if app.launcher_selection > 0 {
                                    app.launcher_selection -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if app.launcher_selection < 6 {
                                    app.launcher_selection += 1;
                                }
                            }

                            KeyCode::Left => {
                                if app.launcher_selection == 2 {
                                    app.config.language = match app.config.language {
                                        Language::English => Language::Russian,
                                        Language::Russian => Language::English,
                                    };
                                    let _res = app.config.save();
                                } else if app.launcher_selection == 5 {
                                    app.updater.prev_version();
                                }
                            }
                            KeyCode::Right => {
                                if app.launcher_selection == 2 {
                                    app.config.language = match app.config.language {
                                        Language::English => Language::Russian,
                                        Language::Russian => Language::English,
                                    };
                                    let _res = app.config.save();
                                } else if app.launcher_selection == 5 {
                                    app.updater.next_version();
                                }
                            }

                            KeyCode::Char('o') | KeyCode::Char('O') => {
                                let url = "https://www.overtake.gg/downloads/ac-pro-engineer-zero-lag-telemetry-setup-cloud-rust-powered.81695/";
                                #[cfg(target_os = "windows")]
                                {
                                    std::process::Command::new("cmd")
                                        .args(["/C", "start", url])
                                        .spawn()
                                        .ok();
                                }
                                #[cfg(not(target_os = "windows"))]
                                {
                                    if let Ok(mut child) =
                                        std::process::Command::new("xdg-open").arg(url).spawn()
                                    {
                                        child.wait().ok();
                                    }
                                }
                            }

                            KeyCode::Char('h') | KeyCode::Char('H') => {
                                app.config.review_banner_hidden = true;
                                let _res = app.config.save();
                            }

                            KeyCode::Enter => match app.launcher_selection {
                                0 => app.stage = AppStage::Running,
                                1 => {
                                    app.stage = AppStage::Running;
                                    app.active_tab = AppTab::Settings;
                                }
                                2 => {
                                    app.config.language = match app.config.language {
                                        Language::English => Language::Russian,
                                        Language::Russian => Language::English,
                                    };
                                    let _res = app.config.save();
                                }
                                5 => {
                                    let current_status = app.updater.status.safe_lock().clone();
                                    match current_status {
                                        UpdateStatus::Downloaded(new_file) => {
                                            app.updater.restart_and_apply(&new_file);
                                        }
                                        UpdateStatus::Downloading(_) => {}
                                        _ => {
                                            app.updater.download_selected();
                                        }
                                    }
                                }
                                6 => break,
                                _ => {}
                            },
                            KeyCode::Char('q')
                            | KeyCode::Char('Q')
                            | KeyCode::Char('й')
                            | KeyCode::Char('Й')
                            | KeyCode::Esc => break,
                            _ => {}
                        }
                        continue;
                    }

                    if app.active_tab == AppTab::Analysis {
                        let menu_active = app.ui_state.analysis.load_menu.borrow().active;
                        if menu_active {
                            match key.code {
                                KeyCode::Up => app.ui_state.analysis.menu_up(),
                                KeyCode::Down => app.ui_state.analysis.menu_down(),
                                KeyCode::Enter => {
                                    app.ui_state.analysis.load_selected_file(&mut app.analyzer)
                                }
                                KeyCode::Esc
                                | KeyCode::Char('l')
                                | KeyCode::Char('L')
                                | KeyCode::Char('д')
                                | KeyCode::Char('Д') => {
                                    app.ui_state.analysis.toggle_load_menu();
                                }
                                _ => {}
                            }
                            continue;
                        }
                    }

                    if app.active_tab == AppTab::Settings {
                        let was_editing = app.ui_state.settings.is_editing;
                        app.ui_state
                            .settings
                            .handle_input(key.code, &mut app.config);
                        if was_editing || app.ui_state.settings.is_editing {
                            continue;
                        }
                        match key.code {
                            KeyCode::Up
                            | KeyCode::Down
                            | KeyCode::Left
                            | KeyCode::Right
                            | KeyCode::Enter => continue,
                            _ => {}
                        }
                    }

                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _)
                        | (KeyCode::Char('Q'), _)
                        | (KeyCode::Char('й'), _)
                        | (KeyCode::Char('Й'), _) => {
                            app.stage = AppStage::Launcher;
                            app.disconnect();
                        }
                        (KeyCode::Esc, _) => {
                            app.stage = AppStage::Launcher;
                            app.disconnect();
                        }

                        (KeyCode::Char('1'), _) | (KeyCode::F(1), _) => {
                            app.active_tab = AppTab::Dashboard
                        }
                        (KeyCode::Char('2'), _) | (KeyCode::F(2), _) => {
                            app.active_tab = AppTab::Telemetry
                        }
                        (KeyCode::Char('3'), _) | (KeyCode::F(3), _) => {
                            app.active_tab = AppTab::Engineer
                        }
                        (KeyCode::Char('4'), _) | (KeyCode::F(4), _) => {
                            app.active_tab = AppTab::Setup
                        }
                        (KeyCode::Char('5'), _) | (KeyCode::F(5), _) => {
                            app.active_tab = AppTab::Analysis
                        }
                        (KeyCode::Char('6'), _) | (KeyCode::F(6), _) => {
                            app.active_tab = AppTab::Strategy
                        }
                        (KeyCode::Char('7'), _) | (KeyCode::F(7), _) => {
                            app.active_tab = AppTab::Ffb
                        }
                        (KeyCode::Char('8'), _) | (KeyCode::F(8), _) => {
                            app.active_tab = AppTab::Settings
                        }
                        (KeyCode::Char('9'), _) | (KeyCode::F(9), _) => {
                            app.active_tab = AppTab::Guide
                        }

                        (KeyCode::Char('l'), _)
                        | (KeyCode::Char('L'), _)
                        | (KeyCode::Char('д'), _)
                        | (KeyCode::Char('Д'), _) => {
                            if app.active_tab == AppTab::Analysis {
                                app.ui_state.analysis.toggle_load_menu();
                                continue;
                            }
                        }
                        (KeyCode::Char('s'), _)
                        | (KeyCode::Char('S'), _)
                        | (KeyCode::Char('ы'), _)
                        | (KeyCode::Char('Ы'), _) => {
                            if app.active_tab == AppTab::Analysis {
                                if let Some(idx) = app.ui_state.setup_list_state.selected() {
                                    if let Some(lap) = app.analyzer.laps.get(idx) {
                                        app.ui_state.analysis.save_lap_data(lap);
                                    }
                                }
                                continue;
                            }
                        }
                        (KeyCode::Char('c'), _)
                        | (KeyCode::Char('C'), _)
                        | (KeyCode::Char('с'), _)
                        | (KeyCode::Char('С'), _) => {
                            if app.active_tab == AppTab::Analysis {
                                app.ui_state.analysis.toggle_compare();
                                continue;
                            }
                        }

                        (KeyCode::Char('b'), _)
                        | (KeyCode::Char('B'), _)
                        | (KeyCode::Char('и'), _)
                        | (KeyCode::Char('И'), _)
                            if app.active_tab == AppTab::Setup =>
                        {
                            let mut active = app.setup_manager.browser_active.safe_lock();
                            *active = !*active;
                            if *active {
                                drop(active);
                                app.setup_manager.load_browser_car();
                            }
                        }

                        (KeyCode::Char('d'), _)
                        | (KeyCode::Char('D'), _)
                        | (KeyCode::Char('в'), _)
                        | (KeyCode::Char('В'), _)
                            if app.active_tab == AppTab::Setup =>
                        {
                            let is_browser = *app.setup_manager.browser_active.safe_lock();

                            if is_browser {
                                if let Some(setup) = app.setup_manager.get_browser_selected_setup()
                                {
                                    let target_car = app.setup_manager.get_browser_target_car();
                                    app.setup_manager.download_setup(&setup, &target_car);
                                }
                            } else if let Some(selected_idx) =
                                app.ui_state.setup_list_state.selected()
                            {
                                if let Some(setup) =
                                    app.setup_manager.get_setup_by_index(selected_idx)
                                {
                                    let target_car =
                                        app.setup_manager.current_car.safe_lock().clone();
                                    app.setup_manager.download_setup(&setup, &target_car);
                                }
                            }
                        }

                        (KeyCode::PageUp, _) => {
                            if app.active_tab == AppTab::Setup {
                                app.setup_manager.scroll_details(-1);
                            }
                        }
                        (KeyCode::PageDown, _) => {
                            if app.active_tab == AppTab::Setup {
                                app.setup_manager.scroll_details(1);
                            }
                        }

                        (KeyCode::Tab, KeyModifiers::NONE) => {
                            app.active_tab = app.active_tab.next();
                        }
                        (KeyCode::BackTab, _) => {
                            app.active_tab = app.active_tab.previous();
                        }
                        (KeyCode::Down, _) => {
                            if app.active_tab == AppTab::Analysis
                                || app.active_tab == AppTab::Engineer
                            {
                                let len = app.analyzer.laps.len();
                                if len > 0 {
                                    let cur = app
                                        .ui_state
                                        .setup_list_state
                                        .selected()
                                        .unwrap_or(len.saturating_sub(1));
                                    let next = if cur >= len - 1 { 0 } else { cur + 1 };
                                    app.ui_state.setup_list_state.select(Some(next));
                                }
                            } else if app.active_tab == AppTab::Guide {
                                let cur = app.ui_state.setup_list_state.selected().unwrap_or(0);
                                let next = if cur >= 15 { 0 } else { cur + 1 };
                                app.ui_state.setup_list_state.select(Some(next));
                            } else if app.active_tab == AppTab::Setup {
                                let is_browser = *app.setup_manager.browser_active.safe_lock();
                                if is_browser {
                                    let col = *app.setup_manager.browser_focus_col.safe_lock();
                                    if col == 0 {
                                        let mut idx = app.setup_manager.browser_car_idx.safe_lock();
                                        let len = app.setup_manager.manifest.safe_lock().len();
                                        if len > 0 {
                                            *idx = if *idx >= len - 1 { 0 } else { *idx + 1 };
                                        }
                                        drop(idx);
                                        app.setup_manager.load_browser_car();
                                    } else {
                                        let mut idx =
                                            app.setup_manager.browser_setup_idx.safe_lock();
                                        let len =
                                            app.setup_manager.browser_setups.safe_lock().len();
                                        if len > 0 {
                                            *idx = if *idx >= len - 1 { 0 } else { *idx + 1 };
                                        }
                                    }
                                } else {
                                    let len = app.setup_manager.get_setups().len();
                                    if len > 0 {
                                        let cur =
                                            app.ui_state.setup_list_state.selected().unwrap_or(0);
                                        let next = if cur >= len - 1 { 0 } else { cur + 1 };
                                        app.ui_state.setup_list_state.select(Some(next));
                                    }
                                }
                            }
                        }
                        (KeyCode::Up, _) => {
                            if app.active_tab == AppTab::Analysis
                                || app.active_tab == AppTab::Engineer
                            {
                                let len = app.analyzer.laps.len();
                                if len > 0 {
                                    let cur = app
                                        .ui_state
                                        .setup_list_state
                                        .selected()
                                        .unwrap_or(len.saturating_sub(1));
                                    let next = if cur == 0 { len - 1 } else { cur - 1 };
                                    app.ui_state.setup_list_state.select(Some(next));
                                }
                            } else if app.active_tab == AppTab::Guide {
                                let cur = app.ui_state.setup_list_state.selected().unwrap_or(0);
                                let next = if cur == 0 { 15 } else { cur - 1 };
                                app.ui_state.setup_list_state.select(Some(next));
                            } else if app.active_tab == AppTab::Setup {
                                let is_browser = *app.setup_manager.browser_active.safe_lock();
                                if is_browser {
                                    let col = *app.setup_manager.browser_focus_col.safe_lock();
                                    if col == 0 {
                                        let mut idx = app.setup_manager.browser_car_idx.safe_lock();
                                        let len = app.setup_manager.manifest.safe_lock().len();
                                        if len > 0 {
                                            *idx = if *idx == 0 { len - 1 } else { *idx - 1 };
                                        }
                                        drop(idx);
                                        app.setup_manager.load_browser_car();
                                    } else {
                                        let mut idx =
                                            app.setup_manager.browser_setup_idx.safe_lock();
                                        let len =
                                            app.setup_manager.browser_setups.safe_lock().len();
                                        if len > 0 {
                                            *idx = if *idx == 0 { len - 1 } else { *idx - 1 };
                                        }
                                    }
                                } else {
                                    let len = app.setup_manager.get_setups().len();
                                    if len > 0 {
                                        let cur =
                                            app.ui_state.setup_list_state.selected().unwrap_or(0);
                                        let next = if cur == 0 { len - 1 } else { cur - 1 };
                                        app.ui_state.setup_list_state.select(Some(next));
                                    }
                                }
                            }
                        }
                        (KeyCode::Left, _) => {
                            if app.active_tab == AppTab::Analysis {
                                app.ui_state.analysis.prev_tab();
                            } else if app.active_tab == AppTab::Engineer {
                                app.ui_state.engineer.prev_tab();
                            } else if app.active_tab == AppTab::Setup {
                                let is_browser = *app.setup_manager.browser_active.safe_lock();
                                if is_browser {
                                    let mut col = app.setup_manager.browser_focus_col.safe_lock();
                                    *col = if *col == 0 { 1 } else { 0 };
                                }
                            }
                        }
                        (KeyCode::Right, _) => {
                            if app.active_tab == AppTab::Analysis {
                                app.ui_state.analysis.next_tab();
                            } else if app.active_tab == AppTab::Engineer {
                                app.ui_state.engineer.next_tab();
                            } else if app.active_tab == AppTab::Setup {
                                let is_browser = *app.setup_manager.browser_active.safe_lock();
                                if is_browser {
                                    let mut col = app.setup_manager.browser_focus_col.safe_lock();
                                    *col = if *col == 0 { 1 } else { 0 };
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    app.record_manager.save();
    Ok(())
}
