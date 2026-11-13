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

/// How often a quest comes round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Every {
    Day,
    Week,
}

/// What a quest asks for, measured over its day or its week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    Laps(u64),
    CleanInARow(u64),
    Races(u64),
    Podiums(u64),
    Wins(u64),
    NewBests(u64),
    Cars(u64),
    Circuits(u64),
    /// Minutes actually lapping.
    Minutes(u64),
    Days(u64),
}

impl Ask {
    pub fn target(self) -> u64 {
        match self {
            Ask::Laps(n)
            | Ask::CleanInARow(n)
            | Ask::Races(n)
            | Ask::Podiums(n)
            | Ask::Wins(n)
            | Ask::NewBests(n)
            | Ask::Cars(n)
            | Ask::Circuits(n)
            | Ask::Minutes(n)
            | Ask::Days(n) => n,
        }
    }
}

/// Every quest there is: an id that never changes, how often, what it asks
/// and the experience it gives.
pub const QUESTS: [(&str, Every, Ask, u32); 15] = [
    ("day_laps", Every::Day, Ask::Laps(15), 60),
    ("day_clean", Every::Day, Ask::CleanInARow(5), 70),
    ("day_race", Every::Day, Ask::Races(1), 80),
    ("day_best", Every::Day, Ask::NewBests(1), 90),
    ("day_two_cars", Every::Day, Ask::Cars(2), 60),
    ("day_half_hour", Every::Day, Ask::Minutes(30), 70),
    ("day_podium", Every::Day, Ask::Podiums(1), 100),
    ("week_laps", Every::Week, Ask::Laps(120), 300),
    ("week_races", Every::Week, Ask::Races(5), 350),
    ("week_podium", Every::Week, Ask::Podiums(2), 350),
    ("week_clean", Every::Week, Ask::CleanInARow(25), 300),
    ("week_circuits", Every::Week, Ask::Circuits(3), 250),
    ("week_bests", Every::Week, Ask::NewBests(3), 300),
    ("week_days", Every::Week, Ask::Days(4), 300),
    ("week_win", Every::Week, Ask::Wins(1), 400),
];

/// How many quests a day and a week bring.
pub const PER_PERIOD: usize = 3;

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
    pub id: &'static str,
    pub every: Every,
    pub ask: Ask,
    pub xp: u32,
    pub progress: u64,
    /// The period it belongs to: a day number or a week number in the
    /// driver's own time.
    pub period: i64,
}

impl Quest {
    pub fn done(&self) -> bool {
        self.progress >= self.ask.target()
    }

    /// What it is kept under once done: one entry per quest per period, so
    /// a day's quest counts once however many computers report it.
    pub fn key(&self) -> String {
        let every = match self.every {
            Every::Day => "d",
            Every::Week => "w",
        };
        format!("{every}:{}:{}", self.period, self.id)
    }
}

/// The week a day falls in, Monday first.
pub fn week_of(day: i64) -> i64 {
    (day + 3).div_euclid(7)
}

