//! Where a wheel locked, and where one spun.
//!
//! **The counts have existed for a year and none of them says where.** The
//! engineer counts frames in which a wheel was sliding, a lap carries
//! `lockup_count`, and a driver reading "lockups: 11" learns that something
//! happened eleven times and nothing about which corner to go and work on.
//! Eleven lockups spread round a lap is a driver braking too hard everywhere;
//! eleven at one corner is one braking point, and those are opposite problems
//! with opposite fixes.
//!
//! So this finds the *places*: one event per application rather than per
//! sample, with which wheels, how long, how fast the car was going and which
//! corner it was in.
//!
//! # Front and rear are different findings
//!
//! A front wheel locked under braking is the car going straight on at the
//! corner — the steering stops doing anything the moment the tyre stops
//! turning. A rear wheel locked is the car trying to swap ends. The first is
//! a lost tenth and a flat-spotted tyre; the second is a spin. They are
//! reported apart because the driver does different things about them.
//!
//! # What this refuses to do
//!
//! **Nothing at all on a lap that carries no wheel data.** `Detail::measured`
//! is false on a lap recorded before the field existed, and every slip is then
//! a zero — which would read as a driver who never once locked a wheel. Saying
//! nothing is the honest answer and it is the one this gives.
//!
//! It also does not name a drivetrain. Which wheels are driven decides whether
//! slip under power is wheelspin or a front axle being dragged, and the core
//! does not know the layout — so the wheels are reported and the reader, who
//! is sitting in the car, knows which ones they are.

use crate::analyzer::TelemetryPoint;
use crate::confidence::Evidence;
use crate::corners::Corner;
use crate::engineer::{Chain, Recommendation, Severity};

/// How much a wheel has to be sliding before it is worth a place on a map.
///
/// **Higher than the bar a counter uses.** `engineer` counts frames past 0.2,
/// which is right for a number that is allowed to include a kerb and a bump;
/// a mark on the road that says "you locked here" has to be a thing the driver
/// would recognise, and 0.2 is not.
pub const SLIDING: f32 = 0.35;

/// The pedal has to be doing something, or a wheel sliding is the car being
/// thrown about rather than the driver doing it.
pub const ON_THE_BRAKE: f32 = 0.15;
pub const ON_THE_THROTTLE: f32 = 0.25;

/// Shorter than this and it is a kerb, a bump or one noisy sample.
///
/// A tenth of a second at 200 km/h is five metres of road, which is about the
/// shortest lockup anybody notices through a wheel.
pub const BRIEFEST_MS: i32 = 100;

/// A fault that has to appear in this many places before it is a habit rather
/// than an incident.
pub const A_HABIT: usize = 3;

/// What went wrong at one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A front wheel stopped turning under braking. The car goes straight on.
    FrontLocked,
    /// A rear wheel did. The car tries to swap ends.
    RearLocked,
    /// A wheel spun up under power.
    Spun,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::FrontLocked => "FRONT LOCKED",
            Kind::RearLocked => "REAR LOCKED",
            Kind::Spun => "WHEELSPIN",
        }
    }

    /// What it does to the car, in the words a driver would use.
    pub fn what_it_does(self) -> &'static str {
        match self {
            Kind::FrontLocked => "the steering stops working and the car runs wide",
            Kind::RearLocked => "the back comes round",
            Kind::Spun => "the power goes into smoke instead of the road",
        }
    }
}

/// One stretch of road where a wheel was sliding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    pub kind: Kind,
    /// Normalised distance where it started and where it stopped.
    pub at: f32,
    pub ends: f32,
    pub ms: i32,
    /// Which of FL FR RL RR were sliding at the worst of it.
    pub wheels: [bool; 4],
    /// The worst slip reached, so one that nearly happened reads differently
    /// from one that did.
    pub worst: f32,
    /// How fast the car was going when it began, km/h.
    pub speed: f32,
    /// The corner it happened in, where it was in one. `None` on a straight,
    /// which is itself worth knowing: a lockup on a straight is a driver
    /// braking in one movement rather than a corner being difficult.
    pub corner: Option<usize>,
}

impl Event {
    /// `T7` or the place round the lap, for a row that has to name it.
    pub fn where_it_was(&self) -> String {
        match self.corner {
            Some(number) => format!("T{number}"),
            None => format!("{:.0} % round", self.at * 100.0),
        }
    }
}

/// Every place on one lap where a wheel let go.
#[derive(Debug, Clone, Default)]
pub struct Found {
    pub events: Vec<Event>,
    /// How many laps went into this. Set by [`over`]; one lap otherwise.
    laps: usize,
    /// Whether the lap carried wheel data at all. `false` means nothing was
    /// looked for, which is not the same as nothing being there.
    pub measured: bool,
}

