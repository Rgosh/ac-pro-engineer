//! What the game leaves behind in Documents: screenshots and replays.
//!
//! **Two folders under the game's own one** — `screens`, where the game and
//! the patch put a screenshot, and `replay`, where a saved replay goes. Both
//! grow for as long as somebody drives and neither is shown anywhere outside
//! the game, which is why a replay folder is so often the largest thing on
//! the disk nobody knew about.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// One file the game wrote: where, how large, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub path: PathBuf,
    /// What it is called, without its extension.
    pub name: String,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
}

/// Where screenshots go, under the game's own Documents folder.
pub fn screenshots_dir(documents: &Path) -> PathBuf {
    documents.join("screens")
}

/// Where saved replays go.
pub fn replays_dir(documents: &Path) -> PathBuf {
    documents.join("replay")
}

/// Every screenshot, newest first.
pub fn screenshots(documents: &Path) -> Vec<Kept> {
    listed(&screenshots_dir(documents), &["png", "jpg", "jpeg", "bmp"])
}

/// Every saved replay, newest first. The game's own `cr` file — the replay
/// of the last session, overwritten each time — is not one of them.
pub fn replays(documents: &Path) -> Vec<Kept> {
    listed(&replays_dir(documents), &["acreplay"])
}

/// The files with one of these extensions in a folder, newest first, ties by
/// name.
fn listed(folder: &Path, extensions: &[&str]) -> Vec<Kept> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<Kept> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
            if !extensions.contains(&extension.as_str()) {
                return None;
            }
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some(Kept {
                name: path.file_stem()?.to_string_lossy().into_owned(),
                bytes: meta.len(),
                modified: meta.modified().ok(),
                path,
            })
        })
        .collect();
    found.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    found
}

/// A size as people say one: 512 KB, 3.4 MB, 1.2 GB.
pub fn size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB * KB {
        format!("{:.0} KB", (b / KB).max(1.0))
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.1} GB", b / (KB * KB * KB))
    }
}

/// What a replay file says about itself at its start: which circuit, which
/// layout and which weather it was recorded on.
///
/// **Read the way Content Manager reads it** (its `ReplayDetails`): a
/// little-endian version, then — from version 14 — eight bytes it skips,
/// the weather, the circuit and the layout; before 14, the circuit alone.
/// A string is a little-endian `u32` length and that many bytes.
///
/// The game needs the circuit named in `race.ini` to play a replay, so a
/// misread here would load the wrong one. Nothing is believed that is not a
/// folder name: printable, no path in it, a sensible length. The caller
/// checks the circuit is installed before using it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayHeader {
    pub version: u32,
    pub weather: String,
    pub track: String,
    pub track_config: String,
}

/// The header of a replay file's first bytes, or `None` if it does not read
/// as one.
pub fn replay_header(bytes: &[u8]) -> Option<ReplayHeader> {
    let mut at = 0usize;
    let u32_at = |at: &mut usize| -> Option<u32> {
        let four: [u8; 4] = bytes.get(*at..*at + 4)?.try_into().ok()?;
        *at += 4;
        Some(u32::from_le_bytes(four))
    };
    let text = |at: &mut usize, may_be_empty: bool| -> Option<String> {
        let len = u32_at(at)? as usize;
        if len > 128 || (len == 0 && !may_be_empty) {
            return None;
        }
        let raw = bytes.get(*at..*at + len)?;
        *at += len;
        let s = std::str::from_utf8(raw).ok()?;
        let plain = s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '));
        (plain && !s.contains("..")).then(|| s.to_string())
    };
    let version = u32_at(&mut at)?;
    if !(1..=64).contains(&version) {
        return None;
    }
    let (weather, track, track_config) = if version >= 14 {
        at += 8;
        let weather = text(&mut at, true)?;
        let track = text(&mut at, false)?;
        let config = text(&mut at, true)?;
        (weather, track, config)
    } else {
        (String::new(), text(&mut at, false)?, String::new())
    };
    Some(ReplayHeader {
        version,
        weather,
        track,
        track_config,
    })
}