/// A small, stable hash: the same on every computer and every build.
fn stir(a: i64, b: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in a.to_le_bytes().iter().chain(b.as_bytes()) {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The quests a day or a week brings: three of its pool, **the same three on
/// every computer**, picked by the period's number.
pub fn picked(every: Every, period: i64) -> Vec<(&'static str, Every, Ask, u32)> {
    let mut pool: Vec<_> = QUESTS.iter().filter(|q| q.1 == every).copied().collect();
    pool.sort_by_key(|q| stir(period, q.0));
    pool.truncate(PER_PERIOD);
    pool
}

/// How far along each quest of a period is, from the results kept in it.
/// `before` are the bests set before the period, which a new best is
/// measured against.
fn progress_in(
    inside: &[(u64, &Outcome)],
    before: &std::collections::BTreeMap<(String, String), u64>,
    utc_offset: i64,
    ask: Ask,
) -> u64 {
    match ask {
        Ask::Laps(_) => inside
            .iter()
            .flat_map(|(_, o)| o.sessions.iter().filter(|s| s.driven()))
            .map(|s| u64::from(s.laps_of(0)))
            .sum(),
        Ask::CleanInARow(_) => {
            let (mut run, mut most) = (0u64, 0u64);
            for lap in inside
                .iter()
                .flat_map(|(_, o)| o.sessions.iter().flat_map(|s| s.laps.iter()))
                .filter(|lap| lap.car == 0 && lap.time > 0)
            {
                run = if lap.cuts == 0 { run + 1 } else { 0 };
                most = most.max(run);
            }
            most
        }
        Ask::Races(_) | Ask::Podiums(_) | Ask::Wins(_) => {
            let r = record(inside.iter().map(|(_, o)| *o));
            match ask {
                Ask::Races(_) => r.races as u64,
                Ask::Podiums(_) => r.podiums as u64,
                _ => r.wins as u64,
            }
        }
        Ask::NewBests(_) => {
            let mut bests = before.clone();
            let mut new = 0u64;
            for (_, outcome) in inside {
                let (Some(car), Some(ms)) = (own_car(outcome), own_best(outcome)) else {
                    continue;
                };
                let key = (car.to_string(), outcome.track.clone());
                if bests.get(&key).is_none_or(|had| ms < *had) {
                    new += 1;
                    bests.insert(key, ms);
                }
            }
            new
        }
        Ask::Cars(_) => inside
            .iter()
            .filter_map(|(_, o)| own_car(o))
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u64,
        Ask::Circuits(_) => inside
            .iter()
            .map(|(_, o)| o.track.as_str())
            .filter(|t| !t.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u64,
        Ask::Minutes(_) => {
            inside
                .iter()
                .flat_map(|(_, o)| o.sessions.iter().flat_map(|s| s.laps.iter()))
                .filter(|l| l.car == 0 && l.time > 0)
                .map(|l| l.time as u64)
                .sum::<u64>()
                / 60_000
        }
        Ask::Days(_) => days_driven(inside.iter().map(|(at, _)| *at), utc_offset).len() as u64,
    }
}

/// Every quest of every day and week there are results in, in order —
/// today's and this week's among them, `today` being the driver's day
/// number. Done ones from the past are what experience is counted from,
/// so a driver's level is the same on every computer that holds the same
/// results.
pub fn quests(stamped: &[(u64, &Outcome)], today: i64, utc_offset: i64) -> Vec<Quest> {
    let mut order: Vec<(u64, &Outcome)> = stamped.iter().map(|(at, o)| (*at, *o)).collect();
    order.sort_by_key(|(at, _)| *at);
    let day_of = |at: u64| {
        i64::try_from(at)
            .unwrap_or(i64::MAX)
            .saturating_add(utc_offset)
            .div_euclid(86_400)
    };
    let mut periods: std::collections::BTreeSet<(Every, i64)> = order
        .iter()
        .flat_map(|(at, _)| {
            let day = day_of(*at);
            [(Every::Day, day), (Every::Week, week_of(day))]
        })
        .collect();
    periods.insert((Every::Day, today));
    periods.insert((Every::Week, week_of(today)));
    let mut out = Vec::new();
    for (every, period) in periods {
        let inside: Vec<(u64, &Outcome)> = order
            .iter()
            .filter(|(at, _)| {
                let day = day_of(*at);
                match every {
                    Every::Day => day == period,
                    Every::Week => week_of(day) == period,
                }
            })
            .copied()
            .collect();
        let starts = |at: u64| {
            let day = day_of(at);
            match every {
                Every::Day => day < period,
                Every::Week => week_of(day) < period,
            }
        };
        let mut before: std::collections::BTreeMap<(String, String), u64> = Default::default();
        for (_, outcome) in order.iter().filter(|(at, _)| starts(*at)) {
            if let (Some(car), Some(ms)) = (own_car(outcome), own_best(outcome)) {
                let had = before
                    .entry((car.to_string(), outcome.track.clone()))
                    .or_insert(ms);
                *had = (*had).min(ms);
            }
        }
        for (id, every, ask, xp) in picked(every, period) {
            out.push(Quest {
                id,
                every,
                ask,
                xp,
                progress: progress_in(&inside, &before, utc_offset, ask).min(ask.target()),
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

    /// **The same three quests on every computer**, never the same quest
    /// twice in a period, and a period's quests done by its own results.
    #[test]
    fn quests_are_picked_the_same_everywhere_and_count_their_own_period() {
        for period in 0..200 {
            let a = picked(Every::Day, period);
            assert_eq!(a, picked(Every::Day, period));
            assert_eq!(a.len(), PER_PERIOD);
            let ids: std::collections::BTreeSet<_> = a.iter().map(|q| q.0).collect();
            assert_eq!(ids.len(), PER_PERIOD);
            assert!(
                picked(Every::Week, period)
                    .iter()
                    .all(|q| q.1 == Every::Week)
            );
        }
        // Not the same three every day.
        assert_ne!(picked(Every::Day, 10), picked(Every::Day, 11));
        let day = 86_400u64;
        let today = 20_000i64;
        let morning = (today as u64) * day + 3600;
        let laps: Vec<i64> = vec![90_000; 20];
        let driven_today = driven("mx5", "vallelunga", &laps, &[]);
        let yesterday = driven("mx5", "vallelunga", &[95_000], &[]);
        let stamped = [(morning - day, &yesterday), (morning, &driven_today)];
        let got = quests(&stamped, today, 0);
        let todays: Vec<&Quest> = got
            .iter()
            .filter(|q| q.every == Every::Day && q.period == today)
            .collect();
        assert_eq!(todays.len(), PER_PERIOD);
        for q in todays {
            let expected = match q.ask {
                Ask::Laps(t) => 20.min(t),
                Ask::CleanInARow(t) => 20.min(t),
                Ask::NewBests(t) => 1.min(t),
                Ask::Cars(t) => 1.min(t),
                Ask::Minutes(t) => 30.min(t),
                Ask::Races(_) | Ask::Podiums(_) | Ask::Wins(_) => 0,
                Ask::Circuits(t) => 1.min(t),
                Ask::Days(t) => 1.min(t),
            };
            assert_eq!(q.progress, expected, "{q:?}");
        }
        // Yesterday's own quests are there too, counted from yesterday.
        assert!(
            got.iter()
                .any(|q| q.every == Every::Day && q.period == today - 1)
        );
        assert!(got.iter().all(|q| !q.key().is_empty()));
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