impl Found {
    /// Every event of one kind, worst first.
    pub fn of(&self, kind: Kind) -> Vec<&Event> {
        let mut found: Vec<&Event> = self
            .events
            .iter()
            .filter(|event| event.kind == kind)
            .collect();
        found.sort_by(|a, b| b.worst.total_cmp(&a.worst));
        found
    }

    /// How many laps this covers. One, unless [`over`] built it.
    pub fn laps(&self) -> usize {
        self.laps.max(1)
    }

    /// The places where one kind happened on more than one lap.
    ///
    /// **The whole point of locating them, and it needs more than one lap.**
    /// Locking at T3 once is a lap; locking at T3 on four of five is a brake
    /// point or a bias, and the driver should stop practising and change
    /// something. Within a single lap a corner is braked into once, so
    /// counting there says nothing — which is what the first version of this
    /// did, and what its own test found.
    pub fn repeated(&self, kind: Kind) -> Vec<(String, usize)> {
        let mut seen: Vec<(String, usize)> = Vec::new();
        for event in self.events.iter().filter(|event| event.kind == kind) {
            let place = event.where_it_was();
            match seen.iter_mut().find(|(name, _)| *name == place) {
                Some((_, count)) => *count += 1,
                None => seen.push((place, 1)),
            }
        }
        seen.retain(|(_, count)| *count > 1);
        seen.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        seen
    }

