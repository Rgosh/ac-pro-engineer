//! Drifting: the angle, the score, and what to work on.
//!
//! **Everything else in this program assumes the car is meant to grip.** The
//! engineer counts oversteer as a fault, the corner table charges a slide as
//! lost time, and a tyre at 120 °C is overheating. For somebody drifting every
//! one of those is the point, and an engineer that says "too much oversteer"
//! to them is an engineer they uninstall. So drifting gets its own measure:
//! how sideways, how fast, for how long, linked how well — and the engineer
//! keeps quiet about grip while it is happening (see [`Style`]).
//!
//! # The angle
//!
//! The angle between where the car points and where it is going, from the
//! velocity in the car's own axes that both games publish: sideways over
//! forwards. Below walking pace that ratio is noise, so it is no angle at all.
//!
//! # The score
//!
//! A drift begins at [`START_DEG`] and [`MIN_SPEED_KMH`], keeps going above
//! [`KEEP_DEG`], and is banked once the car has been straight for
//! [`SETTLE_MS`] — long enough that the instant of zero angle in a
//! transition, left to right, does not end it. While it lasts it earns points
//! for angle and for speed, and every transition raises the combo by
//! [`COMBO_STEP`] up to [`COMBO_MAX`]: a long linked run is worth more than
//! the same seconds spent in single corners, which is how every judged drift
//! format thinks.
//!
//! A spin past [`SPIN_DEG`], a hit, or four wheels off the road ends a drift
//! **with nothing banked**. That is the rule people drift by on every server,
//! and a score that kept a drift ended in the wall would teach the opposite.
//!
//! # What it decides
//!
//! Nothing about the car's setup, and nothing it cannot measure: no line, no
//! zones, no judges. Angle, speed, time, transitions, spins. That is enough to
//! say the three things a coach says first.

use crate::engineer::{Chain, Recommendation, Severity};
use crate::games::Car;

/// Angle that starts a drift, degrees.
pub const START_DEG: f32 = 15.0;
/// Angle a drift has to stay above, degrees — hysteresis, so one sample at
/// fourteen does not end it.
pub const KEEP_DEG: f32 = 8.0;
/// Angle past which it is a spin, not a drift, degrees.
pub const SPIN_DEG: f32 = 110.0;
/// Below this there is no drift, km/h. A car sliding at walking pace is
/// parking badly.
pub const MIN_SPEED_KMH: f32 = 30.0;
/// How long a drift may be straight before it is over, milliseconds.
pub const SETTLE_MS: u64 = 700;
/// What each transition adds to the combo.
pub const COMBO_STEP: f32 = 0.25;
/// The most a combo can multiply by.
pub const COMBO_MAX: f32 = 3.0;
/// Below this the velocity is too small to have a direction, m/s.
const STILL_MS: f32 = 3.0;
/// How many finished drifts a session keeps.
const KEPT: usize = 64;

/// The drift angle of a car, degrees: positive one way, negative the other,
/// or `None` while it is too slow for its direction to mean anything.
pub fn angle_deg(local_velocity_ms: [f32; 3]) -> Option<f32> {
    let [sideways, _, forwards] = local_velocity_ms;
    let speed = sideways.hypot(forwards);
    (speed >= STILL_MS).then(|| sideways.atan2(forwards).to_degrees())
}

/// How a drift ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// Straightened up: banked.
    Banked,
    /// Slowed below [`MIN_SPEED_KMH`] while still sideways: banked.
    Slowed,
    /// Went round: lost.
    Spun,
    /// Hit something: lost.
    Contact,
    /// Four wheels off the road: lost.
    OffTrack,
}

impl Ending {
    /// Whether the points were kept.
    pub fn banked(self) -> bool {
        matches!(self, Ending::Banked | Ending::Slowed)
    }

    /// As a driver says it.
    pub fn label(self) -> &'static str {
        match self {
            Ending::Banked => "banked",
            Ending::Slowed => "banked, slowed",
            Ending::Spun => "spun",
            Ending::Contact => "contact",
            Ending::OffTrack => "off track",
        }
    }
}

