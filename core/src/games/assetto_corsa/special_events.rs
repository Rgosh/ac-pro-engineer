//! Kunos's special events: challenges the game ships, each a fixed car,
//! circuit and conditions with a bronze, a silver and a gold to win.
//!
//! **Where they are and what they say, as the game and Content Manager read
//! them.** Each is a folder under `content/specialevents/` holding
//! `event.ini` and `preview.png`. The file is a `race.ini` already — the
//! circuit, the car, the session, the weather, the field — with three more
//! things in it: `[EVENT]` (its name and description), `[SPECIAL_EVENT]`
//! (its `GUID`) and `[CONDITION_0..2]`, the bronze, the silver and the gold,
//! each a `TYPE` and an `OBJECTIVE`. A `TYPE` of `AI` instead makes the
//! conditions a ladder of the game's drivers' strength.
//!
//! **Starting one is handing the game its own file**, as Content Manager
//! does (its `SpecialEventObject.ConvertConfig`): the file as it is, with the
//! driver's name, each condition's type in lower case and `ACHIEVED=0`, the
//! replay switched off, and the strength chosen on a ladder.
//!
//! **Which cup was won is judged here from the game's own result**
//! (`race_out.json`): a finishing place or a lap time against the objective.
//! The game keeps no record of it a launcher can read, and Content Manager
//! keeps its own; so does this. A kind of objective the result does not
//! carry — a drift score — is said to be the game's to show, never guessed.

use super::ini;
use super::results::Outcome;
use std::path::{Path, PathBuf};

/// What an objective measures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Goal {
    /// Finish at or above this place.
    Position,
    /// A lap at or under this many milliseconds.
    Time,
    /// A score the game counts itself (drift points), not in its result file.
    Points,
    /// The cups are the game's drivers' strength: a ladder, not a target.
    Ai,
    /// A type this program does not know, as written.
    Other(String),
}

impl Goal {
    fn of(text: &str) -> Goal {
        match text.trim().to_ascii_uppercase().as_str() {
            "POSITION" | "PLACE" => Goal::Position,
            "TIME" | "LAPTIME" | "BEST_LAP" => Goal::Time,
            "DRIFT" | "POINTS" | "SCORE" => Goal::Points,
            "AI" => Goal::Ai,
            other => Goal::Other(other.to_string()),
        }
    }
}

/// One special event.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Its folder's name.
    pub id: String,
    pub folder: PathBuf,
    pub name: String,
    pub description: String,
    pub guid: String,
    pub car: String,
    pub skin: String,
    pub track: String,
    pub track_config: String,
    pub weather: String,
    /// Opponents beside you.
    pub opponents: u32,
    pub laps: Option<u32>,
    pub goal: Goal,
    /// The objectives, bronze first — or, for a ladder, its strengths.
    pub objectives: Vec<i64>,
    /// The `event.ini` as written, for starting it.
    pub text: String,
}

