//! Speed runs through traffic — "шашки", "cut up", the No Hesi kind of evening.
//!
//! **A large share of Assetto Corsa's online is not racing at all.** Traffic
//! servers on Shutoko and on long highways fill every evening with drivers
//! weaving through AI traffic as fast as they dare, and nothing in this
//! program had a word to say to them: no laps, no corners, no reference. What
//! they care about is the run — how fast, how long, how clean — and that much
//! the game publishes.
//!
//! # A run
//!
//! It begins when the car passes [`START_KMH`] and lasts until it has been
//! below [`STOP_KMH`] for [`STOP_MS`], or until it touches something. While it
//! lasts it earns points for speed — squared, because twice as fast through
//! the same traffic is far more than twice as hard — times a multiplier that
//! climbs by [`MULTIPLIER_STEP`] for every [`MULTIPLIER_EVERY_MS`] spent clean
//! above [`FAST_KMH`], up to [`MULTIPLIER_MAX`].
//!
//! **A hit ends the run and keeps nothing**, the rule those servers score by.
//! A hit is the game's damage figure rising where the game counts damage, and
//! otherwise a jolt no braking or cornering makes — [`JOLT_G`] in one
//! reading.
//!
//! # What it cannot know
//!
//! Assetto Corsa publishes nothing about the other cars, so there are no near
//! misses here and no overtakes counted: those would be guesses, and a score
//! built on guesses rewards whatever the guess got wrong. Speed, distance,
//! time and contact are measured, and that is what this scores.

use crate::games::Car;

/// Speed that starts a run, km/h.
pub const START_KMH: f32 = 100.0;
/// Below this for [`STOP_MS`] and the run is over, km/h.
pub const STOP_KMH: f32 = 50.0;
/// How long below [`STOP_KMH`] ends a run, milliseconds — long enough to
/// squeeze between two trucks without the run ending under you.
pub const STOP_MS: u64 = 3_000;
/// Above this the multiplier climbs, km/h.
pub const FAST_KMH: f32 = 140.0;
/// Clean time above [`FAST_KMH`] per multiplier step, milliseconds.
pub const MULTIPLIER_EVERY_MS: u64 = 15_000;
/// What each step adds.
pub const MULTIPLIER_STEP: f32 = 0.5;
/// The most the multiplier reaches.
pub const MULTIPLIER_MAX: f32 = 5.0;
/// A change in acceleration in one reading that only a hit makes, g.
pub const JOLT_G: f32 = 3.0;
/// How many finished runs a session keeps.
const KEPT: usize = 64;

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// Slowed down and stayed slow: banked.
    Slowed,
    /// Touched something: lost.
    Contact,
}

/// One run, finished.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub duration_ms: u64,
    /// Kilometres covered.
    pub distance_km: f32,
    pub top_speed: f32,
    pub mean_speed: f32,
    /// Time spent above 200 km/h, milliseconds.
    pub over_200_ms: u64,
    /// The highest multiplier reached.
    pub multiplier: f32,
    /// What it earned, kept or not.
    pub earned: u32,
    /// What was kept.
    pub score: u32,
    pub ending: Ending,
}

#[derive(Debug, Clone, Default)]
struct Open {
    started_ms: u64,
    last_ms: u64,
    slow_since: Option<u64>,
    fast_ms: u64,
    points: f32,
    metres: f32,
    top: f32,
    speed_sum: f32,
    samples: u32,
    over_200_ms: u64,
    multiplier: f32,
    damage_at_start: f32,
    last_g: Option<[f32; 3]>,
}

impl Open {
    fn close(&self, ending: Ending, at_ms: u64) -> Run {
        let earned = self.points.round() as u32;
        Run {
            duration_ms: at_ms.saturating_sub(self.started_ms),
            distance_km: self.metres / 1000.0,
            top_speed: self.top,
            mean_speed: self.speed_sum / self.samples.max(1) as f32,
            over_200_ms: self.over_200_ms,
            multiplier: self.multiplier,
            earned,
            score: match ending {
                Ending::Slowed => earned,
                Ending::Contact => 0,
            },
            ending,
        }
    }
}

/// Runs as they happen, one reading at a time.
#[derive(Debug, Clone, Default)]
pub struct Tracker {
    open: Option<Open>,
    pub session: Session,
}

