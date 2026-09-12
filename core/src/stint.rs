//! How the stint is going, and how much of it is left.
//!
//! **The question a race engineer is actually asked.** Everything else in this
//! program looks at one lap; a race is a stint, and the thing a driver wants
//! to know at lap eight is whether the tyres have gone, how much they are
//! costing, and how many laps are left in them.
//!
//! # What is measured, and what is refused
//!
//! Three answers, all of them from numbers the game published:
//!
//! * **the trend in lap time**, fitted over the valid laps;
//! * **where the stint turned** — the lap after which the times stopped
//!   improving, which is the moment a driver feels and cannot time;
//! * **tyre wear a lap**, and therefore laps left, where the game publishes
//!   wear at all.
//!
//! What is deliberately *not* here is a fuel correction. A car gets quicker as
//! it lightens, so a raw trend is degradation minus fuel burn — and turning
//! one into the other needs a seconds-per-litre figure that varies by car, by
//! circuit and by fuel load. Inventing one would be this project's worst class
//! of bug, a plausible number nobody measured, so the trend is reported as
//! what it is and the sentence says which way fuel pushes it. A driver who
//! knows their car can do the arithmetic; a program that guessed would be
//! wrong quietly.
//!
//! # It refuses to answer early
//!
//! Three laps is not a trend. A driver learning a circuit gets quicker for
//! several laps whatever the tyres are doing, and a line through four points
//! two of which were the driver improving says nothing about rubber.

use crate::analyzer::LapData;
use crate::confidence::Evidence;
use crate::engineer::{Chain, Recommendation, Severity};

/// Fewer valid laps than this and there is nothing to say.
///
/// **Six, not three.** The first laps of a stint are a driver arriving: they
/// improve because the person is learning and the tyres are warming, and a
/// trend fitted through those is a measurement of somebody getting used to a
/// circuit.
pub const ENOUGH: usize = 6;

/// A trend flatter than this is a stint holding station, in seconds a lap.
///
/// Two hundredths a lap is a second over a fifty-lap race and is below what
/// traffic and a missed apex do to any single lap.
pub const FLAT: f32 = 0.02;

/// Past this, the tyres are going and it is worth saying so.
pub const GOING: f32 = 0.08;

/// How a stint is going.
#[derive(Debug, Clone, PartialEq)]
pub struct Stint {
    /// How many valid laps this was worked out from.
    pub laps: usize,
    /// Seconds a lap the times are trending. Positive is getting slower.
    ///
    /// **Fuel burn is in this number and pushes it down.** See the module's
    /// own note on why it is not taken out.
    pub per_lap_s: f32,
    /// How much the laps agree with that line, 0 to 1. A stint with one
    /// mistake in it agrees less, and the advice says so rather than reporting
    /// the mistake as degradation.
    pub agreement: f32,
    /// The lap number after which the times stopped improving, when there was
    /// one. The moment a driver feels and cannot time.
    pub turned_at: Option<i32>,
    /// The best lap of the stint, in milliseconds.
    pub best_ms: i32,
    /// The last lap, so "how far off are you now" is a subtraction.
    pub latest_ms: i32,
    /// Tyre wear a lap, as a fraction, where the game publishes wear.
    pub wear_per_lap: Option<f32>,
    /// How many laps of tyre are left at that rate. `None` where wear is not
    /// published — **not zero**, which would read as tyres about to fail.
    pub laps_left: Option<f32>,
}

impl Stint {
    /// How much slower the latest lap is than the best of the stint, in
    /// seconds. Negative would be the latest *being* the best.
    pub fn off_the_best(&self) -> f32 {
        (self.latest_ms - self.best_ms) as f32 / 1000.0
    }

    /// Whether the tyres are going.
    pub fn going_off(&self) -> bool {
        self.per_lap_s >= GOING
    }

    /// One line for a panel that has room for one.
    pub fn headline(&self) -> String {
        if self.per_lap_s.abs() < FLAT {
            return format!("holding station over {} laps", self.laps);
        }
        let way = match self.per_lap_s > 0.0 {
            true => "slower",
            false => "quicker",
        };
        format!(
            "{:.2} s a lap {way} over {} laps",
            self.per_lap_s.abs(),
            self.laps
        )
    }

