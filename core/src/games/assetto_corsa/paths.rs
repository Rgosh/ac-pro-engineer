//! Finding the Assetto Corsa installation and its Documents folder.
//!
//! Two directories matter and they are not in the same place:
//!
//! * the **install root**, which holds `content/cars` — where car specs and
//!   the reference data come from
//! * the **Documents folder**, which holds `Assetto Corsa/setups` — where the
//!   game reads and writes car setups
//!
//! On Windows these are a Steam library folder and the user's Documents. On
//! Linux the game runs under Proton, so the install root is a normal Steam
//! library path but Documents lives *inside the Proton prefix* — the game is a
//! Windows process and writes to what it believes is `C:\Users\steamuser\
//! Documents`. Resolving that to `~/Documents` finds nothing, which is why
//! local setups were never discovered on Linux.
//!
//! Both can be overridden from the config, which is the escape hatch for a
//! non-Steam install, a second library, or a prefix somewhere unusual.

use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Steam's app id for Assetto Corsa. Names the Proton prefix.
///
/// Only the Proton lookup needs it, and that is Linux-only — on Windows the
/// game writes to the real Documents folder and there is no prefix to find.
#[cfg(not(target_os = "windows"))]
const AC_APP_ID: &str = "244210";

/// Directory name of the game inside a Steam library.
const AC_DIR_NAME: &str = "assettocorsa";

