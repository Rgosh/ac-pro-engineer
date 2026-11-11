//! Which of a server's cars are free to take, as the server itself says.
//!
//! **A car on a server's list is not a car you can drive there.** Every
//! entry in a server's entry list is one place in one car, and the lobby only
//! names the models. Join in a car whose every place is taken and the game
//! starts, connects and is turned away — the session that ends before it
//! begins, after the whole load. The game's `acServer` answers
//! `GET /JSON|<guid>` on its HTTP port with every place: the model, the
//! livery, and whether somebody is in it. Content Manager reads the same
//! answer to grey out a taken car.
//!
//! Read leniently: a field left out is a place assumed free, because the
//! server is what decides in the end and a wrong "taken" would hide a car
//! somebody could have driven.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;

/// The address the places are asked for at, as the game's own launcher and
/// Content Manager ask: with the Steam account that will join, so a place the
/// server keeps for that account is marked (`IsRequestedGUID`). `-1` asks for
/// nobody in particular.
pub fn url(ip: &str, http_port: u16, steam_id: Option<&str>) -> String {
    format!(
        "http://{ip}:{http_port}/JSON%7C{}",
        steam_id.unwrap_or("-1")
    )
}

/// One place in the entry list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Place {
    #[serde(default, rename = "Model")]
    pub model: String,
    #[serde(default, rename = "Skin")]
    pub skin: String,
    #[serde(default, rename = "IsConnected")]
    pub taken: bool,
    /// Kept by the server for the account that asked.
    #[serde(default, rename = "IsRequestedGUID")]
    pub yours: bool,
    /// Who is in it — `null` for an empty place, and for a place whose
    /// driver has left, which keeps the name and is no longer connected.
    #[serde(default, rename = "DriverName")]
    pub driver: Option<String>,
    #[serde(default, rename = "DriverTeam")]
    pub team: Option<String>,
    #[serde(default, rename = "DriverNation")]
    pub nation: Option<String>,
}

/// Somebody on a server right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Driver {
    pub name: String,
    /// The car they are in, by folder.
    pub car: String,
}

/// Who is on a server: the places that are taken and say by whom, by name.
///
/// **Only the connected ones.** A place keeps the name of whoever sat in it
/// last; listing those would show a friend "online" who left an hour ago.
pub fn drivers(places: &[Place]) -> Vec<Driver> {
    let mut found: Vec<Driver> = places
        .iter()
        .filter(|place| place.taken)
        .filter_map(|place| {
            let name = place.driver.as_deref()?.trim();
            (!name.is_empty()).then(|| Driver {
                name: name.to_string(),
                car: place.model.clone(),
            })
        })
        .collect();
    found.sort_by_key(|driver| driver.name.to_lowercase());
    found
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default, rename = "Cars")]
    cars: Vec<Place>,
}

/// How many places a car has on a server, and how many are free.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Room {
    pub free: u32,
    pub places: u32,
}

/// The places in an answer, or `None` when it is not one.
pub fn parse(json: &str) -> Option<Vec<Place>> {
    serde_json::from_str::<Answer>(json).ok().map(|a| a.cars)
}

/// Each car's room, by its folder in lower case — the server and the game
/// both match car folders without regard to case.
pub fn rooms(places: &[Place]) -> BTreeMap<String, Room> {
    let mut by_car: BTreeMap<String, Room> = BTreeMap::new();
    for place in places {
        let room = by_car.entry(place.model.to_ascii_lowercase()).or_default();
        room.places += 1;
        if !place.taken || place.yours {
            room.free += 1;
        }
    }
    by_car
}

/// Ask the server for its places.
pub fn fetch(
    ip: &str,
    http_port: u16,
    steam_id: Option<&str>,
    timeout: Duration,
) -> Result<Vec<Place>, String> {
    let text = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|why| format!("could not make a client: {why}"))?
        .get(url(ip, http_port, steam_id))
        .send()
        .and_then(|answer| answer.error_for_status())
        .and_then(|answer| answer.text())
        .map_err(|why| why.to_string())?;
    parse(&text).ok_or_else(|| "the server's answer was not its list of places".to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    /// The shape of a real answer, from the busiest server of the evening
    /// this was written: two places a car, some taken.
    const ANSWER: &str = r#"{"Features":["WEATHERPATCH"],"Cars":[
        {"Model":"ks_audi_r8_plus","Skin":"00_brilliant_red_t","DriverName":null,"DriverTeam":null,"IsRequestedGUID":false,"IsEntryList":true,"IsConnected":false},
        {"Model":"ks_audi_r8_plus","Skin":"01_white","DriverName":"Somebody","DriverTeam":"","IsRequestedGUID":false,"IsEntryList":true,"IsConnected":true},
        {"Model":"bmw_1m_s3","Skin":"00","DriverName":"A","IsConnected":true},
        {"Model":"BMW_1M_S3","Skin":"01","DriverName":"B","IsConnected":true}
    ]}"#;

    #[test]
    fn a_car_with_every_place_taken_has_no_room() {
        let rooms = rooms(&parse(ANSWER).unwrap());
        assert_eq!(rooms["ks_audi_r8_plus"], Room { free: 1, places: 2 });
        // Matched without regard to case, as the game does.
        assert_eq!(rooms["bmw_1m_s3"], Room { free: 0, places: 2 });
        // A place kept for the account that asked is free to it.
        let kept = r#"{"Cars":[{"Model":"x","IsConnected":false,"IsRequestedGUID":true},
                                {"Model":"x","IsConnected":true}]}"#;
        assert_eq!(super::rooms(&parse(kept).unwrap())["x"].free, 1);
    }

    /// **Who is there now, not who was**: a place that names a driver and is
    /// not connected is somebody who left.
    #[test]
    fn the_drivers_on_a_server_are_the_connected_ones_by_name() {
        let mut places = parse(ANSWER).unwrap();
        places.push(Place {
            model: "bmw_1m_s3".into(),
            driver: Some("Left an hour ago".into()),
            taken: false,
            ..Place::default()
        });
        let names: Vec<String> = drivers(&places).into_iter().map(|d| d.name).collect();
        assert_eq!(names, vec!["A", "B", "Somebody"]);
        assert_eq!(drivers(&places)[2].car, "ks_audi_r8_plus");
    }

    #[test]
    fn something_else_is_not_a_list_of_places() {
        assert_eq!(parse("<html>no</html>"), None);
        assert_eq!(parse("{}"), Some(Vec::new()));
    }

    #[test]
    fn the_address_asks_for_everybody() {
        assert_eq!(url("1.2.3.4", 8081, None), "http://1.2.3.4:8081/JSON%7C-1");
        assert_eq!(
            url("1.2.3.4", 8081, Some("76561198000000000")),
            "http://1.2.3.4:8081/JSON%7C76561198000000000"
        );
    }
}
