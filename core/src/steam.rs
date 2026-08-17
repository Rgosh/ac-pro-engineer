//! Finding Steam, its libraries, and a game's Proton prefix.
//!
//! None of this knows which game is being looked for. That is the point: a
//! library folder is a library folder, a Proton prefix is named after an
//! appid, and `libraryfolders.vdf` has the same three lines in it whichever
//! game is installed. This used to live inside `games/assetto_corsa/paths.rs`,
//! where it was two hundred lines a second game could not reach — and the
//! second game's own copy would have been the same two hundred lines with a
//! different appid in the middle.
//!
//! What a *particular* game is called on disk, what Steam numbers it and where
//! it keeps its documents are facts about that game and stay in its folder.
//! This module takes them as arguments.

use std::path::{Path, PathBuf};
/// Only the Windows registry lookup logs, and it is the only caller.
#[cfg(target_os = "windows")]
use tracing::debug;

/// Drop repeats while keeping the order, which is the order of likelihood.
///
/// `Vec::dedup` only removes *neighbouring* repeats, and these lists are built
/// from several sources that overlap without being adjacent — the registry and
/// `%ProgramFiles%` name the same directory on an ordinary machine.
pub fn dedup_keeping_order(paths: &mut Vec<PathBuf>) {
    let mut seen = std::collections::HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}

/// Steam installation roots to probe, most likely first.
pub fn roots() -> Vec<PathBuf> {
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
        roots.extend(roots_from_registry());

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
fn roots_from_registry() -> Vec<PathBuf> {
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
pub fn drive_roots() -> Vec<PathBuf> {
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
pub fn libraries() -> Vec<PathBuf> {
    let mut libraries = Vec::new();

    for root in roots() {
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
pub fn parse_library_folders(vdf: &Path) -> Vec<PathBuf> {
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

/// A game's folder under `steamapps/common`, in whichever library holds it.
///
/// `dir_name` is what Steam calls the folder — `assettocorsa`, `Assetto Corsa
/// Competizione` — which is a fact about the game and comes from the game's
/// own module.
pub fn install_dir(dir_name: &str) -> Option<PathBuf> {
    libraries()
        .into_iter()
        .map(|lib| lib.join("steamapps").join("common").join(dir_name))
        .find(|candidate| candidate.exists())
}

/// The `Documents` folder inside a game's Proton prefix, if there is one.
///
/// Under Proton the game is a Windows process and writes to what it believes
/// is `C:\Users\steamuser\Documents`. Resolving that to the host's `~/Documents`
/// finds nothing, which is why local setups were invisible on Linux for as
/// long as they were.
#[cfg(not(target_os = "windows"))]
pub fn proton_documents_dir(app_id: &str) -> Option<PathBuf> {
    // The prefix lives beside the library that holds the game, which is not
    // always the library Steam itself is installed in.
    libraries()
        .into_iter()
        .map(|library| {
            library
                .join("steamapps")
                .join("compatdata")
                .join(app_id)
                .join("pfx")
                .join("drive_c")
                .join("users")
                .join("steamuser")
                .join("Documents")
        })
        .find(|candidate| candidate.exists())
}

/// The host's own Documents folder, for a native install or a Wine-less one.
pub fn host_documents_dir() -> Option<PathBuf> {
    directories_next::UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
        .filter(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A scratch directory of its own per test, so the suite stays parallel.
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
            "steam_vdf",
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
        let path = write_vdf("steam_vdf_win", r#"    "path"      "D:\\SteamLibrary""#);
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
}
