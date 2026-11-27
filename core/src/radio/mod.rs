//! The engineer on the team radio: what to say, and when.
//!
//! **Decided here, once, for every front end.** The terminal and RG Pro
//! Engineer only hand this a [`Moment`] each tick and play what comes back;
//! neither chooses a line or a moment of its own, so two engineers never say
//! different things about the same lap — the same rule the advice follows.
//!
//! What it decides, in order:
//!
//! 1. **What is worth saying.** A finding of the engineer's that has a line in
//!    [`lines::LINES`], or an event: the radio check, a personal best, fuel
//!    every few laps, the start of a drift. A finding is said once and then
//!    not again for [`REPEAT_AFTER`], unless it gets worse.
//! 2. **What first.** Urgent before anything (a locking rear, fuel gone),
//!    then the engineer's own order — most severe first, which is the order
//!    the advice already arrives in.
//! 3. **When.** Never under braking or with the car loaded in a corner: a line
//!    waits for [`CALM_FOR`] of straight road. An urgent line only waits for
//!    the brakes to come off. One line at a time, and a line that waited
//!    longer than [`STALE_AFTER`] is dropped rather than said late.
//!
//! What it never does: think about the car. Every verdict is the engineer's;
//! the radio only picks which of them to say aloud and when.

pub mod lines;
pub mod pack;
pub mod voice;

use std::collections::HashMap;

use crate::engineer::{Recommendation, Severity};
use lines::{Event, Line, Style};

/// Straight road needed before a calm line is said, in milliseconds.
pub const CALM_FOR: u64 = 900;
/// How long the same finding stays unsaid after it was said, in milliseconds.
pub const REPEAT_AFTER: u64 = 4 * 60 * 1000;
/// A line that could not be said for this long is dropped, in milliseconds.
pub const STALE_AFTER: u64 = 45 * 1000;
/// The gap between two lines, so the radio is a voice and not a stream.
pub const BETWEEN: u64 = 6 * 1000;
/// How long the subtitle stays up after the line ends, in milliseconds.
pub const SUBTITLE_AFTER: u64 = 2500;
/// Fuel is reported every this many laps, and every lap under five.
pub const FUEL_EVERY: i32 = 5;

/// What the car and the engineer are doing this tick. Filled in by the front
/// end from what it already holds; nothing in it is computed for the radio.
#[derive(Debug, Clone, Copy, Default)]
pub struct Moment<'a> {
    /// A monotonic clock in milliseconds. Any origin; only differences count.
    pub now_ms: u64,
    /// A car on track this tick.
    pub connected: bool,
    pub speed_kmh: f32,
    /// 0..1.
    pub brake: f32,
    /// Sideways load in g, either sign.
    pub lateral_g: f32,
    pub lap_count: i32,
    pub last_lap_ms: i32,
    pub best_lap_ms: i32,
    /// Laps of fuel left, when the engineer knows.
    pub fuel_laps: Option<f32>,
    /// The engineer reads the driving as a drift (see [`crate::drift`]).
    pub drifting: bool,
    /// The engineer's findings, most severe first, in the screen's language.
    pub advice: &'a [Recommendation],
}

/// A line to say now.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// The clip in the voice pack.
    pub key: String,
    pub style: Style,
    /// What is said, in the voice's English.
    pub said: String,
    /// What the subtitle shows: the line in the screen's language where there
    /// is a translation, and the finding's measurement under it.
    pub subtitle: Subtitle,
}

/// What the overlay shows while the engineer speaks.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Subtitle {
    pub text: String,
    /// The measurement behind it — "FL OVERHEATING: 112 °C" — so a driver who
    /// hears "tyres are overheating" can see which one, and by how much.
    pub detail: String,
    pub style: Option<Style>,
    pub critical: bool,
}

#[derive(Debug, Clone)]
struct Waiting {
    call: Call,
    /// The line, and how severe the finding was when it was queued — written
    /// into [`Radio::said`] when it is *said*, not when it is queued, so a
    /// line dropped for waiting too long can be queued again.
    id: &'static str,
    level: u8,
    since: u64,
    urgent: bool,
}

