//! Kunos's career: series of events, each opened by finishing the ones it
//! requires, with the game's own record of how far a driver has come.
//!
//! **As the game and Content Manager read it.** A series is a folder
//! `content/career/series<N>` (not `series0`) with `series.ini`,
//! `opponents.ini` for a championship's field, `preview.png`, and its events
//! `event1`, `event2`… each an `event.ini` like a special event's.
//!
//! `series.ini` says, under `[SERIES]`, its `NAME`, `CODE`, `DESCRIPTION`,
//! the `POINTS` a place earns, and the series it `REQUIRES` (all of them, or
//! any one with `REQUIRESANY=1`); under `[GOALS]`, either a championship's
//! `POINTS` and `RANKING`, or how many firsts, seconds and thirds a series
//! of single events asks for (`TIER3`, `TIER2`, `TIER1`).
//!
//! **The progress is the game's own file**,
//! `Documents/Assetto Corsa/launcherdata/filestore/career.ini`, which the
//! game's career menu reads too: `[CAREER]` `COMPLETE` (the series done),
//! `CURRENTSERIES`, `AI_LEVEL`; and a section per series — its id in capitals
//! — with `EVENT` (the event to drive next), `EVENT<n>` (event n's result),
//! `POINTS`. A single event's result is its cup, 3 for gold down to 1; a
//! championship event's is the place taken. What is written here is written
//! the same way, value by value in place, with the file kept once beside
//! itself first — so a career moved on in the launcher is the career the
//! game shows.

use super::ini;
use super::results::Outcome;
use super::special_events::{self, Cup, Event, Judged};
use std::path::{Path, PathBuf};

/// How a series is won.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Events each with a bronze, a silver and a gold; the series asks for
    /// so many of each.
    SingleEvents {
        firsts: u32,
        seconds: u32,
        thirds: u32,
    },
    /// Points for places across every round, against a field.
    Championship { points_goal: u32, ranking_goal: u32 },
}

/// One series.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub id: String,
    pub folder: PathBuf,
    pub name: String,
    pub code: String,
    pub description: String,
    /// Points for first, second, third…
    pub points: Vec<u32>,
    pub requires: Vec<String>,
    pub requires_any: bool,
    pub kind: Kind,
    pub events: Vec<Event>,
}

fn numbers(text: &str) -> Vec<u32> {
    text.split([',', ' ', ';'])
        .filter_map(|v| v.trim().parse::<u32>().ok())
        .collect()
}

fn list(text: &str) -> Vec<String> {
    text.split([',', ' ', ';'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

/// One series read from its folder, or `None` when it is not one.
pub fn read(folder: &Path) -> Option<Series> {
    let id = folder.file_name()?.to_str()?.to_string();
    let text = std::fs::read_to_string(folder.join("series.ini")).ok()?;
    let get = |section: &str, key: &str| ini::value(&text, section, key).unwrap_or_default();
    let goal = |key: &str| get("GOALS", key).parse::<u32>().unwrap_or(0);
    let points = numbers(&get("SERIES", "POINTS"));
    let (points_goal, ranking_goal) = (goal("POINTS"), goal("RANKING"));
    // Content Manager's rule: no points to reach and no place to hold, or no
    // points for a place at all, is a series of single events.
    let kind = match (points_goal == 0 && ranking_goal == 0) || points.iter().sum::<u32>() == 0 {
        true => Kind::SingleEvents {
            firsts: goal("TIER3"),
            seconds: goal("TIER2"),
            thirds: goal("TIER1"),
        },
        false => Kind::Championship {
            points_goal,
            ranking_goal,
        },
    };
    // event1, event2… until one is missing.
    let events: Vec<Event> = (1..)
        .map(|n| format!("event{n}"))
        .map_while(|name| {
            let dir = folder.join(&name);
            let text = std::fs::read_to_string(dir.join("event.ini")).ok()?;
            special_events::parse(&name, &dir, &text)
        })
        .collect();
    if events.is_empty() {
        return None;
    }
    Some(Series {
        name: match get("SERIES", "NAME") {
            name if name.is_empty() => id.clone(),
            name => name,
        },
        code: get("SERIES", "CODE"),
        description: get("SERIES", "DESCRIPTION")
            .replace("\\n", "\n")
            .replace("<br>", "\n")
            .trim()
            .to_string(),
        points,
        requires: list(&get("SERIES", "REQUIRES")),
        requires_any: matches!(get("SERIES", "REQUIRESANY").trim(), "1" | "true" | "TRUE"),
        kind,
        events,
        id,
        folder: folder.to_path_buf(),
    })
}

/// Every series installed, in their numbers' order.
pub fn all(root: &Path) -> Vec<Series> {
    let at = root.join("content").join("career");
    let mut found: Vec<(u64, Series)> = std::fs::read_dir(&at)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            let number = name.strip_prefix("series")?.parse::<u64>().ok()?;
            (number != 0).then_some(())?;
            Some((number, read(&e.path())?))
        })
        .collect();
    found.sort_by_key(|(n, _)| *n);
    found.into_iter().map(|(_, s)| s).collect()
}

/// The series' picture.
pub fn preview(series: &Series) -> PathBuf {
    series.folder.join("preview.png")
}

/// Where the game keeps a driver's career.
pub fn progress_file(game_documents: &Path) -> PathBuf {
    game_documents
        .join("launcherdata")
        .join("filestore")
        .join("career.ini")
}

/// How far a driver has come in one series.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SeriesProgress {
    /// The event to drive next, from 0.
    pub next: usize,
    /// Each event's result by its number from 0: a cup (3 gold … 1 bronze)
    /// in single events, the place taken in a championship.
    pub results: std::collections::BTreeMap<usize, u32>,
    pub points: u32,
}

