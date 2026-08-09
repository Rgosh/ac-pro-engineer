use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info};
// Only the non-Windows apply path logs at warn level.
#[cfg(not(target_os = "windows"))]
use tracing::warn;

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
    /// How the chosen asset delivers the binary, which decides what the
    /// download step does with it — rename into place, unpack, or refuse.
    #[serde(default)]
    pub delivery: AssetKind,
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

/// Cargo bin name of the end-user application as published in release assets.
const APP_BIN_STEM: &str = "ac_pro_engineer";
/// The same binary as published for Windows.
const APP_BIN_EXE: &str = "ac_pro_engineer.exe";

/// Name prefixes a release *archive* of the application can carry. dist names
/// archives after the Cargo package (`ac_tui`); older manual releases used the
/// binary name.
///
/// Renaming the package means adding the new prefix here, or the updater stops
/// offering updates. That is the safe direction to fail: a missed update is
/// recoverable, installing another package's archive over the running
/// executable is not.
const APP_ARCHIVE_PREFIXES: &[&str] = &["ac_tui-", "ac_pro_engineer-", "ac-pro-engineer-"];

/// Archive extensions a release might plausibly use.
const ARCHIVE_EXTS: &[&str] = &[
    ".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tgz", ".txz", ".tbz2", ".zip", ".7z",
];

/// The subset of [`ARCHIVE_EXTS`] this build has a decoder for. dist is
/// configured to emit exactly these — `unix-archive = ".tar.gz"` in
/// dist-workspace.toml, `.zip` on Windows — so anything else in a release is a
/// format nothing here can open.
#[cfg(not(target_os = "windows"))]
const SUPPORTED_ARCHIVE_EXTS: &[&str] = &[".tar.gz", ".tgz"];
#[cfg(target_os = "windows")]
const SUPPORTED_ARCHIVE_EXTS: &[&str] = &[".zip"];

/// How a release asset delivers the application.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
pub enum AssetKind {
    /// A bare executable that can replace the running binary directly.
    #[default]
    Executable,
    /// A bundle this build can unpack the application out of.
    Archive,
    /// A bundle in a format this build has no decoder for — an `.tar.xz` from
    /// before dist-workspace.toml pinned gzip, say. Surfaced so a newer version
    /// is still reported to the user, but refused before the download starts:
    /// spending the bytes only to fail in the decoder helps nobody.
    UnsupportedArchive,
}

/// Decide whether a release asset carries the application for the OS this build
/// is running on.
///
/// Selection is an allow-list, and deliberately so: `restart_and_apply` moves
/// the chosen file over the running executable, so an asset is accepted only
/// when its name positively identifies both the application and the running
/// platform. Everything else is refused rather than guessed at — a `.deb`, an
/// `.msi`, a detached signature and another workspace package's archive all
/// name a platform, and none of them can be renamed over the running binary.
///
/// Two naming schemes are recognised:
///
/// * bare binaries, as published through v0.2.2 — exactly `ac_pro_engineer` on
///   Linux, exactly `ac_pro_engineer.exe` on Windows
/// * dist archives, which embed the Rust target triple —
///   `ac_tui-x86_64-unknown-linux-gnu.tar.gz`,
///   `ac_tui-x86_64-pc-windows-gnu.zip`
fn classify_asset(name: &str) -> Option<AssetKind> {
    let lower = name.to_ascii_lowercase();

    // A bare binary is only ever the exact Cargo bin name. v0.2.2 shipped the
    // untagged Linux build alongside the Windows one, so the `.exe` extension
    // is what tells the two apart.
    if lower == APP_BIN_STEM {
        return cfg!(target_os = "linux").then_some(AssetKind::Executable);
    }
    if lower == APP_BIN_EXE {
        return cfg!(target_os = "windows").then_some(AssetKind::Executable);
    }

    // Past that point the asset has to be an archive of this application.
    // Requiring a known archive extension is what keeps `.deb`/`.rpm`/`.msi` —
    // names dist starts emitting the moment another installer is enabled —
    // from being chmod +x'd and moved over the running binary. Requiring the
    // package prefix keeps the Wine bridge, the source snapshot and any future
    // workspace member out.
    let is_app_archive = APP_ARCHIVE_PREFIXES.iter().any(|p| lower.starts_with(p))
        && ARCHIVE_EXTS.iter().any(|ext| lower.ends_with(ext));
    if !is_app_archive {
        return None;
    }

    // dist embeds the Rust target triple, so the archive names its platform.
    let claims_windows = lower.contains("windows");
    let claims_linux = lower.contains("linux");
    let claims_macos =
        lower.contains("macos") || lower.contains("darwin") || lower.contains("apple");

    let is_for_this_os = if cfg!(target_os = "windows") {
        claims_windows && !claims_linux && !claims_macos
    } else if cfg!(target_os = "linux") {
        claims_linux && !claims_windows && !claims_macos
    } else {
        // No macOS or other build is published. Refuse rather than install a
        // binary for a foreign platform.
        false
    };

    if !is_for_this_os {
        return None;
    }

    Some(
        if SUPPORTED_ARCHIVE_EXTS
            .iter()
            .any(|ext| lower.ends_with(ext))
        {
            AssetKind::Archive
        } else {
            AssetKind::UnsupportedArchive
        },
    )
}

