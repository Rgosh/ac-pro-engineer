//! Content in packs: what came together, switched on and off together.
//!
//! **A pack is what its folders say it is.** Mod teams name every folder
//! they ship with one prefix — `rss_formula_hybrid_2021`, `vrc_…`, `urd_…` —
//! and Kunos does the same with `ks_` for the game and every DLC. Two or more
//! cars or circuits sharing a prefix are a pack; anything else is a mod on
//! its own. Nothing is guessed from names or authors, which change between
//! versions of the same mod: the folder is what the game itself goes by.
//!
//! **Off is not gone.** A pack switched off is its folders moved to
//! `content-off/`, exactly as one car is (see [`adding::switch_off`]), so it
//! comes back with one press and nothing is downloaded or reinstalled.

use super::adding;
use std::collections::BTreeMap;
use std::path::Path;

/// One thing in a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// `cars` or `tracks`.
    pub kind: &'static str,
    /// Its folder.
    pub id: String,
    /// Whether the game can see it.
    pub on: bool,
}

/// A pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    /// The prefix, or empty for the mods that stand alone.
    pub key: String,
    /// What to call it.
    pub name: String,
    pub items: Vec<Item>,
}

impl Pack {
    /// How many of its cars and circuits there are, and how many are on.
    pub fn counts(&self) -> (usize, usize, usize) {
        let cars = self.items.iter().filter(|i| i.kind == "cars").count();
        let tracks = self.items.len() - cars;
        let on = self.items.iter().filter(|i| i.on).count();
        (cars, tracks, on)
    }

    /// On, off, or some of each.
    pub fn state(&self) -> Option<bool> {
        let (_, _, on) = self.counts();
        match on {
            0 => Some(false),
            n if n == self.items.len() => Some(true),
            _ => None,
        }
    }
}

/// The names the well-known prefixes go by.
const KNOWN: &[(&str, &str)] = &[
    ("ks", "Kunos — the game and its DLC"),
    ("rss", "Race Sim Studio"),
    ("vrc", "VRC Modding Team"),
    ("urd", "URD"),
    ("rt", "Reboot Team"),
    ("tatuus", "Tatuus"),
    ("acf1", "ACF1"),
    ("asr", "ASR Formula"),
    ("lilski", "Lilski"),
    ("rsr", "RSR"),
    ("srp", "Shutoko Revival Project"),
];

/// A folder's prefix: what comes before its first underscore, when that is
/// a short word of letters and digits.
pub fn prefix(id: &str) -> Option<&str> {
    let (head, rest) = id.split_once('_')?;
    let usable = (2..=8).contains(&head.len())
        && head.chars().all(|c| c.is_ascii_alphanumeric())
        && !rest.is_empty();
    usable.then_some(head)
}

/// The packs among these cars and circuits, each `(kind, id, on)`: the
/// prefixes two or more share, then everything alone. Largest first, the
/// game's own at the top.
pub fn group(items: &[(&'static str, String, bool)]) -> Vec<Pack> {
    let mut by_prefix: BTreeMap<String, Vec<Item>> = BTreeMap::new();
    for (kind, id, on) in items {
        let key = prefix(id).unwrap_or("").to_ascii_lowercase();
        by_prefix.entry(key).or_default().push(Item {
            kind,
            id: id.clone(),
            on: *on,
        });
    }
    let mut packs = Vec::new();
    let mut alone: Vec<Item> = Vec::new();
    for (key, mut items) in by_prefix {
        items.sort_by(|a, b| a.kind.cmp(b.kind).then(a.id.cmp(&b.id)));
        if key.is_empty() || items.len() < 2 {
            alone.extend(items);
            continue;
        }
        let name = KNOWN
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, name)| name.to_string())
            .unwrap_or_else(|| key.to_uppercase());
        packs.push(Pack { key, name, items });
    }
    packs.sort_by(|a, b| {
        (b.key == "ks")
            .cmp(&(a.key == "ks"))
            .then(b.items.len().cmp(&a.items.len()))
            .then(a.name.cmp(&b.name))
    });
    if !alone.is_empty() {
        alone.sort_by(|a, b| a.kind.cmp(b.kind).then(a.id.cmp(&b.id)));
        packs.push(Pack {
            key: String::new(),
            name: "Mods on their own".to_string(),
            items: alone,
        });
    }
    packs
}