/// The whole career as the game keeps it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Progress {
    pub complete: Vec<String>,
    pub ai_level: f32,
    text: String,
}

impl Progress {
    /// Read the game's file; an empty career when there is none.
    pub fn read(game_documents: &Path) -> Progress {
        let text = std::fs::read_to_string(progress_file(game_documents)).unwrap_or_default();
        Progress {
            complete: list(&ini::value(&text, "CAREER", "COMPLETE").unwrap_or_default()),
            ai_level: ini::value(&text, "CAREER", "AI_LEVEL")
                .and_then(|v| v.parse().ok())
                .unwrap_or(95.0),
            text,
        }
    }

    /// One series' progress.
    pub fn of(&self, series: &Series) -> SeriesProgress {
        let section = series.id.to_uppercase();
        let entries = ini::entries(&self.text);
        let mut out = SeriesProgress::default();
        for entry in entries
            .iter()
            .filter(|e| e.section.eq_ignore_ascii_case(&section))
        {
            let key = entry.key.to_uppercase();
            let value = entry.value.trim().parse::<u32>().unwrap_or(0);
            if key == "EVENT" {
                out.next = value as usize;
            } else if key == "POINTS" {
                out.points = value;
            } else if let Some(n) = key
                .strip_prefix("EVENT")
                .and_then(|n| n.parse::<usize>().ok())
                && n >= 1
                && value > 0
            {
                out.results.insert(n - 1, value);
            }
        }
        out
    }

    /// Whether a series is done.
    pub fn completed(&self, series: &Series) -> bool {
        self.complete
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&series.id))
    }

    /// Whether a series is open to drive: the series it requires done —
    /// all of them, or one with `REQUIRESANY`.
    pub fn open(&self, series: &Series) -> bool {
        let done = |id: &String| self.complete.iter().any(|c| c.eq_ignore_ascii_case(id));
        match (series.requires.is_empty(), series.requires_any) {
            (true, _) => true,
            (false, true) => series.requires.iter().any(done),
            (false, false) => series.requires.iter().all(done),
        }
    }
}

