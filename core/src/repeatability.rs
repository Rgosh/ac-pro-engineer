//! Not how quick, but how *repeatable* — corner by corner.
//!
//! **Every telemetry tool measures consistency by lap time.** A column of laps
//! with a spread of three tenths says a driver is inconsistent, which they
//! already knew, and says nothing about what to do on Tuesday. The question
//! worth answering is which corner the three tenths are in: a driver who is
//! within four hundredths everywhere except one hairpin has one thing to
//! practise, and no lap-time column will ever tell them that.
//!
//! That is what this is. The same sections [`crate::corners::decompose`]
//! charges time to, measured across a stint instead of across two laps:
//!
//! * how long each corner took on average, and how much that wandered;
//! * the quickest that corner was ever taken, and what the average is losing
//!   to it;
//! * and *why* it wandered — the spread of the entry speed, of the apex speed,
//!   and of where the brakes came on.
//!
//! The last of those is the useful one. A corner whose time varies while the
//! apex speed does not is a corner being entered differently; one where the
//! brake point moves by twenty metres is a corner nobody has a reference for
//! yet.
//!
//! # One skeleton, every lap
//!
//! Corner numbers mean nothing between laps — [`crate::corners::detect`] finds
//! them in each trace on its own, and a lap where the driver ran wide can come
//! out with a different count. So the corners of **one** lap are used as the
//! skeleton and every lap is measured against those distances. Nothing is
//! matched, because nothing has to be: a place on the track is a place on the
//! track, and every lap has a time at it.
//!
//! # What it costs
//!
//! One pass per lap per section, over traces already resampled to a few
//! hundred points, and it is asked for when somebody opens the view rather
//! than while a car is moving. Ten laps of twenty corners is a few thousand
//! interpolations — less than drawing one of the plots it sits beside.

use crate::analyzer::TelemetryPoint;
use crate::corners::{Corner, time_at};
use serde::{Deserialize, Serialize};

/// How repeatable one corner was over the laps it was driven.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Section {
    /// The skeleton corner's number, so it can be named "T7" on screen.
    pub number: usize,
    /// How many laps had a usable time through it.
    pub laps: usize,
    /// The mean time through the section, in milliseconds.
    pub mean_ms: i32,
    /// The standard deviation of that time. **The number this module is for.**
    pub spread_ms: i32,
    /// The quickest it was ever taken.
    pub best_ms: i32,
    /// What the average is losing to that best, per lap.
    pub to_gain_ms: i32,
    /// The spread of the speed at the section's entry, km/h.
    pub entry_spread: f32,
    /// The spread of the slowest speed inside it, km/h — the apex.
    pub apex_spread: f32,
    /// The spread of where the brakes first came on, as a fraction of the lap.
    ///
    /// `None` where the corner was taken flat, or braked for on too few laps
    /// to say anything: one brake point is not a spread.
    pub brake_spread: Option<f32>,
}

impl Section {
    /// Whether this corner is worth putting in front of somebody.
    ///
    /// **A threshold, because a list of twenty corners is not an answer.** Two
    /// hundredths of scatter is a driver being human; a tenth is a corner they
    /// have not settled on.
    pub fn worth_practising(&self) -> bool {
        self.spread_ms >= 60 && self.laps >= 3
    }
}

/// A stint's worth of corners, and what the laps in it agree about.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Repeatability {
    pub sections: Vec<Section>,
    /// How many laps went into it.
    pub laps: usize,
    /// What a lap made of this driver's own best corners would be worth, in
    /// milliseconds.
    ///
    /// **Their own, not a model's.** Every one of those corner times was
    /// driven by them, on this car, on this track — so unlike a simulated
    /// optimum it is a number they have already proved they can reach. It is
    /// the sum of the sections' bests plus the run to the first corner.
    pub own_best_lap_ms: i32,
    /// The quickest whole lap actually driven, for the gap to the above.
    pub best_lap_ms: i32,
}

impl Repeatability {
    /// What a lap of their own best corners would save, in milliseconds.
    pub fn to_find_ms(&self) -> i32 {
        (self.best_lap_ms - self.own_best_lap_ms).max(0)
    }

    /// The corners worth practising, worst scatter first.
    pub fn worst(&self) -> Vec<&Section> {
        let mut found: Vec<&Section> = self
            .sections
            .iter()
            .filter(|section| section.worth_practising())
            .collect();
        found.sort_by_key(|section| -section.spread_ms);
        found
    }
}

