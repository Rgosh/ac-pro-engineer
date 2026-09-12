//! The lap already in your hands, made of the pieces you have driven.
//!
//! **The number a driver asks for after the delta.** "Your best is 2:09.7;
//! your best pieces add up to 2:08.4" says there is a second and a third
//! sitting in laps already driven, and — this is the part that matters —
//! *which* pieces, and *which lap* to go and watch to see one done properly.
//!
//! The core has had a theoretical best since long before this: three sectors,
//! summed. Three is what a timing screen gives and it is far too coarse to act
//! on — a whole sector is a third of a circuit, so "you lost time in sector 2"
//! sends a driver to seven corners. This cuts the lap into thirty pieces
//! instead, which is roughly a corner and its exit, and names the lap each
//! best came from.
//!
//! # Mini-sectors rather than corners
//!
//! Corners would be the obvious unit and are the wrong one here. A corner is
//! detected from what the car did, so two laps can find a different number of
//! them, and matching corner to corner across five laps is the problem
//! [`crate::corners`] warns about at its top. Fixed slices of distance are the
//! same slices on every lap by construction, which is exactly what a sum of
//! bests needs — and it is what every timing system in the sport does for the
//! same reason.
//!
//! # It is not a lap anybody drove
//!
//! Said out loud wherever it is drawn. A sum of bests ignores that the fastest
//! way through one corner may cost the next, and a driver who reads it as a
//! target rather than as a ceiling will go looking for a lap that does not
//! exist.

use crate::analyzer::LapData;
use crate::confidence::Evidence;
use crate::corners::time_at;
use crate::engineer::{Chain, Recommendation, Severity};

/// How many pieces the lap is cut into.
///
/// **Thirty, which is about a corner and its exit.** Three is a timing screen
/// and sends a driver to a third of a circuit; a hundred is a graph, and the
/// noise between two samples starts to win. Thirty is the number a driver can
/// read as a list and each piece is a thing they can picture.
pub const SEGMENTS: usize = 30;

/// Fewer laps than this and there is no "best piece" to speak of.
pub const ENOUGH: usize = 2;

/// A piece of road, and the best anybody did through it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// 1-based, so this is the "7" in "piece 7".
    pub number: usize,
    pub from: f32,
    pub to: f32,
    /// The quickest anybody was through here, in milliseconds.
    pub best_ms: i32,
    /// Which lap that was, so somebody can go and watch it.
    pub from_lap: i32,
    /// What the best *lap* did through the same piece.
    pub on_the_best_lap_ms: i32,
}

impl Segment {
    /// What this piece would give back, in milliseconds. Never negative: on
    /// the pieces the best lap already owns, it is nought.
    pub fn on_the_table_ms(&self) -> i32 {
        (self.on_the_best_lap_ms - self.best_ms).max(0)
    }

    /// Where round the lap it is, as a driver reads it.
    pub fn where_it_is(&self) -> String {
        format!("{:.0}\u{2013}{:.0} %", self.from * 100.0, self.to * 100.0)
    }
}

/// The lap made of the best pieces.
#[derive(Debug, Clone, PartialEq)]
pub struct Ideal {
    pub segments: Vec<Segment>,
    /// The sum of the bests, in milliseconds.
    pub total_ms: i32,
    /// The quickest lap actually driven, and which it was.
    pub best_lap_ms: i32,
    pub best_lap_number: i32,
    /// How many laps went into it.
    pub laps: usize,
}

impl Ideal {
    /// How much is sitting in laps already driven, in milliseconds.
    pub fn on_the_table_ms(&self) -> i32 {
        (self.best_lap_ms - self.total_ms).max(0)
    }

    /// The pieces with the most in them, worst first.
    pub fn worst(&self, how_many: usize) -> Vec<&Segment> {
        let mut found: Vec<&Segment> = self
            .segments
            .iter()
            .filter(|piece| piece.on_the_table_ms() > 0)
            .collect();
        found.sort_by_key(|piece| std::cmp::Reverse(piece.on_the_table_ms()));
        found.truncate(how_many);
        found
    }

    /// How many different laps the best pieces came from.
    ///
    /// **One is a driver who put it together and the rest is noise; five is a
    /// driver whose pace is there and whose consistency is not.** That is a
    /// different problem with a different answer, and the count is the only
    /// thing that separates them.
    pub fn spread_over(&self) -> usize {
        let mut laps: Vec<i32> = self.segments.iter().map(|piece| piece.from_lap).collect();
        laps.sort_unstable();
        laps.dedup();
        laps.len()
    }

    /// The lap time as a driver reads it.
    pub fn as_a_time(&self) -> String {
        clock(self.total_ms)
    }

