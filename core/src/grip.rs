//! How much of the tyre a driver actually used, corner by corner.
//!
//! **Everybody draws the friction circle; almost nobody scores it.** A g-g
//! plot is a cloud of dots, and a driver looking at one can see that it is not
//! quite round without being able to say which corner made it that shape. The
//! useful question is per corner and it is answerable: through *this* corner,
//! how close to the car's own limit was the tyre kept, and where in the corner
//! was it let go?
//!
//! # The limit is the car's own, measured
//!
//! Nothing here knows a tyre model and nothing here should. The reference is
//! the largest combined acceleration this car has actually produced in this
//! stint — measured, from the same laps being judged. A car nobody has pushed
//! yet has a small envelope and everything looks close to it, which is honest:
//! the number means "close to what you have shown this car can do", and it
//! says so on screen rather than pretending to know a physical maximum.
//!
//! # Where it is let go matters more than the average
//!
//! A corner held at 95 % throughout is a corner being driven. One at 95 % on
//! entry and 70 % from the apex out is a corner where the driver has stopped
//! asking — usually waiting for the car to rotate — and that is a tenth
//! nobody sees on a lap time. So entry, middle and exit are scored apart.

use crate::analyzer::TelemetryPoint;
use crate::corners::Corner;
use serde::{Deserialize, Serialize};

/// Below this share of the measured envelope, the tyre is not being asked for
/// much. Two thirds is a car being driven around, not driven.
const LOOSE: f32 = 0.66;

/// How hard one corner was driven, in three parts.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Used {
    pub number: usize,
    /// The share of the envelope used, averaged over each third of the corner.
    pub entry: f32,
    pub middle: f32,
    pub exit: f32,
    /// The best single instant anywhere in it.
    pub peak: f32,
}

impl Used {
    /// The whole corner, as one number.
    pub fn overall(&self) -> f32 {
        (self.entry + self.middle + self.exit) / 3.0
    }

    /// Which third of the corner is the one being left alone, if any.
    ///
    /// **Only when it is clearly the odd one out.** A corner within a few per
    /// cent across all three is a corner being driven, and naming a "weakest
    /// third" of it would be reading noise aloud.
    pub fn let_go(&self) -> Option<Where> {
        let parts = [
            (Where::Entry, self.entry),
            (Where::Middle, self.middle),
            (Where::Exit, self.exit),
        ];
        let worst = parts.iter().min_by(|a, b| a.1.total_cmp(&b.1)).copied()?;
        let best = parts.iter().map(|part| part.1).fold(0.0_f32, f32::max);
        (best - worst.1 > 0.12 && worst.1 < LOOSE).then_some(worst.0)
    }
}

/// Which part of a corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Where {
    Entry,
    Middle,
    Exit,
}

impl Where {
    pub fn label(self) -> &'static str {
        match self {
            Where::Entry => "entry",
            Where::Middle => "the middle",
            Where::Exit => "exit",
        }
    }
}

/// A lap's corners, scored against what this car has been shown to do.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Grip {
    pub corners: Vec<Used>,
    /// The largest combined g this car produced anywhere in the laps measured.
    ///
    /// On screen beside every number that depends on it, because a percentage
    /// of an unstated reference is a percentage of nothing.
    pub envelope_g: f32,
}

impl Grip {
    /// The corners where the tyre is being left alone, loosest first.
    pub fn loosest(&self) -> Vec<&Used> {
        let mut found: Vec<&Used> = self
            .corners
            .iter()
            .filter(|used| used.overall() < LOOSE)
            .collect();
        found.sort_by(|a, b| a.overall().total_cmp(&b.overall()));
        found
    }
}

/// The largest combined acceleration in these laps: the car's own envelope.
///
/// Combined rather than lateral alone, because braking and turning share one
/// tyre — a driver at 1.4 g of braking is using the same rubber a driver at
/// 1.4 g of cornering is.
pub fn envelope(laps: &[&[TelemetryPoint]]) -> f32 {
    laps.iter()
        .flat_map(|lap| lap.iter())
        .map(combined)
        .fold(0.0_f32, f32::max)
}

