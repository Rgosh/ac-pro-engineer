//! Assetto Corsa's public server list.
//!
//! Every server that registers with Kunos's lobby is listed at
//! `http://<lobby>/lobby.ashx/list`, as one JSON array — the list Content
//! Manager's online tab and the game's own are both built from. The field
//! names are the lobby's (`cport` is the server's HTTP port, `tport` the one
//! the game connects to); this module turns them into names a front end can
//! read and answers the two questions every server browser has to: *which of
//! these circuits and cars do I have*, and *how do I join it*.
//!
//! **Nothing here holds an account or signs anybody in**, but the lobby does
//! want to know who is asking. It answers only the game's own client: the
//! request has to carry the game's User-Agent and `guid=` with the Steam id
//! of whoever is signed in, which is what `acs.exe` and Content Manager both
//! send. Asked any other way it redirects to `/`, and a launcher that reads
//! that as "no servers" shows an empty list for a lobby with thousands on it
//! — which is what this module did until the lobby started checking.

use serde::Deserialize;
use std::time::Duration;

/// The lobby hosts tried in turn when nobody has said otherwise.
pub const DEFAULT_HOSTS: &[&str] = &["93.57.10.21"];

/// What the game's own launcher calls itself, and the only client the lobby
/// lists servers for. Measured: the same request as `Pro-Engineer-Lobby`, or
/// as curl, is a `302` to `/` with or without a `guid`.
pub const USER_AGENT: &str = "Assetto Corsa Launcher";

/// The address the list is asked for at.
///
/// **`guid` is the signed-in Steam id, and the lobby will not list without
/// it**: no `guid` is a fifteen-byte `UNKNOWN FAILURE`, and one that is not a
/// real account is a database error. `None` still builds the address, so the
/// failure is the lobby's own words rather than a guess here.
pub fn list_url(host: &str, guid: Option<&str>) -> String {
    match guid {
        Some(id) => format!("http://{host}/lobby.ashx/list?guid={id}"),
        None => format!("http://{host}/lobby.ashx/list"),
    }
}

/// One server, as the lobby lists it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Listing {
    #[serde(default)]
    pub ip: String,
    /// The port the lobby knows it by.
    #[serde(default)]
    pub port: u16,
    /// The server's HTTP port — `SERVER_HTTP_PORT` in the session.
    #[serde(default, rename = "cport")]
    pub http: u16,
    /// The port the game connects to — `SERVER_PORT` in the session.
    #[serde(default, rename = "tport")]
    pub race: u16,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub clients: u32,
    #[serde(default, rename = "maxclients")]
    pub capacity: u32,
    /// The circuit and its layout, joined by a dash: `ks_nordschleife-endurance`.
    #[serde(default)]
    pub track: String,
    #[serde(default)]
    pub cars: Vec<String>,
    /// The session running now, as the game numbers them (1 practice, 2
    /// qualifying, 3 race, 4 hotlap…).
    #[serde(default)]
    pub session: u32,
    #[serde(default, rename = "sessiontypes", deserialize_with = "numbers_or_text")]
    pub sessions: Vec<u32>,
    #[serde(default, rename = "timeleft")]
    pub time_left: i64,
    /// `[name, code]`.
    #[serde(default)]
    pub country: Vec<String>,
    #[serde(default, rename = "pass")]
    pub password: bool,
    /// Whether anybody may simply join, rather than book a place first.
    #[serde(default)]
    pub pickup: bool,
    /// How long each session in `sessions` lasts, in the same order — see
    /// [`Listing::planned`] for what the numbers mean.
    #[serde(default, deserialize_with = "numbers_or_text")]
    pub durations: Vec<u32>,
    /// Whether the race is run to a time rather than a number of laps.
    #[serde(default)]
    pub timed: bool,
    /// Whether one more lap is run after the race's time is up.
    #[serde(default)]
    pub extra: bool,
    /// Whether a pit stop is required in the race.
    #[serde(default)]
    pub pit: bool,
    /// How many of the qualifying order start the race reversed.
    #[serde(default, deserialize_with = "number_or_text")]
    pub inverted: u32,
}

/// How long a session lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Length {
    Minutes(u32),
    Laps(u32),
    /// The server did not say.
    Unknown,
}

/// One session of what a server runs, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Planned {
    /// The game's number for it (1 practice, 2 qualifying, 3 race…).
    pub kind: u32,
    pub length: Length,
    /// Whether it is the one running now.
    pub now: bool,
}