    /// The finding, if there is enough on the table to be worth one.
    pub fn advice(&self) -> Vec<Recommendation> {
        let on_the_table = self.on_the_table_ms();
        // Under a tenth is the sampling, not the driving.
        if on_the_table < 100 {
            return Vec::new();
        }

        let worst = self.worst(3);
        let mut evidence = Evidence::new();
        for piece in &worst {
            evidence.observe(piece.on_the_table_ms() as f32 / 1000.0);
        }
        let places = worst
            .iter()
            .map(|piece| format!("{} (lap {})", piece.where_it_is(), piece.from_lap))
            .collect::<Vec<_>>()
            .join(", ");

        vec![Recommendation {
            component: "PACE".to_string(),
            category: "Driving".to_string(),
            severity: match on_the_table >= 500 {
                true => Severity::Warning,
                false => Severity::Info,
            },
            message: format!(
                "{:.2} s of your best lap is sitting in laps you have already driven.",
                on_the_table as f32 / 1000.0
            ),
            action: match self.spread_over() {
                1 => "It is almost all in one lap — you put it together once and have not \
                      repeated it. Watch that lap back and drive it again."
                    .to_string(),
                many if many >= self.segments.len() / 2 => format!(
                    "The best pieces come from {many} different laps, so the pace is there and \
                     the consistency is not. Stop chasing a quicker lap and drive the same one \
                     twice."
                ),
                _ => format!("Start with {places} — that is where most of it is."),
            },
            parameters: Vec::new(),
            confidence: 0.0,
            chain: Some(Chain {
                cause: "no single lap strung together the best you have already done".to_string(),
                effect: format!(
                    "{} against your best of {}, over {} laps. It is a ceiling and not a lap \
                     anybody drove — the quickest way through one corner can cost the next.",
                    clock(self.total_ms),
                    clock(self.best_lap_ms),
                    self.laps
                ),
                confirm: format!("whether the next lap takes any of {places}"),
                evidence,
            }),
        }]
    }
}

/// `1:29.482`, which is how a lap time is read.
fn clock(ms: i32) -> String {
    if ms <= 0 {
        return "\u{2014}".to_string();
    }
    let seconds = ms as f32 / 1000.0;
    format!(
        "{}:{:06.3}",
        (seconds / 60.0).floor() as i32,
        seconds % 60.0
    )
}

