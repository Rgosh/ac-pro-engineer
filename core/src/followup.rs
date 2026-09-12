//! What was wrong last time, and whether it is still wrong.
//!
//! **The engineer has never once followed up on itself.** It says "raise the
//! rear wing two clicks", the driver does it, and the program's next word on
//! the subject is the same sentence or a different one — never "you did that,
//! and here is what happened". A recommendation carries a [`Chain::confirm`]
//! precisely because somebody is meant to check; nothing ever checked.
//!
//! That missing half is what separates an engineer from a warning light. A
//! warning light repeats itself. An engineer remembers what they told you.
//!
//! # The engineer is its own oracle
//!
//! The obvious design is to model each setting: read the setup file, find the
//! wing, see whether it moved. It is the wrong one. Setup files differ between
//! the two games, a driver changes things in the garage without ever saving
//! one, and half of what the engineer asks for is a driving change with no
//! file to read at all.
//!
//! So the question is not "did the wing move". It is **"is the engineer still
//! complaining"** — and that needs nothing but the findings themselves. If it
//! said the rear tyres were cooking and it no longer says so, the rear tyres
//! are no longer cooking, whatever was done about it. The judge is the same
//! judge as last time, which is the only way the two answers are comparable.
//!
//! # What this does not claim
//!
//! It does not claim the driver's change *caused* the improvement. Track
//! temperature moves, fuel loads differ, and a driver on their second evening
//! is a better driver than on their first. [`Change::said`] is worded as what
//! was observed and never as what was proven, and `then`/`now` are both kept
//! so the reader can disagree with it.

use crate::engineer::{Parameter, Recommendation, Severity};
use serde::{Deserialize, Serialize};

/// One of the engineer's findings, kept small enough to write down.
///
/// A whole [`Recommendation`] carries its `Chain`, its evidence and its
/// parameters; this is what is worth keeping between sessions, which is what
/// it was about and how bad it was.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Said {
    pub component: String,
    pub category: String,
    pub severity: Severity,
    pub message: String,
    /// The settings it asked for, if it asked for any.
    #[serde(default)]
    pub parameters: Vec<Parameter>,
}

impl Said {
    /// What identifies "the same complaint" across two sessions.
    ///
    /// **The component alone.** Never the message: a message carries its own
    /// numbers — "FR at 96 °C" — so it is a different string every session
    /// about the same thing, and matching on it would report every finding as
    /// gone and a new one arrived.
    ///
    /// And not the category either, though that was the first attempt. The
    /// engineer files two tyre findings under two categories of its own, and
    /// matching on the pair reported the tyres as **fixed and new at the same
    /// time** — two lines contradicting each other about one component, which
    /// is worse than either line alone. A driver asks "are the tyres sorted",
    /// not "is the pressure category sorted".
    pub fn about(&self) -> &str {
        &self.component
    }

    /// Whether two findings are about the same thing.
    ///
    /// **Case-insensitive, because the engineer is not consistent about it.**
    /// It emits `TYRES` from one rule and `Tyres` from another, about the same
    /// tyres — so a case-sensitive match put "TYRES no longer flagged" one
    /// line above "Tyres new since last time", in two different colours, on
    /// one screen. Fixing the engineer's casing would fix this one pair and
    /// leave the next one to be found by a driver.
    pub fn same_thing_as(&self, other: &Said) -> bool {
        self.component.eq_ignore_ascii_case(&other.component)
    }
}

impl From<&Recommendation> for Said {
    fn from(advice: &Recommendation) -> Self {
        Self {
            component: advice.component.clone(),
            category: advice.category.clone(),
            severity: advice.severity.clone(),
            message: advice.message.clone(),
            parameters: advice.parameters.clone(),
        }
    }
}

