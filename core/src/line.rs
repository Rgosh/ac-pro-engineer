//! Where the car was across the road, against the reference lap — in metres.
//!
//! **Every tool draws two racing lines on a map; none of them measures the
//! gap.** A driver looking at two lines a pixel apart on a 4 km circuit cannot
//! tell a metre from five, and the difference between those is the difference
//! between a missed apex and a different line. [`crate::corners`] says how
//! much time a corner cost and [`crate::braking`] what the pedal did; neither
//! can say "you were 1.8 m off the apex", which is the one sentence that
//! explains a corner where the braking was identical and the time was not.
//!
//! # How the gap is measured
//!
//! At the same normalised distance both cars are at the same cross-section of
//! the circuit — that is what the game's spline position means — so the gap
//! between the two positions is almost all *across* the road. What little is
//! along it is removed by measuring against the reference's own direction of
//! travel there: the gap is the cross product of the reference's heading with
//! the vector between the cars.
//!
//! # Inside, not left
//!
//! Left and right depend on which way round the game's axes are, and the two
//! simulators disagree. **Inside** does not: it is the side the reference
//! path turns toward, found from the same cross product of the path's
//! direction before and after the apex. A positive gap is toward the inside of
//! that corner, whatever either game calls its axes.
//!
//! # What is refused
//!
//! A lap saved before positions were recorded carries zeros where they would
//! be, and every gap would read as the distance to the origin. Two positions
//! further apart than [`TOO_FAR_M`] are not two lines through one corner but
//! a pit lane, a shortcut or another layout. Both are *no measurement*, never
//! a number.

use crate::analyzer::{LapData, TelemetryPoint};
use crate::corners::{Corner, detect, time_at};
use crate::i18n::tr_fmt;

/// A difference in line smaller than this is the same line, in metres.
///
/// A metre is about a car's half-width — inside it a driver is on the same
/// line driven a little differently, and naming it would be noise.
pub const SAME_LINE_M: f32 = 1.0;

/// Two cars further apart than this at one cross-section are not on one road,
/// in metres.
pub const TOO_FAR_M: f32 = 25.0;

/// How far either side of a point the heading is taken over, as a fraction of
/// a lap. About ten metres on a 5 km circuit: long enough that one noisy
/// position does not swing it, short enough to follow a hairpin.
const HEADING_SPAN: f32 = 0.002;

/// A corner has to have cost at least this, in milliseconds, before its line
/// is offered as the reason.
const WORTH_SAYING_MS: i32 = 80;

/// What the line did through one corner, against the reference.
#[derive(Clone, Debug, PartialEq)]
pub struct CornerLine {
    /// `T5`, numbered on the reference lap.
    pub label: String,
    /// The reference corner's apex, normalised.
    pub apex: f32,
    /// Metres toward the inside at turn-in, the apex and the exit. Positive is
    /// inside the reference line; `None` where it could not be measured.
    pub entry_m: Option<f32>,
    pub apex_m: Option<f32>,
    pub exit_m: Option<f32>,
    /// Minimum speed against the reference, km/h. Positive is quicker.
    pub min_speed_delta: f32,
    /// Speed at the reference's exit, against the reference, km/h.
    pub exit_speed_delta: f32,
    /// Time lost from this corner's entry to the next one's, ms. Positive is
    /// slower, the convention every delta in this program keeps.
    pub delta_ms: i32,
}

/// The shape of what the line did differently, in a driver's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// The same line, within [`SAME_LINE_M`] everywhere it was measured.
    Same,
    /// Further out than the reference at the apex.
    MissedApex,
    /// In earlier and out wider: turned in before the reference did and ran
    /// out of road on the exit — the early apex.
    EarlyApex,
    /// Wide on the way in and tight on the way out: a late, pointed line.
    LateApex,
    /// Different, in a way none of the above names.
    Different,
}

impl CornerLine {
    /// What the line did, from the three gaps.
    pub fn shape(&self) -> Shape {
        let off = |gap: Option<f32>| gap.is_some_and(|m| m.abs() >= SAME_LINE_M);
        if !off(self.entry_m) && !off(self.apex_m) && !off(self.exit_m) {
            return Shape::Same;
        }
        let entry = self.entry_m.unwrap_or(0.0);
        let exit = self.exit_m.unwrap_or(0.0);
        match self.apex_m {
            Some(apex) if apex <= -SAME_LINE_M => Shape::MissedApex,
            _ if entry >= SAME_LINE_M && exit <= -SAME_LINE_M => Shape::EarlyApex,
            _ if entry <= -SAME_LINE_M && exit >= -SAME_LINE_M / 2.0 => Shape::LateApex,
            _ => Shape::Different,
        }
    }

