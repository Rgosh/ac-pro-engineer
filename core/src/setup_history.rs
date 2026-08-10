//! Which setup was on the car, for which laps, and what changed when it changed.
//!
//! Both halves of this existed and never met. `setup_manager` reads the loaded
//! setup and can diff two of them; the analyser knows how the car behaved. What
//! was missing was the thing that ties them together — a record of *which laps
//! were driven on which setup* — without which a change can be made and never
//! attributed to anything.
//!
//! ```text
//! Setup change detected: rear ARB 4 → 3
//!   Exit oversteer  ↓ 18%
//!   T10 exit        +0.12 s
//!   Lap time        −0.21 s
//! ```
//!
//! ## The honesty problem, which is the whole difficulty
//!
//! On a real stint the tyres wear, the fuel burns off and the track rubbers in,
//! and all three move while the driver is also changing the rear ARB. A lap
//! that comes down two tenths after a setup change did not necessarily come
//! down *because* of it.
//!
//! So this **never claims a cause**. It reports what changed, what happened
//! afterwards, and — in the same breath, not in a footnote — everything else
//! that moved at the same time and would explain it just as well. A driver who
//! is told "0.2 s quicker, and the track came up 4 °C and you are 20 kg
//! lighter" can decide for themselves. A driver told "the ARB gained you 0.2 s"
//! has been misled by their own tooling.

use crate::analyzer::LapData;
use crate::confidence::{Confidence, Evidence};
use crate::setup_manager::{CarSetup, SetupDiffItem};

/// Laps on one setup before it was changed.
#[derive(Debug, Clone)]
pub struct Stint {
    pub setup: CarSetup,
    /// Every lap driven on it, in the order they were driven.
    pub laps: Vec<LapData>,
}

impl Stint {
    /// The quickest lap on this setup, in milliseconds. `None` when nothing
    /// valid was driven on it.
    pub fn best_lap_ms(&self) -> Option<i32> {
        self.laps
            .iter()
            .filter(|lap| lap.valid && lap.lap_time_ms > 0)
            .map(|lap| lap.lap_time_ms)
            .min()
    }

    /// The mean lap, which is what says whether the car got easier to drive
    /// rather than whether one lap came together.
    pub fn mean_lap_ms(&self) -> Option<f32> {
        let times: Vec<f32> = self
            .laps
            .iter()
            .filter(|lap| lap.valid && lap.lap_time_ms > 0)
            .map(|lap| lap.lap_time_ms as f32)
            .collect();
        if times.is_empty() {
            return None;
        }
        Some(times.iter().sum::<f32>() / times.len() as f32)
    }

    /// Mean of a per-lap counter, for the behaviour comparisons.
    fn mean_of(&self, get: impl Fn(&LapData) -> f32) -> Option<f32> {
        if self.laps.is_empty() {
            return None;
        }
        Some(self.laps.iter().map(&get).sum::<f32>() / self.laps.len() as f32)
    }

    /// Lap times as evidence: how many laps, and how consistent they were.
    fn lap_evidence(&self) -> Evidence {
        Evidence::from_values(
            self.laps
                .iter()
                .filter(|lap| lap.valid && lap.lap_time_ms > 0)
                .map(|lap| lap.lap_time_ms as f32),
        )
    }
}

/// One measured difference between the laps before a change and the laps after.
#[derive(Debug, Clone)]
pub struct Effect {
    pub name: String,
    pub before: f32,
    pub after: f32,
    pub unit: String,
    /// Whether a smaller number is the better one, so the display can colour it
    /// without knowing what any of these mean.
    pub lower_is_better: bool,
}

impl Effect {
    pub fn change(&self) -> f32 {
        self.after - self.before
    }

    pub fn is_improvement(&self) -> bool {
        if self.lower_is_better {
            self.after < self.before
        } else {
            self.after > self.before
        }
    }

    /// The change as a percentage of where it started. `None` when it started
    /// at zero, where a percentage is either infinite or meaningless.
    pub fn percent(&self) -> Option<f32> {
        if self.before.abs() <= f32::EPSILON {
            return None;
        }
        Some((self.after - self.before) / self.before.abs() * 100.0)
    }
}

