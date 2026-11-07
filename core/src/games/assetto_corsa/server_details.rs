//! What a server says about itself beyond the lobby: the aids it forces,
//! the weather, its own description.
//!
//! **Only servers run through Content Manager's wrapper say any of it.** The
//! game's `acServer` publishes no more than the lobby lists; the wrapper
//! answers `GET /api/details` on a port of its own and announces that port at
//! the end of the server's name — `ℹ9013`, which
//! [`Listing::details_port`](super::lobby::Listing::details_port) reads. A
//! server without the marker simply has nothing more to say, and that is
//! drawn as nothing rather than as "no aids forced".
//!
//! The field names are the wrapper's. Everything is optional and read
//! leniently: a wrapper of another version that leaves a field out costs
//! that line and nothing else.

use serde::Deserialize;
use std::time::Duration;

/// The address the details are asked for at.
pub fn url(ip: &str, port: u16) -> String {
    format!("http://{ip}:{port}/api/details")
}

/// The aids a server allows, as the wrapper writes them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assists {
    /// 0 not allowed, 1 as the car has it, 2 forced on.
    #[serde(default = "factory")]
    pub abs_state: u8,
    #[serde(default = "factory")]
    pub tc_state: u8,
    #[serde(default = "yes")]
    pub stability_allowed: bool,
    #[serde(default = "yes")]
    pub autoclutch_allowed: bool,
    #[serde(default = "yes")]
    pub tyre_blankets: bool,
    #[serde(default)]
    pub force_virtual_mirror: bool,
    /// Percent of the damage the game would do.
    #[serde(default = "whole")]
    pub damage_multiplier: u32,
    #[serde(default = "whole")]
    pub fuel_rate: u32,
    #[serde(default = "whole")]
    pub tyre_wear_rate: u32,
}

fn factory() -> u8 {
    1
}
fn yes() -> bool {
    true
}
fn whole() -> u32 {
    100
}

/// What a server says about itself.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    #[serde(default)]
    pub assists: Option<Assists>,
    /// The owner's own words, often a Discord and the rules.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub ambient_temperature: Option<f32>,
    #[serde(default)]
    pub road_temperature: Option<f32>,
    /// The weather's folder name, `3_clear`.
    #[serde(default)]
    pub current_weather_id: Option<String>,
    /// Percent of a full contact allowed per kilometre, when the server
    /// counts collisions.
    #[serde(default)]
    pub max_contacts_per_km: Option<i32>,
}

/// How a server treats one aid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowed {
    /// Not allowed, whatever the driver chose.
    No,
    /// As the car itself has it.
    Factory,
    /// Allowed, for the driver to choose.
    Yes,
    /// On, whatever the driver chose.
    Forced,
    /// A rate as a percent of the game's own.
    Percent(u32),
}

/// One line of a server's rules: what, and how it is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub what: &'static str,
    pub allowed: Allowed,
    /// Whether it differs from what a driver would get offline with every
    /// aid available — the lines worth reading before joining.
    pub notable: bool,
}

impl Assists {
    /// The rules, in the order the game's own aids screen gives them.
    pub fn rules(&self) -> Vec<Rule> {
        let state = |n: u8| match n {
            0 => Allowed::No,
            2 => Allowed::Forced,
            _ => Allowed::Factory,
        };
        let switch = |on: bool| match on {
            true => Allowed::Yes,
            false => Allowed::No,
        };
        let rule = |what, allowed: Allowed, usual: Allowed| Rule {
            what,
            allowed,
            notable: allowed != usual,
        };
        vec![
            rule("ABS", state(self.abs_state), Allowed::Factory),
            rule("Traction control", state(self.tc_state), Allowed::Factory),
            rule(
                "Stability control",
                switch(self.stability_allowed),
                Allowed::Yes,
            ),
            rule(
                "Automatic clutch",
                switch(self.autoclutch_allowed),
                Allowed::Yes,
            ),
            rule("Tyre blankets", switch(self.tyre_blankets), Allowed::Yes),
            rule(
                "Virtual mirror",
                match self.force_virtual_mirror {
                    true => Allowed::Forced,
                    false => Allowed::Yes,
                },
                Allowed::Yes,
            ),
            rule(
                "Damage",
                Allowed::Percent(self.damage_multiplier),
                Allowed::Percent(100),
            ),
            rule(
                "Fuel use",
                Allowed::Percent(self.fuel_rate),
                Allowed::Percent(100),
            ),
            rule(
                "Tyre wear",
                Allowed::Percent(self.tyre_wear_rate),
                Allowed::Percent(100),
            ),
        ]
    }
}

