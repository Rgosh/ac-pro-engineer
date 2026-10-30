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
//! **Nothing here holds an account or signs anybody in.** It reads a public
//! list, the same one anybody's browser can open.

use serde::Deserialize;
use std::time::Duration;

/// The lobby hosts tried in turn when nobody has said otherwise.
pub const DEFAULT_HOSTS: &[&str] = &["93.57.10.21"];

/// One server, as the lobby lists it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
    #[serde(default, rename = "sessiontypes")]
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
}

impl Listing {
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
        let best = installed
            .iter()
            .filter(|id| {
                self.track == **id
                    || self
                        .track
                        .strip_prefix(**id)
                        .is_some_and(|rest| rest.starts_with('-'))
            })
            .max_by_key(|id| id.len());
        match best {
            Some(id) => (
                id.to_string(),
                self.track
                    .get(id.len() + 1..)
                    .unwrap_or_default()
                    .to_string(),
            ),
            None => match self.track.split_once('-') {
                Some((id, layout)) => (id.to_string(), layout.to_string()),
                None => (self.track.clone(), String::new()),
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
        if !has_track(&self.track) && !has_track(&circuit) {
            out.push(circuit);
        }
        out.extend(self.cars.iter().filter(|car| !has_car(car)).cloned());
        out
    }

    /// The session running now, in words.
    pub fn session_name(&self) -> &'static str {
        match self.session {
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

    /// The country code, when the lobby gave one.
    pub fn country_code(&self) -> &str {
        self.country.get(1).map(String::as_str).unwrap_or("")
    }
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

/// Ask each host in turn for the list, and return the first that answers.
///
/// **Blocking, so called on a thread** — never from anything that draws.
pub fn fetch(hosts: &[String], timeout: Duration) -> Result<Vec<Listing>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent("Pro-Engineer-Lobby")
        .build()
        .map_err(|why| format!("could not make a client: {why}"))?;
    let mut last = "no lobby host to ask".to_string();
    for host in hosts {
        let url = format!("http://{host}/lobby.ashx/list");
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
                    last = format!("{host} answered with no servers");
                }
                Err(why) => last = format!("{host}: {why}"),
            },
            Err(why) => last = format!("{host}: {why}"),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
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
