//! Reading Assetto Corsa's car catalogue off disk.
//!
//! Everything here is AC's own file layout: `content/cars/<id>/ui/ui_car.json`,
//! its spelling of "bhp", and the fact that a modded car folder may capitalise
//! `UI` however it likes. The [`CarSpecs`] that come out are the neutral shape
//! the rest of the program works in — the same split as the telemetry, for the
//! same reason.
//!
//! ACC keeps none of this: no `content/cars`, no `ui_car.json`, and its car
//! list is baked into the executable. Whatever it does keep goes in its own
//! folder beside this one, and nothing above has to learn a second layout.

use crate::games::catalogue::CarSpecs;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Every car installed under an Assetto Corsa root.
///
/// An empty list is the normal answer for a machine with no game installed and
/// is not an error: the car specs sharpen a reference lap time and nothing
/// depends on them existing.
pub fn scan_cars(ac_root: &Path) -> Vec<CarSpecs> {
    let cars_dir = ac_root.join("content").join("cars");
    if !cars_dir.exists() {
        return Vec::new();
    }

    let mut cars = Vec::new();
    for entry in WalkDir::new(&cars_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let car_id = entry.file_name().to_string_lossy().to_string();
        let ui_dir = find_case_insensitive(entry.path(), "ui");
        let ui_path = ui_dir.and_then(|d| find_case_insensitive(&d, "ui_car.json"));

        if let Some(p) = ui_path
            && let Ok(content) = fs::read_to_string(p)
            && let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content)
        {
            let name = json_val["name"].as_str().unwrap_or("Unknown").to_string();
            let brand = json_val["brand"].as_str().unwrap_or("Unknown").to_string();
            let desc = json_val["description"].as_str().unwrap_or("").to_string();
            let class = json_val["class"].as_str().unwrap_or("street").to_string();

            let (power_s, torque_s, weight_s) = if let Some(specs) = json_val.get("specs") {
                (
                    specs["bhp"].as_str().unwrap_or("0").to_string(),
                    specs["torque"].as_str().unwrap_or("0").to_string(),
                    specs["weight"].as_str().unwrap_or("1000").to_string(),
                )
            } else {
                ("0".to_string(), "0".to_string(), "1000".to_string())
            };

            let power_clean = extract_number(&power_s).unwrap_or(100.0);
            let weight_clean = extract_number(&weight_s).unwrap_or(1000.0);

            cars.push(CarSpecs {
                id: car_id,
                name,
                brand,
                description: desc,
                class,
                power: power_s,
                torque: torque_s,
                weight: weight_s,
                year: json_val["year"].as_i64().map(|y| y as i32),
                power_hp: power_clean,
                weight_kg: weight_clean,
                // The car's own answer to the question the class table only
                // guesses at. Read here because this is the one walk of the
                // content folder, and skipped over in the archive without
                // decrypting anything but the one file it wants.
                ideal_pressure: super::car_data::ideal_pressures(entry.path()),
                // And what it lets you change, from the same walk and the
                // same archive, so a screen cannot recommend a part this car
                // has not got.
                adjustable: super::car_data::adjustables(entry.path()),
            });
        }
    }
    cars
}

/// AC writes its specs as human strings — "552bhp", "1 245 kg" — so the number
/// is dug out of whatever the car's author typed.
fn extract_number(s: &str) -> Option<f32> {
    let num_str: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    num_str.parse().ok()
}

