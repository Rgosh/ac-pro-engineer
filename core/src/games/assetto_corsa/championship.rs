//! A championship: rounds at several circuits in one car, points for where
//! each driver finished, and a table that carries over.
//!
//! **The result is the game's own.** After every session Assetto Corsa
//! writes `out/race_out.json` in its Documents folder: who drove what, every
//! lap, and `raceResult` — the cars by index in the order they finished.
//! Content Manager's championships are read from the same file. Nothing here
//! times a lap or decides a position; it adds up what the game wrote.
//!
//! **Drivers are told apart by name**, which is what the table shows and
//! what survives between rounds: the front end gives the game's drivers the
//! same names every round, so "K. Tanaka" in round three is the "K. Tanaka"
//! of round one. You are the game's car 0, whatever your name.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Points for first, second… — the scale Formula 1 has used since 2010,
/// and the one most people mean by "points".
pub const POINTS: &[u32] = &[25, 18, 15, 12, 10, 8, 6, 4, 2, 1];

/// Where the game writes the last session's result.
pub fn race_out(game_documents: &Path) -> PathBuf {
    game_documents.join("out").join("race_out.json")
}

/// One car in a result, as the game lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Player {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub car: String,
}

/// One session in a result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SessionOut {
    #[serde(default)]
    pub name: String,
    /// The game's number for it: 3 is a race.
    #[serde(default, rename = "type")]
    pub kind: u32,
    /// Car indices in the order they finished.
    #[serde(default, rename = "raceResult")]
    pub race_result: Vec<usize>,
    /// Laps completed, by car index.
    #[serde(default, rename = "lapstotal")]
    pub laps_total: Vec<u32>,
}

/// What the game wrote after a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RaceOut {
    #[serde(default)]
    pub track: String,
    #[serde(default)]
    pub players: Vec<Player>,
    #[serde(default)]
    pub sessions: Vec<SessionOut>,
}

impl RaceOut {
    /// The finishing order of the race, by name — the last session that is
    /// a race and has a result. `None` when no race was finished: a session
    /// left in practice is not a round driven.
    ///
    /// **A car that did not complete a lap is not classified**: it is left
    /// out of the order, so it scores nothing.
    pub fn finishing_order(&self) -> Option<Vec<String>> {
        let race = self
            .sessions
            .iter()
            .rev()
            .find(|s| s.kind == 3 && !s.race_result.is_empty())?;
        let order: Vec<String> = race
            .race_result
            .iter()
            .filter(|&&car| race.laps_total.get(car).is_none_or(|laps| *laps > 0))
            .filter_map(|&car| self.players.get(car).map(|p| p.name.clone()))
            .collect();
        (!order.is_empty()).then_some(order)
    }

    /// Your name in it: the game's car 0.
    pub fn you(&self) -> Option<&str> {
        self.players.first().map(|p| p.name.as_str())
    }
}

/// The game's result, read leniently. `None` when it is not one.
pub fn parse(json: &str) -> Option<RaceOut> {
    serde_json::from_str(json).ok()
}

/// One round: a circuit and its layout, and how it ended when it has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Round {
    pub track: String,
    pub layout: String,
    /// The classified finishers, first to last. Empty until it is driven.
    pub result: Vec<String>,
}

/// A championship.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Championship {
    pub name: String,
    /// The car you drive in every round, by folder.
    pub car: String,
    pub rounds: Vec<Round>,
    /// Your name in the results, from the first round driven.
    pub you: String,
}

/// One line of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    pub name: String,
    pub points: u32,
    pub wins: u32,
    /// Finishing places, round by round, `None` where not classified or not
    /// yet driven.
    pub places: Vec<Option<usize>>,
}

impl Championship {
    /// The next round to drive, by index.
    pub fn next(&self) -> Option<usize> {
        self.rounds.iter().position(|r| r.result.is_empty())
    }

    /// Whether every round has been driven.
    pub fn finished(&self) -> bool {
        !self.rounds.is_empty() && self.next().is_none()
    }

    /// Put a round's result in, from what the game wrote. Returns whether
    /// it counted: a session with no finished race is not a round.
    pub fn record(&mut self, round: usize, out: &RaceOut) -> bool {
        let Some(order) = out.finishing_order() else {
            return false;
        };
        if self.you.is_empty()
            && let Some(you) = out.you()
        {
            self.you = you.to_string();
        }
        match self.rounds.get_mut(round) {
            Some(slot) => {
                slot.result = order;
                true
            }
            None => false,
        }
    }