/// Score one lap's corners against an envelope.
///
/// An envelope of zero — no laps, or a car that never accelerated — scores
/// nothing rather than dividing by it.
pub fn measure(trace: &[TelemetryPoint], corners: &[Corner], envelope_g: f32) -> Grip {
    if envelope_g <= 0.01 {
        return Grip::default();
    }

    let mut scored = Vec::with_capacity(corners.len());
    for corner in corners {
        let inside: Vec<&TelemetryPoint> = trace
            .iter()
            .filter(|point| point.distance >= corner.entry && point.distance <= corner.exit)
            .collect();
        if inside.len() < 3 {
            continue;
        }

        let third = inside.len() / 3;
        let share = |part: &[&TelemetryPoint]| -> f32 {
            if part.is_empty() {
                return 0.0;
            }
            part.iter().map(|point| combined(point)).sum::<f32>() / part.len() as f32 / envelope_g
        };

        scored.push(Used {
            number: corner.number,
            entry: share(&inside[..third]),
            middle: share(&inside[third..third * 2]),
            exit: share(&inside[third * 2..]),
            peak: inside
                .iter()
                .map(|point| combined(point))
                .fold(0.0_f32, f32::max)
                / envelope_g,
        });
    }

    Grip {
        corners: scored,
        envelope_g,
    }
}

/// Lateral and longitudinal together, as the tyre feels them.
fn combined(point: &TelemetryPoint) -> f32 {
    (point.lat_g * point.lat_g + point.lon_g * point.lon_g).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corners::Direction;

    fn point(distance: f32, lat: f32, lon: f32) -> TelemetryPoint {
        TelemetryPoint {
            distance,
            time_ms: (distance * 90_000.0) as i32,
            speed: 150.0,
            gas: 0.0,
            brake: 0.0,
            gear: 3,
            steer: 0.0,
            lat_g: lat,
            lon_g: lon,
            slip_avg: 0.0,
            x: 0.0,
            y: 0.0,
            rpms: 6000,
            detail: crate::analyzer::Detail::default(),
        }
    }

    fn corner() -> Corner {
        Corner {
            number: 7,
            direction: Direction::Left,
            entry: 0.0,
            apex: 0.5,
            exit: 1.0,
            entry_speed: 200.0,
            min_speed: 90.0,
            exit_speed: 180.0,
            peak_lat_g: 1.5,
            brake_point: None,
            braking: None,
            throttle_point: None,
            throttle_delay_ms: None,
            entry_time_ms: 0,
            exit_time_ms: 0,
        }
    }

    /// The envelope is the biggest combined g anywhere, not the biggest
    /// lateral one — braking and turning share a tyre.
    #[test]
    fn the_envelope_is_the_car_at_its_hardest() {
        let lap = vec![
            point(0.1, 1.0, 0.0),
            point(0.2, 0.0, 1.8),
            point(0.3, 0.9, 0.9),
        ];
        let found = envelope(&[lap.as_slice()]);
        assert!(
            (found - 1.8).abs() < 0.001,
            "the hardest instant was 1.8 g, got {found}"
        );
    }

    /// A corner driven hard on entry and given up at the exit is named, and
    /// the part that was given up is named with it.
    #[test]
    fn the_third_of_the_corner_being_left_alone_is_named() {
        let mut lap = Vec::new();
        for step in 0..9 {
            let distance = step as f32 / 8.0;
            // Hard for the first two thirds, half as hard for the last.
            let g = if step < 6 { 1.9 } else { 0.6 };
            lap.push(point(distance, g, 0.0));
        }
        let measured = measure(&lap, &[corner()], 2.0);
        let used = measured.corners[0];

        assert!(used.entry > 0.9, "entry came out {}", used.entry);
        assert!(used.exit < 0.5, "exit came out {}", used.exit);
        assert_eq!(used.let_go(), Some(Where::Exit));
        assert!((used.peak - 0.95).abs() < 0.01);
    }

    /// A corner driven evenly has no weakest third, however hard it was.
    #[test]
    fn an_even_corner_is_not_accused_of_anything() {
        let lap: Vec<TelemetryPoint> = (0..9)
            .map(|step| point(step as f32 / 8.0, 1.9, 0.0))
            .collect();
        let measured = measure(&lap, &[corner()], 2.0);
        assert_eq!(measured.corners[0].let_go(), None);
        assert!(measured.loosest().is_empty());

        // And one driven evenly but gently is loose without being accused of
        // a particular third — the whole corner is the answer there.
        let gentle: Vec<TelemetryPoint> = (0..9)
            .map(|step| point(step as f32 / 8.0, 0.8, 0.0))
            .collect();
        let measured = measure(&gentle, &[corner()], 2.0);
        assert_eq!(measured.corners[0].let_go(), None);
        assert_eq!(measured.loosest().len(), 1);
    }

    /// No envelope, no percentages — rather than a division by nothing.
    #[test]
    fn a_car_that_has_never_been_pushed_is_scored_against_nothing() {
        let lap = vec![point(0.1, 0.0, 0.0), point(0.2, 0.0, 0.0)];
        assert_eq!(measure(&lap, &[corner()], 0.0), Grip::default());
        assert_eq!(envelope(&[lap.as_slice()]), 0.0);
    }
}