/// Every track installed under an Assetto Corsa root, layouts counted apart.
///
/// **A layout is an entry, not a footnote.** Of the twenty-one circuits
/// installed on the machine this was written on, eleven have more than one
/// layout, and Barcelona's grand prix and moto layouts are different lengths
/// with different corners. Offering the folder and asking a second question
/// would make a list nobody can simply scroll and pick from.
///
/// A track with one layout keeps `ui/ui_track.json` directly; one with several
/// keeps `ui/<layout>/ui_track.json`. Both spellings are read, and a folder
/// with neither is skipped rather than listed as "Unknown" — an entry that
/// cannot be loaded is worse than an entry that is not there.
pub fn scan_tracks(ac_root: &Path) -> Vec<crate::games::catalogue::TrackListing> {
    let tracks_dir = ac_root.join("content").join("tracks");
    if !tracks_dir.exists() {
        return Vec::new();
    }

    let mut found = Vec::new();
    for entry in WalkDir::new(&tracks_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let Some(ui_dir) = find_case_insensitive(entry.path(), "ui") else {
            continue;
        };

        // The one-layout spelling first, because it is the simpler shape and
        // a track that has it has nothing else to offer.
        if let Some(path) = find_case_insensitive(&ui_dir, "ui_track.json")
            && let Some(listing) = read_track(&path, &id, "")
        {
            found.push(listing);
            continue;
        }

        let Ok(layouts) = fs::read_dir(&ui_dir) else {
            continue;
        };
        let mut mine: Vec<crate::games::catalogue::TrackListing> = layouts
            .flatten()
            .filter(|layout| layout.path().is_dir())
            .filter_map(|layout| {
                let config = layout.file_name().to_string_lossy().to_string();
                let path = find_case_insensitive(&layout.path(), "ui_track.json")?;
                read_track(&path, &id, &config)
            })
            .collect();
        name_the_layouts(&mut mine);
        // Within one circuit, by the layout's own name, so the list is stable
        // between runs — `read_dir` is not ordered.
        mine.sort_by(|a, b| a.name.cmp(&b.name));
        found.extend(mine);
    }

    found.sort_by_key(|one| one.name.to_lowercase());
    found
}

/// One `ui_track.json`, or `None` when it is not one.
fn read_track(
    path: &Path,
    id: &str,
    config: &str,
) -> Option<crate::games::catalogue::TrackListing> {
    let text = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let name = json["name"].as_str().filter(|name| !name.is_empty())?;

    Some(crate::games::catalogue::TrackListing {
        id: id.to_string(),
        config: config.to_string(),
        // The track's own name, untouched here. Whether it needs the layout
        // appending is a question about its siblings, and is answered once all
        // of them have been read — see `name_the_layouts`.
        name: name.to_string(),
        country: json["country"].as_str().unwrap_or("").to_string(),
        length_m: metres(json["length"].as_str().unwrap_or("0")),
        pitboxes: extract_number(json["pitboxes"].as_str().unwrap_or("0")).unwrap_or(0.0) as u32,
    })
}

/// A track's length, whichever unit it was written in.
///
/// **Two tracks that ship with the game disagree.** Imola says `"4909"` and
/// Laguna Seca says `"3.602"`, and both mean about the same distance. Read
/// literally the second is three metres, which is not a circuit and is not a
/// typo anybody is going to fix — so the number is read and then judged.
///
/// A hundred metres is the line: the shortest thing in the game is a two
/// hundred metre drag strip, and no circuit is shorter than that in metres or
/// longer than that in kilometres.
fn metres(text: &str) -> u32 {
    let value = extract_number(text).unwrap_or(0.0);
    match value < 100.0 {
        true => (value * 1_000.0) as u32,
        false => value as u32,
    }
}

/// Append the layout to a name only where the names do not already differ.
///
/// **Most of them already do.** The Nürburgring's four layouts call themselves
/// GP, GP (GT), Sprint and Sprint (GT); appending the folder gives
/// "Nurburgring - GP — gp a", which is worse than what the track said about
/// itself. Where two layouts really do share a name, the folder is the only
/// thing there is to tell them apart, and then it is worth the noise.
fn name_the_layouts(mine: &mut [crate::games::catalogue::TrackListing]) {
    let clashes: Vec<String> = mine
        .iter()
        .filter(|one| mine.iter().filter(|other| other.name == one.name).count() > 1)
        .map(|one| one.name.clone())
        .collect();

    for one in mine.iter_mut() {
        if clashes.contains(&one.name) {
            one.name = format!("{} — {}", one.name, pretty_layout(&one.config));
        }
    }
}

/// `layout_gp` as `gp`, because the prefix is on every one of them and says
/// nothing.
fn pretty_layout(config: &str) -> String {
    config
        .strip_prefix("layout_")
        .unwrap_or(config)
        .replace('_', " ")
}