/// A session's name, from the game's number for it.
pub fn session_kind(kind: u32) -> &'static str {
    match kind {
        0 => "Booking",
        1 => "Practice",
        2 => "Qualifying",
        3 => "Race",
        4 => "Hotlap",
        5 => "Time attack",
        6 => "Drift",
        7 => "Drag",
        _ => "Session",
    }
}

impl Listing {
    /// The server's name without the marker Content Manager's servers end it
    /// with — `ℹ` and a port, where extended details are served. Drawn as it
    /// came, it read "STEWARDED | discord.gg/… ⓘ9013".
    pub fn shown_name(&self) -> &str {
        split_details(&self.name).0
    }

    /// The port that marker names, where a server says more about itself
    /// than the lobby does.
    pub fn details_port(&self) -> Option<u16> {
        split_details(&self.name).1
    }

    /// The circuit as installed folders are named, without the patch's
    /// marker.
    ///
    /// **A server that requires Custom Shaders Patch says so in its track.**
    /// It lists `csp/<build>/../<circuit>` (sometimes with a flag between,
    /// `csp/2651/../H/../<circuit>`): a game without the patch reads that as
    /// a folder that is not there and refuses to join, which is the point,
    /// and the patch reads the build it needs from it. What is installed is
    /// the part after the last `../`.
    pub fn track_id(&self) -> &str {
        match self.track.strip_prefix("csp/") {
            Some(rest) => rest.rsplit("../").next().unwrap_or(rest),
            None => &self.track,
        }
    }

    /// The patch build a server requires, when it requires one.
    pub fn needs_csp(&self) -> Option<u32> {
        let rest = self.track.strip_prefix("csp/")?;
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok().or(Some(0))
    }

    /// Where the circuit's folder name ends and its layout begins, given the
    /// circuits installed here.
    ///
    /// **Asked of what is installed, because the dash is ambiguous.** Folder
    /// names have dashes and underscores in them too, so `rt_suzuka-short` and
    /// a circuit literally called `rt_suzuka-short` read the same. The longest
    /// installed id the string starts with is the circuit; what is left after
    /// its dash is the layout. With nothing installed that matches, the part
    /// before the first dash is the best guess there is.
    pub fn circuit(&self, installed: &[&str]) -> (String, String) {
        let track = self.track_id();
        let best = installed
            .iter()
            .filter(|id| {
                track == **id
                    || track
                        .strip_prefix(**id)
                        .is_some_and(|rest| rest.starts_with('-'))
            })
            .max_by_key(|id| id.len());
        match best {
            Some(id) => (
                id.to_string(),
                track.get(id.len() + 1..).unwrap_or_default().to_string(),
            ),
            None => match track.split_once('-') {
                Some((id, layout)) => (id.to_string(), layout.to_string()),
                None => (track.to_string(), String::new()),
            },
        }
    }

    /// What this server runs that is not installed here: the circuit, if it is
    /// missing, and every car that is.
    pub fn missing(
        &self,
        has_track: impl Fn(&str) -> bool,
        has_car: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        let mut out = Vec::new();
        let (circuit, _) = self.circuit(&[]);
        if !has_track(self.track_id()) && !has_track(&circuit) {
            out.push(circuit);
        }
        out.extend(self.cars.iter().filter(|car| !has_car(car)).cloned());
        out
    }

    /// The session running now, in words.
    pub fn session_name(&self) -> &'static str {
        session_kind(self.session)
    }

    /// Every session the server runs, in order, and how long each lasts.
    ///
    /// **The lobby writes every length in seconds but one**: a race run to
    /// laps is its number of laps. `timed` should say which a race is, and
    /// does not always — the live lobby lists a traffic server's race as
    /// `5400` with `timed` false, which is ninety minutes and not five
    /// thousand laps. Nobody runs six hundred laps, so a race at or above
    /// that is read as seconds whatever `timed` says.
    pub fn planned(&self) -> Vec<Planned> {
        self.sessions
            .iter()
            .enumerate()
            .map(|(at, &kind)| {
                let length = match self.durations.get(at).copied() {
                    None | Some(0) => Length::Unknown,
                    Some(laps) if kind == 3 && !self.timed && laps < 600 => Length::Laps(laps),
                    Some(seconds) => Length::Minutes(seconds.div_ceil(60)),
                };
                Planned {
                    kind,
                    length,
                    now: kind == self.session,
                }
            })
            .collect()
    }

    /// The country code, when the lobby gave one.
    pub fn country_code(&self) -> &str {
        self.country.get(1).map(String::as_str).unwrap_or("")
    }
}

