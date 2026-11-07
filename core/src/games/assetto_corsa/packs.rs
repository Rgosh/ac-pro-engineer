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
    /// The folder inside its pack it is shown in: a car's brand, or
    /// "Circuits" — see [`read`].
    pub group: String,
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

/// Kunos's own cars and circuits whose folders have no `ks_` in front.
///
/// **The game's first cars and circuits predate the prefix.** Kunos began
/// writing `ks_` on everything it made later, and left the originals as they
/// were — so `lotus_exige_s`, `bmw_m3_e30` and `ferrari_458` share a prefix
/// with nothing but their own brand, and were shown as packs called LOTUS,
/// BMW and FERRARI, with the Abarth, the KTM and Monza among "mods on their
/// own". Checked against a real install: these are the ones.
pub const KUNOS_UNPREFIXED: &[&str] = &[
    // Cars.
    "abarth500",
    "abarth500_s1",
    "alfa_romeo_giulietta_qv",
    "alfa_romeo_giulietta_qv_le",
    "bmw_1m",
    "bmw_1m_s3",
    "bmw_m3_e30",
    "bmw_m3_e30_drift",
    "bmw_m3_e30_dtm",
    "bmw_m3_e30_gra",
    "bmw_m3_e30_s1",
    "bmw_m3_e92",
    "bmw_m3_e92_drift",
    "bmw_m3_e92_s1",
    "bmw_m3_gt2",
    "bmw_z4",
    "bmw_z4_drift",
    "bmw_z4_gt3",
    "bmw_z4_s1",
    "ferrari_312t",
    "ferrari_458",
    "ferrari_458_gt2",
    "ferrari_458_s3",
    "ferrari_599xxevo",
    "ferrari_f40",
    "ferrari_f40_s3",
    "ferrari_laferrari",
    "ktm_xbow_r",
    "lotus_2_eleven",
    "lotus_2_eleven_gt4",
    "lotus_49",
    "lotus_98t",
    "lotus_elise_sc",
    "lotus_elise_sc_s1",
    "lotus_elise_sc_s2",
    "lotus_evora_gtc",
    "lotus_evora_gte",
    "lotus_evora_gte_carbon",
    "lotus_evora_gx",
    "lotus_evora_s",
    "lotus_evora_s_s2",
    "lotus_exige_240",
    "lotus_exige_240_s3",
    "lotus_exige_s",
    "lotus_exige_s_roadster",
    "lotus_exige_scura",
    "lotus_exige_v6_cup",
    "lotus_exos_125",
    "lotus_exos_125_s1",
    "mclaren_mp412c",
    "mclaren_mp412c_gt3",
    "mercedes_sls",
    "mercedes_sls_gt3",
    "p4-5_2011",
    "pagani_huayra",
    "pagani_zonda_r",
    "ruf_yellowbird",
    "shelby_cobra_427sc",
    "tatuusfa1",
    // Circuits.
    "drift",
    "imola",
    "magione",
    "monza",
    "monza66",
    "mugello",
    "spa",
    "trento-bondone",
    "vallelunga",
    "silverstone-international",
];

/// Whether a folder is Kunos's own: `ks_` in front, or one of the
/// originals from before the prefix.
pub fn is_kunos(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    lower.starts_with("ks_") || KUNOS_UNPREFIXED.contains(&lower.as_str())
}

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
        let key = match is_kunos(id) {
            true => "ks".to_string(),
            false => prefix(id).unwrap_or("").to_ascii_lowercase(),
        };
        by_prefix.entry(key).or_default().push(Item {
            kind,
            id: id.clone(),
            on: *on,
            group: String::new(),
        });
    }
    let mut packs = Vec::new();
    let mut alone: Vec<Item> = Vec::new();
    for (key, mut items) in by_prefix {
        items.sort_by(|a, b| a.kind.cmp(b.kind).then(a.id.cmp(&b.id)));
        if key.is_empty() || (items.len() < 2 && key != "ks") {
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

/// Every car and circuit in an install, on and off, as packs — each item
/// in the folder it is shown in inside its pack: a car under its brand, as
/// its `ui_car.json` says it, a circuit under "Circuits".
///
/// **Folders that are not a car or circuit at all are left out**: Steam's
/// folder for a DLC somebody does not own holds only its sounds — see
/// [`health::present`](super::health::present).
pub fn read(root: &Path) -> Vec<Pack> {
    let mut items: Vec<(&'static str, String, bool)> = Vec::new();
    let mut at: std::collections::HashMap<String, std::path::PathBuf> = Default::default();
    for kind in ["cars", "tracks"] {
        if let Ok(entries) = std::fs::read_dir(root.join("content").join(kind)) {
            for entry in entries.flatten() {
                if entry.path().is_dir()
                    && super::health::present(kind, &entry.path())
                    && let Some(name) = entry.file_name().to_str()
                {
                    items.push((kind, name.to_string(), true));
                    at.insert(format!("{kind}/{name}"), entry.path());
                }
            }
        }
    }
    for (kind, id, path) in adding::switched_off(root) {
        let kind = if kind == "cars" { "cars" } else { "tracks" };
        at.insert(format!("{kind}/{id}"), path);
        items.push((kind, id, false));
    }
    let mut packs = group(&items);
    for pack in &mut packs {
        for item in &mut pack.items {
            item.group = match item.kind {
                "cars" => at
                    .get(&format!("cars/{}", item.id))
                    .and_then(|path| brand_of(path))
                    .unwrap_or_else(|| "Other".to_string()),
                _ => "Circuits".to_string(),
            };
        }
        pack.items.sort_by(|a, b| {
            (a.kind, a.group.to_lowercase(), &a.id).cmp(&(b.kind, b.group.to_lowercase(), &b.id))
        });
    }
    packs
}

/// A car's brand, from its `ui_car.json`.
fn brand_of(car: &Path) -> Option<String> {
    let ui = std::fs::read_dir(car)
        .ok()?
        .flatten()
        .find(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case("ui"))
        })?
        .path();
    let file = std::fs::read_dir(&ui)
        .ok()?
        .flatten()
        .find(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case("ui_car.json"))
        })?
        .path();
    let raw = std::fs::read_to_string(file).ok()?;
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let value: serde_json::Value = serde_json::from_str(&cleaned).ok()?;
    let brand = value.get("brand")?.as_str()?.trim();
    (!brand.is_empty()).then(|| brand.to_string())
}