/// The radio's memory between ticks.
#[derive(Debug, Default)]
pub struct Radio {
    /// When each line (by clip name, without a value) was last said, and at
    /// what severity.
    said: HashMap<&'static str, (u64, u8)>,
    queue: Vec<Waiting>,
    calm_since: Option<u64>,
    speaking_until: u64,
    last_spoke: Option<u64>,
    subtitle: Option<(Subtitle, u64)>,
    checked: bool,
    best_ms: i32,
    lap_seen: i32,
    was_drifting: bool,
    /// How long each clip lasts, from the voice pack, for when the subtitle
    /// comes down. Without a pack a line is timed by its words.
    lengths: HashMap<String, f32>,
}

impl Radio {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tell the radio how long each clip lasts.
    pub fn with_lengths(mut self, lengths: HashMap<String, f32>) -> Self {
        self.lengths = lengths;
        self
    }

    /// One tick. Returns a line to start saying now, if there is one.
    pub fn tick(&mut self, m: &Moment<'_>) -> Option<Call> {
        if !m.connected {
            self.calm_since = None;
            return None;
        }
        self.notice_events(m);
        self.notice_advice(m);

        let calm = m.brake < 0.05 && m.lateral_g.abs() < 0.6;
        let off_brakes = m.brake < 0.05;
        self.calm_since = match (calm, self.calm_since) {
            (true, Some(t)) => Some(t),
            (true, None) => Some(m.now_ms),
            (false, _) => None,
        };
        let calm_for = self.calm_since.map_or(0, |t| m.now_ms.saturating_sub(t));
        // Slow or stopped — the pits, a grid, a spin — is as good as straight.
        let parked = m.speed_kmh < 30.0;

        self.queue
            .retain(|w| m.now_ms.saturating_sub(w.since) < STALE_AFTER);
        if m.now_ms < self.speaking_until {
            return None;
        }
        let rested = self
            .last_spoke
            .is_none_or(|t| m.now_ms.saturating_sub(t) >= BETWEEN);
        let pick = self.queue.iter().position(|w| {
            if w.urgent {
                off_brakes
            } else {
                rested && (parked || calm_for >= CALM_FOR)
            }
        })?;
        let waiting = self.queue.remove(pick);
        self.said.insert(waiting.id, (m.now_ms, waiting.level));
        let seconds = self
            .lengths
            .get(&waiting.call.key)
            .copied()
            .unwrap_or_else(|| estimated_seconds(&waiting.call.said));
        let ends = m.now_ms + (seconds * 1000.0) as u64;
        self.speaking_until = ends;
        self.last_spoke = Some(ends);
        self.subtitle = Some((waiting.call.subtitle.clone(), ends + SUBTITLE_AFTER));
        Some(waiting.call)
    }

    /// What to show while a line is said, and a moment after.
    pub fn subtitle(&self, now_ms: u64) -> Option<&Subtitle> {
        self.subtitle
            .as_ref()
            .filter(|(_, until)| now_ms < *until)
            .map(|(s, _)| s)
    }

    fn notice_events(&mut self, m: &Moment<'_>) {
        if !self.checked {
            self.checked = true;
            self.event(Event::RadioCheck, None, m.now_ms, String::new());
        }
        if m.lap_count > self.lap_seen {
            let first = self.lap_seen == 0 && self.best_ms == 0;
            self.lap_seen = m.lap_count;
            if m.last_lap_ms > 0 && (self.best_ms == 0 || m.last_lap_ms < self.best_ms) {
                // The first timed lap is a best by definition and says
                // nothing; a best is news when it beats one.
                if self.best_ms > 0 && !first {
                    self.event(Event::PersonalBest, None, m.now_ms, lap_time(m.last_lap_ms));
                }
                self.best_ms = m.last_lap_ms;
            }
            if let Some(laps) = m.fuel_laps {
                let whole = laps.floor() as i32;
                if whole >= 2 && (whole < 5 || m.lap_count % FUEL_EVERY == 0) {
                    let (low, high) = lines::for_event(Event::FuelLaps)
                        .and_then(|l| l.values)
                        .unwrap_or((2, 40));
                    let n = (whole as u32).clamp(low, high);
                    self.event(Event::FuelLaps, Some(n), m.now_ms, format!("{laps:.1}"));
                }
            }
        }
        if m.drifting && !self.was_drifting {
            self.event(Event::Drifting, None, m.now_ms, String::new());
        }
        self.was_drifting = m.drifting;
    }

