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

/// How much of the lap a trace may be missing at the line and still be said
/// to cover it, as a fraction of a lap.
///
/// The analyser records from the first tick after the line — a metre and a
/// half at 300 km/h and 60 Hz, a thousandth of a short lap. Half a percent is
/// generous for that and far short of a corner.
const EDGE: f32 = 0.005;

/// How much quicker than the session's typical time through a section one lap
/// may be before it is a hole in the samples rather than a better corner.
///
/// A corner driven perfectly against one driven badly is a few tenths in a
/// section of five or ten seconds — a few percent. A fifth is a corner that
/// was not driven: a pause, a reset to the pits, a jump in the feed.
const IMPLAUSIBLY_QUICK: f32 = 0.20;

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
    /// Which of the laps handed in that was, by position.
    ///
    /// **So "practise T16" becomes "watch your third lap through T16".** The
    /// spread says a corner is worth an evening; this says which of the
    /// driver's own laps to go and look at, and they have already driven it —
    /// which is the difference between advice and a task.
    ///
    /// An index rather than a lap number, because this module is handed bare
    /// traces and a lap number is the caller's own bookkeeping. Naming it
    /// `best_on_lap` would be a field that lies about what it holds.
    pub best_on: usize,
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
    ///
    /// **From the traces, and short by a sample.** A trace's last point is
    /// before the line by however long the gap between samples is — nine
    /// milliseconds on a sixty-hertz feed — so this is systematically a little
    /// quick. Where the caller has the game's own figure, [`Self::against`]
    /// replaces it: two screens quoting "your best lap" must not quote two
    /// numbers.
    pub best_lap_ms: i32,
}

