//! Every setting a mod keeps in a file, found and made changeable.
//!
//! **Mods configure themselves in INI files, each in a place of its own.** A
//! weather script under `extension/weather/<name>/`, its controller under
//! `extension/weather-controllers/`, whatever a Lua or Python app keeps
//! beside itself, the per-mod folders under `extension/config-ext/`, the
//! post-processing filters in `system/cfg/ppfilters/`. Pure, Sol, a filter
//! pack, a HUD app: all of them are this, and changing one today means
//! finding its file and opening it in a text editor.
//!
//! **Nothing here knows any mod by name.** The files are found by where mods
//! put them, and each is read the way Custom Shaders Patch's own are
//! ([`csp_config`](super::csp_config)): a key, its value, and whatever the
//! author wrote after it — a label, a range, a description — which is what
//! the setting is called and how it is changed. A file with no comments is
//! still every key by its own name, as a switch, a number or text. So a mod
//! released tomorrow has its settings here tomorrow.
//!
//! **Changed in place, and the first change keeps the original.** These
//! files are the mod's own and there is no second file that overrides them,
//! so the value is written where it is read — after a copy of the untouched
//! file is put beside it once, which [`restore`] puts back.

use super::csp_config::{self, Section};
use std::path::{Path, PathBuf};

/// What kind of thing a mod is, which is also where it was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A weather script: `extension/weather/<name>`.
    Weather,
    /// What drives the weather: `extension/weather-controllers/<name>`.
    Controller,
    /// A mod's own folder under `extension/config-ext`.
    Extension,
    /// A Lua app, under `apps/lua`.
    LuaApp,
    /// A Python app, under `apps/python`.
    PythonApp,
    /// A post-processing filter: one file in `system/cfg/ppfilters`.
    Filter,
}

impl Kind {
    /// Every kind, in the order they are offered.
    pub const ALL: [Kind; 6] = [
        Kind::Weather,
        Kind::Controller,
        Kind::Extension,
        Kind::LuaApp,
        Kind::PythonApp,
        Kind::Filter,
    ];

    /// The folder under the game's the kind lives in.
    pub fn folder(self) -> &'static str {
        match self {
            Kind::Weather => "extension/weather",
            Kind::Controller => "extension/weather-controllers",
            Kind::Extension => "extension/config-ext",
            Kind::LuaApp => "apps/lua",
            Kind::PythonApp => "apps/python",
            Kind::Filter => "system/cfg/ppfilters",
        }
    }

    /// What the kind is called.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Weather => "Weather scripts",
            Kind::Controller => "Weather controllers",
            Kind::Extension => "Mods of the patch",
            Kind::LuaApp => "Lua apps",
            Kind::PythonApp => "Python apps",
            Kind::Filter => "Filters",
        }
    }
}

/// One mod and the files it keeps settings in.
#[derive(Debug, Clone, PartialEq)]
pub struct Mod {
    pub kind: Kind,
    /// Its folder's name — or, for a filter, the file's without `.ini`.
    pub id: String,
    /// What it calls itself in its manifest, else its folder made readable.
    pub name: String,
    /// `[ABOUT] VERSION` of its manifest, when it has one.
    pub version: String,
    /// `[ABOUT] AUTHOR`, when it says.
    pub author: String,
    /// Its settings files, by name.
    pub files: Vec<PathBuf>,
}

/// How far below a mod's folder its settings are looked for.
const DEEP: usize = 3;
/// The most settings files one mod is shown with.
const MOST_FILES: usize = 40;
/// The largest file worth reading as settings.
const LARGEST: u64 = 512 * 1024;

/// Every mod in a game folder that keeps settings in a file.
pub fn find(root: &Path) -> Vec<Mod> {
    let mut found = Vec::new();
    for kind in Kind::ALL {
        let folder = kind
            .folder()
            .split('/')
            .fold(root.to_path_buf(), |at, part| at.join(part));
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            match kind {
                // A filter is one file, and the file is the settings.
                Kind::Filter => {
                    if is_settings(&path) {
                        let id = name.trim_end_matches(".ini").to_string();
                        found.push(Mod {
                            kind,
                            name: id.clone(),
                            id,
                            version: String::new(),
                            author: String::new(),
                            files: vec![path],
                        });
                    }
                }
                _ if path.is_dir() => {
                    let files = settings_under(&path);
                    if files.is_empty() {
                        continue;
                    }
                    let about = about(&path);
                    found.push(Mod {
                        kind,
                        name: Some(about.0)
                            .filter(|n| !n.is_empty())
                            .unwrap_or_else(|| readable(&name)),
                        id: name,
                        version: about.1,
                        author: about.2,
                        files,
                    });
                }
                _ => {}
            }
        }
    }
    found.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    found
}

