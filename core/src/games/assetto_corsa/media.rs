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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
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
