use ac_core::ac_structs::{AcGraphics, AcPhysics, AcStatic, StringU16_33};
use ac_core::analyzer::{LapData, RadarStats, TelemetryPoint};
use ac_core::config::Language;
use ac_core::engineer::{Recommendation, Severity};
use ac_core::overlay::OverlayMode;
use ac_core::session_info::SessionInfo;
use ac_tui::ui::UIRenderer;
use ac_tui::ui::screenshot::buffer_to_svg;
use ac_tui::{AppStage, AppState, AppTab, SafeLock};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::fs;
use std::path::Path;

fn create_populated_app_state() -> AppState {
    let mut app = AppState::new(OverlayMode::External);
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

    // Mock Physics — also the base each history sample is derived from below.
    let phys = AcPhysics {
        speed_kmh: 248.5,
        rpms: 11450,
        gear: 6,
        fuel: 36.8,
        gas: 0.94,
        brake: 0.0,
        clutch: 0.0,
        steer_angle: -0.12,
        acc_g: [1.38, 0.0, 0.65],
        wheels_pressure: [27.4, 27.6, 27.5, 27.3],
        tyre_temp_i: [89.2, 88.0, 92.1, 90.5],
        tyre_temp_m: [86.4, 85.2, 89.0, 87.8],
        tyre_temp_o: [82.1, 81.0, 85.2, 84.0],
        brake_temp: [450.0, 442.0, 380.0, 375.0],
        air_temp: 22.5,
        road_temp: 34.0,
        tc: 3.0,
        abs: 2.0,
        ..Default::default()
    };
    app.mock_physics = Some(phys);

    // Mock Graphics
    app.mock_graphics = Some(AcGraphics {
        surface_grip: 0.98,
        completed_laps: 5,
        i_current_time: 42500,
        i_last_time: 81452,
        i_best_time: 81452,
        position: 2,
        fuel_x_lap: 2.85,
        ..Default::default()
    });

    // Mock Static
    app.mock_static = Some(AcStatic {
        max_rpm: 12500,
        max_fuel: 110.0,
        car_model: StringU16_33::from("ks_ferrari_sf70h"),
        track: StringU16_33::from("monza"),
        ..Default::default()
    });

    // Physics & Telemetry History
    let mut history = Vec::with_capacity(300);
    let mut trace_points = Vec::with_capacity(300);
    for i in 0..300 {
        let t = i as f32 * 0.05;
        let mut p = phys;
        p.speed_kmh = 180.0 + (t * 2.0).sin() * 70.0;
        p.rpms = (8000.0 + (t * 2.0).sin() * 3500.0) as i32;
        p.gas = (0.5 + (t * 1.5).cos() * 0.5).clamp(0.0, 1.0);
        p.brake = if (t * 1.5).cos() < -0.3 { 0.8 } else { 0.0 };
        p.steer_angle = (t * 0.8).sin() * 0.4;
        p.acc_g = [(t * 0.8).sin() * 1.5, 0.0, (t * 1.5).cos() * 1.2];
        history.push(p);

        let px = (t * 0.8).cos() * 150.0;
        let py = (t * 0.8).sin() * 80.0;
        trace_points.push(TelemetryPoint {
            rpms: 5000,
            time_ms: i * 50,
            distance: i as f32 * 10.0,
            speed: p.speed_kmh,
            gas: p.gas,
            brake: p.brake,
            steer: p.steer_angle,
            gear: p.gear,
            lat_g: p.acc_g[0],
            lon_g: p.acc_g[2],
            x: px,
            y: py,
            slip_avg: 0.02,
        });
    }
    for p in history {
        app.physics_history.push(p);
    }

    // Lap Data for Analysis
    let mock_lap = LapData {
        lap_number: 4,
        lap_time_ms: 81452,
        sectors: [24120, 28350, 28982],
        valid: true,
        car_model: "Ferrari SF70H".to_string(),
        track_name: "Autodromo Nazionale Monza".to_string(),
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
        car_control_score: 95.0,
        scrubbing_incidents: 0,
        max_steering_over_rotation: 0.0,
        radar_stats: RadarStats {
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

    app.analyzer.laps.push(mock_lap);
    app.analyzer.best_lap_index = Some(0);

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
    });
    app.recommendations.push(Recommendation {
        component: "Aero".to_string(),
        category: "Downforce".to_string(),
        severity: Severity::Warning,
        message: "High speed balance: minor understeer at Lesmo 2 (-0.15s loss)".to_string(),
        action: "Reduce rear wing angle by 1 degree".to_string(),
        parameters: Vec::new(),
        confidence: 0.88,
    });

    app
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting REALISTIC POPULATED SVG Vector Screenshot Generator (14 target screens)...");

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

    // 1. Launcher.svg
    app.stage = AppStage::Launcher;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Launcher.svg"),
    )?;
    println!("  [1/14] Rendered Launcher.svg");

    // 2. Main Running Stage (Live Populated Telemetry)
    app.stage = AppStage::Running;

    let targets = [
        (AppTab::Dashboard, "Dashboard.svg"),
        (AppTab::Telemetry, "Telemetry.svg"),
        (AppTab::Engineer, "Engineer.svg"),
        (AppTab::Setup, "Setup_1.svg"),
        (AppTab::Analysis, "Analysis_Overview.svg"),
        (AppTab::Strategy, "Strategy.svg"),
        (AppTab::Ffb, "FFB_Tuning.svg"),
        (AppTab::Settings, "Settings.svg"),
        (AppTab::Guide, "Guide.svg"),
    ];

    for (tab, filename) in &targets {
        app.active_tab = *tab;
        terminal.draw(|f| renderer.render(f, &app))?;
        buffer_to_svg(
            terminal.backend().buffer(),
            width,
            height,
            &screenshot_dir.join(filename),
        )?;
        println!("  [OK] Rendered {}", filename);
    }

    // 3. Setup_cloud.svg
    app.active_tab = AppTab::Setup;
    *app.setup_manager.browser_active.safe_lock() = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Setup_cloud.svg"),
    )?;
    *app.setup_manager.browser_active.safe_lock() = false;
    println!("  [OK] Rendered Setup_cloud.svg");

    // 3b. Settings_Keys.svg — twenty-three rows in a pane that holds about
    // thirty, so this is the screenshot that shows when it stops fitting.
    app.active_tab = AppTab::Settings;
    app.ui_state
        .settings
        .set_category(ac_tui::ui::tabs::settings::SettingsCategory::Keys);
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Settings_Keys.svg"),
    )?;
    app.ui_state
        .settings
        .set_category(ac_tui::ui::tabs::settings::SettingsCategory::System);
    println!("  [OK] Rendered Settings_Keys.svg");

    // 4. Analysis_Radar.svg
    app.active_tab = AppTab::Analysis;
    app.ui_state.analysis.next_tab();
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Analysis_Radar.svg"),
    )?;
    println!("  [OK] Rendered Analysis_Radar.svg");

    // 5. Help_Modal.svg
    // AppState::show_help, not UIState::show_help. The renderer checks the
    // former; the latter was read by nothing, which is why every generated
    // Help_Modal.svg was byte-identical to the screenshot before it.
    app.show_help = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Help_Modal.svg"),
    )?;
    app.show_help = false;
    println!("  [OK] Rendered Help_Modal.svg");

    // 6. Overlay_Control.svg
    app.ui_state.overlay_mode = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Overlay_Control.svg"),
    )?;
    app.ui_state.overlay_mode = false;
    println!("  [OK] Rendered Overlay_Control.svg");

    println!("\nALL 15 POPULATED REALISTIC VECTOR SVG SCREENSHOTS GENERATED SUCCESSFULLY!");
    Ok(())
}