/// File name the application binary has inside a release archive.
fn app_binary_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ac_pro_engineer.exe"
    } else {
        APP_BIN_STEM
    }
}

/// Unpack the application binary out of a downloaded release archive.
///
/// Only the format this platform actually downloads is compiled into the
/// binary — `.tar.gz` on unix, `.zip` on Windows — but both readers are built
/// under `cfg(test)` so either can be exercised from any host. Without that,
/// the Windows path would only ever be compiled by the Windows CI leg and only
/// ever be tested by nobody.
#[cfg(not(target_os = "windows"))]
fn extract_app_binary(
    archive: &std::path::Path,
    dest: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    extract_named_entry_from_tar_gz(archive, app_binary_file_name(), dest)
}

#[cfg(target_os = "windows")]
fn extract_app_binary(
    archive: &std::path::Path,
    dest: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    extract_named_entry_from_zip(archive, app_binary_file_name(), dest)
}

/// Copy the entry called `wanted` out of a gzipped tarball.
///
/// dist nests every file under a single directory named after the archive stem,
/// e.g. `ac_tui-x86_64-unknown-linux-gnu/ac_pro_engineer`, and the archive also
/// carries LICENSE, README and any other bundled binaries. The wanted entry is
/// therefore located by file name at any depth rather than by a fixed path, so
/// a change to the wrapping directory name does not break updates.
#[cfg(any(not(target_os = "windows"), test))]
fn extract_named_entry_from_tar_gz(
    archive: &std::path::Path,
    wanted: &str,
    dest: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(archive)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));

    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        if path.file_name().and_then(|n| n.to_str()) != Some(wanted) {
            continue;
        }
        let mut out = File::create(dest)?;
        std::io::copy(&mut entry, &mut out)?;
        return Ok(());
    }

    Err(format!("{wanted} not found in {}", archive.display()).into())
}

/// Windows counterpart: dist ships a `.zip` there.
///
/// The `zip` dependency is built with only the `deflate` feature, which covers
/// what dist emits (Deflated) plus Stored, which needs no decoder. A release
/// zipped with bzip2/zstd/lzma would fail here — the round-trip tests pin the
/// two methods that are expected to work.
#[cfg(any(target_os = "windows", test))]
fn extract_named_entry_from_zip(
    archive: &std::path::Path,
    wanted: &str,
    dest: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        // `enclosed_name` rejects absolute paths and `..` traversal, so a
        // hostile archive cannot steer the match outside the tree.
        let is_match = entry
            .enclosed_name()
            .as_deref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            == Some(wanted);
        if !is_match {
            continue;
        }
        let mut out = File::create(dest)?;
        std::io::copy(&mut entry, &mut out)?;
        return Ok(());
    }

    Err(format!("{wanted} not found in {}", archive.display()).into())
}