    fn event(&mut self, event: Event, value: Option<u32>, now: u64, detail: String) {
        let Some(line) = lines::for_event(event) else {
            return;
        };
        // An event that repeats by nature — fuel each lap — is not held back
        // by the cooldown; one that is news once is.
        if value.is_none() && self.recently(line, now, 0) {
            return;
        }
        if self.queue.iter().any(|w| w.id == line.id) {
            return;
        }
        self.enqueue(line, value, now, detail, 0);
    }

    fn notice_advice(&mut self, m: &Moment<'_>) {
        for advice in m.advice {
            let category = english(&advice.category);
            let component = english(&advice.component);
            let cause = advice
                .chain
                .as_ref()
                .map(|c| english(&c.cause))
                .unwrap_or_default();
            let Some(line) = lines::find(&category, &component, &cause) else {
                continue;
            };
            let level = severity_level(&advice.severity);
            if self.recently(line, m.now_ms, level) {
                continue;
            }
            if self.queue.iter().any(|w| w.id == line.id) {
                continue;
            }
            let detail = match advice.chain.as_ref() {
                Some(chain) if !chain.effect.is_empty() => {
                    format!("{} — {}", advice.message, chain.effect)
                }
                _ => advice.message.clone(),
            };
            self.enqueue(line, None, m.now_ms, detail, level);
        }
    }

    /// Said within [`REPEAT_AFTER`], and not worse now than it was then.
    fn recently(&self, line: &Line, now: u64, level: u8) -> bool {
        self.said
            .get(line.id)
            .is_some_and(|(at, was)| now.saturating_sub(*at) < REPEAT_AFTER && level <= *was)
    }

    fn enqueue(
        &mut self,
        line: &'static Line,
        value: Option<u32>,
        now: u64,
        detail: String,
        level: u8,
    ) {
        let critical = level >= 2;
        let urgent = line.style == Style::Urgent;
        let call = Call {
            key: lines::clip_key(line, value),
            style: line.style,
            said: lines::spoken(line, value),
            subtitle: Subtitle {
                text: lines::spoken(line, value),
                detail,
                style: Some(line.style),
                critical: critical || urgent,
            },
        };
        let waiting = Waiting {
            call,
            id: line.id,
            level,
            since: now,
            urgent,
        };
        // Urgent to the front; the rest keep the engineer's order.
        if urgent {
            let at = self
                .queue
                .iter()
                .position(|w| !w.urgent)
                .unwrap_or(self.queue.len());
            self.queue.insert(at, waiting);
        } else {
            self.queue.push(waiting);
        }
        // A radio that has fallen this far behind is not catching up.
        self.queue.truncate(6);
    }
}

/// The radio as a front end holds it: what to say, the voice that says it,
/// and the clock. **One call a tick and nothing else** — the terminal and the
/// window each own one and do the same two things with it, which is what
/// keeps them saying the same thing.
pub struct Station {
    radio: Radio,
    player: Option<voice::Player>,
    sound: Option<voice::Sound>,
    started: std::time::Instant,
}

impl Default for Station {
    fn default() -> Self {
        Self::new()
    }
}