impl Pack {
    /// Its items by the folder each is shown in, in order: each folder's
    /// name and the items in it.
    pub fn folders(&self) -> Vec<(String, Vec<&Item>)> {
        let mut out: Vec<(String, Vec<&Item>)> = Vec::new();
        for item in &self.items {
            match out.iter_mut().find(|(name, _)| *name == item.group) {
                Some((_, list)) => list.push(item),
                None => out.push((item.group.clone(), vec![item])),
            }
        }
        out
    }
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
            ("cars", "lotus_exige_s", true),
            ("cars", "bmw_m3_e30", true),
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
        // The originals without `ks_` are Kunos's too: two cars and Imola.
        let kunos = packs.first().map(Pack::counts);
        assert_eq!(kunos, Some((4, 2, 6)));
        assert!(is_kunos("lotus_exige_s") && is_kunos("KS_SPA") && !is_kunos("lotus_mod"));
        assert_eq!(packs.get(1).and_then(Pack::state), None, "half on");
        assert_eq!(packs.first().and_then(Pack::state), Some(true));
        let alone: Vec<&str> = packs
            .last()
            .map(|p| p.items.iter().map(|i| i.id.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(alone, vec!["lotus", "my_one_off"]);
        assert_eq!(prefix("ks_spa"), Some("ks"));
        assert_eq!(prefix("lotus"), None);
        assert_eq!(prefix("a_b"), None, "a one-letter word is not a team");
    }

    /// **Inside a pack, a folder per brand and one for circuits**, and a
    /// DLC folder with only its sounds is not listed at all.
    #[test]
    fn a_pack_opens_into_folders_and_unowned_dlc_is_left_out() {
        let root = std::env::temp_dir().join(format!("ac-packs-folders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let write = |path: &str, body: &str| {
            let at = root.join(path);
            if let Some(parent) = at.parent() {
                std::fs::create_dir_all(parent).unwrap_or_default();
            }
            std::fs::write(at, body).unwrap_or_default();
        };
        write(
            "content/cars/ks_porsche_911_gt3_r_2016/ui/ui_car.json",
            r#"{"brand":"Porsche"}"#,
        );
        write(
            "content/cars/ks_ferrari_488_gt3/ui/ui_car.json",
            r#"{"brand":"Ferrari"}"#,
        );
        write(
            "content/cars/ferrari_458/ui/ui_car.json",
            r#"{"brand":"Ferrari"}"#,
        );
        write("content/cars/ks_porsche_919_hybrid_2016/sfx/GUIDs.txt", "x");
        write("content/tracks/ks_spa/spa.kn5", "m");
        let packs = read(&root);
        let Some(kunos) = packs.iter().find(|p| p.key == "ks") else {
            unreachable!("Kunos's pack: {packs:?}");
        };
        let folders: Vec<(String, usize)> = kunos
            .folders()
            .into_iter()
            .map(|(name, items)| (name, items.len()))
            .collect();
        assert_eq!(
            folders,
            vec![
                ("Ferrari".to_string(), 2),
                ("Porsche".to_string(), 1),
                ("Circuits".to_string(), 1)
            ]
        );
        assert!(
            kunos
                .items
                .iter()
                .all(|i| i.id != "ks_porsche_919_hybrid_2016"),
            "a DLC not owned is not an installed car"
        );
        let _ = std::fs::remove_dir_all(&root);
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
        // A circuit is its model, not a `data` folder alone.
        std::fs::write(root.join("content/tracks/rss_ring/ring.kn5"), "m").unwrap_or_default();
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
