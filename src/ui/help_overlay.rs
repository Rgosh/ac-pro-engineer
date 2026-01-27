use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, current_tab_index: usize) {
    let popup_area = centered_rect(85, 80, area);
    f.render_widget(Clear, popup_area);

    let (title, content) = get_help_content(current_tab_index);

    let block = Block::default()
        .title(format!(" PADDOCK DATA ASSISTANT: {} ", title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .style(Style::default().fg(Color::Yellow).bg(Color::Black))
        .border_style(Style::default().fg(Color::Cyan));

    let p = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    f.render_widget(p, popup_area);
}

fn get_help_content(tab_index: usize) -> (&'static str, Vec<Line<'static>>) {
    let t = |text: &'static str| Line::from(Span::raw(text));
    let head = |text: &'static str| {
        Line::from(Span::styled(
            text,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))
    };
    let crit = |text: &'static str| {
        Line::from(Span::styled(
            format!("🚨 CRITICAL: {}", text),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ))
    };
    let warn = |text: &'static str| {
        Line::from(Span::styled(
            format!("⚠️ PHYSICS: {}", text),
            Style::default().fg(Color::Yellow),
        ))
    };
    let fix = |text: &'static str| {
        Line::from(Span::styled(
            format!("🔧 ADJUSTMENT: {}", text),
            Style::default().fg(Color::LightGreen),
        ))
    };

    match tab_index {
        0 => ("DASHBOARD (F1)", vec![
            head("DASHBOARD: TIMING & ENGINE MANAGEMENT"),
            Line::from(""),
            warn("SHIFT LIGHTS (POWER BAND):"),
            t("Do not shift at the redline. Modern GT cars lose power at the limiter."),
            t("The RPM bar calculates the peak torque curve for your specific car."),
            fix("Shift the exact millisecond the bar turns BLUE to maximize straight-line speed."),
            Line::from(""),
            warn("ENGINE THERMALS:"),
            t("Water Temp: Regulates block cooling. Target ~90°C."),
            t("Oil Temp: Lubrication viscosity. Target ~100°C."),
            crit("If >115°C, engine damage is occurring. Power drops. Open radiator tape or check for body damage."),
            Line::from(""),
            warn("DELTA DISPLAY:"),
            t("Compares current lap vs your BEST session lap. GREEN = Faster. RED = Slower."),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        1 => ("TELEMETRY (F2)", vec![
            head("LIVE TELEMETRY: RAW DATA FEED"),
            Line::from(""),
            warn("LIVE TYRE DATA:"),
            t("Displays the current dynamic pressure and thermal load on the tyres in real-time."),
            t("Use this screen during qualifying outlaps to warm up tyres symmetrically."),
            Line::from(""),
            warn("SUSPENSION TRAVEL:"),
            t("Watch the bar graphs to see how much of your shock absorber travel is used."),
            fix("If bars max out constantly, increase Bump Stop stiffness (Packer) or ride height."),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        2 => ("ENGINEER (F3)", vec![
            head("ENGINEER: AERO, TYRE THERMODYNAMICS & SUSPENSION"),
            Line::from(""),
            warn("AERODYNAMICS & RAKE (RIDE HEIGHT):"),
            t("Rake is the difference between Rear and Front Ride Height. Higher rake = more front downforce."),
            crit("CAR GLOWS RED FRONT: Understeer detected (Loss of front grip)."),
            fix("Lower front ride height, raise rear ride height, or increase Front Splitter."),
            crit("CAR GLOWS RED REAR: Oversteer detected (Rear step-out)."),
            fix("Lower rear ride height, increase Rear Wing, or soften rear springs."),
            Line::from(""),
            warn("TYRE PHYSICS (I/M/O):"),
            t("Formula: Delta = |Inside_Temp - Outside_Temp|."),
            t("The inside must work harder, but not too hard. Ideal Delta is ~10°C."),
            fix("Delta > 15°C: Camber is too negative. Stand the tyre up (e.g. from -3.5 to -3.0)."),
            fix("Delta < 5°C: Camber is too positive. Lay the tyre down for more cornering grip (-2.0 to -3.0)."),
            Line::from(""),
            warn("BRAKE CAPACITY:"),
            t("Brakes transfer heat to the tyre core. Target: 350-500°C."),
            fix("If >650°C: Open Brake Ducts to prevent fade and tyre cooking."),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        3 => ("SETUP BROWSER (F4)", vec![
            head("SETUP CLOUD: COMMUNITY DATABASE"),
            Line::from(""),
            warn("HOW IT WORKS:"),
            t("Accesses a remote database of e-sports setups via standard HTTP queries."),
            t("Use UP/DOWN to browse setups. Use LEFT/RIGHT to switch cars."),
            fix("Press [ D ] to inject the setup directly into AC. No restarting required."),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        4 => ("ANALYSIS (F5)", vec![
            head("MOTEC GRAPHICS: TRACE ANALYSIS"),
            Line::from(""),
            t("Data Analysis is about comparing YOUR inputs to a REFERENCE (Ghost)."),
            Line::from(""),
            warn("THROTTLE HESITATION:"),
            t("Look at the Throttle graph out of slow corners. If the line stair-steps, you lack mechanical grip."),
            fix("Soften Rear Spring rate or lower differential preload."),
            Line::from(""),
            warn("BRAKING EFFICIENCY:"),
            t("Look at the Brake graph slope. A vertical drop means lockup. A smooth release means good trail braking."),
            Line::from(""),
            t("CONTROLS: [ S ] Save Lap | [ L ] Load Lap | [ C ] Toggle Ghost"),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        5 => ("STRATEGY (F6)", vec![
            head("PIT WALL: FUEL LOAD CALCULATOR"),
            Line::from(""),
            warn("CALCULATION MATRIX:"),
            t("1. Calculates true fuel burned per meter."),
            t("2. Extrapolates based on Session Time remaining."),
            crit("OUT OF FUEL PREDICTION: Triggered when Estimated Laps > Laps Available in Tank."),
            fix("Lift and Coast: Lift throttle 50 meters earlier than normal and coast into braking zone."),
            fix("Short Shift: Shift at lower RPMs to consume less fuel per engine cycle."),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        6 => ("FFB & INPUTS (F7)", vec![
            head("HARDWARE: SIGNAL CLIPPING & LINEARITY"),
            Line::from(""),
            warn("FORCE FEEDBACK CLIPPING:"),
            t("Clipping occurs when AC calculates a physics force (e.g. 15Nm) that exceeds your hardware limit."),
            t("The signal is truncated (clipped) at 100%, erasing all tyre slip and kerb details."),
            crit("RED GRAPH: You are driving blind through the steering wheel."),
            fix("Lower the 'Gain' slider in AC Control Settings until peaks barely touch Yellow."),
            Line::from(""),
            warn("INPUT TELEMETRY:"),
            t("G-Force Circle: Tracks lateral (cornering) vs longitudinal (braking/accel) loads."),
            t("Trail Braking: You should see the brake trace slowly decrease as G-forces rise in corner entry."),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        7 => ("SETTINGS (F8)", vec![
            head("APPLICATION SETTINGS"),
            Line::from(""),
            warn("INTERFACE TUNING:"),
            t("You can change the UI language and adjust the update rate of the terminal."),
            t("Note: Lower update rate means less CPU usage, but the UI might feel less smooth."),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        8 => ("USER GUIDE (F9)", vec![
            head("AC PRO ENGINEER MANUAL"),
            Line::from(""),
            t("You are viewing the comprehensive setup and telemetry guide."),
            t("Use UP/DOWN ARROWS to scroll through the different physics chapters."),
            t("Press any F-key (F1-F8) to return to live data."),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
        _ => ("GENERAL", vec![
            head("NAVIGATION & OVERLAYS"),
            Line::from(""),
            warn("F1 - F9: Module Switching"),
            warn("F10: Enable Compact Overlay (shrinks UI for single monitors)"),
            warn("H: Show/Hide this Engineering Assistant"),
            warn("Q / ESC: Exit safely"),
            Line::from(""),
            t("[ PRESS 'H' TO CLOSE ]"),
        ]),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
