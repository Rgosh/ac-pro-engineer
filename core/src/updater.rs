use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};

const GITHUB_OWNER: &str = "Rgosh";
const GITHUB_REPO: &str = "ac-pro-engineer";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpdateAvailable,
    NoUpdate,
    Downloading(f32),
    Downloaded(String),
    Error(String),
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct RemoteVersion {
    pub version: String,
    pub url: String,
    pub notes: String,
    pub is_latest: bool,
    #[serde(default)]
    pub expected_size: u64,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// Returns the expected asset suffix for the current platform.
fn platform_asset_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        ".exe"
    } else if cfg!(target_os = "linux") {
        "-linux"
    } else if cfg!(target_os = "macos") {
        "-macos"
    } else {
        ""
    }
}

/// Compare two SemVer version strings.
/// Returns Ordering::Greater if `a` is newer than `b`.
fn compare_semver(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> (u32, u32, u32) {
        let s = s.trim_start_matches('v');
        // Strip prerelease suffix (e.g. "-beta.1")
        let s = s.split('-').next().unwrap_or(s);
        let mut parts = s.split('.');
        let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };
    parse(a).cmp(&parse(b))
}

/// Check if a version string is a prerelease (contains `-`).
fn is_prerelease(version: &str) -> bool {
    let v = version.trim_start_matches('v');
    v.contains('-')
}

#[derive(Clone)]
pub struct Updater {
    pub status: Arc<Mutex<UpdateStatus>>,
    pub releases: Arc<Mutex<Vec<RemoteVersion>>>,
    pub selected_index: Arc<Mutex<usize>>,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

impl Updater {
    pub fn new() -> Self {
        let updater = Self {
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
            releases: Arc::new(Mutex::new(Vec::new())),
            selected_index: Arc::new(Mutex::new(0)),
        };

        updater.check_for_updates();
        updater
    }

    fn safe_lock<T, F>(&self, mutex: &Mutex<T>, f: F)
    where
        F: FnOnce(&mut T),
    {
        match mutex.lock() {
            Ok(mut guard) => f(&mut *guard),
            Err(poisoned) => f(&mut *poisoned.into_inner()),
        }
    }

    pub fn next_version(&self) {
        let list_guard = self.releases.lock().unwrap_or_else(|e| e.into_inner());
        if list_guard.is_empty() {
            return;
        }

        let max_idx = list_guard.len() - 1;
        drop(list_guard);

        self.safe_lock(&self.selected_index, |idx| {
            if *idx < max_idx {
                *idx += 1;
            }
        });
    }

    pub fn prev_version(&self) {
        let list_guard = self.releases.lock().unwrap_or_else(|e| e.into_inner());
        if list_guard.is_empty() {
            return;
        }
        drop(list_guard);

        self.safe_lock(&self.selected_index, |idx| {
            if *idx > 0 {
                *idx -= 1;
            }
        });
    }

    pub fn get_selected_release(&self) -> Option<RemoteVersion> {
        let list = self.releases.lock().unwrap_or_else(|e| e.into_inner());
        let idx = *self
            .selected_index
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        list.get(idx).cloned()
    }