impl Station {
    pub fn new() -> Self {
        let lengths = pack::Pack::embedded(pack::Voice::Male)
            .map(|p| p.lengths())
            .unwrap_or_default();
        Self {
            radio: Radio::new().with_lengths(lengths),
            player: None,
            sound: None,
            started: std::time::Instant::now(),
        }
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// One tick, from what the front end already holds.
    pub fn tick(
        &mut self,
        config: &crate::config::RadioConfig,
        car: &crate::games::Car,
        session: &crate::games::Session,
        engineer: &crate::engineer::Engineer,
        advice: &[Recommendation],
    ) {
        if !config.voice_on && !config.subtitles {
            return;
        }
        let fuel = engineer.stats.fuel_laps_remaining;
        let moment = Moment {
            now_ms: self.now_ms(),
            connected: true,
            speed_kmh: car.speed_kmh,
            brake: car.brake,
            lateral_g: car.acc_g[0],
            lap_count: session.completed_laps,
            last_lap_ms: session.last_lap_ms,
            best_lap_ms: session.best_lap_ms,
            fuel_laps: (fuel > 0.0).then_some(fuel),
            drifting: engineer.drifting(session),
            advice,
        };
        let Some(call) = self.radio.tick(&moment) else {
            return;
        };
        if !config.voice_on {
            return;
        }
        let sound = config.sound();
        match (&self.player, self.sound) {
            (Some(player), Some(was)) if was != sound => player.set(sound),
            (Some(_), _) => {}
            (None, _) => self.player = voice::Player::start(sound).ok(),
        }
        self.sound = Some(sound);
        if let Some(player) = &self.player {
            player.say(&call.key);
        }
    }

    /// No car this tick: the radio waits for the straight it will need.
    pub fn idle(&mut self) {
        let moment = Moment {
            now_ms: self.now_ms(),
            ..Moment::default()
        };
        let _ = self.radio.tick(&moment);
    }

    /// What to show now, if the driver wants subtitles.
    pub fn subtitle(&self, config: &crate::config::RadioConfig) -> Option<&Subtitle> {
        config
            .subtitles
            .then(|| self.radio.subtitle(self.now_ms()))
            .flatten()
    }
}

fn severity_level(severity: &Severity) -> u8 {
    match severity {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Critical => 2,
    }
}

/// The English of a word the screen may show in Russian.
fn english(text: &str) -> String {
    crate::i18n::english_of(text).unwrap_or(text).to_string()
}

/// How long a line takes to say when the pack does not know: about two and a
/// half words a second on the radio.
pub fn estimated_seconds(said: &str) -> f32 {
    (said.split_whitespace().count() as f32 / 2.6).max(1.0)
}

fn lap_time(ms: i32) -> String {
    let ms = ms.max(0);
    format!("{}:{:02}.{:03}", ms / 60_000, (ms / 1000) % 60, ms % 1000)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::engineer::Chain;

    fn finding(category: &str, component: &str, cause: &str, severity: Severity) -> Recommendation {
        Recommendation {
            component: component.to_string(),
            category: category.to_string(),
            severity,
            message: format!("{component} {category}"),
            action: String::new(),
            parameters: Vec::new(),
            confidence: 0.9,
            chain: Some(Chain {
                cause: cause.to_string(),
                effect: "112 °C".to_string(),
                confirm: String::new(),
                evidence: Default::default(),
            }),
        }
    }

    fn moment(now_ms: u64, advice: &[Recommendation]) -> Moment<'_> {
        Moment {
            now_ms,
            connected: true,
            speed_kmh: 200.0,
            advice,
            ..Default::default()
        }
    }

    /// Run the radio over a stretch of road and collect what it said.
    fn drive(
        radio: &mut Radio,
        from: u64,
        to: u64,
        advice: &[Recommendation],
        brake: f32,
    ) -> Vec<Call> {
        let mut said = Vec::new();
        let mut t = from;
        while t <= to {
            let mut m = moment(t, advice);
            m.brake = brake;
            if let Some(call) = radio.tick(&m) {
                said.push(call);
            }
            t += 50;
        }
        said
    }

    #[test]
    fn the_radio_checks_in_once() {
        let mut radio = Radio::new();
        let said = drive(&mut radio, 0, 30_000, &[], 0.0);
        assert_eq!(said.iter().filter(|c| c.key == "radio.check").count(), 1);
    }

    #[test]
    fn nothing_is_said_under_braking() {
        let mut radio = Radio::new();
        let hot = [finding(
            "Overheat",
            "Tyres",
            "the tyre is being given more energy",
            Severity::Warning,
        )];
        let said = drive(&mut radio, 0, 60_000, &hot, 0.8);
        assert!(said.is_empty(), "spoke under braking: {said:?}");
        // The brakes come off: the radio check and the line, on the straight.
        let said = drive(&mut radio, 60_000, 80_000, &hot, 0.0);
        assert!(said.iter().any(|c| c.key == "tyres.hot"), "{said:?}");
    }