/// The description as the game writes it: `\n` and `<br>` are line breaks.
fn decode(text: &str) -> String {
    text.replace("\\n", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .trim()
        .to_string()
}

/// One event's file read, or `None` when it is not one.
pub fn parse(id: &str, folder: &Path, text: &str) -> Option<Event> {
    let get = |section: &str, key: &str| ini::value(text, section, key).unwrap_or_default();
    let track = get("RACE", "TRACK");
    let car = get("RACE", "MODEL");
    if track.is_empty() || car.is_empty() {
        return None;
    }
    let conditions: Vec<(Goal, i64)> = (0..4)
        .map_while(|k| {
            let section = format!("CONDITION_{k}");
            let kind = ini::value(text, &section, "TYPE")?;
            let objective = ini::value(text, &section, "OBJECTIVE")?
                .split_whitespace()
                .next()?
                .parse::<f64>()
                .ok()?;
            Some((Goal::of(&kind), objective as i64))
        })
        .collect();
    let goal = conditions
        .first()
        .map_or(Goal::Other(String::new()), |(g, _)| g.clone());
    let objectives = match goal {
        Goal::Ai => conditions.iter().map(|(_, v)| *v).collect(),
        // Three of one kind, or nothing a cup can be judged by.
        _ if conditions.len() >= 3 && conditions[..3].iter().all(|(g, _)| *g == goal) => {
            conditions[..3].iter().map(|(_, v)| *v).collect()
        }
        _ => Vec::new(),
    };
    let number = |section: &str, key: &str| {
        ini::value(text, section, key)
            .and_then(|v| v.split_whitespace().next()?.parse::<u32>().ok())
    };
    Some(Event {
        id: id.to_string(),
        folder: folder.to_path_buf(),
        name: match get("EVENT", "NAME") {
            name if name.is_empty() => id.to_string(),
            name => name,
        },
        description: decode(&get("EVENT", "DESCRIPTION")),
        guid: get("SPECIAL_EVENT", "GUID"),
        car,
        skin: get("CAR_0", "SKIN"),
        track,
        track_config: get("RACE", "CONFIG_TRACK"),
        weather: get("WEATHER", "NAME"),
        opponents: number("RACE", "CARS").unwrap_or(1).saturating_sub(1),
        laps: number("SESSION_0", "LAPS")
            .or_else(|| number("RACE", "RACE_LAPS"))
            .filter(|l| *l > 0),
        goal,
        objectives,
        text: text.to_string(),
    })
}

/// Every special event installed, in the order of their folders' names —
/// the order Kunos numbered them in.
pub fn all(root: &Path) -> Vec<Event> {
    let at = root.join("content").join("specialevents");
    let mut found: Vec<Event> = std::fs::read_dir(&at)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(e.path().join("event.ini")).ok()?;
            parse(&id, &e.path(), &text)
        })
        .collect();
    found.sort_by_key(|e| natural(&e.id));
    found
}

/// A name with its number compared as a number: `se2` before `se10`.
fn natural(id: &str) -> (String, u64) {
    let digits: String = id.chars().rev().take_while(char::is_ascii_digit).collect();
    let number = digits
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .unwrap_or(0);
    (id[..id.len() - digits.len()].to_lowercase(), number)
}

/// The event's picture, where the game keeps it.
pub fn preview(event: &Event) -> PathBuf {
    event.folder.join("preview.png")
}

/// The `race.ini` that starts the event: its own file, with the driver's
/// name, each condition's type lower-cased and not yet achieved, the replay
/// off — and on a ladder, the strength chosen from it.
pub fn race_ini(event: &Event, driver: &str, ladder_step: Option<usize>) -> String {
    let mut text = event.text.clone();
    text = ini::with_value(&text, "CAR_0", "DRIVER_NAME", driver);
    for k in 0..4 {
        let section = format!("CONDITION_{k}");
        if let Some(kind) = ini::value(&text, &section, "TYPE") {
            text = ini::with_value(&text, &section, "TYPE", &kind.to_lowercase());
            text = ini::with_value(&text, &section, "ACHIEVED", "0");
        }
    }
    if event.goal == Goal::Ai
        && let Some(level) = ladder_step.and_then(|k| event.objectives.get(k))
    {
        text = ini::with_value(&text, "RACE", "AI_LEVEL", &level.to_string());
    }
    text = ini::with_value(&text, "REPLAY", "ACTIVE", "0");
    text = ini::with_value(&text, "REMOTE", "ACTIVE", "0");
    text
}

/// A cup: bronze, silver, gold.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Cup {
    Bronze,
    Silver,
    Gold,
}

/// What a finished session earned, judged from the game's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// This cup, the best of the three reached.
    Won(Cup),
    /// Driven, and short of bronze.
    NotYet,
    /// The result does not carry what this event is judged by.
    TheGameSays,
    /// Nothing driven: no result to judge.
    NoResult,
}

