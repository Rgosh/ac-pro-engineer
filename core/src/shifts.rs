//! Every gear change of a lap, and what it cost.
//!
//! **Nothing reads this and it is free.** A lap trace already carries the
//! gear and the revs at every sample; the number of shifts was counted and
//! then thrown away as a single integer. What a driver can act on is not "38
//! shifts" — it is *which* of them were early, which were late, and how long
//! the car spent on the limiter getting nowhere.
//!
//! Three faults, and each is a different thing to change:
//!
//! * **Short-shifting.** Up a gear well below the power band, so the engine
//!   drops out of it and the car pulls worse than it could. Costs most on the
//!   exit of a slow corner, which is where it is most tempting.
//! * **Bouncing off the limiter.** Every millisecond above the limit is a
//!   millisecond of no acceleration at all. A little is a driver reacting; a
//!   lot is a gear held too long, and on a long straight it is free time.
//! * **Downshifting into the limiter.** Asking for a gear the revs cannot take
//!   — the car refuses it or the engine screams — which unsettles the rear
//!   exactly where a driver is already braking.
//!
//! # It says nothing it cannot measure
//!
//! The power band is not published by either simulator. What is published is
//! the rev limit, so "below the band" is a fraction of the limit and is stated
//! as such: this module reports *where the revs were*, not what the engine's
//! torque curve was doing. A car whose limit is unknown gets no verdict at
//! all rather than a verdict against a guessed number.

use crate::analyzer::TelemetryPoint;
use serde::{Deserialize, Serialize};

/// Below this fraction of the limit, an upshift has left the power band.
///
/// **Two thirds, and it is deliberately forgiving.** Engines differ, and this
/// has no torque curve to read — so it is set where nobody sensible argues:
/// almost nothing makes peak power at 65 % of its limit, and a driver shifting
/// there is losing drive on any car.
const SHORT_OF: f32 = 0.65;

/// Within this fraction of the limit, the engine is on it.
const ON_THE_LIMIT: f32 = 0.995;

/// One gear change.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Shift {
    /// Where on the lap it happened, as a fraction.
    pub at: f32,
    /// The gear before it, and after.
    pub from: i32,
    pub to: i32,
    /// The revs it was taken at, as a fraction of the limit.
    pub revs: f32,
}

impl Shift {
    pub fn upshift(&self) -> bool {
        self.to > self.from
    }

    /// An upshift taken so far below the limit that the engine falls out of
    /// its band.
    pub fn short(&self) -> bool {
        self.upshift() && self.revs < SHORT_OF
    }

    /// A downshift into revs the engine cannot take.
    pub fn over(&self) -> bool {
        !self.upshift() && self.revs > ON_THE_LIMIT
    }
}

/// What a lap's gear changes looked like.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Shifting {
    pub shifts: Vec<Shift>,
    /// How much of the lap was spent on the limiter, in milliseconds.
    ///
    /// **Time, not a count.** One sample over the limit is a driver reacting;
    /// four hundred milliseconds is a gear held too long, and the difference
    /// is the whole point of measuring it this way.
    pub on_the_limiter_ms: i32,
    /// The longest single stretch of it, and where that was.
    pub worst_limiter_ms: i32,
    pub worst_limiter_at: f32,
}

impl Shifting {
    /// The upshifts taken below the power band.
    pub fn short(&self) -> Vec<&Shift> {
        self.shifts.iter().filter(|shift| shift.short()).collect()
    }

    /// The downshifts asked for above the limit.
    pub fn over(&self) -> Vec<&Shift> {
        self.shifts.iter().filter(|shift| shift.over()).collect()
    }

    /// Whether any of it is worth saying out loud.
    ///
    /// A tenth on the limiter across a whole lap is nothing; four is a gear
    /// somebody is holding. One short shift is a driver being careful out of a
    /// wet corner; three is a habit.
    pub fn worth_saying(&self) -> bool {
        self.worst_limiter_ms >= 250 || self.short().len() >= 3 || !self.over().is_empty()
    }
}

