//! Custom Shaders Patch presets: a whole look in one file, in the format
//! Content Manager reads and writes.
//!
//! **A preset is every module's settings in one INI**, each section named
//! `MODULE:SECTION` — `WEATHER_FX:BASIC`, `LIGHTING_FX:SUN` — the module
//! being its file's name in capitals. A `[CONFIGURATION]` section and
//! sections marked with ℹ are the file's own notes, not settings. Years of
//! presets are shared in this format on forums and Discords, and Content
//! Manager keeps the ones a driver made in its own presets folder; reading
//! that folder is what makes them all usable here, and writing the same
//! format is what makes a preset made here usable there.
//!
//! **Applying one changes only the driver's own files** in
//! `cfg/extension/` of the game's Documents — the patch's defaults are never
//! touched — and only the keys the preset names, so a preset from an older
//! patch leaves a newer setting as it was.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A preset, read: each module's file and the values it sets, in order.
pub type Preset = BTreeMap<String, Vec<(String, String, String)>>;

/// Read a preset's text. Module files come back in lower case with `.ini`.
pub fn parse(text: &str) -> Preset {
    let mut out: Preset = BTreeMap::new();
    let mut at: Option<(String, String)> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let name = name.trim();
            at = match name.split_once(':') {
                Some((module, section))
                    if !name.contains('\u{2139}')
                        && !module.trim().is_empty()
                        && !section.trim().is_empty()
                        && module
                            .trim()
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') =>
                {
                    Some((
                        format!("{}.ini", module.trim().to_ascii_lowercase()),
                        section.trim().to_string(),
                    ))
                }
                // CONFIGURATION, notes, and anything not a module's section.
                _ => None,
            };
            continue;
        }
        let Some((file, section)) = &at else { continue };
        let body = line.split(';').next().unwrap_or("").trim();
        let Some((key, value)) = body.split_once('=') else {
            continue;
        };
        out.entry(file.clone()).or_default().push((
            section.clone(),
            key.trim().to_string(),
            value.trim().to_string(),
        ));
    }
    out
}

/// Write a preset's text from module files' settings.
pub fn text(preset: &Preset) -> String {
    let affected: Vec<String> = preset
        .keys()
        .map(|file| file.trim_end_matches(".ini").to_ascii_uppercase())
        .collect();
    let mut out = format!(
        "[CONFIGURATION]\nAFFECTED_SECTIONS={}\n",
        affected.join(",")
    );
    for (file, values) in preset {
        let module = file.trim_end_matches(".ini").to_ascii_uppercase();
        let mut section: Option<&str> = None;
        for (in_section, key, value) in values {
            if section != Some(in_section.as_str()) {
                out.push_str(&format!("\n[{module}:{in_section}]\n"));
                section = Some(in_section);
            }
            out.push_str(&format!("{key}={value}\n"));
        }
    }
    out
}

/// Every value the driver has set for the patch: their own files in
/// `cfg/extension/`, read as a preset — what "save this look" keeps.
pub fn current(game_documents: &Path) -> Preset {
    let folder = game_documents.join("cfg").join("extension");
    let mut out: Preset = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return out;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case("ini"))
        })
        .collect();
    files.sort();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let mut section = String::new();
        for line in text.lines() {
            let line = line.split(';').next().unwrap_or("").trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_string();
            } else if let Some((key, value)) = line.split_once('=')
                && !section.is_empty()
            {
                out.entry(file.clone()).or_default().push((
                    section.clone(),
                    key.trim().to_string(),
                    value.trim().to_string(),
                ));
            }
        }
    }
    out
}

/// Apply a preset to the driver's own files. Returns how many values were
/// written.
pub fn apply(game_documents: &Path, preset: &Preset) -> Result<usize, String> {
    let mut written = 0;
    for (file, values) in preset {
        // A file name is only ever a plain name: the preset may have come
        // from anywhere.
        if file.contains(['/', '\\']) || file.contains("..") {
            continue;
        }
        let at = super::csp_config::user_file(game_documents, file);
        let mut body = std::fs::read_to_string(&at).unwrap_or_default();
        for (section, key, value) in values {
            body = super::ini::with_value(&body, section, key, value);
            written += 1;
        }
        if let Some(folder) = at.parent() {
            std::fs::create_dir_all(folder)
                .map_err(|why| format!("{}: {why}", folder.display()))?;
        }
        std::fs::write(&at, body).map_err(|why| format!("{}: {why}", at.display()))?;
    }
    Ok(written)
}

/// One preset on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnDisk {
    pub name: String,
    pub path: PathBuf,
    /// Content Manager's, rather than one made here.
    pub from_content_manager: bool,
}

/// Content Manager's presets folder, found beside the game's Documents —
/// the user's own `AppData/Local` on Windows, and the same folder inside the
/// game's Proton prefix on Linux, where Content Manager runs.
pub fn content_managers(game_documents: &Path) -> Option<PathBuf> {
    // `…/Documents/Assetto Corsa` → `…/AppData/Local/…`
    let home = game_documents.parent()?.parent()?;
    let at = home
        .join("AppData")
        .join("Local")
        .join("AcTools Content Manager")
        .join("Presets")
        .join("Custom Shaders Patch Presets");
    at.is_dir().then_some(at)
}

