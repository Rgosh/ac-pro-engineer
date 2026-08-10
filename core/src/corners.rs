//! Corners found in the trace, and where a lap's time actually went.
//!
//! The application had no notion of a corner. It could say "Speed: 183 km/h"
//! and "you lost 0.4 s somewhere", and a driver wanting to know *where* had to
//! read a graph and guess. This turns a lap's telemetry into a numbered list of
//! corners and a per-corner account of the time, which is the thing every other
//! piece of analysis in `docs/plan-0.3.7-analysis.md` is built on.
//!
//! ## Found, not looked up
//!
//! A corner is a stretch where lateral load stays up long enough to be a corner
//! rather than a kink or a bump. That needs no track data at all, so it works on
//! mods and on tracks nobody has written a table for — which is the whole reason
//! it is done this way rather than from a per-track list of corner positions
//! somebody would have to maintain.
//!
//! ## Distance, never index
//!
//! Corners are identified by **where they are**, not by their position in the
//! list. Two laps of the same track can detect a different number of corners —
//! a driver who ran wide and never loaded the car through a fast kink is
//! missing one — and comparing T5 against T5 by index after that compares two
//! different corners and reports a large, invented delta. Everything here
//! matches by distance window, and a corner found in one lap and not the other
//! is *no comparison* rather than a comparison against nothing.
//!
//! Distances are AC's normalised car position: 0.0 at the line, 1.0 at the line
//! again. Metres are available only when the track's length is known, so
//! anything reported in metres takes it as an argument and says so.

use crate::analyzer::TelemetryPoint;
use serde::{Deserialize, Serialize};

/// Lateral load that begins a corner, in g.
///
/// High enough that a straight with camber on it is not a corner, low enough
/// that a fast fifth-gear kink still is. The pair with [`SUSTAIN_G`] is
/// hysteresis: a corner does not end because one sample dipped.
const ENTER_G: f32 = 0.35;

/// Lateral load a corner has to fall below to be over, in g.
const SUSTAIN_G: f32 = 0.20;

/// The shortest stretch treated as a corner, as a fraction of a lap.
///
/// About 20 m on a 5 km track. Below this it is a bump, a kerb, or one noisy
/// sample, and naming it "T4" would push every later corner's number along.
const MIN_LENGTH: f32 = 0.004;

/// Two stretches the same way round and closer together than this are one
/// corner, as a fraction of a lap.
///
/// A long corner that unloads briefly over a crest is one corner to a driver
/// and would otherwise be reported as two. A direction change is never merged:
/// left-then-right is a chicane, which is two corners no matter how tight.
const MERGE_GAP: f32 = 0.004;

/// How far apart two corners' apexes may be and still be the same corner, as a
/// fraction of a lap.
///
/// About 50 m on a 5 km track. Wide enough that a different line through the
/// same corner still matches, tight enough that two corners in a quick sequence
/// do not match each other.
const MATCH_WINDOW: f32 = 0.010;

/// Pedal pressure that counts as being on the brakes.
const BRAKE_ON: f32 = 0.05;

/// Throttle that counts as being back on the power, rather than feeding it in
/// to balance the car.
const THROTTLE_ON: f32 = 0.50;

/// How far back from a corner's entry to look for the braking point, as a
/// fraction of a lap. About 250 m on a 5 km track, which is longer than any
/// braking zone in a road car.
const BRAKE_LOOKBACK: f32 = 0.05;

/// Which way a corner goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Left,
    Right,
}

impl Direction {
    /// A single character for a list that has no room for a word.
    pub fn arrow(self) -> &'static str {
        match self {
            Direction::Left => "←",
            Direction::Right => "→",
        }
    }
}

/// One corner of one lap, and what the car did through it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Corner {
    /// 1-based and in distance order, so this is the "7" in "T7". Only
    /// meaningful within the lap it was detected in — see the note on matching
    /// at the top of this module.
    pub number: usize,
    pub direction: Direction,

    /// Normalised distance where lateral load came up.
    pub entry: f32,
    /// Normalised distance of the slowest point, which is the apex as far as
    /// the timing is concerned.
    pub apex: f32,
    /// Normalised distance where lateral load fell away again.
    pub exit: f32,

    pub entry_speed: f32,
    pub min_speed: f32,
    pub exit_speed: f32,
    pub peak_lat_g: f32,

    /// Where the driver first touched the brakes for this corner, if they did.
    /// `None` on a corner taken flat.
    pub brake_point: Option<f32>,
    /// Where they got back to real throttle, if they did before the corner
    /// ended.
    pub throttle_point: Option<f32>,
    /// How long after the apex that was, in milliseconds.
    ///
    /// Measured here rather than worked out later from the trace, so a corner
    /// is self-contained: comparing two laps' throttle timing then needs the
    /// two corners and not the two traces they came from.
    pub throttle_delay_ms: Option<i32>,

    /// Lap time in milliseconds at entry and at exit, so a section's duration
    /// is a subtraction.
    pub entry_time_ms: i32,
    pub exit_time_ms: i32,
}

