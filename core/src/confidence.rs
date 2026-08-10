//! How sure the engineer is, and being willing to say "not yet".
//!
//! An engineer who says the same thing about one strange frame and about four
//! consistent corners is not an engineer. Every verdict this project has got
//! wrong got it wrong the same way: it judged on too little and said so with
//! the same certainty as everything else. Four tyres reported WORN OUT from a
//! session that published no wear at all. A camber verdict from a single
//! cornering frame. A lap summary about a lap with no temperatures in it.
//!
//! So confidence is not decoration on the end of a sentence. It is the part
//! that makes the rest of the analysis honest, and it is a **count of
//! corroborating observations and their spread** rather than a number somebody
//! picked while writing the rule.
//!
//! ```text
//! 🟢 High     front tyres 8–11 °C above target across four corners
//! 🟡 Medium   possible rear instability under throttle
//! 🔴 Low      not enough data — one representative corner
//! ```

use serde::{Deserialize, Serialize};

/// Fewer observations than this and there is nothing to be confident about,
/// however cleanly they agree. Two readings agreeing is a coincidence.
const MIN_FOR_MEDIUM: usize = 3;

/// Enough agreeing observations to be sure, given they also agree closely.
const MIN_FOR_HIGH: usize = 4;

/// How much the observations may disagree and still be `High`, as a fraction of
/// their own mean.
///
/// A quarter: four corners reporting 8, 9, 10 and 11 °C over target are the
/// same finding seen four times. The same four reporting 2, 9, 3 and 15 are
/// not, and averaging them into "7 °C over" is the kind of confident nonsense
/// this module exists to prevent.
const HIGH_SPREAD: f32 = 0.25;

/// ...and the looser bound for `Medium`.
const MEDIUM_SPREAD: f32 = 0.50;

/// How sure a piece of advice is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    /// Not enough to say. Shown, rather than hidden: "I do not know yet" is an
    /// answer, and it is the one that stops a driver changing a setup because
    /// of a single odd frame.
    Low,
    Medium,
    High,
}

impl Confidence {
    /// The marker a driver reads before the sentence.
    /// The marker a driver reads before the sentence.
    ///
    /// Geometric shapes rather than the coloured circles the plan sketches:
    /// 🟢🟡🔴 are recent enough that the terminal font renders all three as a
    /// dash, which turned every confidence marker in the screenshots into the
    /// same character. Filled, half and hollow carry the meaning without
    /// colour, which also keeps it legible where colour does not survive.
    pub fn marker(self) -> &'static str {
        match self {
            Confidence::High => "●",
            Confidence::Medium => "◐",
            Confidence::Low => "○",
        }
    }

    pub fn label(self, russian: bool) -> &'static str {
        match (self, russian) {
            (Confidence::High, false) => "High",
            (Confidence::Medium, false) => "Medium",
            (Confidence::Low, false) => "Low",
            (Confidence::High, true) => "Высокая",
            (Confidence::Medium, true) => "Средняя",
            (Confidence::Low, true) => "Низкая",
        }
    }

    /// Whether this is worth acting on, as opposed to worth knowing.
    ///
    /// `Low` advice is still shown — it is what says the analysis is watching
    /// and has not decided — but nothing should be presented as a change to
    /// make on the strength of it.
    pub fn is_actionable(self) -> bool {
        self != Confidence::Low
    }

    /// The tri-level for advice that carries only the old `confidence` score.
    ///
    /// Most of the engineer's rules were written with a number chosen by hand —
    /// `0.95` for something obvious, `0.7` for a guess. Those are not evidence
    /// and this does not pretend they are; it maps them onto the same scale so
    /// one display can show every rule, and the rules that can count their
    /// evidence use [`Evidence::confidence`] instead and mean it.
    pub fn from_score(score: f32) -> Self {
        if score >= 0.9 {
            Confidence::High
        } else if score >= 0.7 {
            Confidence::Medium
        } else {
            Confidence::Low
        }
    }

    /// Back to a score, for the places that still sort by one.
    pub fn score(self) -> f32 {
        match self {
            Confidence::High => 0.95,
            Confidence::Medium => 0.75,
            Confidence::Low => 0.4,
        }
    }
}

/// How many underlying samples make an observation a settled one rather than a
/// reading.
const WELL_AVERAGED: u32 = 30;