/// Pick the best asset in a release for the running OS.
///
/// A bare executable is preferred over an archive, since it needs no unpacking
/// and older releases published one directly. An archive in an unsupported
/// format is the last resort: it cannot be installed, but returning it still
/// lets the UI tell the user a newer version exists.
fn select_asset(assets: &[GitHubAsset]) -> Option<(AssetKind, &GitHubAsset)> {
    assets
        .iter()
        .filter_map(|asset| classify_asset(&asset.name).map(|kind| (kind, asset)))
        .min_by_key(|(kind, _)| match kind {
            AssetKind::Executable => 0,
            AssetKind::Archive => 1,
            AssetKind::UnsupportedArchive => 2,
        })
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

/// Turn the GitHub release feed into the list the launcher's version carousel
/// walks with the arrow keys.
///
/// Every stable release that ships an asset for the running OS goes in,
/// *including releases older than the one running* — the carousel exists to
/// roll back to them, and `launcher.rs` renders a "you won't be able to switch
/// back" warning for exactly that case. Filtering them out left the list
/// holding a single entry whenever the newest release was already installed,
/// which is what made the arrows look dead.
///
/// The result is sorted newest-first, so index 0 is the latest release and
/// moving right walks backwards through history. GitHub returns releases in
/// creation order, which is *usually* the same thing but is not guaranteed —
/// a re-published or backported tag lands out of order.
fn build_version_list(gh_releases: &[GitHubRelease]) -> Vec<RemoteVersion> {
    let mut versions: Vec<RemoteVersion> = Vec::new();

    for release in gh_releases {
        if release.prerelease {
            continue;
        }

        let remote_ver_str = release.tag_name.trim_start_matches('v');

        // Skip prerelease version strings (e.g. "0.3.0-beta.1")
        if is_prerelease(remote_ver_str) {
            continue;
        }

        // Find the asset built for the OS we are running on
        if let Some((kind, asset)) = select_asset(&release.assets) {
            versions.push(RemoteVersion {
                version: remote_ver_str.to_string(),
                url: asset.browser_download_url.clone(),
                notes: release.body.clone().unwrap_or_default(),
                // Filled in below, once the list is in version order.
                is_latest: false,
                expected_size: asset.size,
                delivery: kind,
            });
        } else {
            info!(
                "Release v{} has no asset for {}; skipping",
                remote_ver_str,
                std::env::consts::OS
            );
        }
    }

    versions.sort_by(|a, b| compare_semver(&b.version, &a.version));
    if let Some(newest) = versions.first_mut() {
        newest.is_latest = true;
    }

    versions
}

#[derive(Clone)]
pub struct Updater {
    pub status: Arc<Mutex<UpdateStatus>>,
    pub releases: Arc<Mutex<Vec<RemoteVersion>>>,
    pub selected_index: Arc<Mutex<usize>>,
    /// When the last check was started, so a retry cannot be triggered on
    /// every frame the user sits on the UPDATE item.
    last_check: Arc<Mutex<Instant>>,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

/// How long a failed or empty check has to be left alone before the launcher
/// is allowed to ask GitHub again.
const RECHECK_INTERVAL: Duration = Duration::from_secs(60);

impl Updater {
    pub fn new() -> Self {
        let updater = Self {
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
            releases: Arc::new(Mutex::new(Vec::new())),
            selected_index: Arc::new(Mutex::new(0)),
            last_check: Arc::new(Mutex::new(Instant::now())),
        };

        updater.check_for_updates();
        updater
    }

    /// Ask GitHub again if the last attempt left us with nothing usable.
    ///
    /// The only check ran from `new()`, so a machine that was offline at
    /// startup — or behind a captive portal, which is the common case on a
    /// laptop — kept `Error` and an empty carousel for the rest of the
    /// session, with no way to retry short of restarting the app.
    ///
    /// A check that succeeded is left alone: re-polling the API while the user
    /// scrolls a menu buys nothing and burns their rate limit.
    pub fn recheck_if_stale(&self) {
        let needs_retry = {
            let status = self.status.lock().unwrap_or_else(|e| e.into_inner());
            matches!(*status, UpdateStatus::Error(_) | UpdateStatus::Idle)
        };
        if !needs_retry {
            return;
        }

        {
            let mut last = self.last_check.lock().unwrap_or_else(|e| e.into_inner());
            if last.elapsed() < RECHECK_INTERVAL {
                return;
            }
            *last = Instant::now();
        }

        self.check_for_updates();
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
        let selected = self.selected_index.clone();

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
                        // Same trap as the bridge check: a spent hourly
                        // allowance comes back as 403, which reads as a
                        // permission problem. Sixty requests an hour per
                        // address, two spent per launch.
                        let spent = resp
                            .headers()
                            .get("x-ratelimit-remaining")
                            .and_then(|value| value.to_str().ok())
                            .map(|value| value == "0")
                            .unwrap_or(false);
                        let message = if spent {
                            "GitHub's hourly limit for this address is used up; \
                             the check will work again shortly"
                                .to_string()
                        } else {
                            format!("API error: {}", resp.status())
                        };
                        error!("Update check failed: {message}");
                        let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                        *lock = UpdateStatus::Error(message);
                        return;
                    }

                    match resp.json::<Vec<GitHubRelease>>() {
                        Ok(gh_releases) => {
                            let parsed_versions = build_version_list(&gh_releases);

                            if let Some(newest) = parsed_versions.first() {
                                let newest_version = newest.version.clone();
                                {
                                    let mut r_lock =
                                        releases_store.lock().unwrap_or_else(|e| e.into_inner());
                                    *r_lock = parsed_versions;
                                }
                                // The list was just replaced, so an index left
                                // over from a previous check can be past its
                                // end. Point at the newest release again.
                                {
                                    let mut i_lock =
                                        selected.lock().unwrap_or_else(|e| e.into_inner());
                                    *i_lock = 0;
                                }

                                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());

                                if compare_semver(&newest_version, CURRENT_VERSION)
                                    == std::cmp::Ordering::Greater
                                {
                                    info!("Update available: v{}", newest_version);
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

        // Nothing here can open the asset, so say so instead of spending a
        // download to fail in the decoder. The installer from the release page
        // handles these.
        if info.delivery == AssetKind::UnsupportedArchive {
            error!(
                "Release v{} is in an unsupported archive format",
                info.version
            );
            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
            *lock = UpdateStatus::Error("Unsupported archive - use installer".to_string());
            return;
        }

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
                                            // Clamped because a body longer
                                            // than its Content-Length would
                                            // otherwise report over 100%, and
                                            // the launcher sizes its progress
                                            // bar from this number.
                                            let pct = ((downloaded as f32 / total_size as f32)
                                                * 100.0)
                                                .clamp(0.0, 100.0);
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

                            // An archive holds the binary alongside LICENSE and
                            // README, so unpack it. A bare binary is already
                            // the finished article and only needs renaming.
                            if info.delivery == AssetKind::Archive {
                                if let Err(e) = extract_app_binary(&temp_path, &final_path) {
                                    error!("Failed to extract update archive: {}", e);
                                    let _ = std::fs::remove_file(&temp_path);
                                    let _ = std::fs::remove_file(&final_path);
                                    let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                                    *lock = UpdateStatus::Error("Extract failed".to_string());
                                    return;
                                }
                                // The archive itself is no longer needed.
                                let _ = std::fs::remove_file(&temp_path);
                            } else if let Err(e) = std::fs::rename(&temp_path, &final_path) {
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

    fn asset(name: &str) -> GitHubAsset {
        GitHubAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.invalid/{name}"),
            size: 1234,
        }
    }

    /// Every artifact `dist plan` emits for this workspace, verbatim. Exactly
    /// one of them is installable on any given platform; if dist changes how it
    /// names things, this list is where it shows up.
    const REAL_RELEASE_ASSETS: &[&str] = &[
        "ac_tui-installer.ps1",
        "ac_tui-installer.sh",
        "ac_tui-x86_64-pc-windows-gnu-update",
        "ac_tui-x86_64-pc-windows-gnu.zip",
        "ac_tui-x86_64-pc-windows-gnu.zip.sha256",
        "ac_tui-x86_64-unknown-linux-gnu-update",
        "ac_tui-x86_64-unknown-linux-gnu.tar.gz",
        "ac_tui-x86_64-unknown-linux-gnu.tar.gz.sha256",
        "dist-manifest.json",
        "sha256.sum",
        "shm-bridge-installer.ps1",
        "shm-bridge-installer.sh",
        "shm-bridge-x86_64-pc-windows-gnu-update",
        "shm-bridge-x86_64-pc-windows-gnu.zip",
        "shm-bridge-x86_64-pc-windows-gnu.zip.sha256",
        "source.tar.gz",
        "source.tar.gz.sha256",
    ];

    /// Artifacts that are never the application, on any platform.
    #[test]
    fn non_application_assets_are_rejected() {
        for name in [
            "shm-bridge.exe",
            "shm-bridge-x86_64-pc-windows-gnu.zip",
            "ac_tui-installer.sh",
            "ac_tui-installer.ps1",
            "ac_tui-x86_64-unknown-linux-gnu-update",
            "sha256.sum",
            "ac_pro_engineer.exe.sha256",
            "source.tar.gz",
        ] {
            assert_eq!(classify_asset(name), None, "{name} should be rejected");
        }
    }

    /// The regression the allow-list exists for. Every one of these names a
    /// platform and is not an archive, so the previous "anything without an
    /// archive extension is a bare executable" rule would have picked it *in
    /// preference to* the real archive, chmod +x'd it and moved it over the
    /// running binary. Enabling one more dist installer is all it takes to put
    /// these in a release.
    #[test]
    fn package_formats_are_never_treated_as_executables() {
        for name in [
            "ac_tui-x86_64-pc-windows-gnu.msi",
            "ac_tui-x86_64-unknown-linux-gnu.deb",
            "ac_tui-x86_64-unknown-linux-gnu.rpm",
            "ac_tui-x86_64-unknown-linux-gnu.AppImage",
            "ac_tui-x86_64-apple-darwin.pkg",
            "ac_tui-x86_64-unknown-linux-gnu.tar.gz.sig",
            "ac_tui-x86_64-pc-windows-gnu.zip.asc",
        ] {
            assert_eq!(classify_asset(name), None, "{name} should be rejected");
        }
    }

    /// Another workspace package's archive names a platform *and* ends in a
    /// real archive extension — only the application prefix keeps it out.
    #[test]
    fn other_packages_archives_are_rejected() {
        for name in [
            "shm-bridge-x86_64-pc-windows-gnu.zip",
            "shm-bridge-x86_64-unknown-linux-gnu.tar.gz",
            "some-future-tool-x86_64-unknown-linux-gnu.tar.gz",
        ] {
            assert_eq!(classify_asset(name), None, "{name} should be rejected");
        }
    }

    #[test]
    fn a_real_dist_release_yields_exactly_one_installable_asset() {
        let accepted: Vec<&str> = REAL_RELEASE_ASSETS
            .iter()
            .copied()
            .filter(|name| classify_asset(name).is_some())
            .collect();

        let expected: &[&str] = if cfg!(target_os = "windows") {
            &["ac_tui-x86_64-pc-windows-gnu.zip"]
        } else if cfg!(target_os = "linux") {
            &["ac_tui-x86_64-unknown-linux-gnu.tar.gz"]
        } else {
            &[]
        };

        assert_eq!(accepted, expected);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_selects_the_linux_binary_and_never_the_windows_one() {
        // v0.2.2 shipped all three of these in one release.
        let assets = [
            asset("ac_pro_engineer"),
            asset("ac_pro_engineer.exe"),
            asset("shm-bridge.exe"),
        ];
        let (kind, chosen) = select_asset(&assets).expect("a linux asset exists");
        assert_eq!(chosen.name, "ac_pro_engineer");
        assert_eq!(kind, AssetKind::Executable);

        // A Windows-only release must yield nothing rather than an .exe.
        assert!(select_asset(&[asset("ac_pro_engineer.exe")]).is_none());
        assert_eq!(classify_asset("ac_pro_engineer.exe"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_recognises_the_dist_archive_as_needing_extraction() {
        let assets = [
            asset("ac_tui-x86_64-pc-windows-gnu.zip"),
            asset("ac_tui-x86_64-unknown-linux-gnu.tar.gz"),
        ];
        let (kind, chosen) = select_asset(&assets).expect("a linux archive exists");
        assert_eq!(chosen.name, "ac_tui-x86_64-unknown-linux-gnu.tar.gz");
        assert_eq!(kind, AssetKind::Archive);
    }

    /// dist defaults to `.tar.xz` on unix; dist-workspace.toml overrides that to
    /// gzip precisely because this build links no liblzma. A release from before
    /// that pin is still reported — the version number is useful — but it is
    /// marked unsupported, and `download_update` refuses it up front instead of
    /// pulling the whole file down to fail in the decoder.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_reports_an_xz_archive_but_marks_it_unsupported() {
        assert_eq!(
            classify_asset("ac_tui-x86_64-unknown-linux-gnu.tar.xz"),
            Some(AssetKind::UnsupportedArchive)
        );

        // The supported archive wins whenever a release carries both.
        let assets = [
            asset("ac_tui-x86_64-unknown-linux-gnu.tar.xz"),
            asset("ac_tui-x86_64-unknown-linux-gnu.tar.gz"),
        ];
        let (kind, chosen) = select_asset(&assets).expect("a linux archive exists");
        assert_eq!(chosen.name, "ac_tui-x86_64-unknown-linux-gnu.tar.gz");
        assert_eq!(kind, AssetKind::Archive);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_selects_the_exe_and_never_the_bare_elf() {
        let assets = [
            asset("ac_pro_engineer"),
            asset("ac_pro_engineer.exe"),
            asset("shm-bridge.exe"),
        ];
        let (kind, chosen) = select_asset(&assets).expect("a windows asset exists");
        assert_eq!(chosen.name, "ac_pro_engineer.exe");
        assert_eq!(kind, AssetKind::Executable);

        // The untagged bare binary is a Linux ELF; it must not be picked here.
        assert_eq!(classify_asset("ac_pro_engineer"), None);
        assert!(select_asset(&[asset("ac_pro_engineer")]).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_recognises_the_dist_archive_as_needing_extraction() {
        let assets = [
            asset("ac_tui-x86_64-unknown-linux-gnu.tar.gz"),
            asset("ac_tui-x86_64-pc-windows-gnu.zip"),
        ];
        let (kind, chosen) = select_asset(&assets).expect("a windows archive exists");
        assert_eq!(chosen.name, "ac_tui-x86_64-pc-windows-gnu.zip");
        assert_eq!(kind, AssetKind::Archive);
    }

    /// Only `.zip` has a decoder on Windows; a tarball named for this platform
    /// is reported but not installable.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_marks_a_tarball_unsupported() {
        assert_eq!(
            classify_asset("ac_tui-x86_64-pc-windows-gnu.tar.gz"),
            Some(AssetKind::UnsupportedArchive)
        );
    }

    /// The payload every extraction round-trip looks for.
    const PAYLOAD: &[u8] = b"#!/bin/sh\necho updated\n";

    /// A scratch directory of its own per test, so the suite stays parallel.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create tmp");
        dir
    }

    /// Build a gzipped tarball laid out the way dist does: everything under a
    /// directory named after the archive stem, alongside LICENSE, README and a
    /// sibling binary that must not be mistaken for the application.
    fn write_tar_gz(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        use std::io::Write;

        let gz = flate2::write::GzEncoder::new(
            File::create(path).expect("create archive"),
            flate2::Compression::fast(),
        );
        let mut builder = tar::Builder::new(gz);

        for (name, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, name, *bytes)
                .expect("append");
        }

        let mut gz = builder.into_inner().expect("finish tar");
        gz.flush().expect("flush");
    }

    /// Zip counterpart. `method` is what dist's compression choice maps to.
    fn write_zip(
        path: &std::path::Path,
        entries: &[(&str, &[u8])],
        method: zip::CompressionMethod,
    ) {
        use std::io::Write;

        let mut writer = zip::ZipWriter::new(File::create(path).expect("create archive"));
        let options = zip::write::SimpleFileOptions::default().compression_method(method);

        for (name, bytes) in entries {
            writer.start_file(*name, options).expect("start entry");
            writer.write_all(bytes).expect("write entry");
        }

        writer.finish().expect("finish zip");
    }

    /// The real dist layout, in the format unix downloads.
    #[test]
    fn extracts_the_app_binary_from_a_nested_tar_gz() {
        let tmp = scratch_dir("ac_updater_extract_tar_test");
        let archive_path = tmp.join("ac_tui-x86_64-unknown-linux-gnu.tar.gz");
        let root = "ac_tui-x86_64-unknown-linux-gnu";

        write_tar_gz(
            &archive_path,
            &[
                (&format!("{root}/LICENSE"), b"license"),
                (&format!("{root}/README.md"), b"readme"),
                // A sibling binary that must not be mistaken for the app.
                (&format!("{root}/simulator"), b"not the app"),
                (&format!("{root}/{APP_BIN_STEM}"), PAYLOAD),
            ],
        );

        let dest = tmp.join("ac_pro_engineer_new");
        extract_named_entry_from_tar_gz(&archive_path, APP_BIN_STEM, &dest)
            .expect("extraction should succeed");

        let got = std::fs::read(&dest).expect("read extracted binary");
        assert_eq!(got, PAYLOAD, "extracted the wrong archive entry");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// The same layout in the format Windows downloads. Runs on every host —
    /// gating this to Windows would leave the zip reader compiled only by the
    /// Windows CI leg and verified by nothing.
    ///
    /// Both methods dist can realistically emit are covered: Deflated is what
    /// it uses, Stored needs no decoder and would silently pass either way.
    #[test]
    fn extracts_the_app_binary_from_a_nested_zip() {
        for (label, method) in [
            ("deflated", zip::CompressionMethod::Deflated),
            ("stored", zip::CompressionMethod::Stored),
        ] {
            let tmp = scratch_dir(&format!("ac_updater_extract_zip_{label}_test"));
            let archive_path = tmp.join("ac_tui-x86_64-pc-windows-gnu.zip");
            let root = "ac_tui-x86_64-pc-windows-gnu";
            let wanted = "ac_pro_engineer.exe";

            write_zip(
                &archive_path,
                &[
                    (&format!("{root}/LICENSE"), b"license"),
                    (&format!("{root}/README.txt"), b"readme"),
                    (&format!("{root}/simulator.exe"), b"not the app"),
                    (&format!("{root}/{wanted}"), PAYLOAD),
                ],
                method,
            );

            let dest = tmp.join("ac_pro_engineer_new.exe");
            let extracted = extract_named_entry_from_zip(&archive_path, wanted, &dest);
            assert!(
                extracted.is_ok(),
                "{label} extraction should succeed: {extracted:?}"
            );

            let got = std::fs::read(&dest).expect("read extracted binary");
            assert_eq!(got, PAYLOAD, "{label}: extracted the wrong archive entry");

            let _ = std::fs::remove_dir_all(&tmp);
        }
    }

    /// A release archive missing the application must fail loudly rather than
    /// leave a truncated or wrong file where the binary is expected.
    #[test]
    fn extraction_fails_when_the_archive_has_no_app_binary() {
        let tmp = scratch_dir("ac_updater_extract_missing_test");

        let tar_path = tmp.join("bogus.tar.gz");
        write_tar_gz(&tar_path, &[("some-dir/LICENSE", b"license")]);

        let zip_path = tmp.join("bogus.zip");
        write_zip(
            &zip_path,
            &[("some-dir/LICENSE", b"license")],
            zip::CompressionMethod::Deflated,
        );

        let dest = tmp.join("ac_pro_engineer_new");
        assert!(extract_named_entry_from_tar_gz(&tar_path, APP_BIN_STEM, &dest).is_err());
        assert!(extract_named_entry_from_zip(&zip_path, "ac_pro_engineer.exe", &dest).is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A bare executable is preferred when a release carries both forms.
    #[test]
    fn executable_is_preferred_over_archive() {
        let bare = if cfg!(target_os = "windows") {
            "ac_pro_engineer.exe"
        } else {
            "ac_pro_engineer"
        };
        let archive = if cfg!(target_os = "windows") {
            "ac_tui-x86_64-pc-windows-gnu.zip"
        } else {
            "ac_tui-x86_64-unknown-linux-gnu.tar.gz"
        };

        if cfg!(any(target_os = "windows", target_os = "linux")) {
            let assets = [asset(archive), asset(bare)];
            let (kind, chosen) = select_asset(&assets).expect("an asset exists");
            assert_eq!(chosen.name, bare);
            assert_eq!(kind, AssetKind::Executable);
        }
    }

    #[test]
    fn remote_version_default_expected_size() {
        let json = r#"{"version":"0.3.0","url":"http://x","notes":"","is_latest":true}"#;
        let rv: RemoteVersion = serde_json::from_str(json).expect("should parse");
        assert_eq!(rv.expected_size, 0);
    }

    /// An asset name this platform will actually classify, so the release
    /// carries something installable wherever the test runs.
    fn platform_asset() -> GitHubAsset {
        asset(if cfg!(target_os = "windows") {
            "ac_pro_engineer.exe"
        } else {
            "ac_pro_engineer"
        })
    }

    fn release(tag: &str, prerelease: bool) -> GitHubRelease {
        GitHubRelease {
            tag_name: tag.to_string(),
            body: Some(format!("notes for {tag}")),
            assets: vec![platform_asset()],
            prerelease,
        }
    }

    /// The regression this whole change is about: an older release must survive
    /// into the list, or the launcher's arrows have nowhere to move.
    #[test]
    fn version_list_keeps_releases_older_than_the_running_one() {
        let feed = [release("v0.0.1", false)];
        let versions = build_version_list(&feed);

        assert_eq!(
            versions.len(),
            1,
            "a release older than {CURRENT_VERSION} must still be offered for rollback"
        );
        assert_eq!(versions[0].version, "0.0.1");
    }

    #[test]
    fn version_list_is_sorted_newest_first() {
        // Deliberately out of order: GitHub returns creation order, which a
        // backported or re-published tag breaks.
        let feed = [
            release("v0.2.3", false),
            release("v1.0.0", false),
            release("v0.9.9", false),
        ];
        let versions = build_version_list(&feed);

        let order: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(order, ["1.0.0", "0.9.9", "0.2.3"]);
    }

    #[test]
    fn version_list_marks_only_the_newest_as_latest() {
        let feed = [release("v0.2.3", false), release("v1.0.0", false)];
        let versions = build_version_list(&feed);

        assert!(versions[0].is_latest, "1.0.0 is the latest");
        assert!(!versions[1].is_latest, "0.2.3 is not");
    }

    #[test]
    fn version_list_skips_prereleases() {
        let feed = [
            release("v2.0.0-beta.1", false), // prerelease by version string
            release("v1.5.0", true),         // prerelease by GitHub flag
            release("v1.0.0", false),
        ];
        let versions = build_version_list(&feed);

        let order: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(order, ["1.0.0"]);
    }

    #[test]
    fn version_list_skips_releases_with_no_asset_for_this_platform() {
        let mut no_asset = release("v1.0.0", false);
        no_asset.assets = vec![asset("ac_tui-installer.sh"), asset("sha256.sum")];
        let feed = [no_asset, release("v0.9.0", false)];

        let versions = build_version_list(&feed);
        let order: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
        assert_eq!(order, ["0.9.0"]);
    }

    /// The arrows themselves, over a list with more than one entry — which is
    /// the state the old filter made unreachable.
    #[test]
    fn carousel_walks_the_whole_version_list() {
        let updater = Updater {
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
            releases: Arc::new(Mutex::new(build_version_list(&[
                release("v1.0.0", false),
                release("v0.9.0", false),
                release("v0.2.3", false),
            ]))),
            selected_index: Arc::new(Mutex::new(0)),
            last_check: Arc::new(Mutex::new(Instant::now())),
        };

        let selected = || {
            updater
                .get_selected_release()
                .expect("the list is not empty")
                .version
        };

        assert_eq!(selected(), "1.0.0");

        updater.next_version();
        assert_eq!(
            selected(),
            "0.9.0",
            "right should walk back through history"
        );
        updater.next_version();
        assert_eq!(selected(), "0.2.3");

        // The oldest entry is the end of the road.
        updater.next_version();
        assert_eq!(selected(), "0.2.3");

        updater.prev_version();
        assert_eq!(selected(), "0.9.0", "left should walk forward again");
        updater.prev_version();
        assert_eq!(selected(), "1.0.0");

        // And the newest entry is the other end.
        updater.prev_version();
        assert_eq!(selected(), "1.0.0");
    }
}