/// One drift, finished.
#[derive(Debug, Clone, PartialEq)]
pub struct Drift {
    /// How long it lasted, milliseconds.
    pub duration_ms: u64,
    /// Mean and largest angle, degrees, as magnitudes.
    pub mean_angle: f32,
    pub max_angle: f32,
    /// How much the angle wandered inside it — the standard deviation of its
    /// magnitude, degrees. A held angle is a small number.
    pub angle_spread: f32,
    /// Mean speed, km/h.
    pub mean_speed: f32,
    /// Changes of direction while sideways.
    pub transitions: u32,
    /// What it earned before it ended, whether or not that was kept.
    pub earned: u32,
    /// What was kept: [`Self::earned`] if it was banked, nothing otherwise.
    pub score: u32,
    pub ending: Ending,
}

/// A drift in progress.
#[derive(Debug, Clone, Default)]
struct Open {
    started_ms: u64,
    last_ms: u64,
    straight_since: Option<u64>,
    side: f32,
    transitions: u32,
    points: f32,
    samples: u32,
    angle_sum: f32,
    angle_sq_sum: f32,
    max_angle: f32,
    speed_sum: f32,
    damage_at_start: f32,
    off_since: Option<u64>,
}

impl Open {
    fn combo(&self) -> f32 {
        (1.0 + self.transitions as f32 * COMBO_STEP).min(COMBO_MAX)
    }

    fn close(&self, ending: Ending, at_ms: u64) -> Drift {
        let n = self.samples.max(1) as f32;
        let mean = self.angle_sum / n;
        let variance = (self.angle_sq_sum / n - mean * mean).max(0.0);
        let earned = self.points.round() as u32;
        Drift {
            duration_ms: at_ms.saturating_sub(self.started_ms),
            mean_angle: mean,
            max_angle: self.max_angle,
            angle_spread: variance.sqrt(),
            mean_speed: self.speed_sum / n,
            transitions: self.transitions,
            earned,
            score: if ending.banked() { earned } else { 0 },
            ending,
        }
    }
}

/// Drifts as they happen, one reading at a time.
///
/// Fed every reading with the time it arrived; hands back each drift as it
/// ends. `damage_counts` is the game's own answer to whether it publishes
/// damage — without it a hit cannot be told from a car that was never hit.
#[derive(Debug, Clone, Default)]
pub struct Tracker {
    open: Option<Open>,
    /// The session so far.
    pub session: Session,
}

impl Tracker {
    /// The drift in progress, as `(seconds, points so far, combo)`.
    pub fn current(&self) -> Option<(f32, u32, f32)> {
        self.open.as_ref().map(|open| {
            (
                open.last_ms.saturating_sub(open.started_ms) as f32 / 1000.0,
                open.points.round() as u32,
                open.combo(),
            )
        })
    }

    /// One reading. Returns the drift that just ended, if one did.
    pub fn observe(&mut self, at_ms: u64, car: &Car, damage_counts: bool) -> Option<Drift> {
        let angle = angle_deg(car.local_velocity_ms);
        let speed = car.speed_kmh;
        let damage: f32 = car.damage.iter().sum();
        let magnitude = angle.map(f32::abs).unwrap_or(0.0);

        let Some(open) = self.open.as_mut() else {
            if (START_DEG..SPIN_DEG).contains(&magnitude) && speed >= MIN_SPEED_KMH {
                self.open = Some(Open {
                    started_ms: at_ms,
                    last_ms: at_ms,
                    side: angle.map(f32::signum).unwrap_or(1.0),
                    damage_at_start: damage,
                    ..Default::default()
                });
            }
            return None;
        };

        let dt = at_ms.saturating_sub(open.last_ms).min(250) as f32 / 1000.0;
        open.last_ms = at_ms;

        // The endings that lose everything, first.
        let lost = if magnitude >= SPIN_DEG {
            Some(Ending::Spun)
        } else if damage_counts && damage > open.damage_at_start + 0.5 {
            Some(Ending::Contact)
        } else if car.tyres_off_track >= 4 {
            let since = *open.off_since.get_or_insert(at_ms);
            (at_ms.saturating_sub(since) >= 500).then_some(Ending::OffTrack)
        } else {
            open.off_since = None;
            None
        };
        if let Some(ending) = lost {
            return self.finish(ending, at_ms);
        }
        if speed < MIN_SPEED_KMH * 0.66 {
            return self.finish(Ending::Slowed, at_ms);
        }

        if magnitude >= KEEP_DEG {
            open.straight_since = None;
            let side = angle.map(f32::signum).unwrap_or(open.side);
            if side != open.side && magnitude >= START_DEG {
                open.side = side;
                open.transitions += 1;
            }
            let angle_points = (magnitude.clamp(START_DEG, 70.0) - 10.0) / 20.0;
            let speed_points = (speed / 60.0).clamp(0.5, 2.5);
            open.points += dt * angle_points * speed_points * open.combo() * 100.0;
            open.samples += 1;
            open.angle_sum += magnitude;
            open.angle_sq_sum += magnitude * magnitude;
            open.max_angle = open.max_angle.max(magnitude);
            open.speed_sum += speed;
            None
        } else {
            let since = *open.straight_since.get_or_insert(at_ms);
            match at_ms.saturating_sub(since) >= SETTLE_MS {
                true => self.finish(Ending::Banked, since),
                false => None,
            }
        }
    }

