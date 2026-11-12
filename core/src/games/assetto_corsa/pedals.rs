//! The pedals' curves: where each one starts, where it is full, and how it
//! rises in between — read and written in `controls.ini`.
//!
//! **Three numbers a pedal, as the game keeps them.** `[THROTTLE]`,
//! `[BRAKES]` and `[CLUTCH]` each hold `MIN` and `MAX` — the part of the
//! pedal's travel that counts, from -1 to 1, the wrong way round for a pedal
//! that reads backwards — and the brake a `GAMMA`, the curve between. The
//! throttle's and the clutch's gamma are read only by Custom Shaders Patch,
//! which adds them; the game alone ignores them, and [`Pedal::gamma_needs_patch`]
//! says so rather than offering a setting that does nothing.
//!
//! **The output is what the game feels**, by the same arithmetic the game
//! and Content Manager use: the pedal's position taken inside its range,
//! clamped, raised to the gamma. [`output`] is that, so a curve drawn from
//! it is the curve the car gets.

use super::ini;

/// One pedal the game reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pedal {
    pub section: &'static str,
    pub label: &'static str,
    /// The game alone ignores this pedal's gamma; the patch reads it.
    pub gamma_needs_patch: bool,
}

pub const PEDALS: [Pedal; 3] = [
    Pedal {
        section: "THROTTLE",
        label: "Throttle",
        gamma_needs_patch: true,
    },
    Pedal {
        section: "BRAKES",
        label: "Brake",
        gamma_needs_patch: false,
    },
    Pedal {
        section: "CLUTCH",
        label: "Clutch",
        gamma_needs_patch: true,
    },
];

/// A pedal's curve, in the terms a driver sets it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Curve {
    /// Where the pedal starts to count, 0 to 1 of its travel — the dead zone.
    pub from: f32,
    /// Where it is full, 0 to 1 — below 1 is saturation, full early.
    pub to: f32,
    /// The pedal reads backwards: the game's `MIN` is above its `MAX`.
    pub invert: bool,
    /// The curve: 1 straight, above 1 gentle at first, below 1 sharp.
    pub gamma: f32,
}

impl Default for Curve {
    fn default() -> Self {
        Self {
            from: 0.0,
            to: 1.0,
            invert: false,
            gamma: 1.0,
        }
    }
}

/// The steepest and gentlest curve offered.
pub const GAMMA_RANGE: (f32, f32) = (0.3, 3.0);

fn number(text: &str, section: &str, key: &str, default: f32) -> f32 {
    ini::value(text, section, key)
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(default)
}

/// The curve in the file, as the game would take it.
pub fn read(text: &str, pedal: &Pedal) -> Curve {
    let from = (number(text, pedal.section, "MIN", -1.0) * 0.5 + 0.5).clamp(0.0, 1.0);
    let to = (number(text, pedal.section, "MAX", 1.0) * 0.5 + 0.5).clamp(0.0, 1.0);
    let invert = from > to;
    let (from, to) = match invert {
        true => (1.0 - from, 1.0 - to),
        false => (from, to),
    };
    Curve {
        from,
        to,
        invert,
        gamma: number(text, pedal.section, "GAMMA", 1.0).clamp(GAMMA_RANGE.0, GAMMA_RANGE.1),
    }
}

/// What the game feels for a pedal at `travel` (0 released, 1 floored).
pub fn output(curve: &Curve, travel: f32) -> f32 {
    if !travel.is_finite() {
        return 0.0;
    }
    let travel = match curve.invert {
        true => 1.0 - travel,
        false => travel,
    };
    let span = (curve.to - curve.from).max(1e-6);
    let inside = ((travel - curve.from) / span).clamp(0.0, 1.0);
    inside.powf(curve.gamma.clamp(GAMMA_RANGE.0, GAMMA_RANGE.1))
}

fn decimal(value: f32) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match text {
        "" | "-" | "-0" => "0".to_string(),
        text => text.to_string(),
    }
}

/// The text with a pedal's curve written — the binding, and every other
/// line, left as it was. The range is kept at least a tenth of the travel
/// wide, so no curve makes a pedal all or nothing.
pub fn with_curve(text: &str, pedal: &Pedal, curve: &Curve) -> String {
    let from = curve.from.clamp(0.0, 0.9);
    let to = curve.to.clamp(from + 0.1, 1.0);
    let (min, max) = match curve.invert {
        true => (1.0 - from, 1.0 - to),
        false => (from, to),
    };
    let mut out = ini::with_value(text, pedal.section, "MIN", &decimal(min * 2.0 - 1.0));
    out = ini::with_value(&out, pedal.section, "MAX", &decimal(max * 2.0 - 1.0));
    ini::with_value(
        &out,
        pedal.section,
        "GAMMA",
        &decimal(curve.gamma.clamp(GAMMA_RANGE.0, GAMMA_RANGE.1)),
    )
}

/// The wheel's rotation as the game takes it: `[STEER] LOCK`, in degrees.
pub fn steering_lock(text: &str) -> u32 {
    number(text, "STEER", "LOCK", 900.0)
        .clamp(40.0, 9000.0)
        .round() as u32
}