/// The worst finding about each thing, which is what a session is remembered
/// by.
///
/// Two findings about the same component in one session is the engineer
/// looking at it twice; the one that mattered is the worse one, and carrying
/// both would make every comparison a list of pairs.
pub fn worst_of(advice: &[Recommendation]) -> Vec<Said> {
    let mut kept: Vec<Said> = Vec::new();
    for one in advice {
        let said = Said::from(one);
        match kept.iter_mut().find(|other| other.same_thing_as(&said)) {
            Some(there) if there.severity < said.severity => *there = said,
            Some(_) => {}
            None => kept.push(said),
        }
    }
    kept
}

/// What happened to one complaint between two sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It complained then and says nothing now. The thing is fixed.
    Gone,
    /// Still complaining, but less loudly.
    Eased,
    /// The same complaint at the same severity.
    Same,
    /// Still complaining, and worse.
    Worse,
    /// Nothing then, a complaint now.
    New,
}

impl Outcome {
    /// Whether this is the loop paying off — something the driver was told to
    /// fix and then fixed.
    pub fn is_progress(self) -> bool {
        matches!(self, Outcome::Gone | Outcome::Eased)
    }

    /// The order these are read in. Progress first, because it is the answer
    /// to the question the driver actually asked.
    fn rank(self) -> u8 {
        match self {
            Outcome::Gone => 0,
            Outcome::Eased => 1,
            Outcome::New => 2,
            Outcome::Worse => 3,
            Outcome::Same => 4,
        }
    }
}

/// One complaint, then and now.
#[derive(Debug, Clone)]
pub struct Change {
    pub component: String,
    pub category: String,
    pub outcome: Outcome,
    pub then: Option<Said>,
    pub now: Option<Said>,
}

impl Change {
    /// The sentence a driver reads, after the component's own name.
    ///
    /// **No article and no verb agreeing with the component.** A component is
    /// whatever the engineer calls it — `TYRES`, `FORCE FEEDBACK`, `BRAKING` —
    /// and a sentence built around it produced "the tyres is a new finding".
    /// Written this way it reads correctly after any name there will ever be,
    /// including one added next year.
    ///
    /// Worded as what was observed and never as what was proven. "No longer
    /// flagged" is true; "raising the wing fixed the rear tyres" is a claim
    /// about cause that nothing here can support.
    pub fn said(&self) -> &'static str {
        match self.outcome {
            Outcome::Gone => "no longer flagged",
            Outcome::Eased => "still flagged, less serious than last time",
            Outcome::Same => "unchanged since last time",
            Outcome::Worse => "worse than last time",
            Outcome::New => "new since last time",
        }
    }

    /// What the driver was asked to change about this, last time.
    ///
    /// **The reason a `Gone` is worth reading.** "The rear tyres are fine now"
    /// is pleasant; "the rear tyres are fine now, and last time you were asked
    /// to take 0.3 psi out of them" is the loop closing.
    pub fn was_asked(&self) -> &[Parameter] {
        self.then
            .as_ref()
            .map(|said| said.parameters.as_slice())
            .unwrap_or_default()
    }
}

/// Everything that moved between two sessions in the same car at the same
/// track.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// One per thing that was complained about in either session.
    pub changes: Vec<Change>,
    /// The best lap then and now, in milliseconds. Negative is quicker now.
    pub lap_delta_ms: Option<i32>,
}

impl Report {
    /// Only the complaints that actually moved.
    ///
    /// **`Same` is the majority and says nothing.** A driver who reads six
    /// lines of "unchanged" to find the one that moved has been given a table
    /// again. The count of the unchanged is worth one line; the lines are not.
    pub fn moved(&self) -> Vec<&Change> {
        self.changes
            .iter()
            .filter(|change| change.outcome != Outcome::Same)
            .collect()
    }

    /// How many findings are exactly as they were.
    pub fn unchanged(&self) -> usize {
        self.changes
            .iter()
            .filter(|change| change.outcome == Outcome::Same)
            .count()
    }

    /// Whether anything the driver was told to fix is fixed.
    pub fn any_progress(&self) -> bool {
        self.changes
            .iter()
            .any(|change| change.outcome.is_progress())
    }

