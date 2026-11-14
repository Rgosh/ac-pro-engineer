//! Every session's result, kept — the game keeps only the last one.
//!
//! **`race_out.json` is written over each time a session ends.** The game
//! leaves the whole result there — every driver, every lap with its sectors
//! and cuts, the order they finished in — and the next session replaces it.
//! An evening of five races is one file by the end of it. Content Manager
//! keeps a copy of each; so does this, and nothing more: the numbers shown
//! are the game's own, in the game's own words.
//!
//! [`championship`](super::championship) reads the same file for one thing,
//! the finishing order. This reads all of it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One car in a result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Player {
    pub name: String,
    pub car: String,
    pub skin: String,
}

/// One lap somebody drove.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Lap {
    /// Which lap of theirs, from 0.
    pub lap: u32,
    /// Whose: an index into the players.
    pub car: usize,
    /// In milliseconds. The game writes a negative time for a lap that was
    /// not finished.
    pub time: i64,
    /// How many times the track's edge was cut.
    pub cuts: u32,
    pub sectors: Vec<i64>,
    /// The compound's short name, when the game says.
    pub tyre: String,
}

/// A car's best lap in a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Best {
    pub car: usize,
    pub time: i64,
    pub lap: u32,
}

/// One session of a result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Session {
    pub name: String,
    /// The game's number for it (1 practice, 2 qualifying, 3 race…).
    #[serde(rename = "type")]
    pub kind: u32,
    pub laps: Vec<Lap>,
    /// Laps completed, by car.
    #[serde(rename = "lapstotal")]
    pub laps_total: Vec<u32>,
    #[serde(rename = "bestLaps")]
    pub best_laps: Vec<Best>,
    /// Cars in the order they finished.
    #[serde(rename = "raceResult")]
    pub race_result: Vec<usize>,
}

/// One row of a session's classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// 1 for first.
    pub place: usize,
    pub name: String,
    pub car: String,
    pub laps: u32,
    /// The best lap in milliseconds, when one was timed.
    pub best: Option<u64>,
    /// Cuts over the whole session.
    pub cuts: u32,
    /// Whether this is the car the result was written for: the game's car 0.
    pub you: bool,
}

impl Session {
    /// A car's best timed lap: what the game says it was, else the quickest
    /// of the laps it lists.
    pub fn best_of(&self, car: usize) -> Option<u64> {
        let said = self
            .best_laps
            .iter()
            .filter(|best| best.car == car && best.time > 0)
            .map(|best| best.time as u64)
            .min();
        said.or_else(|| {
            self.laps
                .iter()
                .filter(|lap| lap.car == car && lap.time > 0)
                .map(|lap| lap.time as u64)
                .min()
        })
    }

    /// Laps a car completed.
    pub fn laps_of(&self, car: usize) -> u32 {
        self.laps_total.get(car).copied().unwrap_or_else(|| {
            self.laps
                .iter()
                .filter(|lap| lap.car == car && lap.time > 0)
                .count() as u32
        })
    }

    /// The classification: a race in the order it finished, anything else
    /// by best lap — those with a time first, quickest first.
    pub fn table(&self, players: &[Player]) -> Vec<Row> {
        let mut order: Vec<usize> = match self.kind == 3 && !self.race_result.is_empty() {
            true => self.race_result.clone(),
            false => {
                let mut by_time: Vec<usize> = (0..players.len()).collect();
                by_time.sort_by_key(|&car| (self.best_of(car).unwrap_or(u64::MAX), car));
                by_time
            }
        };
        // A car the order leaves out is still a car that was there.
        for car in 0..players.len() {
            if !order.contains(&car) {
                order.push(car);
            }
        }
        order
            .into_iter()
            .filter_map(|car| players.get(car).map(|player| (car, player)))
            .enumerate()
            .map(|(at, (car, player))| Row {
                place: at + 1,
                name: player.name.clone(),
                car: player.car.clone(),
                laps: self.laps_of(car),
                best: self.best_of(car),
                cuts: self
                    .laps
                    .iter()
                    .filter(|lap| lap.car == car)
                    .map(|lap| lap.cuts)
                    .sum(),
                you: car == 0,
            })
            .collect()
    }

    /// Whether anybody drove a lap in it.
    pub fn driven(&self) -> bool {
        self.laps.iter().any(|lap| lap.time > 0) || self.laps_total.iter().any(|laps| *laps > 0)
    }
}

/// What the game wrote after a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Outcome {
    pub track: String,
    pub players: Vec<Player>,
    pub sessions: Vec<Session>,
}

impl Outcome {
    /// The session worth showing first: the last one anybody drove in.
    pub fn main(&self) -> Option<&Session> {
        self.sessions.iter().rev().find(|session| session.driven())
    }

    /// Whether there is anything in it to keep: a session left before a
    /// lap was finished is not a result.
    pub fn worth_keeping(&self) -> bool {
        !self.players.is_empty() && self.main().is_some()
    }
}

/// What a driver's kept results add up to, for a profile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    /// Results kept with a session driven in them.
    pub sessions: usize,
    /// Of those, races against at least one other car.
    pub races: usize,
    pub wins: usize,
    /// First, second or third.
    pub podiums: usize,
    /// Laps completed, every session counted.
    pub laps: u64,
    /// Laps with a time the game kept, and of those the ones without a cut
    /// — what a clean-lap rate is worked out from. **Its own count**, not
    /// `laps`: the game's lap total includes laps it kept no time for, and a
    /// rate over those would call an untimed lap dirty.
    pub timed_laps: u64,
    pub clean_laps: u64,
    /// The places of every counted race, added up — with `races`, the
    /// average finish.
    pub places: usize,
}

impl Record {
    /// Wins per counted race, 0 → 1. `None` before the first race.
    pub fn win_rate(&self) -> Option<f32> {
        (self.races > 0).then(|| self.wins as f32 / self.races as f32)
    }

    /// Podiums per counted race, 0 → 1.
    pub fn podium_rate(&self) -> Option<f32> {
        (self.races > 0).then(|| self.podiums as f32 / self.races as f32)
    }

    /// The average place over counted races.
    pub fn average_finish(&self) -> Option<f32> {
        (self.races > 0).then(|| self.places as f32 / self.races as f32)
    }

    /// Timed laps without a cut, 0 → 1. `None` before the first timed lap.
    pub fn clean_rate(&self) -> Option<f32> {
        (self.timed_laps > 0).then(|| self.clean_laps as f32 / self.timed_laps as f32)
    }
}

/// Add up results. **A race alone is not a win**: a race is counted only
/// when there was somebody to beat, so a hot-lap session on an empty grid
/// does not fill the trophy cabinet.
pub fn record<'a>(outcomes: impl IntoIterator<Item = &'a Outcome>) -> Record {
    let mut out = Record::default();
    for outcome in outcomes {
        let driven: Vec<&Session> = outcome.sessions.iter().filter(|s| s.driven()).collect();
        if driven.is_empty() {
            continue;
        }
        out.sessions += 1;
        out.laps += driven.iter().map(|s| u64::from(s.laps_of(0))).sum::<u64>();
        for lap in driven
            .iter()
            .flat_map(|s| s.laps.iter())
            .filter(|lap| lap.car == 0 && lap.time > 0)
        {
            out.timed_laps += 1;
            out.clean_laps += u64::from(lap.cuts == 0);
        }
        let race = driven
            .iter()
            .rev()
            .find(|s| s.kind == 3 && outcome.players.len() > 1);
        if let Some(race) = race
            && let Some(you) = race.table(&outcome.players).into_iter().find(|r| r.you)
        {
            out.races += 1;
            out.wins += usize::from(you.place == 1);
            out.podiums += usize::from(you.place <= 3);
            out.places += you.place;
        }
    }
    out
}

/// The days something was driven, and how many sessions each — a key per
/// day counted from 1970 in the driver's own time, `utc_offset` seconds
/// from UTC, so a session at half past midnight belongs to the day the
/// driver would say it did.
pub fn days_driven(
    stamps: impl IntoIterator<Item = u64>,
    utc_offset: i64,
) -> std::collections::BTreeMap<i64, usize> {
    let mut days = std::collections::BTreeMap::new();
    for at in stamps {
        let local = i64::try_from(at)
            .unwrap_or(i64::MAX)
            .saturating_add(utc_offset);
        *days.entry(local.div_euclid(86_400)).or_insert(0) += 1;
    }
    days
}