    fn finish(&mut self, ending: Ending, at_ms: u64) -> Option<Drift> {
        let open = self.open.take()?;
        // A flick that never got going is not a drift worth a line.
        if open.samples < 5 && ending.banked() {
            return None;
        }
        let drift = open.close(ending, at_ms);
        self.session.add(drift.clone());
        Some(drift)
    }
}

/// A session of drifting, counted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    /// The latest drifts, oldest first.
    pub drifts: Vec<Drift>,
    /// Every drift counted, including those no longer kept.
    pub count: u32,
    pub best: u32,
    pub total: u32,
    pub spins: u32,
}

impl Session {
    fn add(&mut self, drift: Drift) {
        self.count += 1;
        self.best = self.best.max(drift.score);
        self.total += drift.score;
        if drift.ending == Ending::Spun {
            self.spins += 1;
        }
        self.drifts.push(drift);
        if self.drifts.len() > KEPT {
            self.drifts.remove(0);
        }
    }

    /// The three things a coach says first, worst first — or nothing until
    /// there are [`ENOUGH`] drifts to say them about.
    pub fn advice(&self) -> Vec<Recommendation> {
        let recent: Vec<&Drift> = self.drifts.iter().rev().take(20).collect();
        if recent.len() < ENOUGH {
            return Vec::new();
        }
        let mut said = Vec::new();
        let spun = recent.iter().filter(|d| d.ending == Ending::Spun).count();
        if spun * 10 >= recent.len() * 3 {
            said.push(finding(
                Severity::Warning,
                format!(
                    "{spun} of your last {} drifts ended in a spin.",
                    recent.len()
                ),
                "Less throttle once the car is sideways: catch the angle with \
                 the wheel first, then feed the power in."
                    .to_string(),
                "the rear is being asked for more than it has at full angle",
                format!("{spun} spins in {} drifts", recent.len()),
            ));
        }
        let kept: Vec<&&Drift> = recent.iter().filter(|d| d.ending.banked()).collect();
        if kept.len() >= ENOUGH {
            let mean = kept.iter().map(|d| d.mean_angle).sum::<f32>() / kept.len() as f32;
            let wander = kept.iter().map(|d| d.angle_spread).sum::<f32>() / kept.len() as f32;
            if mean < 25.0 {
                said.push(finding(
                    Severity::Info,
                    format!("Shallow drifts: {mean:.0}° on average."),
                    "Commit to the entry — a sharper flick or a clutch kick — \
                     and hold more throttle through the middle."
                        .to_string(),
                    "the car is sliding, but not far enough to score",
                    format!("mean angle {mean:.0}° over {} drifts", kept.len()),
                ));
            }
            if wander > 12.0 {
                said.push(finding(
                    Severity::Info,
                    format!("The angle wanders ±{wander:.0}° inside a drift."),
                    "Hold the angle with the throttle, not the wheel: smaller, \
                     steadier pedal changes."
                        .to_string(),
                    "the angle is being chased rather than held",
                    format!("angle spread {wander:.0}° over {} drifts", kept.len()),
                ));
            }
        }
        said
    }

