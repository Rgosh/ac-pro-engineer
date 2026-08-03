use ac_core::config::Language;
use ac_core::overlay::OverlayMode;
use ac_core::updater::UpdateStatus;
// Only the Linux startup path reaches into `platform`.
#[cfg(target_os = "linux")]
use ac_tui::platform;
use ac_tui::ui::UIRenderer;
use ac_tui::{AppLogLevel, AppStage, AppState, AppTab, SafeLock, setup_logging};
use clap::Parser;
use crossterm::{
    event::{self, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, SetSize, enable_raw_mode},
};
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info};

#[cfg(target_os = "windows")]
fn set_console_icon() {
    use windows::Win32::System::Console::GetConsoleWindow;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE, LoadImageW, SendMessageW,
        WM_SETICON,
    };
    use windows::core::PCWSTR;

    unsafe {
        let hwnd = GetConsoleWindow();
        // MAKEINTRESOURCE(1): the icon's numeric resource id travels in the
        // pointer's address, so it is an integer address with no provenance,
        // not a pointer to anything. `without_provenance` says exactly that -
        // note that clippy's `dangling` suggestion would silently change the
        // id to align_of::<u16>() == 2.
        let icon_resource_id = PCWSTR(std::ptr::without_provenance(1));
        if hwnd.0 != 0
            && let Ok(hinstance) = GetModuleHandleW(None)
            && let Ok(hicon) = LoadImageW(
                hinstance,
                icon_resource_id,
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE,
            )
        {
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

#[derive(Parser, Debug)]
#[command(version, about)]
struct AppArgs {
    #[arg(short, long, conflicts_with = "log-level", conflicts_with = "log")]
    silent: bool,

    #[arg(short, long, id = "log-level", conflicts_with = "silent")]
    log_level: Option<AppLogLevel>,

    #[arg(long, conflicts_with = "silent")]
    log: Option<PathBuf>,

    #[arg(long = "overlay-test-d")]
    overlay_test_d: bool,

    #[arg(long = "overlay-test-vr")]
    overlay_test_vr: bool,

    #[arg(
        short = 'd',
        long = "demo",
        help = "Run in live simulation mode with realistic telemetry data"
    )]
    demo: bool,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    ac_core::crash_logger::init_crash_handler();
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        crossterm::terminal::disable_raw_mode().ok();
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )
        .ok();
        original_hook(panic_info);
    }));

    let args = AppArgs::parse();

    let overlay_mode = if args.overlay_test_d {
        OverlayMode::StandaloneTest
    } else if args.overlay_test_vr {
        OverlayMode::VR
    } else {
        OverlayMode::NativeDesktop
    };

    if !args.silent
        && let Err(error) = setup_logging(args.log.as_ref(), args.log_level.unwrap_or_default())
    {
        // Not being able to write a log is a reason to run without one, not a
        // reason to refuse to start. This used to abort before the TUI was
        // drawn, so a read-only working directory looked like the app being
        // broken.
        eprintln!("Continuing without a log file: {error}");
    }

    info!("Starting application and connecting to telemetry...");

    // Not fatal. `Command::spawn` returns NotFound when protontricks-launch
    // is not installed, and `?` here killed the app before the TUI existed —
    // so anyone running AC natively, through a different launcher, or just
    // wanting to review saved laps offline could not start it at all. The
    // launcher already has a "WAITING FOR SIMULATOR..." state for exactly
    // this situation.
    #[cfg(target_os = "linux")]
    let _mem_bridge = match platform::linux::SharedMemoryBridge::start().await {
        Ok(bridge) => Some(bridge),
        Err(error) => {
            eprintln!(
                "Could not start the shared-memory bridge: {error}\n                 Live telemetry will be unavailable; everything else still works."
            );
            None
        }
    };

    #[cfg(target_os = "windows")]
    set_console_icon();

    let mut app = AppState::new(overlay_mode);
    if args.demo {
        app.enable_demo_simulation();
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();

    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Ask for more room only when the terminal has less than the UI needs.
    // This used to be an unconditional SetSize(140, 40), which shrank the
    // window of anyone running maximised and never put it back on exit.
    // Terminals are free to ignore the request either way, which is why the
    // renderer has its own too-small guard rather than relying on this.
    const PREFERRED_COLS: u16 = 140;
    const PREFERRED_ROWS: u16 = 40;
    if let Ok((cols, rows)) = crossterm::terminal::size()
        && (cols < PREFERRED_COLS || rows < PREFERRED_ROWS)
    {
        execute!(
            stdout,
            SetSize(cols.max(PREFERRED_COLS), rows.max(PREFERRED_ROWS))
        )
        .ok();
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    struct TerminalGuard;
    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            crossterm::terminal::disable_raw_mode().ok();
            crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::event::DisableMouseCapture
            )
            .ok();
        }
    }
    let _guard = TerminalGuard;

    let renderer = UIRenderer::new();

    'outer: loop {
        if !args.demo {
            app.stage = AppStage::Launcher;
        }

        loop {
            let target_frame_time = Duration::from_millis(app.config.update_rate);
            let start = Instant::now();

            app.tick();

            // Sitting on the UPDATE item is the one moment the release list
            // matters, so it is where a check that failed at startup gets
            // another go. Debounced inside the updater, so holding the
            // selection here does not hammer the API.
            if app.launcher_selection == 5 {
                app.updater.recheck_if_stale();
            }

            app.overlay_manager.render_manual_state();

            terminal.draw(|f| renderer.render(f, &app))?;

            if event::poll(target_frame_time.saturating_sub(start.elapsed()))?
                && let Event::Key(key) = event::read()?
                && is_key_action(key.kind)
            {
                // Checked before the modal below. It used to sit after it, so
                // the first-run prompt every new user sees could not be
                // escaped: Ctrl+C, q and Esc all fell into the modal's
                // `_ => {}` and did nothing.
                if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                    break 'outer;
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
                        // Dismiss without opening the guide, which is what
                        // Esc means everywhere else in the app.
                        KeyCode::Esc => app.show_first_run_prompt = false,
                        _ => {}
                    }
                    continue;
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
                && is_key_action(key.kind)
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
                        // F1 included because the modal itself says
                        // "PRESS ESC, ?, Q, OR F1 TO CLOSE" in nine places,
                        // and F1 was the one key of the four that did nothing
                        // -- which reads as the modal being stuck.
                        KeyCode::Esc
                        | KeyCode::Char('?')
                        | KeyCode::Char('q')
                        | KeyCode::Char('Q')
                        | KeyCode::Char('й')
                        | KeyCode::Char('Й')
                        | KeyCode::F(1) => {
                            app_lock.show_help = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                // The analysis load menu owns Esc while it is open. Without
                // this the global Esc below fired first and dropped the whole
                // session back to the launcher, disconnecting on the way --
                // while the menu's own footer promised "ESC: Close".
                if app_lock.active_tab == AppTab::Analysis
                    && app_lock.ui_state.analysis.load_menu.borrow().active
                    && key.code == KeyCode::Esc
                {
                    app_lock.ui_state.analysis.load_menu.borrow_mut().active = false;
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
                    KeyCode::Char('l') | KeyCode::Char('L')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
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
                            let changed = {
                                let AppState {
                                    ui_state, config, ..
                                } = &mut *app_lock;
                                ui_state.settings.handle_input(key.code, config)
                            };
                            if changed {
                                // Nothing used to write these back, so every
                                // unit, threshold and target the user set was
                                // discarded on exit.
                                if let Err(error) = app_lock.config.save() {
                                    error!(error = ?error, "Could not save settings");
                                }
                                // And nothing re-read them either: history
                                // size and the engineer's thresholds only took
                                // effect on the next launch.
                                app_lock.apply_config();
                            }
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
                                let current =
                                    app_lock.ui_state.guide_list_state.selected().unwrap_or(0);
                                if current > 0 {
                                    app_lock.ui_state.guide_list_state.select(Some(current - 1));
                                }
                            }
                            KeyCode::Down => {
                                let current =
                                    app_lock.ui_state.guide_list_state.selected().unwrap_or(0);
                                if current < 15 {
                                    app_lock.ui_state.guide_list_state.select(Some(current + 1));
                                }
                            }
                            _ => {}
                        },
                        AppTab::Analysis => match key.code {
                            KeyCode::Char('s')
                            | KeyCode::Char('S')
                            | KeyCode::Char('ы')
                            | KeyCode::Char('Ы') => {
                                // The lap the user has selected, not the
                                // fastest one. This used to read
                                // `best_lap_index`, so selecting lap 3 and
                                // pressing S silently wrote lap 5 — while the
                                // export handler two arms down correctly used
                                // the selection.
                                let selected = app_lock.ui_state.analysis.selected_lap_index;
                                match app_lock.analyzer.laps.get(selected).cloned() {
                                    Some(lap) => app_lock.ui_state.analysis.save_lap_data(&lap),
                                    // Previously a silent no-op with nothing
                                    // on screen to say why.
                                    None => app_lock
                                        .ui_state
                                        .analysis
                                        .set_status("No lap to save".to_string()),
                                }
                            }
                            KeyCode::Char('l')
                            | KeyCode::Char('L')
                            | KeyCode::Char('д')
                            | KeyCode::Char('Д') => {
                                app_lock.ui_state.analysis.toggle_load_menu();
                            }
                            KeyCode::Char('c')
                            | KeyCode::Char('C')
                            | KeyCode::Char('с')
                            | KeyCode::Char('С') => {
                                app_lock.ui_state.analysis.toggle_compare();
                            }
                            KeyCode::Char('e')
                            | KeyCode::Char('E')
                            | KeyCode::Char('у')
                            | KeyCode::Char('У') => {
                                let sel = app_lock
                                    .ui_state
                                    .analysis
                                    .selected_lap_index
                                    .min(app_lock.analyzer.laps.len().saturating_sub(1));
                                if let Some(lap) = app_lock.analyzer.laps.get(sel) {
                                    let export_path = app_lock.config.resolve_data_path().join(
                                        format!("exports/lap_{}_export.csv", lap.lap_number + 1),
                                    );
                                    if let Ok(p) =
                                        ac_core::analyzer::export_lap_to_csv(lap, &export_path)
                                    {
                                        app_lock
                                            .ui_state
                                            .analysis
                                            .set_status(format!("Exported CSV: {}", p.display()));
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
                                let AppState {
                                    ui_state, analyzer, ..
                                } = &mut *app_lock;
                                ui_state.analysis.load_selected_file(analyzer);
                            }
                            _ => {}
                        },
                        AppTab::Setup => {
                            let in_browser = *app_lock.setup_manager.browser_active.safe_lock();
                            match key.code {
                                // 'B' toggles between the local setup list and
                                // the cloud browser, in either direction.
                                KeyCode::Char('b')
                                | KeyCode::Char('B')
                                | KeyCode::Char('и')
                                | KeyCode::Char('И') => {
                                    let mut active =
                                        app_lock.setup_manager.browser_active.safe_lock();
                                    *active = !*active;
                                }
                                _ if in_browser => handle_setup_browser_key(key.code, &app_lock),
                                KeyCode::Up => {
                                    let current =
                                        app_lock.ui_state.setup_list_state.selected().unwrap_or(0);
                                    if current > 0 {
                                        app_lock
                                            .ui_state
                                            .setup_list_state
                                            .select(Some(current - 1));
                                    }
                                }
                                KeyCode::Down => {
                                    let current =
                                        app_lock.ui_state.setup_list_state.selected().unwrap_or(0);
                                    let total = app_lock.setup_manager.setups.safe_lock().len();
                                    if total > 0 && current + 1 < total {
                                        app_lock
                                            .ui_state
                                            .setup_list_state
                                            .select(Some(current + 1));
                                    }
                                }
                                KeyCode::PageUp => app_lock.setup_manager.scroll_details(-1),
                                KeyCode::PageDown => app_lock.setup_manager.scroll_details(1),
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
            Err(_) => return Err(anyhow::anyhow!("Failed to unwrap AppState Arc")),
        };
    }

    app.record_manager.save();

    // The Settings tab describes this toggle as "save settings on exit", and
    // until now nothing acted on it either way. Settings are written as they
    // are edited now, so this is the belt to that braces — it also catches
    // the language and banner toggles made from the launcher.
    if app.config.auto_save
        && let Err(error) = app.config.save()
    {
        error!(error = ?error, "Could not save the config on exit");
    }

    Ok(())
}

/// Whether a key event should act, as opposed to being a key release.
///
/// Windows reports a held key as `Repeat` rather than a stream of `Press`
/// events, and only `Press` was accepted — so holding an arrow in the guide
/// or a lap list moved exactly one row and then stopped.
fn is_key_action(kind: event::KeyEventKind) -> bool {
    matches!(
        kind,
        event::KeyEventKind::Press | event::KeyEventKind::Repeat
    )
}

/// Keys for the Setup Cloud browser.
///
/// Nothing routed to these before: the Setup tab handled only Up/Down/B, so
/// the browser opened onto an empty SETUPS column that could never be filled
/// and a DETAILS pane permanently reading "Select a car and setup". The
/// on-screen hint, the help overlay and the README all documented D to
/// download, and `download_setup`, `load_browser_car`,
/// `get_browser_selected_setup` and `scroll_details` had no callers anywhere
/// in the workspace.
fn handle_setup_browser_key(key: KeyCode, app: &AppState) {
    let manager = &app.setup_manager;
    let focus_col = *manager.browser_focus_col.safe_lock();

    match key {
        KeyCode::Left => *manager.browser_focus_col.safe_lock() = 0,
        KeyCode::Right => *manager.browser_focus_col.safe_lock() = 1,

        KeyCode::Up | KeyCode::Down => {
            let forward = key == KeyCode::Down;
            if focus_col == 0 {
                let total = manager.get_manifest().len();
                let mut idx = manager.browser_car_idx.safe_lock();
                let moved = step(*idx, total, forward);
                if moved != *idx {
                    *idx = moved;
                    drop(idx);
                    // Moving the car selection is what loads its setups. This
                    // call is the reason the SETUPS column was always empty.
                    manager.load_browser_car();
                }
            } else {
                let total = manager.get_browser_setups().len();
                let mut idx = manager.browser_setup_idx.safe_lock();
                *idx = step(*idx, total, forward);
                drop(idx);
                *manager.details_scroll.safe_lock() = 0;
            }
        }

        // Enter on the car column is an explicit "load this one", which also
        // gives the user a way to retry after a failed fetch.
        KeyCode::Enter if focus_col == 0 => {
            manager.load_browser_car();
            *manager.browser_focus_col.safe_lock() = 1;
        }

        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Char('в') | KeyCode::Char('В') => {
            if let Some(setup) = manager.get_browser_selected_setup() {
                let target = manager.get_browser_target_car();
                if manager.download_setup(&setup, &target) {
                    info!("Installed setup '{}' for {}", setup.name, target);
                } else {
                    error!("Could not install setup '{}' for {}", setup.name, target);
                }
            }
        }

        KeyCode::PageUp => manager.scroll_details(-1),
        KeyCode::PageDown => manager.scroll_details(1),
        _ => {}
    }
}

/// Move a list selection one step, staying inside the list.
fn step(current: usize, total: usize, forward: bool) -> usize {
    if total == 0 {
        return 0;
    }
    if forward {
        (current + 1).min(total - 1)
    } else {
        current.saturating_sub(1)
    }
}