/// Days in a row with something driven: the run that reaches `today` or the
/// day before it — a streak is not broken by a day that has not finished —
/// and the longest there has been.
pub fn streaks(days: &std::collections::BTreeMap<i64, usize>, today: i64) -> (u32, u32) {
    let (mut longest, mut run, mut last) = (0u32, 0u32, None::<i64>);
    for &day in days.keys() {
        run = match last {
            Some(before) if day == before + 1 => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        last = Some(day);
    }
    let current = match last {
        Some(day) if day >= today - 1 => run,
        _ => 0,
    };
    (current, longest)
}

/// The game's result, read leniently. `None` when it is not one.
pub fn parse(json: &str) -> Option<Outcome> {
    serde_json::from_str(json).ok()
}

/// A driver's best lap on one car at one circuit, and when it was set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonalBest {
    pub car: String,
    pub track: String,
    /// Milliseconds.
    pub ms: u64,
    /// When the result it came from was kept, seconds since 1970.
    pub at: u64,
}

/// The driver's own car in a result: the game's car 0.
fn own_car(outcome: &Outcome) -> Option<&str> {
    outcome
        .players
        .first()
        .map(|p| p.car.as_str())
        .filter(|c| !c.is_empty())
}

/// The quickest lap the driver timed in a result, over every session in it.
fn own_best(outcome: &Outcome) -> Option<u64> {
    outcome
        .sessions
        .iter()
        .filter_map(|session| session.best_of(0))
        .min()
}

/// Every car and circuit's best lap over kept results, `(when, result)`.
/// **A minimum, so merging is safe**: bests from two computers put together
/// are each pair's quicker one, and nothing is counted twice.
pub fn personal_bests(stamped: &[(u64, &Outcome)]) -> Vec<PersonalBest> {
    let mut bests: std::collections::BTreeMap<(String, String), (u64, u64)> = Default::default();
    for (at, outcome) in stamped {
        let (Some(car), Some(ms)) = (own_car(outcome), own_best(outcome)) else {
            continue;
        };
        if outcome.track.is_empty() {
            continue;
        }
        let key = (car.to_string(), outcome.track.clone());
        let entry = bests.entry(key).or_insert((ms, *at));
        if ms < entry.0 {
            *entry = (ms, *at);
        }
    }
    bests
        .into_iter()
        .map(|((car, track), (ms, at))| PersonalBest { car, track, ms, at })
        .collect()
}

/// Two lists of bests as one: each pair's quicker lap, the earlier when
/// equal — so merging in either order gives the same list.
pub fn merge_bests(mine: &[PersonalBest], theirs: &[PersonalBest]) -> Vec<PersonalBest> {
    let mut all: std::collections::BTreeMap<(String, String), PersonalBest> = Default::default();
    for one in mine.iter().chain(theirs) {
        let key = (one.car.clone(), one.track.clone());
        match all.get(&key) {
            Some(had) if (had.ms, had.at) <= (one.ms, one.at) => {}
            _ => {
                all.insert(key, one.clone());
            }
        }
    }
    all.into_values().collect()
}

/// One achievement: how far along, and when it was reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achievement {
    /// Never changes; the front ends name and draw it.
    pub id: &'static str,
    pub progress: u64,
    pub target: u64,
    /// When the result that reached it was kept.
    pub at: Option<u64>,
}

impl Achievement {
    pub fn done(&self) -> bool {
        self.progress >= self.target
    }
}

/// What can be achieved, and how much of it it takes.
pub const ACHIEVEMENTS: [(&str, u64); 12] = [
    ("first_win", 1),
    ("first_podium", 1),
    ("ten_wins", 10),
    ("fifty_races", 50),
    ("clean_hundred", 100),
    ("week_streak", 7),
    ("month_streak", 30),
    ("thousand_laps", 1_000),
    ("one_car_thousand", 1_000),
    ("ten_circuits", 10),
    ("twenty_five_cars", 25),
    ("marathon", 50),
];

/// Every achievement, worked out from kept results in the order they were
/// driven. **Counted from what the game wrote**, so nothing here can be had
/// without driving it — and the moment each was reached is the result that
/// reached it, not the day the launcher was first opened.
pub fn achievements(stamped: &[(u64, &Outcome)], utc_offset: i64) -> Vec<Achievement> {
    let mut order: Vec<&(u64, &Outcome)> = stamped.iter().collect();
    order.sort_by_key(|(at, _)| *at);
    let mut reached: std::collections::BTreeMap<&str, u64> = Default::default();
    let mut best: std::collections::BTreeMap<&str, u64> = Default::default();
    let mut note = |id: &'static str,
                    value: u64,
                    at: u64,
                    best: &mut std::collections::BTreeMap<&str, u64>| {
        let had = best.entry(id).or_insert(0);
        *had = (*had).max(value);
        let target = ACHIEVEMENTS
            .iter()
            .find(|(k, _)| *k == id)
            .map_or(u64::MAX, |(_, t)| *t);
        if value >= target {
            reached.entry(id).or_insert(at);
        }
    };
    let (mut wins, mut podiums, mut races, mut laps, mut clean_run) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut per_car: std::collections::BTreeMap<String, u64> = Default::default();
    let mut tracks: std::collections::BTreeSet<String> = Default::default();
    let mut cars: std::collections::BTreeSet<String> = Default::default();
    let mut days: std::collections::BTreeMap<i64, usize> = Default::default();
    for (at, outcome) in order {
        let driven: Vec<&Session> = outcome.sessions.iter().filter(|s| s.driven()).collect();
        if driven.is_empty() {
            continue;
        }
        let at = *at;
        let local = i64::try_from(at)
            .unwrap_or(i64::MAX)
            .saturating_add(utc_offset);
        *days.entry(local.div_euclid(86_400)).or_insert(0) += 1;
        let (_, longest) = streaks(&days, i64::MAX);
        note("week_streak", u64::from(longest), at, &mut best);
        note("month_streak", u64::from(longest), at, &mut best);
        let these: u64 = driven.iter().map(|s| u64::from(s.laps_of(0))).sum();
        laps += these;
        note("thousand_laps", laps, at, &mut best);
        let most_in_one = driven
            .iter()
            .map(|s| u64::from(s.laps_of(0)))
            .max()
            .unwrap_or(0);
        note("marathon", most_in_one, at, &mut best);
        if let Some(car) = own_car(outcome) {
            let on_car = per_car.entry(car.to_string()).or_insert(0);
            *on_car += these;
            let most = *on_car;
            note("one_car_thousand", most, at, &mut best);
            cars.insert(car.to_string());
            note("twenty_five_cars", cars.len() as u64, at, &mut best);
        }
        if !outcome.track.is_empty() {
            tracks.insert(outcome.track.clone());
            note("ten_circuits", tracks.len() as u64, at, &mut best);
        }
        for lap in driven
            .iter()
            .flat_map(|s| s.laps.iter())
            .filter(|lap| lap.car == 0 && lap.time > 0)
        {
            clean_run = match lap.cuts {
                0 => clean_run + 1,
                _ => 0,
            };
            note("clean_hundred", clean_run, at, &mut best);
        }
        let race = driven
            .iter()
            .rev()
            .find(|s| s.kind == 3 && outcome.players.len() > 1);
        if let Some(race) = race
            && let Some(you) = race.table(&outcome.players).into_iter().find(|r| r.you)
        {
            races += 1;
            wins += u64::from(you.place == 1);
            podiums += u64::from(you.place <= 3);
            note("fifty_races", races, at, &mut best);
            note("first_win", wins, at, &mut best);
            note("ten_wins", wins, at, &mut best);
            note("first_podium", podiums, at, &mut best);
        }
    }
    ACHIEVEMENTS
        .iter()
        .map(|(id, target)| Achievement {
            id,
            progress: best.get(id).copied().unwrap_or(0).min(*target),
            target: *target,
            at: reached.get(id).copied(),
        })
        .collect()
}

/// A stretch of driving, told back: what a monthly or yearly recap shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recap {
    pub sessions: usize,
    pub races: usize,
    pub wins: usize,
    pub podiums: usize,
    pub laps: u64,
    pub timed_laps: u64,
    pub clean_laps: u64,
    /// Time spent on timed laps, milliseconds — the time a driver was
    /// actually lapping, not the time the game was open.
    pub lapping_ms: u64,
    pub days: usize,
    pub longest_streak: u32,
    /// The car and the circuit with the most laps, and how many.
    pub top_car: Option<(String, u64)>,
    pub top_track: Option<(String, u64)>,
    /// The biggest drop in a best lap on one car and circuit over the
    /// stretch: from the best before it (or its first in it) to its best in
    /// it.
    pub biggest_gain: Option<Gain>,
    /// New bests set in the stretch.
    pub new_bests: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gain {
    pub car: String,
    pub track: String,
    pub from_ms: u64,
    pub to_ms: u64,
}

