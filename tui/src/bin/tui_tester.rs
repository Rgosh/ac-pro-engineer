use ac_core::ac_structs::{AcGraphics, AcPhysics, AcStatic, StringU16_33};
use ac_core::analyzer::{LapData, RadarStats, TelemetryPoint};
use ac_core::config::Language;
use ac_core::engineer::{Recommendation, Severity};
use ac_core::overlay::OverlayMode;
use ac_core::session_info::SessionInfo;
use ac_tui::ui::UIRenderer;
use ac_tui::{AppStage, AppState, AppTab, SafeLock};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::fs;
use std::path::Path;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn buffer_to_svg(
    buffer: &ratatui::buffer::Buffer,
    width: u16,
    height: u16,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let char_w = 10u32;
    let char_h = 20u32;
    let header_h = 32u32;
    let font_size = 14u32;
    let content_w = (width as u32) * char_w;
    let content_h = (height as u32) * char_h;
    let total_w = content_w + 16;
    let total_h = content_h + header_h + 16;

    let mut svg = String::with_capacity(64 * 1024);
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}" style="background-color: #0b0f19; font-family: 'Fira Code', 'JetBrains Mono', 'Cascadia Code', 'Consolas', monospace; font-size: {}px;">"#,
        total_w, total_h, total_w, total_h, font_size
    ));
    svg.push('\n');

    // Outer Card Frame with Shadow & Border
    svg.push_str(&format!(
        r##"  <rect x="4" y="4" width="{}" height="{}" rx="10" fill="#0d1117" stroke="#30363d" stroke-width="1.5"/>"##,
        total_w - 8, total_h - 8
    ));
    svg.push('\n');

    // Window Control Buttons (Red, Yellow, Green)
    svg.push_str(r##"  <circle cx="20" cy="20" r="5.5" fill="#ff5f56"/>"##);
    svg.push('\n');
    svg.push_str(r##"  <circle cx="36" cy="20" r="5.5" fill="#ffbd2e"/>"##);
    svg.push('\n');
    svg.push_str(r##"  <circle cx="52" cy="20" r="5.5" fill="#27c93f"/>"##);
    svg.push('\n');

    // Window Title Text
    svg.push_str(&format!(
        r##"  <text x="{}" y="24" fill="#8b949e" font-size="12px" font-weight="bold" text-anchor="middle">AC Pro Engineer v0.2.3 — High-Performance Sim Telemetry Suite</text>"##,
        total_w / 2
    ));
    svg.push('\n');

    // Divider Line
    svg.push_str(&format!(
        r##"  <line x1="4" y1="{}" x2="{}" y2="{}" stroke="#21262d" stroke-width="1"/>"##,
        header_h, total_w - 4, header_h
    ));
    svg.push('\n');

    // 1. Draw Cell Backgrounds
    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            let bg_color = match cell.bg {
                ratatui::style::Color::DarkGray => "#21262d",
                ratatui::style::Color::Gray => "#30363d",
                ratatui::style::Color::Blue => "#1f6feb",
                ratatui::style::Color::Red => "#da3633",
                ratatui::style::Color::Green => "#238636",
                ratatui::style::Color::Yellow => "#9e6a03",
                ratatui::style::Color::Cyan => "#1b7c83",
                ratatui::style::Color::Magenta => "#8957e5",
                _ => continue,
            };

            let px = 8 + (x as u32) * char_w;
            let py = header_h + (y as u32) * char_h;
            svg.push_str(&format!(
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                px, py, char_w, char_h, bg_color
            ));
            svg.push('\n');
        }
    }

    // 2. Draw Cell Text Glyphs
    for y in 0..height {
        for x in 0..width {
            let cell = buffer.get(x, y);
            let symbol = cell.symbol();
            if symbol.is_empty() || symbol == " " {
                continue;
            }

            let fg_color = match cell.fg {
                ratatui::style::Color::Red => "#ff7b72",
                ratatui::style::Color::Green => "#7ee787",
                ratatui::style::Color::Yellow => "#f2cc60",
                ratatui::style::Color::Blue => "#79c0ff",
                ratatui::style::Color::Magenta => "#d2a8ff",
                ratatui::style::Color::Cyan => "#56d4dd",
                ratatui::style::Color::Gray => "#8b949e",
                ratatui::style::Color::DarkGray => "#484f58",
                ratatui::style::Color::White => "#f0f6fc",
                _ => "#c9d1d9",
            };

            let px = 8 + (x as u32) * char_w + 1;
            let py = header_h + (y as u32) * char_h + 15;

            let escaped = escape_xml(symbol);
            svg.push_str(&format!(
                r#"  <text x="{}" y="{}" fill="{}">{}</text>"#,
                px, py, fg_color, escaped
            ));
            svg.push('\n');
        }
    }

    svg.push_str("</svg>\n");

    fs::write(output_path, svg)?;
    Ok(())
}