impl Tracker {
    /// The run in progress, as `(seconds, points so far, multiplier)`.
    pub fn current(&self) -> Option<(f32, u32, f32)> {
        self.open.as_ref().map(|open| {
            (
                open.last_ms.saturating_sub(open.started_ms) as f32 / 1000.0,
                open.points.round() as u32,
                open.multiplier,
            )
        })
    }

    /// One reading. Returns the run that just ended, if one did.
    pub fn observe(&mut self, at_ms: u64, car: &Car, damage_counts: bool) -> Option<Run> {
        let speed = car.speed_kmh;
        let damage: f32 = car.damage.iter().sum();
        let Some(open) = self.open.as_mut() else {
            if speed >= START_KMH {
                self.open = Some(Open {
                    started_ms: at_ms,
                    last_ms: at_ms,
                    multiplier: 1.0,
                    damage_at_start: damage,
                    last_g: Some(car.acc_g),
                    ..Default::default()
                });
            }
            return None;
        };

        let dt_ms = at_ms.saturating_sub(open.last_ms).min(250);
        open.last_ms = at_ms;

        // A jolt: the change in horizontal acceleration since the last
        // reading. Braking and cornering build up over a tenth of a second;
        // a hit arrives in one reading.
        let jolt = open
            .last_g
            .is_some_and(|last| (car.acc_g[0] - last[0]).hypot(car.acc_g[2] - last[2]) >= JOLT_G);
        open.last_g = Some(car.acc_g);
        let hit = (damage_counts && damage > open.damage_at_start + 0.5) || jolt;
        if hit {
            return self.finish(Ending::Contact, at_ms);
        }

        if speed < STOP_KMH {
            let since = *open.slow_since.get_or_insert(at_ms);
            if at_ms.saturating_sub(since) >= STOP_MS {
                return self.finish(Ending::Slowed, since);
            }
        } else {
            open.slow_since = None;
        }

        let dt = dt_ms as f32 / 1000.0;
        if speed >= FAST_KMH {
            open.fast_ms += dt_ms;
            let steps = (open.fast_ms / MULTIPLIER_EVERY_MS) as f32;
            open.multiplier = (1.0 + steps * MULTIPLIER_STEP).min(MULTIPLIER_MAX);
        }
        if speed >= 200.0 {
            open.over_200_ms += dt_ms;
        }
        open.points += dt * (speed / 100.0).powi(2) * open.multiplier * 10.0;
        open.metres += speed / 3.6 * dt;
        open.top = open.top.max(speed);
        open.speed_sum += speed;
        open.samples += 1;
        None
    }

    fn finish(&mut self, ending: Ending, at_ms: u64) -> Option<Run> {
        let open = self.open.take()?;
        let run = open.close(ending, at_ms);
        // A burst past a hundred and back is not a run.
        if run.duration_ms < 5_000 && ending == Ending::Slowed {
            return None;
        }
        self.session.add(run.clone());
        Some(run)
    }
}

/// A session of runs, counted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    pub runs: Vec<Run>,
    pub count: u32,
    pub best: u32,
    /// The longest clean run, kilometres.
    pub longest_clean_km: f32,
    pub top_speed: f32,
    pub contacts: u32,
}

impl Session {
    fn add(&mut self, run: Run) {
        self.count += 1;
        self.best = self.best.max(run.score);
        self.top_speed = self.top_speed.max(run.top_speed);
        match run.ending {
            Ending::Contact => self.contacts += 1,
            Ending::Slowed => self.longest_clean_km = self.longest_clean_km.max(run.distance_km),
        }
        self.runs.push(run);
        if self.runs.len() > KEPT {
            self.runs.remove(0);
        }
    }

    /// One line for the top of a screen.
    pub fn headline(&self) -> String {
        format!(
            "best {} · longest clean {:.1} km · top {:.0} km/h · {} runs, {} ended in contact",
            self.best, self.longest_clean_km, self.top_speed, self.count, self.contacts
        )
    }