/// What was actually seen, and how much it agreed with itself.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Evidence {
    /// Every observation of the thing being judged — one per corner, per lap,
    /// per wheel, whatever the rule is counting.
    values: Vec<f32>,
    /// How many samples each of those observations was itself averaged from.
    ///
    /// One means each is a single reading. The camber verdict is two wheels,
    /// each averaged over sixty frames of cornering load, and that is a
    /// different thing from two frames — counting them the same would either
    /// call a settled two-wheel finding a coincidence, or call two noisy
    /// samples a finding. See [`Evidence::averaged_over`].
    #[serde(default)]
    samples_each: u32,
}

impl Evidence {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one observation.
    pub fn observe(&mut self, value: f32) {
        self.values.push(value);
    }

    /// Record one observation, fluently.
    pub fn with(mut self, value: f32) -> Self {
        self.observe(value);
        self
    }

    pub fn from_values(values: impl IntoIterator<Item = f32>) -> Self {
        Self {
            values: values.into_iter().collect(),
            samples_each: 1,
        }
    }

    /// Say that each observation is itself the mean of `samples` readings.
    ///
    /// An observation settled over a second of telemetry is worth more than one
    /// frame, and several of the engineer's rules work that way — the camber
    /// verdict averages sixty cornering frames per wheel before it says
    /// anything. A well-averaged observation counts double, so two settled
    /// wheels can reach `High` where two single frames cannot reach `Medium`.
    pub fn averaged_over(mut self, samples: u32) -> Self {
        self.samples_each = samples;
        self
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }

