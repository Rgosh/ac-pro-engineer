#![allow(unsafe_code)]

#[cfg(target_os = "windows")]
use std::mem::size_of;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(target_os = "windows")]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

#[cfg(target_os = "windows")]
pub fn is_process_running(target_name: &str) -> bool {
    unsafe {
        let snapshot: HANDLE = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };

        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&x| x == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);

                if name.eq_ignore_ascii_case(target_name) {
                    let _res = CloseHandle(snapshot);
                    return true;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _res = CloseHandle(snapshot);
        false
    }
}

/// Whether a `/proc` cmdline entry names the process we are looking for.
///
/// Two things make this fiddlier than a substring test.
///
/// The game runs under Proton, so its command line is a *Windows* path with
/// backslashes — `C:\\Program Files\\...\\acs.exe`. Matching only on `/`
/// misses it entirely, which is what broke game detection once already.
///
/// And a Linux build of a Windows-named binary drops the extension: the
/// simulator ships as `simulator.exe` under Wine but `cargo build` produces
/// plain `simulator`, so the launcher waited forever on the platform the
/// bridge exists for.
///
/// The name must still occupy a whole final path component, or `my_acs.exe`
/// would count as a hit.
#[cfg(not(target_os = "windows"))]
fn matches_process_name(cmdline_first: &str, target_lower: &str) -> bool {
    let candidate = cmdline_first.to_lowercase();
    let without_exe = target_lower.strip_suffix(".exe").unwrap_or(target_lower);

    [target_lower, without_exe].iter().any(|name| {
        if candidate == **name {
            return true;
        }
        // A whole trailing component, under either platform's separator.
        candidate
            .strip_suffix(*name)
            .is_some_and(|prefix| prefix.ends_with('/') || prefix.ends_with('\\'))
    })
}

#[cfg(not(target_os = "windows"))]
pub fn is_process_running(target_name: &str) -> bool {
    use std::fs;

    let target_lower = target_name.to_lowercase();
    let is_proc_alive = if let Ok(entries) = fs::read_dir("/proc") {
        entries.flatten().any(|entry| {
            let path = entry.path().join("cmdline");
            if let Ok(cmdline) = fs::read_to_string(path) {
                let parts: Vec<&str> = cmdline.split('\0').collect();
                parts
                    .first()
                    .is_some_and(|first| matches_process_name(first, &target_lower))
            } else {
                false
            }
        })
    } else {
        false
    };

    if !is_proc_alive {
        return false;
    }

    std::path::Path::new("/dev/shm/acpmf_physics").exists() && is_proc_alive
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn get_process_id(process_name: &str) -> Option<u32> {
    unsafe {
        let snapshot: HANDLE = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return None,
        };

        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&x| x == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);

                if name.eq_ignore_ascii_case(process_name) {
                    let _res = CloseHandle(snapshot);
                    return Some(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _res = CloseHandle(snapshot);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_existent_process_returns_false() {
        let running = is_process_running("non_existent_process_xyz_9999");
        assert!(!running);
    }
}

/// A cached answer to "is this process running", refreshed at most once a
/// second.
///
/// The uncached call reads `/proc/*/cmdline` for every process on Linux, and
/// walks the whole toolhelp snapshot on Windows. The launcher asked for two
/// process names twice per frame at a 16 ms update rate — around 124 full
/// process-table scans a second while the user simply sat in the menu, doing
/// nothing.
///
/// A game does not start or stop within a second of mattering, so a one
/// second answer is as good as a fresh one here.
pub struct ProcessWatcher {
    names: Vec<String>,
    last_checked: Option<std::time::Instant>,
    last_answer: bool,
}

/// How long a cached answer stays good.
const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(1);

impl ProcessWatcher {
    /// Watch for any of `names` being present.
    pub fn new(names: &[&str]) -> Self {
        Self {
            names: names.iter().map(|n| n.to_string()).collect(),
            last_checked: None,
            last_answer: false,
        }
    }

    /// Whether any watched process is running, rescanning if the cached
    /// answer has expired.
    pub fn is_running(&mut self) -> bool {
        let expired = self.last_checked.is_none_or(|at| at.elapsed() >= CACHE_TTL);

        if expired {
            self.last_answer = self.names.iter().any(|name| is_process_running(name));
            self.last_checked = Some(std::time::Instant::now());
        }

        self.last_answer
    }

    /// The last answer without rescanning, for callers that only want to
    /// display what is already known.
    pub fn cached(&self) -> bool {
        self.last_answer
    }
}

#[cfg(test)]
mod watcher_tests {
    use super::*;

    #[test]
    fn a_watcher_with_no_names_never_reports_running() {
        let mut watcher = ProcessWatcher::new(&[]);
        assert!(!watcher.is_running());
    }

    /// The first call has to scan; there is nothing cached to return.
    #[test]
    fn the_first_call_scans() {
        let mut watcher = ProcessWatcher::new(&["definitely_not_a_real_process_xyz"]);
        assert!(!watcher.cached(), "nothing known before the first call");
        watcher.is_running();
        assert!(
            watcher.last_checked.is_some(),
            "the first call must have scanned"
        );
    }

    /// A second call inside the TTL must not scan again — that is the whole
    /// point of the type.
    #[test]
    fn a_second_call_within_the_ttl_reuses_the_answer() {
        let mut watcher = ProcessWatcher::new(&["definitely_not_a_real_process_xyz"]);
        watcher.is_running();
        let first_checked = watcher.last_checked.expect("scanned once");

        watcher.is_running();
        assert_eq!(
            watcher.last_checked.expect("still scanned once"),
            first_checked,
            "the cached answer should have been reused"
        );
    }

    /// The case that broke game detection: Proton hands us a Windows path,
    /// and matching only on `/` never sees it.
    ///
    /// `matches_process_name` parses /proc cmdlines, which only the non-Windows
    /// implementation has.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_wine_style_windows_path_is_matched() {
        assert!(matches_process_name(
            r"c:\program files (x86)\steam\steamapps\common\assettocorsa\acs.exe",
            "acs.exe"
        ));
        assert!(matches_process_name(r"z:\home\user\ac\acs.exe", "acs.exe"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_unix_path_is_matched() {
        assert!(matches_process_name("/usr/bin/acs.exe", "acs.exe"));
        assert!(matches_process_name("acs.exe", "acs.exe"));
    }

    /// A Linux build drops the .exe the callers ask for.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn the_extensionless_linux_build_is_matched() {
        assert!(matches_process_name(
            "/home/user/projects/target/release/simulator",
            "simulator.exe"
        ));
        assert!(matches_process_name("simulator", "simulator.exe"));
    }

    /// ...but the name has to be a whole component, or an unrelated binary
    /// that merely ends with the same letters counts as the game running.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_partial_component_is_not_matched() {
        assert!(!matches_process_name("/usr/bin/my_acs.exe", "acs.exe"));
        assert!(!matches_process_name("/opt/notsimulator", "simulator.exe"));
        assert!(!matches_process_name("/usr/bin/steam", "acs.exe"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn matching_ignores_case() {
        assert!(matches_process_name(r"C:\Games\AC\ACS.EXE", "acs.exe"));
    }
}