impl Repeatability {
    /// Use the game's own lap time instead of the trace's last sample.
    ///
    /// The traces are all this module is given, and their last point is before
    /// the line. A caller holding the lap's recorded time has the truth, and
    /// the gap to the best-corners lap is measured from it.
    pub fn against(mut self, best_lap_ms: i32) -> Self {
        if best_lap_ms > 0 {
            self.best_lap_ms = best_lap_ms;
        }
        self
    }

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

/// The session's corners, measured over every valid lap driven in it.
///
/// **In the core because it was in the window.** The window worked this out
/// itself — which laps count, which one decides where the corners are — and
/// the moment the terminal wanted the same answer for its session report
/// there would have been two. The quickest valid lap is the skeleton, as
/// [`measure`] asks, and its own recorded time is the best lap, as
/// [`Repeatability::against`] asks. `None` below three laps, where a spread is
/// two numbers and a guess.
pub fn of_session(laps: &[crate::analyzer::LapData]) -> Option<Repeatability> {
    let driven: Vec<&crate::analyzer::LapData> = laps
        .iter()
        .filter(|lap| !lap.from_file && lap.valid && !lap.telemetry_trace.is_empty())
        .filter(|lap| in_order(&lap.telemetry_trace))
        .collect();
    if driven.len() < 3 {
        return None;
    }
    let quickest = driven
        .iter()
        .filter(|lap| lap.lap_time_ms > 0)
        .min_by_key(|lap| lap.lap_time_ms)
        .copied()
        .unwrap_or(driven[0]);
    let skeleton = crate::corners::detect(&quickest.telemetry_trace);
    if skeleton.is_empty() {
        return None;
    }
    let traces: Vec<&[TelemetryPoint]> = driven
        .iter()
        .map(|lap| lap.telemetry_trace.as_slice())
        .collect();
    Some(measure(&traces, &skeleton).against(quickest.lap_time_ms))
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

        // Which lap each time came from, so the quickest can be pointed at.
        let mut from_lap = Vec::with_capacity(laps.len());
        let mut times = Vec::with_capacity(laps.len());
        let mut entries = Vec::with_capacity(laps.len());
        let mut apexes = Vec::with_capacity(laps.len());
        let mut brakes = Vec::with_capacity(laps.len());

        for (which, lap) in laps.iter().enumerate() {
            // **Only a lap that was there for the whole section.** `time_at`
            // answers outside a trace with the trace's first or last time
            // rather than with nothing, so a lap joined halfway round — or
            // one whose recording began late — came back as having driven
            // every section before it in no time at all. The quickest time
            // through T1 became a few milliseconds, the lap of best corners
            // seconds quicker than anything driven, and the corner nobody
            // drove was the one it said to go and watch.
            if !covers(lap, from, to) {
                continue;
            }
            let (Some(start), Some(end)) = (time_at(lap, from), time_at(lap, to)) else {
                continue;
            };
            if end <= start {
                continue;
            }
            times.push(end - start);
            from_lap.push(which);

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
        // Held to the session's typical time here, not to the quickest lap's:
        // a glitch quick enough to matter usually made its lap the quickest,
        // and would then be its own yardstick. The upper middle, so two laps
        // are judged by the slower of them.
        let typical = {
            let mut sorted = times.clone();
            sorted.sort_unstable();
            sorted[sorted.len() / 2]
        };
        let floor = (typical as f32 * (1.0 - IMPLAUSIBLY_QUICK)) as i32;
        let quickest = times
            .iter()
            .enumerate()
            .filter(|(_, took)| **took >= floor)
            .min_by_key(|(_, took)| **took)
            .map(|(at, took)| (at, *took));
        let (best_at, best) = quickest.unwrap_or((0, 0));
        let best_on = from_lap.get(best_at).copied().unwrap_or(0);
        own_best_sum += best;
        sections.push(Section {
            number: corner.number,
            laps: times.len(),
            mean_ms: mean(&times.iter().map(|ms| *ms as f32).collect::<Vec<_>>()) as i32,
            spread_ms: spread(&times.iter().map(|ms| *ms as f32).collect::<Vec<_>>()) as i32,
            best_ms: best,
            best_on,
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
        .filter(|lap| covers(lap, 0.0, skeleton[0].entry))
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

/// Whether a trace holds the stretch from one distance to another, rather than
/// clamping to it. The line at either end is allowed [`EDGE`] of slack.
fn covers(trace: &[TelemetryPoint], from: f32, to: f32) -> bool {
    let (Some(first), Some(last)) = (trace.first(), trace.last()) else {
        return false;
    };
    let start = if from <= EDGE { EDGE } else { from };
    let end = if to >= 1.0 - EDGE { 1.0 - EDGE } else { to };
    first.distance <= start && last.distance >= end
}

/// Whether the clock and the distance only ever move forward. A clock that
/// runs backwards is a trace from two places stitched together, and every
/// time read off it is a guess at a moment that happened twice.
fn in_order(trace: &[TelemetryPoint]) -> bool {
    trace
        .windows(2)
        .all(|pair| pair[1].time_ms >= pair[0].time_ms && pair[1].distance >= pair[0].distance)
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

    /// **"Practise T16" is advice; "watch your third lap through T16" is a
    /// task.** The index of the lap that did it best is the whole difference,
    /// and it has to point at the lap that was actually quickest there rather
    /// than at the quickest lap overall.
    #[test]
    fn a_section_names_the_lap_that_did_it_best() {
        use crate::corners::Direction;
        // Three laps. The second is slowest overall and quickest through the
        // one corner, which is exactly the case the field exists for.
        let lap = |slow_from: f32, slow_by: f32| -> Vec<TelemetryPoint> {
            (0..=200)
                .map(|step| {
                    let at = step as f32 / 200.0;
                    let late = if at > slow_from { slow_by } else { 0.0 };
                    TelemetryPoint {
                        distance: at,
                        time_ms: (at * 90_000.0 + late) as i32,
                        speed: 180.0,
                        gas: 1.0,
                        brake: 0.0,
                        gear: 5,
                        steer: 0.0,
                        lat_g: 0.0,
                        lon_g: 0.0,
                        slip_avg: 0.0,
                        x: at,
                        y: at,
                        rpms: 7_000,
                        detail: Default::default(),
                    }
                })
                .collect()
        };
        let corner = |number: usize, entry: f32| Corner {
            number,
            direction: Direction::Left,
            entry,
            apex: entry + 0.02,
            exit: entry + 0.05,
            entry_speed: 200.0,
            min_speed: 90.0,
            exit_speed: 150.0,
            peak_lat_g: 1.4,
            brake_point: Some(entry),
            braking: None,
            throttle_point: None,
            throttle_delay_ms: None,
            entry_time_ms: 0,
            exit_time_ms: 1_000,
        };

        // Laps 0 and 2 lose time in the first corner's section; lap 1 does not
        // and loses far more later, so it is the slowest lap overall.
        let first = lap(0.12, 400.0);
        let second = lap(0.60, 3_000.0);
        let third = lap(0.12, 500.0);
        let traces: Vec<&[TelemetryPoint]> = vec![&first, &second, &third];

        let measured = measure(&traces, &[corner(1, 0.10), corner(2, 0.50)]);
        let t1 = measured
            .sections
            .iter()
            .find(|section| section.number == 1)
            .expect("the first corner was measured");
        assert_eq!(
            t1.best_on, 1,
            "the quickest through T1 is the slowest lap overall"
        );
    }
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

    /// **A lap joined halfway round drove nothing before it.** Its trace
    /// starts after T7, and the time it was given there by clamping was a
    /// few milliseconds — which made it the "best" T7 and the lap of best
    /// corners seconds quicker than anything driven.
    #[test]
    fn a_lap_that_was_not_there_drives_no_section() {
        let mut joined = lap(5_000, 90.0, 0.22);
        joined.retain(|point| point.distance >= 0.45);
        let laps: Vec<Vec<TelemetryPoint>> =
            vec![lap(5_200, 90.0, 0.22), lap(5_300, 90.0, 0.22), joined];
        let borrowed: Vec<&[TelemetryPoint]> = laps.iter().map(|lap| lap.as_slice()).collect();
        let measured = measure(&borrowed, &skeleton());
        let seven = &measured.sections[0];
        assert_eq!(seven.laps, 2, "only two laps drove T7");
        assert_eq!(seven.best_ms, 5_200);
        // T8 runs from 0.40 to the line, and the joined lap starts at 0.45:
        // clamped, it "drove" it 2.5 s quicker than anybody did.
        let eight = &measured.sections[1];
        assert_eq!(eight.laps, 2, "nor T8: it began after T8's entry");
        assert_eq!(eight.best_ms, 30_000);
        assert_eq!(
            measured.own_best_lap_ms, 45_200,
            "the opening, the best T7 and T8 as somebody drove them"
        );
    }

    /// **A section crossed in a fraction of the time anybody can is a hole in
    /// the samples**, and it is not allowed to be the best corner — even
    /// when, as it usually would, it made its own lap the quickest.
    #[test]
    fn a_section_no_car_could_drive_is_not_the_best() {
        let laps: Vec<Vec<TelemetryPoint>> = vec![
            lap(5_000, 90.0, 0.22),
            lap(5_100, 90.0, 0.22),
            lap(900, 90.0, 0.22),
        ];
        let borrowed: Vec<&[TelemetryPoint]> = laps.iter().map(|lap| lap.as_slice()).collect();
        let measured = measure(&borrowed, &skeleton());
        let seven = &measured.sections[0];
        assert_eq!(seven.best_ms, 5_000);
        assert_eq!(seven.best_on, 0);
    }

    /// Nothing to measure is not a crash and not a row of zeros.
    #[test]
    fn no_laps_and_no_corners_measure_nothing() {
        assert_eq!(measure(&[], &skeleton()), Repeatability::default());
        let one = lap(5_000, 90.0, 0.22);
        assert_eq!(measure(&[one.as_slice()], &[]), Repeatability::default());
    }
}