    /// The count that decides confidence, after weighting.
    fn effective_count(&self) -> usize {
        if self.samples_each >= WELL_AVERAGED {
            self.values.len() * 2
        } else {
            self.values.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The average observation, which is the number the advice quotes.
    pub fn mean(&self) -> f32 {
        if self.values.is_empty() {
            return 0.0;
        }
        self.values.iter().sum::<f32>() / self.values.len() as f32
    }

    pub fn min(&self) -> f32 {
        self.values.iter().copied().fold(f32::MAX, f32::min)
    }

    pub fn max(&self) -> f32 {
        self.values.iter().copied().fold(f32::MIN, f32::max)
    }

    /// Population standard deviation of the observations.
    pub fn spread(&self) -> f32 {
        if self.values.len() < 2 {
            return 0.0;
        }
        let mean = self.mean();
        let variance = self
            .values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f32>()
            / self.values.len() as f32;
        variance.sqrt()
    }

    /// The verdict: enough observations, agreeing closely enough.
    pub fn confidence(&self) -> Confidence {
        // One observation is never confident however it was arrived at. A
        // single wheel averaged over a whole stint is still one wheel, and
        // weighting must not turn it into a corroborated finding.
        if self.values.len() < 2 {
            return Confidence::Low;
        }
        let count = self.effective_count();
        if count < MIN_FOR_MEDIUM {
            return Confidence::Low;
        }

        let mean = self.mean().abs();
        // Observations that cancel out — half of them one way and half the
        // other — average to nothing, and a rule reading only the mean would
        // call that a perfectly balanced car. It is the least confident thing
        // there is. Nothing observed at all lands here too, and is equally Low.
        if mean <= f32::EPSILON {
            return Confidence::Low;
        }

        let relative = self.spread() / mean;
        if count >= MIN_FOR_HIGH && relative <= HIGH_SPREAD {
            Confidence::High
        } else if relative <= MEDIUM_SPREAD {
            Confidence::Medium
        } else {
            Confidence::Low
        }
    }

    /// The range, for a sentence that wants to say "8–11 °C" rather than "9.4".
    ///
    /// `None` when there is nothing to describe a range of.
    pub fn range(&self) -> Option<(f32, f32)> {
        if self.values.is_empty() {
            return None;
        }
        Some((self.min(), self.max()))
    }

    /// What the advice says about its own evidence: "across four corners",
    /// "from one sample".
    pub fn describe(&self, unit: &str, russian: bool) -> String {
        match (self.count(), russian) {
            (0, true) => "нет наблюдений".to_string(),
            (0, false) => "no observations".to_string(),
            (1, true) => format!("по одному {unit}"),
            (1, false) => format!("from one {unit}"),
            (n, true) => format!("по {n} {unit}"),
            (n, false) => format!("across {n} {unit}s"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this exists to prevent: one observation is never a verdict, no
    /// matter how extreme it is.
    #[test]
    fn one_observation_is_never_confident() {
        let evidence = Evidence::from_values([40.0]);
        assert_eq!(evidence.confidence(), Confidence::Low);
        assert!(!evidence.confidence().is_actionable());
    }

    #[test]
    fn two_agreeing_observations_are_still_a_coincidence() {
        let evidence = Evidence::from_values([10.0, 10.1]);
        assert_eq!(evidence.confidence(), Confidence::Low);
    }

    /// Four corners saying 8, 9, 10 and 11 °C over target are one finding seen
    /// four times.
    #[test]
    fn four_close_observations_are_high() {
        let evidence = Evidence::from_values([8.0, 9.0, 10.0, 11.0]);
        assert_eq!(evidence.confidence(), Confidence::High);
        assert!((evidence.mean() - 9.5).abs() < 0.01);
        assert_eq!(evidence.range(), Some((8.0, 11.0)));
    }

    /// The same four, disagreeing wildly, are not — and averaging them into a
    /// single confident number is exactly the failure mode this prevents.
    #[test]
    fn four_scattered_observations_are_not() {
        let evidence = Evidence::from_values([2.0, 9.0, 3.0, 15.0]);
        assert_eq!(evidence.confidence(), Confidence::Low, "{evidence:?}");
    }

    #[test]
    fn three_close_observations_are_medium() {
        let evidence = Evidence::from_values([10.0, 11.0, 12.0]);
        assert_eq!(evidence.confidence(), Confidence::Medium);
    }

    /// Observations that cancel out are the least confident thing there is.
    /// Two corners saying +10 and two saying −10 average to zero, and a rule
    /// reading only the mean would call that a perfectly balanced car.
    #[test]
    fn observations_that_cancel_out_are_not_agreement() {
        let evidence = Evidence::from_values([10.0, -10.0, 10.0, -10.0]);
        assert_eq!(evidence.confidence(), Confidence::Low);
        assert!(evidence.mean().abs() < 0.01);
    }

    #[test]
    fn nothing_observed_is_low_and_not_a_crash() {
        let evidence = Evidence::new();
        assert_eq!(evidence.confidence(), Confidence::Low);
        assert_eq!(evidence.mean(), 0.0);
        assert_eq!(evidence.range(), None);
        assert_eq!(evidence.spread(), 0.0);
    }

    /// Two wheels averaged over a second of cornering are a finding; two
    /// single frames are not. The camber verdict is the first kind, and
    /// counting it as the second was the reason it needed a frame gate bolted
    /// on beside it rather than expressed in the confidence itself.
    #[test]
    fn well_averaged_observations_count_for_more() {
        let raw = Evidence::from_values([9.0, 10.0]);
        assert_eq!(raw.confidence(), Confidence::Low);

        let settled = Evidence::from_values([9.0, 10.0]).averaged_over(60);
        assert_eq!(settled.confidence(), Confidence::High);
    }

    /// ...but one wheel is one wheel, however long it was watched.
    #[test]
    fn averaging_does_not_turn_one_observation_into_agreement() {
        let evidence = Evidence::from_values([40.0]).averaged_over(600);
        assert_eq!(evidence.confidence(), Confidence::Low);
    }

    #[test]
    fn the_legacy_scores_land_where_they_read() {
        assert_eq!(Confidence::from_score(0.95), Confidence::High);
        assert_eq!(Confidence::from_score(0.85), Confidence::Medium);
        assert_eq!(Confidence::from_score(0.7), Confidence::Medium);
        assert_eq!(Confidence::from_score(0.5), Confidence::Low);
    }

    #[test]
    fn confidence_orders_worst_to_best() {
        let mut levels = [Confidence::High, Confidence::Low, Confidence::Medium];
        levels.sort();
        assert_eq!(
            levels,
            [Confidence::Low, Confidence::Medium, Confidence::High]
        );
    }

    #[test]
    fn the_description_counts_what_it_saw() {
        assert_eq!(
            Evidence::from_values([1.0, 2.0, 3.0, 4.0]).describe("corner", false),
            "across 4 corners"
        );
        assert_eq!(
            Evidence::from_values([1.0]).describe("corner", false),
            "from one corner"
        );
        assert_eq!(
            Evidence::from_values([1.0, 2.0, 3.0]).describe("круг", true),
            "по 3 круг"
        );
    }
}