/// The presets in Content Manager's folder and in this program's own.
pub fn list(game_documents: &Path, own: &Path) -> Vec<OnDisk> {
    let mut found = Vec::new();
    let folders = content_managers(game_documents)
        .map(|at| (at, true))
        .into_iter()
        .chain(std::iter::once((own.to_path_buf(), false)));
    for (folder, from_content_manager) in folders {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name
                .strip_suffix(".ini")
                .or_else(|| name.strip_suffix(".INI"))
                && path.is_file()
            {
                found.push(OnDisk {
                    name: stem.to_string(),
                    path,
                    from_content_manager,
                });
            }
        }
    }
    found.sort_by_key(|p| (p.from_content_manager, p.name.to_lowercase()));
    found
}

/// Save what is set now as a preset of this program's own.
pub fn save(game_documents: &Path, own: &Path, name: &str) -> Result<PathBuf, String> {
    let name: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c => c,
        })
        .collect();
    if name.is_empty() {
        return Err("a preset needs a name".to_string());
    }
    let preset = current(game_documents);
    if preset.is_empty() {
        return Err("the patch has no settings of yours to keep yet".to_string());
    }
    std::fs::create_dir_all(own).map_err(|why| format!("{}: {why}", own.display()))?;
    let at = own.join(format!("{name}.ini"));
    std::fs::write(&at, text(&preset)).map_err(|why| format!("{}: {why}", at.display()))?;
    Ok(at)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    use super::*;

    const CM: &str = "[CONFIGURATION]\nAFFECTED_SECTIONS=WEATHER_FX,LIGHTING_FX\n\n\
        [WEATHER_FX:BASIC]\nENABLED=1\nIMPLEMENTATION=pure ; the style\n\n\
        [GENERAL:\u{2139}]\nDESCRIPTION=a note, not a setting\n\n\
        [LIGHTING_FX:SUN]\nINTENSITY=1.2\n\n[../../ESCAPE:X]\nA=1\n";

    /// **A Content Manager preset reads as module files and values** — its
    /// notes and anything not a module's section left out.
    #[test]
    fn a_content_manager_preset_reads_as_the_files_it_sets() {
        let preset = parse(CM);
        assert_eq!(
            preset.keys().collect::<Vec<_>>(),
            vec!["lighting_fx.ini", "weather_fx.ini"]
        );
        assert_eq!(
            preset["weather_fx.ini"],
            vec![
                ("BASIC".into(), "ENABLED".into(), "1".into()),
                ("BASIC".into(), "IMPLEMENTATION".into(), "pure".into())
            ]
        );
        // Written and read again, it is the same preset.
        assert_eq!(parse(&text(&preset)), preset);
        assert!(
            text(&preset)
                .starts_with("[CONFIGURATION]\nAFFECTED_SECTIONS=LIGHTING_FX,WEATHER_FX\n")
        );
    }

    /// **Applied to the driver's files only, saved from them, and found in
    /// Content Manager's folder** in a Proton prefix's layout.
    #[test]
    fn a_preset_is_applied_saved_and_found() {
        let dir = std::env::temp_dir().join(format!("acpe-cspp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let docs = dir.join("users/steamuser/Documents/Assetto Corsa");
        let own = dir.join("own");
        std::fs::create_dir_all(docs.join("cfg/extension")).expect("a fixture");
        std::fs::write(
            docs.join("cfg/extension/weather_fx.ini"),
            "[BASIC]\nENABLED=0\nKEEP=me\n",
        )
        .expect("a fixture");
        assert_eq!(apply(&docs, &parse(CM)).ok(), Some(3));
        let weather =
            std::fs::read_to_string(docs.join("cfg/extension/weather_fx.ini")).unwrap_or_default();
        assert!(weather.contains("ENABLED=1"));
        assert!(weather.contains("IMPLEMENTATION=pure"));
        assert!(
            weather.contains("KEEP=me"),
            "a key the preset does not name stays"
        );
        assert!(docs.join("cfg/extension/lighting_fx.ini").is_file());

        let saved = save(&docs, &own, "Evening: gold").expect("it is saved");
        assert!(saved.ends_with("Evening- gold.ini"));
        let back = parse(&std::fs::read_to_string(&saved).unwrap_or_default());
        assert!(back["weather_fx.ini"].contains(&("BASIC".into(), "KEEP".into(), "me".into())));

        let cm = dir.join(
            "users/steamuser/AppData/Local/AcTools Content Manager/Presets/Custom Shaders Patch Presets",
        );
        std::fs::create_dir_all(&cm).expect("a fixture");
        std::fs::write(cm.join("Pure Night.ini"), CM).expect("a fixture");
        let found = list(&docs, &own);
        let names: Vec<(&str, bool)> = found
            .iter()
            .map(|p| (p.name.as_str(), p.from_content_manager))
            .collect();
        assert_eq!(names, vec![("Evening- gold", false), ("Pure Night", true)]);
        assert!(save(&docs, &own, "  ").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