/// Measure a stint's corners against one lap's corner list.
///
/// `skeleton` should be the corners of the quickest lap — it decides where the
/// sections are, and a clean lap puts them in the right places. Laps too short
/// to hold a section contribute nothing to it rather than a zero, because a
/// zero would drag the mean towards a time nobody drove.
pub fn measure(laps: &[&[TelemetryPoint]], skeleton: &[Corner]) -> Repeatability {
    if laps.is_empty() || skeleton.is_empty() {
        return Repeatability::default();
    }

    let mut sections = Vec::with_capacity(skeleton.len());
    let mut own_best_sum = 0i32;

    for (index, corner) in skeleton.iter().enumerate() {
        // Each corner owns the track from its own entry to the next one's,
        // exactly as `corners::decompose` charges it — so a bad exit is paid
        // for by the corner that led onto it, and the sections tile the lap.
        let from = corner.entry;
        let to = skeleton
            .get(index + 1)
            .map(|next| next.entry)
            .unwrap_or(1.0);

        let mut times = Vec::with_capacity(laps.len());
        let mut entries = Vec::with_capacity(laps.len());
        let mut apexes = Vec::with_capacity(laps.len());
        let mut brakes = Vec::with_capacity(laps.len());

        for lap in laps {
            let (Some(start), Some(end)) = (time_at(lap, from), time_at(lap, to)) else {
                continue;
            };
            if end <= start {
                continue;
            }
            times.push(end - start);

            let inside: Vec<&TelemetryPoint> = lap
                .iter()
                .filter(|point| point.distance >= from && point.distance <= to)
                .collect();
            if let Some(first) = inside.first() {
                entries.push(first.speed);
            }
            if let Some(slowest) = inside.iter().min_by(|a, b| a.speed.total_cmp(&b.speed)) {
                apexes.push(slowest.speed);
            }
            // Where the brakes first came on inside the section. A corner
            // taken flat contributes nothing rather than the section's start.
            if let Some(braked) = inside.iter().find(|point| point.brake > 0.15) {
                brakes.push(braked.distance);
            }
        }

        if times.is_empty() {
            continue;
        }
        let best = *times.iter().min().unwrap_or(&0);
        own_best_sum += best;
        sections.push(Section {
            number: corner.number,
            laps: times.len(),
            mean_ms: mean(&times.iter().map(|ms| *ms as f32).collect::<Vec<_>>()) as i32,
            spread_ms: spread(&times.iter().map(|ms| *ms as f32).collect::<Vec<_>>()) as i32,
            best_ms: best,
            to_gain_ms: (mean(&times.iter().map(|ms| *ms as f32).collect::<Vec<_>>()) as i32
                - best)
                .max(0),
            entry_spread: spread(&entries),
            apex_spread: spread(&apexes),
            // Two brake points are a pair, not a habit.
            brake_spread: (brakes.len() >= 3).then(|| spread(&brakes)),
        });
    }

    // The run from the line to the first corner belongs to no corner and is
    // still time, so the best of it goes into the best-corners lap the same
    // way `decompose` charges it separately.
    let opening_best = laps
        .iter()
        .filter_map(|lap| time_at(lap, skeleton[0].entry))
        .min()
        .unwrap_or(0);

    let best_lap_ms = laps
        .iter()
        .filter_map(|lap| lap.last().map(|point| point.time_ms))
        .filter(|ms| *ms > 0)
        .min()
        .unwrap_or(0);

    Repeatability {
        sections,
        laps: laps.len(),
        own_best_lap_ms: own_best_sum + opening_best,
        best_lap_ms,
    }
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f32>() / values.len() as f32
}