/// A list of numbers that the lobby may write as numbers or as text.
///
/// **It writes `sessiontypes` as text** — `["1","3"]` — and a `Vec<u32>`
/// refuses that, which made every one of the nine thousand entries fail to
/// parse and the whole list read as no servers. Measured on the live lobby:
/// every entry, every time. A value that is neither is left out rather than
/// costing the server its place in the list.
fn numbers_or_text<'de, D>(from: D) -> Result<Vec<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(from)?;
    Ok(values
        .iter()
        .filter_map(|value| match value {
            serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
            serde_json::Value::String(text) => text.trim().parse().ok(),
            _ => None,
        })
        .collect())
}

/// A number the lobby may write as a number or as text, or leave out.
fn number_or_text<'de, D>(from: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(from)?;
    Ok(match value {
        serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .unwrap_or(0))
}

/// The lobby's answer, read leniently: an entry that does not parse is left
/// out rather than costing the whole list.
pub fn parse(json: &str) -> Vec<Listing> {
    let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(json) else {
        return Vec::new();
    };
    values
        .into_iter()
        .filter_map(|value| serde_json::from_value::<Listing>(value).ok())
        .filter(|listing| !listing.ip.is_empty() && listing.http > 0)
        .collect()
}

/// What the lobby said, when it answered with a refusal rather than a list.
///
/// Its refusals are one line of text, `ERROR,` and a reason in capitals, or
/// `UNKNOWN FAILURE`. Anything that is not JSON and is short enough to be a
/// sentence is passed on as it is, without the `ERROR,`.
pub fn refusal(answer: &str) -> Option<String> {
    let answer = answer.trim();
    if answer.is_empty() || answer.starts_with('[') || answer.len() > 200 {
        return None;
    }
    let said = answer.strip_prefix("ERROR,").unwrap_or(answer);
    Some(said.replace(',', " — "))
}

/// Ask each host in turn for the list, and return the first that answers.
///
/// **Blocking, so called on a thread** — never from anything that draws.
///
/// The list is the whole world's servers — nine thousand of them, about
/// 7.5 MB of JSON, measured at four to twelve seconds. Allow thirty in
/// `timeout`. (The lobby would gzip it to a fifth, but reqwest's `gzip`
/// feature adds crates to every lockfile that builds this core, and the
/// window's CI builds `--locked`.)
pub fn fetch(hosts: &[String], timeout: Duration) -> Result<Vec<Listing>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|why| format!("could not make a client: {why}"))?;
    let guid = crate::steam::signed_in().map(|account| account.id64);
    let mut last = match guid {
        Some(_) => "no lobby host to ask".to_string(),
        None => "Steam is not signed in on this machine, and the lobby lists servers \
                 only for a Steam account"
            .to_string(),
    };
    for host in hosts {
        let url = list_url(host, guid.as_deref());
        match client
            .get(&url)
            .send()
            .and_then(|answer| answer.error_for_status())
        {
            Ok(answer) => match answer.text() {
                Ok(text) => {
                    let listed = parse(&text);
                    if !listed.is_empty() {
                        return Ok(listed);
                    }
                    // **The lobby's own words, when it has any.** It refuses
                    // in plain text — `ERROR,SLOW DOWN,TOO MANY CONCURRENT
                    // REQUESTS` — and "no servers" is a wrong account of that.
                    if let Some(said) = refusal(&text) {
                        last = format!("{host}: {said}");
                        continue;
                    }
                    last = match guid {
                        Some(_) => format!("{host} answered with no servers"),
                        None => format!(
                            "{host} answered with no servers — it lists them only for a \
                             signed-in Steam account, and none was found here"
                        ),
                    };
                }
                Err(why) => last = format!("{host}: {why}"),
            },
            Err(why) => last = format!("{host}: {why}"),
        }
    }
    Err(last)
}