/// Recap results kept from `from` (inclusive) to `to` (exclusive), seconds
/// since 1970; results before `from` only set the bests the stretch is
/// measured against.
pub fn recap(stamped: &[(u64, &Outcome)], from: u64, to: u64, utc_offset: i64) -> Recap {
    let mut order: Vec<&(u64, &Outcome)> = stamped.iter().collect();
    order.sort_by_key(|(at, _)| *at);
    let mut out = Recap::default();
    let mut before: std::collections::BTreeMap<(String, String), u64> = Default::default();
    let mut first_in: std::collections::BTreeMap<(String, String), u64> = Default::default();
    let mut best_in: std::collections::BTreeMap<(String, String), u64> = Default::default();
    let mut car_laps: std::collections::BTreeMap<String, u64> = Default::default();
    let mut track_laps: std::collections::BTreeMap<String, u64> = Default::default();
    let mut inside: Vec<&Outcome> = Vec::new();
    let mut stamps: Vec<u64> = Vec::new();
    for (at, outcome) in order {
        let key = own_car(outcome).map(|car| (car.to_string(), outcome.track.clone()));
        let lap = own_best(outcome);
        if *at < from {
            if let (Some(key), Some(ms)) = (key, lap) {
                let had = before.entry(key).or_insert(ms);
                *had = (*had).min(ms);
            }
            continue;
        }
        if *at >= to {
            continue;
        }
        if !outcome.sessions.iter().any(|s| s.driven()) {
            continue;
        }
        inside.push(outcome);
        stamps.push(*at);
        let laps: u64 = outcome
            .sessions
            .iter()
            .filter(|s| s.driven())
            .map(|s| u64::from(s.laps_of(0)))
            .sum();
        out.lapping_ms += outcome
            .sessions
            .iter()
            .flat_map(|s| s.laps.iter())
            .filter(|l| l.car == 0 && l.time > 0)
            .map(|l| l.time as u64)
            .sum::<u64>();
        if let Some(car) = own_car(outcome) {
            *car_laps.entry(car.to_string()).or_insert(0) += laps;
        }
        if !outcome.track.is_empty() {
            *track_laps.entry(outcome.track.clone()).or_insert(0) += laps;
        }
        if let (Some(key), Some(ms)) = (key, lap) {
            let was = best_in
                .get(&key)
                .copied()
                .or_else(|| before.get(&key).copied());
            if was.is_none_or(|was| ms < was) {
                out.new_bests += 1;
            }
            first_in.entry(key.clone()).or_insert(ms);
            let b = best_in.entry(key).or_insert(ms);
            *b = (*b).min(ms);
        }
    }
    let record = record(inside.iter().copied());
    out.sessions = record.sessions;
    out.races = record.races;
    out.wins = record.wins;
    out.podiums = record.podiums;
    out.laps = record.laps;
    out.timed_laps = record.timed_laps;
    out.clean_laps = record.clean_laps;
    let days = days_driven(stamps, utc_offset);
    out.days = days.len();
    out.longest_streak = streaks(&days, i64::MAX).1;
    out.top_car = car_laps
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n > 0);
    out.top_track = track_laps
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(_, n)| *n > 0);
    out.biggest_gain = best_in
        .iter()
        .filter_map(|(key, now)| {
            let was = before
                .get(key)
                .copied()
                .or_else(|| first_in.get(key).copied())?;
            (was > *now).then(|| Gain {
                car: key.0.clone(),
                track: key.1.clone(),
                from_ms: was,
                to_ms: *now,
            })
        })
        .max_by_key(|gain| gain.from_ms - gain.to_ms);
    out
}

// ── quests, experience and levels ────────────────────────────────────────
//
// **Every driver has their own quests, and none comes back for months.** A
// quest is a pattern from a slot's table — what is counted, where, and how
// much — and a day or a week takes one from each of its three slots. Which
// one is a permutation of the slot's whole table keyed by the driver's own
// number, so a slot walks through every quest it has before any returns, and
// two drivers walk through them in different orders.
//
// **Counted by driving, not by farming.** A lap is counted only with a time
// and at a real pace — within 15 % of the driver's own best in that session —
// so a crawl round the circuit adds nothing. A race counts with another car
// on the grid and three laps driven; a top half needs four cars, a podium
// or a win five. And a period's quests give what they give: driving more
// past the target earns nothing more, which is what makes farming pointless.

/// How often a quest comes round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Every {
    Day,
    Week,
}

impl Every {
    fn letter(self) -> &'static str {
        match self {
            Every::Day => "d",
            Every::Week => "w",
        }
    }
}

/// What a quest counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    /// Counted laps.
    Laps,
    /// Counted laps without a cut.
    CleanLaps,
    /// Counted laps without a cut, one after another.
    CleanInARow,
    /// Minutes of counted laps.
    Minutes,
    /// Sessions of at least three counted laps.
    Sessions,
    /// Minutes of counted laps in one session, the longest.
    LongRun,
    /// Counted laps within so many thousandths of the driver's best on
    /// that car and circuit.
    NearBest {
        permille: u16,
    },
    /// Counted laps within so many thousandths of the session's own best:
    /// consistency.
    Steady {
        permille: u16,
    },
    /// New bests on a car and circuit — a first time on one counts.
    NewBests,
    Cars,
    Circuits,
    Days,
    Races,
    /// Races finished in the top half of a field of four or more.
    TopHalf,
    /// Races finished in the top three of a field of five or more.
    Podiums,
    /// Races won against a field of five or more.
    Wins,
}

/// Where it has to be done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Anywhere,
    /// All of it in one session.
    OneSession,
    /// All of it in one car.
    OneCar,
    /// All of it on one circuit.
    OneCircuit,
    /// Only in races.
    InRaces,
    /// Only with another car on track — any online server, or AI.
    InCompany,
}

/// What a quest asks: a measure, where, and how much.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ask {
    pub measure: Measure,
    pub scope: Scope,
    pub target: u64,
}

impl Ask {
    pub fn target(self) -> u64 {
        self.target
    }
}

/// One row of a slot's table: every param, scope and target of it is a
/// quest of its own.
struct Pattern {
    measure: Measure,
    /// For `NearBest` and `Steady`, the thousandths; for the rest one zero.
    params: &'static [u16],
    scopes: &'static [Scope],
    targets: &'static [u64],
    /// Experience at the smallest target and at the largest.
    xp: (u32, u32),
}

const ANY5: &[Scope] = &[
    Scope::Anywhere,
    Scope::OneSession,
    Scope::OneCar,
    Scope::OneCircuit,
    Scope::InCompany,
];
const ANY4: &[Scope] = &[
    Scope::Anywhere,
    Scope::OneSession,
    Scope::OneCar,
    Scope::OneCircuit,
];
const WIDE4: &[Scope] = &[
    Scope::Anywhere,
    Scope::OneCar,
    Scope::OneCircuit,
    Scope::InCompany,
];
const SPREAD: &[Scope] = &[Scope::Anywhere, Scope::OneCar, Scope::OneCircuit];
const ONLY: &[Scope] = &[Scope::Anywhere];
const SOCIAL: &[Scope] = &[Scope::Anywhere, Scope::InCompany];
const RACES: &[Scope] = &[Scope::InRaces];
const ZERO: &[u16] = &[0];

const fn p(
    measure: Measure,
    params: &'static [u16],
    scopes: &'static [Scope],
    targets: &'static [u64],
    xp: (u32, u32),
) -> Pattern {
    Pattern {
        measure,
        params,
        scopes,
        targets,
        xp,
    }
}

/// Today's first slot: driving. Every one can be done alone in practice,
/// on any server, in any car.
const DAY_DRIVE: &[Pattern] = &[
    p(
        Measure::Laps,
        ZERO,
        ANY5,
        &[8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22],
        (40, 90),
    ),
    p(
        Measure::Minutes,
        ZERO,
        ANY5,
        &[15, 20, 25, 30, 35, 40, 45],
        (40, 90),
    ),
    p(Measure::Sessions, ZERO, SPREAD, &[2, 3], (45, 65)),
];

