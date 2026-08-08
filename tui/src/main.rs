use ac_core::config::Language;
use ac_core::updater::UpdateStatus;
// Only the Linux startup path reaches into `platform`.
use ac_tui::keys;
#[cfg(target_os = "linux")]
use ac_tui::platform;
use ac_tui::ui::UIRenderer;
use ac_tui::{AppLogLevel, AppStage, AppState, AppTab, SafeLock, setup_logging};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
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
// `name` and `about` spelled out rather than taken from the crate. Clap uses
// the *package* name, so `ac_pro_engineer --version` answered "ac_tui 0.3.5" —
// a name that appears nowhere a user can see, on the one command whose whole
// job is to identify the program.
#[command(
    name = "ac_pro_engineer",
    version,
    about = "AC Pro Engineer — telemetry, race engineering and an in-game overlay for Assetto Corsa"
)]
struct AppArgs {
    /// Do not write a log file at all.
    ///
    /// The log is small and it is the first thing a bug report needs, so this
    /// is not the default — but it is written under the config directory, and
    /// a machine where that is not writable should still start.
    #[arg(short, long, conflicts_with = "log-level", conflicts_with = "log")]
    silent: bool,

    /// How much to log. Defaults to `info`.
    ///
    /// `debug` adds the telemetry loop and the overlay writer. `trace` adds
    /// every shared-memory read, which is tens of lines a second and is only
    /// worth it when chasing a connection that comes and goes.
    #[arg(short, long, id = "log-level", conflicts_with = "silent")]
    log_level: Option<AppLogLevel>,

    /// Write the log to this file instead of under the config directory.
    #[arg(long, conflicts_with = "silent")]
    log: Option<PathBuf>,

    #[arg(
        short = 'd',
        long = "demo",
        help = "Run in live simulation mode with realistic telemetry data"
    )]
    demo: bool,

    /// Write the in-game Lua panel somewhere, and exit.
    ///
    /// The application installs it into Assetto Corsa by itself at startup, so
    /// this is for when that cannot work: a game folder it may not write to, an
    /// install in a place `ac_paths` does not find, a second copy of AC. The
    /// files come out of this binary, so what lands is exactly the panel this
    /// build's frame is shaped for — which a copy downloaded separately would
    /// not be.
    ///
    /// A flag rather than a folder in the release archive: the panel's folder
    /// has to be named `ac_pro_engineer` for CSP to find its entry point, and
    /// that is also the name of this binary. Shipping both in one archive is a
    /// collision, and it is the one that failed the v0.3.4 build.
    #[arg(
        long = "export-overlay",
        value_name = "DIR",
        help = "Write the in-game Lua panel into DIR/ac_pro_engineer and exit"
    )]
    export_overlay: Option<PathBuf>,
}

