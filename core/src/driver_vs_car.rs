//! "You drove that badly" against "the car will not go faster".
//!
//! The most valuable thing in `docs/plan-0.3.7-analysis.md` and the hardest,
//! and the reason is worth stating before any of the code: **one lap cannot
//! tell them apart.** A car that understeers and a driver who turns in too
//! early produce the same trace. There is no cleverness that separates them
//! from a single lap, and every tool that claims to is guessing.
//!
//! The discriminator is whether the behaviour persists *across different
//! inputs*. A car that understeers does it on the lap the driver got right and
//! on the lap they got wrong; a driver's mistake follows the driving. So this
//! reasons over a stint, and it refuses to answer before it has one.
//!
//! **That refusal is the feature, not a limitation to work around.** An
//! engineer who answers a question they cannot answer is worse than one who
//! says "give me four more laps" — the wrong answer here sends somebody to
//! change a setup that was never the problem.

use crate::analyzer::LapData;
use crate::confidence::{Confidence, Evidence};
use crate::i18n::Translate;

/// Laps needed before this will say anything at all.
///
/// Four: enough that a symptom appearing in all of them is not three
/// coincidences, and few enough to be one run out of the pits.
pub const MIN_LAPS: usize = 4;

/// How many times a lap a symptom has to show up before it is a symptom.
///
/// Two of anything is a car being driven near the limit, which is where it is
/// supposed to be.
const NOISE_FLOOR: f32 = 2.0;

/// Whose problem it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blame {
    /// It happens whatever the driver does, so it is the car.
    Car,
    /// It follows the driving, so it is the driver.
    Driver,
    /// It could be either, and saying which would be a guess.
    Undecided,
}

impl Blame {
    pub fn label(self, russian: bool) -> &'static str {
        match self {
            Blame::Car => "the car",
            Blame::Driver => "the driving",
            Blame::Undecided => "could be either",
        }
        .tr(russian)
    }
}

/// One symptom, and whose it is.
#[derive(Debug, Clone)]
pub struct Verdict {
    /// What was seen: "understeer", "lockups".
    pub symptom: String,
    pub blame: Blame,
    pub confidence: Confidence,
    /// Why it was decided that way, in the terms it was decided on. Shown, not
    /// hidden behind the verdict: this is a judgement made from four laps, and
    /// a driver is entitled to see the reasoning and disagree with it.
    pub reason: String,
    /// Mean occurrences per lap over the stint.
    pub per_lap: f32,
    pub laps: usize,
}

/// Nothing can be said yet, and why.
///
/// Returned rather than an empty list, so a caller draws "four more laps"
/// instead of a blank panel that reads as the analysis being broken.
#[derive(Debug, Clone)]
pub struct NotYet {
    pub laps: usize,
    pub needed: usize,
}

/// What a stint says about the car and the driving.
#[derive(Debug, Clone)]
pub enum Assessment {
    /// Not enough laps to separate anything.
    NotYet(NotYet),
    /// Enough laps. May still be empty, which means nothing was wrong often
    /// enough to be worth attributing.
    Verdicts(Vec<Verdict>),
}

impl Assessment {
    pub fn verdicts(&self) -> &[Verdict] {
        match self {
            Assessment::Verdicts(verdicts) => verdicts,
            Assessment::NotYet(_) => &[],
        }
    }
}

/// What the driver did differently from lap to lap, as evidence.
///
/// The reference the symptoms are judged against: if the lap times moved about
/// and the symptom did not, the symptom is not following the driving.
fn pace_varied(laps: &[LapData]) -> bool {
    let evidence = Evidence::from_values(
        laps.iter()
            .filter(|lap| lap.valid && lap.lap_time_ms > 0)
            .map(|lap| lap.lap_time_ms as f32),
    );
    // A tenth of a percent of a ninety-second lap is a tenth of a second, and
    // laps within a tenth of each other are the same lap driven again.
    evidence.count() >= 2 && evidence.spread() / evidence.mean().abs().max(1.0) > 0.001
}

