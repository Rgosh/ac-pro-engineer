//! Custom Shaders Patch's own switches, read and set without the game.
//!
//! **Two files per module, and the second one wins.** The patch ships its
//! defaults under `extension/config/<module>.ini` in the game folder, and
//! writes what a driver changed to `cfg/extension/<module>.ini` in the game's
//! Documents folder — which is also where Content Manager puts it. Reading
//! only the first would show the patch's defaults to somebody who turned
//! half of it off; writing to the first would be overwritten by the next
//! patch update.
//!
//! **Only `[BASIC] ENABLED`.** Every module of the patch is switched on and
//! off by that one key, and that is the part a launcher can say honestly.
//! The hundreds of keys under it are the patch's to explain, in its own
//! settings app, and a second copy of their meaning here would be wrong the
//! first time the patch changed one.
//!
//! **"Not stated" is an answer.** A module neither file mentions is reported
//! as `None`, not as off: the patch decides then, and it may well be on.

pub use super::ini::with_value;
use std::path::{Path, PathBuf};

/// One of the patch's modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Module {
    /// Its file, under `extension/config` and under `cfg/extension`.
    pub file: &'static str,
    /// What the patch calls it.
    pub name: &'static str,
    /// What it does, in a line a driver can read.
    pub about: &'static str,
}

/// The modules worth a switch on a launcher, in the order people look for
/// them.
pub const MODULES: &[Module] = &[
    Module {
        file: "weather_fx.ini",
        name: "WeatherFX",
        about: "Dynamic weather, the sky and the time of day moving on their own.",
    },
    Module {
        file: "rain_fx.ini",
        name: "RainFX",
        about: "Rain: wet tarmac, puddles, drops on the windscreen.",
    },
    Module {
        file: "lighting_fx.ini",
        name: "Lighting FX",
        about: "Headlights, track lights and lit cockpits at night.",
    },
    Module {
        file: "particles_fx.ini",
        name: "Particles FX",
        about: "Sparks, smoke and dust thrown up off the track.",
    },
    Module {
        file: "smart_mirror.ini",
        name: "Smart mirror",
        about: "A real-time rear-view mirror instead of the game's flat one.",
    },
    Module {
        file: "tyres_fx.ini",
        name: "Tyres FX",
        about: "Tyres that show wear, dirt and grass.",
    },
    Module {
        file: "graphics_adjustments.ini",
        name: "Graphics adjustments",
        about: "Colour, sharpness and the look of the whole picture.",
    },
    Module {
        file: "chaser_camera.ini",
        name: "Chase camera",
        about: "A chase camera that moves like one, not on a rail.",
    },
    Module {
        file: "vr_tweaks.ini",
        name: "VR tweaks",
        about: "Fixes and extras for driving in a headset.",
    },
];

/// The section and the key that switch a module.
const SECTION: &str = "BASIC";
const KEY: &str = "ENABLED";

/// Where the patch keeps its defaults for a module.
pub fn default_file(root: &Path, module: &Module) -> PathBuf {
    root.join("extension").join("config").join(module.file)
}

/// Where a driver's own choice for a module is written.
///
/// `documents` is the game's own folder in Documents —
/// [`game_documents_dir`](super::paths::game_documents_dir).
pub fn user_file(documents: &Path, module: &Module) -> PathBuf {
    documents.join("cfg").join("extension").join(module.file)
}

/// Whether a module is on: the driver's file first, the patch's defaults
/// after, and `None` where neither says.
pub fn enabled(root: &Path, documents: Option<&Path>, module: &Module) -> Option<bool> {
    let mine = documents
        .and_then(|docs| std::fs::read_to_string(user_file(docs, module)).ok())
        .and_then(|text| stated(&text));
    mine.or_else(|| {
        std::fs::read_to_string(default_file(root, module))
            .ok()
            .and_then(|text| stated(&text))
    })
}