/// A mod's name, version and author from the manifest in its folder.
fn about(folder: &Path) -> (String, String, String) {
    let text = ["manifest.ini", "MANIFEST.ini", "Manifest.ini"]
        .iter()
        .find_map(|name| std::fs::read_to_string(folder.join(name)).ok())
        .unwrap_or_default();
    let said = |key: &str| super::ini::value(&text, "ABOUT", key).unwrap_or_default();
    (said("NAME"), said("VERSION"), said("AUTHOR"))
}

/// The settings files under a mod's folder, nearest first, then by name.
fn settings_under(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(folder)
        .max_depth(DEEP)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|path| is_settings(path))
        .take(MOST_FILES)
        .collect();
    files.sort_by_key(|path| path.components().count());
    files
}

/// Whether a file is settings: an INI of a size somebody wrote by hand, that
/// is not a manifest and has a key in it.
fn is_settings(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !name.ends_with(".ini") || name == "manifest.ini" || name == "desktop.ini" {
        return false;
    }
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= LARGEST) {
        return false;
    }
    std::fs::read_to_string(path).is_ok_and(|text| !sections_of(&text).is_empty())
}

/// A settings file's text as sections of settings. Nothing is "the driver's
/// own" here: there is one file, and its values are the values.
fn sections_of(text: &str) -> Vec<Section> {
    csp_config::sections(text, "")
}

/// Every setting in a file.
pub fn read(file: &Path) -> Vec<Section> {
    std::fs::read_to_string(file)
        .map(|text| sections_of(&text))
        .unwrap_or_default()
}

/// Where the untouched copy of a settings file is kept.
pub fn original_of(file: &Path) -> PathBuf {
    let mut name = file
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".original");
    file.with_file_name(name)
}

/// Whether a file has been changed here — its original is kept beside it.
pub fn is_changed(file: &Path) -> bool {
    original_of(file).is_file()
}

/// Set one value in a mod's settings file, keeping the untouched file
/// beside it the first time.
pub fn set(file: &Path, section: &str, key: &str, value: &str) -> Result<(), String> {
    let original = original_of(file);
    if !original.exists() {
        std::fs::copy(file, &original)
            .map_err(|why| format!("could not keep {} as it was: {why}", file.display()))?;
    }
    let before =
        std::fs::read_to_string(file).map_err(|why| format!("{}: {why}", file.display()))?;
    let after = match with_value_keeping_comment(&before, section, key, value) {
        Some(after) => after,
        // A key the file does not have yet is added the usual way.
        None => super::ini::with_value(&before, section, key, value),
    };
    std::fs::write(file, after).map_err(|why| format!("{}: {why}", file.display()))
}

/// The text with a key's value replaced **and the comment after it left
/// where it is** — the comment is the setting's label, range and
/// description, and a change that dropped it would leave the next reading
/// with a bare key. `None` when the section has no such key.
fn with_value_keeping_comment(text: &str, section: &str, key: &str, value: &str) -> Option<String> {
    let mut in_section = false;
    let mut done = false;
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let body = line.split(';').next().unwrap_or("").trim();
        if let Some(name) = body.strip_prefix('[').and_then(|b| b.strip_suffix(']')) {
            in_section = name.trim().eq_ignore_ascii_case(section);
        } else if in_section
            && !done
            && let Some((k, _)) = body.split_once('=')
            && k.trim().eq_ignore_ascii_case(key)
        {
            let comment = line.split_once(';').map(|(_, said)| said);
            out.push(match comment {
                Some(said) => format!("{}={value} ;{said}", k.trim()),
                None => format!("{}={value}", k.trim()),
            });
            done = true;
            continue;
        }
        out.push(line.to_string());
    }
    // The file's own line endings: most of these were written on Windows.
    let eol = match text.contains("\r\n") {
        true => "\r\n",
        false => "\n",
    };
    done.then(|| {
        let mut joined = out.join(eol);
        if text.ends_with('\n') {
            joined.push_str(eol);
        }
        joined
    })
}

/// Put a settings file back as it was before anything here changed it.
pub fn restore(file: &Path) -> Result<(), String> {
    let original = original_of(file);
    if !original.is_file() {
        return Err(format!("{} was never changed here", file.display()));
    }
    std::fs::copy(&original, file)
        .map_err(|why| format!("could not put {} back: {why}", file.display()))?;
    std::fs::remove_file(&original).map_err(|why| why.to_string())
}