/// A name and the details port at its end, when it has one: `ℹ` (or the
/// circled `ⓘ` some servers type instead) followed only by digits.
fn split_details(name: &str) -> (&str, Option<u16>) {
    let trimmed = name.trim_end();
    for marker in ['\u{2139}', '\u{24D8}'] {
        if let Some(at) = trimmed.rfind(marker) {
            let after = trimmed.get(at + marker.len_utf8()..).unwrap_or("").trim();
            if !after.is_empty() && after.chars().all(|c| c.is_ascii_digit()) {
                let before = trimmed.get(..at).unwrap_or(trimmed).trim_end();
                return (before, after.parse().ok());
            }
        }
    }
    (trimmed, None)
}

#[cfg(test)]
mod tests {
    #[test]
    fn content_managers_details_marker_is_taken_off_the_name() {
        let mut one = super::Listing {
            name: "AE001| FULL FORCE SIM | MONZA ℹ9013".to_string(),
            ..super::Listing::default()
        };
        assert_eq!(one.shown_name(), "AE001| FULL FORCE SIM | MONZA");
        assert_eq!(one.details_port(), Some(9013));
        one.name = "Circled \u{24D8}8081 ".to_string();
        assert_eq!(one.shown_name(), "Circled");
        assert_eq!(one.details_port(), Some(8081));
        // An ℹ that is not a port is part of the name.
        one.name = "Info ℹ here".to_string();
        assert_eq!(one.shown_name(), "Info ℹ here");
        assert_eq!(one.details_port(), None);
    }

    /// **A server that requires the patch is said to**, and its circuit is
    /// still found under its own folder name.
    #[test]
    fn a_server_that_needs_the_patch_is_told_apart() {
        let mut listing = Listing {
            ip: String::new(),
            port: 0,
            http: 0,
            race: 0,
            name: String::new(),
            clients: 0,
            capacity: 0,
            track: "csp/2651/../H/../ks_nordschleife-endurance".into(),
            cars: Vec::new(),
            session: 0,
            sessions: Vec::new(),
            time_left: 0,
            country: Vec::new(),
            password: false,
            pickup: false,
            ..Listing::default()
        };
        assert_eq!(listing.needs_csp(), Some(2651));
        assert_eq!(listing.track_id(), "ks_nordschleife-endurance");
        assert_eq!(
            listing.circuit(&["ks_nordschleife"]),
            ("ks_nordschleife".to_string(), "endurance".to_string())
        );
        assert!(
            listing
                .missing(|t| t == "ks_nordschleife", |_| true)
                .is_empty()
        );
        listing.track = "csp/2144/../imola".into();
        assert_eq!(listing.track_id(), "imola");
        listing.track = "spa".into();
        assert_eq!(listing.needs_csp(), None);
        assert_eq!(listing.track_id(), "spa");
    }

    use super::*;

