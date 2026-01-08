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
use crate::config::{AppConfig, Language};
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
}

impl AppState {
    fn new() -> Self {
        // Загружаем конфиг с диска. Если язык был сохранен, он загрузится здесь.
        let config = AppConfig::load().unwrap_or_default();
        
        Self {
            physics_mem: None,
            graphics_mem: None,
            static_mem: None,
            setup_manager: SetupManager::new(),
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
        }
    }
    
    fn tick(&mut self) {
        self.ui_state.update_blink();
        
        if self.stage != AppStage::Running {
            return;
        }

        let process_active = is_process_running("acs.exe");
        self.is_game_running = process_active;
        
        if !process_active && self.is_connected {
            self.disconnect();
        } else if process_active && !self.is_connected {
            self.connect_memory();
        }

        if !self.is_connected {
            return;
        }
        
        let (phys, gfx) = if let (Some(phys_mem), Some(gfx_mem)) = (&self.physics_mem, &self.graphics_mem) {
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
        
        if completed_laps < self.current_lap_number {
            self.current_lap_physics.clear();
            self.current_lap_graphics.clear();
            self.current_lap_number = completed_laps;
        }

        if completed_laps > self.current_lap_number {
            let last_lap_time = gfx.i_last_time; 
            if last_lap_time > 10000 && !self.current_lap_physics.is_empty() {
                self.analyzer.process_lap(
                    self.current_lap_number, 
                    last_lap_time,
                    &self.current_lap_physics,
                    &self.current_lap_graphics
                );
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
            self.setup_manager.set_context(&self.session_info.car_name, &self.session_info.track_name);
        }
        let active_setup = self.setup_manager.get_active_setup();
        self.recommendations = self.engineer.analyze_live(&phys, &gfx, active_setup.as_ref());
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
                self.setup_manager.set_context(&self.session_info.car_name, &self.session_info.track_name);
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
            0 => "Booking".to_string(), 1 => "Practice".to_string(), 2 => "Qualifying".to_string(),
            3 => "Race".to_string(), 4 => "Hotlap".to_string(), 5 => "Time Attack".to_string(),
            6 => "Drift".to_string(), 7 => "Drag".to_string(), _ => "Unknown".to_string(),
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
        terminal.draw(|f| renderer.render(f, &app))?;
        
        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                        break;
                    }

                    if app.stage == AppStage::Launcher {
                        match key.code {
                            KeyCode::Up => if app.launcher_selection > 0 { app.launcher_selection -= 1; },
                            KeyCode::Down => if app.launcher_selection < 6 { app.launcher_selection += 1; },
                            
                            // Смена языка в лаунчере + МГНОВЕННОЕ СОХРАНЕНИЕ
                            KeyCode::Left | KeyCode::Right => {
                                if app.launcher_selection == 2 {
                                    app.config.language = match app.config.language {
                                        Language::English => Language::Russian,
                                        Language::Russian => Language::English,
                                    };
                                    // ВАЖНО: Сохраняем конфиг сразу, чтобы запомнить выбор
                                    app.config.save().ok(); 
                                }
                            },
                            
                            KeyCode::Enter => {
                                match app.launcher_selection {
                                    0 => app.stage = AppStage::Running, 
                                    1 => { 
                                        app.stage = AppStage::Running; 
                                        app.active_tab = AppTab::Settings; 
                                    },
                                    2 => { 
                                        app.config.language = match app.config.language {
                                            Language::English => Language::Russian,
                                            Language::Russian => Language::English,
                                        };
                                        app.config.save().ok(); // Сохраняем и по Enter
                                    }, 
                                    6 => break,
                                    _ => {}
                                }
                            },
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            _ => {}
                        }
                        continue; 
                    }

                    if app.active_tab == AppTab::Settings {
                        let was_editing = app.ui_state.settings.is_editing;
                        app.ui_state.settings.handle_input(key.code, &mut app.config);
                        
                        if was_editing || app.ui_state.settings.is_editing {
                            continue;
                        }
                        
                        match key.code {
                            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Tab => continue,
                            _ => {}
                        }
                    }

                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) => {
                            app.stage = AppStage::Launcher;
                            app.disconnect();
                        },
                        (KeyCode::Esc, _) => {
                             app.stage = AppStage::Launcher;
                             app.disconnect();
                        },
                        
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
                        },
                        (KeyCode::Down, _) => {
                            if app.active_tab == AppTab::Analysis {
                                let laps_len = app.analyzer.laps.len();
                                if laps_len > 0 {
                                    let current = app.ui_state.setup_list_state.selected().unwrap_or(0);
                                    let next = if current >= laps_len - 1 { 0 } else { current + 1 };
                                    app.ui_state.setup_list_state.select(Some(next));
                                }
                            } else if app.active_tab == AppTab::Setup {
                                let len = app.setup_manager.get_setups().len();
                                if len > 0 {
                                    let i = app.ui_state.setup_list_state.selected().unwrap_or(0);
                                    let n = if i >= len - 1 { 0 } else { i + 1 };
                                    app.ui_state.setup_list_state.select(Some(n));
                                }
                            }
                        },
                        (KeyCode::Up, _) => {
                             if app.active_tab == AppTab::Analysis {
                                let laps_len = app.analyzer.laps.len();
                                if laps_len > 0 {
                                    let current = app.ui_state.setup_list_state.selected().unwrap_or(0);
                                    let next = if current == 0 { laps_len - 1 } else { current - 1 };
                                    app.ui_state.setup_list_state.select(Some(next));
                                }
                            } else if app.active_tab == AppTab::Setup {
                                let len = app.setup_manager.get_setups().len();
                                if len > 0 {
                                    let i = app.ui_state.setup_list_state.selected().unwrap_or(0);
                                    let n = if i == 0 { len - 1 } else { i - 1 };
                                    app.ui_state.setup_list_state.select(Some(n));
                                }
                            }
                        },
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {}
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