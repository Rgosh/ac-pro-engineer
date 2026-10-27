//! Coming back from a break, and what changed while the car stood still.
//!
//! **The engineer judged the first lap after a stop as if it were any other.**
//! A driver who sat in the garage for ten minutes — a setup change, a drink, a
//! phone call — goes out on tyres twenty degrees colder than the ones they came
//! in on, and every rule that reads temperature or balance then speaks about a
//! car that has not warmed up yet: cold tyres, understeer, a lap time that says
//! nothing about the setup that was just changed. None of it is wrong as a
//! measurement. All of it is wrong as advice, and the one thing worth saying —
//! *this lap is a warm-up* — was never said.
//!
//! This watches for that and nothing else: how long the car stood, how much
//! the tyres lost while it did, and when the warm-up is over. It decides
//! nothing about the setup and makes no measurement a game does not publish.
//!
//! # What counts as a break
//!
//! The car stationary **while the simulation runs**. A paused game freezes the
//! physics and the tyres with it, and the menus are not a session at all — so
//! only `Status::Live` time with the car standing is counted. Two minutes of
//! it, because a pit stop under a minute is a pit stop and not a break.

use crate::games::reading::{Car, Session, Status};
use std::time::{Duration, Instant};

/// How long the car has to stand before going out again is a rejoin.
pub const BREAK_AFTER: Duration = Duration::from_secs(120);

/// Slower than this is standing still.
const STANDING_KMH: f32 = 3.0;

/// Faster than this is going out again, rather than rolling in the garage.
const MOVING_KMH: f32 = 10.0;

/// How close to the old temperature counts as warm again.
const WARM_AGAIN_C: f32 = 5.0;

/// The longest a briefing is held, whatever the tyres say.
///
/// A car that never gets back to where it was — rain arrived, the track cooled
/// at dusk — must not carry "warm-up lap" for the rest of the evening.
const HELD_AT_MOST: Duration = Duration::from_secs(240);

/// What the break did, said once the car is moving again.
#[derive(Clone, Debug, PartialEq)]
pub struct Briefing {
    /// How long the car stood.
    pub away: Duration,
    /// The mean tyre temperature when it stopped, °C.
    pub was_c: f32,
    /// The mean tyre temperature when it went out again, °C.
    pub now_c: f32,
    /// Laps completed when it went out again, so one full lap ends it.
    laps_at_rejoin: i32,
    /// When it went out again, so it cannot be held for ever.
    since: Instant,
}

impl Briefing {
    /// How much the tyres lost, °C. Never negative: a car that came back
    /// warmer had nothing to warm up.
    pub fn cooled_c(&self) -> f32 {
        (self.was_c - self.now_c).max(0.0)
    }

    /// Whole minutes stood, for a sentence.
    pub fn minutes(&self) -> u64 {
        (self.away.as_secs() + 30) / 60
    }
}

/// Watches for a break and holds a briefing while the car warms up again.
#[derive(Default)]
pub struct Rejoin {
    /// When the car stopped, while it is stopped.
    stopped_at: Option<Instant>,
    /// How long it has stood, counting only time the simulation ran.
    stood: Duration,
    /// When the last reading was taken, to count the time between them.
    last_seen: Option<Instant>,
    /// The mean tyre temperature the last time the car was moving.
    warm_c: Option<f32>,
    /// The briefing, while it holds.
    briefing: Option<Briefing>,
}

impl Rejoin {
    /// Take a reading in.
    ///
    /// `tyre_c` is the mean tyre temperature by whatever this game measures —
    /// the engineer knows which, and zero is "not measured", never "frozen".
    /// `now` is an argument so the tests can run a ten-minute break in a
    /// millisecond.
    pub fn observe(&mut self, now: Instant, car: &Car, session: &Session, tyre_c: f32) {
        let since_last = self
            .last_seen
            .map(|then| now.saturating_duration_since(then))
            .unwrap_or_default()
            // A gap in readings is the program not running, not the car
            // standing: a reader that stalled for an hour did not watch a
            // car stand for an hour.
            .min(Duration::from_secs(2));
        self.last_seen = Some(now);

        if session.status != Status::Live {
            // Paused or in the menus: nothing cools and nothing is counted.
            return;
        }

        let measured = tyre_c > 0.0;
        if car.speed_kmh < STANDING_KMH {
            if self.stopped_at.is_none() {
                self.stopped_at = Some(now);
                self.stood = Duration::ZERO;
            }
            self.stood += since_last;
            return;
        }

        if car.speed_kmh >= MOVING_KMH {
            if self.stopped_at.take().is_some() {
                if self.stood >= BREAK_AFTER
                    && let Some(was_c) = self.warm_c.filter(|_| measured)
                {
                    self.briefing = Some(Briefing {
                        away: self.stood,
                        was_c,
                        now_c: tyre_c,
                        laps_at_rejoin: session.completed_laps,
                        since: now,
                    });
                }
                self.stood = Duration::ZERO;
            }
            // The temperature to come back to is the one the car had while it
            // was being driven, and only while no briefing is open: during the
            // warm-up it is still climbing towards that number.
            if self.briefing.is_none() && measured {
                self.warm_c = Some(tyre_c);
            }
        }

        // **Over when the car is ready, or a lap has been driven, or it has
        // been long enough.** Whichever comes first.
        let over = self.briefing.as_ref().is_some_and(|briefing| {
            (measured && tyre_c >= briefing.was_c - WARM_AGAIN_C)
                || session.completed_laps > briefing.laps_at_rejoin
                || now.saturating_duration_since(briefing.since) >= HELD_AT_MOST
        });
        if over {
            self.briefing = None;
            if measured {
                self.warm_c = Some(tyre_c);
            }
        }
    }