/// The best pieces of every lap, strung together.
///
/// `None` until there are two laps with traces to compare, or where the lap
/// is too short to cut into pieces.
pub fn look(laps: &[LapData]) -> Option<Ideal> {
    let usable: Vec<&LapData> = laps
        .iter()
        .filter(|lap| {
            lap.valid
                && lap.lap_time_ms > 0
                && lap.telemetry_trace.len() > SEGMENTS * 2
                // **And the trace has to reach the end of the lap.** A trace
                // that stops half way is not a lap to take pieces from beyond
                // where it stopped — and if its recorded time happened to be
                // the quickest, it became the best lap and then had no time
                // through the second half, which made the whole answer
                // disappear. Found by the test that was written to check
                // something milder.
                && lap
                    .telemetry_trace
                    .last()
                    .is_some_and(|point| point.distance >= 0.98)
        })
        .collect();
    if usable.len() < ENOUGH {
        return None;
    }

    let best_lap = usable.iter().min_by_key(|lap| lap.lap_time_ms)?;
    let mut segments = Vec::with_capacity(SEGMENTS);

    for piece in 0..SEGMENTS {
        let from = piece as f32 / SEGMENTS as f32;
        let to = (piece + 1) as f32 / SEGMENTS as f32;

        // **Every lap that reaches both ends of the piece.** A lap whose trace
        // stops short cannot contribute a time through a piece it never
        // finished, and letting it would put a shorter number into the sum.
        let through = |lap: &LapData| -> Option<i32> {
            let start = time_at(&lap.telemetry_trace, from)?;
            let end = time_at(&lap.telemetry_trace, to)?;
            (end > start).then_some(end - start)
        };

        let mut quickest: Option<(i32, i32)> = None;
        for lap in &usable {
            if let Some(took) = through(lap)
                && quickest.is_none_or(|(was, _)| took < was)
            {
                quickest = Some((took, lap.lap_number));
            }
        }
        let on_the_best_lap_ms = through(best_lap)?;
        let (best_ms, from_lap) = quickest?;

        segments.push(Segment {
            number: piece + 1,
            from,
            to,
            best_ms,
            from_lap,
            on_the_best_lap_ms,
        });
    }

    // **A sum missing a term is not a lap time**, it is a smaller number that
    // looks like one — the same rule the sector version already keeps.
    if segments.len() != SEGMENTS {
        return None;
    }

    Some(Ideal {
        total_ms: segments.iter().map(|piece| piece.best_ms).sum(),
        best_lap_ms: best_lap.lap_time_ms,
        best_lap_number: best_lap.lap_number,
        laps: usable.len(),
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::TelemetryPoint;

    /// A lap that takes `ms` overall, losing `slow_ms` extra through the piece
    /// starting at `slow_from`.
    fn lap(number: i32, ms: i32, slow_from: Option<f32>, slow_ms: i32) -> LapData {
        let count = 400;
        let trace: Vec<TelemetryPoint> = (0..=count)
            .map(|step| {
                let at = step as f32 / count as f32;
                // The extra time is added once the car is past the slow piece,
                // so every later sample carries it — which is what a real lap
                // does after losing time somewhere.
                let late = match slow_from {
                    Some(from) if at > from + 1.0 / SEGMENTS as f32 => slow_ms,
                    Some(from) if at > from => {
                        (slow_ms as f32 * ((at - from) * SEGMENTS as f32)) as i32
                    }
                    _ => 0,
                };
                TelemetryPoint {
                    distance: at,
                    time_ms: (at * ms as f32) as i32 + late,
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
            .collect();
        LapData {
            lap_number: number,
            lap_time_ms: ms + slow_ms,
            valid: true,
            telemetry_trace: trace,
            ..Default::default()
        }
    }

    /// One lap has nothing to be the best piece of.
    #[test]
    fn one_lap_is_not_an_ideal_lap() {
        assert!(look(&[lap(1, 90_000, None, 0)]).is_none());
    }

    /// **The number a driver asks for**: two laps, each losing time somewhere
    /// the other did not, and the sum of the good halves is quicker than
    /// either.
    #[test]
    fn the_best_pieces_of_two_laps_beat_both_of_them() {
        // One loses half a second early, the other loses it late.
        let early = lap(1, 90_000, Some(0.10), 500);
        let late = lap(2, 90_000, Some(0.70), 500);
        let ideal = look(&[early, late]).expect("two laps");

        assert_eq!(ideal.laps, 2);
        assert_eq!(ideal.segments.len(), SEGMENTS);
        assert!(
            ideal.total_ms < ideal.best_lap_ms,
            "{} against {}",
            ideal.total_ms,
            ideal.best_lap_ms
        );
        // Roughly the half second the best lap gave away in the other's good
        // half. Loosely, because the pieces are cut by distance and the loss
        // is spread over one of them.
        let on_the_table = ideal.on_the_table_ms();
        assert!(
            (400..=600).contains(&on_the_table),
            "half a second is on the table, not {on_the_table}"
        );
        // And it says which laps the pieces came from.
        assert_eq!(ideal.spread_over(), 2);
    }

    /// **It names where and which lap**, or it is one more number a driver can
    /// do nothing with.
    #[test]
    fn it_says_where_the_time_is_and_which_lap_to_watch() {
        let early = lap(1, 90_000, Some(0.10), 800);
        let late = lap(2, 90_000, Some(0.70), 300);
        let ideal = look(&[early, late]).expect("two laps");

        let worst = ideal.worst(3);
        assert!(!worst.is_empty());
        // The biggest piece on the table is where the *best* lap lost most,
        // which is lap 2 at 70 % — so the piece to watch came from lap 1.
        assert_eq!(worst[0].from_lap, 1);
        assert!(
            worst[0].from > 0.65 && worst[0].from < 0.75,
            "{}",
            worst[0].from
        );

        let advice = ideal.advice();
        assert_eq!(advice.len(), 1);
        assert!(
            advice[0].message.contains("0.3") || advice[0].message.contains("0.2"),
            "{}",
            advice[0].message
        );
        // The one thing that must be said, or it is read as a target.
        assert!(
            advice[0]
                .chain
                .as_ref()
                .expect("a chain")
                .effect
                .contains("not a lap anybody drove")
        );
    }

    /// A driver who put one lap together and has nothing else in the others is
    /// a different problem from one whose pace is scattered, and the advice
    /// has to tell them apart.
    #[test]
    fn one_good_lap_and_scattered_pace_are_different_findings() {
        // Everything on lap 1: the others are simply slower everywhere.
        let put_together = lap(1, 89_000, None, 0);
        let worse = lap(2, 90_500, None, 0);
        let ideal = look(&[put_together, worse]).expect("two laps");
        assert_eq!(ideal.spread_over(), 1);
        assert!(ideal.advice().is_empty(), "nothing is on the table");
    }

    /// **A lap whose trace stops half way is not a lap.** It was worse than
    /// that: this one's recorded time was the quickest, so it became the best
    /// lap, and then had no time through the second half — which made the
    /// whole answer disappear rather than merely be wrong.
    #[test]
    fn a_trace_that_stops_short_is_not_a_lap_at_all() {
        let whole = lap(1, 90_000, None, 0);
        let also_whole = lap(3, 90_400, None, 0);
        let mut cut = lap(2, 88_000, None, 0);
        cut.telemetry_trace.retain(|point| point.distance < 0.5);

        let ideal = look(&[whole.clone(), also_whole, cut.clone()]).expect("two whole laps");
        assert_eq!(ideal.laps, 2, "the short one is not counted");
        assert_eq!(ideal.best_lap_number, 1, "and is certainly not the best");
        assert!(
            ideal.segments.iter().all(|piece| piece.from_lap != 2),
            "no piece comes from a lap that stopped"
        );

        // And two laps, one of which is short, is one lap: not enough.
        assert!(look(&[whole, cut]).is_none());
    }

    /// Under a tenth is the sampling, not the driving, and a finding about it
    /// makes every other finding worth less.
    #[test]
    fn a_hundredth_on_the_table_is_not_a_finding() {
        let one = lap(1, 90_000, Some(0.10), 30);
        let two = lap(2, 90_000, Some(0.70), 30);
        assert!(look(&[one, two]).expect("two laps").advice().is_empty());
    }
}