    /// The findings, worst first.
    pub fn advice(&self) -> Vec<Recommendation> {
        if !self.measured {
            return Vec::new();
        }
        let mut found: Vec<Recommendation> = [Kind::RearLocked, Kind::FrontLocked, Kind::Spun]
            .into_iter()
            .filter_map(|kind| self.finding(kind))
            .collect();
        // A rear that locks is the one that ends a session, so it is read
        // first whatever the counts say; within a severity, worse slip wins.
        found.sort_by(|a, b| {
            b.severity
                .partial_cmp(&a.severity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        found
    }

    fn finding(&self, kind: Kind) -> Option<Recommendation> {
        let events = self.of(kind);
        if events.is_empty() {
            return None;
        }

        let mut evidence = Evidence::new();
        for event in &events {
            evidence.observe(event.worst);
        }
        let places: Vec<String> = events
            .iter()
            .take(4)
            .map(|event| event.where_it_was())
            .collect();
        let where_it_is = places.join(", ");
        let again = self.repeated(kind);

        // **The same place every time is a setting; scattered is a habit.**
        // Which of those it is decides what the driver should go and change,
        // and it is the only thing this module knows that a counter does not.
        let (message, action) = match (kind, again.first()) {
            (Kind::FrontLocked, Some((place, count))) => (
                format!(
                    "The fronts lock at {place}, on {count} of your {} laps.",
                    self.laps()
                ),
                "One corner, every time: this is the brake point or the bias, not your \
                 technique. Try a click of bias rearward, or brake a car's length earlier and \
                 build the pressure."
                    .to_string(),
            ),
            (Kind::FrontLocked, None) => (
                format!("The fronts locked in {} places.", events.len()),
                "Spread about, so it is the first squeeze rather than any one corner. Come \
                 onto the pedal and then press, instead of arriving on it."
                    .to_string(),
            ),
            (Kind::RearLocked, Some((place, count))) => (
                format!(
                    "The rears lock at {place}, on {count} of your {} laps.",
                    self.laps()
                ),
                "The back will come round. Move the brake bias forward a click before you do \
                 anything else."
                    .to_string(),
            ),
            (Kind::RearLocked, None) => (
                format!("The rears locked in {} places.", events.len()),
                "Each one is the back trying to come round. Bias forward, and be gentler \
                 coming off the pedal on the way in."
                    .to_string(),
            ),
            (Kind::Spun, Some((place, count))) => (
                format!(
                    "The wheels spin up at {place}, on {count} of your {} laps.",
                    self.laps()
                ),
                "One corner, every time: you are asking for the throttle before the car is \
                 straight enough to take it. Wait for the wheel to come back, then squeeze."
                    .to_string(),
            ),
            (Kind::Spun, None) => (
                format!("The wheels spun up in {} places.", events.len()),
                "Squeeze rather than press: the tyre takes everything you give it if you give \
                 it over half a second."
                    .to_string(),
            ),
        };

        let lost = events.iter().map(|event| event.ms).sum::<i32>();
        Some(Recommendation {
            component: match kind {
                Kind::Spun => "TRACTION".to_string(),
                _ => "BRAKING".to_string(),
            },
            category: "Driving".to_string(),
            // **A locked rear is the only one of the three that ends a
            // session.** The other two cost a tenth and a tyre.
            severity: match (kind, events.len() >= A_HABIT) {
                (Kind::RearLocked, _) => Severity::Critical,
                (_, true) => Severity::Warning,
                _ => Severity::Info,
            },
            message,
            action,
            parameters: Vec::new(),
            confidence: 0.0,
            chain: Some(Chain {
                cause: format!("{}, {}", kind.label().to_lowercase(), kind.what_it_does()),
                effect: format!(
                    "{:.1} s of it across the lap, at {where_it_is}",
                    lost as f32 / 1000.0
                ),
                confirm: format!("whether it still happens at {where_it_is} next run"),
                evidence,
            }),
        })
    }
}

/// The same, over several laps.
///
/// **Which is where this becomes worth reading.** One lap says a wheel locked
/// at T3; several say whether it locks at T3 *every time*, and those are a
/// mistake and a setting — different things with different fixes. Each place
/// is counted once per lap, because a corner braked into twice in one lap is
/// one corner.
pub fn over(laps: &[&[TelemetryPoint]], corners: &[Corner]) -> Found {
    let mut all: Vec<Event> = Vec::new();
    let mut measured = false;
    let mut counted = 0;
    for lap in laps {
        let found = look(lap, corners);
        if !found.measured {
            continue;
        }
        measured = true;
        counted += 1;
        // One event per place per lap: the same corner locked twice on one
        // lap is still that corner, once.
        let mut seen: Vec<String> = Vec::new();
        for event in found.events {
            let place = format!("{}{:?}", event.where_it_was(), event.kind);
            if seen.contains(&place) {
                continue;
            }
            seen.push(place);
            all.push(event);
        }
    }
    all.sort_by(|a, b| a.at.total_cmp(&b.at));
    Found {
        events: all,
        laps: counted,
        measured,
    }
}

/// Find every place on this lap where a wheel let go.
pub fn look(trace: &[TelemetryPoint], corners: &[Corner]) -> Found {
    // **Nothing at all where nothing was measured.** Every slip is zero on a
    // lap recorded before the field existed, and a screen saying "no lockups"
    // about that is the program inventing a clean lap.
    let measured = trace.iter().any(|point| point.detail.measured);
    if !measured {
        return Found::default();
    }

    let mut events: Vec<Event> = Vec::new();
    // One run per kind at a time, so a lockup and a wheelspin overlapping —
    // which happens on a car being caught out of a slow corner — are two
    // events and not one confused one.
    for kind in [Kind::FrontLocked, Kind::RearLocked, Kind::Spun] {
        let mut running: Option<Event> = None;
        for point in trace {
            match sliding(point, kind) {
                Some((wheels, worst)) => match running.as_mut() {
                    Some(event) => {
                        event.ends = point.distance;
                        event.ms = point.time_ms - event.ms.max(0);
                        if worst > event.worst {
                            event.worst = worst;
                            event.wheels = wheels;
                        }
                    }
                    None => {
                        running = Some(Event {
                            kind,
                            at: point.distance,
                            ends: point.distance,
                            // The start time, turned into a duration when it
                            // ends: a running event has no length yet.
                            ms: point.time_ms,
                            wheels,
                            worst,
                            speed: point.speed,
                            corner: corner_at(corners, point.distance),
                        });
                    }
                },
                None => {
                    if let Some(mut event) = running.take() {
                        event.ms = finished(&event, trace);
                        if event.ms >= BRIEFEST_MS {
                            events.push(event);
                        }
                    }
                }
            }
        }
        if let Some(mut event) = running.take() {
            event.ms = finished(&event, trace);
            if event.ms >= BRIEFEST_MS {
                events.push(event);
            }
        }
    }

    events.sort_by(|a, b| a.at.total_cmp(&b.at));
    Found {
        events,
        laps: 1,
        measured,
    }
}

/// How long an event lasted, from the trace's own clock.
///
/// **Measured from the times at its ends rather than accumulated.** The
/// running event carries the time it started in `ms` until it finishes, which
/// is a field doing two jobs for the length of one loop and is the only reason
/// this function exists — accumulating instead would drift by a sample every
/// time.
fn finished(event: &Event, trace: &[TelemetryPoint]) -> i32 {
    let end = trace
        .iter()
        .rev()
        .find(|point| point.distance <= event.ends)
        .map(|point| point.time_ms)
        .unwrap_or(event.ms);
    (end - event.ms).max(0)
}

/// Whether this sample is one kind of letting go, and how badly.
fn sliding(point: &TelemetryPoint, kind: Kind) -> Option<([bool; 4], f32)> {
    if !point.detail.measured {
        return None;
    }
    let slip = &point.detail.wheel_slip;
    let (which, pedal): ([usize; 2], bool) = match kind {
        Kind::FrontLocked => ([0, 1], point.brake >= ON_THE_BRAKE),
        Kind::RearLocked => ([2, 3], point.brake >= ON_THE_BRAKE),
        // Under power, any wheel: which ones are driven is the car's layout
        // and the core does not know it. The wheels are reported and the
        // person in the car knows which they are.
        Kind::Spun => (
            [0, 2],
            point.gas >= ON_THE_THROTTLE && point.brake < ON_THE_BRAKE,
        ),
    };
    if !pedal {
        return None;
    }
    let mut wheels = [false; 4];
    let mut worst = 0.0f32;
    let range = match kind {
        Kind::Spun => 0..4,
        _ => which[0]..which[1] + 1,
    };
    for wheel in range {
        let amount = slip[wheel].abs();
        if amount >= SLIDING {
            wheels[wheel] = true;
            worst = worst.max(amount);
        }
    }
    wheels.iter().any(|on| *on).then_some((wheels, worst))
}

/// Which corner a place round the lap is in, if any.
fn corner_at(corners: &[Corner], distance: f32) -> Option<usize> {
    corners
        .iter()
        .find(|corner| distance >= corner.entry && distance <= corner.exit)
        .map(|corner| corner.number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::Detail;

    /// One sample: how fast, which pedal, and what each wheel is doing.
    fn sample(at: f32, ms: i32, brake: f32, gas: f32, slip: [f32; 4]) -> TelemetryPoint {
        TelemetryPoint {
            distance: at,
            time_ms: ms,
            speed: 180.0,
            gas,
            brake,
            gear: 4,
            steer: 0.0,
            lat_g: 0.0,
            lon_g: 0.0,
            slip_avg: slip.iter().sum::<f32>() / 4.0,
            x: at * 1000.0,
            y: 0.0,
            rpms: 7_000,
            detail: Detail {
                measured: true,
                wheel_slip: slip,
                ..Default::default()
            },
        }
    }

    /// A lap that is quiet everywhere except one stretch.
    fn lap_with(from: f32, to: f32, brake: f32, gas: f32, slip: [f32; 4]) -> Vec<TelemetryPoint> {
        (0..200)
            .map(|step| {
                let at = step as f32 / 200.0;
                let ms = step * 50;
                match (from..=to).contains(&at) {
                    true => sample(at, ms, brake, gas, slip),
                    false => sample(at, ms, 0.0, 0.4, [0.0; 4]),
                }
            })
            .collect()
    }

    /// **The whole point: a place, not a count.** "Lockups: 11" says something
    /// happened and nothing about where to go.
    #[test]
    fn a_lockup_is_a_place_and_not_a_number() {
        let lap = lap_with(0.30, 0.34, 0.9, 0.0, [0.6, 0.55, 0.0, 0.0]);
        let found = look(&lap, &[]);

        let locked = found.of(Kind::FrontLocked);
        assert_eq!(locked.len(), 1, "one application, one event");
        assert!((locked[0].at - 0.30).abs() < 0.01, "{}", locked[0].at);
        assert!(locked[0].ms >= BRIEFEST_MS, "{} ms", locked[0].ms);
        assert_eq!(locked[0].wheels, [true, true, false, false]);
        assert!(found.of(Kind::RearLocked).is_empty());
    }

    /// A front that locks and a rear that locks are opposite problems, and
    /// only one of them ends a session.
    #[test]
    fn a_locked_rear_is_read_before_anything_else() {
        let lap = lap_with(0.30, 0.36, 0.9, 0.0, [0.0, 0.0, 0.6, 0.58]);
        let found = look(&lap, &[]);

        assert_eq!(found.of(Kind::RearLocked).len(), 1);
        let advice = found.advice();
        assert_eq!(advice[0].severity, Severity::Critical);
        assert!(
            advice[0].action.to_lowercase().contains("bias"),
            "the one thing to change first: {}",
            advice[0].action
        );
    }

    /// Forty samples of one lockup is one lockup. Reporting each sample would
    /// make a single corner look like a driver who cannot brake at all.
    #[test]
    fn a_long_slide_is_one_event() {
        let lap = lap_with(0.20, 0.45, 0.9, 0.0, [0.7, 0.7, 0.0, 0.0]);
        let found = look(&lap, &[]);
        assert_eq!(found.of(Kind::FrontLocked).len(), 1);
        assert!(found.of(Kind::FrontLocked)[0].ms > 1_000);
    }

    /// A kerb, a bump or one noisy sample is not a lockup.
    #[test]
    fn one_sample_is_not_a_lockup() {
        let mut lap = lap_with(0.0, 0.0, 0.0, 0.4, [0.0; 4]);
        lap[80] = sample(0.40, 4_000, 0.9, 0.0, [0.8, 0.8, 0.0, 0.0]);
        assert!(look(&lap, &[]).events.is_empty());
    }

    /// **Nothing at all where nothing was measured.** Every slip is zero on a
    /// lap recorded before the field existed, and "no lockups" about that is
    /// the program inventing a clean lap.
    #[test]
    fn a_lap_with_no_wheel_data_is_not_a_clean_lap() {
        let lap: Vec<TelemetryPoint> = (0..200)
            .map(|step| {
                let mut point = sample(step as f32 / 200.0, step * 50, 0.9, 0.0, [0.0; 4]);
                point.detail.measured = false;
                point
            })
            .collect();
        let found = look(&lap, &[]);
        assert!(!found.measured);
        assert!(found.events.is_empty());
        assert!(
            found.advice().is_empty(),
            "a lap nothing was measured on has no findings"
        );
    }

    /// A wheel sliding with no pedal down is the car being thrown about, not
    /// the driver doing something.
    #[test]
    fn a_wheel_sliding_off_the_pedals_is_not_a_fault() {
        let lap = lap_with(0.30, 0.40, 0.0, 0.0, [0.8, 0.8, 0.8, 0.8]);
        assert!(look(&lap, &[]).events.is_empty());
    }

    /// **The same corner every lap is a setting; once is a lap**, and telling
    /// them apart is the only thing this knows that a counter does not.
    ///
    /// It needs more than one lap to know it, which the first version of this
    /// did not — it counted within a lap, where a corner is braked into once
    /// and the count therefore says nothing. Its own test found that.
    #[test]
    fn the_same_corner_every_lap_reads_differently_from_once() {
        use crate::corners::Direction;
        let t3 = Corner {
            number: 3,
            direction: Direction::Left,
            entry: 0.28,
            apex: 0.30,
            exit: 0.34,
            entry_speed: 200.0,
            min_speed: 90.0,
            exit_speed: 150.0,
            peak_lat_g: 1.4,
            brake_point: Some(0.28),
            braking: None,
            throttle_point: None,
            throttle_delay_ms: None,
            entry_time_ms: 0,
            exit_time_ms: 1_000,
        };

        // Four laps, and the fronts let go in T3 on three of them.
        let locking = lap_with(0.29, 0.33, 0.9, 0.0, [0.6, 0.6, 0.0, 0.0]);
        let clean = lap_with(0.0, 0.0, 0.0, 0.4, [0.0; 4]);
        let laps: Vec<&[TelemetryPoint]> = vec![&locking, &locking, &clean, &locking];

        let found = over(&laps, std::slice::from_ref(&t3));
        assert_eq!(found.laps(), 4);
        assert_eq!(
            found
                .repeated(Kind::FrontLocked)
                .first()
                .map(|(place, count)| (place.as_str(), *count)),
            Some(("T3", 3))
        );
        let advice = found.advice();
        assert!(advice[0].message.contains("T3"), "{}", advice[0].message);
        assert!(
            advice[0].message.contains("3 of your 4"),
            "it says how often: {}",
            advice[0].message
        );

        // One lap with one lockup is not a habit and must not read as one.
        let once = over(&[&locking], std::slice::from_ref(&t3));
        assert!(once.repeated(Kind::FrontLocked).is_empty());
        assert!(
            !once.advice()[0].message.contains("of your"),
            "{}",
            once.advice()[0].message
        );
    }

    /// A lockup on a straight is a driver arriving on the pedal, not a corner
    /// being difficult — so it is named by where it is rather than left blank.
    #[test]
    fn a_place_that_is_not_a_corner_still_has_a_name() {
        let lap = lap_with(0.60, 0.65, 0.9, 0.0, [0.7, 0.7, 0.0, 0.0]);
        let found = look(&lap, &[]);
        let event = found.of(Kind::FrontLocked)[0];
        assert_eq!(event.corner, None);
        assert!(
            event.where_it_was().contains("%"),
            "{}",
            event.where_it_was()
        );
    }
}
