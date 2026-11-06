//! What can be known to go wrong before the game is started.
//!
//! **Each of these is a reason sessions end before they begin**, found in
//! the threads people write after it happens: a car or circuit whose files
//! are not all there, the game on a Windows (NTFS) partition under Linux,
//! and the Documents folder inside OneDrive on Windows. Each is checked from
//! the files alone, so asking costs nothing and starts nothing.

use std::path::Path;

/// Whether a car's folder holds what the game needs to load it: its data
/// (packed as `data.acd` or loose as `data/`) and at least one model.
pub fn car_ready(root: &Path, car: &str) -> Result<(), String> {
    let folder = root.join("content").join("cars").join(car);
    if !folder.is_dir() {
        return Err(format!("{car} is not installed"));
    }
    if !folder.join("data.acd").is_file() && !folder.join("data").is_dir() {
        return Err(format!(
            "{car} has no data — it was not installed completely"
        ));
    }
    if !has_model(&folder) {
        return Err(format!(
            "{car} has no model (.kn5) — it was not installed completely"
        ));
    }
    Ok(())
}

/// Whether a circuit's folder, and its layout when it has one, is there.
pub fn track_ready(root: &Path, track: &str, layout: &str) -> Result<(), String> {
    let folder = root.join("content").join("tracks").join(track);
    if !folder.is_dir() {
        return Err(format!("{track} is not installed"));
    }
    if !layout.is_empty() && !folder.join(layout).is_dir() {
        return Err(format!("{track} has no layout {layout}"));
    }
    if !has_model(&folder) {
        return Err(format!(
            "{track} has no model (.kn5) — it was not installed completely"
        ));
    }
    Ok(())
}

fn has_model(folder: &Path) -> bool {
    std::fs::read_dir(folder).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("kn5"))
        })
    })
}

/// The filesystem `path` is on, from `/proc/mounts`: the longest mount point
/// that holds it. `None` where there is no such file (not Linux).
pub fn filesystem(path: &Path) -> Option<String> {
    let mounts = std::fs::read_to_string("/proc/mounts").ok()?;
    filesystem_in(&mounts, path)
}

/// The same, from the text of a mounts table — split out to be tested.
pub fn filesystem_in(mounts: &str, path: &Path) -> Option<String> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    mounts
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let _device = parts.next()?;
            let point = parts.next()?.replace("\\040", " ");
            let kind = parts.next()?.to_string();
            path.starts_with(&point).then_some((point.len(), kind))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, kind)| kind)
}

/// Whether the game sits on an NTFS partition — a known cause of crashes
/// under Proton. `fuseblk` is how most distributions mount NTFS.
pub fn on_ntfs(root: &Path) -> bool {
    filesystem(root).is_some_and(|kind| is_ntfs(&kind))
}

/// Whether a filesystem name is NTFS as Linux mounts it.
pub fn is_ntfs(kind: &str) -> bool {
    matches!(kind, "ntfs" | "ntfs3" | "fuseblk")
}

/// Whether a folder is inside OneDrive — Windows moving Documents there is
/// behind hangs and lost settings, as the sync locks and rewrites the
/// game's files while it runs.
pub fn in_onedrive(folder: &Path) -> bool {
    folder.components().any(|part| {
        part.as_os_str()
            .to_str()
            .is_some_and(|name| name.to_ascii_lowercase().starts_with("onedrive"))
    })
}

/// Whether a driver name will do: some servers refuse an empty one.
pub fn name_is_usable(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && name.chars().count() <= 32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn install(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("ac-preflight-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn a_car_is_ready_only_with_its_data_and_a_model() {
        let root = install("car");
        let car = root.join("content/cars/gt3");
        assert!(car_ready(&root, "gt3").is_err());
        let _ = std::fs::create_dir_all(&car);
        assert!(car_ready(&root, "gt3").is_err_and(|e| e.contains("no data")));
        let _ = std::fs::write(car.join("data.acd"), b"");
        assert!(car_ready(&root, "gt3").is_err_and(|e| e.contains("no model")));
        let _ = std::fs::write(car.join("gt3.KN5"), b"");
        assert!(car_ready(&root, "gt3").is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_circuit_is_ready_with_its_layout() {
        let root = install("track");
        let track = root.join("content/tracks/spa");
        let _ = std::fs::create_dir_all(track.join("gp"));
        let _ = std::fs::write(track.join("spa.kn5"), b"");
        assert!(track_ready(&root, "spa", "gp").is_ok());
        assert!(track_ready(&root, "spa", "").is_ok());
        assert!(track_ready(&root, "spa", "short").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_filesystem_is_the_longest_mount_that_holds_the_path() {
        let mounts = "/dev/a / ext4 rw 0 0\n/dev/b /mnt/games fuseblk rw 0 0\n\
                      /dev/c /mnt/games/fast btrfs rw 0 0\n/dev/d /mnt/my\\040disk ntfs3 rw 0 0\n";
        let kind = |p: &str| filesystem_in(mounts, Path::new(p));
        assert_eq!(kind("/home/x").as_deref(), Some("ext4"));
        assert_eq!(kind("/mnt/games/steam").as_deref(), Some("fuseblk"));
        assert_eq!(kind("/mnt/games/fast/ac").as_deref(), Some("btrfs"));
        assert_eq!(kind("/mnt/my disk/ac").as_deref(), Some("ntfs3"));
        assert!(is_ntfs("fuseblk") && is_ntfs("ntfs3") && !is_ntfs("btrfs"));
    }

    #[test]
    fn onedrive_and_names() {
        assert!(in_onedrive(Path::new(
            "C:/Users/me/OneDrive/Documents/Assetto Corsa"
        )));
        assert!(in_onedrive(Path::new(
            "/c/Users/me/OneDrive - Work/Documents"
        )));
        assert!(!in_onedrive(Path::new("/home/me/Documents/Assetto Corsa")));
        assert!(name_is_usable("Rgosh"));
        assert!(!name_is_usable("   "));
    }
}