/// Mod folders capitalise `UI` and `ui_car.json` inconsistently, and Linux
/// filesystems care where Windows does not.
fn find_case_insensitive(base: &Path, name: &str) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
            {
                return Some(entry.path());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {

    fn a_track(at: &std::path::Path, id: &str, config: &str, name: &str, length: &str) {
        let ui = match config.is_empty() {
            true => at.join("content/tracks").join(id).join("ui"),
            false => at.join("content/tracks").join(id).join("ui").join(config),
        };
        fs::create_dir_all(&ui).expect("a fixture");
        fs::write(
            ui.join("ui_track.json"),
            format!(r#"{{"name":"{name}","country":"Italy","length":"{length}","pitboxes":"24"}}"#),
        )
        .expect("a fixture");
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ac-content-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    /// **Both spellings, because half the circuits use each.** A track with
    /// one layout keeps `ui/ui_track.json`; one with several keeps
    /// `ui/<layout>/ui_track.json`, and the layout is what the game is told.
    #[test]
    fn a_layout_is_an_entry_of_its_own() {
        let dir = scratch("layouts");
        a_track(&dir, "imola", "", "Imola", "4909");
        a_track(&dir, "ks_barcelona", "layout_gp", "Barcelona GP", "4655");
        a_track(
            &dir,
            "ks_barcelona",
            "layout_moto",
            "Barcelona Moto",
            "4727",
        );

        let found = scan_tracks(&dir);
        assert_eq!(found.len(), 3, "{found:?}");

        let plain = found.iter().find(|t| t.id == "imola").expect("Imola");
        assert_eq!(plain.config, "", "a single-layout track carries no config");

        let gp = found
            .iter()
            .find(|t| t.config == "layout_gp")
            .expect("the GP layout");
        assert_eq!(gp.id, "ks_barcelona");
        assert_eq!(gp.as_game_asks(), ("ks_barcelona", "layout_gp"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// **Two tracks that ship with the game disagree about the unit.** Imola
    /// says "4909" and Laguna Seca says "3.602"; read literally the second is
    /// three metres, which is not a circuit.
    #[test]
    fn a_length_in_kilometres_is_not_read_as_three_metres() {
        let dir = scratch("length");
        a_track(&dir, "imola", "", "Imola", "4909");
        a_track(&dir, "ks_laguna_seca", "", "Laguna Seca", "3.602");

        let found = scan_tracks(&dir);
        let by = |name: &str| {
            found
                .iter()
                .find(|t| t.name == name)
                .map(|one| one.length_m)
                .unwrap_or_default()
        };
        assert_eq!(by("Imola"), 4909);
        assert_eq!(by("Laguna Seca"), 3602, "kilometres were read as metres");
        let _ = fs::remove_dir_all(&dir);
    }

    /// **Most layouts already name themselves.** The Nürburgring's four call
    /// themselves GP, GP (GT), Sprint and Sprint (GT), and appending the
    /// folder to those gives "Nurburgring - GP — gp a", which is worse than
    /// what the track said about itself.
    #[test]
    fn the_layout_is_appended_only_where_the_names_would_collide() {
        let dir = scratch("naming");
        a_track(
            &dir,
            "ks_nurburgring",
            "layout_gp_a",
            "Nurburgring - GP",
            "5148",
        );
        a_track(
            &dir,
            "ks_nurburgring",
            "layout_gp_b",
            "Nurburgring - GP (GT)",
            "5137",
        );
        a_track(&dir, "twins", "layout_one", "Twin Ring", "3000");
        a_track(&dir, "twins", "layout_two", "Twin Ring", "3100");

        let found = scan_tracks(&dir);
        assert!(
            found.iter().any(|t| t.name == "Nurburgring - GP"),
            "a name that already differs was decorated: {found:?}"
        );
        assert!(
            found.iter().any(|t| t.name == "Twin Ring — one"),
            "two layouts sharing a name were left indistinguishable: {found:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// An entry that cannot be loaded is worse than an entry that is not
    /// there: it is a row somebody presses that does nothing.
    #[test]
    fn a_folder_with_no_description_is_skipped_rather_than_listed_as_unknown() {
        let dir = scratch("empty");
        a_track(&dir, "imola", "", "Imola", "4909");
        fs::create_dir_all(dir.join("content/tracks/half_a_mod/ui")).expect("a fixture");

        let found = scan_tracks(&dir);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].id, "imola");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_machine_with_no_game_lists_nothing_rather_than_failing() {
        assert!(scan_tracks(std::path::Path::new("/nowhere-at-all")).is_empty());
    }
    use super::*;

    #[test]
    fn a_number_is_dug_out_of_whatever_the_author_typed() {
        assert_eq!(extract_number("552bhp"), Some(552.0));
        assert_eq!(extract_number("1245 kg"), Some(1245.0));
        assert_eq!(extract_number("N/A"), None);
    }

    /// No game installed is an empty catalogue, not a failure.
    #[test]
    fn a_root_with_no_cars_scans_to_nothing() {
        assert!(scan_cars(Path::new("/nonexistent/assettocorsa")).is_empty());
    }
}
