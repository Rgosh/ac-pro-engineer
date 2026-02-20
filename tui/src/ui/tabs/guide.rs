use crate::AppState;
use ratatui::{prelude::*, widgets::*};

pub fn render(f: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" AC PRO ENGINEER: GRAND PRIX ENGINEERING HANDBOOK ")
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(Color::White));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(inner);

    let current_chapter = app.ui_state.setup_list_state.selected().unwrap_or(0);

    render_toc(f, layout[0], &app.ui_state.setup_list_state);
    render_content(f, layout[1], current_chapter);
}

fn render_toc(f: &mut Frame<'_>, area: Rect, state: &ListState) {
    let block = Block::default().borders(Borders::RIGHT).title(" SECTIONS ");

    let items = vec![
        ListItem::new(Span::styled(
            "1. Philosophy of Speed",
            Style::default().fg(Color::White),
        )),
        ListItem::new(Span::styled(
            "2. Advanced Braking Physics",
            Style::default().fg(Color::Cyan),
        )),
        ListItem::new(Span::styled(
            "3. Traction & Differentials",
            Style::default().fg(Color::Cyan),
        )),
        ListItem::new(Span::styled(
            "4. Aero: Centers of Pressure",
            Style::default().fg(Color::Yellow),
        )),
        ListItem::new(Span::styled(
            "5. Tyre Molecular Dynamics",
            Style::default().fg(Color::Yellow),
        )),
        ListItem::new(Span::styled(
            "6. Suspension: Frequencies",
            Style::default().fg(Color::Magenta),
        )),
        ListItem::new(Span::styled(
            "7. Dampers: Histograms",
            Style::default().fg(Color::Magenta),
        )),
        ListItem::new(Span::styled(
            "8. FFB: Pneumatic Trail",
            Style::default().fg(Color::Green),
        )),
        ListItem::new(Span::styled(
            "9. Fuel Strategy Math",
            Style::default().fg(Color::LightBlue),
        )),
        ListItem::new(Span::styled(
            "10. Ghost Data Analysis",
            Style::default().fg(Color::Gray),
        )),
        ListItem::new(Span::styled(
            "11. Setup: Car Layouts (FR/MR)",
            Style::default().fg(Color::White),
        )),
        ListItem::new(Span::styled(
            "12. Setup: Rain Engineering",
            Style::default().fg(Color::White),
        )),
        ListItem::new(Span::styled(
            "13. Setup Troubleshooting A",
            Style::default().fg(Color::LightRed),
        )),
        ListItem::new(Span::styled(
            "14. Setup Troubleshooting B",
            Style::default().fg(Color::LightRed),
        )),
        ListItem::new(Span::styled(
            "15. Connection Issues",
            Style::default().fg(Color::Red),
        )),
        ListItem::new(Span::styled(
            "16. Technical Support",
            Style::default().fg(Color::Red),
        )),
    ];

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state_clone = state.clone();
    f.render_stateful_widget(list, area, &mut state_clone);
}