/// Today's second slot: clean driving.
const DAY_CLEAN: &[Pattern] = &[
    p(
        Measure::CleanLaps,
        ZERO,
        ANY5,
        &[5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
        (45, 100),
    ),
    p(
        Measure::CleanInARow,
        ZERO,
        ANY5,
        &[3, 4, 5, 6, 7, 8, 9, 10],
        (45, 110),
    ),
];

/// Today's third slot: pace and variety.
const DAY_PACE: &[Pattern] = &[
    p(
        Measure::NearBest { permille: 0 },
        &[5, 8, 10, 15, 20, 25, 30],
        ANY4,
        &[1, 2, 3],
        (45, 85),
    ),
    p(
        Measure::Steady { permille: 0 },
        &[10, 15, 20, 25, 30],
        ANY4,
        &[3, 4, 5, 6],
        (40, 80),
    ),
    p(Measure::NewBests, ZERO, SOCIAL, &[1], (90, 90)),
    p(
        Measure::LongRun,
        ZERO,
        SOCIAL,
        &[15, 20, 25, 30, 35, 40],
        (45, 90),
    ),
    p(Measure::Cars, ZERO, ONLY, &[2, 3], (50, 75)),
    p(Measure::Circuits, ZERO, ONLY, &[2, 3], (50, 75)),
];

/// This week's first slot: a week of driving.
const WEEK_DRIVE: &[Pattern] = &[
    p(
        Measure::Laps,
        ZERO,
        WIDE4,
        &[60, 70, 80, 90, 100, 110, 120, 130, 140, 150],
        (250, 450),
    ),
    p(
        Measure::Minutes,
        ZERO,
        WIDE4,
        &[90, 120, 150, 180, 210, 240],
        (250, 450),
    ),
    p(Measure::Days, ZERO, ONLY, &[3, 4, 5], (300, 450)),
    p(
        Measure::Sessions,
        ZERO,
        SPREAD,
        &[8, 10, 12, 15],
        (250, 400),
    ),
];

/// This week's second slot: skill.
const WEEK_SKILL: &[Pattern] = &[
    p(
        Measure::CleanInARow,
        ZERO,
        ANY4,
        &[12, 14, 16, 18, 20, 22, 24, 26, 28, 30],
        (280, 500),
    ),
    p(
        Measure::NearBest { permille: 0 },
        &[5, 8, 10],
        SPREAD,
        &[5, 8, 10, 15],
        (300, 500),
    ),
    p(
        Measure::Steady { permille: 0 },
        &[10, 15, 20],
        ANY4,
        &[8, 10, 12, 15],
        (280, 480),
    ),
    p(Measure::NewBests, ZERO, SPREAD, &[2, 3, 4, 5], (280, 450)),
];

/// This week's third slot: racing and range. A race against the game's own
/// cars counts as much as one online.
const WEEK_RACE: &[Pattern] = &[
    p(
        Measure::Races,
        ZERO,
        SPREAD,
        &[2, 3, 4, 5, 6, 7, 8],
        (280, 500),
    ),
    p(Measure::TopHalf, ZERO, SPREAD, &[1, 2, 3, 4], (300, 500)),
    p(
        Measure::Podiums,
        ZERO,
        &[Scope::Anywhere, Scope::OneCar],
        &[1, 2, 3],
        (350, 550),
    ),
    p(Measure::Wins, ZERO, ONLY, &[1, 2], (450, 600)),
    p(Measure::Cars, ZERO, ONLY, &[3, 4, 5, 6], (250, 380)),
    p(Measure::Circuits, ZERO, ONLY, &[3, 4, 5, 6], (250, 380)),
    p(
        Measure::LongRun,
        ZERO,
        SOCIAL,
        &[45, 60, 75, 90],
        (280, 450),
    ),
    p(
        Measure::CleanLaps,
        ZERO,
        RACES,
        &[20, 30, 40, 50],
        (280, 450),
    ),
    p(
        Measure::Laps,
        ZERO,
        RACES,
        &[20, 30, 40, 50, 60],
        (260, 450),
    ),
];

/// How many quests a day and a week bring: one from each slot.
pub const PER_PERIOD: usize = 3;

/// Experience for doing all three of a day's quests, and of a week's.
pub const SET_XP_DAY: u32 = 50;
pub const SET_XP_WEEK: u32 = 150;

fn table(every: Every, slot: u8) -> &'static [Pattern] {
    match (every, slot) {
        (Every::Day, 0) => DAY_DRIVE,
        (Every::Day, 1) => DAY_CLEAN,
        (Every::Day, _) => DAY_PACE,
        (Every::Week, 0) => WEEK_DRIVE,
        (Every::Week, 1) => WEEK_SKILL,
        (Every::Week, _) => WEEK_RACE,
    }
}

/// Harder where it is narrower: the share of the experience each scope adds.
fn scope_share(scope: Scope) -> u32 {
    match scope {
        Scope::Anywhere => 100,
        Scope::OneCar | Scope::OneCircuit => 110,
        Scope::InCompany => 115,
        Scope::InRaces => 120,
        Scope::OneSession => 125,
    }
}

fn round5(x: u32) -> u32 {
    (x + 2) / 5 * 5
}

/// Every quest a slot holds, in a fixed order — what an index points into.
pub fn space(every: Every, slot: u8) -> Vec<(Ask, u32)> {
    let mut out = Vec::new();
    for pattern in table(every, slot) {
        for (pi, param) in pattern.params.iter().enumerate() {
            let measure = match pattern.measure {
                Measure::NearBest { .. } => Measure::NearBest { permille: *param },
                Measure::Steady { .. } => Measure::Steady { permille: *param },
                other => other,
            };
            // The tightest margin is worth a quarter more than the loosest.
            let param_share = match pattern.params.len() {
                0 | 1 => 100,
                n => 125 - 25 * pi as u32 / (n as u32 - 1),
            };
            for scope in pattern.scopes {
                for (ti, target) in pattern.targets.iter().enumerate() {
                    let span = pattern.targets.len().max(2) as u32 - 1;
                    let base = pattern.xp.0
                        + (pattern.xp.1 - pattern.xp.0) * ti.min(span as usize) as u32 / span;
                    let xp = round5(base * scope_share(*scope) / 100 * param_share / 100);
                    out.push((
                        Ask {
                            measure,
                            scope: *scope,
                            target: *target,
                        },
                        xp,
                    ));
                }
            }
        }
    }
    out
}

/// A small, stable hash over words: the same on every computer and build.
fn stir(words: &[u64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for word in words {
        for byte in word.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    // A last mix, so neighbouring inputs land far apart.
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^ (h >> 33)
}

/// A permutation of `0..size` keyed by `key`: a four-round Feistel network
/// over the smallest even power of two that holds it, walked until it lands
/// inside. A bijection, so every index comes out exactly once.
fn shuffle(index: u64, size: u64, key: u64) -> u64 {
    if size <= 1 {
        return 0;
    }
    let mut bits = 2u32;
    while (1u64 << bits) < size {
        bits += 2;
    }
    let half = bits / 2;
    let mask = (1u64 << half) - 1;
    let mut x = index;
    loop {
        let (mut left, mut right) = (x >> half, x & mask);
        for round in 0..4u64 {
            let f = stir(&[key, round, right]) & mask;
            let next = right;
            right = left ^ f;
            left = next;
        }
        x = (left << half) | right;
        if x < size {
            return x;
        }
    }
}

/// Which quest of a slot a period brings for a driver. A slot's whole
/// table goes round once before a quest of it can come back, and each round
/// in a new order.
pub fn chosen(every: Every, slot: u8, period: i64, seed: u64) -> u32 {
    let size = space(every, slot).len() as i64;
    let round = period.div_euclid(size);
    let at = period.rem_euclid(size);
    let key = stir(&[seed, every as u64, u64::from(slot), round as u64]);
    shuffle(at as u64, size as u64, key) as u32
}

/// Experience for each achievement, by id.
pub fn achievement_xp(id: &str) -> u32 {
    match id {
        "first_win" | "first_podium" | "ten_circuits" | "week_streak" => 300,
        "marathon" | "thousand_laps" | "fifty_races" | "clean_hundred" => 600,
        "ten_wins" | "twenty_five_cars" | "one_car_thousand" => 1000,
        "month_streak" => 1500,
        _ => 200,
    }
}

/// A quest as it stands in its period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quest {
    pub every: Every,
    /// Which of the period's three: 0 driving, 1 clean or skill, 2 pace or
    /// racing.
    pub slot: u8,
    /// Its place in the slot's table.
    pub index: u32,
    pub ask: Ask,
    pub xp: u32,
    pub progress: u64,
    /// The period it belongs to: a day number or a week number in the
    /// driver's own time.
    pub period: i64,
}

impl Quest {
    pub fn done(&self) -> bool {
        self.progress >= self.ask.target
    }

    /// What it is kept under once done: one entry per slot per period, so a
    /// day's quest counts once however many computers report it.
    pub fn key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.every.letter(),
            self.period,
            self.slot,
            self.index
        )
    }
}

/// What all three of a period's quests done is kept under.
pub fn set_key(every: Every, period: i64) -> String {
    format!("{}:{period}:set", every.letter())
}

/// What a kept key is worth, or `None` for one that is not a quest this
/// table gives. Experience that arrives from another computer is counted
/// only through this — a key cannot claim more than its quest gives.
pub fn ledger_xp(key: &str) -> Option<u32> {
    let parts: Vec<&str> = key.split(':').collect();
    let every = match parts.first() {
        Some(&"d") => Every::Day,
        Some(&"w") => Every::Week,
        _ => return None,
    };
    let period = parts.get(1)?;
    if period.is_empty() || period.len() > 8 || !period.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match parts.as_slice() {
        [_, _, "set"] => Some(match every {
            Every::Day => SET_XP_DAY,
            Every::Week => SET_XP_WEEK,
        }),
        [_, _, slot, index] => {
            let slot: u8 = match *slot {
                "0" => 0,
                "1" => 1,
                "2" => 2,
                _ => return None,
            };
            if index.is_empty() || index.len() > 4 || !index.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let index: usize = index.parse().ok()?;
            space(every, slot).get(index).map(|(_, xp)| *xp)
        }
        _ => None,
    }
}