    /// One line for the top of a screen.
    pub fn headline(&self) -> String {
        format!(
            "best {} · total {} · {} drifts · {} spins",
            self.best, self.total, self.count, self.spins
        )
    }
}

/// Drifts it takes before anything is said about them.
pub const ENOUGH: usize = 5;

fn finding(
    severity: Severity,
    message: String,
    action: String,
    cause: &str,
    effect: String,
) -> Recommendation {
    Recommendation {
        component: "DRIFT".to_string(),
        category: "Drift".to_string(),
        severity,
        message,
        action,
        parameters: Vec::new(),
        confidence: 0.7,
        chain: Some(Chain {
            cause: cause.to_string(),
            effect,
            confirm: "the same figure over your next five drifts".to_string(),
            evidence: Default::default(),
        }),
    }
}

/// Whether the driver is drifting, as opposed to driving a car that slides.
///
/// **What keeps the engineer quiet about grip.** A drift session says so
/// itself; a drift *server* is usually a practice session like any other, so
/// this also watches the driving: the share of the last twenty seconds or so
/// spent sideways at speed. A racing driver's slides are moments; a drifter's
/// are most of the lap.
#[derive(Debug, Clone, Copy, Default)]
pub struct Style {
    share: f32,
}

impl Style {
    /// One reading, `dt_s` after the last.
    pub fn observe(&mut self, car: &Car, dt_s: f32) {
        let sideways = car.speed_kmh >= MIN_SPEED_KMH
            && angle_deg(car.local_velocity_ms).is_some_and(|a| a.abs() >= START_DEG);
        // A time constant of twenty seconds, whatever the rate.
        let weight = (dt_s / 20.0).clamp(0.0, 1.0);
        self.share += weight * (f32::from(u8::from(sideways)) - self.share);
    }

    /// Whether that share says drifting.
    pub fn drifting(&self) -> bool {
        self.share >= 0.25
    }