/// Resolve the Assetto Corsa install root.
///
/// `configured` wins if it is set and exists, so a user with an install this
/// cannot find is never stuck.
pub fn ac_install_root(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|p| !p.as_os_str().is_empty()) {
        if path.exists() {
            debug!("Using configured AC install path: {}", path.display());
            return Some(path.to_path_buf());
        }
        info!(
            "Configured AC install path does not exist, falling back to auto-detection: {}",
            path.display()
        );
    }

    let from_steam = steam_libraries()
        .into_iter()
        .map(|lib| lib.join("steamapps").join("common").join(AC_DIR_NAME))
        .find(|candidate| candidate.exists());
    if from_steam.is_some() {
        return from_steam;
    }

    // A copy that is not in a Steam library at all: moved to another disk by
    // hand, or installed from somewhere that is not Steam. Only reached when
    // every library has been looked in, so it costs nothing in the normal case.
    #[cfg(target_os = "windows")]
    {
        for drive in drive_roots() {
            for candidate in [
                drive.join(AC_DIR_NAME),
                drive.join("Games").join(AC_DIR_NAME),
                drive.join("Games").join("Assetto Corsa"),
            ] {
                if candidate.join("content").join("cars").is_dir() {
                    info!(
                        "Found Assetto Corsa outside Steam at {}",
                        candidate.display()
                    );
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Drop repeats while keeping the order, which is the order of likelihood.
///
/// `Vec::dedup` only removes *neighbouring* repeats, and these lists are built
/// from several sources that overlap without being adjacent — the registry and
/// `%ProgramFiles%` name the same directory on an ordinary machine.
fn dedup_keeping_order(paths: &mut Vec<PathBuf>) {
    let mut seen = std::collections::HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}

/// Resolve the Documents folder Assetto Corsa reads setups from.
pub fn ac_documents_dir(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|p| !p.as_os_str().is_empty()) {
        if path.exists() {
            debug!("Using configured AC documents path: {}", path.display());
            return Some(path.to_path_buf());
        }
        info!(
            "Configured AC documents path does not exist, falling back to auto-detection: {}",
            path.display()
        );
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Under Proton the game writes to the prefix, not to the host's
        // ~/Documents. Prefer the prefix and only fall back to the host
        // directory for a native or Wine-less setup.
        if let Some(prefix_docs) = proton_documents_dir() {
            return Some(prefix_docs);
        }
    }

    directories_next::UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
        .filter(|p| p.exists())
}

/// The `Documents` folder inside Assetto Corsa's Proton prefix, if there is
/// one.
#[cfg(not(target_os = "windows"))]
fn proton_documents_dir() -> Option<PathBuf> {
    steam_roots()
        .into_iter()
        .map(|root| {
            root.join("steamapps")
                .join("compatdata")
                .join(AC_APP_ID)
                .join("pfx")
                .join("drive_c")
                .join("users")
                .join("steamuser")
                .join("Documents")
        })
        .find(|candidate| candidate.exists())
}

/// Steam installation roots to probe, most likely first.
fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    #[cfg(not(target_os = "windows"))]
    if let Some(home) = directories_next::UserDirs::new().map(|d| d.home_dir().to_path_buf()) {
        roots.push(home.join(".steam").join("steam"));
        roots.push(home.join(".steam").join("root"));
        roots.push(home.join(".local").join("share").join("Steam"));
        // Flatpak keeps its own home.
        roots.push(
            home.join(".var")
                .join("app")
                .join("com.valvesoftware.Steam")
                .join(".local")
                .join("share")
                .join("Steam"),
        );
        // Snap.
        roots.push(
            home.join("snap")
                .join("steam")
                .join("common")
                .join(".local")
                .join("share")
                .join("Steam"),
        );
    }

    #[cfg(target_os = "windows")]
    {
        // Steam's own record of where it is, which is the only source that is
        // right for every installation rather than for the common one.
        roots.extend(steam_roots_from_registry());

        // Then the default locations, from the environment rather than from a
        // literal `C:` — Windows is not always on C, and this used to be the
        // first thing tried and hardcoded.
        for variable in ["ProgramFiles(x86)", "ProgramFiles", "ProgramW6432"] {
            if let Ok(dir) = std::env::var(variable) {
                roots.push(PathBuf::from(dir).join("Steam"));
            }
        }

        // And finally every drive there is, in the places people put Steam by
        // hand. This replaces a hardcoded `D:`, `E:`, `F:` guess that missed a
        // machine whose second drive is anything else, and it is what makes
        // "Steam is not in Program Files" stop being an unfindable install.
        for drive in drive_roots() {
            roots.push(drive.join("Steam"));
            roots.push(drive.join("SteamLibrary"));
            roots.push(drive.join("Games").join("Steam"));
            roots.push(drive.join("Program Files").join("Steam"));
            roots.push(drive.join("Program Files (x86)").join("Steam"));
        }
    }

    roots.retain(|p| p.exists());
    dedup_keeping_order(&mut roots);
    roots
}

/// Where Steam says it is installed.
///
/// Steam writes its own location to the registry on install, and it is the one
/// answer that does not depend on guessing directory names: `SteamPath` under
/// the current user, `InstallPath` under the machine. The user value uses
/// forward slashes, which `PathBuf` handles on Windows.
#[cfg(target_os = "windows")]
fn steam_roots_from_registry() -> Vec<PathBuf> {
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
    };
    use windows::core::{HSTRING, PCWSTR};

    let sources = [
        (HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Valve\Steam",
            "InstallPath",
        ),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Valve\Steam", "InstallPath"),
    ];

    let mut roots = Vec::new();
    for (hive, subkey, value) in sources {
        let subkey = HSTRING::from(subkey);
        let value = HSTRING::from(value);

        // Ask for the size first: registry strings have no useful upper bound,
        // and a fixed buffer here would silently truncate a long path into one
        // that does not exist.
        let mut bytes = 0u32;
        // SAFETY: both names are null-terminated HSTRINGs that outlive the
        // call, and a null data pointer with a length out-parameter is how
        // RegGetValueW is asked for the size it needs.
        let status = unsafe {
            RegGetValueW(
                hive,
                PCWSTR(subkey.as_ptr()),
                PCWSTR(value.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut bytes),
            )
        };
        if status.is_err() || bytes == 0 {
            continue;
        }

        let mut buffer = vec![0u16; bytes as usize / 2 + 1];
        let mut size = bytes;
        // SAFETY: `buffer` is at least `size` bytes and stays alive for the
        // call; the value was reported as REG_SZ, so it is UTF-16.
        let status = unsafe {
            RegGetValueW(
                hive,
                PCWSTR(subkey.as_ptr()),
                PCWSTR(value.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if status.is_err() {
            continue;
        }

        let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        let path = String::from_utf16_lossy(&buffer[..end]);
        if !path.is_empty() {
            debug!("Steam registered at {path}");
            roots.push(PathBuf::from(path));
        }
    }
    roots
}

/// The drive letters that are actually mounted, as `X:\`.
///
/// From the bitmask rather than by probing A to Z: touching a letter that is a
/// disconnected network drive blocks until it times out, and doing that
/// twenty-six times on startup is a frozen splash screen.
#[cfg(target_os = "windows")]
fn drive_roots() -> Vec<PathBuf> {
    // SAFETY: no arguments, no pointers; returns a bitmask of drive letters.
    let mask = unsafe { windows::Win32::Storage::FileSystem::GetLogicalDrives() };
    (0..26)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| PathBuf::from(format!("{}:\\", (b'A' + bit as u8) as char)))
        .collect()
}

/// Every Steam library folder, including the ones on other drives.
///
/// Steam records extra libraries in `libraryfolders.vdf`. Reading it is what
/// makes a game installed outside the default library findable — the previous
/// implementation hardcoded four Windows drive letters, so a library on any
/// other drive, or any library at all on Linux, was invisible.
fn steam_libraries() -> Vec<PathBuf> {
    let mut libraries = Vec::new();

    for root in steam_roots() {
        libraries.push(root.clone());
        libraries.extend(parse_library_folders(
            &root.join("steamapps").join("libraryfolders.vdf"),
        ));
    }

    // A library on a drive Steam's own metadata does not describe — a folder
    // copied to another disk, a library added while Steam was not running.
    // This used to be a literal `D:`, `E:`, `F:` list, which found nothing on a
    // machine whose second drive is any other letter.
    #[cfg(target_os = "windows")]
    for drive in drive_roots() {
        libraries.push(drive.join("SteamLibrary"));
        libraries.push(drive.join("Steam"));
        libraries.push(drive.join("Games").join("SteamLibrary"));
    }

    dedup_keeping_order(&mut libraries);
    libraries
}

/// Pull the library paths out of a `libraryfolders.vdf`.
///
/// VDF is a small nested key-value format. Rather than take a dependency on a
/// parser for one field, this picks out `"path"   "/some/where"` lines, which
/// is the only key needed and is stable across the format's revisions.
fn parse_library_folders(vdf: &Path) -> Vec<PathBuf> {
    let Ok(content) = std::fs::read_to_string(vdf) else {
        return Vec::new();
    };

    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("\"path\"")?;
            let mut parts = rest.split('"').skip(1);
            let raw = parts.next()?;
            // VDF escapes backslashes, so Windows paths arrive doubled.
            Some(PathBuf::from(raw.replace("\\\\", "\\")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A scratch directory of its own per test, so the suite stays parallel.
    /// Same pattern the updater tests use, rather than a new dependency.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create tmp");
        dir
    }

    fn write_vdf(name: &str, body: &str) -> PathBuf {
        let path = scratch_dir(name).join("libraryfolders.vdf");
        let mut file = std::fs::File::create(&path).expect("create vdf");
        file.write_all(body.as_bytes()).expect("write vdf");
        path
    }

    #[test]
    fn library_folders_are_read_from_the_vdf() {
        let path = write_vdf(
            "acpaths_vdf",
            r#"
"libraryfolders"
{
    "0"
    {
        "path"      "/home/user/.local/share/Steam"
        "label"     ""
    }
    "1"
    {
        "path"      "/mnt/games/SteamLibrary"
        "label"     ""
    }
}
"#,
        );

        assert_eq!(
            parse_library_folders(&path),
            vec![
                PathBuf::from("/home/user/.local/share/Steam"),
                PathBuf::from("/mnt/games/SteamLibrary"),
            ]
        );
    }

    /// VDF escapes backslashes, so a Windows library arrives doubled.
    #[test]
    fn windows_paths_are_unescaped() {
        let path = write_vdf("acpaths_vdf_win", r#"    "path"      "D:\\SteamLibrary""#);
        assert_eq!(
            parse_library_folders(&path),
            vec![PathBuf::from(r"D:\SteamLibrary")]
        );
    }

    /// The candidate lists are built from sources that overlap without being
    /// next to each other — the registry and `%ProgramFiles%` name the same
    /// directory on an ordinary machine, with a dozen drive guesses in
    /// between. `Vec::dedup` only removes neighbours, so it left both, and the
    /// second one meant probing the same missing folder twice.
    #[test]
    fn repeats_go_but_the_order_stays() {
        let mut paths = vec![
            PathBuf::from(r"C:\Program Files (x86)\Steam"),
            PathBuf::from(r"D:\SteamLibrary"),
            PathBuf::from(r"C:\Program Files (x86)\Steam"),
            PathBuf::from(r"E:\Steam"),
            PathBuf::from(r"D:\SteamLibrary"),
        ];
        dedup_keeping_order(&mut paths);

        assert_eq!(
            paths,
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
                PathBuf::from(r"E:\Steam"),
            ],
            "the order is the order of likelihood and has to survive"
        );
    }

    /// Every mounted drive, and nothing that is not mounted. A machine always
    /// has at least one.
    #[cfg(target_os = "windows")]
    #[test]
    fn the_drive_letters_are_the_ones_that_exist() {
        let drives = drive_roots();
        assert!(!drives.is_empty(), "a Windows machine has a drive");
        for drive in &drives {
            assert!(
                drive.exists(),
                "{} came back from the bitmask but is not there",
                drive.display()
            );
        }
    }

    /// Steam not being installed is the normal case on a CI runner, not an
    /// error.
    #[test]
    fn a_missing_vdf_is_not_an_error() {
        assert!(parse_library_folders(Path::new("/nonexistent/libraryfolders.vdf")).is_empty());
    }

    /// The configured path is the escape hatch for an install this module
    /// cannot find, so it has to win over detection.
    #[test]
    fn a_configured_install_path_is_preferred() {
        let configured = scratch_dir("acpaths_configured_root");
        assert_eq!(ac_install_root(Some(&configured)), Some(configured));
    }

    /// ...but only when it exists, or a stale config would silently disable
    /// detection with no way to tell why.
    #[test]
    fn a_configured_path_that_does_not_exist_falls_back_to_detection() {
        let missing = PathBuf::from("/nonexistent/assettocorsa");
        assert_ne!(ac_install_root(Some(&missing)), Some(missing));
    }

    #[test]
    fn an_empty_configured_path_is_ignored() {
        let empty = PathBuf::new();
        assert_ne!(ac_install_root(Some(&empty)), Some(empty));
    }

    #[test]
    fn a_configured_documents_dir_is_preferred() {
        let configured = scratch_dir("acpaths_configured_docs");
        assert_eq!(ac_documents_dir(Some(&configured)), Some(configured));
    }
}
