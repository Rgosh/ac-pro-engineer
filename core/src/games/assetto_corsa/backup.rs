//! The game's settings and setups, kept as one file and put back from one.
//!
//! **Everything a driver has tuned lives in two folders** of the game's
//! Documents: `cfg/` — video, controls, the wheel, the patch's settings, the
//! apps' places on screen — and `setups/`. A patch update, a new wheel, a
//! "reset to defaults" tried out of curiosity, a move to another computer:
//! each is an evening of setting it all up again unless there is a copy.
//!
//! One zip per backup, named by when it was made. Restoring writes only into
//! those two folders, and first makes a backup of what is there — so a
//! restore is itself undone by restoring the one before it.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// The folders of the game's Documents that are backed up.
pub const FOLDERS: &[&str] = &["cfg", "setups"];

/// The largest file put in a backup. Settings are text; something larger is
/// a cache or a recording somebody left there.
const LARGEST: u64 = 8 * 1024 * 1024;

/// One backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    /// When it was made, in seconds since 1970 — its name.
    pub at: u64,
    pub path: PathBuf,
    pub bytes: u64,
}

/// Make a backup of the game's settings and setups. Returns the file and how
/// many files went into it.
pub fn make(game_documents: &Path, into: &Path, now: u64) -> Result<(PathBuf, usize), String> {
    std::fs::create_dir_all(into).map_err(|why| format!("{}: {why}", into.display()))?;
    let mut at = now;
    while into.join(format!("settings-{at}.zip")).exists() {
        at += 1;
    }
    let path = into.join(format!("settings-{at}.zip"));
    let file = std::fs::File::create(&path).map_err(|why| format!("{}: {why}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    let mut count = 0usize;
    for folder in FOLDERS {
        let base = game_documents.join(folder);
        for entry in walkdir::WalkDir::new(&base)
            .sort_by_file_name()
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            if entry.metadata().is_ok_and(|m| m.len() > LARGEST) {
                continue;
            }
            let Ok(rest) = entry.path().strip_prefix(game_documents) else {
                continue;
            };
            let name = rest
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let Ok(bytes) = std::fs::read(entry.path()) else {
                continue;
            };
            zip.start_file(name, options)
                .map_err(|why| why.to_string())?;
            zip.write_all(&bytes).map_err(|why| why.to_string())?;
            count += 1;
        }
    }
    zip.finish().map_err(|why| why.to_string())?;
    if count == 0 {
        let _ = std::fs::remove_file(&path);
        return Err("there were no settings to back up".to_string());
    }
    Ok((path, count))
}

/// Every backup in a folder, newest first.
pub fn list(folder: &Path) -> Vec<Backup> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<Backup> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let at = name
                .strip_prefix("settings-")?
                .strip_suffix(".zip")?
                .parse::<u64>()
                .ok()?;
            Some(Backup {
                at,
                path: entry.path(),
                bytes: entry.metadata().map(|m| m.len()).unwrap_or(0),
            })
        })
        .collect();
    found.sort_by_key(|backup| std::cmp::Reverse(backup.at));
    found
}

