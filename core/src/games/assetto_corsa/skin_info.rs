//! A livery's own details: who drives it, for which team, under which
//! number, from which country — `skins/<livery>/ui_skin.json`.
//!
//! **Four values changed, everything else in the file kept.** The file is
//! read as the game's own JSON (leniently, as `content` reads `ui_car.json`)
//! and only `drivername`, `team`, `number` and `country` are set; keys this
//! program has never heard of — a priority, a skin's display name, whatever a
//! modder added — are written back as they were. A number that was a number
//! stays a number.

use serde_json::{Map, Value};
use std::path::Path;

const FILE: &str = "ui_skin.json";

/// The details shown and edited.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkinInfo {
    pub driver: String,
    pub team: String,
    pub number: String,
    pub country: String,
}

fn text_of(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn object(skin_dir: &Path) -> Map<String, Value> {
    std::fs::read_to_string(skin_dir.join(FILE))
        .ok()
        .and_then(|text| super::content::read_json_text(&text))
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

/// What a livery says about itself; empty where it says nothing.
pub fn read(skin_dir: &Path) -> SkinInfo {
    let map = object(skin_dir);
    SkinInfo {
        driver: text_of(map.get("drivername")),
        team: text_of(map.get("team")),
        number: text_of(map.get("number")),
        country: text_of(map.get("country")),
    }
}

/// Write the four details, keeping everything else in the file.
pub fn write(skin_dir: &Path, info: &SkinInfo) -> Result<(), String> {
    let mut map = object(skin_dir);
    let clean = |s: &str| -> String {
        s.trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(64)
            .collect()
    };
    let number_was_a_number = matches!(map.get("number"), Some(Value::Number(_)));
    map.insert("drivername".into(), Value::String(clean(&info.driver)));
    map.insert("team".into(), Value::String(clean(&info.team)));
    map.insert("country".into(), Value::String(clean(&info.country)));
    let number = clean(&info.number);
    let as_number = number.parse::<i64>().ok().filter(|_| number_was_a_number);
    map.insert(
        "number".into(),
        match as_number {
            Some(n) => Value::Number(n.into()),
            None => Value::String(number),
        },
    );
    let text = serde_json::to_string_pretty(&Value::Object(map)).map_err(|e| e.to_string())?;
    crate::atomic_file::write_atomic(&skin_dir.join(FILE), text.as_bytes())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn four_details_change_and_the_rest_stays() {
        let dir = std::env::temp_dir().join(format!("acpe-skin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(FILE),
            "{\"skinname\": \"Rosso\", \"drivername\": \"\", \"number\": 7, \"priority\": 2}",
        )
        .unwrap();
        let mut info = read(&dir);
        assert_eq!(info.number, "7");
        info.driver = "Rgosh".into();
        info.team = "Team Sideways\n".into();
        info.number = "21".into();
        info.country = "Italy".into();
        write(&dir, &info).unwrap();
        let back: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(FILE)).unwrap()).unwrap();
        assert_eq!(back["skinname"], "Rosso", "a key it does not set is kept");
        assert_eq!(back["priority"], 2);
        assert_eq!(back["number"], 21, "a number stays a number");
        assert_eq!(back["team"], "Team Sideways", "no line break written");
        assert_eq!(read(&dir).driver, "Rgosh");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_livery_with_no_file_gets_one() {
        let dir = std::env::temp_dir().join(format!("acpe-skin-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(read(&dir), SkinInfo::default());
        write(
            &dir,
            &SkinInfo {
                number: "5".into(),
                ..SkinInfo::default()
            },
        )
        .unwrap();
        assert_eq!(read(&dir).number, "5");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