/// A setup change, what happened after it, and what else moved at the same time.
#[derive(Debug, Clone)]
pub struct Attribution {
    /// What was actually changed on the car.
    pub changes: Vec<SetupDiffItem>,
    /// What the car and the driver did differently afterwards.
    pub effects: Vec<Effect>,
    /// Everything else that moved between the two stints and would explain the
    /// effects just as well.
    ///
    /// Never empty on a real stint, and printed beside the effects rather than
    /// under them: this is the difference between a measurement and a claim.
    pub confounders: Vec<String>,
    /// How much to trust any of it, from how many laps were driven either side
    /// and how consistent they were.
    pub confidence: Confidence,
    pub laps_before: usize,
    pub laps_after: usize,
}

impl Attribution {
    /// Whether this is worth putting in front of a driver at all.
    ///
    /// One lap either side is not a comparison, it is two laps.
    pub fn is_worth_reporting(&self) -> bool {
        !self.changes.is_empty() && self.laps_before >= 2 && self.laps_after >= 2
    }
}

/// How much the track temperature has to move before it is worth naming as a
/// confounder, in °C. Below this it is the same afternoon.
const TEMP_CONFOUNDER_C: f32 = 2.0;

/// ...and how much grip, as a fraction of the scale the analyser records it on.
const GRIP_CONFOUNDER: f32 = 1.0;

/// The setups this session has been driven on, oldest first.
#[derive(Debug, Clone, Default)]
pub struct SetupHistory {
    stints: Vec<Stint>,
}

impl SetupHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stints(&self) -> &[Stint] {
        &self.stints
    }

    pub fn is_empty(&self) -> bool {
        self.stints.is_empty()
    }

    /// Note the setup that is on the car now.
    ///
    /// Starts a new stint when it differs from the one the last laps were
    /// driven on. Called freely — an unchanged setup is not a change, and a
    /// session where nobody touches anything stays one stint.
    pub fn observe_setup(&mut self, setup: &CarSetup) {
        let unchanged = self
            .stints
            .last()
            .is_some_and(|last| last.setup.generate_diff(setup).is_empty());
        if unchanged {
            return;
        }

        // A change made before a single lap was driven on the previous setup
        // replaces it rather than stacking: a driver flicking through presets
        // in the garage has not driven three stints.
        if self.stints.last().is_some_and(|last| last.laps.is_empty())
            && let Some(last) = self.stints.last_mut()
        {
            last.setup = setup.clone();
            return;
        }

        self.stints.push(Stint {
            setup: setup.clone(),
            laps: Vec::new(),
        });
    }

    /// Record a finished lap against whatever setup is current.
    ///
    /// A lap arriving before any setup was seen is dropped rather than
    /// attributed to a setup nobody knows: an unattributable lap is worse than
    /// no lap, because it would be compared against the next stint as though it
    /// belonged to the previous one.
    pub fn record_lap(&mut self, lap: &LapData) {
        // A lap read back from a file was driven on another day, in another
        // car, on a setup this history knows nothing about. It reaches
        // `analyzer.laps` the same way a driven one does, so the guard is here
        // rather than at every call site.
        if lap.from_file {
            return;
        }
        if let Some(current) = self.stints.last_mut() {
            current.laps.push(lap.clone());
        }
    }

    /// What the most recent setup change did, if there is anything to say.
    ///
    /// `None` before a second setup, or before either side has laps on it.
    pub fn last_change(&self) -> Option<Attribution> {
        let after = self.stints.last()?;
        let before = self.stints.get(self.stints.len().checked_sub(2)?)?;
        Some(attribute(before, after))
    }
}

/// Compare two stints without claiming that the setup caused the difference.
pub fn attribute(before: &Stint, after: &Stint) -> Attribution {
    let changes = after.setup.generate_diff(&before.setup);

    let mut effects = Vec::new();
    let mut add = |name: &str, unit: &str, lower_is_better: bool, get: fn(&LapData) -> f32| {
        if let (Some(b), Some(a)) = (before.mean_of(get), after.mean_of(get)) {
            effects.push(Effect {
                name: name.to_string(),
                before: b,
                after: a,
                unit: unit.to_string(),
                lower_is_better,
            });
        }
    };

    add("Lap time", "s", true, |lap| lap.lap_time_ms as f32 / 1000.0);
    add("Oversteer", "x", true, |lap| lap.oversteer_count as f32);
    add("Understeer", "x", true, |lap| lap.understeer_count as f32);
    add("Lockups", "x", true, |lap| lap.lockup_count as f32);
    add("Min corner speed", "km/h", false, |lap| {
        lap.min_corner_speed_avg
    });
    add("Car control", "", false, |lap| lap.car_control_score);

    Attribution {
        changes,
        effects,
        confounders: confounders(before, after),
        confidence: agreement(before, after),
        laps_before: before.laps.len(),
        laps_after: after.laps.len(),
    }
}

