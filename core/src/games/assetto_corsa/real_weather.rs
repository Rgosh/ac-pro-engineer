//! The weather at the circuit, right now.
//!
//! **A circuit says where it is.** `ui_track.json` carries `geotags` — a
//! latitude and a longitude — for most circuits, Kunos's and modders' alike.
//! With those, Open-Meteo (free, no key, no account) says what the sky over
//! the real place is doing this minute, and what time it is there.
//!
//! **Asked only when somebody presses for it**, and what is sent is two
//! coordinates of a racing circuit: nothing about the person asking.
//!
//! The game's own weather is seven skies and no rain; the nearest of them is
//! chosen and what was measured is said beside it, so "light rain, drawn as
//! heavy cloud" is not presented as anything else.

use serde::Deserialize;
use std::time::Duration;

/// Where a circuit is: latitude and longitude in degrees, north and east
/// positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub latitude: f64,
    pub longitude: f64,
}

/// A circuit's place from its `ui_track.json`, or `None` when it does not
/// say — or says something that is not on the Earth.
pub fn place_in(ui_track: &str) -> Option<Place> {
    let value: serde_json::Value = serde_json::from_str(ui_track).ok()?;
    let tags = value.get("geotags")?.as_array()?;
    let latitude = degrees(tags.first()?)?;
    let longitude = degrees(tags.get(1)?)?;
    // `0, 0` is what an author leaves when they did not fill it in.
    let filled = latitude != 0.0 || longitude != 0.0;
    (filled && latitude.abs() <= 90.0 && longitude.abs() <= 180.0).then_some(Place {
        latitude,
        longitude,
    })
}

/// One coordinate as authors write them: `45.62`, `"45.62"`, `"45.62° N"`,
/// `"1.02 W"`, `"N45.62"`. South and west are negative.
fn degrees(tag: &serde_json::Value) -> Option<f64> {
    if let Some(number) = tag.as_f64() {
        return Some(number);
    }
    let text = tag.as_str()?.trim();
    let upper = text.to_ascii_uppercase();
    let negative = upper.contains('S') || upper.contains('W');
    let number: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | ','))
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    let value: f64 = number.parse().ok()?;
    Some(match negative {
        true => -value.abs(),
        false => value,
    })
}

/// What is asked of Open-Meteo for a place.
pub fn url(place: Place) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}\
         &current=temperature_2m,weather_code,cloud_cover,wind_speed_10m,wind_direction_10m\
         &wind_speed_unit=kmh&timezone=auto",
        place.latitude, place.longitude
    )
}

/// The weather at a place, as measured.
#[derive(Debug, Clone, PartialEq)]
pub struct Now {
    /// Air temperature in °C.
    pub air_c: f64,
    /// The WMO weather code: 0 clear … 45 fog … 61 rain … 95 thunderstorm.
    pub code: u32,
    /// How much of the sky is covered, 0 to 100.
    pub cloud: u32,
    pub wind_kmh: f64,
    /// Where the wind blows from, in degrees.
    pub wind_from: u32,
    /// The local time there, in minutes since midnight.
    pub minute: u32,
}

#[derive(Deserialize)]
struct Answer {
    current: Current,
}

#[derive(Deserialize)]
struct Current {
    #[serde(default)]
    time: String,
    temperature_2m: f64,
    #[serde(default)]
    weather_code: f64,
    #[serde(default)]
    cloud_cover: f64,
    #[serde(default)]
    wind_speed_10m: f64,
    #[serde(default)]
    wind_direction_10m: f64,
}

/// Open-Meteo's answer, read. `None` when it is not one.
pub fn parse(json: &str) -> Option<Now> {
    let answer: Answer = serde_json::from_str(json).ok()?;
    let current = answer.current;
    // `2026-11-11T14:15`, local to the place because the question said so.
    let minute = current
        .time
        .split_once('T')
        .and_then(|(_, clock)| clock.split_once(':'))
        .and_then(|(hour, minute)| {
            let hour: u32 = hour.parse().ok()?;
            let minute: u32 = minute.get(..2)?.parse().ok()?;
            (hour < 24 && minute < 60).then_some(hour * 60 + minute)
        })
        .unwrap_or(13 * 60);
    Some(Now {
        air_c: current.temperature_2m,
        code: current.weather_code.max(0.0) as u32,
        cloud: current.cloud_cover.clamp(0.0, 100.0) as u32,
        wind_kmh: current.wind_speed_10m.max(0.0),
        wind_from: (current.wind_direction_10m.rem_euclid(360.0)) as u32,
        minute,
    })
}