    /// The share itself, 0..1.
    pub fn share(&self) -> f32 {
        self.share
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A car at `speed` km/h, `angle` degrees sideways.
    fn car(speed: f32, angle: f32) -> Car {
        let ms = speed / 3.6;
        let radians = angle.to_radians();
        Car {
            speed_kmh: speed,
            local_velocity_ms: [ms * radians.sin(), 0.0, ms * radians.cos()],
            ..Default::default()
        }
    }

    /// Feed `(seconds, speed, angle)` at 60 Hz and collect what ended.
    fn drive(tracker: &mut Tracker, script: &[(f32, f32, f32)]) -> Vec<Drift> {
        let mut at = 0u64;
        let mut ended = Vec::new();
        for &(seconds, speed, angle) in script {
            let ticks = (seconds * 60.0) as u64;
            for _ in 0..ticks {
                at += 16;
                if let Some(drift) = tracker.observe(at, &car(speed, angle), true) {
                    ended.push(drift);
                }
            }
        }
        ended
    }

    #[test]
    fn the_angle_is_sideways_over_forwards() {
        assert!((angle_deg([10.0, 0.0, 10.0]).expect("fast enough") - 45.0).abs() < 0.01);
        assert!((angle_deg([-10.0, 0.0, 10.0]).expect("fast enough") + 45.0).abs() < 0.01);
        assert!(angle_deg([0.0, 0.0, 20.0]).expect("fast enough").abs() < 0.01);
        assert_eq!(
            angle_deg([0.5, 0.0, 1.0]),
            None,
            "too slow to have a direction"
        );
    }

    /// A held drift, straightened: banked, with its angle and speed.
    #[test]
    fn a_drift_straightened_is_banked() {
        let mut tracker = Tracker::default();
        let ended = drive(
            &mut tracker,
            &[(1.0, 80.0, 0.0), (3.0, 80.0, 35.0), (2.0, 80.0, 0.0)],
        );
        assert_eq!(ended.len(), 1);
        let drift = &ended[0];
        assert_eq!(drift.ending, Ending::Banked);
        assert!(drift.score > 0 && drift.score == drift.earned);
        assert!((drift.mean_angle - 35.0).abs() < 0.5, "{drift:?}");
        assert!(drift.angle_spread < 0.5);
        assert!((2800..=3200).contains(&drift.duration_ms), "{drift:?}");
        assert_eq!(tracker.session.best, drift.score);
    }

    /// **A spin keeps nothing.** That is the rule on every drift server.
    #[test]
    fn a_spin_loses_the_drift() {
        let mut tracker = Tracker::default();
        let ended = drive(&mut tracker, &[(2.0, 70.0, 40.0), (0.2, 60.0, 130.0)]);
        assert_eq!(ended.len(), 1);
        assert_eq!(ended[0].ending, Ending::Spun);
        assert_eq!(ended[0].score, 0);
        assert!(ended[0].earned > 0);
        assert_eq!(tracker.session.spins, 1);
    }

    /// Left to right is one drift, worth more than two.
    #[test]
    fn a_transition_links_and_raises_the_combo() {
        let mut linked = Tracker::default();
        let one = drive(
            &mut linked,
            &[
                (2.0, 80.0, 35.0),
                (0.2, 80.0, 0.0),
                (2.0, 80.0, -35.0),
                (2.0, 80.0, 0.0),
            ],
        );
        assert_eq!(one.len(), 1, "one drift through the transition");
        assert_eq!(one[0].transitions, 1);

        let mut single = Tracker::default();
        let two = drive(&mut single, &[(2.0, 80.0, 35.0), (2.0, 80.0, 0.0)]);
        assert!(
            one[0].score > two[0].score * 2,
            "{} against {}",
            one[0].score,
            two[0].score
        );
    }

    /// A hit, where the game counts hits, keeps nothing either.
    #[test]
    fn contact_loses_the_drift() {
        let mut tracker = Tracker::default();
        drive(&mut tracker, &[(1.0, 70.0, 30.0)]);
        let mut hit = car(70.0, 30.0);
        hit.damage = [2.0, 0.0, 0.0, 0.0, 0.0];
        let drift = tracker.observe(100_000, &hit, true).expect("ended");
        assert_eq!(drift.ending, Ending::Contact);
        assert_eq!(drift.score, 0);
    }

    /// A car crawling sideways is not drifting.
    #[test]
    fn nothing_below_drifting_speed() {
        let mut tracker = Tracker::default();
        assert!(drive(&mut tracker, &[(3.0, 20.0, 40.0), (2.0, 20.0, 0.0)]).is_empty());
    }

    /// Spins and shallow angles are what a coach says, and only once there is
    /// enough driving to say it about.
    #[test]
    fn the_coach_speaks_after_five_drifts() {
        let mut tracker = Tracker::default();
        drive(&mut tracker, &[(2.0, 80.0, 18.0), (1.0, 80.0, 0.0)]);
        assert!(
            tracker.session.advice().is_empty(),
            "one drift is not a habit"
        );
        for _ in 0..5 {
            drive(&mut tracker, &[(2.0, 80.0, 18.0), (1.0, 80.0, 0.0)]);
        }
        let said = tracker.session.advice();
        assert!(
            said.iter().any(|one| one.message.starts_with("Shallow")),
            "{said:?}"
        );
    }

    /// The engineer's switch: a lap mostly sideways is drifting, a slide now
    /// and then is not.
    #[test]
    fn drifting_is_most_of_the_time_sideways() {
        let mut style = Style::default();
        for step in 0..1200 {
            let angle = if step % 10 == 0 { 25.0 } else { 2.0 };
            style.observe(&car(120.0, angle), 1.0 / 60.0);
        }
        assert!(!style.drifting(), "a racer's slides: {}", style.share());
        for _ in 0..1200 {
            style.observe(&car(70.0, 35.0), 1.0 / 60.0);
        }
        assert!(style.drifting(), "{}", style.share());
    }
}
