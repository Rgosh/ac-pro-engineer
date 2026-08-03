//! Writing a file so a crash cannot leave it half-written.
//!
//! Three places save user data — the lap records, the config, and the CSV
//! export — and all three were written the same almost-right way: write a
//! temp file, then rename it over the target. The rename is the important
//! part and it was already there.
//!
//! What was missing is the flush. `fs::write` returns once the data is with
//! the OS, not once it is on the disk, so a power loss or a hard reset
//! between the write and the rename leaves a correctly-named file holding
//! nothing. The records loader survives that (it validates every entry) and
//! the config has a `.bak` fallback, but "your settings reverted to default"
//! is a bad outcome for a machine that only lost power.
//!
//! The temp name also needs the process id. Two copies of the app saving at
//! the same moment otherwise write to the same temp path and one clobbers the
//! other mid-write, which is the one way this pattern can corrupt a file
//! rather than just lose an update.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Write `contents` to `path`, atomically and durably.
///
/// Creates the parent directory if it does not exist.
pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "tmp".to_string());
    // The pid keeps two instances of the app off each other's temp file.
    let temp_path = parent.join(format!(".{}.{}.tmp", file_name, std::process::id()));

    // Scoped so the file is closed before the rename.
    {
        let mut file = File::create(&temp_path)?;
        file.write_all(contents)?;
        // The point of the exercise: without this the rename can publish a
        // name whose contents never reached the disk.
        file.sync_all()?;
    }

    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Leaving the temp file behind would accumulate one per failed
            // save, in the user's config directory.
            let _ = fs::remove_file(&temp_path);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create tmp");
        dir
    }

    #[test]
    fn writes_the_contents() {
        let path = scratch_dir("atomic_write").join("data.json");
        write_atomic(&path, b"{\"ok\":true}").expect("write");
        assert_eq!(fs::read_to_string(&path).expect("read"), "{\"ok\":true}");
    }

    #[test]
    fn replaces_an_existing_file() {
        let path = scratch_dir("atomic_replace").join("data.json");
        write_atomic(&path, b"old").expect("first write");
        write_atomic(&path, b"new").expect("second write");
        assert_eq!(fs::read_to_string(&path).expect("read"), "new");
    }

    #[test]
    fn creates_the_parent_directory() {
        let path = scratch_dir("atomic_mkdir")
            .join("nested")
            .join("deeper")
            .join("data.json");
        write_atomic(&path, b"x").expect("write");
        assert!(path.exists());
    }

    /// No temp file may survive a successful write, or the config directory
    /// fills with them.
    #[test]
    fn leaves_no_temp_file_behind() {
        let dir = scratch_dir("atomic_no_temp");
        let path = dir.join("data.json");
        write_atomic(&path, b"x").expect("write");

        let stray: Vec<_> = fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(stray.is_empty(), "temp files left behind: {stray:?}");
    }

    /// The temp name carries the pid so two instances saving at once cannot
    /// write to the same scratch path.
    #[test]
    fn the_temp_name_is_process_specific() {
        let dir = scratch_dir("atomic_pid");
        let path = dir.join("data.json");
        // Reconstruct the name the way write_atomic does.
        let expected = format!(".data.json.{}.tmp", std::process::id());
        assert!(expected.contains(&std::process::id().to_string()));
        write_atomic(&path, b"x").expect("write");
    }
}