/// Everything that moved alongside the setup and would explain the same effect.
///
/// The plan's one caution, and the reason this module is not simply a diff: on
/// a real stint the tyres, the fuel and the track all move too.
fn confounders(before: &Stint, after: &Stint) -> Vec<String> {
    let mut out = Vec::new();

    if let (Some(b), Some(a)) = (
        before.mean_of(|lap| lap.road_temp),
        after.mean_of(|lap| lap.road_temp),
    ) && (a - b).abs() >= TEMP_CONFOUNDER_C
    {
        out.push(format!("the track went from {b:.0} °C to {a:.0} °C",));
    }

    if let (Some(b), Some(a)) = (
        before.mean_of(|lap| lap.track_grip),
        after.mean_of(|lap| lap.track_grip),
    ) && (a - b).abs() >= GRIP_CONFOUNDER
    {
        out.push(format!("grip went from {b:.0}% to {a:.0}%"));
    }

    // Fuel always burns off, so this is not conditional on a threshold the way
    // the weather is — it is conditional on there being a measurement at all.
    if let (Some(b), Some(a)) = (
        before.mean_of(|lap| lap.fuel_used),
        after.mean_of(|lap| lap.fuel_used),
    ) && (a - b).abs() > 0.05
    {
        out.push(format!(
            "fuel use per lap went from {b:.2} L to {a:.2} L, so the car was carrying a different weight"
        ));
    }

    // The tyres are older on the later stint unless they were changed, and this
    // has no way to know whether they were.
    if !after.laps.is_empty() && !before.laps.is_empty() {
        out.push("the tyres are older than they were before the change".to_string());
    }

    out
}