fn render_content(f: &mut Frame<'_>, area: Rect, chapter: usize) {
    let mut text = Vec::new();

    let h1 = |t: &'static str| {
        Line::from(Span::styled(
            t,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))
    };
    let h2 = |t: &'static str| {
        Line::from(Span::styled(
            t,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
    };
    let p = |t: &'static str| Line::from(Span::styled(t, Style::default().fg(Color::Gray)));
    let secret = |t: &'static str| {
        Line::from(Span::styled(
            format!("   [SECRET]: {}", t),
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::ITALIC),
        ))
    };
    let warn = |t: &'static str| {
        Line::from(Span::styled(
            format!("   [!] {}", t),
            Style::default().fg(Color::LightRed),
        ))
    };
    let fix = |t: &'static str| {
        Line::from(Span::styled(
            format!("   [FIX]: {}", t),
            Style::default().fg(Color::LightGreen),
        ))
    };
    let art = |t: &'static str| Line::from(Span::styled(t, Style::default().fg(Color::Green)));
    let bad_art = |t: &'static str| Line::from(Span::styled(t, Style::default().fg(Color::Red)));
    let math = |t: &'static str| {
        Line::from(Span::styled(
            format!("   [MATH]: {}", t),
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::ITALIC),
        ))
    };
    let crit = |t: &'static str| {
        Line::from(Span::styled(
            format!("   [CRITICAL]: {}", t),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
    };
    let br = || Line::from("");

    match chapter {
        0 => {
            text.push(h1("1. THE PHILOSOPHY OF SPEED (BEYOND BASICS)"));
            text.push(br());
            text.push(h2("THE 333HZ REALITY"));
            text.push(p("Assetto Corsa is not a game. It is a physics integrator running at 333Hz. Every 3 milliseconds, the engine calculates the load on each tyre node."));
            text.push(p("To go fast, you must stop driving 'visually' and start driving 'mathematically'. You are managing 4 contact patches of rubber, each the size of a credit card."));
            text.push(br());
            text.push(h2("THE FRICTION CIRCLE (GG DIAGRAM)"));
            text.push(art(r#"      ^ Braking (1.5G)"#));
            text.push(art(r#"      |      "#));
            text.push(art(r#"Left <+> Right (Turning)"#));
            text.push(art(r#"      |      "#));
            text.push(art(r#"      v Accel"#));
            text.push(br());
            text.push(p(
                "Most drivers treat inputs as binary switches. Pros treat them as a blend.",
            ));
            text.push(secret("If you are Braking at 100%, you have 0% Grip left for Turning. To turn, you MUST release the brake. This blending is where 90% of lap time is found."));
        }
        1 => {
            text.push(h1("2. ADVANCED BRAKING PHYSICS & TRAIL BRAKING"));
            text.push(br());
            text.push(h2("THE 'SHARK FIN' TRACE"));
            text.push(p(
                "Look at the Analysis Tab (F5). Your brake trace should look like a shark fin.",
            ));
            text.push(art(r#" 100% |   |\     <-- Instant Attack"#));
            text.push(art(r#"      |   | \    <-- Modulation Phase"#));
            text.push(art(r#"      |   |  \   <-- Trail Braking into Apex"#));
            text.push(art(r#"   0% |___|___\_______"#));
            text.push(br());
            text.push(h2("WHY THE RECTANGLE IS SLOW"));
            text.push(bad_art(
                r#" 100% |   |---|  <-- Holding max pressure too long"#,
            ));
            text.push(bad_art(r#"      |   |   |  <-- Sudden release"#));
            text.push(bad_art(r#"   0% |___|___|_______"#));
            text.push(p("Sudden release causes the front suspension to spring up (Rebound). This unloads the front tyres instantly."));
            text.push(warn("Result: Understeer. The car refuses to turn because you removed the weight from the front wheels."));
            text.push(secret(
                "Smooth release = Compressed springs = Turned in car.",
            ));
        }
        2 => {
            text.push(h1("3. TRACTION, DIFFERENTIALS & YAW"));
            text.push(br());
            text.push(h2("DIFFERENTIAL TUNING (THE DARK ART)"));
            text.push(p(
                "The Diff controls how the rear wheels rotate relative to each other.",
            ));
            text.push(br());
            text.push(h2("POWER LOCK (ACCELERATION)"));
            text.push(p("High Lock %: Wheels spin at same speed. Great traction, but pushes the nose wide (Understeer on exit)."));
            text.push(p("Low Lock %: Wheels spin independently. Car rotates easily, but inside wheel spins (One-tyre fire)."));
            text.push(br());
            text.push(h2("COAST LOCK (BRAKING/ENTRY)"));
            text.push(p("This is your 'Stability Control'."));
            text.push(art(
                r#"   [High Coast Lock] -> Car wants to go straight. Stable braking."#,
            ));
            text.push(art(
                r#"   [Low Coast Lock]  -> Car rotates eagerly. Risk of spin on entry."#,
            ));
            text.push(fix(
                "If you spin entering a corner: Increase Coast Lock or Preload.",
            ));
            text.push(fix("If you can't hit the apex: Decrease Coast Lock."));
        }
        3 => {
            text.push(h1("4. AERODYNAMICS: CENTER OF PRESSURE (CoP)"));
            text.push(br());
            text.push(h2("THE AERO SEESAW"));
            text.push(p("Think of your car as a seesaw balanced on the CoP."));
            text.push(art(r#"      Front Wing       CoP        Rear Wing"#));
            text.push(art(r#"          |_____________A_____________|  "#));
            text.push(art(r#"                        ^                "#));
            text.push(br());
            text.push(h2("RAKE ANGLE (THE SECRET WEAPON)"));
            text.push(p(
                "Rake is the angle of the floor. Rear Height - Front Height.",
            ));
            text.push(art(r#"      /---\   "#));
            text.push(art(r#"   __/__|__\__   <-- High Rake (Nose Down)"#));
            text.push(art(r#"  (O)_______(O)  "#));
            text.push(br());
            text.push(secret("Increasing Rake (lifting the rear) shifts Aero Balance FORWARD. This cures high-speed understeer without adding front wing drag."));
            text.push(warn("Too much Rake? The diffuser stalls (airflow detaches). You lose ALL rear grip instantly."));
        }
        4 => {
            text.push(h1("5. TYRE THERMODYNAMICS & HYSTERESIS"));
            text.push(br());
            text.push(h2("MOLECULAR FRICTION"));
            text.push(p("Tyres don't just 'rub'. The rubber deforms into the asphalt pores. This deformation generates heat (Hysteresis)."));
            text.push(br());
            text.push(h2("THE I/M/O MATRIX"));
            text.push(p("We monitor 3 zones: Inner, Middle, Outer."));
            text.push(art(r#"   Inside | Middle | Outside "#));
            text.push(art(r#"    95°C  |  90°C  |  82°C   <-- Ideal Pattern"#));
            text.push(br());
            text.push(p("Why is Inside hotter? Because of Negative Camber. We drive on the inside edge to prepare for the corner roll."));
            text.push(crit(
                "If Middle > Inside: Over-inflation (Ballooning). Reduce Pressure.",
            ));
            text.push(crit("If Outside > Inside: Positive Camber problem. You are rolling over the tyre. Increase Negative Camber immediately."));
        }
        5 => {
            text.push(h1("6. SUSPENSION: FREQUENCIES & MOTION RATIO"));
            text.push(br());
            text.push(h2("NATURAL FREQUENCY (HZ)"));
            text.push(p("Suspension stiffness is best measured in Hz, not N/mm."));
            text.push(p("GT3 Target: 2.5Hz Front / 3.0Hz Rear."));
            text.push(br());
            text.push(h2("THE MOTION RATIO TRAP"));
            text.push(p("Real cars have leverage. A 200 N/mm spring on the car might only be 150 N/mm at the wheel."));
            text.push(math("Wheel_Rate = Spring_Rate * (Motion_Ratio)^2"));
            text.push(secret(
                "Stiff Front Springs = Stable Aero Platform but Understeer.",
            ));
            text.push(secret(
                "Soft Rear Springs = Great Traction but unstable Diffuser height.",
            ));
        }
        6 => {
            text.push(h1("7. DAMPERS: HISTOGRAMS & PACKERS"));
            text.push(br());
            text.push(h2("BUMP VS REBOUND"));
            text.push(p(
                "Bump: Controls how fast the wheel moves UP (hitting a bump).",
            ));
            text.push(p(
                "Rebound: Controls how fast the wheel moves DOWN (returning to track).",
            ));
            text.push(br());
            text.push(h2("PACKERS (BUMP STOPS)"));
            text.push(p(
                "These are rubber pucks that stop the suspension travel physically.",
            ));
            text.push(art(r#"   [ Chassis ] "#));
            text.push(art(r#"       |       "#));
            text.push(art(r#"     [===] <-- Packer (Gap Limiter)"#));
            text.push(art(r#"    (Spring)   "#));
            text.push(br());
            text.push(secret("Use Packers to stop the car from scraping the floor at high speed, while keeping soft springs for slow corners. This is the 'Third Spring' trick."));
        }
        7 => {
            text.push(h1("8. FFB: PNEUMATIC TRAIL & SELF ALIGNMENT"));
            text.push(br());
            text.push(h2("WHY STEERING GOES LIGHT"));
            text.push(p("Force Feedback is generated by 'Pneumatic Trail' - the distance between the tyre's contact patch center and the actual grip forces."));
            text.push(br());
            text.push(art(r#" Grip Force |      /--\      "#));
            text.push(art(r#"            |     /    \     "#));
            text.push(art(r#"            |____/______\___ "#));
            text.push(art(r#" Slip Angle |   6°   10°     "#));
            text.push(br());
            text.push(p("At peak grip (6°), force is highest. Beyond the limit (10°+), pneumatic trail collapses. The steering goes light."));
            text.push(warn("If the wheel goes light mid-corner, DO NOT turn more. You are understeering. Straighten the wheel slightly to regain grip."));
        }
        8 => {
            text.push(h1("9. RACE STRATEGY: FUEL MATH"));
            text.push(br());
            text.push(h2("WEIGHT PENALTY"));
            text.push(p(
                "10L of Fuel = ~7.5kg. In a GT3 car, 10kg costs 0.1s per lap.",
            ));
            text.push(p("Starting with 100L vs 50L is a 0.5s per lap difference."));
            text.push(br());
            text.push(h2("LIFT AND COAST (L&C)"));
            text.push(p(
                "The most efficient way to save fuel without losing time.",
            ));
            text.push(art(r#" Throttle | ____           "#));
            text.push(art(r#"          |     \          "#));
            text.push(art(r#"          |      \_______  "#));
            text.push(art(r#"          | Full | Coast | Brake |"#));
            text.push(br());
            text.push(fix("Lift 100m before the braking zone. Coast. Then brake normally. Saves 0.5L per lap."));
        }
        9 => {
            text.push(h1("10. GHOST DATA ANALYSIS"));
            text.push(br());
            text.push(h2("READING THE DELTA"));
            text.push(p("Enable Ghost Mode [C] in Analysis Tab."));
            text.push(br());
            text.push(h2("CASE A: THE OVERSLOW"));
            text.push(bad_art(r#" Speed |   /--\  (Ghost) "#));
            text.push(bad_art(r#"       |  /    \         "#));
            text.push(bad_art(r#"       | /__    \ (You)  "#));
            text.push(bad_art(r#"       |/   \____\       "#));
            text.push(p(
                "You braked too much and your mid-corner speed is 10km/h lower. Trust the aero.",
            ));
            text.push(br());
            text.push(h2("CASE B: THE LATE THROTTLE"));
            text.push(p("Ghost is at 100% throttle 20 meters before you. Setup issue: Rear instability or too much Diff Power Lock."));
        }
        10 => {
            text.push(h1("11. CAR LAYOUTS: FR vs MR vs RR"));
            text.push(br());
            text.push(h2("FR (FRONT ENGINE) - e.g. AMG, BMW M4"));
            text.push(p("Heavy front. Engine acts as a pendulum."));
            text.push(warn(
                "Characteristics: Stable, but prone to Understeer. Tyres wear evenly.",
            ));
            text.push(fix(
                "Needs stiffer rear springs to help rotation. Use kerbs aggressively.",
            ));
            text.push(br());
            text.push(h2("MR (MID ENGINE) - e.g. Ferrari 296, Audi R8"));
            text.push(p("Weight is central. Low polar moment of inertia."));
            text.push(warn(
                "Characteristics: Extremely agile. Turns fast. Prone to Snap Oversteer.",
            ));
            text.push(fix("Needs high Aero Rake. Smooth inputs required."));
            text.push(br());
            text.push(h2("RR (REAR ENGINE) - e.g. Porsche 911"));
            text.push(p("Engine is behind the rear axle."));
            text.push(warn(
                "Characteristics: Massive traction (squat). Fronts are light (Understeer entry).",
            ));
            text.push(fix("Brake LATE and DEEP to keep weight on the nose. Soft rear springs for max traction."));
        }
        11 => {
            text.push(h1("12. RAIN ENGINEERING (WET SETUP)"));
            text.push(br());
            text.push(h2("THE GOLDEN RULES OF RAIN"));
            text.push(p(
                "Water reduces friction coefficient (mu) from 1.0 to 0.7 or less.",
            ));
            text.push(br());
            text.push(h2("1. SOFTEN EVERYTHING"));
            text.push(fix(
                "Disconnect Anti-Roll Bars (Set to 0 or 1). Soften Springs by 2 clicks.",
            ));
            text.push(p(
                "Why? The car needs to lean to find grip. A stiff car will slide instantly.",
            ));
            text.push(br());
            text.push(h2("2. RAISE THE CAR"));
            text.push(fix("Increase Ride Height by +5mm to +10mm."));
            text.push(p(
                "Why? Prevents aquaplaning (the floor hitting the water layer).",
            ));
            text.push(br());
            text.push(h2("3. MAX WING"));
            text.push(fix("Set Rear Wing to Maximum."));
            text.push(p(
                "Drag doesn't matter in rain (you are slower anyway). Downforce is life.",
            ));
        }
        12 => {
            text.push(h1("13. SETUP TROUBLESHOOTING MATRIX (A)"));
            text.push(br());
            text.push(crit(
                "PROBLEM: Car pushes straight (Understeer) in slow corners.",
            ));
            text.push(fix("1. Soften Front Anti-Roll Bar."));
            text.push(fix("2. Soften Front Springs."));
            text.push(fix("3. Increase Front Camber (more negative)."));
            text.push(br());
            text.push(crit("PROBLEM: Car spins (Oversteer) on corner exit."));
            text.push(fix("1. Soften Rear Springs."));
            text.push(fix("2. Decrease Rear Ride Height."));
            text.push(fix("3. Increase Traction Control (TC2)."));
            text.push(br());
            text.push(crit(
                "PROBLEM: Car feels sluggish/lazy to change direction.",
            ));
            text.push(fix("1. Stiffen ALL Anti-Roll Bars."));
            text.push(fix("2. Increase Front Toe-Out (Negative Toe)."));
        }
        13 => {
            text.push(h1("14. SETUP TROUBLESHOOTING MATRIX (B)"));
            text.push(br());
            text.push(crit("PROBLEM: Locking Front Wheels under braking."));
            text.push(fix("1. Move Brake Bias REARWARD (-1%)."));
            text.push(fix("2. Stiffen Front Springs (prevent diving)."));
            text.push(br());
            text.push(crit("PROBLEM: Unstable Rear under braking (Dancing rear)."));
            text.push(fix("1. Move Brake Bias FORWARD (+1%)."));
            text.push(fix("2. Increase Diff Coast Lock."));
            text.push(br());
            text.push(crit("PROBLEM: Bottoming out on straights (Sparks)."));
            text.push(fix(
                "1. Increase Packers (Bump Stops). Do NOT just raise ride height if aero is good.",
            ));
            text.push(fix("2. Stiffen Fast Bump Dampers."));
        }
        14 => {
            text.push(h1("15. CONNECTION ISSUES & SHARED MEMORY"));
            text.push(br());
            text.push(warn("STATUS: 'WAITING FOR AC...'"));
            text.push(p("This means the app cannot read the RAM map."));
            text.push(br());
            text.push(h2("STEP 1: ENABLE IN CONTENT MANAGER"));
            text.push(p("Go to: Settings -> Assetto Corsa -> System."));
            text.push(p("Ensure 'Shared Memory' is checked."));
            text.push(br());
            text.push(h2("STEP 2: CHECK MEMORY FORMAT"));
            text.push(p("Some mods change the physics format. Ensure 'Project CARS' format is UNCHECKED. We need native AC format."));
            text.push(br());
            text.push(h2("STEP 3: PERMISSIONS"));
            text.push(p("Try running 'ac_pro_engineer.exe' as Administrator. Windows sometimes blocks RAM access."));
        }
        15 => {
            text.push(h1("16. TECHNICAL SUPPORT"));
            text.push(br());
            text.push(h2("SETUP DOWNLOADS FAILED?"));
            text.push(p("If pressing [D] does nothing:"));
            text.push(p(
                "1. Go to game -> Setup -> Save a dummy setup (e.g. 'test').",
            ));
            text.push(p("2. This forces AC to create the folder structure in 'Documents/Assetto Corsa/setups'."));
            text.push(p("3. Try downloading again."));
            text.push(br());
            text.push(h2("INVALID LAPS"));
            text.push(p("If telemetry is not saving: You likely cut the track. Invalid laps are discarded to prevent bad data in the Ghost system."));
        }
        _ => {}
    }

    let p = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .block(Block::default().padding(Padding::new(2, 2, 1, 1)));
    f.render_widget(p, area);
}