/// Write the embedded Lua panel into `dir/ac_pro_engineer` and say what to do
/// with it.
///
/// Prints rather than logs: this is a command run in a terminal by someone who
/// is already having trouble, and the answer belongs on their screen.
fn export_overlay(dir: &std::path::Path) -> Result<(), anyhow::Error> {
    use ac_core::overlay::install::{APP_DIR, InstallOutcome, install_into};

    let target = dir.join(APP_DIR);
    match install_into(&target) {
        Ok(InstallOutcome::Installed { updated }) => {
            println!("Wrote {updated} file(s) to {}", target.display());
        }
        Ok(InstallOutcome::AlreadyCurrent) => {
            println!("{} is already up to date", target.display());
        }
        Ok(InstallOutcome::NoGameFound) => {
            // install_into never returns this; matched so a future variant
            // cannot be silently ignored.
            println!("Nothing was written to {}", target.display());
        }
        Err(error) => {
            eprintln!("Could not write {}: {error}", target.display());
            return Err(error.into());
        }
    }

    println!(
        "\nThis is the panel for AC Pro Engineer v{}. Copy the whole\n\
         {APP_DIR} folder into:\n\n    \
         <Assetto Corsa>/apps/lua/\n\n\
         then enable \"AC Pro Engineer\" in CSP's app sidebar. The panel reads\n\
         from the running application, so keep that open too.",
        ac_core::updater::CURRENT_VERSION
    );
    Ok(())
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

    // Before anything that needs a terminal, a config or a game: this writes
    // four files and exits, and it has to work on a machine where the rest of
    // the application cannot start.
    if let Some(target) = args.export_overlay.as_deref() {
        return export_overlay(target);
    }

    // Started from a file manager or a desktop entry, there is no terminal to
    // draw on: raw mode fails and the process dies before showing anything.
    // Open one and run there instead. Must happen before the panic hook or
    // any terminal setup, since neither applies to a process that is about to
    // hand off to a child.
    #[cfg(target_os = "linux")]
    if platform::relaunch::needs_terminal() {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ac_pro_engineer"));
        let forwarded: Vec<String> = std::env::args().skip(1).collect();
        match platform::relaunch::relaunch_in_terminal(&exe, &forwarded) {
            Ok(terminal) => {
                info!("No terminal attached; relaunched in {terminal}");
                return Ok(());
            }
            Err(reason) => {
                // Carry on rather than exit. The application will fail
                // visibly on stderr, which is more use than a silent exit,
                // and a user who piped stdout deliberately still gets to run.
                eprintln!("Could not open a terminal window ({reason}).");
                eprintln!("Run this from a terminal instead: ac_pro_engineer");
            }
        }
    }

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

    let mut app = AppState::new();
    if args.demo {
        app.enable_demo_simulation();
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Mouse capture is deliberately not enabled. It was, and nothing ever
    // handled an `Event::Mouse` — the only effect was to take selection and
    // copy away from the user's terminal, which is how anyone gets a log line
    // or a lap time out of a TUI. Enabling it again means handling the events.
    execute!(stdout, EnterAlternateScreen)?;

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

                if app.onboarding == ac_tui::OverlayOnboarding::Offer && !app.show_first_run_prompt
                {
                    match key.code {
                        KeyCode::Left => app.overlay_card_selection = 0,
                        KeyCode::Right => app.overlay_card_selection = 1,
                        KeyCode::Enter => {
                            if app.overlay_card_selection == 0 {
                                app.install_overlay_now();
                                app.onboarding = ac_tui::OverlayOnboarding::Tips;
                            } else {
                                app.finish_onboarding();
                            }
                        }
                        KeyCode::Esc => app.finish_onboarding(),
                        _ => {}
                    }
                    continue;
                }

                if app.onboarding == ac_tui::OverlayOnboarding::Tips {
                    match key.code {
                        KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') => {
                            app.finish_onboarding()
                        }
                        _ => {}
                    }
                    continue;
                }

                if app.show_overlay_card && !app.show_first_run_prompt {
                    match key.code {
                        KeyCode::Left => app.overlay_card_selection = 0,
                        KeyCode::Right => app.overlay_card_selection = 1,
                        KeyCode::Enter => {
                            if app.overlay_card_selection == 0 {
                                app.install_overlay_now();
                            } else {
                                app.show_overlay_card = false;
                            }
                        }
                        KeyCode::Char('d') | KeyCode::Char('D') => {
                            app.config.overlay.startup_card = false;
                            let _ = app.config.save();
                            app.show_overlay_card = false;
                        }
                        // Its own key rather than a third button: the two
                        // buttons are the common path, and a bridge is fetched
                        // once, on the machine where it is wrong.
                        KeyCode::Char('b') | KeyCode::Char('B') => app.fetch_bridge_now(),
                        KeyCode::Esc => app.show_overlay_card = false,
                        _ => {}
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
                let mut app_lock = bg_app.safe_lock();
                app_lock.tick();
                app_lock.perf.last_tick = Instant::now();
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

                terminal.draw(|f| renderer.render(f, &app_lock))?;

                // Measured around the draw only, so it reports the cost of
                // rendering rather than the frame budget the loop sleeps out.
                app_lock.perf.frame_time = start.elapsed();
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

                // One place decides what a key means, and the hints at the
                // bottom of every tab are printed from the same table. They
                // used to be three independent claims about the key map, and
                // two of them were wrong.
                let action = keys::resolve(key, &app_lock.config.keys, app_lock.active_tab);

                // Ahead of the help and of every tab: it is opened over
                // whatever was on screen and closed again, and Esc has to
                // reach it rather than dropping the session to the launcher.
                if app_lock.show_overlay_diagnosis {
                    match key.code {
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            app_lock.refresh_overlay_diagnosis()
                        }
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            app_lock.show_overlay_diagnosis = false
                        }
                        _ => {}
                    }
                    continue;
                }

                if app_lock.show_help {
                    // Whatever opens the help closes it, plus the fixed
                    // aliases the modal names in nine places. Resolved rather
                    // than listed: F1 used to be the one key of the four the
                    // modal advertised that did nothing, which reads as the
                    // modal being stuck.
                    if matches!(action, Some(keys::Action::Help) | Some(keys::Action::Quit)) {
                        app_lock.show_help = false;
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

                // The Settings screen's key-capture mode wants the next
                // keypress raw, whatever it is bound to — otherwise F10 could
                // never be rebound, because it would toggle the overlay on the
                // way past.
                if app_lock.active_tab == AppTab::Settings && app_lock.ui_state.settings.capturing {
                    let AppState {
                        ui_state, config, ..
                    } = &mut *app_lock;
                    if ui_state.settings.capture_key(key, config)
                        && let Err(error) = app_lock.config.save()
                    {
                        error!(error = ?error, "Could not save the key bindings");
                    }
                    continue;
                }

                match action {
                    Some(keys::Action::Help) => {
                        app_lock.show_help = true;
                    }
                    Some(keys::Action::Quit) => {
                        app_lock.stage = AppStage::Launcher;
                        if !app_lock.is_demo_mode {
                            app_lock.disconnect();
                        }
                        continue;
                    }
                    // The screenshot dumps the frame that was just drawn.
                    // Handled here rather than in a tab arm because the buffer
                    // belongs to the terminal, not to any one screen.
                    Some(keys::Action::Screenshot) => {
                        let size = terminal.size().unwrap_or_default();
                        let result = save_screenshot(
                            terminal.current_buffer_mut(),
                            size.width,
                            size.height,
                            &app_lock.config.resolve_data_path(),
                        );
                        let message = match result {
                            Ok(path) => format!("Screenshot: {}", path.display()),
                            Err(error) => {
                                error!(error = ?error, "Could not write screenshot");
                                format!("Screenshot failed: {}", error)
                            }
                        };
                        app_lock.ui_state.analysis.set_status(message);
                    }
                    Some(keys::Action::Language) => {
                        app_lock.config.language = match app_lock.config.language {
                            Language::English => Language::Russian,
                            Language::Russian => Language::English,
                        };
                        let _res = app_lock.config.save();
                    }
                    Some(keys::Action::NextTab) => {
                        app_lock.active_tab = app_lock.active_tab.next();
                    }
                    Some(keys::Action::PrevTab) => {
                        app_lock.active_tab = app_lock.active_tab.previous();
                    }
                    Some(keys::Action::GoToTab(tab)) => app_lock.active_tab = tab,
                    _ => match app_lock.active_tab {
                        AppTab::Settings => {
                            // The overlay category has an action, not just
                            // values: pushing the panel into the game folder
                            // is a thing you do, not a number you set.
                            // A result nobody sees is not a result: both of
                            // these raise a card over the settings.
                            if app_lock.overlay_result_popup {
                                app_lock.overlay_result_popup = false;
                                continue;
                            }

                            // [I] and [U] are neighbours, and one of them
                            // deletes: neither happens without a second key.
                            if let Some(action) = app_lock.overlay_confirm {
                                match key.code {
                                    KeyCode::Left => app_lock.overlay_confirm_selection = 0,
                                    KeyCode::Right => app_lock.overlay_confirm_selection = 1,
                                    KeyCode::Enter => {
                                        if app_lock.overlay_confirm_selection == 0 {
                                            match action {
                                                ac_tui::OverlayAction::Install => {
                                                    app_lock.install_overlay_now()
                                                }
                                                ac_tui::OverlayAction::Uninstall => {
                                                    app_lock.uninstall_overlay_now()
                                                }
                                            }
                                            app_lock.overlay_result_popup = true;
                                        }
                                        app_lock.overlay_confirm = None;
                                    }
                                    KeyCode::Esc => app_lock.overlay_confirm = None,
                                    _ => {}
                                }
                                continue;
                            }

                            // Resolved from the bindings rather than matched
                            // as letters, so these three rebind like every
                            // other key and the caption below them is printed
                            // from what they actually are.
                            if app_lock.ui_state.settings.category
                                == ac_tui::ui::tabs::settings::SettingsCategory::Overlay
                            {
                                match action {
                                    Some(keys::Action::OverlayInstall) => {
                                        app_lock.overlay_confirm =
                                            Some(ac_tui::OverlayAction::Install);
                                        app_lock.overlay_confirm_selection = 1;
                                        continue;
                                    }
                                    Some(keys::Action::OverlayUninstall) => {
                                        app_lock.overlay_confirm =
                                            Some(ac_tui::OverlayAction::Uninstall);
                                        app_lock.overlay_confirm_selection = 1;
                                        continue;
                                    }
                                    // Measured on the way in, not held from
                                    // startup: the answer changes when the
                                    // bridge is started, which is exactly what
                                    // someone does between two looks at this.
                                    Some(keys::Action::OverlayDiagnostics) => {
                                        app_lock.refresh_overlay_diagnosis();
                                        app_lock.show_overlay_diagnosis = true;
                                        continue;
                                    }
                                    _ => {}
                                }
                            }

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
                        AppTab::Analysis => match (action, key.code) {
                            (Some(keys::Action::AnalysisSave), _) => {
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
                            (Some(keys::Action::AnalysisLoad), _) => {
                                app_lock.ui_state.analysis.toggle_load_menu();
                            }
                            (Some(keys::Action::AnalysisCompare), _) => {
                                app_lock.ui_state.analysis.toggle_compare();
                            }
                            (Some(keys::Action::AnalysisExport), _) => {
                                let sel = app_lock
                                    .ui_state
                                    .analysis
                                    .selected_lap_index
                                    .min(app_lock.analyzer.laps.len().saturating_sub(1));
                                match app_lock.analyzer.laps.get(sel).cloned() {
                                    Some(lap) => {
                                        // Named after the car, track and lap
                                        // rather than "lap_3_export.csv",
                                        // which collided with itself across
                                        // every session at every circuit.
                                        let file_name = format!(
                                            "{}_{}_lap{}_{}.csv",
                                            sanitise_for_file_name(&lap.car_model),
                                            sanitise_for_file_name(&lap.track_name),
                                            lap.lap_number + 1,
                                            chrono::Local::now().format("%Y%m%d-%H%M%S"),
                                        );
                                        let export_path = app_lock
                                            .config
                                            .resolve_data_path()
                                            .join("exports")
                                            .join(file_name);

                                        let status = match ac_core::analyzer::export_lap_to_csv(
                                            &lap,
                                            &export_path,
                                        ) {
                                            Ok(p) => format!("Exported CSV: {}", p.display()),
                                            // Previously `if let Ok(..)`, so a
                                            // failed export was silent and
                                            // indistinguishable from a
                                            // successful one.
                                            Err(error) => {
                                                error!(error = ?error, "CSV export failed");
                                                format!("Export failed: {error}")
                                            }
                                        };
                                        app_lock.ui_state.analysis.set_status(status);
                                    }
                                    None => app_lock
                                        .ui_state
                                        .analysis
                                        .set_status("No lap to export".to_string()),
                                }
                            }
                            (_, KeyCode::Left) => app_lock.ui_state.analysis.prev_tab(),
                            (_, KeyCode::Right) => app_lock.ui_state.analysis.next_tab(),
                            (_, KeyCode::Up) => {
                                let laps_len = app_lock.analyzer.laps.len();
                                app_lock.ui_state.analysis.menu_up(laps_len);
                            }
                            (_, KeyCode::Down) => {
                                let laps_len = app_lock.analyzer.laps.len();
                                app_lock.ui_state.analysis.menu_down(laps_len);
                            }
                            (_, KeyCode::Enter) => {
                                let AppState {
                                    ui_state, analyzer, ..
                                } = &mut *app_lock;
                                ui_state.analysis.load_selected_file(analyzer);
                            }
                            _ => {}
                        },
                        AppTab::Setup => {
                            let in_browser = *app_lock.setup_manager.browser_active.safe_lock();
                            match (action, key.code) {
                                // The browser key toggles between the local
                                // setup list and the cloud browser, in either
                                // direction.
                                (Some(keys::Action::SetupBrowser), _) => {
                                    let mut active =
                                        app_lock.setup_manager.browser_active.safe_lock();
                                    *active = !*active;
                                }
                                // Download reached nothing outside the browser,
                                // while the hint on the list screen advertised
                                // it. It opens the browser now, which is where
                                // there is something to download.
                                (Some(keys::Action::SetupDownload), _) if !in_browser => {
                                    *app_lock.setup_manager.browser_active.safe_lock() = true;
                                    app_lock.setup_manager.load_browser_car();
                                }
                                _ if in_browser => {
                                    handle_setup_browser_key(action, key.code, &app_lock)
                                }
                                (_, KeyCode::Up) => {
                                    let current =
                                        app_lock.ui_state.setup_list_state.selected().unwrap_or(0);
                                    if current > 0 {
                                        app_lock
                                            .ui_state
                                            .setup_list_state
                                            .select(Some(current - 1));
                                    }
                                }
                                (_, KeyCode::Down) => {
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
                                (_, KeyCode::PageUp) => app_lock.setup_manager.scroll_details(-1),
                                (_, KeyCode::PageDown) => app_lock.setup_manager.scroll_details(1),
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

    // Before anything else on the way out: the in-game overlay watches for
    // this and hides, rather than leaving the last frame on screen looking
    // live.
    app.shutdown_overlay();

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

/// Reduce a car or track name to something safe in a file name.
fn sanitise_for_file_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    }
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
fn handle_setup_browser_key(action: Option<keys::Action>, key: KeyCode, app: &AppState) {
    let manager = &app.setup_manager;
    let focus_col = *manager.browser_focus_col.safe_lock();

    if action == Some(keys::Action::SetupDownload) {
        if let Some(setup) = manager.get_browser_selected_setup() {
            let target = manager.get_browser_target_car();
            if manager.download_setup(&setup, &target) {
                info!("Installed setup '{}' for {}", setup.name, target);
            } else {
                error!("Could not install setup '{}' for {}", setup.name, target);
            }
        }
        return;
    }

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

/// Write the current frame to `<data>/screenshots/<timestamp>.png`.
///
/// Named by timestamp so repeated presses accumulate rather than overwrite —
/// the point is usually to capture a sequence, or something that just
/// happened and may not happen again.
///
/// A PNG, because the thing a driver does with this is paste it into a bug
/// report or a Discord message, and neither of those opens an SVG.
fn save_screenshot(
    buffer: &ratatui::buffer::Buffer,
    width: u16,
    height: u16,
    data_dir: &std::path::Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = data_dir.join("screenshots");
    std::fs::create_dir_all(&dir)?;

    let name = format!(
        "ac_pro_engineer_{}.png",
        chrono::Local::now().format("%Y%m%d_%H%M%S%.3f")
    );
    let path = dir.join(name);

    ac_tui::ui::screenshot::buffer_to_png(buffer, width, height, &path, 2.0)?;
    Ok(path)
}