/// How much to trust the comparison, from the laps behind it.
///
/// Both sides have to be consistent: five scattered laps before a change and
/// two after is not a measurement of the change, it is a measurement of the
/// driver warming up.
fn agreement(before: &Stint, after: &Stint) -> Confidence {
    let both = before
        .lap_evidence()
        .confidence()
        .min(after.lap_evidence().confidence());
    // Never better than Medium, whatever the numbers say. Something that
    // cannot separate a setup change from a track that rubbered in has no
    // business calling itself certain — see the note at the top of the module.
    both.min(Confidence::Medium)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(arb_rear: u32) -> CarSetup {
        CarSetup {
            name: format!("arb{arb_rear}"),
            arb_rear,
            arb_front: 5,
            ..Default::default()
        }
    }

    fn lap(number: i32, time_ms: i32, oversteer: i32) -> LapData {
        LapData {
            lap_number: number,
            lap_time_ms: time_ms,
            valid: true,
            oversteer_count: oversteer,
            road_temp: 30.0,
            track_grip: 98.0,
            fuel_used: 2.8,
            ..Default::default()
        }
    }

    #[test]
    fn an_unchanged_setup_is_not_a_new_stint() {
        let mut history = SetupHistory::new();
        history.observe_setup(&setup(4));
        history.record_lap(&lap(1, 91_000, 5));
        history.observe_setup(&setup(4));
        history.observe_setup(&setup(4));

        assert_eq!(history.stints().len(), 1);
        assert_eq!(history.stints()[0].laps.len(), 1);
    }

    /// A driver flicking through presets in the garage has not driven three
    /// stints, and three empty stints would push the real comparison off the
    /// end of the history.
    #[test]
    fn setups_changed_without_driving_replace_rather_than_stack() {
        let mut history = SetupHistory::new();
        history.observe_setup(&setup(4));
        history.observe_setup(&setup(5));
        history.observe_setup(&setup(6));

        assert_eq!(history.stints().len(), 1);
        assert_eq!(history.stints()[0].setup.arb_rear, 6);
    }

    /// A ghost loaded from disk was driven on another day, on a setup this
    /// knows nothing about. It reaches `analyzer.laps` the same way a driven
    /// lap does, and counting it would attribute somebody else's lap to the
    /// change the driver just made.
    #[test]
    fn a_lap_loaded_from_a_file_is_not_a_lap_of_this_stint() {
        let mut history = SetupHistory::new();
        history.observe_setup(&setup(4));

        let mut ghost = lap(9, 88_000, 0);
        ghost.from_file = true;
        history.record_lap(&ghost);
        history.record_lap(&lap(1, 91_000, 5));

        assert_eq!(history.stints()[0].laps.len(), 1);
        assert_eq!(history.stints()[0].laps[0].lap_time_ms, 91_000);
    }

    #[test]
    fn a_lap_before_any_setup_is_dropped_rather_than_misattributed() {
        let mut history = SetupHistory::new();
        history.record_lap(&lap(1, 91_000, 5));
        assert!(history.is_empty());
    }

    /// The output the plan asks for: what changed, and what happened next.
    #[test]
    fn a_change_is_measured_against_the_laps_either_side_of_it() {
        let mut history = SetupHistory::new();

        history.observe_setup(&setup(4));
        history.record_lap(&lap(1, 91_400, 10));
        history.record_lap(&lap(2, 91_600, 12));

        history.observe_setup(&setup(3));
        history.record_lap(&lap(3, 91_200, 8));
        history.record_lap(&lap(4, 91_000, 8));

        let attribution = history.last_change().expect("two stints, laps on each");
        assert!(attribution.is_worth_reporting());
        assert_eq!(attribution.laps_before, 2);
        assert_eq!(attribution.laps_after, 2);

        assert!(
            attribution
                .changes
                .iter()
                .any(|change| change.name.contains("ARB")),
            "{:?}",
            attribution.changes
        );

        let lap_time = attribution
            .effects
            .iter()
            .find(|effect| effect.name == "Lap time")
            .expect("lap time is always an effect");
        assert!(lap_time.is_improvement(), "{lap_time:?}");
        assert!((lap_time.change() + 0.4).abs() < 0.01, "{lap_time:?}");

        let oversteer = attribution
            .effects
            .iter()
            .find(|effect| effect.name == "Oversteer")
            .expect("oversteer is counted per lap");
        assert!(oversteer.is_improvement());
        assert!(
            oversteer.percent().is_some_and(|p| p < -20.0),
            "{oversteer:?}"
        );
    }

    /// The point of the whole module: never claim a cause.
    #[test]
    fn everything_that_moved_alongside_is_reported_too() {
        let mut history = SetupHistory::new();

        history.observe_setup(&setup(4));
        history.record_lap(&lap(1, 91_400, 10));
        history.record_lap(&lap(2, 91_600, 12));

        history.observe_setup(&setup(3));
        let mut warmer = lap(3, 91_200, 8);
        warmer.road_temp = 36.0;
        warmer.fuel_used = 2.6;
        history.record_lap(&warmer);
        let mut warmer2 = lap(4, 91_000, 8);
        warmer2.road_temp = 36.0;
        warmer2.fuel_used = 2.6;
        history.record_lap(&warmer2);

        let attribution = history
            .last_change()
            .expect("a change with laps either side");
        assert!(
            attribution
                .confounders
                .iter()
                .any(|note| note.contains("track went from")),
            "{:?}",
            attribution.confounders
        );
        assert!(
            attribution
                .confounders
                .iter()
                .any(|note| note.contains("fuel use")),
            "{:?}",
            attribution.confounders
        );
        assert!(
            attribution
                .confounders
                .iter()
                .any(|note| note.contains("tyres")),
            "the tyres are always older: {:?}",
            attribution.confounders
        );
    }

    /// One lap either side is two laps, not a comparison.
    #[test]
    fn one_lap_each_side_is_not_worth_reporting() {
        let mut history = SetupHistory::new();
        history.observe_setup(&setup(4));
        history.record_lap(&lap(1, 91_400, 10));
        history.observe_setup(&setup(3));
        history.record_lap(&lap(2, 91_000, 8));

        let attribution = history.last_change().expect("there were two setups");
        assert!(!attribution.is_worth_reporting());
        assert_eq!(attribution.confidence, Confidence::Low);
    }

    /// However many laps agree, this cannot separate a setup change from a
    /// track that rubbered in — so it never says High.
    #[test]
    fn attribution_is_never_more_than_medium() {
        let mut history = SetupHistory::new();
        history.observe_setup(&setup(4));
        for number in 0..8 {
            history.record_lap(&lap(number, 91_400, 10));
        }
        history.observe_setup(&setup(3));
        for number in 8..16 {
            history.record_lap(&lap(number, 91_000, 8));
        }

        let attribution = history.last_change().expect("eight laps either side");
        assert!(attribution.is_worth_reporting());
        assert_eq!(attribution.confidence, Confidence::Medium);
    }

    #[test]
    fn a_percentage_of_nothing_is_not_a_percentage() {
        let effect = Effect {
            name: "Lockups".to_string(),
            before: 0.0,
            after: 3.0,
            unit: "x".to_string(),
            lower_is_better: true,
        };
        assert_eq!(effect.percent(), None);
        assert!(!effect.is_improvement());
    }
}
