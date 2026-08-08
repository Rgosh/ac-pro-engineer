//! Fetching a newer `shm-bridge.exe` from the release page.
//!
//! The bridge is the one piece of this that a user cannot rebuild: it is a
//! Windows binary, and cross-building it needs a mingw toolchain that a Linux
//! machine will not have unless someone installed it on purpose. So when
//! [`crate::overlay::bridge::status`] reports a bridge that cannot serve this
//! build's frames, the useful next step is not "rebuild it" — it is "fetch the
//! one that was published alongside this release".
//!
//! Deliberately separate from [`crate::updater`], which replaces the running
//! application. This downloads one auxiliary file and never touches the
//! executable, so it needs none of that machinery — no archive decoding, no
//! restart, no rollback.
//!
//! The downloaded file is verified before it replaces anything: the marker the
//! bridge compiles into itself has to be there and has to say the version the
//! release page promised. A truncated download, an HTML error page saved under
//! an `.exe` name, or an asset from another project all fail that check, and
//! the bridge already in place is left alone.

use crate::overlay::bridge::{BRIDGE_EXE, version_in_bytes};
use crate::overlay::frame::OVERLAY_MMF_NAME;
use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};

const GITHUB_OWNER: &str = "Rgosh";
const GITHUB_REPO: &str = "ac-pro-engineer";

/// Long enough for a slow connection, short enough that a launcher card does
/// not appear to have hung.
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// A published bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteBridge {
    /// The release tag, without its `v`.
    pub version: String,
    pub url: String,
    pub size: u64,
    /// Whether the asset is the `.exe` or a `.zip` around it.
    pub delivery: Delivery,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Compare two versions the way the release page numbers them.
///
/// `Greater` means `a` is newer. A prerelease suffix is dropped rather than
/// ordered: this decides whether to offer a download, and "0.4.0-beta.1 is
/// older than 0.4.0" is not a distinction worth a wrong answer here.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> (u32, u32, u32) {
        let s = s.trim().trim_start_matches('v');
        let s = s.split('-').next().unwrap_or(s);
        let mut parts = s.split('.');
        let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };
    parse(a).cmp(&parse(b))
}

/// Whether `remote` is worth downloading over `local`.
///
/// `None` for `local` — a bridge with no marker, so built before this check
/// existed — counts as worth replacing. That is the whole population this is
/// for.
pub fn is_worth_fetching(remote: &str, local: Option<&str>) -> bool {
    match local {
        None => true,
        Some(local) => compare_versions(remote, local) == std::cmp::Ordering::Greater,
    }
}

/// How a release delivers the bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// The `.exe` itself, as the hand-made releases up to v0.2.2 published it.
    Executable,
    /// A `.zip` with the `.exe` inside, which is what `dist` produces and what
    /// every release since v0.3.0 actually carries.
    Zip,
}

/// Is this asset the bridge, and how does it deliver it?
///
/// Matched by stem rather than by exact name, because the published name is
/// `shm-bridge-x86_64-pc-windows-gnu.zip` and has been since dist took over
/// releases. Matching only `shm-bridge.exe` found nothing newer than v0.2.2 and
/// would have offered that as an "update" — a downgrade past the release that
/// taught the bridge about the overlay at all.
///
/// Checksums, installers and dist's own `-update` artifacts carry the same stem
/// and are not the bridge.
fn classify_asset(name: &str) -> Option<Delivery> {
    let lower = name.to_ascii_lowercase();
    if !lower.contains("shm-bridge") {
        return None;
    }
    if lower.ends_with(".sha256") || lower.contains("installer") {
        return None;
    }
    if lower.ends_with(".exe") {
        return Some(Delivery::Executable);
    }
    if lower.ends_with(".zip") {
        return Some(Delivery::Zip);
    }
    None
}