/// Whether a series' progress reaches its goal — Content Manager's rule: a
/// first counts for a second or a third the series asks for, a second for a
/// third; a championship needs its points and, if it names one, its place
/// after the last round.
pub fn reached(series: &Series, progress: &SeriesProgress) -> bool {
    match &series.kind {
        Kind::SingleEvents {
            firsts,
            seconds,
            thirds,
        } => {
            let count = |cup: u32| progress.results.values().filter(|v| **v == cup).count() as i64;
            let first = count(3) - i64::from(*firsts);
            if first < 0 {
                return false;
            }
            let second = count(2) + first - i64::from(*seconds);
            if second < 0 {
                return false;
            }
            count(1) + second - i64::from(*thirds) >= 0
        }
        Kind::Championship {
            points_goal,
            ranking_goal,
        } => {
            progress.points >= *points_goal
                && (*ranking_goal == 0 || progress.results.len() == series.events.len())
        }
    }
}

/// The `race.ini` for one of a series' events: its own file with the
/// driver's name, the special-event section the game would mistake it for
/// taken out, a championship's field filled from `opponents.ini` where the
/// event names fewer cars than it races, and replay, benchmark and online
/// switched off — as Content Manager's `KunosCareerEventObject` writes it.
pub fn race_ini(series: &Series, event: &Event, driver: &str) -> String {
    let mut text = remove_section(&event.text, "SPECIAL_EVENT");
    text = ini::with_value(&text, "CAR_0", "DRIVER_NAME", driver);
    if matches!(series.kind, Kind::Championship { .. }) {
        let cars: usize = ini::value(&text, "RACE", "CARS")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let opponents =
            std::fs::read_to_string(series.folder.join("opponents.ini")).unwrap_or_default();
        for k in 1..cars {
            let car = format!("CAR_{k}");
            let ai = format!("AI{k}");
            if ini::value(&text, &car, "MODEL").is_some() {
                continue;
            }
            for key in [
                "MODEL",
                "SKIN",
                "DRIVER_NAME",
                "NATIONALITY",
                "AI_LEVEL",
                "SETUP",
            ] {
                if let Some(value) = ini::value(&opponents, &ai, key) {
                    text = ini::with_value(&text, &car, key, &value);
                }
            }
        }
    }
    for section in ["REPLAY", "BENCHMARK", "REMOTE"] {
        text = ini::with_value(&text, section, "ACTIVE", "0");
    }
    text
}

