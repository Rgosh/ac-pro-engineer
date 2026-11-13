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

use serde::Deserialize;
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
}