/// The population standard deviation.
///
/// Population rather than sample: these are all the laps there were, not a
/// draw from a larger set of them, and with three laps the difference between
/// the two divisors is louder than anything it would tell somebody.
fn spread(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = mean(values);
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f32>()
        / values.len() as f32;
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corners::Direction;

    /// A lap whose section between `from` and `to` takes `section_ms`.
    ///
    /// Built as a straight ramp of time against distance, which is all this
    /// module reads: it interpolates a time at two distances and subtracts.
    fn lap(section_ms: i32, apex_speed: f32, brake_at: f32) -> Vec<TelemetryPoint> {
        (0..=100)
            .map(|step| {
                let distance = step as f32 / 100.0;
                TelemetryPoint {
                    distance,
                    // The section is 0.2..0.4 in every test below, so time is
                    // laid out to make that stretch cost `section_ms`.
                    time_ms: if distance <= 0.2 {
                        (distance * 50_000.0) as i32
                    } else if distance <= 0.4 {
                        10_000 + ((distance - 0.2) / 0.2 * section_ms as f32) as i32
                    } else {
                        10_000 + section_ms + ((distance - 0.4) * 50_000.0) as i32
                    },
                    speed: if (0.28..0.32).contains(&distance) {
                        apex_speed
                    } else {
                        200.0
                    },
                    brake: if (brake_at..brake_at + 0.02).contains(&distance) {
                        0.9
                    } else {
                        0.0
                    },
                    gas: 0.0,
                    gear: 3,
                    steer: 0.0,
                    lat_g: 0.0,
                    lon_g: 0.0,
                    slip_avg: 0.0,
                    x: 0.0,
                    y: 0.0,
                    rpms: 6000,
                    detail: crate::analyzer::Detail::default(),
                }
            })
            .collect()
    }

    fn corner(number: usize, direction: Direction, entry: f32, apex: f32, exit: f32) -> Corner {
        Corner {
            number,
            direction,
            entry,
            apex,
            exit,
            entry_speed: 200.0,
            min_speed: 90.0,
            exit_speed: 180.0,
            peak_lat_g: 1.5,
            brake_point: Some(entry + 0.01),
            braking: None,
            throttle_point: None,
            throttle_delay_ms: None,
            entry_time_ms: 0,
            exit_time_ms: 0,
        }
    }

    fn skeleton() -> Vec<Corner> {
        vec![
            corner(7, Direction::Left, 0.2, 0.3, 0.4),
            corner(8, Direction::Right, 0.4, 0.5, 0.6),
        ]
    }

    /// The whole point: a corner that wanders is named, and one that does not
    /// is left alone.
    #[test]
    fn the_corner_that_wanders_is_the_one_reported() {
        let laps: Vec<Vec<TelemetryPoint>> = vec![
            lap(5_000, 90.0, 0.22),
            lap(5_400, 80.0, 0.25),
            lap(5_000, 90.0, 0.22),
            lap(5_600, 78.0, 0.26),
        ];
        let borrowed: Vec<&[TelemetryPoint]> = laps.iter().map(|lap| lap.as_slice()).collect();
        let measured = measure(&borrowed, &skeleton());

        let seven = measured
            .sections
            .iter()
            .find(|section| section.number == 7)
            .expect("the skeleton has a T7");
        assert_eq!(seven.laps, 4);
        assert_eq!(seven.best_ms, 5_000);
        assert!(
            seven.spread_ms > 200,
            "four laps spanning 600 ms should show a spread, not {}",
            seven.spread_ms
        );
        assert!(
            seven.to_gain_ms > 200,
            "the average is losing to the best by {} ms",
            seven.to_gain_ms
        );
        assert!(seven.worth_practising());
        assert!(
            seven.apex_spread > 4.0,
            "the apex speed moved by twelve km/h; the spread came out {}",
            seven.apex_spread
        );
        assert!(
            seven.brake_spread.is_some_and(|spread| spread > 0.005),
            "the brake point moved four per cent of a lap"
        );
    }

    /// A driver who does the same thing every lap is told to practise nothing.
    #[test]
    fn a_repeatable_corner_is_not_worth_practising() {
        let laps: Vec<Vec<TelemetryPoint>> = (0..5).map(|_| lap(5_000, 90.0, 0.22)).collect();
        let borrowed: Vec<&[TelemetryPoint]> = laps.iter().map(|lap| lap.as_slice()).collect();
        let measured = measure(&borrowed, &skeleton());

        let seven = &measured.sections[0];
        assert_eq!(seven.spread_ms, 0);
        assert_eq!(seven.to_gain_ms, 0);
        assert!(!seven.worth_practising());
        assert!(measured.worst().is_empty());
    }

    /// Two laps are not a habit: one brake point cannot be a spread and nor
    /// can two.
    #[test]
    fn a_brake_point_needs_three_laps_to_be_a_habit() {
        let laps: Vec<Vec<TelemetryPoint>> = vec![lap(5_000, 90.0, 0.22), lap(5_200, 88.0, 0.25)];
        let borrowed: Vec<&[TelemetryPoint]> = laps.iter().map(|lap| lap.as_slice()).collect();
        let measured = measure(&borrowed, &skeleton());
        assert!(measured.sections[0].brake_spread.is_none());
    }

    /// The lap made of their own best corners is quicker than the best lap
    /// they drove, and it is made of times they actually drove.
    #[test]
    fn the_best_corners_make_a_lap_nobody_has_driven_yet() {
        let laps: Vec<Vec<TelemetryPoint>> = vec![lap(5_000, 90.0, 0.22), lap(5_600, 78.0, 0.26)];
        let borrowed: Vec<&[TelemetryPoint]> = laps.iter().map(|lap| lap.as_slice()).collect();
        let measured = measure(&borrowed, &skeleton());

        assert!(measured.own_best_lap_ms > 0);
        assert!(
            measured.own_best_lap_ms <= measured.best_lap_ms,
            "a lap of best corners cannot be slower than the best lap: {} against {}",
            measured.own_best_lap_ms,
            measured.best_lap_ms
        );
    }

    /// Nothing to measure is not a crash and not a row of zeros.
    #[test]
    fn no_laps_and_no_corners_measure_nothing() {
        assert_eq!(measure(&[], &skeleton()), Repeatability::default());
        let one = lap(5_000, 90.0, 0.22);
        assert_eq!(measure(&[one.as_slice()], &[]), Repeatability::default());
    }
}