    /// One sentence, the same in every front end — or `None` for a corner
    /// where the line was the same and something else lost the time.
    pub fn said(&self, ru: bool) -> Option<String> {
        let metres = |gap: Option<f32>| format!("{:.1} m", gap.unwrap_or(0.0).abs());
        let time = format!("{:+.2} s", self.delta_ms as f32 / 1000.0);
        let (template, gap): (&str, String) = match self.shape() {
            Shape::Same => return None,
            Shape::MissedApex => (
                "{0}: {1} wide of your reference at the apex · {2}",
                metres(self.apex_m),
            ),
            Shape::EarlyApex => (
                "{0}: turned in early and ran {1} wider on the exit · {2}",
                metres(self.exit_m),
            ),
            Shape::LateApex => (
                "{0}: {1} wider on the way in, a later apex · {2}",
                metres(self.entry_m),
            ),
            Shape::Different => (
                "{0}: a different line, up to {1} off the reference · {2}",
                format!("{:.1} m", self.widest()),
            ),
        };
        Some(tr_fmt(template, ru, &[&self.label, &gap, &time]))
    }

    /// The largest gap measured, in metres, whichever side.
    pub fn widest(&self) -> f32 {
        [self.entry_m, self.apex_m, self.exit_m]
            .into_iter()
            .flatten()
            .map(f32::abs)
            .fold(0.0, f32::max)
    }
}

/// A lap's line against a reference lap's, corner by corner.
#[derive(Clone, Debug, PartialEq)]
pub struct LineComparison {
    /// Every corner of the reference that could be measured, in order.
    pub corners: Vec<CornerLine>,
}

impl LineComparison {
    /// The corners where the line differed **and** the corner cost time, most
    /// expensive first. A different line through a corner that was no slower
    /// is a driver trying something, not a mistake.
    pub fn costly(&self) -> Vec<&CornerLine> {
        let mut found: Vec<&CornerLine> = self
            .corners
            .iter()
            .filter(|corner| corner.shape() != Shape::Same && corner.delta_ms >= WORTH_SAYING_MS)
            .collect();
        found.sort_by_key(|corner| -corner.delta_ms);
        found
    }

    /// The mean absolute gap at the apexes, in metres — how closely this lap
    /// followed the reference overall. `None` with nothing measured.
    pub fn mean_apex_gap_m(&self) -> Option<f32> {
        let gaps: Vec<f32> = self
            .corners
            .iter()
            .filter_map(|corner| corner.apex_m.map(f32::abs))
            .collect();
        (!gaps.is_empty()).then(|| gaps.iter().sum::<f32>() / gaps.len() as f32)
    }
}

/// Compare a lap's line with a reference lap's.
///
/// `None` when either lap carries no positions — laps saved before they were
/// recorded — or the two are not the same circuit.
pub fn compare(lap: &LapData, reference: &LapData) -> Option<LineComparison> {
    if lap.track_name != reference.track_name {
        return None;
    }
    compare_traces(&lap.telemetry_trace, &reference.telemetry_trace)
}

/// The same, for two traces a caller already knows are of one circuit — a
/// front end holding the lap being driven, or a reference it has decimated.
pub fn compare_traces(
    lap: &[TelemetryPoint],
    reference: &[TelemetryPoint],
) -> Option<LineComparison> {
    if !has_positions(lap) || !has_positions(reference) {
        return None;
    }
    let corners = detect(reference);
    let mut measured = Vec::with_capacity(corners.len());
    for (index, corner) in corners.iter().enumerate() {
        let section_end = corners.get(index + 1).map(|next| next.entry).unwrap_or(1.0);
        measured.push(measure(lap, reference, corner, section_end));
    }
    Some(LineComparison { corners: measured })
}

/// One corner of the reference, and where the lap went through it.
fn measure(
    mine: &[TelemetryPoint],
    theirs: &[TelemetryPoint],
    corner: &Corner,
    section_end: f32,
) -> CornerLine {
    let inside = turning(theirs, corner.entry, corner.exit);
    let gap =
        |distance: f32| inside.and_then(|sign| gap_m(mine, theirs, distance).map(|m| m * sign));
    let speed = |trace: &[TelemetryPoint], distance: f32| sample(trace, distance, |p| p.speed);

    let lost = |trace: &[TelemetryPoint]| -> Option<i32> {
        Some(time_at(trace, section_end.min(0.9999))? - time_at(trace, corner.entry)?)
    };
    CornerLine {
        label: corner.label(),
        apex: corner.apex,
        entry_m: gap(corner.entry),
        apex_m: gap(corner.apex),
        exit_m: gap(corner.exit),
        min_speed_delta: lowest(mine, corner.entry, corner.exit) - corner.min_speed,
        exit_speed_delta: speed(mine, corner.exit).unwrap_or(0.0)
            - speed(theirs, corner.exit).unwrap_or(0.0),
        delta_ms: match (lost(mine), lost(theirs)) {
            (Some(a), Some(b)) => a - b,
            _ => 0,
        },
    }
}

