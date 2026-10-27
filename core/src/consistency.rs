//! How many laps in a row were close to your best — the consistency trainer.
//!
//! **Pace is one lap; racing is twenty.** Every screen in this program can say
//! how fast the best lap was, and none of them could say the thing a driver
//! practising for a race actually works on: whether they can put that lap in
//! again, and again. A personal best that arrives once in an evening and is
//! followed by eight laps a second off it is not race pace, and a driver who
//! knows that practises differently.
//!
//! So this counts. A lap is a **hit** when it was valid and inside a window of
//! the session's best; the **streak** is the hits in a row ending with the
//! newest lap, and the **longest** is the best run of the session. An invalid
//! lap, a spin, an in-lap — anything outside the window — ends a run, because a
//! run is consecutive or it is not a run.
//!
//! # The window
//!
//! Half a percent of the best lap, and never less than three tenths. Half a
//! percent is 0.6 s round a two-minute lap and 0.4 s round an eighty-second
//! one — the gap between a driver on a clean stint and one who is not, in
//! every league that publishes a consistency table. A fixed number would be
//! generous at Brands Hatch and impossible at the Nordschleife.
//!
//! # What it decides
//!
//! Nothing about the car. It reads lap times the analyser already decided were
//! laps, and validity the game already decided — and says nothing at all
//! until there is a valid lap to measure against, because "0 in a row" before
//! anybody has driven is not a fact about the driving.

use crate::analyzer::LapData;
use crate::i18n::{Translate, tr_fmt};

/// The shortest lap anything here believes, the same floor the records use.
const MIN_LAP_MS: i32 = 10_000;

/// The window as a share of the best lap.
const WINDOW_SHARE: f32 = 0.005;

/// And never tighter than this.
const WINDOW_FLOOR_MS: i32 = 300;

/// A lap slower than this share of the best is not a racing lap — an out-lap,
/// a spin, a trip through the pits — and is left out of the spread, though it
/// still ends a run.
const RACING_LAP: f32 = 1.07;

/// The session, counted.
#[derive(Clone, Debug, PartialEq)]
pub struct Consistency {
    /// The session's best valid lap.
    pub best_ms: i32,
    /// Which lap that was, as the game numbers them.
    pub best_lap: i32,
    /// How close counts as a hit.
    pub window_ms: i32,
    /// Laps judged: every timed lap driven this session.
    pub judged: usize,
    /// Of those, how many were hits.
    pub hits: usize,
    /// Hits in a row, ending with the newest lap.
    pub streak: usize,
    /// The longest run of the session.
    pub longest: usize,
    /// The standard deviation of the racing laps, in milliseconds — `None`
    /// with fewer than three, where a spread is two numbers and a guess.
    pub spread_ms: Option<i32>,
}

/// The window for a best lap of this length.
pub fn window_for(best_ms: i32) -> i32 {
    ((best_ms as f32 * WINDOW_SHARE).round() as i32).max(WINDOW_FLOOR_MS)
}

/// Count the session.
///
/// Laps loaded from a file are not this session's driving and are left out.
/// `None` until a valid lap has been timed.
pub fn measure(laps: &[LapData]) -> Option<Consistency> {
    let timed: Vec<&LapData> = laps
        .iter()
        .filter(|lap| !lap.from_file && lap.lap_time_ms >= MIN_LAP_MS)
        .collect();
    let best = timed
        .iter()
        .filter(|lap| lap.valid)
        .min_by_key(|lap| lap.lap_time_ms)?;
    let best_ms = best.lap_time_ms;
    let window_ms = window_for(best_ms);

    let hit = |lap: &&LapData| lap.valid && lap.lap_time_ms <= best_ms + window_ms;
    let mut hits = 0;
    let mut run = 0;
    let mut longest = 0;
    for lap in &timed {
        if hit(lap) {
            hits += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }

    let racing: Vec<f64> = timed
        .iter()
        .filter(|lap| lap.valid && (lap.lap_time_ms as f32) <= best_ms as f32 * RACING_LAP)
        .map(|lap| lap.lap_time_ms as f64)
        .collect();
    let spread_ms = (racing.len() >= 3).then(|| {
        let mean = racing.iter().sum::<f64>() / racing.len() as f64;
        let variance =
            racing.iter().map(|ms| (ms - mean).powi(2)).sum::<f64>() / racing.len() as f64;
        variance.sqrt().round() as i32
    });

    Some(Consistency {
        best_ms,
        best_lap: best.lap_number,
        window_ms,
        judged: timed.len(),
        hits,
        streak: run,
        longest,
        spread_ms,
    })
}

impl Consistency {
    /// The share of laps that were hits, 0–100.
    pub fn hit_rate(&self) -> u32 {
        if self.judged == 0 {
            return 0;
        }
        (self.hits as f32 * 100.0 / self.judged as f32).round() as u32
    }

    /// The window as a driver reads it: "0.5 s".
    pub fn window(&self) -> String {
        format!("{:.1} s", self.window_ms as f32 / 1000.0)
    }

    /// One line, the same in every front end.
    pub fn headline(&self, ru: bool) -> String {
        tr_fmt(
            "{0} in a row within {1} of your best · longest {2} · {3} of {4} laps",
            ru,
            &[
                &self.streak.to_string(),
                &self.window(),
                &self.longest.to_string(),
                &self.hits.to_string(),
                &self.judged.to_string(),
            ],
        )
    }

    /// What the spread says, when there is one.
    pub fn spread(&self, ru: bool) -> Option<String> {
        self.spread_ms.map(|ms| {
            tr_fmt(
                "lap-to-lap spread ±{0}",
                ru,
                &[&format!("{:.2} s", ms as f32 / 1000.0)],
            )
        })
    }

    /// Whether the newest lap just set the session's longest run — worth a
    /// word on screen, and nothing else here is.
    pub fn new_longest(&self) -> bool {
        self.streak >= 3 && self.streak == self.longest
    }

    /// The label above it, for both front ends.
    pub fn title(ru: bool) -> &'static str {
        "CONSISTENCY".tr(ru)
    }
}