/// The newest published bridge.
///
/// Walks every release rather than asking for `releases/latest`, because the
/// bridge is not republished with every application release — the newest
/// release that *ships one* is the answer, and it is not always the newest
/// release. Drafts and prereleases are skipped.
pub fn latest_published() -> Result<RemoteBridge, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases");
    info!("Looking for a published shm-bridge at {url}");

    // Off the caller's thread: this is reached both from a background thread
    // and straight from the terminal's key handler, and `reqwest::blocking`
    // panics on the second if the caller is inside an async runtime. See
    // `crate::net`.
    let releases: Vec<GitHubRelease> = crate::net::off_runtime(|| {
        let client = reqwest::blocking::Client::builder()
            .user_agent("AC-Pro-Engineer-Bridge-Check")
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| format!("could not build an HTTP client: {e}"))?;

        let response = client
            .get(&url)
            .send()
            .map_err(|e| format!("could not reach GitHub: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("GitHub answered {}", response.status()));
        }

        response
            .json()
            .map_err(|e| format!("could not read GitHub's answer: {e}"))
    })?;

    let mut found: Vec<RemoteBridge> = Vec::new();
    for release in releases {
        if release.draft || release.prerelease {
            continue;
        }
        let version = release.tag_name.trim_start_matches('v').to_string();
        // Prefer the zip: it is what dist publishes, and a release carrying
        // both is one where the loose .exe is the older artifact.
        let mut best: Option<(Delivery, GitHubAsset)> = None;
        for asset in release.assets {
            let Some(delivery) = classify_asset(&asset.name) else {
                continue;
            };
            if delivery == Delivery::Zip || best.is_none() {
                best = Some((delivery, asset));
            }
        }
        if let Some((delivery, asset)) = best {
            found.push(RemoteBridge {
                version: version.clone(),
                url: asset.browser_download_url,
                size: asset.size,
                delivery,
            });
        }
    }

    found.sort_by(|a, b| compare_versions(&b.version, &a.version));
    found
        .into_iter()
        .next()
        .ok_or_else(|| format!("no release of {GITHUB_OWNER}/{GITHUB_REPO} publishes {BRIDGE_EXE}"))
}

/// Download `remote` and put it at `destination`.
///
/// Written beside the destination and renamed into place, so a failed download
/// cannot leave half a bridge where a working one was. The previous bridge is
/// kept as `<name>.previous` — replacing the only copy of a binary the user
/// cannot rebuild is not a thing to do without a way back.
pub fn download_to(remote: &RemoteBridge, destination: &Path) -> Result<PathBuf, String> {
    // Same reason as above: `[B]` on the launcher's overlay card calls this
    // one keystroke after `latest_published`, from the same thread.
    let bytes: Vec<u8> = crate::net::off_runtime(|| {
        let client = reqwest::blocking::Client::builder()
            .user_agent("AC-Pro-Engineer-Bridge-Check")
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| format!("could not build an HTTP client: {e}"))?;

        let mut response = client
            .get(&remote.url)
            .send()
            .map_err(|e| format!("could not reach GitHub: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("download answered {}", response.status()));
        }

        let mut bytes: Vec<u8> = Vec::new();
        response
            .copy_to(&mut bytes)
            .map_err(|e| format!("download broke off: {e}"))?;
        Ok(bytes)
    })?;

    let bytes = match remote.delivery {
        Delivery::Executable => bytes,
        Delivery::Zip => unzip_bridge(&bytes)?,
    };

    verify(&bytes, &remote.version)?;

    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("could not create {}: {e}", parent.display()))?;

    let staged = destination.with_extension("exe.part");
    {
        let mut file = std::fs::File::create(&staged)
            .map_err(|e| format!("could not create {}: {e}", staged.display()))?;
        file.write_all(&bytes)
            .map_err(|e| format!("could not write {}: {e}", staged.display()))?;
        file.sync_all()
            .map_err(|e| format!("could not flush {}: {e}", staged.display()))?;
    }

    if destination.exists() {
        let kept = destination.with_extension("exe.previous");
        if let Err(error) = std::fs::rename(destination, &kept) {
            warn!(?error, "could not keep the bridge that was there");
        }
    }

    std::fs::rename(&staged, destination).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        format!("could not put the bridge at {}: {e}", destination.display())
    })?;

    // Wine runs it, but a Linux user will reach for it from a shell first.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o755));
    }

    info!(
        "Fetched shm-bridge {} into {}",
        remote.version,
        destination.display()
    );
    Ok(destination.to_path_buf())
}