/// Put a backup back. What is there now is backed up first, into the same
/// folder the backup came from, so this can be undone. Returns how many
/// files were written.
///
/// **Only into the folders a backup is made of**, and never through a name
/// that climbs out of them: the file is a zip somebody may have been sent.
pub fn restore(backup: &Path, game_documents: &Path, now: u64) -> Result<usize, String> {
    let file = std::fs::File::open(backup).map_err(|why| format!("{}: {why}", backup.display()))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|why| format!("{} is not a backup: {why}", backup.display()))?;
    if let Some(folder) = backup.parent() {
        // Nothing to keep is not a reason to refuse the restore.
        let _ = make(game_documents, folder, now);
    }
    let mut count = 0usize;
    for at in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(at) else {
            continue;
        };
        let Ok(name) = entry.name().map(|name| name.replace('\\', "/")) else {
            continue;
        };
        let parts: Vec<&str> = name.split('/').filter(|part| !part.is_empty()).collect();
        let inside = parts
            .first()
            .is_some_and(|first| FOLDERS.iter().any(|f| f.eq_ignore_ascii_case(first)));
        let safe = parts
            .iter()
            .all(|part| *part != ".." && *part != "." && !part.contains(':'));
        if !inside || !safe || entry.is_dir() || entry.size() > LARGEST {
            continue;
        }
        let to = parts
            .iter()
            .fold(game_documents.to_path_buf(), |at, part| at.join(part));
        if let Some(above) = to.parent() {
            std::fs::create_dir_all(above).map_err(|why| format!("{}: {why}", above.display()))?;
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|why| why.to_string())?;
        std::fs::write(&to, bytes).map_err(|why| format!("{}: {why}", to.display()))?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    use super::*;

    fn file(root: &Path, relative: &str, said: &str) {
        let full = relative
            .split('/')
            .fold(root.to_path_buf(), |at, part| at.join(part));
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("a fixture");
        std::fs::write(full, said).expect("a fixture");
    }

    /// **Settings and setups go in, come back, and the restore itself can
    /// be undone** — and nothing outside the two folders is touched.
    #[test]
    fn a_backup_holds_the_settings_and_puts_them_back() {
        let dir = std::env::temp_dir().join(format!("acpe-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (docs, kept) = (dir.join("documents"), dir.join("backups"));
        file(&docs, "cfg/video.ini", "[VIDEO]\nWIDTH=1920\n");
        file(
            &docs,
            "cfg/extension/weather_fx.ini",
            "[BASIC]\nENABLED=1\n",
        );
        file(
            &docs,
            "setups/ks_mazda_mx5_cup/imola/quick.ini",
            "[FUEL]\nVALUE=20\n",
        );
        // Not settings: left out.
        file(&docs, "replay/last.acreplay", "big");
        let (zip, count) = make(&docs, &kept, 1000).expect("a backup");
        assert_eq!(count, 3);

        // Changed since, and a file lost.
        file(&docs, "cfg/video.ini", "[VIDEO]\nWIDTH=800\n");
        std::fs::remove_file(docs.join("setups/ks_mazda_mx5_cup/imola/quick.ini")).expect("gone");
        assert_eq!(restore(&zip, &docs, 2000).ok(), Some(3));
        assert_eq!(
            std::fs::read_to_string(docs.join("cfg/video.ini"))
                .ok()
                .as_deref(),
            Some("[VIDEO]\nWIDTH=1920\n")
        );
        assert!(
            docs.join("setups/ks_mazda_mx5_cup/imola/quick.ini")
                .is_file()
        );
        assert!(docs.join("replay/last.acreplay").is_file());

        // What was there before the restore was kept: newest first.
        let have = list(&kept);
        assert_eq!(
            have.iter().map(|b| b.at).collect::<Vec<_>>(),
            vec![2000, 1000]
        );
        restore(&have[0].path, &docs, 3000).expect("the restore is undone");
        assert_eq!(
            std::fs::read_to_string(docs.join("cfg/video.ini"))
                .ok()
                .as_deref(),
            Some("[VIDEO]\nWIDTH=800\n")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A zip that is not one of these writes nothing outside the folders.
    #[test]
    fn a_restore_writes_only_into_the_settings_folders() {
        let dir = std::env::temp_dir().join(format!("acpe-backup-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let docs = dir.join("documents");
        std::fs::create_dir_all(&docs).expect("a fixture");
        let bad = dir.join("settings-1.zip");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&bad).expect("a fixture"));
            let options = zip::write::SimpleFileOptions::default();
            for name in ["cfg/../../escaped.ini", "elsewhere/x.ini", "cfg/ok.ini"] {
                zip.start_file(name, options).expect("a fixture");
                zip.write_all(b"x").expect("a fixture");
            }
            zip.finish().expect("a fixture");
        }
        assert_eq!(restore(&bad, &docs, 5).ok(), Some(1));
        assert!(docs.join("cfg/ok.ini").is_file());
        assert!(!dir.join("escaped.ini").exists());
        assert!(!docs.join("elsewhere").exists());
        assert!(restore(&dir.join("nothing.zip"), &docs, 6).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