/// A lap time as a driver reads it: `1:42.315`.
///
/// Here because the report and both front ends print the same times, and the
/// window and the terminal each had their own.
pub fn lap_time(ms: i32) -> String {
    if ms <= 0 {
        return "—".to_string();
    }
    let minutes = ms / 60_000;
    let seconds = (ms % 60_000) as f32 / 1000.0;
    format!("{minutes}:{seconds:06.3}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lap(number: i32, ms: i32, valid: bool) -> LapData {
        LapData {
            lap_number: number,
            lap_time_ms: ms,
            valid,
            ..Default::default()
        }
    }

    /// The window scales with the lap and has a floor.
    #[test]
    fn the_window_is_half_a_percent_and_never_tiny() {
        assert_eq!(window_for(120_000), 600);
        assert_eq!(window_for(80_000), 400);
        assert_eq!(window_for(40_000), WINDOW_FLOOR_MS);
    }

    /// **A run is consecutive or it is not a run.** A cut lap ends it, even a
    /// fast one, and a slow lap ends it.
    #[test]
    fn runs_are_counted_and_broken() {
        let laps = vec![
            lap(1, 125_000, true), // out of the window: the best is 100.0
            lap(2, 100_300, true),
            lap(3, 100_000, true),
            lap(4, 100_400, true),
            lap(5, 99_000, false), // quick, and cut
            lap(6, 100_200, true),
            lap(7, 100_100, true),
        ];
        let counted = measure(&laps).expect("there is a valid lap");
        assert_eq!(counted.best_ms, 100_000, "the cut 1:39 is not the best");
        assert_eq!(counted.best_lap, 3);
        assert_eq!(counted.window_ms, 500);
        assert_eq!(counted.judged, 7);
        assert_eq!(counted.hits, 5);
        assert_eq!(counted.longest, 3);
        assert_eq!(counted.streak, 2);
        assert_eq!(counted.hit_rate(), 71);
        assert!(!counted.new_longest());
    }

    /// Nothing is said before there is a lap to measure against.
    #[test]
    fn nothing_before_a_valid_lap() {
        assert_eq!(measure(&[]), None);
        assert_eq!(measure(&[lap(1, 95_000, false)]), None);
    }

    /// A lap loaded from a file is not this session's driving.
    #[test]
    fn laps_from_files_are_not_counted() {
        let mut saved = lap(1, 90_000, true);
        saved.from_file = true;
        let counted = measure(&[saved, lap(2, 100_000, true)]).expect("one lap");
        assert_eq!(counted.best_ms, 100_000);
        assert_eq!(counted.judged, 1);
    }

    /// The spread leaves out the in-lap, and needs three laps to mean anything.
    #[test]
    fn the_spread_is_of_racing_laps() {
        let two = measure(&[lap(1, 100_000, true), lap(2, 100_400, true)]).expect("laps");
        assert_eq!(two.spread_ms, None);

        let laps = vec![
            lap(1, 100_000, true),
            lap(2, 100_200, true),
            lap(3, 100_400, true),
            lap(4, 140_000, true), // through the pits
        ];
        let counted = measure(&laps).expect("laps");
        assert_eq!(counted.spread_ms, Some(163));
    }

    /// The newest lap extending the longest run is the moment worth marking.
    #[test]
    fn a_new_longest_run_is_noticed() {
        let laps: Vec<LapData> = (1..=4).map(|n| lap(n, 100_000 + n * 50, true)).collect();
        let counted = measure(&laps).expect("laps");
        assert_eq!(counted.streak, 4);
        assert!(counted.new_longest());
        assert_eq!(
            counted.headline(false),
            "4 in a row within 0.5 s of your best · longest 4 · 4 of 4 laps"
        );
    }

    #[test]
    fn lap_times_read_as_a_driver_writes_them() {
        assert_eq!(lap_time(102_315), "1:42.315");
        assert_eq!(lap_time(59_999), "0:59.999");
        assert_eq!(lap_time(0), "—");
    }
}