/// Switch a module on or off, in the driver's own file.
///
/// **Only the one key changes.** Everything else in the file — the driver's
/// other choices, their comments, the order — is written back as it was.
pub fn set(documents: &Path, module: &Module, on: bool) -> Result<PathBuf, String> {
    let at = user_file(documents, module);
    let before = std::fs::read_to_string(&at).unwrap_or_default();
    let after = with_value(&before, SECTION, KEY, if on { "1" } else { "0" });
    if let Some(folder) = at.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    std::fs::write(&at, after).map_err(|e| format!("{}: {e}", at.display()))?;
    Ok(at)
}

/// What `[BASIC] ENABLED` says in an ini's text, if it says.
pub fn stated(text: &str) -> Option<bool> {
    match super::ini::value(text, SECTION, KEY)?
        .to_ascii_lowercase()
        .as_str()
    {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Whether a car or a circuit ships its own configuration for the patch —
/// lights, instruments, rain, the things a mod is often made around.
///
/// `folder` is the car's or the circuit's own folder under `content`.
pub fn ships_extras(folder: &Path) -> bool {
    folder.join("extension").join("ext_config.ini").is_file() || folder.join("extension").is_dir()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("acpe-csp-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_switch_is_read_from_its_section_only() {
        assert_eq!(stated("[BASIC]\nENABLED=1\n"), Some(true));
        assert_eq!(stated("[BASIC]\nENABLED = 0 ; off\n"), Some(false));
        assert_eq!(stated("[OTHER]\nENABLED=1\n"), None);
        assert_eq!(stated("[BASIC]\n;ENABLED=1\n"), None);
        assert_eq!(stated(""), None);
        assert_eq!(stated("[BASIC]\nENABLED=1\nENABLED=0\n"), Some(false));
    }

    #[test]
    fn setting_it_changes_that_key_and_nothing_else() {
        let before = "; mine\n[BASIC]\nENABLED=1\nOTHER=7\n[MORE]\nENABLED=1\n";
        let after = with_value(before, "BASIC", "ENABLED", "0");
        assert_eq!(
            after,
            "; mine\n[BASIC]\nENABLED=0\nOTHER=7\n[MORE]\nENABLED=1\n"
        );
        assert_eq!(stated(&after), Some(false));
    }

    #[test]
    fn a_missing_key_goes_into_its_section_and_a_missing_section_is_made() {
        let after = with_value("[BASIC]\nOTHER=7\n[MORE]\nX=1\n", "BASIC", "ENABLED", "1");
        assert_eq!(after, "[BASIC]\nOTHER=7\nENABLED=1\n[MORE]\nX=1\n");
        assert_eq!(
            with_value("[MORE]\nX=1", "BASIC", "ENABLED", "1"),
            "[MORE]\nX=1\n\n[BASIC]\nENABLED=1\n"
        );
        assert_eq!(
            with_value("", "BASIC", "ENABLED", "1"),
            "[BASIC]\nENABLED=1\n"
        );
    }

    #[test]
    fn the_drivers_file_wins_over_the_defaults_and_neither_is_not_off() {
        let root = scratch("root");
        let docs = scratch("docs");
        let rain = MODULES.iter().find(|m| m.file == "rain_fx.ini").unwrap();
        assert_eq!(enabled(root.as_path(), Some(docs.as_path()), rain), None);

        let default = default_file(root.as_path(), rain);
        std::fs::create_dir_all(default.parent().unwrap()).unwrap();
        std::fs::write(&default, "[BASIC]\nENABLED=1\n").unwrap();
        assert_eq!(
            enabled(root.as_path(), Some(docs.as_path()), rain),
            Some(true)
        );

        let written = set(docs.as_path(), rain, false).unwrap();
        assert_eq!(written, user_file(docs.as_path(), rain));
        assert_eq!(
            enabled(root.as_path(), Some(docs.as_path()), rain),
            Some(false)
        );
        // The defaults are the patch's, and an update replaces them.
        assert_eq!(
            std::fs::read_to_string(&default).unwrap(),
            "[BASIC]\nENABLED=1\n"
        );
    }

    #[test]
    fn a_car_with_its_own_patch_folder_ships_extras() {
        let car = scratch("car");
        assert!(!ships_extras(car.as_path()));
        std::fs::create_dir_all(car.as_path().join("extension")).unwrap();
        assert!(ships_extras(car.as_path()));
    }
}