/// Pull `shm-bridge.exe` out of a release zip.
///
/// dist's zip carries the binary beside a README, a LICENSE and a CHANGELOG, so
/// the entry is found by name rather than by taking the first one.
fn unzip_bridge(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let reader = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("the download is not a zip: {e}"))?;

    let index = (0..archive.len()).find(|index| {
        archive
            .by_index(*index)
            .ok()
            .and_then(|entry| {
                entry
                    .enclosed_name()
                    .and_then(|path| path.file_name().map(|n| n.to_ascii_lowercase()))
            })
            .is_some_and(|name| name == BRIDGE_EXE)
    });

    let Some(index) = index else {
        return Err(format!("the release zip has no {BRIDGE_EXE} in it"));
    };

    let mut entry = archive
        .by_index(index)
        .map_err(|e| format!("could not open {BRIDGE_EXE} inside the zip: {e}"))?;
    let mut out = Vec::new();
    entry
        .read_to_end(&mut out)
        .map_err(|e| format!("could not unpack {BRIDGE_EXE}: {e}"))?;
    Ok(out)
}

/// Is this really a bridge that can serve this application's overlay?
///
/// Three checks, each catching something the others do not:
///
/// 1. **A PE header.** A rate-limit page, a redirect body and an error JSON all
///    arrive happily under an `.exe` name.
/// 2. **The overlay mapping's name.** A bridge built before the overlay existed
///    maps AC's four `acpmf_*` pages and nothing else — it runs, it announces
///    nothing wrong, and the panel waits forever. Every release up to and
///    including v0.3.3 published exactly that: v0.3.3 was tagged eleven minutes
///    before the commit that taught the bridge about the overlay. Downloading
///    one of those over a working bridge would be a downgrade into the bug this
///    whole module exists to catch, so it is refused by name.
/// 3. **The version marker**, when the binary has one. Bridges from before it
///    existed do not, and cannot be made to, so its absence is not fatal — but
///    when it is there it has to agree with the tag, which catches an asset
///    built from the wrong commit.
fn verify(bytes: &[u8], expected_version: &str) -> Result<(), String> {
    if !bytes.starts_with(b"MZ") {
        return Err("what arrived is not a Windows executable".to_string());
    }

    if !contains(bytes, OVERLAY_MMF_NAME.as_bytes()) {
        return Err(format!(
            "shm-bridge {expected_version} does not know about the overlay mapping \
             ({OVERLAY_MMF_NAME}) — it maps AC's own pages and nothing else, so the \
             panel would never appear. Build one from this checkout instead: \
             cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu"
        ));
    }

    match version_in_bytes(bytes) {
        Some(found) if found == expected_version.trim_start_matches('v') => Ok(()),
        Some(found) => Err(format!(
            "the download says it is shm-bridge {found}, the release said {expected_version}"
        )),
        // No marker, but it does carry the mapping name — so it is a bridge that
        // knows about the overlay, from before the marker was added. Allowed:
        // refusing it would rule out the only published bridges that work.
        None => Ok(()),
    }
}

