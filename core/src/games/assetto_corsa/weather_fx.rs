//! Custom Shaders Patch's weather: rain, snow and storms, and weather that
//! changes during a session.
//!
//! **The game's own weather is seven skies, all dry.** With the patch's
//! WeatherFX on, a session can ask for any of the patch's thirty-three
//! weather types — drizzle to a thunderstorm, sleet, snow, fog, haze — and
//! the patch draws it, with a wet track and rain on the glass under RainFX.
//! The session asks in `race.ini`'s `[LIGHTING]`, with two keys the patch
//! reads: which *controller* drives the weather, and which *type* it starts
//! as. They carry Content Manager's prefix because Content Manager wrote
//! them first; the patch reads them from any launcher.
//!
//! **A controller decides how the weather moves.** The patch ships them in
//! `extension/weather-controllers/`; Pure ships its own. `base` holds one
//! type for the session; `base-schedule` walks a list of types, a set number
//! of minutes each — its list is a small file in the patch's state folder,
//! [`Schedule`]. Which controllers there are is read from the folder, so a
//! controller installed tomorrow is offered tomorrow.
//!
//! The numbers are the patch's own `WeatherType` values, as its Lua SDK
//! publishes them.

use std::path::{Path, PathBuf};

/// One of the patch's weather types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Type {
    /// The patch's number for it.
    pub id: u8,
    /// What it is called.
    pub label: &'static str,
    /// Whether the track is wet under it.
    pub wet: bool,
}

const fn t(id: u8, label: &'static str, wet: bool) -> Type {
    Type { id, label, wet }
}

/// Every type, in the order they are offered: dry to wet to extreme.
pub const TYPES: [Type; 33] = [
    t(15, "Clear", false),
    t(16, "Few clouds", false),
    t(17, "Scattered clouds", false),
    t(18, "Broken clouds", false),
    t(19, "Overcast", false),
    t(21, "Mist", false),
    t(20, "Fog", false),
    t(23, "Haze", false),
    t(31, "Windy", false),
    t(30, "Hot", false),
    t(29, "Cold", false),
    t(3, "Light drizzle", true),
    t(4, "Drizzle", true),
    t(5, "Heavy drizzle", true),
    t(6, "Light rain", true),
    t(7, "Rain", true),
    t(8, "Heavy rain", true),
    t(0, "Light thunderstorm", true),
    t(1, "Thunderstorm", true),
    t(2, "Heavy thunderstorm", true),
    t(26, "Squalls", true),
    t(32, "Hail", true),
    t(12, "Light sleet", true),
    t(13, "Sleet", true),
    t(14, "Heavy sleet", true),
    t(9, "Light snow", true),
    t(10, "Snow", true),
    t(11, "Heavy snow", true),
    t(22, "Smoke", false),
    t(24, "Sand", false),
    t(25, "Dust", false),
    t(27, "Tornado", true),
    t(28, "Hurricane", true),
];

/// A type by the patch's number.
pub fn by_id(id: u8) -> Option<Type> {
    TYPES.iter().copied().find(|t| t.id == id)
}

/// The controller that holds one type for the whole session.
pub const STEADY: &str = "base";
/// The controller that walks a list of types.
pub const SCHEDULED: &str = "base-schedule";

/// The weather controllers installed with the patch and beside it.
pub fn controllers(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join("extension").join("weather-controllers")) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with('.') && !name.starts_with('_'))
        .collect();
    found.sort_by_key(|name| name.to_lowercase());
    found
}

/// The lines `race.ini`'s `[LIGHTING]` gains to ask the patch for this
/// weather, each ending in a newline.
pub fn lighting_lines(controller: &str, start: u8) -> String {
    format!("__CM_WEATHER_CONTROLLER={controller}\n__CM_WEATHER_TYPE={start}\n")
}

/// Weather that changes during a session: each type in turn, for so many
/// minutes each — or in an order of the patch's choosing.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Schedule {
    /// The types, in order, by the patch's number.
    pub slots: Vec<u8>,
    /// Minutes of game time each lasts.
    pub minutes_each: u32,
    /// The patch picks the order rather than following the list.
    pub random: bool,
}

/// The most types a schedule holds.
pub const MOST_SLOTS: usize = 10;

impl Schedule {
    /// The schedule's file, as the patch's controller reads it.
    pub fn text(&self) -> String {
        let slots: Vec<u8> = self.slots.iter().copied().take(MOST_SLOTS).collect();
        let mut out = format!(
            "[SETTINGS]\nSLOTS={}\nTIME_PER_SLOT={}\nRANDOM={}\n",
            slots.len(),
            self.minutes_each.clamp(1, 600),
            u8::from(self.random)
        );
        for (at, id) in slots.iter().enumerate() {
            out.push_str(&format!("SLOT_{at}={id}\n"));
        }
        out
    }

    /// The type a session starts as: the first of the list.
    pub fn first(&self) -> u8 {
        self.slots.first().copied().unwrap_or(15)
    }
}

/// Where the schedule controller keeps its list, in the game's Documents.
pub fn schedule_file(game_documents: &Path) -> PathBuf {
    game_documents
        .join("cfg")
        .join("extension")
        .join("state")
        .join("lua")
        .join("wfx_controller")
        .join(format!("{SCHEDULED}__settings.ini"))
}

