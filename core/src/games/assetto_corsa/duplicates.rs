//! The same car or circuit installed twice, under two folder names.
//!
//! **A library of mods collects copies.** The same pack downloaded from two
//! sites, a car re-uploaded under a new folder name, a track installed once
//! by hand and once by a launcher — each copy takes its full size again and
//! shows up twice in every list, usually with the same name, so a driver
//! picks one at random and wonders why the livery they installed is missing.
//! Nothing in the game or in Content Manager says "these two are one".
//!
//! # What counts as the same
//!
//! * **Identical** — every file under the two folders has the same path
//!   (any case) and the same size, and the largest file, byte for byte, is
//!   the same. Keeping one loses nothing.
//! * **The same model** — the largest model (`.kn5`) is byte for byte the
//!   same and the rest differs: one copy has more liveries, a newer
//!   `data.acd`, a sound pack. The same car, but which to keep is the
//!   driver's call, so it is said and not decided.
//!
//! Sizes are compared first and bytes only between folders whose sizes
//! already agree, so the cost is one walk of the folders plus a read of the
//! few files that could be copies — not a hash of the whole library.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// How alike a group of folders is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Likeness {
    /// Every file the same: keeping one loses nothing.
    Identical,
    /// The same model, other files different.
    SameModel,
}

/// Folders that hold the same car or circuit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// `cars` or `tracks`.
    pub kind: &'static str,
    /// The folders, by name, sorted.
    pub ids: Vec<String>,
    /// The size of each, in the order of `ids`.
    pub bytes: Vec<u64>,
    pub likeness: Likeness,
}

impl Group {
    /// What deleting or switching off all but the largest would give back,
    /// for an identical group; nothing is promised for the others.
    pub fn wasted(&self) -> u64 {
        match self.likeness {
            Likeness::Identical => {
                self.bytes.iter().sum::<u64>() - self.bytes.iter().max().copied().unwrap_or(0)
            }
            Likeness::SameModel => 0,
        }
    }
}

/// One folder, as far as comparing it goes.
struct Folder {
    kind: &'static str,
    id: String,
    /// Every file: its path under the folder (lowercase, `/`) and size.
    files: Vec<(String, u64)>,
    /// The largest model file, and its size.
    model: Option<(PathBuf, u64)>,
    /// The largest file of any kind.
    largest: Option<(PathBuf, u64)>,
}

impl Folder {
    fn total(&self) -> u64 {
        self.files.iter().map(|(_, b)| b).sum()
    }
}

fn read(kind: &'static str, id: String, at: &Path) -> Folder {
    let mut files = Vec::new();
    let mut model: Option<(PathBuf, u64)> = None;
    let mut largest: Option<(PathBuf, u64)> = None;
    let mut stack = vec![at.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = path.symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            let bytes = meta.len();
            let relative = path
                .strip_prefix(at)
                .map(|p| p.to_string_lossy().replace('\\', "/").to_lowercase())
                .unwrap_or_default();
            if relative.ends_with(".kn5") && model.as_ref().is_none_or(|(_, b)| bytes > *b) {
                model = Some((path.clone(), bytes));
            }
            if largest.as_ref().is_none_or(|(_, b)| bytes > *b) {
                largest = Some((path.clone(), bytes));
            }
            files.push((relative, bytes));
        }
    }
    files.sort();
    Folder {
        kind,
        id,
        files,
        model,
        largest,
    }
}

/// Whether two files hold the same bytes. Sizes are assumed equal already.
fn same_bytes(a: &Path, b: &Path) -> bool {
    let (Ok(mut a), Ok(mut b)) = (std::fs::File::open(a), std::fs::File::open(b)) else {
        return false;
    };
    let mut left = vec![0u8; 1 << 16];
    let mut right = vec![0u8; 1 << 16];
    loop {
        let Ok(n) = a.read(&mut left) else {
            return false;
        };
        if n == 0 {
            return b.read(&mut right).is_ok_and(|m| m == 0);
        }
        if b.read_exact(&mut right[..n]).is_err() || left[..n] != right[..n] {
            return false;
        }
    }
}

/// Every group of folders holding the same car or circuit, largest waste
/// first. Folders that are not a car or circuit at all — Steam's sound-only
/// folders for DLC not owned — are never compared.
pub fn find(root: &Path) -> Vec<Group> {
    let mut groups = Vec::new();
    for kind in ["cars", "tracks"] {
        let Ok(entries) = std::fs::read_dir(root.join("content").join(kind)) else {
            continue;
        };
        let folders: Vec<Folder> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter(|e| super::health::present(kind, &e.path()))
            .filter_map(|e| {
                let id = e.file_name().to_str()?.to_string();
                Some(read(kind, id, &e.path()))
            })
            .collect();
        groups.extend(alike(&folders));
    }
    groups.sort_by(|a, b| {
        (std::cmp::Reverse(a.wasted()), a.likeness, &a.ids).cmp(&(
            std::cmp::Reverse(b.wasted()),
            b.likeness,
            &b.ids,
        ))
    });
    groups
}

