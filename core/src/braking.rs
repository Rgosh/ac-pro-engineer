//! What the driver did with the brake pedal, against a reference lap.
//!
//! [`crate::corners`] already measures every braking zone — where the pedal
//! came on, how hard, how long it was trailed, and how all of that differs from
//! the reference. It says nothing about whether any of it was a *mistake*, and
//! it should not: a measurement that judges itself cannot be reused by a rule
//! that judges differently.
//!
//! This is the judging. It reads a [`Decomposition`] and says which of the five
//! things a driver can do wrong with a brake pedal they are doing, in which
//! corners, and what it cost.
//!
//! **Why this is a rule and not a table.** A table of twenty corners with five
//! deltas beside each is a hundred numbers and no instruction. A driver leaves
//! it knowing that T7 was 0.28 slower and not knowing what to do about T7. The
//! whole value is in the last step — "you reach 1.1 g where the reference
//! reaches 1.4, in six corners, and it costs you a third of a second" — and
//! that step is a verdict, so it lives in the core with every other verdict and
//! not in whichever front end drew the table.
//!
//! **One fault per corner, on purpose.** A corner where the driver braked
//! early, softly, and arrived slow has one thing wrong with it and three
//! symptoms. Reporting three findings for one corner triples the reading and
//! divides the attention; [`AtCorner::fault`] picks the one that explains the
//! others, in the order the checks are written.
//!
//! **A pattern, not an incident.** Confidence comes from [`Evidence`] built
//! across corners, so a fault seen once is `Low` and the same fault seen in six
//! corners with the same magnitude is `High`. That is the honest difference
//! between a driver who locked a wheel at T4 and a driver who does not brake
//! hard enough anywhere.

use crate::analyzer::TelemetryPoint;
use crate::confidence::Evidence;
use crate::corners::Decomposition;
use crate::engineer::{Chain, Recommendation, Severity};

/// Differences below these are the measurement, not the driver.
///
/// They are deliberately generous. A brake point is resolved to whatever the
/// telemetry's distance channel resolves to, and at 200 km/h a car covers five
/// metres in under a tenth — reporting a 3 m difference as a fault would fill
/// the screen with corners nobody drove differently.
pub const NOISE_M: f32 = 8.0;
/// Peak deceleration, in g.
pub const NOISE_G: f32 = 0.12;
/// Trail length, in milliseconds.
pub const NOISE_MS: i32 = 100;
/// Minimum speed, in km/h.
pub const NOISE_KMH: f32 = 3.0;

/// A section that cost less than this taught nothing, whatever the pedal did.
///
/// Braking differently from the reference is not a fault unless it cost time.
/// A driver who brakes 10 m earlier and is *faster* through the corner has
/// found something, and telling them to stop would be the program being wrong
/// with confidence.
pub const COSTLY_MS: i32 = 30;

/// A fault worth a whole finding has to add up to this much across the lap.
///
/// A tenth of a second. Below it there is a real difference and no reason to
/// spend a driver's evening on it.
pub const WORTH_SAYING_MS: i32 = 100;

/// Past this, the finding is a `Warning` rather than something to know.
pub const SERIOUS_MS: i32 = 300;

/// The five things that go wrong with a brake pedal.
///
/// Ordered as they are checked, which is by how much telling the driver about
/// one explains the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Peak deceleration below the reference's. The car will stop harder than
    /// it is being asked to.
    Soft,
    /// The brakes came on earlier, and the time went between entry and apex.
    Early,
    /// Off the pedal in one movement, where the reference rolled it off. The
    /// front unloads before the car has finished turning.
    Released,
    /// Still on the pedal long after the reference is off it.
    Trailed,
    /// The braking itself matched and the car still arrived slower. Not a
    /// pedal fault — a commitment one.
    OverSlowed,
}