    /// The one line at the top.
    pub fn headline(&self) -> String {
        let fixed = self
            .changes
            .iter()
            .filter(|change| change.outcome == Outcome::Gone)
            .count();
        let fresh = self
            .changes
            .iter()
            .filter(|change| change.outcome == Outcome::New)
            .count();
        match (fixed, fresh) {
            (0, 0) => "the same things are wrong as last time".to_string(),
            (0, new) => format!("{new} new since last time"),
            (done, 0) => format!("{done} fixed since last time"),
            (done, new) => format!("{done} fixed, {new} new since last time"),
        }
    }

    /// The lap time difference as a driver reads it, when both sessions had a
    /// valid lap.
    ///
    /// `None` rather than "0.000" when either session never set one: a session
    /// with no lap is not a session that went equally well.
    pub fn pace(&self) -> Option<String> {
        let delta = self.lap_delta_ms?;
        Some(format!("{:+.3} s", delta as f32 / 1000.0))
    }
}

/// Compare what the engineer said last time with what it says now.
///
/// `before` and `now` are each one session's [`worst_of`]. The lap times are
/// the best of each session in milliseconds, zero or less where there was
/// none.
pub fn compare(before: &[Said], now: &[Said], before_best_ms: i32, now_best_ms: i32) -> Report {
    let mut changes: Vec<Change> = Vec::new();

    for then in before {
        let current = now.iter().find(|said| said.same_thing_as(then));
        let outcome = match current {
            None => Outcome::Gone,
            Some(said) if said.severity < then.severity => Outcome::Eased,
            Some(said) if said.severity > then.severity => Outcome::Worse,
            Some(_) => Outcome::Same,
        };
        changes.push(Change {
            component: then.component.clone(),
            category: then.category.clone(),
            outcome,
            then: Some(then.clone()),
            now: current.cloned(),
        });
    }

    for said in now {
        if before.iter().any(|then| then.same_thing_as(said)) {
            continue;
        }
        changes.push(Change {
            component: said.component.clone(),
            category: said.category.clone(),
            outcome: Outcome::New,
            then: None,
            now: Some(said.clone()),
        });
    }

    // Progress first, then what is new, then what got worse. Stable within a
    // rank, so two findings of the same kind keep the order the engineer
    // reported them in.
    changes.sort_by_key(|change| change.outcome.rank());

    Report {
        changes,
        lap_delta_ms: (before_best_ms > 0 && now_best_ms > 0).then(|| now_best_ms - before_best_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(component: &str, severity: Severity) -> Said {
        Said {
            component: component.to_string(),
            category: "Setup".to_string(),
            severity,
            message: format!("{component} is not right"),
            parameters: Vec::new(),
        }
    }

    /// The whole point: the engineer complained, the driver fixed it, and the
    /// program says so. Without this the feature is a diff of two lists.
    #[test]
    fn a_complaint_that_stopped_is_reported_as_fixed() {
        let before = vec![
            said("TYRES", Severity::Critical),
            said("BRAKES", Severity::Warning),
        ];
        let now = vec![said("BRAKES", Severity::Warning)];

        let report = compare(&before, &now, 91_000, 90_400);
        let fixed = report
            .changes
            .iter()
            .find(|change| change.component == "TYRES")
            .expect("the tyre finding is accounted for");
        assert_eq!(fixed.outcome, Outcome::Gone);
        assert!(report.any_progress());
        assert_eq!(report.headline(), "1 fixed since last time");
        assert_eq!(report.pace().as_deref(), Some("-0.600 s"));
    }

    /// A message carries its own numbers, so it is a different string every
    /// session about the same thing. Matching on it would report every finding
    /// as gone and a new one arrived — which is the failure that makes the
    /// whole feature worthless rather than merely wrong.
    #[test]
    fn the_same_complaint_with_different_numbers_is_the_same_complaint() {
        let mut before = said("TYRES", Severity::Warning);
        before.message = "FR at 96 °C".to_string();
        let mut now = said("TYRES", Severity::Warning);
        now.message = "FR at 94 °C".to_string();

        let report = compare(&[before], &[now], 0, 0);
        assert_eq!(report.changes.len(), 1);
        assert_eq!(report.changes[0].outcome, Outcome::Same);
    }

    /// The engineer files two findings about one component under two
    /// categories of its own. Matching on the pair reported the tyres as
    /// **fixed and new at the same time** — two lines contradicting each other
    /// about one component, which is worse than either line alone.
    #[test]
    fn one_component_is_never_both_fixed_and_new() {
        let mut before = said("TYRES", Severity::Critical);
        before.category = "Pressure".to_string();
        let mut now = said("TYRES", Severity::Warning);
        now.category = "Temperature".to_string();

        let report = compare(&[before], &[now], 0, 0);
        assert_eq!(report.changes.len(), 1, "one component, one line");
        assert_eq!(report.changes[0].outcome, Outcome::Eased);
    }

    /// The engineer writes `TYRES` from one rule and `Tyres` from another,
    /// about the same tyres. A case-sensitive match put "TYRES no longer
    /// flagged" one line above "Tyres new since last time", in two different
    /// colours, on one screen.
    #[test]
    fn the_same_component_spelled_two_ways_is_one_component() {
        let mut shouted = said("TYRES", Severity::Critical);
        shouted.category = "Pressure".to_string();
        let mut quiet = said("Tyres", Severity::Warning);
        quiet.category = "Temperature".to_string();

        let report = compare(&[shouted], &[quiet], 0, 0);
        assert_eq!(report.changes.len(), 1);
        assert_eq!(report.changes[0].outcome, Outcome::Eased);

        // And the same when one session reports it twice.
        let kept = worst_of(&[]);
        assert!(kept.is_empty());
    }

    /// A component is whatever the engineer calls it, and a sentence built
    /// around the name produced "the tyres is a new finding".
    #[test]
    fn the_sentence_reads_after_any_component_name() {
        for name in ["TYRES", "FORCE FEEDBACK", "BRAKING", "FUEL"] {
            let report = compare(&[], &[said(name, Severity::Warning)], 0, 0);
            let line = format!(
                "{} — {}",
                report.changes[0].component,
                report.changes[0].said()
            );
            assert!(line.starts_with(name), "{line}");
            assert!(line.ends_with("new since last time"), "{line}");
        }
    }

    /// Severity is what says whether it got better, and it moves both ways.
    #[test]
    fn a_finding_can_ease_and_can_worsen() {
        let eased = compare(
            &[said("TYRES", Severity::Critical)],
            &[said("TYRES", Severity::Warning)],
            0,
            0,
        );
        assert_eq!(eased.changes[0].outcome, Outcome::Eased);
        assert!(eased.any_progress());

        let worse = compare(
            &[said("TYRES", Severity::Info)],
            &[said("TYRES", Severity::Critical)],
            0,
            0,
        );
        assert_eq!(worse.changes[0].outcome, Outcome::Worse);
        assert!(!worse.any_progress());
    }

    /// Good news first. The driver came to this panel to find out whether what
    /// they did worked, and burying it under four unchanged findings answers a
    /// different question.
    #[test]
    fn progress_is_read_before_everything_else() {
        let before = vec![
            said("FUEL", Severity::Info),
            said("TYRES", Severity::Critical),
        ];
        let now = vec![
            said("FUEL", Severity::Info),
            said("BRAKES", Severity::Warning),
        ];

        let report = compare(&before, &now, 0, 0);
        assert_eq!(report.changes[0].outcome, Outcome::Gone);
        assert_eq!(report.changes[0].component, "TYRES");
        assert_eq!(report.moved().len(), 2, "the unchanged FUEL is not `moved`");
        assert_eq!(report.unchanged(), 1);
    }

    /// A car model is whatever the game calls it, and mod authors call them
    /// anything at all — including things with slashes in, which is a path
    /// escaping the directory it was meant to be in.
    #[test]
    fn a_car_that_names_itself_a_path_stays_in_its_own_directory() {
        let root = std::env::temp_dir().join("rg-followup-path-test");
        let store = store::Store::in_dir(root.clone());
        let session = store::Remembered {
            started: "2026-09-12 10:00".to_string(),
            car: "../../etc/passwd".to_string(),
            track: "../../../root".to_string(),
            laps: 1,
            best_ms: 90_000,
            findings: vec![said("TYRES", Severity::Warning)],
        };
        store.remember(&session).expect("it writes");

        let inside: Vec<_> = std::fs::read_dir(store.at())
            .expect("the directory exists")
            .flatten()
            .collect();
        assert_eq!(inside.len(), 1, "one file, and it is in here");
        assert_eq!(
            store.last_time("../../etc/passwd", "../../../root", "now"),
            Some(session)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Handing back the session being driven now makes every finding compare
    /// against itself, which reads as a driver who has changed nothing.
    #[test]
    fn the_session_being_driven_is_not_its_own_reference() {
        let root = std::env::temp_dir().join("rg-followup-self-test");
        let store = store::Store::in_dir(root.clone());
        let session = store::Remembered {
            started: "2026-09-12 10:00".to_string(),
            car: "bmw_z4_gt3".to_string(),
            track: "spa".to_string(),
            laps: 4,
            best_ms: 90_000,
            findings: vec![said("TYRES", Severity::Warning)],
        };
        store.remember(&session).expect("it writes");

        assert_eq!(
            store.last_time("bmw_z4_gt3", "spa", "2026-09-12 10:00"),
            None,
            "this is the session in progress"
        );
        assert!(
            store
                .last_time("bmw_z4_gt3", "spa", "2026-09-12 21:30")
                .is_some(),
            "a later session compares against it"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A session with nothing wrong in it has nothing for the next one to
    /// follow up on, and offering it would make everything read as new.
    #[test]
    fn a_session_with_no_findings_is_no_reference() {
        let root = std::env::temp_dir().join("rg-followup-empty-test");
        let store = store::Store::in_dir(root.clone());
        store
            .remember(&store::Remembered {
                started: "2026-09-12 10:00".to_string(),
                car: "bmw_z4_gt3".to_string(),
                track: "spa".to_string(),
                laps: 4,
                best_ms: 90_000,
                findings: Vec::new(),
            })
            .expect("it writes");
        assert_eq!(store.last_time("bmw_z4_gt3", "spa", "later"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A session with no valid lap is not a session that went equally well,
    /// and "+0.000 s" would say that it was.
    #[test]
    fn a_session_without_a_lap_has_no_pace_to_compare() {
        assert_eq!(compare(&[], &[], 0, 90_000).pace(), None);
        assert_eq!(compare(&[], &[], 90_000, 0).pace(), None);
    }

    /// Two findings about one component in one session is the engineer looking
    /// at it twice; the one that mattered is the worse one.
    #[test]
    fn one_component_is_remembered_by_its_worst_finding() {
        use crate::engineer::Recommendation;
        let advice = |severity: Severity| Recommendation {
            component: "TYRES".to_string(),
            category: "Setup".to_string(),
            severity,
            message: String::new(),
            action: String::new(),
            parameters: Vec::new(),
            confidence: 0.5,
            chain: None,
        };

        let kept = worst_of(&[advice(Severity::Info), advice(Severity::Critical)]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].severity, Severity::Critical);
    }

    /// What the driver was asked to do is kept with the finding, so a `Gone`
    /// can say what was asked for rather than only that something improved.
    #[test]
    fn a_fixed_finding_still_knows_what_was_asked_for() {
        let mut then = said("TYRES", Severity::Warning);
        then.parameters = vec![Parameter {
            name: "FR cold pressure".to_string(),
            current: 27.5,
            target: 27.2,
            unit: "psi".to_string(),
        }];

        let report = compare(&[then], &[], 0, 0);
        assert_eq!(report.changes[0].outcome, Outcome::Gone);
        assert_eq!(report.changes[0].was_asked().len(), 1);
        assert_eq!(report.changes[0].was_asked()[0].target, 27.2);
    }
}

/// Where what the engineer said is kept between sessions.
///
/// **In the core, so all three front ends answer the question the same way.**
/// The window grew this first, keeping the findings inside its own session
/// folders; the terminal has no such folders, and a second implementation of
/// "what did it say last time" is a second set of answers to one question.
/// This is the one place, and it is the same shape as [`crate::records`] next
/// door: a small JSON file per car and track under the application's own
/// directory.
///
/// One session per car and track — the last one. A history is a different
/// feature and this is not it: the question is "did what I changed work", and
/// the answer comes from the session before this one.
pub mod store {
    use super::Said;
    use serde::{Deserialize, Serialize};
    use std::path::{Path, PathBuf};

    /// What one finished session is remembered by.
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    pub struct Remembered {
        /// When it started, as the front end writes it. Compared, never
        /// parsed: it is how a session tells itself apart from the one being
        /// driven now.
        pub started: String,
        pub car: String,
        pub track: String,
        pub laps: usize,
        /// The quickest lap, in milliseconds. Zero when none was valid.
        pub best_ms: i32,
        pub findings: Vec<Said>,
    }

    impl Remembered {
        /// The best lap as a driver reads it, or a dash where there was none.
        pub fn best(&self) -> String {
            if self.best_ms <= 0 {
                return "—".to_string();
            }
            let seconds = self.best_ms as f32 / 1000.0;
            format!(
                "{}:{:06.3}",
                (seconds / 60.0).floor() as i32,
                seconds % 60.0
            )
        }
    }

    /// One file per car and track, under the application's own directory.
    pub struct Store {
        at: PathBuf,
    }

    impl Default for Store {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Store {
        pub fn new() -> Self {
            Self::in_dir(crate::config::app_dir())
        }

        /// For the tests, which must not write where a driver's own sessions
        /// are — and for anything that wants to keep this somewhere else.
        pub fn in_dir(data_dir: PathBuf) -> Self {
            Self {
                at: data_dir.join("followup"),
            }
        }

        /// The file one car at one track is kept in.
        ///
        /// **Named by a digest and not by the car.** A car model is whatever
        /// the game calls it and mod authors call them anything at all —
        /// including things with slashes in, which is a path escaping its own
        /// directory. The names are inside the file, where they cannot be a
        /// path.
        fn file_for(&self, car: &str, track: &str) -> PathBuf {
            let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
            for byte in car.as_bytes().iter().chain(b"\x1f").chain(track.as_bytes()) {
                digest ^= *byte as u64;
                digest = digest.wrapping_mul(0x100_0000_01b3);
            }
            self.at.join(format!("{digest:016x}.json"))
        }

        /// Write this session down as the one the next will compare against.
        ///
        /// **Called on every lap and not at the end.** A program that is killed
        /// keeps what it knew up to the last lap, and the session most worth
        /// remembering is the one where something went wrong.
        pub fn remember(&self, session: &Remembered) -> std::io::Result<()> {
            if session.car.is_empty() || session.track.is_empty() {
                return Ok(());
            }
            std::fs::create_dir_all(&self.at)?;
            let written = serde_json::to_vec_pretty(session)
                .map_err(|why| std::io::Error::other(why.to_string()))?;
            crate::atomic_file::write_atomic(&self.file_for(&session.car, &session.track), &written)
        }

        /// The last session in this car at this track, if it is not the one
        /// being driven now.
        ///
        /// `started_now` is this session's own stamp: without it the store
        /// would hand back the session in progress and every finding would
        /// compare against itself, which reads as a driver who has changed
        /// nothing all evening.
        pub fn last_time(&self, car: &str, track: &str, started_now: &str) -> Option<Remembered> {
            let bytes = std::fs::read(self.file_for(car, track)).ok()?;
            let kept: Remembered = serde_json::from_slice(&bytes).ok()?;
            (kept.started != started_now && !kept.findings.is_empty()).then_some(kept)
        }

        /// Where the files are, for a front end that wants to say so.
        pub fn at(&self) -> &Path {
            &self.at
        }
    }
}