impl Corner {
    /// How long the car was in the corner, in milliseconds.
    pub fn duration_ms(&self) -> i32 {
        (self.exit_time_ms - self.entry_time_ms).max(0)
    }

    /// How long after the apex the driver got back on the throttle, in seconds.
    ///
    /// `None` when they never did within the corner, which is itself the
    /// finding — there is no number to compare, and inventing one would read as
    /// "on the power immediately".
    pub fn throttle_delay_s(&self) -> Option<f32> {
        self.throttle_delay_ms.map(|delay| delay as f32 / 1000.0)
    }

    /// The name a driver reads: `T7`.
    pub fn label(&self) -> String {
        format!("T{}", self.number)
    }
}

/// Interpolated lap time at a normalised distance, in milliseconds.
///
/// The trace is recorded in distance order, so this is a binary search and two
/// multiplications rather than a scan.
pub fn time_at(trace: &[TelemetryPoint], distance: f32) -> Option<i32> {
    interpolate(trace, distance, |point| point.time_ms as f32).map(|value| value as i32)
}

/// Interpolated anything at a normalised distance.
fn interpolate<F>(trace: &[TelemetryPoint], distance: f32, get: F) -> Option<f32>
where
    F: Fn(&TelemetryPoint) -> f32,
{
    if trace.is_empty() {
        return None;
    }
    let first = trace.first()?;
    let last = trace.last()?;
    if distance <= first.distance {
        return Some(get(first));
    }
    if distance >= last.distance {
        return Some(get(last));
    }

    // The sample at or after `distance`. `partition_point` needs the trace
    // sorted by distance, which is how the analyser records it.
    let index = trace.partition_point(|point| point.distance < distance);
    let after = trace.get(index)?;
    let before = trace.get(index.saturating_sub(1))?;

    let span = after.distance - before.distance;
    if span <= f32::EPSILON {
        return Some(get(after));
    }
    let factor = ((distance - before.distance) / span).clamp(0.0, 1.0);
    Some(get(before) + factor * (get(after) - get(before)))
}

/// A run of samples that is one corner, before it is measured.
struct Stretch {
    direction: Direction,
    start: usize,
    end: usize,
}

/// Find the corners in one lap's trace, in distance order.
///
/// Returns nothing rather than guessing when the trace is too short to hold a
/// corner — an out-lap cut short, or a lap the recorder only caught the end of.
pub fn detect(trace: &[TelemetryPoint]) -> Vec<Corner> {
    let mut stretches: Vec<Stretch> = Vec::new();
    let mut current: Option<Stretch> = None;

    for (index, point) in trace.iter().enumerate() {
        let magnitude = point.lat_g.abs();
        let direction = if point.lat_g < 0.0 {
            Direction::Left
        } else {
            Direction::Right
        };

        match current.as_mut() {
            // In a corner: it continues while the load stays up and stays the
            // same way round.
            Some(open) if magnitude >= SUSTAIN_G && open.direction == direction => {
                open.end = index;
            }
            Some(_) => {
                // Whatever ended it, the corner is over. A direction change
                // starts the next one on the same sample rather than waiting
                // for the load to build again, which is what makes a chicane
                // two corners that meet.
                if let Some(open) = current.take() {
                    stretches.push(open);
                }
                if magnitude >= ENTER_G {
                    current = Some(Stretch {
                        direction,
                        start: index,
                        end: index,
                    });
                }
            }
            None if magnitude >= ENTER_G => {
                current = Some(Stretch {
                    direction,
                    start: index,
                    end: index,
                });
            }
            None => {}
        }
    }
    if let Some(open) = current.take() {
        stretches.push(open);
    }

    let stretches = merge_and_filter(&stretches, trace);

    stretches
        .iter()
        .enumerate()
        .filter_map(|(index, stretch)| measure(stretch, trace, index + 1))
        .collect()
}