/// The wrapper's answer, or `None` when it is not one.
pub fn parse(json: &str) -> Option<Details> {
    serde_json::from_str(json).ok()
}

/// Ask a server for its details.
///
/// **Blocking, so called on a thread** — never from anything that draws.
/// The answer is a few kilobytes; a server that has not answered in a few
/// seconds is not going to, and the panel draws without it meanwhile.
pub fn fetch(ip: &str, port: u16, timeout: Duration) -> Result<Details, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|why| format!("could not make a client: {why}"))?;
    let text = client
        .get(url(ip, port))
        .send()
        .and_then(|answer| answer.error_for_status())
        .and_then(|answer| answer.text())
        .map_err(|why| why.to_string())?;
    parse(&text).ok_or_else(|| "the server's details did not read".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An answer in the wrapper's shape, cut to the fields read here plus
    /// a few that are not, which must not stop it reading.
    const ANSWER: &str = r#"{
        "ambientTemperature": 24, "roadTemperature": 31.5,
        "currentWeatherId": "3_clear", "maxContactsPerKm": 3,
        "description": "Stewarded. Clean racing or a kick.",
        "assists": {"absState": 2, "tcState": 0, "stabilityAllowed": false,
                    "autoclutchAllowed": true, "tyreBlankets": true,
                    "forceVirtualMirror": false, "damageMultiplier": 50,
                    "fuelRate": 100, "tyreWearRate": 200},
        "content": {"cars": {}}, "frequency": 18, "players": {"Cars": []}
    }"#;

    /// **What differs from an open server is marked**, and what is the same
    /// is still listed, so a driver sees the whole of it in one place.
    #[test]
    fn the_wrapper_s_answer_reads_into_rules() {
        let Some(details) = parse(ANSWER) else {
            unreachable!("the wrapper's shape must read");
        };
        assert_eq!(details.road_temperature, Some(31.5));
        assert_eq!(details.current_weather_id.as_deref(), Some("3_clear"));
        let rules = details.assists.map(|a| a.rules()).unwrap_or_default();
        let notable: Vec<(&str, Allowed)> = rules
            .iter()
            .filter(|r| r.notable)
            .map(|r| (r.what, r.allowed))
            .collect();
        assert_eq!(
            notable,
            vec![
                ("ABS", Allowed::Forced),
                ("Traction control", Allowed::No),
                ("Stability control", Allowed::No),
                ("Damage", Allowed::Percent(50)),
                ("Tyre wear", Allowed::Percent(200)),
            ]
        );
        assert_eq!(rules.len(), 9);
    }

    /// **A field left out is the game's own default**, never a refusal: an
    /// older wrapper that does not write `tyreBlankets` does not forbid them.
    #[test]
    fn what_a_wrapper_leaves_out_is_the_usual() {
        let details = parse(r#"{"assists": {"absState": 1}}"#);
        let rules = details
            .and_then(|d| d.assists)
            .map(|a| a.rules())
            .unwrap_or_default();
        assert_eq!(rules.len(), 9);
        assert!(rules.iter().all(|r| !r.notable), "{rules:?}");
        assert_eq!(parse("<html>"), None);
        assert_eq!(
            url("203.0.113.4", 9013),
            "http://203.0.113.4:9013/api/details"
        );
    }
}