/// Every car and circuit in an install, on and off, as packs.
pub fn read(root: &Path) -> Vec<Pack> {
    let mut items: Vec<(&'static str, String, bool)> = Vec::new();
    for kind in ["cars", "tracks"] {
        if let Ok(entries) = std::fs::read_dir(root.join("content").join(kind)) {
            for entry in entries.flatten() {
                if entry.path().is_dir()
                    && let Some(name) = entry.file_name().to_str()
                {
                    items.push((kind, name.to_string(), true));
                }
            }
        }
    }
    for (kind, id, _) in adding::switched_off(root) {
        let kind = if kind == "cars" { "cars" } else { "tracks" };
        items.push((kind, id, false));
    }
    group(&items)
}

/// Switch one item on or off. Returns what happened, in words.
pub fn switch(root: &Path, item: &Item, on: bool) -> Result<(), String> {
    match (item.on, on) {
        (true, false) => {
            adding::switch_off(root, &root.join("content").join(item.kind).join(&item.id))
                .map(|_| ())
        }
        (false, true) => {
            adding::switch_on(root, &root.join(adding::OFF).join(item.kind).join(&item.id))
                .map(|_| ())
        }
        _ => Ok(()),
    }
}

/// Switch a whole pack on or off. Returns how many were moved, and the
/// first thing that refused when one did — the rest are still tried.
pub fn switch_pack(root: &Path, pack: &Pack, on: bool) -> (usize, Option<String>) {
    let mut moved = 0;
    let mut first_error = None;
    for item in pack.items.iter().filter(|i| i.on != on) {
        match switch(root, item, on) {
            Ok(()) => moved += 1,
            Err(why) => {
                first_error.get_or_insert(why);
            }
        }
    }
    (moved, first_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<(&'static str, String, bool)> {
        [
            ("cars", "ks_ferrari_488_gt3", true),
            ("cars", "ks_porsche_911_gt3_r", true),
            ("tracks", "ks_spa", true),
            ("cars", "rss_formula_hybrid_2021", true),
            ("cars", "rss_gt_m4", false),
            ("cars", "my_one_off", true),
            ("cars", "lotus", true),
            ("tracks", "imola", true),
        ]
        .into_iter()
        .map(|(k, id, on)| (k, id.to_string(), on))
        .collect()
    }

    /// **A prefix two folders share is a pack; one alone is not.** The
    /// game's own content is at the top, the stand-alone mods at the end,
    /// and a pack with some of it switched off says so.
    #[test]
    fn folders_that_share_a_prefix_are_a_pack() {
        let packs = group(&items());
        let names: Vec<&str> = packs.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Kunos — the game and its DLC",
                "Race Sim Studio",
                "Mods on their own"
            ]
        );
        let kunos = packs.first().map(Pack::counts);
        assert_eq!(kunos, Some((2, 1, 3)));
        assert_eq!(packs.get(1).and_then(Pack::state), None, "half on");
        assert_eq!(packs.first().and_then(Pack::state), Some(true));
        let alone: Vec<&str> = packs
            .last()
            .map(|p| p.items.iter().map(|i| i.id.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(alone, vec!["lotus", "my_one_off", "imola"]);
        assert_eq!(prefix("ks_spa"), Some("ks"));
        assert_eq!(prefix("lotus"), None);
        assert_eq!(prefix("a_b"), None, "a one-letter word is not a team");
    }

    /// **A whole pack goes off and comes back**, its folders moved aside
    /// and returned, and what was already off stays where it was.
    #[test]
    fn a_pack_switches_off_and_back_on() {
        let root = std::env::temp_dir().join(format!("ac-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for folder in [
            "content/cars/rss_a",
            "content/cars/rss_b",
            "content/tracks/rss_ring",
        ] {
            std::fs::create_dir_all(root.join(folder).join("data")).unwrap_or_default();
        }
        let packs = read(&root);
        let rss = packs.iter().find(|p| p.key == "rss").cloned();
        let Some(rss) = rss else {
            unreachable!("no pack: {packs:?}");
        };
        assert_eq!(rss.counts(), (2, 1, 3));
        assert_eq!(switch_pack(&root, &rss, false), (3, None));
        assert!(!root.join("content/cars/rss_a").exists());
        assert!(root.join("content-off/tracks/rss_ring/data").is_dir());
        let off = read(&root).into_iter().find(|p| p.key == "rss");
        assert_eq!(off.as_ref().and_then(Pack::state), Some(false));
        let Some(off) = off else { unreachable!() };
        assert_eq!(switch_pack(&root, &off, true), (3, None));
        assert!(root.join("content/cars/rss_b/data").is_dir());
        let _ = std::fs::remove_dir_all(&root);
    }
}