    /// The table: most points first, then most wins, then the better best
    /// finish — the tie-breaks a championship is usually settled by.
    pub fn standings(&self) -> Vec<Standing> {
        let mut table: Vec<Standing> = Vec::new();
        for (at, round) in self.rounds.iter().enumerate() {
            for (place, name) in round.result.iter().enumerate() {
                let index = match table.iter().position(|s| &s.name == name) {
                    Some(index) => index,
                    None => {
                        table.push(Standing {
                            name: name.clone(),
                            points: 0,
                            wins: 0,
                            places: vec![None; self.rounds.len()],
                        });
                        table.len() - 1
                    }
                };
                if let Some(line) = table.get_mut(index) {
                    line.points += POINTS.get(place).copied().unwrap_or(0);
                    line.wins += u32::from(place == 0);
                    if let Some(slot) = line.places.get_mut(at) {
                        *slot = Some(place + 1);
                    }
                }
            }
        }
        let best = |s: &Standing| {
            s.places
                .iter()
                .flatten()
                .min()
                .copied()
                .unwrap_or(usize::MAX)
        };
        table.sort_by(|a, b| {
            b.points
                .cmp(&a.points)
                .then(b.wins.cmp(&a.wins))
                .then(best(a).cmp(&best(b)))
                .then(a.name.cmp(&b.name))
        });
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A result as the game writes it, cut to what is read here: a
    /// practice, then a race of four where car 3 never completed a lap.
    const OUT: &str = r#"{
        "track": "ks_spa",
        "number_of_sessions": 2,
        "players": [
            {"name": "Rgosh", "car": "ks_ferrari_488_gt3", "skin": "red"},
            {"name": "A. Moreau", "car": "ks_ferrari_488_gt3", "skin": "blue"},
            {"name": "L. Becker", "car": "ks_ferrari_488_gt3", "skin": "white"},
            {"name": "K. Tanaka", "car": "ks_ferrari_488_gt3", "skin": "black"}
        ],
        "sessions": [
            {"event": 0, "name": "Practice", "type": 1, "raceResult": [], "lapstotal": [5, 4, 4, 3]},
            {"event": 1, "name": "Race", "type": 3, "lapsCount": 5,
             "raceResult": [1, 0, 3, 2], "lapstotal": [5, 5, 0, 5],
             "bestLaps": [{"car": 1, "time": 138000, "lap": 2}]}
        ]
    }"#;

    #[test]
    fn the_game_s_result_reads_into_an_order() {
        let Some(out) = parse(OUT) else {
            unreachable!("the game's shape must read");
        };
        assert_eq!(
            out.finishing_order(),
            Some(vec![
                "A. Moreau".to_string(),
                "Rgosh".to_string(),
                "K. Tanaka".to_string()
            ]),
            "L. Becker did no lap and is not classified"
        );
        assert_eq!(out.you(), Some("Rgosh"));
        // Only a practice: no round driven.
        let practice = parse(
            r#"{"players": [{"name": "Rgosh"}], "sessions": [{"type": 1, "raceResult": [0]}]}"#,
        );
        assert_eq!(practice.and_then(|o| o.finishing_order()), None);
        assert_eq!(parse("not json"), None);
    }

    /// **Points by place, wins to break a tie**, a round not yet driven
    /// scoring nothing, and the next round found.
    #[test]
    fn the_table_adds_up_round_by_round() {
        let mut season = Championship {
            name: "GT3 evenings".into(),
            car: "ks_ferrari_488_gt3".into(),
            rounds: ["ks_spa", "monza", "ks_brands_hatch"]
                .iter()
                .map(|t| Round {
                    track: t.to_string(),
                    ..Round::default()
                })
                .collect(),
            you: String::new(),
        };
        assert_eq!(season.next(), Some(0));
        let Some(out) = parse(OUT) else {
            unreachable!()
        };
        assert!(season.record(0, &out));
        assert_eq!(season.you, "Rgosh");
        // Round two: you win, Moreau third.
        let two = parse(
            r#"{"players": [{"name": "Rgosh"}, {"name": "A. Moreau"}, {"name": "K. Tanaka"}],
                "sessions": [{"type": 3, "raceResult": [0, 2, 1], "lapstotal": [5, 5, 5]}]}"#,
        );
        let Some(two) = two else { unreachable!() };
        assert!(season.record(1, &two));
        assert_eq!(season.next(), Some(2));
        assert!(!season.finished());
        let table = season.standings();
        let lines: Vec<(&str, u32, u32)> = table
            .iter()
            .map(|s| (s.name.as_str(), s.points, s.wins))
            .collect();
        // You 18 + 25 = 43; Moreau 25 + 15 = 40; Tanaka 15 + 18 = 33.
        assert_eq!(
            lines,
            vec![("Rgosh", 43, 1), ("A. Moreau", 40, 1), ("K. Tanaka", 33, 0)]
        );
        assert_eq!(
            table.first().map(|s| s.places.clone()),
            Some(vec![Some(2), Some(1), None])
        );
    }

    /// A tie on points goes to wins, then to the better best finish.
    #[test]
    fn a_tie_is_settled_by_wins() {
        let season = Championship {
            rounds: vec![
                Round {
                    result: vec!["B".into(), "A".into()],
                    ..Round::default()
                },
                Round {
                    result: vec!["A".into(), "C".into(), "B".into()],
                    ..Round::default()
                },
            ],
            ..Championship::default()
        };
        // A: 18 + 25 = 43, one win. B: 25 + 15 = 40.
        let table = season.standings();
        assert_eq!(table.first().map(|s| s.name.as_str()), Some("A"));
        assert!(season.finished());
    }
}