impl Fault {
    /// The short name a column header uses.
    pub fn label(self) -> &'static str {
        match self {
            Fault::Soft => "SOFT",
            Fault::Early => "EARLY",
            Fault::Released => "RELEASED",
            Fault::Trailed => "TRAILED",
            Fault::OverSlowed => "SLOW IN",
        }
    }

    /// One line, for a row that has no space for the whole finding.
    pub fn summary(self) -> &'static str {
        match self {
            Fault::Soft => "not stopping the car as hard as it will stop",
            Fault::Early => "on the brakes earlier than the reference",
            Fault::Released => "off the pedal in one movement",
            Fault::Trailed => "still braking well past the reference",
            Fault::OverSlowed => "same braking, less speed through the middle",
        }
    }
}

/// One corner's braking, and the verdict on it.
///
/// Every delta is `Option` because a corner taken flat in either lap has no
/// braking to compare, and that is not a difference of zero — it is no answer,
/// and a front end draws it as one.
#[derive(Debug, Clone)]
pub struct AtCorner {
    /// The "7" in "T7", from [`crate::corners::Corner::number`].
    pub number: usize,
    /// What the whole section cost, in milliseconds. Positive is slower.
    pub delta_ms: i32,
    /// Of that, what went between entry and apex — the part braking owns.
    pub entry_loss_ms: Option<i32>,

    /// Where the brakes came on, normalised. Kept so the circuit map can put a
    /// mark on the road without going back to the trace.
    pub start: Option<f32>,
    /// And where they came on in the reference lap, for the second mark.
    pub reference_start: Option<f32>,

    /// Later than the reference, in metres. Positive is later — deeper in.
    pub brake_delta_m: Option<f32>,
    /// Harder than the reference, in g. Positive is harder.
    pub decel_delta_g: Option<f32>,
    /// Longer on the pedal after peak pressure, in milliseconds.
    pub trail_delta_ms: Option<i32>,
    /// Faster through the slowest point, in km/h.
    pub min_speed_delta: Option<f32>,

    /// What was actually reached, so a sentence can quote both numbers rather
    /// than only their difference. "1.1 g where the reference reaches 1.4"
    /// tells a driver what to aim at; "-0.3 g" does not.
    pub peak_decel_g: Option<f32>,
    pub reference_decel_g: Option<f32>,

    /// The one thing wrong with this corner, if anything is.
    pub fault: Option<Fault>,
}

/// Every braking zone of one lap, judged.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub corners: Vec<AtCorner>,
}

impl Report {
    /// Every corner showing one fault, worst first.
    pub fn showing(&self, fault: Fault) -> Vec<&AtCorner> {
        let mut found: Vec<&AtCorner> = self
            .corners
            .iter()
            .filter(|corner| corner.fault == Some(fault))
            .collect();
        found.sort_by_key(|corner| -corner.delta_ms);
        found
    }

    /// What one fault cost across the lap, in milliseconds.
    pub fn cost_of(&self, fault: Fault) -> i32 {
        self.corners
            .iter()
            .filter(|corner| corner.fault == Some(fault))
            .map(|corner| corner.entry_loss_ms.unwrap_or(corner.delta_ms).max(0))
            .sum()
    }

    /// Everything braking cost this lap, in milliseconds.
    pub fn cost(&self) -> i32 {
        self.corners
            .iter()
            .filter(|corner| corner.fault.is_some())
            .map(|corner| corner.entry_loss_ms.unwrap_or(corner.delta_ms).max(0))
            .sum()
    }

    /// The findings, worst first.
    ///
    /// One per fault rather than one per corner: six corners braked softly is
    /// one habit and one thing to work on, and printing it six times would bury
    /// the second habit under it.
    pub fn advice(&self) -> Vec<Recommendation> {
        let mut found: Vec<Recommendation> = [
            Fault::Soft,
            Fault::Early,
            Fault::Released,
            Fault::Trailed,
            Fault::OverSlowed,
        ]
        .into_iter()
        .filter_map(|fault| self.finding(fault))
        .collect();
        // Negated rather than reversed, so two habits costing the same keep the
        // order they are checked in, which is the order of how much acting on
        // one explains the other.
        found.sort_by_key(|advice| -(cost_in(advice) as i64));
        found
    }