/// Read a lap's gear changes.
///
/// `rev_limit` is what the game publishes for this car. **Zero means unknown**
/// and everything that depends on it goes unreported rather than guessed — a
/// verdict measured against a number nobody published is the class of bug this
/// project cares most about.
pub fn read(trace: &[TelemetryPoint], rev_limit: i32) -> Shifting {
    if trace.len() < 2 || rev_limit <= 0 {
        return Shifting::default();
    }
    let limit = rev_limit as f32;

    let mut shifts = Vec::new();
    let mut on_the_limiter_ms = 0;
    let mut worst_limiter_ms = 0;
    let mut worst_limiter_at = 0.0;
    let mut run_ms = 0;
    let mut run_from = 0.0;

    for pair in trace.windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        let step = (after.time_ms - before.time_ms).max(0);

        if after.gear != before.gear && before.gear > 0 && after.gear > 0 {
            shifts.push(Shift {
                at: after.distance,
                from: before.gear,
                to: after.gear,
                // The revs *going into* the change: after it they are already
                // whatever the new gear made them, which says nothing about
                // the decision.
                revs: before.rpms as f32 / limit,
            });
        }

        if before.rpms as f32 / limit >= ON_THE_LIMIT {
            if run_ms == 0 {
                run_from = before.distance;
            }
            run_ms += step;
            on_the_limiter_ms += step;
            if run_ms > worst_limiter_ms {
                worst_limiter_ms = run_ms;
                worst_limiter_at = run_from;
            }
        } else {
            run_ms = 0;
        }
    }

    Shifting {
        shifts,
        on_the_limiter_ms,
        worst_limiter_ms,
        worst_limiter_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trace of `(distance, gear, rpms)` a tenth of a second apart.
    fn trace(steps: &[(f32, i32, i32)]) -> Vec<TelemetryPoint> {
        steps
            .iter()
            .enumerate()
            .map(|(index, (distance, gear, rpms))| TelemetryPoint {
                distance: *distance,
                time_ms: index as i32 * 100,
                speed: 200.0,
                gas: 1.0,
                brake: 0.0,
                gear: *gear,
                steer: 0.0,
                lat_g: 0.0,
                lon_g: 0.0,
                slip_avg: 0.0,
                x: 0.0,
                y: 0.0,
                rpms: *rpms,
                detail: crate::analyzer::Detail::default(),
            })
            .collect()
    }

    /// A car whose limit nobody published gets no verdict at all.
    #[test]
    fn an_unknown_rev_limit_reports_nothing() {
        let lap = trace(&[(0.1, 3, 7000), (0.2, 4, 5000)]);
        assert_eq!(read(&lap, 0), Shifting::default());
    }

    /// An upshift at two thirds of the limit is a short shift; one at the top
    /// is not.
    #[test]
    fn a_short_shift_is_the_one_taken_off_the_band() {
        let lap = trace(&[
            (0.10, 3, 5000),
            (0.11, 4, 6000),
            (0.20, 4, 7900),
            (0.21, 5, 8000),
        ]);
        let read = read(&lap, 8000);

        assert_eq!(read.shifts.len(), 2);
        assert!(read.shifts[0].short(), "5000 of 8000 is off the band");
        assert!(!read.shifts[1].short(), "7900 of 8000 is not");
        assert_eq!(read.short().len(), 1);
    }

    /// Time on the limiter is time, and the longest stretch is found with it.
    #[test]
    fn the_limiter_is_measured_in_time_not_in_samples() {
        // Four samples at the limit is three steps of 100 ms between them.
        let lap = trace(&[
            (0.10, 5, 7000),
            (0.20, 5, 8000),
            (0.30, 5, 8000),
            (0.40, 5, 8000),
            (0.50, 5, 8000),
            (0.60, 5, 7000),
        ]);
        let read = read(&lap, 8000);
        assert_eq!(read.on_the_limiter_ms, 400);
        assert_eq!(read.worst_limiter_ms, 400);
        assert!((read.worst_limiter_at - 0.20).abs() < 0.001);
        assert!(read.worth_saying());
    }

    /// A downshift into the limit is its own fault, and it is not a short
    /// shift.
    #[test]
    fn a_downshift_into_the_limit_is_reported_as_one() {
        let lap = trace(&[(0.40, 5, 8000), (0.41, 4, 6000)]);
        let read = read(&lap, 8000);
        assert_eq!(read.over().len(), 1);
        assert!(read.short().is_empty());
        assert!(read.worth_saying());
    }

    /// Neutral and reverse are not gears somebody shifted into on purpose.
    #[test]
    fn neutral_is_not_a_gear_change() {
        let lap = trace(&[(0.10, 3, 7000), (0.11, 0, 3000), (0.12, 3, 7000)]);
        assert!(read(&lap, 8000).shifts.is_empty());
    }

    /// A clean lap says nothing.
    #[test]
    fn a_tidy_lap_is_not_worth_saying() {
        let lap = trace(&[(0.10, 3, 7800), (0.11, 4, 6200), (0.50, 4, 7900)]);
        let read = read(&lap, 8000);
        assert!(!read.worth_saying());
    }
}
