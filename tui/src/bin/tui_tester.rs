use ac_core::analyzer::{LapData, RadarStats, TelemetryPoint};
use ac_core::config::Language;
use ac_core::engineer::{Recommendation, Severity};
use ac_core::games::{Capabilities, Car, Fixed, Reading, Session, Status};
use ac_core::session_info::SessionInfo;
use ac_tui::ui::UIRenderer;
use ac_tui::ui::screenshot::buffer_to_png;
use ac_tui::ui::tabs::analysis::AnalysisSubTab;
use ac_tui::{AppStage, AppState, AppTab, SafeLock};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::fs;
use std::path::Path;

/// Write one screenshot of what the terminal just drew.
///
/// One file per screen. This used to write an SVG beside every PNG on the
/// grounds that the SVG was the exact record — but nothing ever read one, they
/// doubled what a screenshot refresh put in a diff, and GitHub will not show
/// one inline anyway.
fn capture(
    terminal: &ratatui::Terminal<TestBackend>,
    width: u16,
    height: u16,
    dir: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let buffer = terminal.backend().buffer();
    // Scaled by two, for a screen that is 1416 CSS pixels wide: readable
    // inline on GitHub and still crisp when opened.
    buffer_to_png(buffer, width, height, &dir.join(format!("{name}.png")), 2.0)?;
    println!("  [OK] Rendered {name}.png");
    Ok(())
}