    /// One fault turned into one finding, or `None` when it did not cost
    /// enough to be worth an evening.
    fn finding(&self, fault: Fault) -> Option<Recommendation> {
        let corners = self.showing(fault);
        if corners.is_empty() {
            return None;
        }
        let cost_ms = self.cost_of(fault);
        if cost_ms < WORTH_SAYING_MS {
            return None;
        }

        // The magnitude of the fault in each corner, so the spread says whether
        // this is a habit or one bad corner dominating an average.
        let mut evidence = Evidence::new();
        for corner in &corners {
            evidence.observe(match fault {
                Fault::Soft => corner.decel_delta_g.unwrap_or(0.0).abs(),
                Fault::Early => corner.brake_delta_m.unwrap_or(0.0).abs(),
                Fault::Released | Fault::Trailed => {
                    corner.trail_delta_ms.unwrap_or(0) as f32 / 1000.0
                }
                Fault::OverSlowed => corner.min_speed_delta.unwrap_or(0.0).abs(),
            });
        }
        let typical = evidence.mean();

        let where_it_is = corners
            .iter()
            .take(4)
            .map(|corner| format!("T{}", corner.number))
            .collect::<Vec<_>>()
            .join(", ");
        let cost = format!("{:.2} s", cost_ms as f32 / 1000.0);
        let effect = format!("{cost} between entry and apex across {where_it_is}");

        let (message, action, cause, confirm) = match fault {
            Fault::Soft => {
                // Both numbers, not their difference: a driver can aim at 1.4 g
                // and cannot aim at "0.3 more".
                let reached = mean_of(&corners, |corner| corner.peak_decel_g);
                let theirs = mean_of(&corners, |corner| corner.reference_decel_g);
                (
                    "You are not asking the car for all the braking it has.".to_string(),
                    match (reached, theirs) {
                        (Some(mine), Some(reference)) => format!(
                            "Press harder in the first moment of the pedal. You reach {mine:.2} g \
                             where the reference reaches {reference:.2} g."
                        ),
                        _ => format!(
                            "Press harder in the first moment of the pedal — about {typical:.2} g \
                             more than you do now."
                        ),
                    },
                    format!("initial brake pressure {typical:.2} g below what the car will take"),
                    format!("peak deceleration in {where_it_is} next run"),
                )
            }
            Fault::Early => (
                "You brake earlier than the reference and give the distance away.".to_string(),
                format!(
                    "Move the brake point about {typical:.0} m later in {where_it_is}. Do it one \
                     corner at a time, not all of them at once."
                ),
                format!("brake point {typical:.0} m conservative"),
                "the marker you brake at in those corners".to_string(),
            ),
            Fault::Released => (
                "You come off the brake in one movement before the apex.".to_string(),
                format!(
                    "Bleed the pedal off toward the apex instead of lifting off it. The reference \
                     is still braking {:.0} ms after you have finished.",
                    typical * 1000.0
                ),
                "no trail, so the front unloads before the car has finished turning".to_string(),
                "how long the pedal is held after peak pressure".to_string(),
            ),
            Fault::Trailed => (
                "You stay on the brake well past the point the reference is off it.".to_string(),
                format!(
                    "Be done with the brake sooner — {:.0} ms of trail beyond the reference is \
                     that long with the front tyre asked to slow and steer at once.",
                    typical * 1000.0
                ),
                format!("trail {:.0} ms longer than the reference", typical * 1000.0),
                "where the pedal reaches zero, against the apex".to_string(),
            ),
            Fault::OverSlowed => (
                "Your braking matches the reference and your minimum speed does not.".to_string(),
                format!(
                    "The car had more mid-corner grip than you used. Carry about {typical:.0} km/h \
                     more through {where_it_is}."
                ),
                format!("minimum speed {typical:.0} km/h below the reference on matched braking"),
                format!("minimum speed in {where_it_is}"),
            ),
        };

        Some(Recommendation {
            component: "BRAKING".to_string(),
            category: "Driving".to_string(),
            // A habit is never Critical. Critical is for a car that is about to
            // stop being a car; this is for an evening's practice.
            severity: if cost_ms >= SERIOUS_MS {
                Severity::Warning
            } else {
                Severity::Info
            },
            message,
            action,
            // Nothing to turn on the car. Braking is the driver, and offering a
            // setup value here would be the program inventing one.
            parameters: Vec::new(),
            confidence: 0.0,
            chain: Some(Chain {
                cause,
                effect,
                confirm,
                evidence,
            }),
        })
    }
}

