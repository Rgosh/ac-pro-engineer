use ac_core::config::Language;
use ac_core::overlay::OverlayMode;
use ac_core::updater::UpdateStatus;
use ac_tui::platform;
use ac_tui::ui::{UIRenderer, UIState};
use ac_tui::{setup_logging, AppLogLevel, AppStage, AppState, AppTab, SafeLock};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetSize,
    },
};
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info};

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

#[derive(Parser, Debug)]
#[command(version, about)]
struct AppArgs {
    #[arg(short, long, conflicts_with = "log-level", conflicts_with = "log")]
    silent: bool,

    #[arg(short, long, id = "log-level", conflicts_with = "silent")]
    log_level: Option<AppLogLevel>,

    #[arg(long, conflicts_with = "silent")]
    log: Option<PathBuf>,

    #[arg(long = "overlay--test--d")]
    overlay_test_d: bool,

    #[arg(long = "overlay--test-vr")]
    overlay_test_vr: bool,

    #[arg(short = 'd', long = "demo", help = "Run in live simulation mode with realistic telemetry data")]
    demo: bool,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    ac_core::crash_logger::init_crash_handler();
    let args = AppArgs::parse();

    let overlay_mode = if args.overlay_test_d {
        OverlayMode::StandaloneTest
    } else if args.overlay_test_vr {
        OverlayMode::VR
    } else {
        OverlayMode::External
    };