/// Group folders by their model's size, then confirm by bytes.
fn alike(folders: &[Folder]) -> Vec<Group> {
    let mut by_model: BTreeMap<u64, Vec<&Folder>> = BTreeMap::new();
    for folder in folders {
        // A model under a few kilobytes is a placeholder, not a fingerprint.
        if let Some((_, bytes)) = folder.model
            && bytes >= 4096
        {
            by_model.entry(bytes).or_default().push(folder);
        }
    }
    let mut out = Vec::new();
    for candidates in by_model.values().filter(|c| c.len() > 1) {
        // Split the candidates into sets whose models are byte-identical.
        let mut sets: Vec<Vec<&Folder>> = Vec::new();
        for folder in candidates {
            let Some((model, _)) = &folder.model else {
                continue;
            };
            match sets.iter_mut().find(|set| {
                set.first()
                    .and_then(|first| first.model.as_ref())
                    .is_some_and(|(theirs, _)| same_bytes(model, theirs))
            }) {
                Some(set) => set.push(folder),
                None => sets.push(vec![folder]),
            }
        }
        for set in sets.into_iter().filter(|set| set.len() > 1) {
            let first = set[0];
            let identical = set.iter().skip(1).all(|other| {
                other.files == first.files
                    && match (&first.largest, &other.largest) {
                        (Some((a, _)), Some((b, _))) => same_bytes(a, b),
                        _ => false,
                    }
            });
            let mut members: Vec<&Folder> = set;
            members.sort_by(|a, b| a.id.cmp(&b.id));
            out.push(Group {
                kind: first.kind,
                ids: members.iter().map(|f| f.id.clone()).collect(),
                bytes: members.iter().map(|f| f.total()).collect(),
                likeness: match identical {
                    true => Likeness::Identical,
                    false => Likeness::SameModel,
                },
            });
        }
    }
    out
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

    fn car(cars: &Path, id: &str, model: &[u8], skins: &[&str]) {
        let at = cars.join(id);
        file(&at, "data.acd", b"data");
        file(&at, "car.kn5", model);
        file(&at, "ui/ui_car.json", br#"{"name":"Car"}"#);
        for skin in skins {
            file(&at, &format!("skins/{skin}/preview.jpg"), b"p");
        }
    }

    /// **A copy is found, a near copy is said, and two cars of the same size
    /// are not mistaken for one.**
    #[test]
    fn copies_are_found_and_only_copies() {
        let root = std::env::temp_dir().join(format!("ac-dupes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cars = root.join("content/cars");
        let model: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
        let mut other = model.clone();
        other[10_000] ^= 0xff; // same size, different car
        car(&cars, "mod_rx7", &model, &["red"]);
        car(&cars, "mod_rx7_copy", &model, &["red"]);
        car(&cars, "mod_rx7_more_skins", &model, &["red", "blue"]);
        car(&cars, "mod_other", &other, &["red"]);
        // Steam's sound-only folder is never a car to compare.
        file(&cars.join("ks_unowned"), "sfx/GUIDs.txt", b"x");

        let groups = find(&root);
        assert_eq!(groups.len(), 1, "{groups:#?}");
        let group = &groups[0];
        assert_eq!(
            group.ids,
            vec!["mod_rx7", "mod_rx7_copy", "mod_rx7_more_skins"]
        );
        assert_eq!(
            group.likeness,
            Likeness::SameModel,
            "one has an extra livery"
        );
        assert_eq!(group.wasted(), 0, "nothing is promised for a near copy");

        std::fs::remove_dir_all(cars.join("mod_rx7_more_skins")).unwrap();
        let groups = find(&root);
        assert_eq!(groups[0].likeness, Likeness::Identical);
        assert_eq!(groups[0].wasted(), groups[0].bytes[0]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn bytes_are_compared_to_the_end() {
        let at = std::env::temp_dir().join(format!("ac-dupes-bytes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        let long: Vec<u8> = (0..200_000u32).map(|i| (i % 13) as u8).collect();
        let mut late = long.clone();
        *late.last_mut().unwrap() ^= 1;
        file(&at, "a", &long);
        file(&at, "b", &long);
        file(&at, "c", &late);
        assert!(same_bytes(&at.join("a"), &at.join("b")));
        assert!(!same_bytes(&at.join("a"), &at.join("c")));
        let _ = std::fs::remove_dir_all(&at);
    }
}