/// `race.ini` for watching a replay: the session that would be driven next,
/// with the replay switched on, named by its file name in the replay folder,
/// and the circuit, layout and weather it was recorded on — the game loads
/// the circuit `race.ini` names before it plays anything.
pub fn watch_race_ini(base: &str, file_name: &str, header: &ReplayHeader) -> String {
    let mut text = super::ini::with_value(base, "REPLAY", "ACTIVE", "1");
    text = super::ini::with_value(&text, "REPLAY", "FILENAME", file_name);
    text = super::ini::with_value(&text, "RACE", "TRACK", &header.track);
    text = super::ini::with_value(&text, "RACE", "CONFIG_TRACK", &header.track_config);
    if !header.weather.is_empty() {
        text = super::ini::with_value(&text, "WEATHER", "NAME", &header.weather);
    }
    text
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    fn string(s: &str) -> Vec<u8> {
        let mut out = (s.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(s.as_bytes());
        out
    }

    /// A version-16 header, a version-12 one, and files that are not one.
    #[test]
    fn a_replay_says_where_it_was_recorded() {
        let mut v16 = 16u32.to_le_bytes().to_vec();
        v16.extend_from_slice(&40.0f64.to_le_bytes());
        for s in ["3_clear", "ks_nordschleife", "nordschleife"] {
            v16.extend(string(s));
        }
        v16.extend_from_slice(&[9, 9, 9, 9]);
        let header = super::replay_header(&v16).unwrap();
        assert_eq!(header.track, "ks_nordschleife");
        assert_eq!(header.track_config, "nordschleife");
        assert_eq!(header.weather, "3_clear");

        let mut v12 = 12u32.to_le_bytes().to_vec();
        v12.extend(string("monza"));
        let old = super::replay_header(&v12).unwrap();
        assert_eq!(
            (old.track.as_str(), old.track_config.as_str()),
            ("monza", "")
        );

        assert!(super::replay_header(b"not a replay").is_none());
        let mut evil = 16u32.to_le_bytes().to_vec();
        evil.extend_from_slice(&[0; 8]);
        evil.extend(string("3_clear"));
        evil.extend(string("../../windows"));
        evil.extend(string(""));
        assert!(
            super::replay_header(&evil).is_none(),
            "a path is not a circuit"
        );
    }

    #[test]
    fn watching_switches_the_replay_on_at_its_circuit() {
        let base = "[RACE]\nTRACK=monza\nCONFIG_TRACK=\nMODEL=ks_bmw\n[REPLAY]\nACTIVE=0\n";
        let header = super::ReplayHeader {
            version: 16,
            weather: "7_heavy_clouds".into(),
            track: "spa".into(),
            track_config: String::new(),
        };
        let ini = super::watch_race_ini(base, "evening.acreplay", &header);
        assert!(ini.contains("ACTIVE=1") && ini.contains("FILENAME=evening.acreplay"));
        assert!(ini.contains("TRACK=spa") && ini.contains("MODEL=ks_bmw"));
        assert!(ini.contains("NAME=7_heavy_clouds"));
    }
    use super::*;

    #[test]
    fn only_pictures_and_replays_are_listed_newest_first() {
        let docs = std::env::temp_dir().join(format!("acpe-media-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&docs);
        let screens = screenshots_dir(&docs);
        let replay = replays_dir(&docs);
        std::fs::create_dir_all(&screens).unwrap();
        std::fs::create_dir_all(&replay).unwrap();
        std::fs::write(screens.join("old.png"), b"x").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(screens.join("new.JPG"), b"xx").unwrap();
        std::fs::write(screens.join("notes.txt"), b"x").unwrap();
        std::fs::create_dir_all(screens.join("folder.png")).unwrap();
        std::fs::write(replay.join("spa race.acreplay"), vec![0u8; 3000]).unwrap();
        std::fs::write(replay.join("cr"), b"x").unwrap();

        let shots = screenshots(&docs);
        let names: Vec<&str> = shots.iter().map(|k| k.name.as_str()).collect();
        assert_eq!(names, vec!["new", "old"]);
        let replays = replays(&docs);
        assert_eq!(replays.len(), 1);
        assert_eq!(replays[0].name, "spa race");
        assert_eq!(replays[0].bytes, 3000);
        assert!(screenshots(&docs.join("nowhere")).is_empty());
        let _ = std::fs::remove_dir_all(&docs);
    }

    #[test]
    fn a_size_reads_as_people_say_it() {
        assert_eq!(size(10), "1 KB");
        assert_eq!(size(512 * 1024), "512 KB");
        assert_eq!(size(3_565_158), "3.4 MB");
        assert_eq!(size(1_288_490_189), "1.2 GB");
    }
}