/// Join stretches that are one corner, and drop what is too short to be one.
///
/// Merging happens before the length filter on purpose: a long corner broken
/// into three brief stretches by a crest would otherwise have all three
/// discarded and the corner would vanish entirely.
fn merge_and_filter(stretches: &[Stretch], trace: &[TelemetryPoint]) -> Vec<Stretch> {
    let distance_of = |index: usize| trace.get(index).map(|point| point.distance).unwrap_or(0.0);

    let mut merged: Vec<Stretch> = Vec::new();
    for stretch in stretches {
        let joined = merged.last_mut().is_some_and(|previous| {
            previous.direction == stretch.direction
                && distance_of(stretch.start) - distance_of(previous.end) < MERGE_GAP
        });
        if joined {
            if let Some(previous) = merged.last_mut() {
                previous.end = stretch.end;
            }
        } else {
            merged.push(Stretch {
                direction: stretch.direction,
                start: stretch.start,
                end: stretch.end,
            });
        }
    }

    merged.retain(|stretch| distance_of(stretch.end) - distance_of(stretch.start) >= MIN_LENGTH);
    merged
}

/// Measure one stretch into a [`Corner`].
fn measure(stretch: &Stretch, trace: &[TelemetryPoint], number: usize) -> Option<Corner> {
    let samples = trace.get(stretch.start..=stretch.end)?;
    let first = samples.first()?;
    let last = samples.last()?;

    // The apex is the slowest point, not the tightest: it is the timing that
    // this is for, and the slowest point is where the lap time is decided.
    let slowest = samples.iter().min_by(|a, b| {
        a.speed
            .partial_cmp(&b.speed)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    let peak_lat_g = samples
        .iter()
        .map(|point| point.lat_g.abs())
        .fold(0.0f32, f32::max);

    let throttle = throttle_point(trace, slowest.distance, last.distance);
    let throttle_delay_ms =
        throttle.and_then(|point| Some(time_at(trace, point)? - time_at(trace, slowest.distance)?));

    Some(Corner {
        number,
        direction: stretch.direction,
        entry: first.distance,
        apex: slowest.distance,
        exit: last.distance,
        entry_speed: first.speed,
        min_speed: slowest.speed,
        exit_speed: last.speed,
        peak_lat_g,
        brake_point: brake_point(trace, stretch.start, slowest.distance),
        throttle_point: throttle,
        throttle_delay_ms,
        entry_time_ms: first.time_ms,
        exit_time_ms: last.time_ms,
    })
}

/// Where the braking for this corner started.
///
/// Looked for behind the corner as well as inside it, because braking for a
/// corner happens on the straight before it — which is exactly the number a
/// driver wants when they are asking whether they braked too early.
fn brake_point(trace: &[TelemetryPoint], entry_index: usize, apex: f32) -> Option<f32> {
    let entry_distance = trace.get(entry_index)?.distance;
    let earliest = entry_distance - BRAKE_LOOKBACK;

    // The *last* braking application before the apex, not the first: a driver
    // who brushed the brakes 200 m earlier and then lifted has not started
    // braking for this corner there. Tracked as runs rather than by clearing on
    // release, because on most corners the driver comes off the brakes exactly
    // at the point this is trying to measure — clearing there threw away the
    // only answer and reported a braked corner as taken flat.
    let mut open: Option<f32> = None;
    let mut last_finished: Option<f32> = None;

    for point in trace.iter() {
        if point.distance < earliest {
            continue;
        }
        if point.distance > apex {
            break;
        }
        if point.brake > BRAKE_ON {
            if open.is_none() {
                open = Some(point.distance);
            }
        } else if let Some(start) = open.take() {
            last_finished = Some(start);
        }
    }

    // Still on the brakes at the apex is trail braking, and that run is the
    // one; otherwise the most recent finished one.
    open.or(last_finished)
}

/// Where the driver got back to real throttle after the apex.
fn throttle_point(trace: &[TelemetryPoint], apex: f32, exit: f32) -> Option<f32> {
    trace
        .iter()
        .find(|point| point.distance >= apex && point.distance <= exit && point.gas >= THROTTLE_ON)
        .map(|point| point.distance)
}

/// Two corners believed to be the same corner of the same track.
#[derive(Debug, Clone)]
pub struct CornerComparison {
    pub corner: Corner,
    /// The same corner in the reference lap. `None` when the reference has no
    /// corner at this distance, which is not a delta of zero and not a large
    /// delta — it is no answer, and is drawn as one.
    pub reference: Option<Corner>,
    /// Time lost or gained in this corner's section, in milliseconds. Positive
    /// is slower than the reference.
    pub delta_ms: i32,
}

impl CornerComparison {
    /// How much later the brakes came on than in the reference, in metres.
    ///
    /// Positive is later — deeper into the corner. `None` when either lap was
    /// flat through here, or the track's length is not known, since "14 m
    /// later" needs a track to be metres of.
    pub fn braking_delta_m(&self, track_length_m: f32) -> Option<f32> {
        if track_length_m <= 0.0 {
            return None;
        }
        let mine = self.corner.brake_point?;
        let theirs = self.reference.as_ref()?.brake_point?;
        Some((mine - theirs) * track_length_m)
    }

    /// How much later the driver got back on the throttle than in the
    /// reference, in seconds. Positive is later.
    ///
    /// `None` unless both laps got to real throttle inside the corner: a lap
    /// that never did has no delay to be later than, and calling that "0.00 s"
    /// would read as the two being identical.
    pub fn throttle_delta_s(&self) -> Option<f32> {
        let mine = self.corner.throttle_delay_s()?;
        let theirs = self.reference.as_ref()?.throttle_delay_s()?;
        Some(mine - theirs)
    }

    /// Entry, minimum and exit speed against the reference, in km/h.
    pub fn speed_deltas(&self) -> Option<(f32, f32, f32)> {
        let reference = self.reference.as_ref()?;
        Some((
            self.corner.entry_speed - reference.entry_speed,
            self.corner.min_speed - reference.min_speed,
            self.corner.exit_speed - reference.exit_speed,
        ))
    }
}

/// Where a lap's time went, corner by corner.
#[derive(Debug, Clone, Default)]
pub struct Decomposition {
    pub sections: Vec<CornerComparison>,
    /// The run from the line to the first corner, which belongs to no corner
    /// and is still time. Positive is slower.
    pub opening_ms: i32,
    /// The whole lap's delta, in milliseconds. The sections and the opening sum
    /// to this by construction — see the note in [`decompose`].
    pub total_ms: i32,
}

impl Decomposition {
    /// Only the corners that cost more than `threshold` seconds.
    ///
    /// This filter is the point of the whole decomposition. Twenty corners with
    /// a number beside each is another table to read; three corners that cost a
    /// tenth each is where to go and work. Sorted worst first.
    pub fn losses_over(&self, threshold_s: f32) -> Vec<&CornerComparison> {
        let threshold_ms = (threshold_s * 1000.0) as i32;
        let mut worst: Vec<&CornerComparison> = self
            .sections
            .iter()
            .filter(|section| section.delta_ms > threshold_ms)
            .collect();
        // Negated rather than reversed, so corners costing the same time keep
        // the order they are driven in.
        worst.sort_by_key(|section| -section.delta_ms);
        worst
    }
}

/// Match this lap's corners to a reference lap's and say where the time went.
///
/// Each corner owns the track from its own entry to the next corner's entry, so
/// the straight after a corner is charged to the corner that led onto it —
/// which is where a bad exit is actually paid for. Sections therefore tile the
/// lap, and their deltas plus the opening add up to the lap's own delta rather
/// than to some other number the reader has to reconcile.
pub fn decompose(
    lap: &[TelemetryPoint],
    reference: &[TelemetryPoint],
    corners: &[Corner],
    reference_corners: &[Corner],
) -> Decomposition {
    if lap.is_empty() || reference.is_empty() {
        return Decomposition::default();
    }

    // Time lost by a given distance: how far behind the reference this lap is
    // at that point on the track.
    let behind_at = |distance: f32| -> i32 {
        let mine = time_at(lap, distance).unwrap_or(0);
        let theirs = time_at(reference, distance).unwrap_or(0);
        mine - theirs
    };

    let mut sections = Vec::with_capacity(corners.len());
    for (index, corner) in corners.iter().enumerate() {
        // To the next corner's entry, or to the line.
        let section_end = corners
            .get(index + 1)
            .map(|next| next.entry)
            .unwrap_or(f32::MAX);

        sections.push(CornerComparison {
            reference: match_corner(corner, reference_corners),
            delta_ms: behind_at(section_end) - behind_at(corner.entry),
            corner: corner.clone(),
        });
    }

    let opening_ms = corners
        .first()
        .map(|first| behind_at(first.entry))
        .unwrap_or(0);

    Decomposition {
        total_ms: behind_at(f32::MAX),
        sections,
        opening_ms,
    }
}

/// The reference lap's corner at the same place on the track, if it has one.
///
/// Nearest apex within [`MATCH_WINDOW`], and the directions have to agree: two
/// corners that close together going opposite ways are a chicane, and matching
/// its left to its right would report the difference between two different
/// corners as a driver's mistake.
fn match_corner(corner: &Corner, candidates: &[Corner]) -> Option<Corner> {
    candidates
        .iter()
        .filter(|candidate| candidate.direction == corner.direction)
        .filter(|candidate| (candidate.apex - corner.apex).abs() <= MATCH_WINDOW)
        .min_by(|a, b| {
            (a.apex - corner.apex)
                .abs()
                .partial_cmp(&(b.apex - corner.apex).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trace with one corner in it: straight, sustained lateral load, straight.
    fn trace_with_corners(corners: &[(f32, f32, f32)]) -> Vec<TelemetryPoint> {
        // 1000 samples over the lap, 100 ms apart — a 100-second lap.
        (0..1000)
            .map(|index| {
                let distance = index as f32 / 1000.0;
                let mut lat_g = 0.0;
                let mut speed = 200.0;
                let mut brake = 0.0;
                let mut gas = 1.0;
                for (start, end, g) in corners {
                    if distance >= *start && distance <= *end {
                        lat_g = *g;
                        speed = 100.0;
                        gas = 0.0;
                    }
                    // Braking for 2% of the lap before each corner.
                    if distance >= start - 0.02 && distance < *start {
                        brake = 1.0;
                        gas = 0.0;
                        speed = 150.0;
                    }
                }
                TelemetryPoint {
                    distance,
                    time_ms: index * 100,
                    speed,
                    gas,
                    brake,
                    gear: 4,
                    steer: 0.0,
                    lat_g,
                    lon_g: 0.0,
                    slip_avg: 0.0,
                    x: 0.0,
                    y: 0.0,
                    rpms: 7000,
                }
            })
            .collect()
    }

    #[test]
    fn a_straight_has_no_corners() {
        let trace = trace_with_corners(&[]);
        assert!(detect(&trace).is_empty());
    }

    #[test]
    fn one_sustained_load_is_one_corner() {
        let trace = trace_with_corners(&[(0.20, 0.25, 1.2)]);
        let corners = detect(&trace);

        assert_eq!(corners.len(), 1, "{corners:?}");
        let corner = &corners[0];
        assert_eq!(corner.number, 1);
        assert_eq!(
            corner.direction,
            Direction::Right,
            "positive lat_g is right"
        );
        assert!((corner.entry - 0.20).abs() < 0.01, "{}", corner.entry);
        assert!((corner.min_speed - 100.0).abs() < 1.0);
    }

    /// The trap the module is written around: a kerb or a bump is not a corner,
    /// and calling one "T4" pushes the number of every corner after it.
    #[test]
    fn a_brief_flick_is_not_a_corner() {
        // Two samples of load — 0.2% of the lap, under MIN_LENGTH.
        let trace = trace_with_corners(&[(0.20, 0.201, 1.2)]);
        assert!(detect(&trace).is_empty(), "{:?}", detect(&trace));
    }

    /// Left then right is a chicane: two corners, however close together.
    #[test]
    fn a_direction_change_is_never_merged() {
        let trace = trace_with_corners(&[(0.20, 0.25, 1.2), (0.2505, 0.30, -1.2)]);
        let corners = detect(&trace);

        assert_eq!(corners.len(), 2, "{corners:?}");
        assert_eq!(corners[0].direction, Direction::Right);
        assert_eq!(corners[1].direction, Direction::Left);
        assert_eq!(corners[1].number, 2, "numbered in distance order");
    }

    /// A long corner that unloads over a crest is still one corner.
    #[test]
    fn a_momentary_unload_does_not_split_a_corner() {
        let mut trace = trace_with_corners(&[(0.20, 0.30, 1.2)]);
        // One sample in the middle with no load at all, as a crest would give.
        if let Some(point) = trace.get_mut(250) {
            point.lat_g = 0.0;
        }

        let corners = detect(&trace);
        assert_eq!(
            corners.len(),
            1,
            "one crest is not two corners: {corners:?}"
        );
    }

    #[test]
    fn the_braking_point_is_found_on_the_straight_before() {
        let trace = trace_with_corners(&[(0.30, 0.35, 1.2)]);
        let corners = detect(&trace);

        let brake = corners[0].brake_point.expect("the corner was braked for");
        assert!(
            (brake - 0.28).abs() < 0.005,
            "braking starts 2% of a lap before the corner, found {brake}"
        );
    }

    #[test]
    fn a_corner_taken_flat_has_no_braking_point() {
        // Load, but the synthetic trace only brakes before a corner it knows
        // about — so build one by hand with no brake at all.
        let mut trace = trace_with_corners(&[(0.30, 0.35, 1.2)]);
        for point in trace.iter_mut() {
            point.brake = 0.0;
        }

        let corners = detect(&trace);
        assert_eq!(corners[0].brake_point, None);
    }

    /// The reason everything here is keyed by distance. A reference lap that
    /// detected an extra corner must not shift every later comparison by one.
    #[test]
    fn corners_match_by_distance_and_not_by_index() {
        let mine = detect(&trace_with_corners(&[(0.50, 0.55, 1.2)]));
        // The reference found a corner earlier in the lap that this lap did
        // not — so by index, its T2 is my T1.
        let theirs = detect(&trace_with_corners(&[(0.10, 0.15, 1.2), (0.50, 0.55, 1.2)]));

        assert_eq!(mine.len(), 1);
        assert_eq!(theirs.len(), 2);

        let matched = match_corner(&mine[0], &theirs).expect("same corner, same place");
        assert!(
            (matched.apex - 0.50).abs() < 0.02,
            "matched the corner at the same distance, not the one with the same index: {matched:?}"
        );
    }

    /// Every degenerate trace this can be handed, in one place. All of these
    /// reach code that indexes and slices, and a panic here takes down a
    /// terminal that is drawing sixty times a second — a lap the recorder
    /// caught two samples of is not a reason to lose the session.
    #[test]
    fn nothing_here_panics_on_a_trace_that_is_not_a_lap() {
        let empty: Vec<TelemetryPoint> = Vec::new();
        assert!(detect(&empty).is_empty());
        assert_eq!(time_at(&empty, 0.5), None);

        let full = trace_with_corners(&[(0.20, 0.25, 1.2)]);
        let one = full.get(..1).map(<[_]>::to_vec).unwrap_or_default();
        assert!(detect(&one).is_empty(), "one sample is not a corner");
        assert!(time_at(&one, 0.5).is_some(), "clamped to the only sample");

        // A lap against nothing, and nothing against a lap.
        let corners = detect(&full);
        assert_eq!(
            decompose(&full, &empty, &corners, &[]).sections.len(),
            0,
            "there is no reference to decompose against"
        );
        assert_eq!(decompose(&empty, &full, &[], &corners).total_ms, 0);

        // A corner that runs to the very last sample, so `measure` slices to
        // the end of the trace.
        let to_the_line = trace_with_corners(&[(0.90, 1.10, 1.2)]);
        let found = detect(&to_the_line);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].exit <= 1.0);
    }

    /// AC has published a NaN into the physics page before now. A comparison
    /// against NaN is always false, so it must fall out as "not cornering"
    /// rather than opening a corner that never closes.
    #[test]
    fn a_nan_in_the_trace_is_not_a_corner() {
        let mut trace = trace_with_corners(&[]);
        for point in trace.iter_mut().skip(100).take(50) {
            point.lat_g = f32::NAN;
        }
        assert!(detect(&trace).is_empty(), "{:?}", detect(&trace));
    }

    #[test]
    fn a_corner_the_reference_does_not_have_is_no_comparison() {
        let mine = detect(&trace_with_corners(&[(0.50, 0.55, 1.2)]));
        let theirs = detect(&trace_with_corners(&[(0.10, 0.15, 1.2)]));

        assert_eq!(match_corner(&mine[0], &theirs), None);
    }

    /// A chicane's left must not match its right.
    #[test]
    fn a_matching_corner_has_to_go_the_same_way() {
        let mine = detect(&trace_with_corners(&[(0.20, 0.25, 1.2)]));
        let theirs = detect(&trace_with_corners(&[(0.20, 0.25, -1.2)]));

        assert_eq!(mine.len(), 1);
        assert_eq!(theirs.len(), 1);
        assert_eq!(match_corner(&mine[0], &theirs), None);
    }

    /// The decomposition has to add up, or it is a set of numbers rather than
    /// an account of the lap.
    #[test]
    fn the_sections_and_the_opening_sum_to_the_lap_delta() {
        let mine = trace_with_corners(&[(0.20, 0.25, 1.2), (0.60, 0.65, -1.2)]);
        // The reference is the same lap driven 5 s quicker, spread evenly.
        let theirs: Vec<TelemetryPoint> = mine
            .iter()
            .map(|point| TelemetryPoint {
                time_ms: (point.time_ms as f32 * 0.95) as i32,
                ..point.clone()
            })
            .collect();

        let my_corners = detect(&mine);
        let their_corners = detect(&theirs);
        let decomposition = decompose(&mine, &theirs, &my_corners, &their_corners);

        let summed: i32 = decomposition.opening_ms
            + decomposition
                .sections
                .iter()
                .map(|section| section.delta_ms)
                .sum::<i32>();

        assert_eq!(
            summed, decomposition.total_ms,
            "the parts have to account for the whole: {decomposition:?}"
        );
        assert!(
            decomposition.total_ms > 0,
            "this lap is the slower one: {}",
            decomposition.total_ms
        );
    }

    /// The filter is the feature: what it hides is the value.
    #[test]
    fn only_the_corners_that_cost_real_time_are_reported() {
        let decomposition = Decomposition {
            sections: vec![
                comparison(1, 180),
                comparison(2, 20),
                comparison(3, -140),
                comparison(4, 350),
            ],
            opening_ms: 0,
            total_ms: 410,
        };

        let worst = decomposition.losses_over(0.10);
        assert_eq!(worst.len(), 2, "two corners cost more than a tenth");
        assert_eq!(worst[0].corner.number, 4, "worst first");
        assert_eq!(worst[1].corner.number, 1);
    }

    fn comparison(number: usize, delta_ms: i32) -> CornerComparison {
        CornerComparison {
            corner: Corner {
                number,
                direction: Direction::Left,
                entry: 0.0,
                apex: 0.0,
                exit: 0.0,
                entry_speed: 0.0,
                min_speed: 0.0,
                exit_speed: 0.0,
                peak_lat_g: 0.0,
                brake_point: None,
                throttle_point: None,
                throttle_delay_ms: None,
                entry_time_ms: 0,
                exit_time_ms: 0,
            },
            reference: None,
            delta_ms,
        }
    }

    /// A lap that never got back on the power has no delay to be later than.
    #[test]
    fn a_throttle_delta_needs_both_laps_to_have_one() {
        let mut pair = comparison(1, 0);
        pair.corner.throttle_delay_ms = Some(800);
        pair.reference = Some(pair.corner.clone());

        assert_eq!(pair.throttle_delta_s(), Some(0.0));

        if let Some(reference) = pair.reference.as_mut() {
            reference.throttle_delay_ms = Some(600);
        }
        let delta = pair.throttle_delta_s().expect("both laps got to throttle");
        assert!((delta - 0.2).abs() < 0.001, "{delta}");

        pair.corner.throttle_delay_ms = None;
        assert_eq!(
            pair.throttle_delta_s(),
            None,
            "never on the power is not the same as on it at the same moment"
        );
    }

    #[test]
    fn braking_in_metres_needs_a_track_to_be_metres_of() {
        let mut pair = comparison(1, 0);
        pair.corner.brake_point = Some(0.300);
        pair.reference = Some(pair.corner.clone());
        if let Some(reference) = pair.reference.as_mut() {
            reference.brake_point = Some(0.297);
        }

        // 5000 m track, 0.003 of a lap later on the brakes: 15 m.
        let metres = pair.braking_delta_m(5000.0).expect("both laps braked");
        assert!((metres - 15.0).abs() < 0.1, "{metres}");

        assert_eq!(
            pair.braking_delta_m(0.0),
            None,
            "an unknown track length is not a delta of zero"
        );
    }

    #[test]
    fn time_at_interpolates_between_samples() {
        let trace = trace_with_corners(&[]);
        // Halfway between sample 10 (1000 ms) and sample 11 (1100 ms).
        let time = time_at(&trace, 0.0105).expect("inside the trace");
        assert!((time - 1050).abs() <= 1, "{time}");
    }
}