impl Now {
    /// The game's own sky nearest to this, by its folder name.
    pub fn sky(&self) -> &'static str {
        match self.code {
            45 | 48 if self.cloud >= 70 => "1_heavy_fog",
            45 | 48 => "2_light_fog",
            // Rain, snow, showers, storms: the game draws none of them, and
            // the darkest sky it has is the honest stand-in.
            51..=99 => "7_heavy_clouds",
            _ => match self.cloud {
                0..=10 => "3_clear",
                11..=30 => "4_mid_clear",
                31..=55 => "5_light_clouds",
                56..=80 => "6_mid_clouds",
                _ => "7_heavy_clouds",
            },
        }
    }

    /// What it is doing there, in a few words.
    pub fn said(&self) -> &'static str {
        match self.code {
            0 => "Clear sky",
            1 => "Mainly clear",
            2 => "Partly cloudy",
            3 => "Overcast",
            45 | 48 => "Fog",
            51..=57 => "Drizzle",
            61..=67 => "Rain",
            71..=77 => "Snow",
            80..=82 => "Rain showers",
            85 | 86 => "Snow showers",
            95..=99 => "Thunderstorm",
            _ => "Cloud",
        }
    }

    /// Whether the game's sky can only stand in for it: rain, snow, storms.
    pub fn wetter_than_drawn(&self) -> bool {
        (51..=99).contains(&self.code)
    }
}

/// Ask Open-Meteo. Blocking; call it from a thread.
pub fn fetch(place: Place, timeout: Duration) -> Result<Now, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|why| format!("could not make a client: {why}"))?;
    let text = client
        .get(url(place))
        .send()
        .and_then(|answer| answer.error_for_status())
        .and_then(|answer| answer.text())
        .map_err(|why| why.to_string())?;
    parse(&text).ok_or_else(|| "the weather service's answer did not read".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_circuit_says_where_it_is_however_its_author_wrote_it() {
        let at = |tags: &str| place_in(&format!(r#"{{"name":"x","geotags":{tags}}}"#));
        let spa = Some(Place {
            latitude: 50.4372,
            longitude: 5.9714,
        });
        assert_eq!(at(r#"["50.4372","5.9714"]"#), spa);
        assert_eq!(at(r#"[50.4372, 5.9714]"#), spa);
        assert_eq!(at(r#"["50.4372° N","5.9714° E"]"#), spa);
        assert_eq!(at(r#"["N50.4372","E5.9714"]"#), spa);
        // South and west are negative: Interlagos.
        assert_eq!(
            at(r#"["23.7036 S","46.6997 W"]"#),
            Some(Place {
                latitude: -23.7036,
                longitude: -46.6997
            })
        );
        // Not filled in, not there, not on the Earth.
        assert_eq!(at(r#"["0","0"]"#), None);
        assert_eq!(at(r#"[]"#), None);
        assert_eq!(at(r#"["lat","lon"]"#), None);
        assert_eq!(at(r#"["123.0","5.0"]"#), None);
        assert_eq!(place_in(r#"{"name":"x"}"#), None);
    }

    #[test]
    fn the_services_answer_reads_and_names_the_nearest_sky() {
        let answer = r#"{"latitude":50.44,"longitude":5.97,"utc_offset_seconds":3600,
            "current":{"time":"2026-11-11T14:15","interval":900,"temperature_2m":8.4,
            "weather_code":61,"cloud_cover":100,"wind_speed_10m":17.6,"wind_direction_10m":245}}"#;
        let now = parse(answer).expect("it reads");
        assert_eq!(now.minute, 14 * 60 + 15);
        assert_eq!(now.wind_from, 245);
        assert_eq!(now.said(), "Rain");
        // Rain is not drawn; the darkest sky stands in, and it is said.
        assert_eq!(now.sky(), "7_heavy_clouds");
        assert!(now.wetter_than_drawn());

        let sky = |code: u32, cloud: u32| {
            Now {
                air_c: 20.0,
                code,
                cloud,
                wind_kmh: 0.0,
                wind_from: 0,
                minute: 0,
            }
            .sky()
        };
        assert_eq!(sky(0, 0), "3_clear");
        assert_eq!(sky(1, 25), "4_mid_clear");
        assert_eq!(sky(2, 50), "5_light_clouds");
        assert_eq!(sky(3, 75), "6_mid_clouds");
        assert_eq!(sky(3, 100), "7_heavy_clouds");
        assert_eq!(sky(45, 40), "2_light_fog");
        assert_eq!(sky(48, 100), "1_heavy_fog");
        assert_eq!(parse("{}"), None);
    }

    #[test]
    fn the_question_names_two_coordinates_and_nothing_else() {
        let asked = url(Place {
            latitude: 50.43721,
            longitude: -5.97149,
        });
        assert!(asked.starts_with("https://api.open-meteo.com/v1/forecast?"));
        assert!(asked.contains("latitude=50.4372&longitude=-5.9715"));
        assert!(asked.contains("timezone=auto"));
    }
}