/// `pure_config` as "Pure config".
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

    fn file(root: &Path, relative: &str, said: &str) -> PathBuf {
        let full = relative
            .split('/')
            .fold(root.to_path_buf(), |at, part| at.join(part));
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("a fixture");
        std::fs::write(&full, said).expect("a fixture");
        full
    }

    fn game(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("acpe-modcfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a fixture");
        root
    }

    /// **A weather script, an app and a filter, none of them known here by
    /// name**: each is found where its kind lives, named by its manifest,
    /// and its settings read with the author's own labels and ranges.
    #[test]
    fn mods_are_found_where_they_live_and_read_as_their_authors_wrote_them() {
        let root = game("find");
        file(
            &root,
            "extension/weather/pure/manifest.ini",
            "[ABOUT]\nNAME=Pure\nAUTHOR=Peter Boese\nVERSION=0.257\n",
        );
        file(
            &root,
            "extension/weather/pure/settings/sun.ini",
            "[SUN]\nBRIGHTNESS=1.2 ; Sun brightness (how strong the sun is), from 0 to 3\n\
             GODRAYS=1 ; God rays\nSTYLE=warm\n",
        );
        // A weather script with no settings is not a mod with settings.
        file(&root, "extension/weather/default/weather.lua", "--");
        file(
            &root,
            "apps/lua/Hud/manifest.ini",
            "[ABOUT]\nNAME=Race HUD\nVERSION=2.1\n",
        );
        file(&root, "apps/lua/Hud/settings.ini", "[MAIN]\nSCALE=1.5\n");
        file(
            &root,
            "system/cfg/ppfilters/Evening.ini",
            "[TONEMAPPING]\nEXPOSURE=0.42\n[DOF]\nENABLED=0\n",
        );
        // Not settings: no keys, and a manifest alone.
        file(&root, "apps/python/empty/notes.ini", "; nothing\n");
        file(&root, "apps/python/bare/manifest.ini", "[ABOUT]\nNAME=x\n");

        let found = find(&root);
        let said: Vec<(Kind, &str, &str)> = found
            .iter()
            .map(|m| (m.kind, m.name.as_str(), m.version.as_str()))
            .collect();
        assert_eq!(
            said,
            vec![
                (Kind::Weather, "Pure", "0.257"),
                (Kind::LuaApp, "Race HUD", "2.1"),
                (Kind::Filter, "Evening", ""),
            ]
        );
        let sun = read(&found[0].files[0]);
        assert_eq!(sun[0].settings[0].label, "Sun brightness");
        assert_eq!(sun[0].settings[0].about, "how strong the sun is");
        assert_eq!(
            sun[0].settings[0].kind,
            csp_config::Kind::Number {
                range: Some((0.0, 3.0)),
                step: None,
                percent: false
            }
        );
        assert_eq!(sun[0].settings[1].kind, csp_config::Kind::Switch);
        // No comment: the key by its own name, and text is text.
        assert_eq!(sun[0].settings[2].label, "Style");
        assert_eq!(sun[0].settings[2].kind, csp_config::Kind::Text);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// **The first change keeps the file as it was, and restore puts it
    /// back** — comments, order and the keys nobody touched included.
    #[test]
    fn a_change_is_written_in_place_and_can_be_undone() {
        let root = game("set");
        let was = "[SUN]\nBRIGHTNESS=1.2 ; Sun brightness\nGODRAYS=1\n";
        let at = file(&root, "extension/weather/pure/sun.ini", was);
        assert!(!is_changed(&at));
        set(&at, "SUN", "BRIGHTNESS", "2").expect("it is written");
        set(&at, "SUN", "GODRAYS", "0").expect("it is written");
        let now = read(&at);
        assert_eq!(now[0].settings[0].value(), "2");
        assert_eq!(now[0].settings[1].value(), "0");
        // The author's comment is still there to be the label.
        assert_eq!(now[0].settings[0].label, "Sun brightness");
        assert!(is_changed(&at));
        // The original is the file before the *first* change, not the second.
        assert_eq!(
            std::fs::read_to_string(original_of(&at)).ok().as_deref(),
            Some(was)
        );
        // The copy kept beside it is not itself offered as settings.
        assert_eq!(find(&root)[0].files, vec![at.clone()]);
        restore(&at).expect("it goes back");
        assert_eq!(std::fs::read_to_string(&at).ok().as_deref(), Some(was));
        assert!(!is_changed(&at));
        assert!(restore(&at).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