/// What a finding said it cost, recovered from its own sentence.
///
/// The cost is already in `Chain::effect` and putting it in a field as well
/// would be two places to keep agreeing. Findings are at most five.
fn cost_in(advice: &Recommendation) -> i32 {
    advice
        .chain
        .as_ref()
        .and_then(|chain| chain.effect.split(' ').next())
        .and_then(|seconds| seconds.parse::<f32>().ok())
        .map(|seconds| (seconds * 1000.0) as i32)
        .unwrap_or(0)
}

/// The mean of one field over the corners that have it. `None` when none do.
fn mean_of(corners: &[&AtCorner], field: impl Fn(&AtCorner) -> Option<f32>) -> Option<f32> {
    let values: Vec<f32> = corners.iter().filter_map(|corner| field(corner)).collect();
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f32>() / values.len() as f32)
}

/// Judge every braking zone of a lap against the reference it was decomposed
/// against.
///
/// The traces are wanted for one thing only — [`crate::corners::CornerComparison::inside`],
/// which is what separates time lost braking from time lost everywhere else.
/// Without it a corner that cost 0.3 s on the exit would be charged to the
/// brakes, and the driver would be sent to fix something that was not broken.
pub fn look(
    decomposition: &Decomposition,
    lap: &[TelemetryPoint],
    reference: &[TelemetryPoint],
    track_length_m: f32,
) -> Report {
    let mut corners = Vec::with_capacity(decomposition.sections.len());

    for (index, section) in decomposition.sections.iter().enumerate() {
        // Each corner owns the track to the next corner's entry, the same way
        // `corners::decompose` charges it. Recomputed rather than stored, so
        // there is one definition of a section and not two that can drift.
        let section_end = decomposition
            .sections
            .get(index + 1)
            .map(|next| next.corner.entry)
            .unwrap_or(f32::MAX);

        let entry_loss_ms = section
            .inside(lap, reference, section_end)
            .map(|inside| inside.braking_ms);

        let mut at = AtCorner {
            number: section.corner.number,
            delta_ms: section.delta_ms,
            entry_loss_ms,
            start: section.corner.braking.map(|braking| braking.start),
            reference_start: section
                .reference
                .as_ref()
                .and_then(|corner| corner.braking)
                .map(|braking| braking.start),
            brake_delta_m: section.braking_delta_m(track_length_m),
            decel_delta_g: section.peak_decel_delta_g(),
            trail_delta_ms: section.trail_delta_ms(),
            min_speed_delta: section.speed_deltas().map(|(_, minimum, _)| minimum),
            peak_decel_g: section.corner.braking.map(|braking| braking.peak_decel_g),
            reference_decel_g: section
                .reference
                .as_ref()
                .and_then(|corner| corner.braking)
                .map(|braking| braking.peak_decel_g),
            fault: None,
        };
        at.fault = fault_in(&at);
        corners.push(at);
    }

    Report { corners }
}