    /// The findings.
    pub fn advice(&self) -> Vec<Recommendation> {
        let mut found = Vec::new();

        if self.going_off() {
            let mut evidence = Evidence::new();
            evidence.observe(self.per_lap_s);
            evidence = evidence.averaged_over(self.laps as u32);
            found.push(Recommendation {
                component: "TYRES".to_string(),
                category: "Stint".to_string(),
                severity: match self.per_lap_s >= GOING * 2.0 {
                    true => Severity::Warning,
                    false => Severity::Info,
                },
                message: format!("The pace is falling {:.2} s a lap.", self.per_lap_s),
                action: match self.turned_at {
                    Some(lap) => format!(
                        "It turned after lap {lap}. If this is a race, the stint is done \
                         earning; if it is practice, this is the point to look at pressures \
                         and how hard the first laps were driven."
                    ),
                    None => "Look at pressures and at how hard the opening laps were driven — \
                             tyres that go early were usually asked for too much early."
                        .to_string(),
                },
                parameters: Vec::new(),
                confidence: 0.0,
                chain: Some(Chain {
                    cause: "the tyres are giving less each lap than the lap before".to_string(),
                    effect: format!(
                        "{:.2} s a lap over {} laps, and the latest is {:.2} s off the best of \
                         the stint. Fuel burn is in that number and pushes it the other way, so \
                         the rubber is losing at least this much.",
                        self.per_lap_s,
                        self.laps,
                        self.off_the_best()
                    ),
                    confirm: "whether the next three laps keep falling at the same rate"
                        .to_string(),
                    evidence,
                }),
            });
        }

        // **Wear is the one number that needs no coefficient**, so where the
        // game publishes it, it is worth its own line: it answers "how many
        // laps are left" directly rather than by inference.
        if let (Some(per_lap), Some(left)) = (self.wear_per_lap, self.laps_left)
            && left < 12.0
        {
            let mut evidence = Evidence::new();
            evidence.observe(per_lap);
            evidence = evidence.averaged_over(self.laps as u32);
            found.push(Recommendation {
                component: "TYRES".to_string(),
                category: "Wear".to_string(),
                severity: match left < 5.0 {
                    true => Severity::Critical,
                    false => Severity::Warning,
                },
                message: format!("About {left:.0} laps of tyre left."),
                action: "Plan the stop around that rather than around the fuel.".to_string(),
                parameters: Vec::new(),
                confidence: 0.0,
                chain: Some(Chain {
                    cause: "the tread is going at a steady rate".to_string(),
                    effect: format!("{:.1} % a lap over {} laps", per_lap * 100.0, self.laps),
                    confirm: "the wear figure in three laps against this estimate".to_string(),
                    evidence,
                }),
            });
        }

        found
    }
}