fn create_populated_app_state() -> AppState {
    let mut app = AppState::new(OverlayMode::External);
    app.config.language = Language::English;
    app.is_connected = true;
    app.is_game_running = true;

    // Session Info
    let mut sess = SessionInfo::default();
    sess.car_name = "Ferrari SF70H".to_string();
    sess.track_name = "Autodromo Nazionale Monza".to_string();
    sess.track_config = "GP".to_string();
    sess.player_name = "Pro Driver".to_string();
    sess.session_type = "Practice".to_string();
    sess.lap_count = 5;
    sess.session_time_left = 1_800_000.0;
    sess.max_rpm = 12500;
    sess.max_fuel = 110.0;
    app.session_info = sess;

    // Mock Physics
    let mut phys = AcPhysics::default();
    phys.speed_kmh = 248.5;
    phys.rpms = 11450;
    phys.gear = 6;
    phys.fuel = 36.8;
    phys.gas = 0.94;
    phys.brake = 0.0;
    phys.clutch = 0.0;
    phys.steer_angle = -0.12;
    phys.acc_g = [1.38, 0.0, 0.65];
    phys.wheels_pressure = [27.4, 27.6, 27.5, 27.3];
    phys.tyre_temp_i = [89.2, 88.0, 92.1, 90.5];
    phys.tyre_temp_m = [86.4, 85.2, 89.0, 87.8];
    phys.tyre_temp_o = [82.1, 81.0, 85.2, 84.0];
    phys.brake_temp = [450.0, 442.0, 380.0, 375.0];
    phys.air_temp = 22.5;
    phys.road_temp = 34.0;
    phys.tc = 3.0;
    phys.abs = 2.0;

    app.mock_physics = Some(phys);

    // Mock Graphics
    let mut gfx = AcGraphics::default();
    gfx.surface_grip = 0.98;
    gfx.completed_laps = 5;
    gfx.i_current_time = 42500;
    gfx.i_last_time = 81452;
    gfx.i_best_time = 81452;
    gfx.position = 2;
    gfx.fuel_x_lap = 2.85;
    app.mock_graphics = Some(gfx);

    // Mock Static
    let mut stat = AcStatic::default();
    stat.max_rpm = 12500;
    stat.max_fuel = 110.0;
    stat.car_model = StringU16_33::from("ks_ferrari_sf70h");
    stat.track = StringU16_33::from("monza");
    app.mock_static = Some(stat);

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
    app.physics_history = history;

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
        avg_pressure: 27.4,
        min_corner_speed_avg: 78.5,
        fuel_used: 2.85,
        gear_shifts: 42,
        peak_lat_g: 2.45,
        peak_brake_g: 4.85,
        avg_tyre_temp: [88.5, 87.2, 91.0, 89.4],
        max_brake_temp: [580.0, 565.0, 490.0, 485.0],
        pressure_deviation: 0.15,
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
        message: "Front Right pressure target optimal at 27.5 PSI (+0.2 PSI recommended)".to_string(),
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
    app.ui_state.show_help = true;
    terminal.draw(|f| renderer.render(f, &app))?;
    buffer_to_svg(
        terminal.backend().buffer(),
        width,
        height,
        &screenshot_dir.join("Help_Modal.svg"),
    )?;
    app.ui_state.show_help = false;
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

    println!("\nALL 14 POPULATED REALISTIC VECTOR SVG SCREENSHOTS GENERATED SUCCESSFULLY!");
    Ok(())
}