    const LIST: &str = r#"[
        {"ip":"203.0.113.10","port":9600,"cport":8081,"tport":9600,"name":"Friday GT3 · Spa",
         "clients":17,"maxclients":24,"track":"ks_spa","cars":["ks_ferrari_488_gt3","ks_audi_r8_lms_2016"],
         "timeofday":-16,"session":3,"sessiontypes":[1,2,3],"durations":[600,600,1800],"timeleft":1200,
         "country":["Belgium","BE"],"pass":false,"timestamp":0,"pickup":true,"l":false},
        {"ip":"198.51.100.4","port":9700,"cport":8091,"tport":9700,"name":"Nordschleife Touristen",
         "clients":3,"maxclients":40,"track":"ks_nordschleife-touristenfahrten","cars":["ks_mazda_miata","mod_unknown"],
         "session":1,"sessiontypes":[1],"country":["Germany","DE"],"pass":true,"pickup":true},
        {"name":"broken entry with no address"},
        "not even an object"
    ]"#;

    /// The lobby redirects anything that does not look like the game, and a
    /// redirect followed to `/` is a page of HTML that parses as no servers.
    #[test]
    fn the_list_is_asked_for_the_way_the_game_asks() {
        assert_eq!(USER_AGENT, "Assetto Corsa Launcher");
        assert_eq!(
            list_url("93.57.10.21", Some("76561199148681321")),
            "http://93.57.10.21/lobby.ashx/list?guid=76561199148681321"
        );
        assert_eq!(
            list_url("93.57.10.21", None),
            "http://93.57.10.21/lobby.ashx/list"
        );
    }

    /// One entry exactly as the live lobby wrote it on 1 November 2026,
    /// name and address made up: session types as text, a `timeleft` past
    /// `u32`, and the fields this does not read.
    #[test]
    fn an_entry_as_the_lobby_really_writes_it_is_read() {
        let live = r#"[{"ip":"203.0.113.7","port":9713,"cport":8113,"tport":9713,
            "name":"Long Island | Traffic","clients":3,"maxclients":75,
            "track":"long_island_noz-long_island","cars":["bmw_m5_g90"],
            "timeofday":-80,"session":3,"sessiontypes":["1","3"],
            "durations":[7200,5400],"timeleft":4294967292,"country":["Germany","DE"],
            "pass":false,"timestamp":0,"json":null,"l":false,"pickup":true,
            "timed":false,"extra":false,"pit":false,"inverted":0}]"#;
        let listed = parse(live);
        assert_eq!(listed.len(), 1, "the live lobby's shape must parse");
        assert_eq!(listed[0].sessions, vec![1, 3]);
        assert_eq!(listed[0].http, 8113);
    }

    /// **Every length in seconds but a race's laps** — and a race the
    /// lobby calls untimed at ninety minutes is ninety minutes.
    #[test]
    fn the_sessions_a_server_runs_and_how_long_each_lasts() {
        let listed = parse(LIST);
        let spa: Vec<Planned> = listed[0].planned();
        assert_eq!(
            spa,
            vec![
                Planned {
                    kind: 1,
                    length: Length::Minutes(10),
                    now: false
                },
                Planned {
                    kind: 2,
                    length: Length::Minutes(10),
                    now: false
                },
                Planned {
                    kind: 3,
                    length: Length::Minutes(30),
                    now: true
                },
            ]
        );
        let mut laps = listed[0].clone();
        laps.durations = vec![600, 600, 15];
        assert_eq!(
            laps.planned().last().map(|p| p.length),
            Some(Length::Laps(15))
        );
        laps.timed = true;
        laps.durations = vec![600, 600, 15];
        assert_eq!(
            laps.planned().last().map(|p| p.length),
            Some(Length::Minutes(1))
        );
        laps.durations.clear();
        assert_eq!(
            laps.planned().first().map(|p| p.length),
            Some(Length::Unknown)
        );
        assert_eq!(session_kind(2), "Qualifying");
    }

    #[test]
    fn a_refusal_is_reported_in_the_lobby_s_own_words() {
        assert_eq!(
            refusal("ERROR,SLOW DOWN,TOO MANY CONCURRENT REQUESTS").as_deref(),
            Some("SLOW DOWN — TOO MANY CONCURRENT REQUESTS")
        );
        assert_eq!(
            refusal("UNKNOWN FAILURE").as_deref(),
            Some("UNKNOWN FAILURE")
        );
        assert_eq!(refusal(LIST), None);
        assert_eq!(refusal("  "), None);
    }

    #[test]
    fn the_list_reads_and_a_broken_entry_costs_only_itself() {
        let listed = parse(LIST);
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].http, 8081);
        assert_eq!(listed[0].race, 9600);
        assert_eq!(listed[0].capacity, 24);
        assert_eq!(listed[0].session_name(), "Race");
        assert_eq!(listed[0].country_code(), "BE");
        assert!(listed[1].password);
    }

    #[test]
    fn the_circuit_is_the_longest_installed_id_and_the_rest_is_the_layout() {
        let listed = parse(LIST);
        let installed = ["ks_nordschleife", "ks_spa", "ks_nord"];
        assert_eq!(
            listed[1].circuit(&installed),
            (
                "ks_nordschleife".to_string(),
                "touristenfahrten".to_string()
            )
        );
        assert_eq!(
            listed[0].circuit(&installed),
            ("ks_spa".to_string(), String::new())
        );
        // Nothing installed: the first dash is the best guess there is.
        assert_eq!(
            listed[1].circuit(&[]),
            (
                "ks_nordschleife".to_string(),
                "touristenfahrten".to_string()
            )
        );
    }

    #[test]
    fn what_is_missing_is_what_is_not_installed() {
        let listed = parse(LIST);
        let missing = listed[1].missing(
            |track| track == "ks_nordschleife",
            |car| car == "ks_mazda_miata",
        );
        assert_eq!(missing, vec!["mod_unknown".to_string()]);
        let missing = listed[0].missing(|_| false, |_| true);
        assert_eq!(missing, vec!["ks_spa".to_string()]);
    }

    #[test]
    fn an_answer_that_is_not_a_list_is_no_servers() {
        assert!(parse("<html>blocked</html>").is_empty());
        assert!(parse("{}").is_empty());
    }
}