/// How this stint is going, or `None` while it is too early to say.
///
/// `wear_published` is the game's own answer, from
/// [`Capabilities::tyre_wear`](crate::games::Capabilities). **Not inferred
/// from the trace**: `Detail::measured` says a lap carried detail at all, not
/// that every field in it was measured, so reading wear behind that flag gave
/// "about 78 laps of tyre left" on a game that publishes none. It was safe by
/// accident on a real session — unpublished wear is a flat line of zeros and
/// the fit rejects it — and safe by accident is how the aero rule got through
/// for a year.
pub fn look(laps: &[LapData], wear_published: bool) -> Option<Stint> {
    // A lap somebody spun on is not a slower lap, it is not a lap.
    let valid: Vec<&LapData> = laps
        .iter()
        .filter(|lap| lap.valid && lap.lap_time_ms > 0 && !lap.from_file)
        .collect();
    if valid.len() < ENOUGH {
        return None;
    }

    let best = valid
        .iter()
        .min_by_key(|lap| lap.lap_time_ms)
        .map(|lap| (lap.lap_number, lap.lap_time_ms))?;

    // **The trend from the best lap onward, not from the pit exit.**
    //
    // Fitting the whole stint reported a set of tyres falling away as a driver
    // getting *quicker*: an out-lap on cold rubber is several seconds off, and
    // two laps like that at the start drag a least-squares line down far
    // enough to outvote five laps of real degradation. The screen said "0.14 s
    // a lap quicker" in green beside "the latest lap is 2.85 s off the best",
    // which is two numbers contradicting each other about one stint.
    //
    // `ENOUGH` was meant to handle this and cannot: it gates how many laps
    // there are, not which of them were the driver arriving. The best lap is
    // where arriving stopped — before it the tyres and the driver were both
    // still coming in, after it whatever happens is the stint.
    let from = match valid.iter().filter(|lap| lap.lap_number >= best.0).count() >= 3 {
        true => best.0,
        // Unless the best is so late that nothing follows it, in which case
        // nothing has turned and the whole stint is the answer.
        false => valid.first().map(|lap| lap.lap_number).unwrap_or(0),
    };
    let points: Vec<(f32, f32)> = valid
        .iter()
        .filter(|lap| lap.lap_number >= from)
        .map(|lap| (lap.lap_number as f32, lap.lap_time_ms as f32 / 1000.0))
        .collect();
    let (slope, agreement) = fit(&points)?;

    // **Where it turned**: the best lap, if every lap after it was slower.
    // A best lap in the middle with quicker laps after it is a driver having a
    // moment, not a stint turning.
    let after: Vec<&&LapData> = valid.iter().filter(|lap| lap.lap_number > best.0).collect();
    let turned_at =
        (after.len() >= 2 && after.iter().all(|lap| lap.lap_time_ms > best.1)).then_some(best.0);

    // Wear, where the game publishes it: the difference between the first and
    // the last lap that carried a figure, over the laps between them.
    let worn: Vec<(f32, f32)> = match wear_published {
        false => Vec::new(),
        true => valid
            .iter()
            .filter_map(|lap| {
                let point = lap.telemetry_trace.last()?;
                point.detail.measured.then(|| {
                    let worst = point
                        .detail
                        .tyre_wear
                        .iter()
                        .copied()
                        .fold(f32::MAX, f32::min);
                    (lap.lap_number as f32, worst)
                })
            })
            .collect(),
    };
    // Wear in this game counts *down* from one, so a falling figure is rubber
    // going: the rate is the negated slope, and a rising one is a tyre change
    // rather than a tyre growing back.
    let (wear_per_lap, laps_left) = match fit(&worn) {
        Some((slope, _)) if slope < -0.0005 => {
            let rate = -slope;
            let left = worn.last().map(|(_, worn)| worn / rate);
            (Some(rate), left)
        }
        _ => (None, None),
    };

    Some(Stint {
        laps: valid.len(),
        per_lap_s: slope,
        agreement,
        turned_at,
        best_ms: best.1,
        latest_ms: valid.last().map(|lap| lap.lap_time_ms).unwrap_or(0),
        wear_per_lap,
        laps_left,
    })
}