/// Whether a trace recorded where the car was, rather than zeros.
fn has_positions(trace: &[TelemetryPoint]) -> bool {
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for point in trace {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    // A circuit is more than a hundred metres across in some direction.
    trace.len() > 10 && (max_x - min_x).max(max_y - min_y) > 100.0
}

/// Linear interpolation of anything in a trace at a normalised distance.
///
/// `None` outside the stretch the trace holds — clamping would put the car at
/// the end of its own lap and call that a line.
fn sample(
    trace: &[TelemetryPoint],
    distance: f32,
    value: impl Fn(&TelemetryPoint) -> f32,
) -> Option<f32> {
    let first = trace.first()?;
    let last = trace.last()?;
    if distance < first.distance || distance > last.distance {
        return None;
    }
    let index = trace.partition_point(|point| point.distance < distance);
    let after = trace.get(index)?;
    let before = trace.get(index.saturating_sub(1))?;
    let span = after.distance - before.distance;
    if span <= f32::EPSILON {
        return Some(value(after));
    }
    let factor = ((distance - before.distance) / span).clamp(0.0, 1.0);
    Some(value(before) + factor * (value(after) - value(before)))
}

/// Where the car was at a distance, in metres.
fn position(trace: &[TelemetryPoint], distance: f32) -> Option<(f32, f32)> {
    Some((
        sample(trace, distance, |p| p.x)?,
        sample(trace, distance, |p| p.y)?,
    ))
}

/// The direction of travel at a distance, as a unit vector.
fn heading(trace: &[TelemetryPoint], distance: f32) -> Option<(f32, f32)> {
    let (ax, ay) = position(trace, (distance - HEADING_SPAN).max(0.0))?;
    let (bx, by) = position(trace, (distance + HEADING_SPAN).min(1.0))?;
    let (dx, dy) = (bx - ax, by - ay);
    let length = (dx * dx + dy * dy).sqrt();
    (length > 0.1).then(|| (dx / length, dy / length))
}

/// Which way the reference turns between two places: `+1` for the side the
/// cross product calls positive, `-1` for the other, `None` for a path that
/// did not turn enough to have an inside.
fn turning(trace: &[TelemetryPoint], from: f32, to: f32) -> Option<f32> {
    let (ax, ay) = heading(trace, from)?;
    let (bx, by) = heading(trace, to)?;
    let cross = ax * by - ay * bx;
    // About three degrees: less than that and there is no inside to speak of.
    (cross.abs() > 0.05).then_some(cross.signum())
}

/// The gap across the road from the reference to the lap at a distance, in
/// metres, signed by the cross product with the reference's heading.
fn gap_m(lap: &[TelemetryPoint], reference: &[TelemetryPoint], distance: f32) -> Option<f32> {
    let (mx, my) = position(lap, distance)?;
    let (rx, ry) = position(reference, distance)?;
    let (hx, hy) = heading(reference, distance)?;
    let (dx, dy) = (mx - rx, my - ry);
    if (dx * dx + dy * dy).sqrt() > TOO_FAR_M {
        return None;
    }
    Some(hx * dy - hy * dx)
}

/// The slowest the lap went between two places, km/h.
fn lowest(trace: &[TelemetryPoint], from: f32, to: f32) -> f32 {
    trace
        .iter()
        .filter(|point| point.distance >= from && point.distance <= to)
        .map(|point| point.speed)
        .fold(f32::MAX, f32::min)
        .min(sample(trace, from, |p| p.speed).unwrap_or(f32::MAX))
        .clamp(0.0, 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A circuit that is a circle of radius 300 m, driven anticlockwise, with
    /// lateral load from 0.30 to 0.45 of the lap and `offset(distance)` metres
    /// toward the centre — which is the inside of every corner on it.
    fn lap(number: i32, offset: impl Fn(f32) -> f32, slow_ms: i32) -> LapData {
        let mut clock = 0;
        let trace: Vec<TelemetryPoint> = (0..=1000)
            .map(|step| {
                let at = step as f32 / 1000.0;
                let corner = (0.30..0.45).contains(&at);
                if step > 0 {
                    clock += 90 + if corner { slow_ms / 150 } else { 0 };
                }
                let angle = at * std::f32::consts::TAU;
                let radius = 300.0 - offset(at);
                TelemetryPoint {
                    distance: at,
                    time_ms: clock,
                    speed: if corner { 110.0 } else { 200.0 },
                    gas: 1.0,
                    brake: 0.0,
                    gear: 4,
                    steer: 0.0,
                    lat_g: if corner { 1.3 } else { 0.0 },
                    lon_g: 0.0,
                    slip_avg: 0.0,
                    x: radius * angle.cos(),
                    y: radius * angle.sin(),
                    rpms: 7_000,
                    detail: Default::default(),
                }
            })
            .collect();
        LapData {
            lap_number: number,
            lap_time_ms: clock,
            valid: true,
            track_name: "a_circle".to_string(),
            telemetry_trace: trace,
            ..Default::default()
        }
    }

    /// **The sign is the whole feature.** Two metres toward the centre of a
    /// circle is two metres inside, and two metres away from it is outside.
    #[test]
    fn inside_is_toward_the_centre_of_the_turn() {
        let reference = lap(1, |_| 0.0, 0);
        let inside = lap(2, |_| 2.0, 0);
        let outside = lap(3, |_| -2.0, 400);

        let line = compare(&inside, &reference).expect("positions");
        let corner = &line.corners[0];
        assert!(
            (corner.apex_m.expect("measured") - 2.0).abs() < 0.1,
            "{corner:?}"
        );

        let line = compare(&outside, &reference).expect("positions");
        let corner = &line.corners[0];
        assert!(
            (corner.apex_m.expect("measured") + 2.0).abs() < 0.1,
            "{corner:?}"
        );
        assert_eq!(corner.shape(), Shape::MissedApex);
        assert!(corner.delta_ms > 250, "{corner:?}");
        assert_eq!(line.costly().len(), 1);
        let said = corner.said(false).expect("a different line");
        assert!(said.starts_with("T1: 2.0 m wide"), "{said}");
    }

    /// The same circle driven the other way round has its inside on the other
    /// side of the axes, and the answer must not change.
    #[test]
    fn the_direction_of_travel_does_not_change_inside() {
        let mirror = |lap: LapData| LapData {
            telemetry_trace: lap
                .telemetry_trace
                .into_iter()
                .map(|point| TelemetryPoint {
                    y: -point.y,
                    ..point
                })
                .collect(),
            ..lap
        };
        let reference = mirror(lap(1, |_| 0.0, 0));
        let inside = mirror(lap(2, |_| 1.5, 0));
        let line = compare(&inside, &reference).expect("positions");
        assert!((line.corners[0].apex_m.expect("measured") - 1.5).abs() < 0.1);
    }

    /// In early, out wide.
    #[test]
    fn an_early_apex_is_named() {
        let reference = lap(1, |_| 0.0, 0);
        let early = lap(
            2,
            |at| match at {
                at if at < 0.33 => 2.0,
                at if at < 0.40 => 0.0,
                _ => -2.0,
            },
            200,
        );
        let line = compare(&early, &reference).expect("positions");
        assert_eq!(
            line.corners[0].shape(),
            Shape::EarlyApex,
            "{:?}",
            line.corners[0]
        );
    }

    /// Within a metre is the same line, and says nothing.
    #[test]
    fn the_same_line_says_nothing() {
        let reference = lap(1, |_| 0.0, 0);
        let close = lap(2, |_| 0.4, 300);
        let line = compare(&close, &reference).expect("positions");
        assert_eq!(line.corners[0].shape(), Shape::Same);
        assert_eq!(line.corners[0].said(false), None);
        assert!(line.costly().is_empty(), "the time went somewhere else");
        assert!(line.mean_apex_gap_m().expect("measured") < 0.5);
    }

    /// A lap saved before positions were recorded is no measurement.
    #[test]
    fn no_positions_no_line() {
        let reference = lap(1, |_| 0.0, 0);
        let mut old = lap(2, |_| 0.0, 0);
        for point in old.telemetry_trace.iter_mut() {
            point.x = 0.0;
            point.y = 0.0;
        }
        assert_eq!(compare(&old, &reference), None);
        assert_eq!(compare(&reference, &old), None);
    }

    /// Fifty metres apart is not a line, it is another road.
    #[test]
    fn a_pit_lane_is_not_a_line() {
        let reference = lap(1, |_| 0.0, 0);
        let pits = lap(2, |_| 50.0, 0);
        let line = compare(&pits, &reference).expect("positions");
        assert!(line.corners.iter().all(|corner| corner.apex_m.is_none()));
        assert_eq!(line.mean_apex_gap_m(), None);
    }

    /// Another circuit is no comparison at all.
    #[test]
    fn another_circuit_is_refused() {
        let reference = lap(1, |_| 0.0, 0);
        let mut elsewhere = lap(2, |_| 0.0, 0);
        elsewhere.track_name = "somewhere_else".to_string();
        assert_eq!(compare(&elsewhere, &reference), None);
    }
}