/// The one thing wrong with one corner.
///
/// The order is the point. A driver who brakes early *and* softly is told to
/// brake harder, because pressure is what they can change on the next lap and
/// the brake point then moves on its own. Told to brake later while still
/// pressing gently, they arrive at the corner too fast with no way to fix it —
/// which is how a program that means well makes somebody crash.
fn fault_in(corner: &AtCorner) -> Option<Fault> {
    // Nothing was lost *braking*, so there is nothing here to explain.
    //
    // **Judged on the entry-to-apex part and not on the whole section**, where
    // the split is available. A corner that cost 0.17 s before the apex and
    // won it back on the exit is still 0.17 s lying on the table, and gating on
    // the section's own delta would hide every one of them — which is what this
    // did until a demo lap with visibly soft braking reported nothing at all.
    //
    // The section's delta is the fallback and not the rule: without the traces
    // there is no split to read, and then all there is to go on is whether the
    // corner cost anything.
    match corner.entry_loss_ms {
        Some(entry_loss) if entry_loss <= COSTLY_MS => return None,
        // A corner driven differently and *faster* overall is the driver being
        // right, and with no split that is all that can be said.
        None if corner.delta_ms <= COSTLY_MS => return None,
        _ => {}
    }

    if corner.decel_delta_g.is_some_and(|delta| delta < -NOISE_G) {
        return Some(Fault::Soft);
    }
    if corner.brake_delta_m.is_some_and(|delta| delta < -NOISE_M) {
        return Some(Fault::Early);
    }
    if corner.trail_delta_ms.is_some_and(|delta| delta < -NOISE_MS) {
        return Some(Fault::Released);
    }
    if corner.trail_delta_ms.is_some_and(|delta| delta > NOISE_MS) {
        return Some(Fault::Trailed);
    }
    if corner
        .min_speed_delta
        .is_some_and(|delta| delta < -NOISE_KMH)
    {
        return Some(Fault::OverSlowed);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corners::{Braking, Corner, CornerComparison, Direction};

    /// A corner as it would come out of `detect`, with the braking numbers the
    /// test cares about and plausible everything else.
    fn corner(number: usize, brake_at: f32, decel: f32, trail_ms: i32, min_speed: f32) -> Corner {
        Corner {
            number,
            direction: Direction::Left,
            entry: brake_at,
            apex: brake_at + 0.02,
            exit: brake_at + 0.04,
            entry_speed: 240.0,
            min_speed,
            exit_speed: 190.0,
            peak_lat_g: 1.5,
            brake_point: Some(brake_at),
            braking: Some(Braking {
                start: brake_at,
                release: brake_at + 0.015,
                duration_ms: 1_200,
                peak_decel_g: decel,
                peak_pressure: 0.9,
                release_ms: trail_ms,
                entry_speed: 240.0,
                release_speed: min_speed,
            }),
            throttle_point: Some(brake_at + 0.025),
            throttle_delay_ms: Some(300),
            entry_time_ms: (brake_at * 90_000.0) as i32,
            exit_time_ms: (brake_at * 90_000.0) as i32 + 2_000,
        }
    }

    fn section(mine: Corner, theirs: Corner, delta_ms: i32) -> CornerComparison {
        CornerComparison {
            corner: mine,
            reference: Some(theirs),
            delta_ms,
        }
    }

    fn lap_of(sections: Vec<CornerComparison>) -> Decomposition {
        Decomposition {
            total_ms: sections.iter().map(|section| section.delta_ms).sum(),
            sections,
            opening_ms: 0,
        }
    }

    /// The track these fixtures are on, so a metre is a metre.
    const TRACK_M: f32 = 5_000.0;

    /// A lap driven exactly like the reference has nothing to say about it.
    /// This is the test that stops the whole module being a machine for
    /// generating advice out of noise.
    #[test]
    fn a_lap_matching_the_reference_produces_no_advice() {
        let lap = lap_of(
            (1..=8)
                .map(|number| {
                    let at = number as f32 / 10.0;
                    section(
                        corner(number, at, 1.40, 400, 95.0),
                        corner(number, at, 1.40, 400, 95.0),
                        0,
                    )
                })
                .collect(),
        );

        let report = look(&lap, &[], &[], TRACK_M);
        assert!(report.corners.iter().all(|corner| corner.fault.is_none()));
        assert!(report.advice().is_empty());
    }

    /// Six corners braked gently is one habit, one finding, and confident —
    /// not six findings, and not a shrug.
    #[test]
    fn braking_softly_everywhere_is_one_confident_finding() {
        let lap = lap_of(
            (1..=6)
                .map(|number| {
                    let at = number as f32 / 10.0;
                    section(
                        corner(number, at, 1.10, 400, 95.0),
                        corner(number, at, 1.42, 400, 95.0),
                        90,
                    )
                })
                .collect(),
        );

        let report = look(&lap, &[], &[], TRACK_M);
        assert_eq!(report.showing(Fault::Soft).len(), 6);

        let advice = report.advice();
        assert_eq!(advice.len(), 1, "one habit is one finding, not six");
        assert_eq!(advice[0].component, "BRAKING");
        assert_eq!(
            advice[0].confidence_level(),
            crate::confidence::Confidence::High,
            "six corners agreeing is not a guess"
        );
        // Both numbers, so the driver has something to aim at.
        assert!(advice[0].action.contains("1.10 g"), "{}", advice[0].action);
        assert!(advice[0].action.contains("1.42 g"), "{}", advice[0].action);
        // A driving habit is never a component about to fail.
        assert!(advice[0].severity != Severity::Critical);
        assert!(advice[0].parameters.is_empty(), "there is nothing to turn");
    }

    /// Braking early *and* softly is told as pressure, because pressure is
    /// what moves the brake point. The other way round sends somebody into a
    /// corner too fast with no way out of it.
    #[test]
    fn pressure_is_reported_before_the_brake_point_it_explains() {
        let early_and_soft = corner(1, 0.20, 1.05, 400, 95.0);
        let theirs = corner(1, 0.204, 1.40, 400, 95.0);
        let lap = lap_of(vec![section(early_and_soft, theirs, 200)]);

        let report = look(&lap, &[], &[], TRACK_M);
        assert_eq!(report.corners[0].fault, Some(Fault::Soft));
        // And the brake point really was early, so this is a choice between
        // two true things rather than only one of them being detectable.
        assert!(
            report.corners[0]
                .brake_delta_m
                .expect("both laps braked here")
                < -NOISE_M
        );
    }

    /// A corner driven differently and *faster* is the driver being right.
    #[test]
    fn a_corner_that_gained_time_is_not_a_fault() {
        let lap = lap_of(vec![section(
            corner(1, 0.20, 1.05, 400, 95.0),
            corner(1, 0.24, 1.40, 400, 95.0),
            -150,
        )]);

        let report = look(&lap, &[], &[], TRACK_M);
        assert_eq!(report.corners[0].fault, None);
    }

    /// One corner braked softly costs 0.09 s and is not worth an evening.
    #[test]
    fn a_difference_too_small_to_act_on_is_not_reported() {
        let lap = lap_of(vec![section(
            corner(1, 0.20, 1.20, 400, 95.0),
            corner(1, 0.20, 1.40, 400, 95.0),
            90,
        )]);

        let report = look(&lap, &[], &[], TRACK_M);
        assert_eq!(report.corners[0].fault, Some(Fault::Soft));
        assert!(
            report.advice().is_empty(),
            "under {WORTH_SAYING_MS} ms there is a difference and no instruction"
        );
    }

    /// Time lost on the exit is not the brakes, and charging it to them sends
    /// a driver to fix something that was not broken.
    #[test]
    fn time_lost_after_the_apex_is_not_charged_to_braking() {
        // Two traces identical to the apex, then the reference pulls away.
        let trace = |slow_after: f32| -> Vec<TelemetryPoint> {
            (0..=200)
                .map(|step| {
                    let at = step as f32 / 200.0;
                    let late = if at > 0.22 { slow_after } else { 0.0 };
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
        let mine = trace(400.0);
        let theirs = trace(0.0);

        // The braking itself was softer, and still the time went afterwards.
        let lap = lap_of(vec![section(
            corner(1, 0.20, 1.10, 400, 95.0),
            corner(1, 0.20, 1.40, 400, 95.0),
            400,
        )]);

        let report = look(&lap, &mine, &theirs, TRACK_M);
        assert!(
            report.corners[0]
                .entry_loss_ms
                .expect("both traces reach the apex")
                <= COSTLY_MS,
            "entry to apex was matched: {:?}",
            report.corners[0].entry_loss_ms
        );
        assert_eq!(
            report.corners[0].fault, None,
            "the exit cost the time and the brakes are being blamed for it"
        );
    }

    /// Time lost before the apex and won back after it is still time lost
    /// before the apex. Gating on the section's own delta hid every one of
    /// these, which is how a demo lap with visibly soft braking reported
    /// nothing at all.
    #[test]
    fn braking_that_cost_time_counts_even_when_the_exit_won_it_back() {
        // Behind by 0.17 s at the apex, level again by the time the section
        // ends — so the section's own delta is nothing and the brakes still
        // cost a sixth of a second.
        let trace = |late_to_apex: f32| -> Vec<TelemetryPoint> {
            (0..=200)
                .map(|step| {
                    let at = step as f32 / 200.0;
                    let late = if (0.205..0.30).contains(&at) {
                        late_to_apex
                    } else {
                        0.0
                    };
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

        let lap = lap_of(vec![section(
            corner(1, 0.20, 1.05, 400, 95.0),
            corner(1, 0.20, 1.40, 400, 95.0),
            0,
        )]);
        let report = look(&lap, &trace(170.0), &trace(0.0), TRACK_M);

        assert!(
            report.corners[0]
                .entry_loss_ms
                .expect("both traces reach the apex")
                > COSTLY_MS
        );
        assert_eq!(report.corners[0].fault, Some(Fault::Soft));
    }

    /// A corner taken flat in either lap has no braking to compare, and that
    /// is not a difference of zero.
    #[test]
    fn a_corner_taken_flat_is_no_answer_rather_than_a_match() {
        let mut flat = corner(1, 0.20, 1.40, 400, 95.0);
        flat.brake_point = None;
        flat.braking = None;

        let lap = lap_of(vec![section(flat, corner(1, 0.20, 1.40, 400, 95.0), 200)]);
        let report = look(&lap, &[], &[], TRACK_M);

        assert_eq!(report.corners[0].decel_delta_g, None);
        assert_eq!(report.corners[0].brake_delta_m, None);
        assert_eq!(report.corners[0].start, None);
    }

    /// Coming off the pedal in one movement and trailing far too long are
    /// opposite faults and must not collapse into each other.
    #[test]
    fn the_two_trail_faults_point_opposite_ways() {
        let released = lap_of(vec![section(
            corner(1, 0.20, 1.40, 120, 95.0),
            corner(1, 0.20, 1.40, 500, 95.0),
            200,
        )]);
        assert_eq!(
            look(&released, &[], &[], TRACK_M).corners[0].fault,
            Some(Fault::Released)
        );

        let trailed = lap_of(vec![section(
            corner(1, 0.20, 1.40, 900, 95.0),
            corner(1, 0.20, 1.40, 500, 95.0),
            200,
        )]);
        assert_eq!(
            look(&trailed, &[], &[], TRACK_M).corners[0].fault,
            Some(Fault::Trailed)
        );
    }

    /// Matched braking and a slower middle is the driver, not the pedal.
    #[test]
    fn matched_braking_and_a_slow_middle_is_its_own_finding() {
        let lap = lap_of(
            (1..=5)
                .map(|number| {
                    let at = number as f32 / 10.0;
                    section(
                        corner(number, at, 1.40, 400, 88.0),
                        corner(number, at, 1.40, 400, 95.0),
                        120,
                    )
                })
                .collect(),
        );

        let report = look(&lap, &[], &[], TRACK_M);
        assert_eq!(report.showing(Fault::OverSlowed).len(), 5);
        let advice = report.advice();
        assert_eq!(advice.len(), 1);
        assert!(advice[0].action.contains("km/h"), "{}", advice[0].action);
    }

    /// Two habits in one lap are ordered by what they cost, so the first line
    /// a driver reads is the one worth the most.
    #[test]
    fn findings_are_ordered_by_what_they_cost() {
        let mut sections = Vec::new();
        // A cheap soft-braking habit.
        for number in 1..=2 {
            let at = number as f32 / 20.0;
            sections.push(section(
                corner(number, at, 1.20, 400, 95.0),
                corner(number, at, 1.40, 400, 95.0),
                80,
            ));
        }
        // An expensive early-braking one.
        for number in 3..=8 {
            let at = number as f32 / 20.0;
            sections.push(section(
                corner(number, at - 0.01, 1.40, 400, 95.0),
                corner(number, at, 1.40, 400, 95.0),
                200,
            ));
        }

        let advice = look(&lap_of(sections), &[], &[], TRACK_M).advice();
        assert_eq!(advice.len(), 2);
        assert!(
            advice[0].action.contains("brake point"),
            "the expensive habit comes first: {}",
            advice[0].action
        );
    }
}