/// One thing worth counting per lap: what to call it, and how to count it.
type Symptom = (&'static str, fn(&LapData) -> f32);

/// Assess a stint — the laps driven on one set of tyres and one setup.
pub fn assess(laps: &[LapData]) -> Assessment {
    if laps.len() < MIN_LAPS {
        return Assessment::NotYet(NotYet {
            laps: laps.len(),
            needed: MIN_LAPS,
        });
    }

    let varied = pace_varied(laps);
    let mut verdicts = Vec::new();

    let symptoms: [Symptom; 4] = [
        ("understeer", |lap| lap.understeer_count as f32),
        ("oversteer", |lap| lap.oversteer_count as f32),
        ("lockups", |lap| lap.lockup_count as f32),
        ("over-rotation", |lap| lap.scrubbing_incidents as f32),
    ];

    for (name, get) in symptoms {
        let evidence = Evidence::from_values(laps.iter().map(get));
        let per_lap = evidence.mean();
        if per_lap < NOISE_FLOOR {
            continue;
        }

        // A symptom that turns up in the same quantity every lap did not
        // follow the driving. One that comes and goes did.
        let consistent = evidence.confidence();
        let (blame, reason) = match (consistent, varied) {
            (Confidence::High, true) => (
                Blame::Car,
                format!(
                    "{per_lap:.0} a lap on every one of {} laps, while the lap times moved about — \
                     it is not following the driving",
                    laps.len()
                ),
            ),
            (Confidence::High, false) => (
                Blame::Undecided,
                format!(
                    "{per_lap:.0} a lap on every one of {} laps, but every lap was driven the same \
                     way — there is nothing to tell the car from the habit",
                    laps.len()
                ),
            ),
            (_, _) => (
                Blame::Driver,
                format!(
                    "between {:.0} and {:.0} a lap over {} laps — it comes and goes with the lap",
                    evidence.min(),
                    evidence.max(),
                    laps.len()
                ),
            ),
        };

        verdicts.push(Verdict {
            symptom: name.to_string(),
            blame,
            // Never better than the evidence behind it, and never better than
            // Medium for anything blamed on the driver: "you are inconsistent"
            // from four laps is a hypothesis.
            confidence: match blame {
                Blame::Car => consistent,
                Blame::Driver => consistent.min(Confidence::Medium),
                Blame::Undecided => Confidence::Low,
            },
            reason,
            per_lap,
            laps: laps.len(),
        });
    }

    // Worst first, by how often it happens.
    verdicts.sort_by(|a, b| {
        b.per_lap
            .partial_cmp(&a.per_lap)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Assessment::Verdicts(verdicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lap(time_ms: i32, understeer: i32, lockups: i32) -> LapData {
        LapData {
            lap_time_ms: time_ms,
            valid: true,
            understeer_count: understeer,
            lockup_count: lockups,
            ..Default::default()
        }
    }

    /// The refusal is the feature. Three laps is not a stint.
    #[test]
    fn it_will_not_answer_from_too_few_laps() {
        let laps = vec![lap(91_000, 9, 0), lap(91_200, 9, 0), lap(91_100, 9, 0)];
        let assessment = assess(&laps);
        let Assessment::NotYet(not_yet) = &assessment else {
            unreachable!("three laps is not a stint: {assessment:?}")
        };
        assert_eq!(not_yet.laps, 3);
        assert_eq!(not_yet.needed, MIN_LAPS);
    }

    /// Understeer in the same quantity on every lap, while the lap times moved
    /// about, did not follow the driving.
    #[test]
    fn a_symptom_that_ignores_the_driving_is_the_car() {
        let laps = vec![
            lap(91_000, 9, 0),
            lap(92_400, 9, 0),
            lap(90_600, 10, 0),
            lap(93_100, 9, 0),
            lap(91_800, 10, 0),
        ];

        let verdicts = assess(&laps);
        let understeer = verdicts
            .verdicts()
            .iter()
            .find(|verdict| verdict.symptom == "understeer")
            .expect("nine a lap is well over the noise floor");

        assert_eq!(understeer.blame, Blame::Car, "{}", understeer.reason);
        assert_eq!(understeer.confidence, Confidence::High);
    }

    /// The same symptom coming and going is the driver.
    #[test]
    fn a_symptom_that_follows_the_lap_is_the_driver() {
        let laps = vec![
            lap(91_000, 1, 0),
            lap(92_400, 14, 0),
            lap(90_600, 0, 0),
            lap(93_100, 12, 0),
            lap(91_800, 2, 0),
        ];

        let verdicts = assess(&laps);
        let understeer = verdicts
            .verdicts()
            .iter()
            .find(|verdict| verdict.symptom == "understeer")
            .expect("nearly six a lap on average");

        assert_eq!(understeer.blame, Blame::Driver, "{}", understeer.reason);
        assert!(
            understeer.confidence <= Confidence::Medium,
            "blaming the driver from five laps is a hypothesis"
        );
    }

    /// Five identical laps with the same symptom every time cannot separate a
    /// car that understeers from a driver with one habit. Saying so is the
    /// right answer.
    #[test]
    fn identical_laps_cannot_separate_the_car_from_the_habit() {
        let laps = vec![
            lap(91_000, 9, 0),
            lap(91_000, 9, 0),
            lap(91_000, 9, 0),
            lap(91_000, 9, 0),
        ];

        let verdicts = assess(&laps);
        let understeer = verdicts
            .verdicts()
            .iter()
            .find(|verdict| verdict.symptom == "understeer")
            .expect("nine a lap");

        assert_eq!(understeer.blame, Blame::Undecided, "{}", understeer.reason);
        assert_eq!(understeer.confidence, Confidence::Low);
    }

    /// A car being driven near the limit locks a wheel now and then. That is
    /// not a symptom and it must not produce a verdict.
    #[test]
    fn the_ordinary_business_of_driving_is_not_a_symptom() {
        let laps = vec![
            lap(91_000, 0, 1),
            lap(92_400, 0, 2),
            lap(90_600, 0, 1),
            lap(93_100, 0, 0),
        ];

        assert!(
            assess(&laps).verdicts().is_empty(),
            "{:?}",
            assess(&laps).verdicts()
        );
    }

    #[test]
    fn the_worst_symptom_is_first() {
        let laps = vec![
            lap(91_000, 4, 12),
            lap(92_400, 5, 11),
            lap(90_600, 4, 13),
            lap(93_100, 5, 12),
        ];

        let assessment = assess(&laps);
        let verdicts = assessment.verdicts();
        assert!(verdicts.len() >= 2, "{verdicts:?}");
        assert_eq!(verdicts[0].symptom, "lockups");
    }
}