/// The slope of the least-squares line, and how much the points agree with it.
///
/// Agreement is `r²`, which is nought where the points say nothing about each
/// other and one where they lie on the line. Returned beside the slope because
/// a slope without it is a number that always exists however meaningless.
fn fit(points: &[(f32, f32)]) -> Option<(f32, f32)> {
    if points.len() < 3 {
        return None;
    }
    let count = points.len() as f32;
    let mean_x = points.iter().map(|(x, _)| x).sum::<f32>() / count;
    let mean_y = points.iter().map(|(_, y)| y).sum::<f32>() / count;
    let mut across = 0.0;
    let mut along = 0.0;
    for (x, y) in points {
        across += (x - mean_x) * (y - mean_y);
        along += (x - mean_x) * (x - mean_x);
    }
    if along.abs() < f32::EPSILON {
        return None;
    }
    let slope = across / along;
    let total: f32 = points.iter().map(|(_, y)| (y - mean_y).powi(2)).sum();
    let left: f32 = points
        .iter()
        .map(|(x, y)| (y - (mean_y + slope * (x - mean_x))).powi(2))
        .sum();
    let agreement = match total > f32::EPSILON {
        true => (1.0 - left / total).clamp(0.0, 1.0),
        false => 0.0,
    };
    Some((slope, agreement))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{Detail, TelemetryPoint};

    /// A finished lap, with a time and optionally a tyre worn to `wear`.
    fn lap(number: i32, ms: i32, wear: Option<f32>) -> LapData {
        LapData {
            lap_number: number,
            lap_time_ms: ms,
            valid: true,
            telemetry_trace: wear
                .map(|worn| {
                    vec![TelemetryPoint {
                        distance: 1.0,
                        time_ms: ms,
                        speed: 180.0,
                        gas: 1.0,
                        brake: 0.0,
                        gear: 5,
                        steer: 0.0,
                        lat_g: 0.0,
                        lon_g: 0.0,
                        slip_avg: 0.0,
                        x: 0.0,
                        y: 0.0,
                        rpms: 7_000,
                        detail: Detail {
                            measured: true,
                            tyre_wear: [worn; 4],
                            ..Default::default()
                        },
                    }]
                })
                .unwrap_or_default(),
            ..Default::default()
        }
    }

    /// **Three laps is not a trend.** The first laps of a stint are a driver
    /// arriving — they improve because the person is learning and the tyres
    /// are warming — and a line through those measures somebody getting used
    /// to a circuit.
    #[test]
    fn it_refuses_to_answer_before_there_is_a_stint() {
        let early: Vec<LapData> = (1..=4).map(|n| lap(n, 90_000 + n * 100, None)).collect();
        assert!(look(&early, true).is_none());
    }

    /// A lap somebody spun on is not a slower lap, it is not a lap.
    #[test]
    fn an_invalid_lap_is_not_a_slow_lap() {
        let mut laps: Vec<LapData> = (1..=7).map(|n| lap(n, 90_000, None)).collect();
        laps[3].lap_time_ms = 120_000;
        laps[3].valid = false;
        let stint = look(&laps, true).expect("six valid laps");
        assert_eq!(stint.laps, 6);
        assert!(
            stint.per_lap_s.abs() < FLAT,
            "a spin is not degradation: {}",
            stint.per_lap_s
        );
    }

    /// The number a race engineer is asked for.
    #[test]
    fn a_stint_that_is_going_off_says_so_and_by_how_much() {
        // A tenth a lap, which is a tyre going.
        let laps: Vec<LapData> = (1..=10)
            .map(|n| lap(n, 90_000 + (n - 1) * 100, None))
            .collect();
        let stint = look(&laps, true).expect("ten laps");
        assert!((stint.per_lap_s - 0.1).abs() < 0.01, "{}", stint.per_lap_s);
        assert!(
            stint.agreement > 0.99,
            "a straight line: {}",
            stint.agreement
        );
        assert!(stint.going_off());
        assert!((stint.off_the_best() - 0.9).abs() < 0.01);

        let advice = stint.advice();
        assert!(!advice.is_empty());
        assert!(advice[0].message.contains("0.10"), "{}", advice[0].message);
        // **The one thing that must be said out loud**, or the number is
        // read as the whole truth when it is a floor.
        assert!(
            advice[0]
                .chain
                .as_ref()
                .expect("a chain")
                .effect
                .contains("Fuel burn"),
            "the sentence has to say fuel is in the number"
        );
    }

    /// **An out-lap is not the stint.** Fitting the whole thing reported a set
    /// of tyres falling away as a driver getting quicker: two cold laps at the
    /// start drag a least-squares line down far enough to outvote five laps of
    /// real degradation, and the screen said "0.14 s a lap quicker" in green
    /// beside "the latest lap is 2.85 s off the best".
    #[test]
    fn a_cold_out_lap_does_not_turn_degradation_into_improvement() {
        // The shape of a real stint: scrappy out-lap, rubber comes in, best at
        // lap 4, then away.
        let times = [
            135_000, 131_500, 130_300, 130_000, 130_500, 131_200, 132_000, 132_900,
        ];
        let laps: Vec<LapData> = times
            .iter()
            .enumerate()
            .map(|(at, ms)| lap(at as i32 + 1, *ms, None))
            .collect();

        let stint = look(&laps, true).expect("eight laps");
        assert_eq!(stint.turned_at, Some(4));
        assert!(
            stint.per_lap_s > 0.0,
            "the tyres are going and it must not read as improvement: {}",
            stint.per_lap_s
        );
        assert!(
            (stint.per_lap_s - 0.725).abs() < 0.15,
            "{}",
            stint.per_lap_s
        );
        assert!(stint.going_off());
        assert!(
            !stint.headline().contains("quicker"),
            "{}",
            stint.headline()
        );
    }

    /// A stint holding station is not a finding, and saying it is would make
    /// every other finding worth less.
    #[test]
    fn a_flat_stint_has_nothing_to_report() {
        let laps: Vec<LapData> = (1..=10)
            .map(|n| lap(n, 90_000 + (n % 3) * 10, None))
            .collect();
        let stint = look(&laps, true).expect("ten laps");
        assert!(!stint.going_off());
        assert!(stint.advice().is_empty());
        assert!(
            stint.headline().contains("holding station"),
            "{}",
            stint.headline()
        );
    }

    /// **Where it turned** is the moment a driver feels and cannot time — and
    /// a best lap with quicker laps after it is a driver having a moment,
    /// not a stint turning.
    #[test]
    fn the_lap_it_turned_on_is_only_a_turn_if_nothing_after_it_was_better() {
        // Quicker to lap 4, then away.
        let going: Vec<LapData> = (1..=10)
            .map(|n| lap(n, 90_000 - (n.min(4) * 100) + ((n - 4).max(0) * 150), None))
            .collect();
        assert_eq!(look(&going, true).expect("ten laps").turned_at, Some(4));

        // The same stint with the quickest lap last: nothing turned.
        let mut improving: Vec<LapData> = (1..=10).map(|n| lap(n, 91_000 - n * 50, None)).collect();
        improving.last_mut().expect("a last lap").lap_time_ms = 89_000;
        assert_eq!(look(&improving, true).expect("ten laps").turned_at, None);
    }

    /// **Wear needs no coefficient**, so where the game publishes it the
    /// answer is direct: how many laps are left.
    #[test]
    fn wear_says_how_many_laps_are_left() {
        // A percent a lap, from a full tyre.
        let laps: Vec<LapData> = (1..=10)
            .map(|n| lap(n, 90_000, Some(1.0 - n as f32 * 0.01)))
            .collect();
        let stint = look(&laps, true).expect("ten laps");
        let rate = stint.wear_per_lap.expect("wear was published");
        assert!((rate - 0.01).abs() < 0.002, "{rate}");
        let left = stint.laps_left.expect("and therefore laps left");
        assert!((left - 90.0).abs() < 5.0, "{left}");
    }

    /// **The game's answer, not the trace's.** `Detail::measured` says a lap
    /// carried detail at all, not that every field in it was measured — so
    /// reading wear behind that flag offered "about 78 laps of tyre left" on a
    /// game that publishes none. It was safe by accident on a real session,
    /// where unpublished wear is a flat line of zeros the fit rejects, and
    /// safe by accident is how the aero rule survived a year.
    #[test]
    fn a_game_that_withholds_wear_is_asked_rather_than_guessed() {
        // Wear present in the trace, and the game saying it publishes none —
        // which is exactly the demo's own Competizione mode, and exactly the
        // shape of a lap recorded before a capability was declared.
        let laps: Vec<LapData> = (1..=10)
            .map(|n| lap(n, 90_000, Some(1.0 - n as f32 * 0.01)))
            .collect();

        let asked = look(&laps, false).expect("ten laps");
        assert_eq!(asked.wear_per_lap, None);
        assert_eq!(asked.laps_left, None);
        assert!(asked.advice().iter().all(|one| one.category != "Wear"));

        // And with the game saying it does, the same laps answer.
        assert!(look(&laps, true).expect("ten laps").laps_left.is_some());
    }

    /// **`None` and never zero** where the game publishes no wear. Zero laps
    /// left reads as tyres about to fail, which is the worst possible thing to
    /// invent.
    #[test]
    fn a_game_that_publishes_no_wear_is_not_a_tyre_about_to_fail() {
        let laps: Vec<LapData> = (1..=10).map(|n| lap(n, 90_000 + n * 90, None)).collect();
        let stint = look(&laps, true).expect("ten laps");
        assert_eq!(stint.wear_per_lap, None);
        assert_eq!(stint.laps_left, None);
        assert!(
            stint.advice().iter().all(|one| one.category != "Wear"),
            "nothing is said about wear that was never measured"
        );
    }
}