/// Write a schedule where the controller reads it.
pub fn write_schedule(game_documents: &Path, schedule: &Schedule) -> Result<PathBuf, String> {
    let at = schedule_file(game_documents);
    if let Some(folder) = at.parent() {
        std::fs::create_dir_all(folder).map_err(|why| format!("{}: {why}", folder.display()))?;
    }
    std::fs::write(&at, schedule.text()).map_err(|why| format!("{}: {why}", at.display()))?;
    Ok(at)
}

/// The schedule the controller last read, when there is one.
pub fn read_schedule(game_documents: &Path) -> Option<Schedule> {
    let text = std::fs::read_to_string(schedule_file(game_documents)).ok()?;
    let value = |key: &str| super::ini::value(&text, "SETTINGS", key);
    let count: usize = value("SLOTS")?.parse().ok()?;
    let slots: Vec<u8> = (0..count.min(MOST_SLOTS))
        .filter_map(|at| value(&format!("SLOT_{at}"))?.parse().ok())
        .filter(|id| by_id(*id).is_some())
        .collect();
    Some(Schedule {
        slots,
        minutes_each: value("TIME_PER_SLOT")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),
        random: value("RANDOM").is_some_and(|v| v.trim() == "1"),
    })
}

/// The patch's type nearest to the weather a service measured — a WMO code
/// and how much of the sky is covered. **Rain is rain**: where the game
/// alone could only stand heavy cloud in for it, the patch draws it.
pub fn from_wmo(code: u32, cloud: u32) -> u8 {
    match code {
        0 => 15,
        1 => 16,
        2 => 17,
        3 if cloud >= 95 => 19,
        3 => 18,
        45 | 48 if cloud >= 70 => 20,
        45 | 48 => 21,
        51 => 3,
        53 | 56 => 4,
        55 | 57 => 5,
        61 | 80 => 6,
        63 | 66 | 81 => 7,
        65 | 67 | 82 => 8,
        71 | 85 => 9,
        73 => 10,
        75 | 77 | 86 => 11,
        95 => 1,
        96 => 32,
        99 => 2,
        _ => match cloud {
            0..=10 => 15,
            11..=30 => 16,
            31..=55 => 17,
            56..=85 => 18,
            _ => 19,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_of_the_patch_is_offered_once() {
        let mut ids: Vec<u8> = TYPES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, (0..=32).collect::<Vec<u8>>());
        assert_eq!(by_id(7).map(|t| t.label), Some("Rain"));
        assert!(by_id(7).is_some_and(|t| t.wet));
        assert!(by_id(15).is_some_and(|t| !t.wet));
        assert_eq!(by_id(33), None);
    }

    /// **The file the patch's controller reads, written and read back** —
    /// and a list longer than the controller takes is cut, not overflowed.
    #[test]
    fn a_schedule_is_written_as_the_controller_reads_it() {
        let schedule = Schedule {
            slots: vec![15, 18, 6, 7],
            minutes_each: 12,
            random: false,
        };
        assert_eq!(
            schedule.text(),
            "[SETTINGS]\nSLOTS=4\nTIME_PER_SLOT=12\nRANDOM=0\nSLOT_0=15\nSLOT_1=18\nSLOT_2=6\nSLOT_3=7\n"
        );
        assert_eq!(schedule.first(), 15);
        let dir = std::env::temp_dir().join(format!("acpe-wfx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let at = write_schedule(&dir, &schedule).expect("it is written");
        assert!(at.ends_with("cfg/extension/state/lua/wfx_controller/base-schedule__settings.ini"));
        assert_eq!(read_schedule(&dir), Some(schedule));
        let long = Schedule {
            slots: vec![15; 25],
            minutes_each: 0,
            random: true,
        };
        assert!(
            long.text()
                .starts_with("[SETTINGS]\nSLOTS=10\nTIME_PER_SLOT=1\nRANDOM=1\n")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_session_asks_for_a_controller_and_a_type() {
        assert_eq!(
            lighting_lines(SCHEDULED, 6),
            "__CM_WEATHER_CONTROLLER=base-schedule\n__CM_WEATHER_TYPE=6\n"
        );
    }

    #[test]
    fn measured_weather_becomes_the_nearest_type_rain_included() {
        assert_eq!(from_wmo(0, 0), 15);
        assert_eq!(from_wmo(3, 100), 19);
        assert_eq!(from_wmo(61, 100), 6);
        assert_eq!(from_wmo(65, 100), 8);
        assert_eq!(from_wmo(95, 100), 1);
        assert_eq!(from_wmo(73, 100), 10);
        assert_eq!(from_wmo(45, 90), 20);
        // Every answer is a type the patch has.
        for code in 0..100 {
            assert!(by_id(from_wmo(code, 50)).is_some(), "{code}");
        }
    }

    #[test]
    fn the_controllers_are_what_is_installed() {
        let root = std::env::temp_dir().join(format!("acpe-wfx-ctl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for name in ["base", "base-schedule", "pureCtrl", "_internal"] {
            std::fs::create_dir_all(root.join("extension/weather-controllers").join(name))
                .expect("a fixture");
        }
        assert_eq!(
            controllers(&root),
            vec!["base", "base-schedule", "pureCtrl"]
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