    pub fn check_for_updates(&self) {
        let status = self.status.clone();
        let releases_store = self.releases.clone();

        {
            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
            *lock = UpdateStatus::Checking;
        }

        thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .user_agent("AC-Pro-Engineer-Updater")
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default();

            let url = format!(
                "https://api.github.com/repos/{}/{}/releases",
                GITHUB_OWNER, GITHUB_REPO
            );

            info!("Checking for updates at: {}", url);

            match client.get(&url).send() {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        error!("GitHub API returned non-success status: {}", resp.status());
                        let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                        *lock = UpdateStatus::Error(format!("API error: {}", resp.status()));
                        return;
                    }

                    match resp.json::<Vec<GitHubRelease>>() {
                        Ok(gh_releases) => {
                            let suffix = platform_asset_suffix();
                            let mut parsed_versions = Vec::new();

                            for (i, release) in gh_releases.iter().enumerate() {
                                // Skip prereleases by default
                                if release.prerelease {
                                    continue;
                                }

                                let remote_ver_str = release.tag_name.trim_start_matches('v');

                                // Skip prerelease version strings (e.g. "0.3.0-beta.1")
                                if is_prerelease(remote_ver_str) {
                                    continue;
                                }

                                // Skip versions older than current (no downgrade)
                                if compare_semver(remote_ver_str, CURRENT_VERSION)
                                    == std::cmp::Ordering::Less
                                {
                                    continue;
                                }

                                // Find platform-appropriate asset
                                let asset = if suffix.is_empty() {
                                    // Unknown platform — take the first asset
                                    release.assets.first()
                                } else {
                                    release.assets.iter().find(|a| a.name.contains(suffix))
                                };

                                if let Some(asset) = asset {
                                    parsed_versions.push(RemoteVersion {
                                        version: remote_ver_str.to_string(),
                                        url: asset.browser_download_url.clone(),
                                        notes: release.body.clone().unwrap_or_default(),
                                        is_latest: i == 0 || parsed_versions.is_empty(),
                                        expected_size: asset.size,
                                    });
                                }
                            }

                            if !parsed_versions.is_empty() {
                                {
                                    let mut r_lock =
                                        releases_store.lock().unwrap_or_else(|e| e.into_inner());
                                    *r_lock = parsed_versions.clone();
                                }

                                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());

                                if compare_semver(&parsed_versions[0].version, CURRENT_VERSION)
                                    == std::cmp::Ordering::Greater
                                {
                                    info!("Update available: v{}", parsed_versions[0].version);
                                    *lock = UpdateStatus::UpdateAvailable;
                                } else {
                                    info!("App is up to date.");
                                    *lock = UpdateStatus::NoUpdate;
                                }
                            } else {
                                info!("No compatible updates found for this platform.");
                                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                                *lock = UpdateStatus::NoUpdate;
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse GitHub JSON response: {}", e);
                            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                            *lock = UpdateStatus::Error("Parse error".to_string());
                        }
                    }
                }
                Err(e) => {
                    error!("Network error while fetching updates: {}", e);
                    let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                    *lock = UpdateStatus::Error("Net Error (Check logs)".to_string());
                }
            }
        });
    }

    pub fn download_selected(&self) {
        if let Some(info) = self.get_selected_release() {
            self.download_update(info);
        }
    }

    fn download_update(&self, info: RemoteVersion) {
        let status = self.status.clone();
        thread::spawn(move || {
            {
                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                *lock = UpdateStatus::Downloading(0.0);
            }

            let current_exe =
                env::current_exe().unwrap_or_else(|_| PathBuf::from("ac_pro_engineer"));
            let exe_dir = current_exe
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));

            // Use .tmp extension during download, rename atomically after verification
            let final_name = if cfg!(target_os = "windows") {
                "ac_pro_engineer_new.exe"
            } else {
                "ac_pro_engineer_new"
            };
            let temp_path = exe_dir.join(format!("{}.tmp", final_name));
            let final_path = exe_dir.join(final_name);
            let final_path_str = final_path.to_str().unwrap_or(final_name).to_string();

            let client = reqwest::blocking::Client::builder()
                .user_agent("AC-Pro-Engineer-Updater")
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default();

            info!("Starting download from: {}", info.url);

            match client.get(&info.url).send() {
                Ok(mut resp) => {
                    if !resp.status().is_success() {
                        error!("Download failed with status: {}", resp.status());
                        let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                        *lock = UpdateStatus::Error("Download failed".to_string());
                        return;
                    }
                    let total_size = resp.content_length().unwrap_or(0);

                    match File::create(&temp_path) {
                        Ok(mut file) => {
                            let mut buffer = [0; 8192];
                            let mut downloaded: u64 = 0;
                            loop {
                                match resp.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if file.write_all(&buffer[..n]).is_err() {
                                            error!("Failed to write bytes to disk.");
                                            let _ = std::fs::remove_file(&temp_path);
                                            let mut lock =
                                                status.lock().unwrap_or_else(|e| e.into_inner());
                                            *lock = UpdateStatus::Error("Write error".to_string());
                                            return;
                                        }
                                        downloaded += n as u64;
                                        if total_size > 0 {
                                            let pct =
                                                (downloaded as f32 / total_size as f32) * 100.0;
                                            let mut lock =
                                                status.lock().unwrap_or_else(|e| e.into_inner());
                                            *lock = UpdateStatus::Downloading(pct);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error reading download stream: {}", e);
                                        let _ = std::fs::remove_file(&temp_path);
                                        let mut lock =
                                            status.lock().unwrap_or_else(|e| e.into_inner());
                                        *lock =
                                            UpdateStatus::Error("Download interrupted".to_string());
                                        return;
                                    }
                                }
                            }

                            // Verify download size
                            if info.expected_size > 0 && downloaded != info.expected_size {
                                error!(
                                    "Size mismatch: expected {} bytes, got {} bytes",
                                    info.expected_size, downloaded
                                );
                                let _ = std::fs::remove_file(&temp_path);
                                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                                *lock = UpdateStatus::Error(format!(
                                    "Incomplete download ({}/{})",
                                    downloaded, info.expected_size
                                ));
                                return;
                            }

                            if downloaded == 0 {
                                error!("Downloaded 0 bytes — aborting");
                                let _ = std::fs::remove_file(&temp_path);
                                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                                *lock = UpdateStatus::Error("Empty download".to_string());
                                return;
                            }

                            // Atomic rename from .tmp to final
                            if let Err(e) = std::fs::rename(&temp_path, &final_path) {
                                error!("Failed to rename temp file: {}", e);
                                let _ = std::fs::remove_file(&temp_path);
                                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                                *lock = UpdateStatus::Error("Rename failed".to_string());
                                return;
                            }

                            // Set executable permission on Linux/macOS
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(metadata) = std::fs::metadata(&final_path) {
                                    let mut perms = metadata.permissions();
                                    perms.set_mode(0o755);
                                    let _ = std::fs::set_permissions(&final_path, perms);
                                }
                            }

                            info!("Download completed and verified: {} bytes", downloaded);
                            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                            *lock = UpdateStatus::Downloaded(final_path_str);
                        }
                        Err(e) => {
                            error!("Could not create temp file for update: {}", e);
                            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                            *lock = UpdateStatus::Error("File access error".to_string());
                        }
                    }
                }
                Err(e) => {
                    error!("Connection lost during download: {}", e);
                    let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                    *lock = UpdateStatus::Error("Net Error (Check logs)".to_string());
                }
            }
        });
    }

    pub fn restart_and_apply(
        &self,
        _new_file_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("ac_pro_engineer"));
        let exe_dir = current_exe
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            let exe_path = current_exe.to_str().unwrap_or("ac_pro_engineer.exe");
            let new_exe = exe_dir.join("ac_pro_engineer_new.exe");
            let new_exe_str = new_exe.to_str().unwrap_or("ac_pro_engineer_new.exe");
            let bat_path = exe_dir.join("updater.bat");

            let script = format!(
                "@echo off\r\n\
                 chcp 65001 >nul\r\n\
                 :wait_close\r\n\
                 timeout /t 1 /nobreak > NUL\r\n\
                 del \"{0}.bak\" >nul 2>&1\r\n\
                 if exist \"{0}.bak\" goto wait_close\r\n\
                 move /y \"{0}\" \"{0}.bak\" >nul\r\n\
                 move /y \"{1}\" \"{0}\" >nul\r\n\
                 start \"\" \"{0}\"\r\n\
                 (goto) 2>nul & del \"%~f0\"\r\n\
                 exit",
                exe_path, new_exe_str
            );

            let mut file = File::create(&bat_path)?;
            file.write_all(script.as_bytes())?;
            drop(file);

            Command::new("cmd")
                .args(["/C", "start", "/MIN", "updater.bat"])
                .current_dir(exe_dir)
                .spawn()?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            use std::process::Command;
            let new_exe = exe_dir.join("ac_pro_engineer_new");
            let backup = exe_dir.join("ac_pro_engineer.bak");

            if !new_exe.exists() {
                return Err("Update binary not found".into());
            }

            // Rename current → backup, new → current
            if current_exe.exists() {
                std::fs::rename(&current_exe, &backup)?;
            }
            if new_exe.exists() {
                std::fs::rename(&new_exe, &current_exe)?;
            }

            // Launch the new binary
            Command::new(&current_exe).spawn()?;

            warn!("Update applied via rename; restarting.");
        }

        std::process::exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_newer_version_is_greater() {
        assert_eq!(
            compare_semver("0.3.0", "0.2.3"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn semver_same_version_is_equal() {
        assert_eq!(compare_semver("0.2.3", "0.2.3"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn semver_older_version_is_less() {
        assert_eq!(compare_semver("0.1.0", "0.2.3"), std::cmp::Ordering::Less);
    }

    #[test]
    fn semver_strips_v_prefix() {
        assert_eq!(
            compare_semver("v1.0.0", "0.9.9"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn semver_patch_only_difference() {
        assert_eq!(
            compare_semver("0.2.4", "0.2.3"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn prerelease_detected() {
        assert!(is_prerelease("0.3.0-beta.1"));
        assert!(is_prerelease("v1.0.0-rc1"));
        assert!(!is_prerelease("0.2.3"));
        assert!(!is_prerelease("v0.2.3"));
    }

    #[test]
    fn platform_suffix_is_not_empty() {
        let suffix = platform_asset_suffix();
        // On test machines (Linux CI), should be "-linux"
        // On Windows, should be ".exe"
        assert!(
            !suffix.is_empty()
                || cfg!(not(any(
                    target_os = "windows",
                    target_os = "linux",
                    target_os = "macos"
                ))),
            "suffix should not be empty on known platforms"
        );
    }

    #[test]
    fn remote_version_default_expected_size() {
        let json = r#"{"version":"0.3.0","url":"http://x","notes":"","is_latest":true}"#;
        let rv: RemoteVersion = serde_json::from_str(json).expect("should parse");
        assert_eq!(rv.expected_size, 0);
    }
}