/// Your finishing place and best lap in a result — you are the first car.
fn yours(outcome: &Outcome) -> Option<(Option<usize>, Option<u64>)> {
    let session = outcome.main()?;
    let place = session
        .race_result
        .iter()
        .position(|car| *car == 0)
        .map(|p| p + 1);
    Some((place, session.best_of(0)))
}

/// Which cup a result earns. On a ladder, a win at the step chosen earns
/// that step's cup: the first bronze, the second silver, the third gold and
/// any harder one gold.
pub fn judge(event: &Event, outcome: &Outcome, ladder_step: Option<usize>) -> Judged {
    let Some((place, best)) = yours(outcome) else {
        return Judged::NoResult;
    };
    let cups = [Cup::Bronze, Cup::Silver, Cup::Gold];
    let reached = |ok: &dyn Fn(i64) -> bool| -> Judged {
        let won = event
            .objectives
            .iter()
            .zip(cups)
            .filter(|(target, _)| ok(**target))
            .map(|(_, cup)| cup)
            .max();
        won.map_or(Judged::NotYet, Judged::Won)
    };
    match event.goal {
        _ if event.objectives.is_empty() => Judged::TheGameSays,
        Goal::Position => match place {
            Some(place) => reached(&|target| place as i64 <= target),
            None => Judged::NoResult,
        },
        Goal::Time => match best {
            Some(best) => reached(&|target| best as i64 <= target),
            None => Judged::NotYet,
        },
        Goal::Ai => match (place, ladder_step) {
            (Some(1), Some(step)) => Judged::Won(cups[step.min(2)]),
            (Some(_), _) => Judged::NotYet,
            _ => Judged::NoResult,
        },
        Goal::Points | Goal::Other(_) => Judged::TheGameSays,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const EVENT: &str = "[EVENT]\nNAME=Hot lap at Monza\nDESCRIPTION=Beat the clock.\\nThen beat it again.\n\
        [SPECIAL_EVENT]\nGUID=1234\n[RACE]\nTRACK=monza\nCONFIG_TRACK=\nMODEL=ks_ferrari_458\nCARS=1\nAI_LEVEL=90\n\
        [SESSION_0]\nNAME=Hotlap\nTYPE=4\n[CAR_0]\nSKIN=00_rosso\nDRIVER_NAME=\n[WEATHER]\nNAME=3_clear\n\
        [CONDITION_0]\nTYPE=TIME\nOBJECTIVE=112000\n[CONDITION_1]\nTYPE=TIME\nOBJECTIVE=110000\n\
        [CONDITION_2]\nTYPE=TIME\nOBJECTIVE=108500\n";

    fn outcome(place: Option<usize>, best: Option<i64>) -> Outcome {
        let json = format!(
            r#"{{"track":"monza","players":[{{"name":"You","car":"x","skin":""}},{{"name":"B","car":"x","skin":""}}],
            "sessions":[{{"name":"Race","type":3,"laps":[{{"lap":0,"car":0,"time":{t},"cuts":0,"sectors":[],"tyre":"M"}}],
            "lapsTotal":[3,3],"bestLaps":{best},"raceResult":{order}}}]}}"#,
            t = best.unwrap_or(120_000),
            best = match best {
                Some(b) => format!(r#"[{{"car":0,"time":{b},"lap":0}}]"#),
                None => "[]".to_string(),
            },
            order = match place {
                Some(1) => "[0,1]",
                Some(_) => "[1,0]",
                None => "[]",
            },
        );
        super::super::results::parse(&json).unwrap()
    }

    #[test]
    fn an_event_is_read_as_the_game_writes_it() {
        let e = parse("se1", Path::new("/x/se1"), EVENT).unwrap();
        assert_eq!(e.name, "Hot lap at Monza");
        assert_eq!(e.description, "Beat the clock.\nThen beat it again.");
        assert_eq!(
            (e.car.as_str(), e.skin.as_str(), e.track.as_str()),
            ("ks_ferrari_458", "00_rosso", "monza")
        );
        assert_eq!(e.goal, Goal::Time);
        assert_eq!(e.objectives, vec![112000, 110000, 108500], "bronze first");
        assert_eq!(e.guid, "1234");
        assert!(parse("x", Path::new("/x"), "[EVENT]\nNAME=No car\n").is_none());
    }

    #[test]
    fn starting_it_hands_the_game_its_own_file() {
        let e = parse("se1", Path::new("/x/se1"), EVENT).unwrap();
        let ini = race_ini(&e, "Rgosh", None);
        assert_eq!(
            ini::value(&ini, "CAR_0", "DRIVER_NAME").as_deref(),
            Some("Rgosh")
        );
        assert_eq!(
            ini::value(&ini, "CONDITION_2", "TYPE").as_deref(),
            Some("time")
        );
        assert_eq!(
            ini::value(&ini, "CONDITION_0", "ACHIEVED").as_deref(),
            Some("0")
        );
        assert_eq!(
            ini::value(&ini, "SPECIAL_EVENT", "GUID").as_deref(),
            Some("1234")
        );
        assert_eq!(ini::value(&ini, "REPLAY", "ACTIVE").as_deref(), Some("0"));
        assert_eq!(ini::value(&ini, "RACE", "TRACK").as_deref(), Some("monza"));
    }

    #[test]
    fn a_lap_time_earns_the_cups_it_beats() {
        let e = parse("se1", Path::new("/x/se1"), EVENT).unwrap();
        assert_eq!(
            judge(&e, &outcome(Some(1), Some(109_000)), None),
            Judged::Won(Cup::Silver)
        );
        assert_eq!(
            judge(&e, &outcome(Some(1), Some(108_000)), None),
            Judged::Won(Cup::Gold)
        );
        assert_eq!(
            judge(&e, &outcome(Some(1), Some(115_000)), None),
            Judged::NotYet
        );
    }

    #[test]
    fn a_place_and_a_ladder_are_judged_too() {
        let race = EVENT
            .replace("TYPE=TIME\nOBJECTIVE=112000", "TYPE=POSITION\nOBJECTIVE=3")
            .replace("TYPE=TIME\nOBJECTIVE=110000", "TYPE=POSITION\nOBJECTIVE=2")
            .replace("TYPE=TIME\nOBJECTIVE=108500", "TYPE=POSITION\nOBJECTIVE=1");
        let e = parse("se2", Path::new("/x"), &race).unwrap();
        assert_eq!(
            judge(&e, &outcome(Some(1), None), None),
            Judged::Won(Cup::Gold)
        );
        assert_eq!(
            judge(&e, &outcome(Some(2), None), None),
            Judged::Won(Cup::Silver)
        );

        let ladder = race.replace("TYPE=POSITION", "TYPE=AI");
        let e = parse("se3", Path::new("/x"), &ladder).unwrap();
        assert_eq!(e.goal, Goal::Ai);
        assert_eq!(
            judge(&e, &outcome(Some(1), None), Some(1)),
            Judged::Won(Cup::Silver)
        );
        assert_eq!(judge(&e, &outcome(Some(2), None), Some(2)), Judged::NotYet);
        let ini = race_ini(&e, "Me", Some(2));
        assert_eq!(ini::value(&ini, "RACE", "AI_LEVEL").as_deref(), Some("1"));

        let drift = EVENT.replace("TYPE=TIME", "TYPE=DRIFT");
        let e = parse("se4", Path::new("/x"), &drift).unwrap();
        assert_eq!(
            judge(&e, &outcome(Some(1), Some(1)), None),
            Judged::TheGameSays
        );
    }

    #[test]
    fn events_come_in_kunos_s_order() {
        let mut ids = ["se10", "se2", "se1"];
        ids.sort_by_key(|id| natural(id));
        assert_eq!(ids, ["se1", "se2", "se10"]);
    }
}