    #[test]
    fn a_finding_is_said_once_until_it_gets_worse() {
        let mut radio = Radio::new();
        let warm = [finding(
            "Overheat",
            "Tyres",
            "the tyre is being given more energy",
            Severity::Warning,
        )];
        let said = drive(&mut radio, 0, 120_000, &warm, 0.0);
        assert_eq!(said.iter().filter(|c| c.key == "tyres.hot").count(), 1);
        let worse = [finding(
            "Overheat",
            "Tyres",
            "the tyre is being given more energy",
            Severity::Critical,
        )];
        let said = drive(&mut radio, 120_000, 140_000, &worse, 0.0);
        assert_eq!(
            said.iter().filter(|c| c.key == "tyres.hot").count(),
            1,
            "worse is news"
        );
    }

    #[test]
    fn urgent_goes_first_and_does_not_wait_for_a_straight() {
        let mut radio = Radio::new();
        let both = [
            finding(
                "Overheat",
                "Tyres",
                "the tyre is being given more energy",
                Severity::Warning,
            ),
            finding(
                "Bias",
                "Brakes",
                "too much of the braking is landing on the rear axle",
                Severity::Critical,
            ),
        ];
        // Loaded in a corner, off the brakes: only the urgent line can go.
        let mut said = Vec::new();
        for t in (0..20_000).step_by(50) {
            let mut m = moment(t, &both);
            m.lateral_g = 1.5;
            said.extend(radio.tick(&m));
        }
        assert_eq!(
            said.first().map(|c| c.key.as_str()),
            Some("bias.rear"),
            "{said:?}"
        );
        assert!(
            said.iter().all(|c| c.key != "tyres.hot"),
            "a calm line in a corner"
        );
    }

    #[test]
    fn the_subtitle_carries_the_measurement() {
        let mut radio = Radio::new();
        let hot = [finding(
            "Overheat",
            "Tyres",
            "the tyre is being given more energy",
            Severity::Warning,
        )];
        let said = drive(&mut radio, 0, 30_000, &hot, 0.0);
        let call = said.iter().find(|c| c.key == "tyres.hot").unwrap();
        assert!(
            call.subtitle.detail.contains("112 °C"),
            "{:?}",
            call.subtitle
        );
        assert!(call.subtitle.text.starts_with("Tyres are overheating"));
    }

    #[test]
    fn a_best_lap_is_news_only_when_it_beats_one() {
        let mut radio = Radio::new();
        let mut said = Vec::new();
        for (lap, time) in [(1, 92_000), (2, 91_500), (3, 93_000)] {
            for t in 0..400 {
                let mut m = moment(lap as u64 * 100_000 + t * 50, &[]);
                m.lap_count = lap;
                m.last_lap_ms = time;
                said.extend(radio.tick(&m));
            }
        }
        assert_eq!(
            said.iter().filter(|c| c.key == "best.lap").count(),
            1,
            "{said:?}"
        );
    }

    #[test]
    fn fuel_is_said_with_its_number() {
        let mut radio = Radio::new();
        let mut said = Vec::new();
        for t in 0..400 {
            let mut m = moment(t * 50, &[]);
            m.lap_count = 5;
            m.fuel_laps = Some(11.6);
            said.extend(radio.tick(&m));
        }
        let fuel = said
            .iter()
            .find(|c| c.key.starts_with("fuel.laps"))
            .unwrap();
        assert_eq!(fuel.key, "fuel.laps.11");
        assert_eq!(fuel.said, "You've got fuel for eleven more laps.");
    }

    #[test]
    fn a_russian_screen_gets_the_same_line() {
        use crate::i18n::Translate;
        let mut radio = Radio::new();
        let category = "Overheat".tr(true).to_string();
        let component = "Tyres".tr(true).to_string();
        let cause = "the tyre is being given more energy than it can shed"
            .tr(true)
            .to_string();
        let hot = [finding(&category, &component, &cause, Severity::Warning)];
        let said = drive(&mut radio, 0, 30_000, &hot, 0.0);
        assert!(said.iter().any(|c| c.key == "tyres.hot"), "{said:?}");
    }
}
