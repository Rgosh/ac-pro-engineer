//! The apps the game draws on screen while driving: which there are, and
//! which are switched on.
//!
//! **Two kinds, kept in two places.** Python apps live in `apps/python/<name>/`
//! with a script of the same name, and the game loads only the ones
//! `cfg/python.ini` in its Documents folder marks `ACTIVE=1` — the list of
//! ticks in the game's own General settings. Lua apps are Custom Shaders
//! Patch's, in `apps/lua/<name>/` with a manifest saying what they are, and
//! the patch offers every one of them in the game without a switch.
//!
//! A Python app nobody ticked does not exist as far as the game is
//! concerned, and one that crashes on load takes the session with it — so
//! "which are on" is worth seeing, and changing, without starting the game
//! to reach its settings.

use std::path::{Path, PathBuf};

/// One app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// Its folder.
    pub id: String,
    /// What it calls itself, or the folder made readable.
    pub name: String,
    pub version: String,
    pub author: String,
    pub about: String,
    /// Python: whether the game loads it. Lua: always true — the patch
    /// offers every Lua app, and there is no file that says otherwise.
    pub on: bool,
    pub lua: bool,
}

/// Where the game keeps which Python apps are on.
pub fn python_file(game_documents: &Path) -> PathBuf {
    game_documents.join("cfg").join("python.ini")
}

/// Every app in a game folder: Python first, then Lua, each by name.
pub fn list(root: &Path, game_documents: Option<&Path>) -> Vec<App> {
    let ticks = game_documents
        .and_then(|docs| std::fs::read_to_string(python_file(docs)).ok())
        .unwrap_or_default();
    let mut found = Vec::new();
    for (lua, folder) in [(false, "python"), (true, "lua")] {
        let Ok(entries) = std::fs::read_dir(root.join("apps").join(folder)) else {
            continue;
        };
        let mut apps: Vec<App> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let id = entry.file_name().to_string_lossy().into_owned();
                if !path.is_dir() || id.starts_with('.') {
                    return None;
                }
                match lua {
                    // A Lua app is a folder with a manifest.
                    true => {
                        let manifest = std::fs::read_to_string(path.join("manifest.ini")).ok()?;
                        let said = |key: &str| {
                            super::ini::value(&manifest, "ABOUT", key).unwrap_or_default()
                        };
                        Some(App {
                            name: Some(said("NAME"))
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| readable(&id)),
                            version: said("VERSION"),
                            author: said("AUTHOR"),
                            about: said("DESCRIPTION"),
                            on: true,
                            lua,
                            id,
                        })
                    }
                    // A Python app is a folder with a script of its name.
                    false => {
                        let script = std::fs::read_dir(&path).ok()?.flatten().any(|inside| {
                            inside
                                .file_name()
                                .to_string_lossy()
                                .eq_ignore_ascii_case(&format!("{id}.py"))
                        });
                        script.then(|| App {
                            name: readable(&id),
                            version: String::new(),
                            author: String::new(),
                            about: String::new(),
                            on: super::ini::value(&ticks, &id, "ACTIVE")
                                .is_some_and(|v| v.trim() == "1"),
                            lua,
                            id,
                        })
                    }
                }
            })
            .collect();
        apps.sort_by_key(|app| app.name.to_lowercase());
        found.extend(apps);
    }
    found
}

/// Switch a Python app on or off, in the game's own file.
///
/// **Under the name the file already uses for it.** The game writes the
/// section in capitals; a file somebody edited may not. A second section for
/// the same app in another case would leave the game reading whichever it
/// meets first.
pub fn set_python(game_documents: &Path, id: &str, on: bool) -> Result<PathBuf, String> {
    let file = python_file(game_documents);
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let section = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix('[')?.strip_suffix(']'))
        .find(|name| name.trim().eq_ignore_ascii_case(id))
        .map(|name| name.trim().to_string())
        .unwrap_or_else(|| id.to_uppercase());
    super::ini::set_in_file(
        &file,
        &section,
        "ACTIVE",
        match on {
            true => "1",
            false => "0",
        },
    )
}

/// `race_essentials` as "Race essentials".
fn readable(name: &str) -> String {
    let words = name.replace(['_', '-'], " ");
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
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

    #[test]
    fn apps_are_listed_with_the_games_own_ticks_and_switched_in_its_file() {
        let dir = std::env::temp_dir().join(format!("acpe-apps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (root, docs) = (dir.join("game"), dir.join("documents"));
        file(&root, "apps/python/helicorsa/helicorsa.py", "#");
        file(&root, "apps/python/Sidekick/sidekick.py", "#");
        // A folder with no script of its name is a library, not an app.
        file(&root, "apps/python/third_party/readme.txt", "x");
        file(
            &root,
            "apps/lua/Pure Config/manifest.ini",
            "[ABOUT]\nNAME=Pure Config\nAUTHOR=Peter Boese\nVERSION=1.4\nDESCRIPTION=Pure's settings\n",
        );
        file(&root, "apps/lua/not-an-app/readme.txt", "x");
        file(
            &docs,
            "cfg/python.ini",
            "[HELICORSA]\nACTIVE=1\n[sidekick]\nACTIVE=0\n",
        );

        let found = list(&root, Some(&docs));
        let said: Vec<(&str, bool, bool)> = found
            .iter()
            .map(|a| (a.name.as_str(), a.on, a.lua))
            .collect();
        assert_eq!(
            said,
            vec![
                ("Helicorsa", true, false),
                ("Sidekick", false, false),
                ("Pure Config", true, true),
            ]
        );
        assert_eq!(found[2].author, "Peter Boese");

        // Switched under the name the file already has for it.
        set_python(&docs, "Sidekick", true).expect("it is written");
        set_python(&docs, "helicorsa", false).expect("it is written");
        let text = std::fs::read_to_string(python_file(&docs)).expect("it is there");
        assert_eq!(text.matches("[sidekick]").count(), 1);
        assert!(!text.contains("[SIDEKICK]"));
        let again = list(&root, Some(&docs));
        assert!(!again[0].on);
        assert!(again[1].on);
        // An app the file never mentioned gets the game's own capitals.
        file(&root, "apps/python/new_app/new_app.py", "#");
        set_python(&docs, "new_app", true).expect("it is written");
        let text = std::fs::read_to_string(python_file(&docs)).expect("it is there");
        assert!(text.contains("[NEW_APP]"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
