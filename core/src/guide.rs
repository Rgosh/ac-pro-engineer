//! The Grand Prix Engineering Handbook — sixteen chapters, and the one copy.
//!
//! **The words live here and the styling does not.** Both front ends draw this
//! text and neither owns it: the terminal renders a [`Line`] as a coloured
//! ratatui span, a graphical front end renders the same one as a heading or a
//! panel, and the chapter itself is written once.
//!
//! That is the same rule the rest of this crate keeps for numbers — a
//! threshold implemented twice is two thresholds that disagree — applied to
//! prose, where it bites harder: two copies of a paragraph do not fail a test
//! when they drift, they simply tell two drivers different things.
//!
//! The kinds are what the writing actually uses. They are not styles: `Secret`
//! is not "yellow italic", it is *the line the chapter exists for*, and a
//! front end is free to draw that however it draws emphasis.

/// One line of a chapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// The chapter's own title.
    H1(&'static str),
    /// A section within it.
    H2(&'static str),
    /// Prose.
    P(&'static str),
    /// A diagram drawn out of characters. **It must stay monospace and must
    /// not be wrapped** — any other font, or a line break, and it is nonsense.
    Art(&'static str),
    /// The same, showing what a fault looks like rather than what right looks
    /// like.
    BadArt(&'static str),
    /// The line the chapter exists for.
    Secret(&'static str),
    /// A warning.
    Warn(&'static str),
    /// A symptom, in the troubleshooting chapters.
    Crit(&'static str),
    /// What to change about it.
    Fix(&'static str),
    /// A calculation worth showing.
    Math(&'static str),
    /// A break.
    Br,
}

/// A chapter: what it is called, and what it says.
#[derive(Debug, Clone, Copy)]
pub struct Chapter {
    pub title: &'static str,
    pub lines: &'static [Line],
}

/// Every chapter, in reading order.
pub const CHAPTERS: [Chapter; 16] = [
    Chapter {
        title: "1. Philosophy of Speed",
        lines: &[
            Line::H1("1. THE PHILOSOPHY OF SPEED (BEYOND BASICS)"),
            Line::Br,
            Line::H2("THE 333HZ REALITY"),
            Line::P(
                "Assetto Corsa is not a game. It is a physics integrator running at 333Hz. Every 3 milliseconds, the engine calculates the load on each tyre node.",
            ),
            Line::P(
                "To go fast, you must stop driving 'visually' and start driving 'mathematically'. You are managing 4 contact patches of rubber, each the size of a credit card.",
            ),
            Line::Br,
            Line::H2("THE FRICTION CIRCLE (GG DIAGRAM)"),
            Line::Art("^ Braking (1.5G)"),
            Line::Art("|"),
            Line::Art("Left <+> Right (Turning)"),
            Line::Art("|"),
            Line::Art("v Accel"),
            Line::Br,
            Line::P("Most drivers treat inputs as binary switches. Pros treat them as a blend."),
            Line::Secret(
                "If you are Braking at 100%, you have 0% Grip left for Turning. To turn, you MUST release the brake. This blending is where 90% of lap time is found.",
            ),
        ],
    },
    Chapter {
        title: "2. Advanced Braking Physics",
        lines: &[
            Line::H1("2. ADVANCED BRAKING PHYSICS & TRAIL BRAKING"),
            Line::Br,
            Line::H2("THE 'SHARK FIN' TRACE"),
            Line::Art("100% |   |\\     <-- Instant Attack"),
            Line::Art("|   | \\    <-- Modulation Phase"),
            Line::Art("|   |  \\   <-- Trail Braking into Apex"),
            Line::Art("0% |___|___\\_______"),
            Line::Br,
            Line::H2("WHY THE RECTANGLE IS SLOW"),
            Line::BadArt("100% |   |---|  <-- Holding max pressure too long"),
            Line::BadArt("|   |   |  <-- Sudden release"),
            Line::BadArt("0% |___|___|_______"),
            Line::P(
                "Sudden release causes the front suspension to spring up (Rebound). This unloads the front tyres instantly.",
            ),
            Line::Warn(
                "Result: Understeer. The car refuses to turn because you removed the weight from the front wheels.",
            ),
            Line::Secret("Smooth release = Compressed springs = Turned in car."),
        ],
    },
    Chapter {
        title: "3. Traction & Differentials",
        lines: &[
            Line::H1("3. TRACTION, DIFFERENTIALS & YAW"),
            Line::Br,
            Line::H2("DIFFERENTIAL TUNING (THE DARK ART)"),
            Line::P("The Diff controls how the rear wheels rotate relative to each other."),
            Line::Br,
            Line::H2("POWER LOCK (ACCELERATION)"),
            Line::P(
                "High Lock %: Wheels spin at same speed. Great traction, but pushes the nose wide (Understeer on exit).",
            ),
            Line::P(
                "Low Lock %: Wheels spin independently. Car rotates easily, but inside wheel spins (One-tyre fire).",
            ),
            Line::Br,
            Line::H2("COAST LOCK (BRAKING/ENTRY)"),
            Line::P("This is your 'Stability Control'."),
            Line::Art("[High Coast Lock] -> Car wants to go straight. Stable braking."),
            Line::Art("[Low Coast Lock]  -> Car rotates eagerly. Risk of spin on entry."),
            Line::Fix("If you spin entering a corner: Increase Coast Lock or Preload."),
            Line::Fix("If you can't hit the apex: Decrease Coast Lock."),
        ],
    },
    Chapter {
        title: "4. Aero: Centers of Pressure",
        lines: &[
            Line::H1("4. AERODYNAMICS: CENTER OF PRESSURE (CoP)"),
            Line::Br,
            Line::H2("THE AERO SEESAW"),
            Line::P("Think of your car as a seesaw balanced on the CoP."),
            Line::Art("Front Wing       CoP        Rear Wing"),
            Line::Art("|_____________A_____________|"),
            Line::Art("^"),
            Line::Br,
            Line::H2("RAKE ANGLE (THE SECRET WEAPON)"),
            Line::P("Rake is the angle of the floor. Rear Height - Front Height."),
            Line::Art("/---\\"),
            Line::Art("__/__|__\\__   <-- High Rake (Nose Down)"),
            Line::Art("(O)_______(O)"),
            Line::Br,
            Line::Secret(
                "Increasing Rake (lifting the rear) shifts Aero Balance FORWARD. This cures high-speed understeer without adding front wing drag.",
            ),
            Line::Warn(
                "Too much Rake? The diffuser stalls (airflow detaches). You lose ALL rear grip instantly.",
            ),
        ],
    },
    Chapter {
        title: "5. Tyre Molecular Dynamics",
        lines: &[
            Line::H1("5. TYRE THERMODYNAMICS & HYSTERESIS"),
            Line::Br,
            Line::H2("MOLECULAR FRICTION"),
            Line::P(
                "Tyres don't just 'rub'. The rubber deforms into the asphalt pores. This deformation generates heat (Hysteresis).",
            ),
            Line::Br,
            Line::H2("THE I/M/O MATRIX"),
            Line::P("We monitor 3 zones: Inner, Middle, Outer."),
            Line::Art("Inside | Middle | Outside"),
            Line::Art("95°C  |  90°C  |  82°C   <-- Ideal Pattern"),
            Line::Br,
            Line::P(
                "Why is Inside hotter? Because of Negative Camber. We drive on the inside edge to prepare for the corner roll.",
            ),
            Line::Crit("If Middle > Inside: Over-inflation (Ballooning). Reduce Pressure."),
            Line::Crit(
                "If Outside > Inside: Positive Camber problem. You are rolling over the tyre. Increase Negative Camber immediately.",
            ),
        ],
    },
    Chapter {
        title: "6. Suspension: Frequencies",
        lines: &[
            Line::H1("6. SUSPENSION: FREQUENCIES & MOTION RATIO"),
            Line::Br,
            Line::H2("NATURAL FREQUENCY (HZ)"),
            Line::P("Suspension stiffness is best measured in Hz, not N/mm."),
            Line::P("GT3 Target: 2.5Hz Front / 3.0Hz Rear."),
            Line::Br,
            Line::H2("THE MOTION RATIO TRAP"),
            Line::P(
                "Real cars have leverage. A 200 N/mm spring on the car might only be 150 N/mm at the wheel.",
            ),
            Line::Math("Wheel_Rate = Spring_Rate * (Motion_Ratio)^2"),
            Line::Secret("Stiff Front Springs = Stable Aero Platform but Understeer."),
            Line::Secret("Soft Rear Springs = Great Traction but unstable Diffuser height."),
        ],
    },
    Chapter {
        title: "7. Dampers: Histograms",
        lines: &[
            Line::H1("7. DAMPERS: HISTOGRAMS & PACKERS"),
            Line::Br,
            Line::H2("BUMP VS REBOUND"),
            Line::P("Bump: Controls how fast the wheel moves UP (hitting a bump)."),
            Line::P("Rebound: Controls how fast the wheel moves DOWN (returning to track)."),
            Line::Br,
            Line::H2("PACKERS (BUMP STOPS)"),
            Line::P("These are rubber pucks that stop the suspension travel physically."),
            Line::Art("[ Chassis ]"),
            Line::Art("|"),
            Line::Art("[===] <-- Packer (Gap Limiter)"),
            Line::Art("(Spring)"),
            Line::Br,
            Line::Secret(
                "Use Packers to stop the car from scraping the floor at high speed, while keeping soft springs for slow corners. This is the 'Third Spring' trick.",
            ),
        ],
    },
    Chapter {
        title: "8. FFB: Pneumatic Trail",
        lines: &[
            Line::H1("8. FFB: PNEUMATIC TRAIL & SELF ALIGNMENT"),
            Line::Br,
            Line::H2("WHY STEERING GOES LIGHT"),
            Line::P(
                "Force Feedback is generated by 'Pneumatic Trail' - the distance between the tyre's contact patch center and the actual grip forces.",
            ),
            Line::Br,
            Line::Art("Grip Force |      /--\\"),
            Line::Art("|     /    \\"),
            Line::Art("|____/______\\___"),
            Line::Art("Slip Angle |   6°   10°"),
            Line::Br,
            Line::P(
                "At peak grip (6°), force is highest. Beyond the limit (10°+), pneumatic trail collapses. The steering goes light.",
            ),
            Line::Warn(
                "If the wheel goes light mid-corner, DO NOT turn more. You are understeering. Straighten the wheel slightly to regain grip.",
            ),
        ],
    },
    Chapter {
        title: "9. Fuel Strategy Math",
        lines: &[
            Line::H1("9. RACE STRATEGY: FUEL MATH"),
            Line::Br,
            Line::H2("WEIGHT PENALTY"),
            Line::P("10L of Fuel = ~7.5kg. In a GT3 car, 10kg costs 0.1s per lap."),
            Line::P("Starting with 100L vs 50L is a 0.5s per lap difference."),
            Line::Br,
            Line::H2("LIFT AND COAST (L&C)"),
            Line::P("The most efficient way to save fuel without losing time."),
            Line::Art("Throttle | ____"),
            Line::Art("|     \\"),
            Line::Art("|      \\_______"),
            Line::Art("| Full | Coast | Brake |"),
            Line::Br,
            Line::Fix(
                "Lift 100m before the braking zone. Coast. Then brake normally. Saves 0.5L per lap.",
            ),
        ],
    },
    Chapter {
        title: "10. Ghost Data Analysis",
        lines: &[
            Line::H1("10. GHOST DATA ANALYSIS"),
            Line::Br,
            Line::H2("READING THE DELTA"),
            Line::P("Enable Ghost Mode [C] in Analysis Tab."),
            Line::Br,
            Line::H2("CASE A: THE OVERSLOW"),
            Line::BadArt("Speed |   /--\\  (Ghost)"),
            Line::BadArt("|  /    \\"),
            Line::BadArt("| /__    \\ (You)"),
            Line::BadArt("|/   \\____\\"),
            Line::P(
                "You braked too much and your mid-corner speed is 10km/h lower. Trust the aero.",
            ),
            Line::Br,
            Line::H2("CASE B: THE LATE THROTTLE"),
            Line::P(
                "Ghost is at 100% throttle 20 meters before you. Setup issue: Rear instability or too much Diff Power Lock.",
            ),
        ],
    },
    Chapter {
        title: "11. Setup: Car Layouts (FR/MR)",
        lines: &[
            Line::H1("11. CAR LAYOUTS: FR vs MR vs RR"),
            Line::Br,
            Line::H2("FR (FRONT ENGINE) - e.g. AMG, BMW M4"),
            Line::P("Heavy front. Engine acts as a pendulum."),
            Line::Warn("Characteristics: Stable, but prone to Understeer. Tyres wear evenly."),
            Line::Fix("Needs stiffer rear springs to help rotation. Use kerbs aggressively."),
            Line::Br,
            Line::H2("MR (MID ENGINE) - e.g. Ferrari 296, Audi R8"),
            Line::P("Weight is central. Low polar moment of inertia."),
            Line::Warn("Characteristics: Extremely agile. Turns fast. Prone to Snap Oversteer."),
            Line::Fix("Needs high Aero Rake. Smooth inputs required."),
            Line::Br,
            Line::H2("RR (REAR ENGINE) - e.g. Porsche 911"),
            Line::P("Engine is behind the rear axle."),
            Line::Warn(
                "Characteristics: Massive traction (squat). Fronts are light (Understeer entry).",
            ),
            Line::Fix(
                "Brake LATE and DEEP to keep weight on the nose. Soft rear springs for max traction.",
            ),
        ],
    },
    Chapter {
        title: "12. Setup: Rain Engineering",
        lines: &[
            Line::H1("12. RAIN ENGINEERING (WET SETUP)"),
            Line::Br,
            Line::H2("THE GOLDEN RULES OF RAIN"),
            Line::P("Water reduces friction coefficient (mu) from 1.0 to 0.7 or less."),
            Line::Br,
            Line::H2("1. SOFTEN EVERYTHING"),
            Line::Fix("Disconnect Anti-Roll Bars (Set to 0 or 1). Soften Springs by 2 clicks."),
            Line::P("Why? The car needs to lean to find grip. A stiff car will slide instantly."),
            Line::Br,
            Line::H2("2. RAISE THE CAR"),
            Line::Fix("Increase Ride Height by +5mm to +10mm."),
            Line::P("Why? Prevents aquaplaning (the floor hitting the water layer)."),
            Line::Br,
            Line::H2("3. MAX WING"),
            Line::Fix("Set Rear Wing to Maximum."),
            Line::P("Drag doesn't matter in rain (you are slower anyway). Downforce is life."),
        ],
    },
    Chapter {
        title: "13. Setup Troubleshooting A",
        lines: &[
            Line::H1("13. SETUP TROUBLESHOOTING MATRIX (A)"),
            Line::Br,
            Line::Crit("PROBLEM: Car pushes straight (Understeer) in slow corners."),
            Line::Fix("1. Soften Front Anti-Roll Bar."),
            Line::Fix("2. Soften Front Springs."),
            Line::Fix("3. Increase Front Camber (more negative)."),
            Line::Br,
            Line::Crit("PROBLEM: Car spins (Oversteer) on corner exit."),
            Line::Fix("1. Soften Rear Springs."),
            Line::Fix("2. Decrease Rear Ride Height."),
            Line::Fix("3. Increase Traction Control (TC2)."),
            Line::Br,
            Line::Crit("PROBLEM: Car feels sluggish/lazy to change direction."),
            Line::Fix("1. Stiffen ALL Anti-Roll Bars."),
            Line::Fix("2. Increase Front Toe-Out (Negative Toe)."),
        ],
    },
    Chapter {
        title: "14. Setup Troubleshooting B",
        lines: &[
            Line::H1("14. SETUP TROUBLESHOOTING MATRIX (B)"),
            Line::Br,
            Line::Crit("PROBLEM: Locking Front Wheels under braking."),
            Line::Fix("1. Move Brake Bias REARWARD (-1%)."),
            Line::Fix("2. Stiffen Front Springs (prevent diving)."),
            Line::Br,
            Line::Crit("PROBLEM: Unstable Rear under braking (Dancing rear)."),
            Line::Fix("1. Move Brake Bias FORWARD (+1%)."),
            Line::Fix("2. Increase Diff Coast Lock."),
            Line::Br,
            Line::Crit("PROBLEM: Bottoming out on straights (Sparks)."),
            Line::Fix(
                "1. Increase Packers (Bump Stops). Do NOT just raise ride height if aero is good.",
            ),
            Line::Fix("2. Stiffen Fast Bump Dampers."),
        ],
    },
    Chapter {
        title: "15. Connection Issues",
        lines: &[
            Line::H1("15. CONNECTION ISSUES & SHARED MEMORY"),
            Line::Br,
            Line::Warn("STATUS: 'WAITING FOR AC...'"),
            Line::P("This means the app cannot read the RAM map."),
            Line::Br,
            Line::H2("STEP 1: ENABLE IN CONTENT MANAGER"),
            Line::P("Go to: Settings -> Assetto Corsa -> System."),
            Line::P("Ensure 'Shared Memory' is checked."),
            Line::Br,
            Line::H2("STEP 2: CHECK MEMORY FORMAT"),
            Line::P(
                "Some mods change the physics format. Ensure 'Project CARS' format is UNCHECKED. We need native AC format.",
            ),
            Line::Br,
            Line::H2("STEP 3: PERMISSIONS"),
            Line::P(
                "Try running 'ac_pro_engineer.exe' as Administrator. Windows sometimes blocks RAM access.",
            ),
        ],
    },
    Chapter {
        title: "16. Technical Support",
        lines: &[
            Line::H1("16. TECHNICAL SUPPORT"),
            Line::Br,
            Line::H2("SETUP DOWNLOADS FAILED?"),
            Line::P("If pressing [D] does nothing:"),
            Line::P("1. Go to game -> Setup -> Save a dummy setup (e.g. 'test')."),
            Line::P(
                "2. This forces AC to create the folder structure in 'Documents/Assetto Corsa/setups'.",
            ),
            Line::P("3. Try downloading again."),
            Line::Br,
            Line::H2("INVALID LAPS"),
            Line::P(
                "If telemetry is not saving: You likely cut the track. Invalid laps are discarded to prevent bad data in the Ghost system.",
            ),
        ],
    },
];