    /// The briefing, while the car is warming up after a break.
    pub fn briefing(&self) -> Option<&Briefing> {
        self.briefing.as_ref()
    }

    /// Whether this is a warm-up, so other rules can be careful.
    pub fn warming_up(&self) -> bool {
        self.briefing.is_some()
    }

    /// Forget everything — a new session is not a rejoin.
    pub fn forget(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(speed_kmh: f32, laps: i32) -> (Car, Session) {
        let car = Car {
            speed_kmh,
            ..Default::default()
        };
        let session = Session {
            status: Status::Live,
            completed_laps: laps,
            ..Default::default()
        };
        (car, session)
    }

    /// Drive, stand for `stood`, and go out again, a reading every second.
    fn break_of(
        rejoin: &mut Rejoin,
        start: Instant,
        stood: Duration,
        warm: f32,
        cold: f32,
    ) -> Instant {
        let mut now = start;
        let (car, session) = live(150.0, 4);
        for _ in 0..5 {
            rejoin.observe(now, &car, &session, warm);
            now += Duration::from_secs(1);
        }
        let (car, session) = live(0.0, 4);
        let until = now + stood;
        while now < until {
            rejoin.observe(now, &car, &session, cold);
            now += Duration::from_secs(1);
        }
        let (car, session) = live(40.0, 4);
        rejoin.observe(now, &car, &session, cold);
        now
    }

    /// **Ten minutes in the garage is a warm-up lap**, and says how much the
    /// tyres lost.
    #[test]
    fn a_long_stop_is_a_rejoin() {
        let mut rejoin = Rejoin::default();
        break_of(
            &mut rejoin,
            Instant::now(),
            Duration::from_secs(600),
            88.0,
            61.0,
        );
        let briefing = rejoin.briefing().expect("ten minutes stood is a break");
        assert_eq!(briefing.minutes(), 10);
        assert!((briefing.cooled_c() - 27.0).abs() < 0.01);
        assert!(rejoin.warming_up());
    }

    /// A pit stop is not a break.
    #[test]
    fn a_pit_stop_is_not_a_rejoin() {
        let mut rejoin = Rejoin::default();
        break_of(
            &mut rejoin,
            Instant::now(),
            Duration::from_secs(40),
            88.0,
            85.0,
        );
        assert!(rejoin.briefing().is_none());
    }

    /// **A paused game is not a car standing in the cold.** The physics are
    /// frozen, and so are the tyres.
    #[test]
    fn a_pause_is_not_counted() {
        let mut rejoin = Rejoin::default();
        let mut now = Instant::now();
        let (car, session) = live(150.0, 2);
        rejoin.observe(now, &car, &session, 88.0);
        let paused = Session {
            status: Status::Paused,
            ..session
        };
        let still = Car {
            speed_kmh: 0.0,
            ..Default::default()
        };
        for _ in 0..900 {
            now += Duration::from_secs(1);
            rejoin.observe(now, &still, &paused, 88.0);
        }
        now += Duration::from_secs(1);
        rejoin.observe(now, &car, &session, 88.0);
        assert!(rejoin.briefing().is_none());
    }

    /// **Never a verdict on a number nobody measured.** A game that publishes
    /// no tyre temperature has no warm-up to talk about.
    #[test]
    fn nothing_is_said_without_a_temperature() {
        let mut rejoin = Rejoin::default();
        break_of(
            &mut rejoin,
            Instant::now(),
            Duration::from_secs(600),
            0.0,
            0.0,
        );
        assert!(rejoin.briefing().is_none());
    }

    /// Over when the tyres are back, or after a lap, or after four minutes.
    #[test]
    fn the_warm_up_ends() {
        let mut rejoin = Rejoin::default();
        let mut now = break_of(
            &mut rejoin,
            Instant::now(),
            Duration::from_secs(600),
            88.0,
            61.0,
        );
        let (car, session) = live(160.0, 4);
        now += Duration::from_secs(1);
        rejoin.observe(now, &car, &session, 75.0);
        assert!(rejoin.warming_up(), "still well short of where it was");
        now += Duration::from_secs(1);
        rejoin.observe(now, &car, &session, 84.0);
        assert!(!rejoin.warming_up(), "within five degrees is warm again");

        let mut by_lap = Rejoin::default();
        let now = break_of(
            &mut by_lap,
            Instant::now(),
            Duration::from_secs(600),
            88.0,
            61.0,
        );
        let (car, session) = live(160.0, 5);
        by_lap.observe(now + Duration::from_secs(1), &car, &session, 70.0);
        assert!(!by_lap.warming_up(), "a lap driven is a warm-up done");

        let mut by_time = Rejoin::default();
        let now = break_of(
            &mut by_time,
            Instant::now(),
            Duration::from_secs(600),
            88.0,
            61.0,
        );
        let (car, session) = live(160.0, 4);
        by_time.observe(now + HELD_AT_MOST, &car, &session, 65.0);
        assert!(!by_time.warming_up(), "and it is never held for ever");
    }

    /// **A program that stalled did not watch a car stand.** The gap between
    /// two readings counts for at most two seconds.
    #[test]
    fn a_gap_in_readings_is_not_a_break() {
        let mut rejoin = Rejoin::default();
        let start = Instant::now();
        let (car, session) = live(150.0, 2);
        rejoin.observe(start, &car, &session, 88.0);
        let (still, _) = live(0.0, 2);
        rejoin.observe(start + Duration::from_secs(1), &still, &session, 87.0);
        rejoin.observe(start + Duration::from_secs(3600), &still, &session, 87.0);
        rejoin.observe(start + Duration::from_secs(3601), &car, &session, 87.0);
        assert!(rejoin.briefing().is_none());
    }
}