/// The angle the game turns the car's wheel to for a wheel at `position`
/// (-1 full left, 1 full right), in degrees.
pub fn steering_angle(lock: u32, position: f32) -> f32 {
    position.clamp(-1.0, 1.0) * lock as f32 / 2.0
}

/// What a wheel's own rotation and the game's lock say together, when they
/// disagree by more than a few degrees: the wheel turns `wheel` degrees end
/// to end, the game thinks `game`. Equal is `None`.
pub fn rotation_mismatch(wheel: u32, game: u32) -> Option<i64> {
    let gap = i64::from(game) - i64::from(wheel);
    (gap.abs() > 5).then_some(gap)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    const FILE: &str = "[THROTTLE]\nJOY=0\nAXLE=2\nMIN=-1\nMAX=1\n\n\
                        [BRAKES]\nJOY=0\nAXLE=3\nMIN=-0.8\nMAX=0.6\nGAMMA=2.2 ; mine\n\n\
                        [CLUTCH]\nJOY=0\nAXLE=1\nMIN=1\nMAX=-1\n";

    #[test]
    fn a_curve_reads_as_the_game_takes_it() {
        let throttle = read(FILE, &PEDALS[0]);
        assert_eq!(throttle, Curve::default());
        let brake = read(FILE, &PEDALS[1]);
        assert!((brake.from - 0.1).abs() < 1e-4);
        assert!((brake.to - 0.8).abs() < 1e-4);
        assert!((brake.gamma - 2.2).abs() < 1e-4);
        assert!(!brake.invert);
        let clutch = read(FILE, &PEDALS[2]);
        assert!(clutch.invert);
        assert!((clutch.from - 0.0).abs() < 1e-4 && (clutch.to - 1.0).abs() < 1e-4);
        // Nothing written at all is the game's own default.
        assert_eq!(read("", &PEDALS[1]), Curve::default());
    }

    /// **The output is the game's arithmetic**: nothing in the dead zone,
    /// full from the saturation point, the gamma in between, and an
    /// inverted pedal read from the other end.
    #[test]
    fn the_output_is_what_the_car_gets() {
        let brake = read(FILE, &PEDALS[1]);
        assert_eq!(output(&brake, 0.05), 0.0);
        assert_eq!(output(&brake, 0.9), 1.0);
        let half = output(&brake, 0.45);
        assert!((half - 0.5f32.powf(2.2)).abs() < 1e-3, "{half}");
        let straight = Curve::default();
        assert!((output(&straight, 0.3) - 0.3).abs() < 1e-6);
        let clutch = read(FILE, &PEDALS[2]);
        assert!((output(&clutch, 0.0) - 1.0).abs() < 1e-6);
        assert!((output(&clutch, 1.0) - 0.0).abs() < 1e-6);
        // Never outside 0..1, whatever comes in.
        for travel in [-5.0, -0.1, 0.0, 0.5, 1.0, 7.0, f32::NAN] {
            let out = output(&brake, travel);
            assert!((0.0..=1.0).contains(&out), "{travel} → {out}");
        }
    }

    /// **Written and read back the same**, the binding and the comment
    /// untouched, and a curve squeezed narrower than a tenth widened.
    #[test]
    fn a_curve_is_written_and_read_back() {
        let wanted = Curve {
            from: 0.05,
            to: 0.9,
            invert: false,
            gamma: 1.6,
        };
        let after = with_curve(FILE, &PEDALS[1], &wanted);
        let back = read(&after, &PEDALS[1]);
        assert!((back.from - 0.05).abs() < 1e-3 && (back.to - 0.9).abs() < 1e-3);
        assert!((back.gamma - 1.6).abs() < 1e-3);
        assert!(after.contains("AXLE=3"));
        assert!(after.contains("[CLUTCH]\nJOY=0\nAXLE=1"));
        let inverted = with_curve(
            FILE,
            &PEDALS[2],
            &Curve {
                invert: true,
                ..wanted
            },
        );
        let back = read(&inverted, &PEDALS[2]);
        assert!(back.invert && (back.from - 0.05).abs() < 1e-3);
        let squeezed = with_curve(
            FILE,
            &PEDALS[0],
            &Curve {
                from: 0.7,
                to: 0.71,
                ..Curve::default()
            },
        );
        let back = read(&squeezed, &PEDALS[0]);
        assert!(back.to - back.from >= 0.099, "{back:?}");
    }

    #[test]
    fn the_wheel_and_the_game_are_held_to_the_same_rotation() {
        assert_eq!(steering_lock("[STEER]\nLOCK=1080\n"), 1080);
        assert_eq!(steering_lock(""), 900);
        assert_eq!(steering_lock("[STEER]\nLOCK=5\n"), 40);
        assert_eq!(steering_angle(900, 0.2), 90.0);
        assert_eq!(steering_angle(900, -3.0), -450.0);
        assert_eq!(rotation_mismatch(900, 900), None);
        assert_eq!(rotation_mismatch(900, 903), None);
        assert_eq!(rotation_mismatch(900, 1080), Some(180));
        assert_eq!(rotation_mismatch(1080, 900), Some(-180));
    }
}
