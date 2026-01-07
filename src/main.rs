mod ac_structs;
mod memory;
mod setup_manager;
mod process;
mod engineer;
mod ui;
mod analyzer;
mod config;
mod session_info;

use std::{io, time::{Duration, Instant}};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use crate::memory::SharedMemory;
use crate::ac_structs::{AcPhysics, AcGraphics, AcStatic, read_ac_string};
use crate::process::is_process_running;
use crate::setup_manager::SetupManager;
use crate::engineer::{Engineer, Recommendation};
use crate::analyzer::{TelemetryAnalyzer, AnalysisResult};
use crate::config::AppConfig;
use crate::ui::{UIState, UIRenderer};
use crate::session_info::SessionInfo;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppTab {
    Dashboard,
    Telemetry,
    Engineer,
    Setup,
    Analysis,
    Strategy,
    Settings,
}

pub struct AppState {
    pub physics_mem: Option<SharedMemory<AcPhysics>>,
    pub graphics_mem: Option<SharedMemory<AcGraphics>>,
    pub static_mem: Option<SharedMemory<AcStatic>>,
    
    pub setup_manager: SetupManager,
    pub engineer: Engineer,
    pub analyzer: TelemetryAnalyzer,
    pub ui_state: UIState,
    
    pub is_game_running: bool,
    pub is_connected: bool,
    pub active_tab: AppTab,
    pub session_info: SessionInfo,
    
    pub physics_history: Vec<AcPhysics>,
    pub graphics_history: Vec<AcGraphics>,
    pub recommendations: Vec<Recommendation>,
    pub analysis_results: Vec<AnalysisResult>,
    
    pub lap_times: Vec<f32>,
    pub best_lap: f32,
    pub current_lap_start: Option<Instant>,
    pub last_update: Instant,
    
    pub config: AppConfig,
}

impl AppState {
    fn new() -> Self {
        let config = AppConfig::load().unwrap_or_default();
        
        Self {
            physics_mem: None,
            graphics_mem: None,
            static_mem: None,
            setup_manager: SetupManager::new(),
            engineer: Engineer::new(&config),
            analyzer: TelemetryAnalyzer::new(),
            ui_state: UIState::new(&config.theme),
            
            is_game_running: false,
            is_connected: false,
            active_tab: AppTab::Dashboard,
            session_info: SessionInfo {
                car_name: "-".to_string(),
                track_name: "-".to_string(),
                track_config: "-".to_string(),
                player_name: "-".to_string(),
                session_type: "-".to_string(),
                lap_count: 0,
                session_time_left: 0.0,
                max_rpm: 8000,
                max_fuel: 100.0,
            },
            
            physics_history: Vec::with_capacity(300),
            graphics_history: Vec::with_capacity(300),
            recommendations: Vec::new(),
            analysis_results: Vec::new(),
            
            lap_times: Vec::new(),
            best_lap: 0.0,
            current_lap_start: None,
            last_update: Instant::now(),
            
            config,
        }
    }
    
    fn tick(&mut self) {
        self.ui_state.update_blink(); // Update UI state here
        self.is_game_running = is_process_running("acs.exe");
        self.last_update = Instant::now();
        
        if !self.is_game_running {
            self.is_connected = false;
            return;
        }
        
        self.connect_memory();
        
        if self.is_connected {
            // Retrieve copies of the data to release the borrow on self
            let (phys, gfx) = if let (Some(phys_mem), Some(gfx_mem)) = (&self.physics_mem, &self.graphics_mem) {
                (*phys_mem.get(), *gfx_mem.get())
            } else {
                return;
            };

            // Now we can mutate self because the borrow from phys_mem/gfx_mem is over
            self.update_buffers(&phys, &gfx);
            self.update_session_info(&gfx);
            
            self.engineer.update(&phys, &gfx, &self.session_info);
            
            let active_setup = self.setup_manager.get_active_setup();
            self.recommendations = self.engineer.analyze_live(&phys, &gfx, active_setup.as_ref());
            
            if phys.packet_id % 120 == 0 {
                self.analysis_results = self.analyzer.analyze_session(
                    &self.physics_history,
                    &self.graphics_history,
                    &self.session_info,
                    active_setup.as_ref()
                );
            }
            
            if phys.packet_id % 60 == 0 {
                self.setup_manager.detect_current(
                    phys.fuel,
                    phys.brake_bias,
                    &phys.wheels_pressure,
                    &phys.tyre_temp_m
                );
            }
        }
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
                self.setup_manager.set_context(&self.session_info.car_name, &self.session_info.track_name);
                self.static_mem = Some(mem);
                self.is_connected = true;
            }
        }
    }
    
    fn update_buffers(&mut self, phys: &AcPhysics, gfx: &AcGraphics) {
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

fn main() -> Result<(), anyhow::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    let mut app = AppState::new();
    let renderer = UIRenderer::new();
    
    loop {
        app.tick();
        
        terminal.draw(|f| {
            renderer.render(f, &app);
        })?;
        
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => break,
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                        
                        (KeyCode::Char('1'), _) | (KeyCode::F(1), _) => app.active_tab = AppTab::Dashboard,
                        (KeyCode::Char('2'), _) | (KeyCode::F(2), _) => app.active_tab = AppTab::Telemetry,
                        (KeyCode::Char('3'), _) | (KeyCode::F(3), _) => app.active_tab = AppTab::Engineer,
                        (KeyCode::Char('4'), _) | (KeyCode::F(4), _) => app.active_tab = AppTab::Setup,
                        (KeyCode::Char('5'), _) | (KeyCode::F(5), _) => app.active_tab = AppTab::Analysis,
                        (KeyCode::Char('6'), _) | (KeyCode::F(6), _) => app.active_tab = AppTab::Strategy,
                        (KeyCode::Char('7'), _) | (KeyCode::F(7), _) => app.active_tab = AppTab::Settings,
                        
                        (KeyCode::Tab, KeyModifiers::NONE) => {
                            app.active_tab = match app.active_tab {
                                AppTab::Dashboard => AppTab::Telemetry,
                                AppTab::Telemetry => AppTab::Engineer,
                                AppTab::Engineer => AppTab::Setup,
                                AppTab::Setup => AppTab::Analysis,
                                AppTab::Analysis => AppTab::Strategy,
                                AppTab::Strategy => AppTab::Settings,
                                AppTab::Settings => AppTab::Dashboard,
                            };
                        }
                        (KeyCode::Tab, KeyModifiers::SHIFT) => {
                            app.active_tab = match app.active_tab {
                                AppTab::Dashboard => AppTab::Settings,
                                AppTab::Telemetry => AppTab::Dashboard,
                                AppTab::Engineer => AppTab::Telemetry,
                                AppTab::Setup => AppTab::Engineer,
                                AppTab::Analysis => AppTab::Setup,
                                AppTab::Strategy => AppTab::Analysis,
                                AppTab::Settings => AppTab::Strategy,
                            };
                        }
                        
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {
                    // Auto-resize handled in UI
                }
                _ => {}
            }
        }
    }
    
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    
    app.config.save().ok();
    
    Ok(())
}