    /// What the runs say, when there are enough of them: a driver who ends
    /// most runs in a car is pushing past what the traffic allows.
    pub fn advice(&self) -> Option<String> {
        let recent: Vec<&Run> = self.runs.iter().rev().take(10).collect();
        if recent.len() < 5 {
            return None;
        }
        let hits = recent
            .iter()
            .filter(|r| r.ending == Ending::Contact)
            .count();
        (hits * 2 > recent.len()).then(|| {
            format!(
                "{hits} of your last {} runs ended in contact. The multiplier is worth \
                 more than the last 20 km/h: a run kept clean at 180 outscores one lost at 240.",
                recent.len()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(speed: f32) -> Car {
        Car {
            speed_kmh: speed,
            ..Default::default()
        }
    }

    fn drive(tracker: &mut Tracker, script: &[(f32, f32)]) -> Vec<Run> {
        let mut at = 0u64;
        let mut ended = Vec::new();
        for &(seconds, speed) in script {
            for _ in 0..(seconds * 60.0) as u64 {
                at += 16;
                if let Some(run) = tracker.observe(at, &car(speed), true) {
                    ended.push(run);
                }
            }
        }
        ended
    }

    /// Fast, then slow for long enough: one banked run, measured.
    #[test]
    fn a_run_slowed_down_is_banked() {
        let mut tracker = Tracker::default();
        let ended = drive(&mut tracker, &[(60.0, 216.0), (5.0, 30.0)]);
        assert_eq!(ended.len(), 1);
        let run = &ended[0];
        assert_eq!(run.ending, Ending::Slowed);
        // 3600 readings 16 ms apart: 57.6 s at 60 m/s.
        assert!((run.distance_km - 3.46).abs() < 0.05, "{run:?}");
        assert!((run.top_speed - 216.0).abs() < 0.01);
        assert_eq!(run.multiplier, 2.5, "three clean quarter-minutes above 140");
        assert!(run.over_200_ms > 55_000);
        assert_eq!(run.score, run.earned);
        assert!(tracker.session.longest_clean_km > 3.4);
    }

    /// Squeezing between two trucks does not end it.
    #[test]
    fn a_short_lift_is_still_the_same_run() {
        let mut tracker = Tracker::default();
        let ended = drive(
            &mut tracker,
            &[(20.0, 180.0), (2.0, 40.0), (20.0, 180.0), (5.0, 0.0)],
        );
        assert_eq!(ended.len(), 1);
        assert!(ended[0].duration_ms > 40_000);
    }

    /// **A hit keeps nothing**, and a jolt is a hit where damage is off.
    #[test]
    fn contact_ends_the_run_with_nothing() {
        let mut tracker = Tracker::default();
        drive(&mut tracker, &[(30.0, 200.0)]);
        let mut hit = car(150.0);
        hit.acc_g = [0.0, 1.0, -4.5];
        let run = tracker.observe(10_000_000, &hit, false).expect("ended");
        assert_eq!(run.ending, Ending::Contact);
        assert_eq!(run.score, 0);
        assert!(run.earned > 0);
        assert_eq!(tracker.session.contacts, 1);
    }

    /// Hard braking builds up; it is not a hit.
    #[test]
    fn braking_is_not_a_jolt() {
        let mut tracker = Tracker::default();
        let mut at = 0;
        for step in 0..120 {
            at += 16;
            let mut braking = car(200.0 - step as f32);
            braking.acc_g = [0.0, 1.0, -(step as f32 / 60.0).min(1.6)];
            assert!(tracker.observe(at, &braking, false).is_none());
        }
        assert!(tracker.current().is_some());
    }

    /// Twice as fast is more than twice the points.
    #[test]
    fn speed_counts_squared() {
        let mut slow = Tracker::default();
        let mut fast = Tracker::default();
        let a = drive(&mut slow, &[(10.0, 110.0), (5.0, 0.0)]);
        let b = drive(&mut fast, &[(10.0, 220.0), (5.0, 0.0)]);
        assert!(
            b[0].score > a[0].score * 3,
            "{} against {}",
            b[0].score,
            a[0].score
        );
    }

    #[test]
    fn most_runs_in_a_car_is_said() {
        let mut tracker = Tracker::default();
        for _ in 0..6 {
            drive(&mut tracker, &[(20.0, 200.0)]);
            let mut hit = car(150.0);
            hit.damage = [3.0, 0.0, 0.0, 0.0, 0.0];
            tracker.observe(u64::MAX / 2, &hit, true);
            tracker = Tracker {
                session: tracker.session.clone(),
                ..Default::default()
            };
        }
        assert!(tracker.session.advice().is_some());
    }
}