    if !args.silent {
        setup_logging(args.log.as_ref(), args.log_level.unwrap_or_default())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    info!("Starting application and connecting to telemetry...");

    #[cfg(target_os = "linux")]
    let _mem_bridge = platform::linux::SharedMemoryBridge::start().await?;

    #[cfg(target_os = "windows")]
    set_console_icon();

    let mut app = AppState::new(overlay_mode);
    if args.demo {
        app.enable_demo_simulation();
    }

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

    let renderer = UIRenderer::new();

    'outer: loop {
        if !args.demo {
            app.stage = AppStage::Launcher;
        }

        loop {
            let target_frame_time = Duration::from_millis(app.config.update_rate);
            let start = Instant::now();

            app.tick();

            app.overlay_manager.render_manual_state();

            terminal.draw(|f| renderer.render(f, &app))?;

            if event::poll(target_frame_time.saturating_sub(start.elapsed()))?
                && let Event::Key(key) = event::read()?
                && key.kind == event::KeyEventKind::Press
            {
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

                if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                    break 'outer;
                }

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
                        0 => {
                            app.stage = AppStage::Running;
                        }
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
                                    let result = app.updater.restart_and_apply(&new_file);
                                    if let Err(error) = result {
                                        error!(
                                            error = ?error,
                                            "Could not install an update"
                                        );
                                    }
                                }
                                UpdateStatus::Downloading(_) => {}
                                _ => {
                                    app.updater.download_selected();
                                }
                            }
                        }
                        6 => break 'outer,
                        _ => {}
                    },
                    KeyCode::Char('q')
                    | KeyCode::Char('Q')
                    | KeyCode::Char('й')
                    | KeyCode::Char('Й')
                    | KeyCode::Esc => break 'outer,
                    _ => {}
                }
            }

            if app.stage == AppStage::Running {
                break;
            }
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let app_arc = Arc::new(Mutex::new(app));
        let bg_app = Arc::clone(&app_arc);

        let bg_handle = std::thread::spawn(move || {
            loop {
                if rx.try_recv().is_ok() {
                    break;
                }
                let rate = {
                    let app_lock = bg_app.safe_lock();
                    app_lock.config.update_rate
                };
                std::thread::sleep(Duration::from_millis(rate));
                bg_app.safe_lock().tick();
            }
        });

        loop {
            let rate = {
                let app_lock = app_arc.safe_lock();
                app_lock.config.update_rate
            };
            let start = Instant::now();

            {
                let mut app_lock = app_arc.safe_lock();

                app_lock.overlay_manager.render_manual_state();

                terminal.draw(|f| {
                    renderer.render(f, &app_lock);
                    if app_lock.show_overlay_menu {
                        ac_tui::ui::overlay::render(f, f.size(), &app_lock);
                    }
                })?;
            }

            if event::poll(Duration::from_millis(rate).saturating_sub(start.elapsed()))?
                && let Event::Key(key) = event::read()?
                && key.kind == event::KeyEventKind::Press
            {
                let mut app_lock = app_arc.safe_lock();

                if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                    app_lock.stage = AppStage::Launcher;
                    app_lock.disconnect();
                    continue;
                }

                if key.code == KeyCode::F(10) {
                    app_lock.overlay_manager.toggle();
                    let active = app_lock.overlay_manager.is_active;
                    info!("Master overlay toggled to {}", active);
                    continue;
                }

                if key.code == KeyCode::F(11) {
                    app_lock.show_overlay_menu = !app_lock.show_overlay_menu;
                    continue;
                }

                if app_lock.show_overlay_menu {
                    match key.code {
                        KeyCode::Esc => {
                            app_lock.show_overlay_menu = false;
                        }
                        KeyCode::Up => {
                            if app_lock.overlay_menu_selection > 0 {
                                app_lock.overlay_menu_selection -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if app_lock.overlay_menu_selection < 1 {
                                app_lock.overlay_menu_selection += 1;
                            }
                        }
                        KeyCode::Enter => match app_lock.overlay_menu_selection {
                            0 => {
                                app_lock.overlay_manager.toggle();
                            }
                            1 => {
                                app_lock.overlay_manager.toggle_unlocked();
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                    continue;
                }

                if app_lock.show_help {
                    match key.code {
                        KeyCode::Esc
                        | KeyCode::Char('?')
                        | KeyCode::Char('q')
                        | KeyCode::Char('Q') => {
                            app_lock.show_help = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('?') | KeyCode::Char(',') | KeyCode::F(1) => {
                        app_lock.show_help = true;
                    }
                    KeyCode::Esc
                    | KeyCode::Char('q')
                    | KeyCode::Char('Q')
                    | KeyCode::Char('й')
                    | KeyCode::Char('Й') => {
                        app_lock.stage = AppStage::Launcher;
                        if !app_lock.is_demo_mode {
                            app_lock.disconnect();
                        }
                        continue;
                    }
                    KeyCode::Char('l') | KeyCode::Char('L') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app_lock.config.language = match app_lock.config.language {
                            Language::English => Language::Russian,
                            Language::Russian => Language::English,
                        };
                        let _res = app_lock.config.save();
                    }
                    KeyCode::Tab => {
                        app_lock.active_tab = app_lock.active_tab.next();
                    }
                    KeyCode::BackTab => {
                        app_lock.active_tab = app_lock.active_tab.previous();
                    }
                    KeyCode::Char('1') => app_lock.active_tab = AppTab::Dashboard,
                    KeyCode::Char('2') => app_lock.active_tab = AppTab::Telemetry,
                    KeyCode::Char('3') => app_lock.active_tab = AppTab::Engineer,
                    KeyCode::Char('4') => app_lock.active_tab = AppTab::Setup,
                    KeyCode::Char('5') => app_lock.active_tab = AppTab::Analysis,
                    KeyCode::Char('6') => app_lock.active_tab = AppTab::Strategy,
                    KeyCode::Char('7') => app_lock.active_tab = AppTab::Ffb,
                    KeyCode::Char('8') => app_lock.active_tab = AppTab::Settings,
                    KeyCode::Char('9') => app_lock.active_tab = AppTab::Guide,
                    _ => match app_lock.active_tab {
                        AppTab::Settings => {
                            let AppState { ui_state, config, .. } = &mut *app_lock;
                            ui_state.settings.handle_input(key.code, config);
                        }
                        AppTab::Engineer => match key.code {
                            KeyCode::Left => app_lock.ui_state.engineer.prev_tab(),
                            KeyCode::Right => app_lock.ui_state.engineer.next_tab(),
                            KeyCode::Up => app_lock.ui_state.engineer.prev_lap(),
                            KeyCode::Down => {
                                let total = app_lock.analyzer.laps.len();
                                app_lock.ui_state.engineer.next_lap(total);
                            }
                            _ => {}
                        },
                        AppTab::Guide => match key.code {
                            KeyCode::Up => {
                                let current = app_lock.ui_state.guide_list_state.selected().unwrap_or(0);
                                if current > 0 {
                                    app_lock.ui_state.guide_list_state.select(Some(current - 1));
                                }
                            }
                            KeyCode::Down => {
                                let current = app_lock.ui_state.guide_list_state.selected().unwrap_or(0);
                                if current < 15 {
                                    app_lock.ui_state.guide_list_state.select(Some(current + 1));
                                }
                            }
                            _ => {}
                        },
                        AppTab::Analysis => {
                            match key.code {
                                KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('ы') | KeyCode::Char('Ы') => {
                                    if let Some(best) = app_lock.analyzer.best_lap_index
                                        && let Some(lap) = app_lock.analyzer.laps.get(best).cloned()
                                    {
                                        app_lock.ui_state.analysis.save_lap_data(&lap);
                                    }
                                }
                                KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Char('д') | KeyCode::Char('Д') => {
                                    app_lock.ui_state.analysis.toggle_load_menu();
                                }
                                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('с') | KeyCode::Char('С') => {
                                    app_lock.ui_state.analysis.toggle_compare();
                                }
                                KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::Char('у') | KeyCode::Char('У') => {
                                    let sel = app_lock.ui_state.analysis.selected_lap_index.min(app_lock.analyzer.laps.len().saturating_sub(1));
                                    if let Some(lap) = app_lock.analyzer.laps.get(sel) {
                                        let export_path = app_lock.config.resolve_data_path().join(format!("exports/lap_{}_export.csv", lap.lap_number + 1));
                                        if let Ok(p) = ac_core::analyzer::export_lap_to_csv(lap, &export_path) {
                                            app_lock.ui_state.analysis.set_status(format!("Exported CSV: {}", p.display()));
                                        }
                                    }
                                }
                                KeyCode::Left => app_lock.ui_state.analysis.prev_tab(),
                                KeyCode::Right => app_lock.ui_state.analysis.next_tab(),
                                KeyCode::Up => {
                                    let laps_len = app_lock.analyzer.laps.len();
                                    app_lock.ui_state.analysis.menu_up(laps_len);
                                }
                                KeyCode::Down => {
                                    let laps_len = app_lock.analyzer.laps.len();
                                    app_lock.ui_state.analysis.menu_down(laps_len);
                                }
                                KeyCode::Enter => {
                                    let AppState { ui_state, analyzer, .. } = &mut *app_lock;
                                    ui_state.analysis.load_selected_file(analyzer);
                                }
                                _ => {}
                            }
                        }
                        AppTab::Setup => {
                            match key.code {
                                KeyCode::Up => {
                                    let current = app_lock.ui_state.setup_list_state.selected().unwrap_or(0);
                                    if current > 0 {
                                        app_lock.ui_state.setup_list_state.select(Some(current - 1));
                                    }
                                }
                                KeyCode::Down => {
                                    let current = app_lock.ui_state.setup_list_state.selected().unwrap_or(0);
                                    let total = app_lock.setup_manager.setups.safe_lock().len();
                                    if total > 0 && current + 1 < total {
                                        app_lock.ui_state.setup_list_state.select(Some(current + 1));
                                    }
                                }
                                KeyCode::Char('b') | KeyCode::Char('B') | KeyCode::Char('и') | KeyCode::Char('И') => {
                                    let mut active = app_lock.setup_manager.browser_active.safe_lock();
                                    *active = !*active;
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    },
                }
            }

            let stage = app_arc.safe_lock().stage;
            if stage == AppStage::Launcher {
                let _ = tx.send(());
                let _unused = bg_handle.join();
                break;
            }
        }

        app = match Arc::try_unwrap(app_arc) {
            Ok(mutex) => mutex.into_inner().unwrap_or_else(|e| e.into_inner()),
            Err(arc) => std::mem::take(&mut *arc.safe_lock()),
        };
    }

    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    app.record_manager.save();

    Ok(())
}