/// The week a day falls in, Monday first.
pub fn week_of(day: i64) -> i64 {
    (day + 3).div_euclid(7)
}

/// One counted lap, with what it was driven in.
struct Counted<'a> {
    day: i64,
    session: usize,
    car: &'a str,
    track: &'a str,
    ms: u64,
    clean: bool,
    session_best: u64,
    race: bool,
    company: bool,
}

/// One session the driver drove in.
struct Driven<'a> {
    day: i64,
    session: usize,
    car: &'a str,
    track: &'a str,
    counted: u64,
    ms: u64,
    company: bool,
    /// A counted race: the place and the field.
    raced: Option<(usize, usize)>,
    /// The driver's best in it, for new bests.
    best: Option<u64>,
}

/// Everything the driver did, read once.
fn driven_by<'a>(
    order: &[(u64, &'a Outcome)],
    utc_offset: i64,
) -> (Vec<Counted<'a>>, Vec<Driven<'a>>) {
    let (mut laps, mut sessions) = (Vec::new(), Vec::new());
    let mut uid = 0usize;
    for (at, outcome) in order {
        let day = i64::try_from(*at)
            .unwrap_or(i64::MAX)
            .saturating_add(utc_offset)
            .div_euclid(86_400);
        let car = own_car(outcome).unwrap_or("");
        let field = outcome.players.len();
        for session in outcome.sessions.iter().filter(|s| s.driven()) {
            uid += 1;
            let best = session.best_of(0);
            let ceiling = best.map_or(0, |b| b + b * 15 / 100);
            let raced_at_all = session.kind == 3 && field >= 2 && session.laps_of(0) >= 3;
            let mut count = 0u64;
            let mut ms = 0u64;
            for lap in session.laps.iter().filter(|l| l.car == 0 && l.time > 0) {
                let time = lap.time as u64;
                if time > ceiling {
                    continue;
                }
                count += 1;
                ms += time;
                laps.push(Counted {
                    day,
                    session: uid,
                    car,
                    track: &outcome.track,
                    ms: time,
                    clean: lap.cuts == 0,
                    session_best: best.unwrap_or(time),
                    race: raced_at_all,
                    company: field >= 2,
                });
            }
            let raced = match raced_at_all {
                true => session
                    .table(&outcome.players)
                    .iter()
                    .find(|row| row.you)
                    .map(|row| (row.place, field)),
                false => None,
            };
            sessions.push(Driven {
                day,
                session: uid,
                car,
                track: &outcome.track,
                counted: count,
                ms,
                company: field >= 2,
                raced,
                best,
            });
        }
    }
    (laps, sessions)
}

/// The group a lap or session falls in for a scope; `None` when the scope
/// leaves it out.
fn group_of(
    scope: Scope,
    session: usize,
    car: &str,
    track: &str,
    race: bool,
    company: bool,
) -> Option<String> {
    match scope {
        Scope::Anywhere => Some(String::new()),
        Scope::OneSession => Some(session.to_string()),
        Scope::OneCar => Some(car.to_string()),
        Scope::OneCircuit => Some(track.to_string()),
        Scope::InRaces => race.then(String::new),
        Scope::InCompany => company.then(String::new),
    }
}

/// How far along a quest is, from what was driven in its period. `before`
/// are the bests set before the period.
fn progress_of(
    ask: Ask,
    laps: &[&Counted<'_>],
    sessions: &[&Driven<'_>],
    before: &std::collections::BTreeMap<(String, String), u64>,
) -> u64 {
    use std::collections::{BTreeMap, BTreeSet};
    let scope = ask.scope;
    let lap_groups = || {
        let mut groups: BTreeMap<String, Vec<&Counted<'_>>> = BTreeMap::new();
        for lap in laps {
            if let Some(g) = group_of(
                scope,
                lap.session,
                lap.car,
                lap.track,
                lap.race,
                lap.company,
            ) {
                groups.entry(g).or_default().push(lap);
            }
        }
        groups
    };
    let session_groups = || {
        let mut groups: BTreeMap<String, Vec<&Driven<'_>>> = BTreeMap::new();
        for s in sessions {
            if let Some(g) = group_of(
                scope,
                s.session,
                s.car,
                s.track,
                s.raced.is_some(),
                s.company,
            ) {
                groups.entry(g).or_default().push(s);
            }
        }
        groups
    };
    let most_laps = |each: &dyn Fn(&[&Counted<'_>]) -> u64| {
        lap_groups().values().map(|g| each(g)).max().unwrap_or(0)
    };
    let most_sessions = |each: &dyn Fn(&[&Driven<'_>]) -> u64| {
        session_groups()
            .values()
            .map(|g| each(g))
            .max()
            .unwrap_or(0)
    };
    match ask.measure {
        Measure::Laps => most_laps(&|g| g.len() as u64),
        Measure::CleanLaps => most_laps(&|g| g.iter().filter(|l| l.clean).count() as u64),
        Measure::CleanInARow => most_laps(&|g| {
            let (mut run, mut most) = (0u64, 0u64);
            for lap in g {
                run = if lap.clean { run + 1 } else { 0 };
                most = most.max(run);
            }
            most
        }),
        Measure::Minutes => most_laps(&|g| g.iter().map(|l| l.ms).sum::<u64>() / 60_000),
        Measure::Steady { permille } => most_laps(&|g| {
            g.iter()
                .filter(|l| l.ms * 1000 <= l.session_best * (1000 + u64::from(permille)))
                .count() as u64
        }),
        Measure::NearBest { permille } => most_laps(&|g| {
            g.iter()
                .filter(|l| {
                    let had = before
                        .get(&(l.car.to_string(), l.track.to_string()))
                        .copied()
                        .unwrap_or(u64::MAX);
                    let reference = had.min(l.session_best);
                    l.ms * 1000 <= reference * (1000 + u64::from(permille))
                })
                .count() as u64
        }),
        Measure::Sessions => most_sessions(&|g| g.iter().filter(|s| s.counted >= 3).count() as u64),
        Measure::LongRun => most_sessions(&|g| g.iter().map(|s| s.ms / 60_000).max().unwrap_or(0)),
        Measure::Races => most_sessions(&|g| g.iter().filter(|s| s.raced.is_some()).count() as u64),
        Measure::TopHalf => {
            most_sessions(&|g| {
                g.iter()
                .filter(|s| matches!(s.raced, Some((place, field)) if field >= 4 && place <= field / 2))
                .count() as u64
            })
        }
        Measure::Podiums => most_sessions(&|g| {
            g.iter()
                .filter(|s| matches!(s.raced, Some((place, field)) if field >= 5 && place <= 3))
                .count() as u64
        }),
        Measure::Wins => most_sessions(&|g| {
            g.iter()
                .filter(|s| matches!(s.raced, Some((place, field)) if field >= 5 && place == 1))
                .count() as u64
        }),
        Measure::NewBests => most_sessions(&|g| {
            let mut bests = before.clone();
            let mut new = 0u64;
            for s in g {
                let Some(ms) = s.best else { continue };
                if s.car.is_empty() {
                    continue;
                }
                let key = (s.car.to_string(), s.track.to_string());
                if bests.get(&key).is_none_or(|had| ms < *had) {
                    new += 1;
                    bests.insert(key, ms);
                }
            }
            new
        }),
        Measure::Cars => most_sessions(&|g| {
            g.iter()
                .filter(|s| s.counted > 0 && !s.car.is_empty())
                .map(|s| s.car)
                .collect::<BTreeSet<_>>()
                .len() as u64
        }),
        Measure::Circuits => most_sessions(&|g| {
            g.iter()
                .filter(|s| s.counted > 0 && !s.track.is_empty())
                .map(|s| s.track)
                .collect::<BTreeSet<_>>()
                .len() as u64
        }),
        Measure::Days => most_sessions(&|g| {
            g.iter()
                .filter(|s| s.counted > 0)
                .map(|s| s.day)
                .collect::<BTreeSet<_>>()
                .len() as u64
        }),
    }
}

/// Every quest of every day and week there are results in, in order —
/// today's and this week's among them, `today` being the driver's day
/// number and `seed` the driver's own number (their Steam account, so the
/// same on every computer they use). Done ones from the past are what
/// experience is counted from, so a level is the same on every computer
/// that holds the same results.
pub fn quests(stamped: &[(u64, &Outcome)], today: i64, utc_offset: i64, seed: u64) -> Vec<Quest> {
    let mut order: Vec<(u64, &Outcome)> = stamped.iter().map(|(at, o)| (*at, *o)).collect();
    order.sort_by_key(|(at, _)| *at);
    let (laps, sessions) = driven_by(&order, utc_offset);
    let mut periods: std::collections::BTreeSet<(Every, i64)> = sessions
        .iter()
        .flat_map(|s| [(Every::Day, s.day), (Every::Week, week_of(s.day))])
        .collect();
    periods.insert((Every::Day, today));
    periods.insert((Every::Week, week_of(today)));
    let spaces: Vec<Vec<(Ask, u32)>> = [Every::Day, Every::Week]
        .iter()
        .flat_map(|every| (0..3).map(move |slot| space(*every, slot)))
        .collect();
    let mut out = Vec::new();
    for (every, period) in periods {
        let inside = |day: i64| match every {
            Every::Day => day == period,
            Every::Week => week_of(day) == period,
        };
        let earlier = |day: i64| match every {
            Every::Day => day < period,
            Every::Week => week_of(day) < period,
        };
        let these_laps: Vec<&Counted<'_>> = laps.iter().filter(|l| inside(l.day)).collect();
        let these_sessions: Vec<&Driven<'_>> = sessions.iter().filter(|s| inside(s.day)).collect();
        let mut before: std::collections::BTreeMap<(String, String), u64> = Default::default();
        for s in sessions.iter().filter(|s| earlier(s.day)) {
            if let (Some(ms), false) = (s.best, s.car.is_empty()) {
                let had = before
                    .entry((s.car.to_string(), s.track.to_string()))
                    .or_insert(ms);
                *had = (*had).min(ms);
            }
        }
        for slot in 0..3u8 {
            let index = chosen(every, slot, period, seed);
            let table = &spaces[usize::from(slot) + if every == Every::Week { 3 } else { 0 }];
            let Some((ask, xp)) = table.get(index as usize).copied() else {
                continue;
            };
            out.push(Quest {
                every,
                slot,
                index,
                ask,
                xp,
                progress: progress_of(ask, &these_laps, &these_sessions, &before).min(ask.target),
                period,
            });
        }
    }
    out
}

/// A level from experience: each one costs 100 more than the one before,
/// starting at 300 — reachable in a few evenings at first, a season at the
/// top. Returns the level, the experience into it, and what it takes.
pub fn level(xp: u64) -> (u32, u64, u64) {
    let (mut level, mut left, mut cost) = (1u32, xp, 300u64);
    while left >= cost && level < 99 {
        left -= cost;
        level += 1;
        cost += 100;
    }
    (level, left, cost)
}

/// The title a level carries, and from which level each is had.
pub const TITLES: [(u32, &str); 8] = [
    (1, "rookie"),
    (3, "club_racer"),
    (5, "regular"),
    (8, "contender"),
    (12, "pro"),
    (16, "ace"),
    (20, "master"),
    (30, "legend"),
];

pub fn title(level: u32) -> &'static str {
    TITLES
        .iter()
        .rev()
        .find(|(from, _)| level >= *from)
        .map_or("rookie", |(_, t)| t)
}

/// One kept result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// When it was kept, in seconds since 1970 — the file's name.
    pub at: u64,
    pub path: PathBuf,
}

/// The most results kept; the oldest go when there are more.
pub const MOST: usize = 500;

/// Keep the game's last result, if it is one and is not already the newest
/// kept. Returns where it went.
///
/// **Compared by content, not by time.** The launcher looks when the game
/// closes and when it starts; the same file seen twice is one result.
pub fn keep(game_documents: &Path, archive: &Path, now: u64) -> Option<PathBuf> {
    let text = std::fs::read_to_string(super::championship::race_out(game_documents)).ok()?;
    if !parse(&text)?.worth_keeping() {
        return None;
    }
    let have = kept(archive);
    if let Some(newest) = have.first()
        && std::fs::read_to_string(&newest.path).ok().as_deref() == Some(text.as_str())
    {
        return None;
    }
    std::fs::create_dir_all(archive).ok()?;
    // Two results in one second would share a name; the later takes the next.
    let mut at = now;
    while archive.join(format!("{at}.json")).exists() {
        at += 1;
    }
    let path = archive.join(format!("{at}.json"));
    std::fs::write(&path, text).ok()?;
    for old in have.iter().skip(MOST.saturating_sub(1)) {
        let _ = std::fs::remove_file(&old.path);
    }
    Some(path)
}

/// Every kept result, newest first.
pub fn kept(archive: &Path) -> Vec<Kept> {
    let Ok(entries) = std::fs::read_dir(archive) else {
        return Vec::new();
    };
    let mut found: Vec<Kept> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let at = name.strip_suffix(".json")?.parse::<u64>().ok()?;
            Some(Kept {
                at,
                path: entry.path(),
            })
        })
        .collect();
    found.sort_by_key(|kept| std::cmp::Reverse(kept.at));
    found
}

/// A kept result, read.
pub fn read(path: &Path) -> Option<Outcome> {
    parse(&std::fs::read_to_string(path).ok()?)
}

/// A lap time as people write one: `1:48.123`, or `48.123` under a minute.
pub fn lap_time(milliseconds: u64) -> String {
    let minutes = milliseconds / 60_000;
    let seconds = (milliseconds % 60_000) / 1000;
    let thousandths = milliseconds % 1000;
    match minutes {
        0 => format!("{seconds}.{thousandths:03}"),
        _ => format!("{minutes}:{seconds:02}.{thousandths:03}"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    use super::*;

    const RACE: &str = r#"{
        "track": "ks_brands_hatch",
        "number_of_sessions": 2,
        "players": [
            {"name": "Driver", "car": "ks_mazda_mx5_cup", "skin": "00_official"},
            {"name": "A. Moreau", "car": "ks_mazda_mx5_cup", "skin": "01"},
            {"name": "K. Tanaka", "car": "ks_mazda_mx5_cup", "skin": "02"}
        ],
        "sessions": [
            {"name": "Qualifying", "type": 2,
             "laps": [
                {"lap": 0, "car": 0, "time": 104500, "cuts": 1, "sectors": [30000, 40000, 34500], "tyre": "ST"},
                {"lap": 1, "car": 0, "time": 103250, "cuts": 0, "sectors": [29800, 39500, 33950], "tyre": "ST"},
                {"lap": 0, "car": 1, "time": 102900, "cuts": 0, "sectors": [], "tyre": "ST"},
                {"lap": 0, "car": 2, "time": -1, "cuts": 0, "sectors": [], "tyre": "ST"}
             ],
             "lapstotal": [2, 1, 0],
             "bestLaps": [{"car": 0, "time": 103250, "lap": 1}, {"car": 1, "time": 102900, "lap": 0}]},
            {"name": "Race", "type": 3,
             "laps": [
                {"lap": 0, "car": 0, "time": 105000, "cuts": 0},
                {"lap": 0, "car": 2, "time": 105400, "cuts": 2},
                {"lap": 0, "car": 1, "time": 106100, "cuts": 0}
             ],
             "lapstotal": [1, 1, 1],
             "bestLaps": [],
             "raceResult": [0, 2, 1]}
        ]
    }"#;

    /// **A race in the order it finished, anything else by best lap** — and
    /// the driver the file was written for marked in both.
    #[test]
    fn a_result_reads_as_a_classification() {
        let outcome = parse(RACE).expect("it reads");
        assert!(outcome.worth_keeping());
        let race = outcome.main().expect("somebody drove");
        assert_eq!(race.name, "Race");
        let table = race.table(&outcome.players);
        let said: Vec<(usize, &str, Option<u64>, u32, bool)> = table
            .iter()
            .map(|r| (r.place, r.name.as_str(), r.best, r.cuts, r.you))
            .collect();
        assert_eq!(
            said,
            vec![
                (1, "Driver", Some(105_000), 0, true),
                (2, "K. Tanaka", Some(105_400), 2, false),
                (3, "A. Moreau", Some(106_100), 0, false),
            ]
        );
        // Qualifying: by time; the car with no timed lap is last, not first.
        let quali = outcome.sessions[0].table(&outcome.players);
        let order: Vec<&str> = quali.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(order, vec!["A. Moreau", "Driver", "K. Tanaka"]);
        assert_eq!(quali[1].best, Some(103_250));
        assert_eq!(quali[1].cuts, 1);
        assert_eq!(quali[2].best, None);
    }

    #[test]
    fn a_session_nobody_drove_is_not_a_result() {
        let empty = r#"{"track":"imola","players":[{"name":"Driver","car":"abarth500"}],
            "sessions":[{"name":"Practice","type":1,"laps":[],"lapstotal":[0]}]}"#;
        assert!(!parse(empty).expect("it reads").worth_keeping());
        assert_eq!(parse("not json"), None);
    }

    /// **Kept once each, newest first** — the same file seen again when the
    /// launcher next starts is the same result.
    #[test]
    fn each_result_is_kept_once() {
        let dir = std::env::temp_dir().join(format!("acpe-results-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let documents = dir.join("documents");
        let archive = dir.join("archive");
        std::fs::create_dir_all(documents.join("out")).expect("a fixture");
        // Nothing written yet: nothing kept.
        assert_eq!(keep(&documents, &archive, 100), None);
        std::fs::write(documents.join("out").join("race_out.json"), RACE).expect("a fixture");
        assert!(keep(&documents, &archive, 100).is_some());
        assert_eq!(
            keep(&documents, &archive, 200),
            None,
            "the same result again"
        );
        let other = RACE.replace("ks_brands_hatch", "ks_silverstone");
        std::fs::write(documents.join("out").join("race_out.json"), other).expect("a fixture");
        assert!(keep(&documents, &archive, 200).is_some());
        let have = kept(&archive);
        assert_eq!(
            have.iter().map(|k| k.at).collect::<Vec<_>>(),
            vec![200, 100]
        );
        assert_eq!(
            read(&have[0].path).map(|o| o.track),
            Some("ks_silverstone".into())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_lap_time_is_written_as_people_write_one() {
        assert_eq!(lap_time(108_123), "1:48.123");
        assert_eq!(lap_time(48_005), "48.005");
        assert_eq!(lap_time(600_000), "10:00.000");
    }

    /// **Wins and podiums from races with somebody to beat** — a race on an
    /// empty grid, and a practice, count as sessions and nothing more.
    #[test]
    fn a_record_counts_races_with_somebody_to_beat() {
        let player = |name: &str| Player {
            name: name.into(),
            ..Player::default()
        };
        let lap = |car: usize| Lap {
            car,
            time: 90_000,
            ..Lap::default()
        };
        let race = |order: Vec<usize>| Session {
            kind: 3,
            laps: vec![lap(0), lap(1), lap(2)],
            race_result: order,
            ..Session::default()
        };
        let field = vec![player("me"), player("a"), player("b"), player("c")];
        let won = Outcome {
            players: field.clone(),
            sessions: vec![race(vec![0, 1, 2, 3])],
            ..Outcome::default()
        };
        let third = Outcome {
            players: field.clone(),
            sessions: vec![race(vec![1, 2, 0, 3])],
            ..Outcome::default()
        };
        let fourth = Outcome {
            players: field,
            sessions: vec![race(vec![1, 2, 3, 0])],
            ..Outcome::default()
        };
        let alone = Outcome {
            players: vec![player("me")],
            sessions: vec![race(vec![0])],
            ..Outcome::default()
        };
        let practice = Outcome {
            players: vec![player("me"), player("a")],
            sessions: vec![Session {
                kind: 1,
                laps: vec![lap(0), Lap { cuts: 2, ..lap(0) }],
                ..Session::default()
            }],
            ..Outcome::default()
        };
        let all = [won, third, fourth, alone, practice, Outcome::default()];
        let said = record(&all);
        assert_eq!(
            said,
            Record {
                sessions: 5,
                races: 3,
                wins: 1,
                podiums: 2,
                laps: 6,
                timed_laps: 6,
                clean_laps: 5,
                places: 8,
            }
        );
        assert_eq!(said.win_rate(), Some(1.0 / 3.0));
        assert_eq!(said.average_finish(), Some(8.0 / 3.0));
        assert_eq!(said.clean_rate(), Some(5.0 / 6.0));
        assert_eq!(Record::default().win_rate(), None);
    }

    /// **A day is the driver's day**, and a streak runs to today or
    /// yesterday — tonight's session has not been driven yet.
    #[test]
    fn days_are_the_drivers_own_and_a_streak_survives_until_tomorrow() {
        let day = 86_400u64;
        // 23:30 UTC on day 10 is 01:30 on day 11 two hours east.
        let days = days_driven([10 * day + 84_600], 2 * 3600);
        assert_eq!(days.keys().copied().collect::<Vec<_>>(), vec![11]);
        let stamps = [1, 2, 3, 7, 8, 8, 9, 10, 11].map(|d| d * day + 3600);
        let days = days_driven(stamps, 0);
        assert_eq!(days.get(&8), Some(&2));
        // Days 7–11 are a run of five; 1–3 one of three.
        assert_eq!(streaks(&days, 11), (5, 5));
        assert_eq!(streaks(&days, 12), (5, 5));
        assert_eq!(streaks(&days, 13), (0, 5));
        assert_eq!(streaks(&Default::default(), 13), (0, 0));
    }

    fn driven(car: &str, track: &str, times: &[i64], cuts: &[u32]) -> Outcome {
        Outcome {
            track: track.into(),
            players: vec![Player {
                name: "me".into(),
                car: car.into(),
                ..Player::default()
            }],
            sessions: vec![Session {
                kind: 1,
                laps: times
                    .iter()
                    .zip(cuts.iter().chain(std::iter::repeat(&0)))
                    .enumerate()
                    .map(|(n, (time, cuts))| Lap {
                        lap: n as u32,
                        car: 0,
                        time: *time,
                        cuts: *cuts,
                        ..Lap::default()
                    })
                    .collect(),
                ..Session::default()
            }],
        }
    }

    /// **A best is a minimum**, per car and circuit; merging two computers'
    /// lists in either order gives the same list.
    #[test]
    fn bests_are_the_quickest_and_merge_either_way() {
        let a = driven("mx5", "vallelunga", &[92_000, 90_500], &[]);
        let b = driven("mx5", "vallelunga", &[89_900], &[]);
        let c = driven("gt3", "spa", &[140_000, -1], &[]);
        let stamped = [(10, &a), (20, &b), (30, &c)];
        let bests = personal_bests(&stamped);
        assert_eq!(bests.len(), 2);
        let mx5 = bests.iter().find(|b| b.car == "mx5").expect("the mx5 best");
        assert_eq!((mx5.ms, mx5.at), (89_900, 20));
        let other = vec![PersonalBest {
            car: "mx5".into(),
            track: "vallelunga".into(),
            ms: 89_000,
            at: 50,
        }];
        assert_eq!(merge_bests(&bests, &other), merge_bests(&other, &bests));
        assert_eq!(
            merge_bests(&bests, &other)
                .iter()
                .find(|b| b.car == "mx5")
                .map(|b| b.ms),
            Some(89_000)
        );
    }

    /// **Reached when the driving reached it**: the moment is the result that
    /// crossed the line, and a cut resets the clean run.
    #[test]
    fn achievements_are_reached_by_the_result_that_reaches_them() {
        let day = 86_400u64;
        let mut results = Vec::new();
        for d in 0..8u64 {
            // 15 clean laps a day, the 5th day with one cut in the middle.
            let cuts: Vec<u32> = (0..15).map(|n| u32::from(d == 4 && n == 7)).collect();
            results.push((
                d * day + 3600,
                driven("mx5", "vallelunga", &[90_000; 15], &cuts),
            ));
        }
        let stamped: Vec<(u64, &Outcome)> = results.iter().map(|(at, o)| (*at, o)).collect();
        let got = achievements(&stamped, 0);
        let find = |id: &str| got.iter().find(|a| a.id == id).expect("every id").clone();
        let week = find("week_streak");
        assert!(week.done());
        assert_eq!(week.at, Some(6 * day + 3600), "the seventh day reached it");
        assert!(!find("month_streak").done());
        assert_eq!(find("month_streak").progress, 8);
        // The longest clean run is before the cut: four days of 15 and 7
        // more — 67. After it, 7 and three days of 15 make only 52.
        assert_eq!(find("clean_hundred").progress, 67);
        assert_eq!(find("thousand_laps").progress, 120);
        assert_eq!(find("ten_circuits").progress, 1);
        assert!(!find("first_win").done());
        assert_eq!(got.len(), ACHIEVEMENTS.len());
    }

    /// **A recap measures against what came before**: a best set in the
    /// stretch counts as a gain only against the best before it.
    #[test]
    fn a_recap_tells_back_its_stretch() {
        let before = driven("mx5", "vallelunga", &[92_000], &[]);
        let first = driven("mx5", "vallelunga", &[91_000, 90_000], &[0, 1]);
        let then = driven("gt3", "spa", &[140_000], &[]);
        let after = driven("mx5", "vallelunga", &[80_000], &[]);
        let stamped = [(5, &before), (100, &first), (200, &then), (500, &after)];
        let r = recap(&stamped, 100, 400, 0);
        assert_eq!(r.sessions, 2);
        assert_eq!((r.timed_laps, r.clean_laps), (3, 2));
        assert_eq!(r.lapping_ms, 91_000 + 90_000 + 140_000);
        assert_eq!(r.top_car, Some(("mx5".into(), 2)));
        assert_eq!(r.new_bests, 2, "the mx5 beat 92.0, and the gt3 had none");
        let gain = r.biggest_gain.expect("a gain");
        assert_eq!((gain.from_ms, gain.to_ms), (92_000, 90_000));
        assert_eq!(recap(&stamped, 1000, 2000, 0), Recap::default());
    }

    /// A session somebody drove with other cars: our laps, and where we
    /// finished in a field of `field`.
    fn raced(car: &str, track: &str, times: &[i64], place: usize, field: usize) -> Outcome {
        let mut o = driven(car, track, times, &[]);
        o.players = (0..field)
            .map(|n| Player {
                name: format!("p{n}"),
                car: car.into(),
                ..Player::default()
            })
            .collect();
        let session = &mut o.sessions[0];
        session.kind = 3;
        let mut result: Vec<usize> = (1..field).collect();
        result.insert(place - 1, 0);
        session.race_result = result;
        o
    }

    /// **Every slot is big enough, and pays inside its band**: a day's
    /// quests stay easy and small, a week's are worth five times as much.
    #[test]
    fn every_slot_has_room_and_pays_in_its_band() {
        for slot in 0..3 {
            let day = space(Every::Day, slot);
            let week = space(Every::Week, slot);
            assert!(day.len() >= 90, "day slot {slot} has {}", day.len());
            assert!(week.len() >= 60, "week slot {slot} has {}", week.len());
            assert!(day.iter().all(|(_, xp)| (35..=150).contains(xp)), "{day:?}");
            assert!(
                week.iter().all(|(_, xp)| (240..=800).contains(xp)),
                "{week:?}"
            );
            // No two quests in a slot are the same quest.
            let mut asks: Vec<_> = day.iter().map(|(a, _)| format!("{a:?}")).collect();
            asks.sort();
            asks.dedup();
            assert_eq!(asks.len(), day.len());
        }
    }

    /// **No quest comes back before its slot has gone round**, and two
    /// drivers do not walk the same way.
    #[test]
    fn a_driver_never_meets_a_quest_twice_in_a_round_and_drivers_differ() {
        for slot in 0..3u8 {
            for every in [Every::Day, Every::Week] {
                let size = space(every, slot).len() as i64;
                let start = 20_000 / size * size;
                let seen: std::collections::BTreeSet<u32> = (start..start + size)
                    .map(|p| chosen(every, slot, p, 42))
                    .collect();
                assert_eq!(seen.len() as i64, size, "{every:?} {slot}");
            }
        }
        let same = (20_000..20_100)
            .filter(|p| chosen(Every::Day, 0, *p, 1) == chosen(Every::Day, 0, *p, 2))
            .count();
        assert!(same < 10, "two drivers matched on {same} days of 100");
        assert_eq!(
            chosen(Every::Week, 2, 2900, 7),
            chosen(Every::Week, 2, 2900, 7)
        );
    }

    /// **Only a key the table gives is worth anything**, and only what its
    /// quest gives.
    #[test]
    fn a_kept_key_is_worth_only_what_its_quest_gives() {
        let (_, xp) = space(Every::Day, 1)[3];
        assert_eq!(ledger_xp("d:20000:1:3"), Some(xp));
        assert_eq!(ledger_xp("w:2900:set"), Some(SET_XP_WEEK));
        assert_eq!(ledger_xp("d:20000:set"), Some(SET_XP_DAY));
        for bad in [
            "",
            "d",
            "d:20000",
            "x:20000:1:3",
            "d:-1:1:3",
            "d:20000:3:0",
            "d:20000:1:99999",
            "d:20000:1:-3",
            "d:123456789:1:3",
            "d:20000:1:3:4",
            "d:20000:01:3",
            "d:20000:set:1",
            "d:2e4:1:3",
            "d:20000:1:3 ",
        ] {
            assert_eq!(ledger_xp(bad), None, "{bad:?}");
        }
    }

    /// **Counted by driving, not by farming**: a crawl adds nothing, a cut
    /// ends a clean run, a one-lap race or a field of two is no podium.
    #[test]
    fn crawling_cutting_and_easy_races_count_for_little() {
        let laps = |times: &[i64], cuts: &[u32]| driven("mx5", "vallelunga", times, cuts);
        let none = Default::default();
        let ask = |measure, target| Ask {
            measure,
            scope: Scope::Anywhere,
            target,
        };
        let count = |o: &Outcome, a: Ask| {
            let order = [(86_400u64, o)];
            let (l, s) = driven_by(&order, 0);
            let l: Vec<&Counted<'_>> = l.iter().collect();
            let s: Vec<&Driven<'_>> = s.iter().collect();
            progress_of(a, &l, &s, &none)
        };
        // Four laps at pace and one at a crawl: four laps, and the crawl's
        // minutes are not counted.
        let o = laps(&[90_000, 91_000, 90_500, 92_000, 300_000], &[]);
        assert_eq!(count(&o, ask(Measure::Laps, 10)), 4);
        assert_eq!(count(&o, ask(Measure::Minutes, 10)), 6);
        let o = laps(&[90_000, 90_000, 90_000, 90_000, 90_000], &[0, 0, 1, 0, 0]);
        assert_eq!(count(&o, ask(Measure::CleanInARow, 10)), 2);
        assert_eq!(count(&o, ask(Measure::CleanLaps, 10)), 4);
        // Steady: within 1 % of the session's best.
        let o = laps(&[90_000, 90_800, 91_000, 92_000], &[]);
        assert_eq!(count(&o, ask(Measure::Steady { permille: 10 }, 10)), 2);
        // A win against one other car over three laps is a race, not a
        // podium; against four it is both.
        let o = raced("mx5", "vallelunga", &[90_000; 3], 1, 2);
        assert_eq!(count(&o, ask(Measure::Races, 9)), 1);
        assert_eq!(count(&o, ask(Measure::Podiums, 9)), 0);
        assert_eq!(count(&o, ask(Measure::Wins, 9)), 0);
        let o = raced("mx5", "vallelunga", &[90_000; 3], 1, 5);
        assert_eq!(count(&o, ask(Measure::Podiums, 9)), 1);
        assert_eq!(count(&o, ask(Measure::Wins, 9)), 1);
        assert_eq!(count(&o, ask(Measure::TopHalf, 9)), 1);
        // A race of two laps does not count at all.
        let o = raced("mx5", "vallelunga", &[90_000; 2], 1, 5);
        assert_eq!(count(&o, ask(Measure::Races, 9)), 0);
    }

    /// **A narrow scope takes the best group**, not the sum: twelve laps in
    /// two cars is six in one car.
    #[test]
    fn a_narrow_scope_counts_its_best_group() {
        let a = driven("mx5", "vallelunga", &[90_000; 6], &[]);
        let b = driven("gt3", "vallelunga", &[90_000; 6], &[]);
        let order = [(86_400u64, &a), (86_500, &b)];
        let (l, s) = driven_by(&order, 0);
        let l: Vec<&Counted<'_>> = l.iter().collect();
        let s: Vec<&Driven<'_>> = s.iter().collect();
        let none = Default::default();
        let at = |scope| {
            progress_of(
                Ask {
                    measure: Measure::Laps,
                    scope,
                    target: 99,
                },
                &l,
                &s,
                &none,
            )
        };
        assert_eq!(at(Scope::Anywhere), 12);
        assert_eq!(at(Scope::OneCar), 6);
        assert_eq!(at(Scope::OneCircuit), 12);
        assert_eq!(at(Scope::OneSession), 6);
        assert_eq!(at(Scope::InCompany), 0);
        assert_eq!(at(Scope::InRaces), 0);
    }

    /// **The same quests on every computer of one driver**, three a day and
    /// three a week, each counted from its own period.
    #[test]
    fn a_driver_s_quests_are_the_same_everywhere_and_count_their_own_period() {
        let day = 86_400u64;
        let today = 20_000i64;
        let morning = (today as u64) * day + 3600;
        let driven_today = driven("mx5", "vallelunga", &[90_000; 20], &[]);
        let yesterday = driven("mx5", "vallelunga", &[95_000], &[]);
        let stamped = [(morning - day, &yesterday), (morning, &driven_today)];
        let got = quests(&stamped, today, 0, 99);
        assert_eq!(got, quests(&stamped, today, 0, 99));
        let todays: Vec<&Quest> = got
            .iter()
            .filter(|q| q.every == Every::Day && q.period == today)
            .collect();
        assert_eq!(todays.len(), PER_PERIOD);
        assert_eq!(
            todays.iter().map(|q| q.slot).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        for q in &todays {
            assert!(q.progress <= q.ask.target);
            assert_eq!(ledger_xp(&q.key()), Some(q.xp), "{q:?}");
        }
        assert!(
            got.iter()
                .any(|q| q.every == Every::Day && q.period == today - 1)
        );
        assert!(
            got.iter()
                .any(|q| q.every == Every::Week && q.period == week_of(today))
        );
    }

    #[test]
    fn levels_cost_more_as_they_go_and_titles_follow() {
        assert_eq!(level(0), (1, 0, 300));
        assert_eq!(level(299), (1, 299, 300));
        assert_eq!(level(300), (2, 0, 400));
        assert_eq!(level(300 + 400 + 50), (3, 50, 500));
        assert_eq!(title(1), "rookie");
        assert_eq!(title(4), "club_racer");
        assert_eq!(title(31), "legend");
        assert_eq!(week_of(4), 1, "1970-01-05 was a Monday");
        assert_eq!(week_of(3), 0);
    }
}