fn remove_section(text: &str, section: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for line in text.lines() {
        if let Some(name) = line
            .trim()
            .strip_prefix('[')
            .and_then(|l| l.strip_suffix(']'))
        {
            skipping = name.trim().eq_ignore_ascii_case(section);
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// What a finished career event did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recorded {
    /// A single event's cup, written.
    Cup(Cup),
    /// A championship round's place and points, written.
    Place { place: u32, points: u32 },
    /// The series is done now, and written so.
    SeriesDone,
    /// Driven and short of a cup.
    NoCup,
    /// Nothing to record.
    Nothing,
}

/// Record a finished event of a series into the game's career file — the
/// result if better, the next event, the points, and the series done when
/// it is. The file is kept once beside itself before it is first changed.
pub fn record(
    game_documents: &Path,
    series: &Series,
    event_number: usize,
    outcome: &Outcome,
) -> Result<Vec<Recorded>, String> {
    let Some(event) = series.events.get(event_number) else {
        return Ok(vec![Recorded::Nothing]);
    };
    let mut progress = Progress::read(game_documents);
    let mut mine = progress.of(series);
    let mut said = Vec::new();
    let section = series.id.to_uppercase();
    let key = format!("EVENT{}", event_number + 1);
    let mut text = progress.text.clone();
    match &series.kind {
        Kind::SingleEvents { .. } => match special_events::judge(event, outcome, None) {
            Judged::Won(cup) => {
                let value = match cup {
                    Cup::Gold => 3,
                    Cup::Silver => 2,
                    Cup::Bronze => 1,
                };
                if mine
                    .results
                    .get(&event_number)
                    .is_none_or(|was| value > *was)
                {
                    mine.results.insert(event_number, value);
                    text = ini::with_value(&text, &section, &key, &value.to_string());
                }
                said.push(Recorded::Cup(cup));
            }
            Judged::NotYet => said.push(Recorded::NoCup),
            _ => return Ok(vec![Recorded::Nothing]),
        },
        Kind::Championship { .. } => {
            let place = outcome
                .main()
                .and_then(|s| s.race_result.iter().position(|car| *car == 0))
                .map(|p| p as u32 + 1);
            let Some(place) = place else {
                return Ok(vec![Recorded::Nothing]);
            };
            let earned = series.points.get(place as usize - 1).copied().unwrap_or(0);
            // A round counts once: driven again, its points are taken back
            // before the new ones are given.
            let before = mine
                .results
                .get(&event_number)
                .and_then(|p| series.points.get(*p as usize - 1))
                .copied()
                .unwrap_or(0);
            mine.points = mine.points.saturating_sub(before) + earned;
            mine.results.insert(event_number, place);
            text = ini::with_value(&text, &section, &key, &place.to_string());
            text = ini::with_value(&text, &section, "POINTS", &mine.points.to_string());
            said.push(Recorded::Place {
                place,
                points: earned,
            });
        }
    }
    let next = (event_number + 1).min(series.events.len().saturating_sub(1));
    if next > mine.next {
        text = ini::with_value(&text, &section, "EVENT", &next.to_string());
    }
    text = ini::with_value(&text, "CAREER", "CURRENTSERIES", &series.id);
    if reached(series, &mine) && !progress.completed(series) {
        progress.complete.push(series.id.clone());
        text = ini::with_value(&text, "CAREER", "COMPLETE", &progress.complete.join(","));
        said.push(Recorded::SeriesDone);
    }
    let file = progress_file(game_documents);
    let backup = file.with_extension("ini.backup");
    if file.is_file() && !backup.exists() {
        std::fs::copy(&file, &backup).map_err(|e| e.to_string())?;
    }
    crate::atomic_file::write_atomic(&file, text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(said)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn event_ini(kind: &str, objectives: [i64; 3]) -> String {
        let mut text = "[EVENT]\nNAME=Round\n[SPECIAL_EVENT]\nGUID=x\n[RACE]\nTRACK=monza\nCONFIG_TRACK=\nMODEL=ks_abarth500\nCARS=3\nAI_LEVEL=90\n\
             [SESSION_0]\nNAME=Race\nTYPE=3\n[CAR_0]\nSKIN=\nDRIVER_NAME=\n"
            .to_string();
        if !kind.is_empty() {
            for (k, v) in objectives.iter().enumerate() {
                text.push_str(&format!("[CONDITION_{k}]\nTYPE={kind}\nOBJECTIVE={v}\n"));
            }
        }
        text
    }

    fn a_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("acpe-career-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let single = root.join("content/career/series1");
        std::fs::create_dir_all(single.join("event1")).unwrap();
        std::fs::create_dir_all(single.join("event2")).unwrap();
        std::fs::write(
            single.join("series.ini"),
            "[SERIES]\nNAME=Abarth Cup\nCODE=A1\nDESCRIPTION=Learn.\\nDrive.\nPOINTS=0\n[GOALS]\nTIER1=0\nTIER2=0\nTIER3=1\n",
        )
        .unwrap();
        for n in 1..=2 {
            std::fs::write(
                single.join(format!("event{n}/event.ini")),
                event_ini("POSITION", [3, 2, 1]),
            )
            .unwrap();
        }
        let champ = root.join("content/career/series2");
        std::fs::create_dir_all(champ.join("event1")).unwrap();
        std::fs::write(
            champ.join("series.ini"),
            "[SERIES]\nNAME=GT Championship\nCODE=GT\nPOINTS=10,6,4\nREQUIRES=series1\n[GOALS]\nPOINTS=10\nRANKING=1\n",
        )
        .unwrap();
        std::fs::write(champ.join("event1/event.ini"), event_ini("", [0; 3])).unwrap();
        std::fs::write(
            champ.join("opponents.ini"),
            "[AI1]\nMODEL=ks_bmw\nSKIN=red\nDRIVER_NAME=A. Rival\nAI_LEVEL=92\n[AI2]\nMODEL=ks_audi\nDRIVER_NAME=B. Rival\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("content/career/series0/event1")).unwrap();
        root
    }

    fn won(place: usize) -> Outcome {
        let order = match place {
            1 => "[0,1,2]",
            2 => "[1,0,2]",
            _ => "[1,2,0]",
        };
        let json = format!(
            r#"{{"track":"monza","players":[{{"name":"Me","car":"x","skin":""}},{{"name":"B","car":"x","skin":""}},{{"name":"C","car":"x","skin":""}}],
            "sessions":[{{"name":"Race","type":3,"laps":[{{"lap":0,"car":0,"time":90000,"cuts":0,"sectors":[],"tyre":"M"}}],
            "lapsTotal":[3,3,3],"bestLaps":[{{"car":0,"time":90000,"lap":0}}],"raceResult":{order}}}]}}"#
        );
        super::super::results::parse(&json).unwrap()
    }

    #[test]
    fn series_are_read_in_order_and_series0_left_out() {
        let root = a_root("read");
        let all = all(&root);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "Abarth Cup");
        assert_eq!(all[0].description, "Learn.\nDrive.");
        assert_eq!(
            all[0].kind,
            Kind::SingleEvents {
                firsts: 1,
                seconds: 0,
                thirds: 0
            }
        );
        assert_eq!(all[0].events.len(), 2);
        assert_eq!(
            all[1].kind,
            Kind::Championship {
                points_goal: 10,
                ranking_goal: 1
            }
        );
        assert_eq!(all[1].requires, vec!["series1".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_championship_round_gets_its_field_from_opponents_ini() {
        let root = a_root("field");
        let series = &all(&root)[1];
        let ini = race_ini(series, &series.events[0], "Rgosh");
        assert_eq!(
            ini::value(&ini, "CAR_1", "MODEL").as_deref(),
            Some("ks_bmw")
        );
        assert_eq!(
            ini::value(&ini, "CAR_2", "DRIVER_NAME").as_deref(),
            Some("B. Rival")
        );
        assert_eq!(
            ini::value(&ini, "CAR_0", "DRIVER_NAME").as_deref(),
            Some("Rgosh")
        );
        assert!(
            ini::value(&ini, "SPECIAL_EVENT", "GUID").is_none(),
            "not a special event"
        );
        assert_eq!(ini::value(&ini, "REPLAY", "ACTIVE").as_deref(), Some("0"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// **The career moves on in the game's own file**: a cup, the next
    /// event, the series done and the next one opened — kept once first.
    #[test]
    fn a_career_moves_on_in_the_game_s_own_file() {
        let root = a_root("progress");
        let documents = root.join("Documents/Assetto Corsa");
        std::fs::create_dir_all(progress_file(&documents).parent().unwrap()).unwrap();
        std::fs::write(
            progress_file(&documents),
            "[CAREER]\nINTRO=2\nAI_LEVEL=97\n",
        )
        .unwrap();
        let all = all(&root);
        let (single, champ) = (&all[0], &all[1]);
        assert!(
            !Progress::read(&documents).open(champ),
            "locked until series1 is done"
        );

        let said = record(&documents, single, 0, &won(2)).unwrap();
        assert_eq!(said, vec![Recorded::Cup(Cup::Silver)]);
        let progress = Progress::read(&documents);
        assert_eq!(progress.of(single).results.get(&0), Some(&2));
        assert_eq!(progress.of(single).next, 1);
        assert!(!progress.completed(single), "a first is asked for");

        let said = record(&documents, single, 1, &won(1)).unwrap();
        assert!(said.contains(&Recorded::SeriesDone));
        let progress = Progress::read(&documents);
        assert!(progress.completed(single) && progress.open(champ));
        assert_eq!(progress.ai_level, 97.0, "the rest of the file kept");
        assert!(
            progress_file(&documents)
                .with_extension("ini.backup")
                .is_file()
        );

        // A championship round: points for the place, counted once.
        record(&documents, champ, 0, &won(2)).unwrap();
        assert_eq!(Progress::read(&documents).of(champ).points, 6);
        let said = record(&documents, champ, 0, &won(1)).unwrap();
        assert!(said.contains(&Recorded::Place {
            place: 1,
            points: 10
        }));
        assert_eq!(
            Progress::read(&documents).of(champ).points,
            10,
            "driven again, counted once"
        );
        assert!(Progress::read(&documents).completed(champ));
        let _ = std::fs::remove_dir_all(&root);
    }
}