/// Is `needle` anywhere in `haystack`?
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack.len() >= needle.len()
        && haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `[B]` key, from the context it is actually pressed in.
    ///
    /// `crate::net`'s own test proves the mechanism offline; this proves the
    /// real call path, so it reaches GitHub and is ignored by default. Run it
    /// when touching this module:
    ///
    /// ```text
    /// cargo test -p ac_core -- --ignored survives_being_called
    /// ```
    ///
    /// It asserts nothing about the answer — offline, rate-limited and "no
    /// release publishes a bridge" are all legitimate. Returning at all rather
    /// than killing the process is the assertion.
    #[test]
    #[ignore = "reaches GitHub"]
    fn the_check_survives_being_called_from_inside_a_runtime() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build a runtime");

        let answer = runtime.block_on(async { latest_published() });
        println!("latest_published() said: {answer:?}");
    }

    #[test]
    fn versions_order_the_way_the_release_page_numbers_them() {
        use std::cmp::Ordering;
        assert_eq!(compare_versions("0.3.4", "0.3.3"), Ordering::Greater);
        assert_eq!(compare_versions("v0.4.0", "0.3.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.3.3", "0.3.3"), Ordering::Equal);
        assert_eq!(compare_versions("0.3.3", "0.10.0"), Ordering::Less);
        // A prerelease suffix is dropped rather than ordered.
        assert_eq!(compare_versions("0.4.0-beta.1", "0.4.0"), Ordering::Equal);
    }

    /// The population this exists for: a bridge built before it carried a
    /// version at all.
    #[test]
    fn a_bridge_with_no_marker_is_always_worth_replacing() {
        assert!(is_worth_fetching("0.3.3", None));
    }

    #[test]
    fn only_a_newer_bridge_is_worth_fetching() {
        assert!(is_worth_fetching("0.3.4", Some("0.3.3")));
        assert!(!is_worth_fetching("0.3.3", Some("0.3.3")));
        assert!(!is_worth_fetching("0.3.2", Some("0.3.3")));
    }

    /// The names the release page has actually used. `shm-bridge.exe` is the
    /// hand-made form up to v0.2.2; everything since dist took over is the
    /// windows-gnu zip, and matching only the `.exe` found nothing newer than
    /// v0.2.2 — which would have been offered as an update.
    #[test]
    fn the_bridge_asset_is_recognised_however_the_release_named_it() {
        assert_eq!(
            classify_asset("shm-bridge-x86_64-pc-windows-gnu.zip"),
            Some(Delivery::Zip),
            "this is the name every release since v0.3.0 actually publishes"
        );
        assert_eq!(classify_asset("shm-bridge.exe"), Some(Delivery::Executable));
        assert_eq!(classify_asset("SHM-Bridge.EXE"), Some(Delivery::Executable));

        // Same stem, not the bridge.
        assert_eq!(
            classify_asset("shm-bridge-x86_64-pc-windows-gnu.zip.sha256"),
            None
        );
        assert_eq!(classify_asset("shm-bridge-installer.ps1"), None);
        assert_eq!(classify_asset("shm-bridge-installer.sh"), None);
        assert_eq!(classify_asset("ac_tui-x86_64-pc-windows-gnu.zip"), None);
        assert_eq!(classify_asset("ac_pro_engineer.exe"), None);
    }

    /// A plausible bridge: a PE that knows the overlay mapping's name.
    fn fake_bridge(version: Option<&str>) -> Vec<u8> {
        let mut bytes = b"MZ".to_vec();
        bytes.extend_from_slice(&[0u8; 512]);
        bytes.extend_from_slice(OVERLAY_MMF_NAME.as_bytes());
        if let Some(version) = version {
            bytes.extend_from_slice(format!("ACPE-SHM-BRIDGE-VERSION={version};").as_bytes());
        }
        bytes.extend_from_slice(&[0u8; 512]);
        bytes
    }

    #[test]
    fn a_real_bridge_passes_verification() {
        assert!(verify(&fake_bridge(Some("0.3.3")), "0.3.3").is_ok());
        // The tag carries a `v`; the marker does not.
        assert!(verify(&fake_bridge(Some("0.3.3")), "v0.3.3").is_ok());
    }

    /// The bug this whole module exists for, in the form it actually shipped:
    /// v0.3.3 was tagged eleven minutes before the commit that taught the
    /// bridge about the overlay, so the published binary maps AC's four pages
    /// and nothing else. It runs, it reports no error, and the panel waits
    /// forever. Installing one of those over a working bridge must not happen.
    #[test]
    fn a_bridge_that_does_not_know_the_overlay_is_refused() {
        let mut ancient = b"MZ".to_vec();
        ancient.extend_from_slice(&[0u8; 256]);
        ancient.extend_from_slice(b"acpmf_crewchiefacpmf_staticacpmf_physicsacpmf_graphics");
        ancient.extend_from_slice(&[0u8; 256]);

        let error = verify(&ancient, "0.3.3").expect_err("a pre-overlay bridge is not usable");
        assert!(
            error.contains(OVERLAY_MMF_NAME),
            "the refusal has to name what is missing: {error}"
        );
    }

    /// A bridge that knows the overlay but predates the version marker is the
    /// newest thing that works today. Refusing it would leave nothing to fetch.
    #[test]
    fn a_bridge_without_a_marker_but_with_the_mapping_is_accepted() {
        assert!(verify(&fake_bridge(None), "0.3.4").is_ok());
    }

    /// dist publishes a zip with the binary beside a README, a LICENSE and a
    /// CHANGELOG, so the entry has to be found by name.
    #[test]
    fn the_bridge_is_found_inside_a_release_zip() {
        use std::io::Write;

        let mut buffer = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            for (name, body) in [
                ("README.md", b"not the bridge".to_vec()),
                ("LICENSE", b"nor this".to_vec()),
                ("shm-bridge.exe", fake_bridge(Some("0.3.4"))),
            ] {
                zip.start_file(name, options).expect("zip entry");
                zip.write_all(&body).expect("zip write");
            }
            zip.finish().expect("finish zip");
        }

        let extracted = unzip_bridge(&buffer).expect("the zip carries a bridge");
        assert!(extracted.starts_with(b"MZ"));
        assert!(verify(&extracted, "0.3.4").is_ok());
    }

    #[test]
    fn a_zip_without_a_bridge_in_it_is_refused() {
        use std::io::Write;

        let mut buffer = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            zip.start_file("README.md", options).expect("zip entry");
            zip.write_all(b"nothing here").expect("zip write");
            zip.finish().expect("finish zip");
        }

        let error = unzip_bridge(&buffer).expect_err("no bridge inside");
        assert!(error.contains(BRIDGE_EXE), "{error}");
    }

    /// The failure this is really guarding: GitHub answering with something
    /// that is not the asset at all — a rate-limit page, a redirect body.
    #[test]
    fn an_html_page_saved_under_an_exe_name_is_refused() {
        let page = b"<!doctype html><title>Rate limited</title>".to_vec();
        let error = verify(&page, "0.3.3").expect_err("HTML is not a bridge");
        assert!(error.contains("not a Windows executable"), "{error}");
    }

    #[test]
    fn an_asset_built_from_the_wrong_commit_is_refused() {
        let error = verify(&fake_bridge(Some("0.2.0")), "0.3.3").expect_err("versions disagree");
        assert!(
            error.contains("0.2.0") && error.contains("0.3.3"),
            "{error}"
        );
    }

    /// A truncated download keeps its `MZ` header and loses everything after
    /// it, so the mapping-name check is what catches a transfer that broke off.
    #[test]
    fn a_truncated_download_is_refused() {
        let mut truncated = fake_bridge(Some("0.3.3"));
        truncated.truncate(64);
        let error = verify(&truncated, "0.3.3").expect_err("half a bridge is not a bridge");
        assert!(error.contains(OVERLAY_MMF_NAME), "{error}");
    }

    /// The bridge this checkout builds is the reference for every check above,
    /// so run them against it where it exists rather than only against bytes
    /// made up in this file.
    ///
    /// Skipped without a cross-build, as the marker test in `bridge` is.
    #[test]
    fn the_bridge_this_checkout_builds_passes_verification() {
        let built = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
            .join("target/x86_64-pc-windows-gnu/release")
            .join(BRIDGE_EXE);

        let Ok(bytes) = std::fs::read(&built) else {
            eprintln!("{} has not been cross-built; skipping", built.display());
            return;
        };

        assert_eq!(
            verify(&bytes, env!("CARGO_PKG_VERSION")),
            Ok(()),
            "the bridge this checkout builds must be one this would install"
        );
    }
}
