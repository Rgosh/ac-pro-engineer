//! A car or circuit packed into one archive, ready to send to somebody.
//!
//! **Sharing a mod is how a league gets everybody on the same content**, and
//! the usual way is a zip somebody made by hand: the wrong folder level, a
//! `Thumbs.db`, forty liveries nobody asked for, a readme in place of the car.
//! Content Manager's paid version packs it for you. This does, for free, and
//! packs it in the one layout every tool installs from without asking —
//! `content/cars/<id>/…` or `content/tracks/<id>/…` — which is exactly what
//! [`super::adding::whats_in`] reads back, so a pack made here is a drop
//! anybody's launcher, Content Manager or a plain unzip into the game folder
//! puts in the right place.
//!
//! What is left out is what nobody's game reads: the desktop's own litter
//! (`Thumbs.db`, `desktop.ini`, `.DS_Store`), backups and temporary files.
//! For a car, the liveries can be narrowed to the ones chosen. Pictures that
//! are already compressed are stored as they are rather than squeezed again
//! for nothing.

use std::io::Write;
use std::path::{Path, PathBuf};

/// What came of packing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packed {
    /// The archive written.
    pub archive: PathBuf,
    /// How many files went in.
    pub files: usize,
    /// The files' size before, and the archive's after.
    pub bytes_in: u64,
    pub bytes_out: u64,
}

/// Whether a file is litter no game reads.
fn litter(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(lower.as_str(), "thumbs.db" | "desktop.ini" | ".ds_store")
        || lower.ends_with(".bak")
        || lower.ends_with(".tmp")
        || lower.ends_with('~')
}

/// Whether a file is already compressed, so deflating it again is time spent
/// for a few bytes.
fn already_small(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        ".jpg", ".jpeg", ".png", ".zip", ".7z", ".rar", ".ogg", ".mp3", ".bank",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

/// Pack `folder` — a car's or a circuit's, `kind` being `cars` or `tracks` —
/// into `<into>/<id>.zip`. `skins`, for a car, keeps only those liveries;
/// `None` keeps them all.
///
/// Written beside and renamed into place, so a pack interrupted halfway is
/// no archive at all rather than a broken one somebody sends on.
pub fn pack(
    kind: &str,
    folder: &Path,
    into: &Path,
    skins: Option<&[String]>,
) -> Result<Packed, String> {
    if kind != "cars" && kind != "tracks" {
        return Err(format!("not a kind that packs: {kind}"));
    }
    let id = folder
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the folder has no name")?
        .to_string();
    if !super::health::present(kind, folder) {
        return Err(format!("{id} is not a whole car or circuit"));
    }
    std::fs::create_dir_all(into).map_err(|why| why.to_string())?;
    let archive = into.join(format!("{id}.zip"));
    let partial = into.join(format!(".{id}.zip.{}.partial", std::process::id()));

    let mut files = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|why| why.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = path.symlink_metadata() else {
                continue;
            };
            let relative = path
                .strip_prefix(folder)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if meta.is_dir() {
                // A livery not chosen is left out whole.
                if let Some(keep) = skins
                    && let Some(skin) = relative.strip_prefix("skins/").or_else(|| {
                        relative
                            .to_lowercase()
                            .starts_with("skins/")
                            .then(|| relative.get(6..).unwrap_or(""))
                    })
                    && !skin.contains('/')
                    && !keep.iter().any(|k| k.eq_ignore_ascii_case(skin))
                {
                    continue;
                }
                stack.push(path);
            } else if meta.is_file() && !litter(&entry.file_name().to_string_lossy()) {
                files.push((relative, path, meta.len()));
            }
        }
    }
    files.sort();

    let written = (|| -> Result<(), String> {
        let out = std::fs::File::create(&partial).map_err(|why| why.to_string())?;
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(out));
        for (relative, path, _) in &files {
            let method = match already_small(relative) {
                true => zip::CompressionMethod::Stored,
                false => zip::CompressionMethod::Deflated,
            };
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(method)
                .large_file(true);
            zip.start_file(format!("content/{kind}/{id}/{relative}"), options)
                .map_err(|why| why.to_string())?;
            let mut from = std::fs::File::open(path).map_err(|why| why.to_string())?;
            std::io::copy(&mut from, &mut zip).map_err(|why| why.to_string())?;
        }
        let mut done = zip.finish().map_err(|why| why.to_string())?;
        done.flush().map_err(|why| why.to_string())?;
        Ok(())
    })();
    if let Err(why) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(why);
    }
    std::fs::rename(&partial, &archive).map_err(|why| why.to_string())?;
    Ok(Packed {
        bytes_out: std::fs::metadata(&archive).map(|m| m.len()).unwrap_or(0),
        archive,
        files: files.len(),
        bytes_in: files.iter().map(|(_, _, b)| b).sum(),
    })
}

/// The liveries a car has, by folder name, sorted — what a packer chooses
/// from.
pub fn liveries(car: &Path) -> Vec<String> {
    let skins = std::fs::read_dir(car).ok().and_then(|entries| {
        entries
            .flatten()
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("skins")
            })
            .map(|e| e.path())
    });
    let mut found: Vec<String> = skins
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    found.sort_by_key(|name| name.to_lowercase());
    found
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn file(at: &Path, name: &str, body: &[u8]) {
        let path = at.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    /// **A pack is a drop any launcher installs**: what `whats_in` reads back
    /// is one car, under its own folder name, with the litter and the
    /// liveries not chosen left out.
    #[test]
    fn a_packed_car_reads_back_as_that_car() {
        let root = std::env::temp_dir().join(format!("ac-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let car = root.join("content/cars/mod_rx7");
        file(&car, "data.acd", b"data");
        file(&car, "rx7.kn5", &vec![3u8; 50_000]);
        file(&car, "ui/ui_car.json", br#"{"name":"RX-7"}"#);
        file(&car, "skins/red/preview.jpg", b"red");
        file(&car, "skins/blue/preview.jpg", b"blue");
        file(&car, "skins/red/Thumbs.db", b"litter");
        file(&car, "data.acd.bak", b"old");

        let out = root.join("packs");
        let packed = pack("cars", &car, &out, Some(&["RED".to_string()])).unwrap();
        assert_eq!(packed.archive, out.join("mod_rx7.zip"));
        assert_eq!(packed.files, 4, "data, model, ui, the red livery");
        assert!(packed.bytes_out < packed.bytes_in, "the model compresses");

        let names: Vec<String> = {
            let mut zip =
                zip::ZipArchive::new(std::fs::File::open(&packed.archive).unwrap()).unwrap();
            (0..zip.len())
                .map(|i| zip.by_index(i).unwrap().name().unwrap().to_string())
                .collect()
        };
        assert!(names.contains(&"content/cars/mod_rx7/skins/red/preview.jpg".to_string()));
        assert!(!names.iter().any(|n| n.contains("blue")), "{names:?}");
        assert!(
            !names
                .iter()
                .any(|n| n.contains("Thumbs") || n.ends_with(".bak"))
        );

        let found = super::super::adding::whats_in(&packed.archive);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, super::super::adding::Kind::Car);
        assert_eq!(found[0].id, "mod_rx7");
        assert_eq!(liveries(&car), vec!["blue", "red"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Something that is not a car is not packed as one.
    #[test]
    fn only_a_whole_car_or_circuit_is_packed() {
        let root = std::env::temp_dir().join(format!("ac-pack-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        file(&root.join("sfx_only"), "sfx/GUIDs.txt", b"x");
        assert!(pack("cars", &root.join("sfx_only"), &root, None).is_err());
        assert!(pack("weather", &root, &root, None).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