fn create_populated_app_state() -> AppState {
    let mut app = AppState::new();
    app.config.language = Language::English;
    app.is_connected = true;
    app.is_game_running = true;

    // Session Info
    app.session_info = SessionInfo {
        car_name: "Ferrari SF70H".to_string(),
        track_name: "Autodromo Nazionale Monza".to_string(),
        track_config: "GP".to_string(),
        player_name: "Pro Driver".to_string(),
        session_type: "Practice".to_string(),
        lap_count: 5,
        session_time_left: 1_800_000.0,
        max_rpm: 12500,
        max_fuel: 110.0,
    };

    // The car — also the base each history sample is derived from below.
    let car = Car {
        speed_kmh: 248.5,
        rpm: 11450,
        gear: 5,
        fuel_litres: 36.8,
        throttle: 0.94,
        brake: 0.0,
        clutch: 0.0,
        steer_angle: -0.12,
        acc_g: [1.38, 0.0, 0.65],
        tyre_pressure_psi: [27.4, 27.6, 27.5, 27.3],
        // A stint's worth of wear, not a fresh set. Every screenshot showed
        // "0%" in red on all four corners, because nothing filled this in —
        // which reads as the readout being broken rather than as no data.
        tyre_wear: [96.8, 96.4, 94.9, 95.2],
        tyre_temp_inner_c: [89.2, 88.0, 92.1, 90.5],
        tyre_temp_middle_c: [86.4, 85.2, 89.0, 87.8],
        tyre_temp_outer_c: [82.1, 81.0, 85.2, 84.0],
        brake_temp_c: [450.0, 442.0, 380.0, 375.0],
        air_temp_c: 22.5,
        road_temp_c: 34.0,
        tc: 3.0,
        abs: 2.0,
        // The cockpit block reads these, and at zero it showed "MAP 0" and
        // "BIAS 0.0%" next to live numbers.
        brake_bias: 0.567,
        ..Default::default()
    };
    // The session as it stands.
    let session = Session {
        status: Status::Live,
        surface_grip: 0.98,
        completed_laps: 5,
        current_lap_ms: 42500,
        last_lap_ms: 81452,
        best_lap_ms: 81452,
        position: 2,
        fuel_per_lap: 2.85,
        // Half an hour left, a car a third of the way round, and a delta worth
        // looking at. The footer and the session block read these, and with
        // them at zero the screenshots showed "-:--.---" and "0.0 min" beside
        // live telemetry.
        session_time_left_ms: 1_512_000.0,
        track_position: 0.34,
        current_sector: 1,
        engine_map: 4,
        last_sector_ms: 27_940,
        total_laps: 0,
        ..Default::default()
    };

    app.reading = Some(Reading {
        car,
        session,
        // These screenshots stand in for Assetto Corsa, which measures all of
        // it. A harness that left this at the default would photograph an
        // engineer with nothing to say about tyres.
        capabilities: Capabilities::all(),
        fixed: Fixed {
            max_rpm: 12500,
            max_fuel_litres: 110.0,
            car_model: "ks_ferrari_sf70h".to_string(),
            track: "monza".to_string(),
            ..Default::default()
        },
    });

    // Physics & Telemetry History
    let mut history = Vec::with_capacity(300);
    let mut trace_points = Vec::with_capacity(300);
    for i in 0..300 {
        let t = i as f32 * 0.05;
        let mut p = car;
        p.speed_kmh = 180.0 + (t * 2.0).sin() * 70.0;
        p.rpm = (8000.0 + (t * 2.0).sin() * 3500.0) as i32;
        p.throttle = (0.5 + (t * 1.5).cos() * 0.5).clamp(0.0, 1.0);
        p.brake = if (t * 1.5).cos() < -0.3 { 0.8 } else { 0.0 };
        p.steer_angle = (t * 0.8).sin() * 0.4;
        p.acc_g = [(t * 0.8).sin() * 1.5, 0.0, (t * 1.5).cos() * 1.2];
        history.push(p);

        let px = (t * 0.8).cos() * 150.0;
        let py = (t * 0.8).sin() * 80.0;
        trace_points.push(TelemetryPoint {
            rpms: 5000,
            time_ms: i * 50,
            // AC publishes normalised car position: 0.0 at the line, 1.0 at
            // the line again. This was metres-ish (`i * 10.0`), which every
            // consumer of a trace reads as "three thousand laps" — the delta
            // graph resamples to a ceiling of 1.0 and drew one sample, and
            // corner detection saw the whole lap as one straight.
            distance: i as f32 / 300.0,
            speed: p.speed_kmh,
            gas: p.throttle,
            brake: p.brake,
            steer: p.steer_angle,
            gear: p.gear,
            lat_g: p.acc_g[0],
            lon_g: p.acc_g[2],
            x: px,
            y: py,
            slip_avg: 0.02,
            detail: Default::default(),
        });
    }
    for (index, p) in history.into_iter().enumerate() {
        app.car_history.push(p);
        // The footer reads the *session* history for the lap times, and
        // nothing filled it — so every screenshot showed "L: -:--.---" and
        // "B: -:--.---" underneath a live session.
        let mut g = session;
        g.current_lap_ms = 42_500 + index as i32 * 8;
        app.session_history.push(g);
    }

    // The curated frame goes in last, so the history ends where the live
    // readouts are.
    //
    // The header's rev bar reads `car_history.last()` and the cockpit
    // reads the live physics — the same tick in the running application, and
    // two unrelated numbers here, because the loop above walks a sine and the
    // mock is set to one good-looking frame. Every Dashboard screenshot went
    // out with "4505 / 12500 RPM" across the top and "11450 RPM" in the
    // cockpit underneath it, which reads as the program disagreeing with
    // itself.
    app.car_history.push(car);
    app.session_history.push(session);

    // Lap Data for Analysis
    let mock_lap = LapData {
        lap_number: 4,
        lap_time_ms: 81452,
        sectors: [24120, 28350, 28982],
        valid: true,
        car_model: "Ferrari SF70H".to_string(),
        track_name: "Autodromo Nazionale Monza".to_string(),
        track_length_m: 5793.0,
        track_length_measured: false,
        save_date: "2026-07-30".to_string(),
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
        avg_ride_height: [25.0, 55.0],
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
        scrubbing_incidents: 0,
        max_steering_over_rotation: 0.0,
        // Nought to one, which is the scale `TelemetryAnalyzer` produces —
        // every field there is `score / 100.0`. Written as percentages here,
        // they were a hundred times too big, and the Analysis tab's Inputs box
        // multiplies by a hundred to display them: every screenshot of it went
        // out reading "Aggression: 8800.0%".
        radar_stats: RadarStats {
            consistency: 0.94,
            aggression: 0.88,
            smoothness: 0.93,
            tyre_mgmt: 0.91,
        },
        telemetry_trace: trace_points,
        bounds_min_x: -160.0,
        bounds_max_x: 160.0,
        bounds_min_y: -90.0,
        bounds_max_y: 90.0,
    };

    // A stint, not a lap. The Strategy tab's pace chart plots lap times against
    // lap number and the Analysis tab lists them, and with one lap in the
    // analyzer both drew a single dot in an empty box — a picture of a feature
    // not working. Five laps with a plausible spread: a slower first flying
    // lap, two quick ones, a scrappy one, then the best.
    for (number, delta_ms) in [(1, 1_480), (2, 320), (3, 640), (4, 2_050)] {
        let mut lap = mock_lap.clone();
        // Numbered from one, the way a driver counts them and the way the
        // chart's axis reads.
        lap.lap_number = number;
        lap.lap_time_ms = mock_lap.lap_time_ms + delta_ms;
        // The sectors have to add up to the lap, or the Analysis tab shows a
        // split that disagrees with the time beside it.
        lap.sectors[2] += delta_ms;

        // The trace has to lose that time somewhere, or the Corners sub-tab
        // draws a lap that is 1.5 s slower with every corner at +0.000 — a
        // picture of the decomposition not working. Most of it goes in one
        // band of the lap, the way a real mistake does, and the rest is spread
        // so the straights are not suspiciously perfect.
        // Understeer on every lap regardless of how the lap went, lockups
        // that come and go: the two answers `driver_vs_car` exists to tell
        // apart, so the POST-STINT shot below shows both rather than an empty
        // heading.
        lap.understeer_count = 9 + (number % 2);
        lap.lockup_count = [0, 11, 1, 9][(number as usize) % 4];

        let concentrated = (delta_ms as f32 * 0.6) as i32;
        let spread = delta_ms - concentrated;
        for point in lap.telemetry_trace.iter_mut() {
            let fraction = point.distance.clamp(0.0, 1.0);
            let mut lost = (spread as f32 * fraction) as i32;
            if fraction > 0.42 {
                // All of it by the time the car is out of that corner.
                let through = ((fraction - 0.42) / 0.08).clamp(0.0, 1.0);
                lost += (concentrated as f32 * through) as i32;
            }
            point.time_ms += lost;
        }
        app.analyzer.laps.push(lap);
    }
    let mut best = mock_lap.clone();
    best.lap_number = 5;
    best.understeer_count = 9;
    best.lockup_count = 2;
    app.analyzer.laps.push(best);

    // The Engineer tab's DRIVING STYLE box reads the live driving style and
    // the frame counters, neither of which anything filled — so beside a
    // populated advice feed it drew Smoothness 50%, Aggression 50% and Trail
    // Braking 0%, which is the neutral state the engineer sits at before it
    // has seen a lap. It reads as the panel not working.
    app.engineer.driving_style.smoothness = 91.8;
    app.engineer.driving_style.aggression = 74.0;
    app.engineer.driving_style.trail_braking = 88.4;
    app.engineer.stats.lockup_frames_front = 2;
    app.engineer.stats.wheel_spin_frames = 1;
    // The last one is the quickest, so it is the reference the ghost uses.
    app.analyzer.best_lap_index = Some(app.analyzer.laps.len() - 1);

    // Engineer Recommendations
    app.recommendations.push(Recommendation {
        component: "Tyres".to_string(),
        category: "Pressure".to_string(),
        severity: Severity::Info,
        message: "Front Right pressure target optimal at 27.5 PSI (+0.2 PSI recommended)"
            .to_string(),
        action: "Adjust FR cold pressure +0.2 PSI".to_string(),
        parameters: Vec::new(),
        confidence: 0.95,
        chain: None,
    });
    app.recommendations.push(Recommendation {
        component: "Aero".to_string(),
        category: "Downforce".to_string(),
        severity: Severity::Warning,
        message: "High speed balance: minor understeer at Lesmo 2 (-0.15s loss)".to_string(),
        action: "Reduce rear wing angle by 1 degree".to_string(),
        parameters: Vec::new(),
        confidence: 0.88,
        chain: None,
    });

    app
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Rendering every screen to PNG...");

    let width = 140;
    let height = 40;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;

    let mut app = create_populated_app_state();
    let renderer = UIRenderer::new();

    let screenshot_dir = Path::new("screenshots");
    if !screenshot_dir.exists() {
        fs::create_dir_all(screenshot_dir)?;
    }

    // 1. Launcher
    app.stage = AppStage::Launcher;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Launcher")?;

    // 1b. The game selector, which is the one launcher row with something to
    // say: what the chosen simulator reports and what it does not. Captured
    // because it is also the only place a driver is told *why* the wear and
    // camber advice go quiet on Competizione, and a picture of it is how that
    // gets reviewed without a game.
    app.launcher_selection = ac_tui::ui::launcher::ROW_GAME;
    app.show_overlay_card = false;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Launcher_Game")?;
    app.launcher_selection = ac_tui::ui::launcher::ROW_START;

    // 2. Main Running Stage (Live Populated Telemetry)
    app.stage = AppStage::Running;

    let targets = [
        (AppTab::Dashboard, "Dashboard"),
        (AppTab::Telemetry, "Telemetry"),
        (AppTab::Engineer, "Engineer"),
        (AppTab::Setup, "Setup_1"),
        (AppTab::Analysis, "Analysis_Overview"),
        (AppTab::Strategy, "Strategy"),
        (AppTab::Ffb, "FFB_Tuning"),
        (AppTab::Settings, "Settings"),
        (AppTab::Guide, "Guide"),
    ];

    for (tab, name) in &targets {
        app.active_tab = *tab;
        terminal.draw(|f| renderer.render(f, &app))?;
        capture(&terminal, width, height, screenshot_dir, name)?;
    }

    // 2a. The network tab, with somebody on the other end of it.
    //
    // **The state is real rather than posed where it can be**: the link and
    // the peer list are what a copy holds after it has heard from another
    // one, so the picture is of the screen a driver sees rather than of an
    // empty form. It is the screen the whole of v0.4.5 is about and the one
    // nobody had looked at.
    {
        use ac_core::broadcast::discovery::{Peer, Role};
        use std::time::Instant;

        app.lan.share_simply("Rgosh");
        app.lan.share_to = "192.168.1.42:9001".to_string();
        app.lan.watch_simply();
        app.link = ac_core::broadcast::session::Link {
            listening_on: "0.0.0.0:9001".to_string(),
            from: Some("MartialArts".to_string()),
            quiet: false,
            trouble: None,
            rate_hz: 29.8,
            seen: 4_120,
            age_ms: 38,
            worst_age_ms: 210,
            lost: 3,
        };
        app.peers = vec![
            Peer {
                id: "a".to_string(),
                name: "MartialArts".to_string(),
                role: Role::Driving,
                reachable_at: "192.168.1.42:9001".parse()?,
                car: "ks_audi_rs3_lms".to_string(),
                track: "ks_nordschleife".to_string(),
                version: "0.4.5".to_string(),
                front_end: "window".to_string(),
                last_seen: Instant::now(),
            },
            Peer {
                id: "b".to_string(),
                name: "Ann".to_string(),
                role: Role::Watching,
                reachable_at: "192.168.1.7:9001".parse()?,
                car: String::new(),
                track: String::new(),
                version: "0.4.5".to_string(),
                front_end: "terminal".to_string(),
                last_seen: Instant::now(),
            },
        ];
        app.active_tab = AppTab::Lan;
        terminal.draw(|f| renderer.render(f, &app))?;
        capture(&terminal, width, height, screenshot_dir, "LAN")?;

        // The other half of the same feature: the settings somebody sets once.
        app.active_tab = AppTab::Settings;
        app.ui_state
            .settings
            .set_category(ac_tui::ui::tabs::settings::SettingsCategory::Sharing);
        terminal.draw(|f| renderer.render(f, &app))?;
        capture(&terminal, width, height, screenshot_dir, "Settings_Sharing")?;
        app.ui_state
            .settings
            .set_category(ac_tui::ui::tabs::settings::SettingsCategory::System);
        app.active_tab = AppTab::Lan;

        // And the state everybody meets first: switched off, nobody found.
        app.lan = ac_core::lan::LanWish::default();
        app.link = ac_core::broadcast::session::Link::default();
        app.peers.clear();
        terminal.draw(|f| renderer.render(f, &app))?;
        capture(&terminal, width, height, screenshot_dir, "LAN_Off")?;
    }

    // 2b. The same dashboard, on a game that measures a different set.
    //
    // Every screenshot above stands in for Assetto Corsa, which publishes
    // everything — so none of them shows what the screens do when a game does
    // not. Competizione publishes no tread temperature and no tyre wear and
    // does publish what is left of the brakes, and each of those changes what
    // a tyre reads on the dashboard. This is the picture that gets reviewed
    // when somebody asks "what does it look like on ACC".
    if let Some(reading) = app.reading.as_mut() {
        // Through the registry rather than by naming the module: this is a
        // front end, and a front end that reaches for a game's own constants
        // is one that has to be edited again for the third simulator. The
        // boundary test says so, and caught this.
        reading.capabilities = ac_core::games::registry::chosen("assetto_corsa_competizione")
            .backend()
            .map(|backend| backend.capabilities)
            .unwrap_or_default();
        // What that game actually publishes, in place of what it does not:
        // the core temperature and the pad thickness.
        reading.car.tyre_core_temp_c = [88.0, 87.0, 91.0, 90.0];
        reading.car.tyre_temp_inner_c = [0.0; 4];
        reading.car.tyre_temp_middle_c = [0.0; 4];
        reading.car.tyre_temp_outer_c = [0.0; 4];
        reading.car.tyre_wear = [0.0; 4];
        reading.car.camber_rad = [0.0; 4];
        reading.car.brake_pad_mm = [21.4, 21.2, 23.8, 23.6];
        reading.car.brake_disc_mm = [31.2, 31.1, 31.6, 31.5];
        reading.session.surface_grip = 0.0;
        reading.fixed.car_model = "lamborghini_huracan_gt3_evo".to_string();
        reading.fixed.track = "Spa".to_string();
    }
    app.active_tab = AppTab::Dashboard;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Dashboard_ACC")?;
    app.active_tab = AppTab::Strategy;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Strategy_ACC")?;
    // The screen the missing measurements show up on most, and the one that
    // had no ACC picture until now — which is why four zeros where a tread
    // readout goes, and 0 mm of ride height, went unreviewed through a whole
    // release.
    app.active_tab = AppTab::Engineer;
    app.ui_state.engineer.active_sub_tab = 1;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Engineer_ACC")?;
    app.ui_state.engineer.active_sub_tab = 0;

    // Back to a game that measures everything, so the screenshots after this
    // are Assetto Corsa's the way the ones before it are.
    if let Some(reading) = app.reading.as_mut() {
        reading.capabilities = Capabilities::all();
        reading.car.tyre_temp_inner_c = [90.0, 88.0, 93.0, 91.0];
        reading.car.tyre_temp_middle_c = [86.4, 85.2, 89.0, 87.8];
        reading.car.tyre_temp_outer_c = [82.0, 81.0, 85.0, 84.0];
        reading.car.tyre_wear = [96.5, 96.1, 94.8, 94.2];
        reading.car.brake_pad_mm = [0.0; 4];
        reading.car.brake_disc_mm = [0.0; 4];
        reading.session.surface_grip = 0.98;
        reading.fixed.car_model = "ks_ferrari_sf70h".to_string();
        reading.fixed.track = "monza".to_string();
    }

    // 3. Setup_cloud
    app.active_tab = AppTab::Setup;
    *app.setup_manager.browser_active.safe_lock() = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Setup_cloud")?;
    *app.setup_manager.browser_active.safe_lock() = false;

    // 3b. Settings_Keys — twenty-three rows in a pane that holds about
    // thirty, so this is the screenshot that shows when it stops fitting.
    app.active_tab = AppTab::Settings;
    app.ui_state
        .settings
        .set_category(ac_tui::ui::tabs::settings::SettingsCategory::Keys);
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Settings_Keys")?;
    app.ui_state
        .settings
        .set_category(ac_tui::ui::tabs::settings::SettingsCategory::System);

    // 4. Analysis_Traces — the TELEMETRY sub-tab.
    //
    // This was called `Analysis_Radar` and the README captioned it "Driver
    // radar", describing a screen that scores braking and consistency. There
    // is no radar sub-tab: `next_tab` lands on TELEMETRY, and the scores are
    // the Driving Evaluation box on OVERVIEW, which the shot above already
    // shows. The picture and its caption had never agreed.
    //
    // Set rather than stepped: `next_tab()` used to land here and now lands on
    // CORNERS, so a shot captioned "traces" would have quietly become a shot
    // of a different screen the moment a sub-tab was inserted before it.
    app.active_tab = AppTab::Analysis;
    app.ui_state.analysis.current_tab = AnalysisSubTab::Graphs;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Analysis_Traces")?;

    // 3c. Engineer_PostStint — the lap debrief, what the stint says about the
    // car versus the driving, and the refusal to answer before it has one.
    // Never captured at all before, so the whole POST-STINT column went out
    // unreviewed release after release.
    app.active_tab = AppTab::Engineer;
    app.ui_state.engineer.active_sub_tab = 1;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(
        &terminal,
        width,
        height,
        screenshot_dir,
        "Engineer_PostStint",
    )?;
    app.ui_state.engineer.active_sub_tab = 0;

    // 4a. Analysis_Corners — where the lap actually went, and the same screen
    // again with the filter on, which is the half of the feature that decides
    // what a driver reads.
    app.ui_state.analysis.current_tab = AnalysisSubTab::Corners;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Analysis_Corners")?;

    app.ui_state.analysis.corners_filter = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(
        &terminal,
        width,
        height,
        screenshot_dir,
        "Analysis_Corners_Losses",
    )?;
    app.ui_state.analysis.corners_filter = false;
    app.ui_state.analysis.current_tab = AnalysisSubTab::Overview;

    // 4b. Overlay_Diagnostics — the answer to "why is the panel blank",
    // which until this release only existed as a cargo example.
    app.active_tab = AppTab::Settings;
    app.ui_state
        .settings
        .set_category(ac_tui::ui::tabs::settings::SettingsCategory::Overlay);
    app.show_overlay_diagnosis = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(
        &terminal,
        width,
        height,
        screenshot_dir,
        "Overlay_Diagnostics",
    )?;
    app.show_overlay_diagnosis = false;
    app.ui_state
        .settings
        .set_category(ac_tui::ui::tabs::settings::SettingsCategory::System);

    // 5. Help_Modal
    // AppState::show_help, not UIState::show_help. The renderer checks the
    // former; the latter was read by nothing, which is why every generated
    // Help_Modal.png was byte-identical to the screenshot before it.
    app.show_help = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    capture(&terminal, width, height, screenshot_dir, "Help_Modal")?;
    app.show_help = false;

    println!("\nDone.");
    Ok(())
}
