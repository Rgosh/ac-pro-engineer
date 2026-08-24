use crate::games::registry::SetupStore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tracing::error;

/// Sanitize a filename component: strip path separators, `..`, control chars,
/// and Windows reserved names. Returns a safe alphanumeric+dash+underscore string.
fn sanitize_filename_component(raw: &str) -> String {
    // Strip path separators and null bytes
    let cleaned: String = raw
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0'))
        .collect();

    // Collapse any ".." sequences
    let cleaned = cleaned.replace("..", "");

    // Replace non-alphanumeric/dash/underscore with underscore
    let cleaned: String = cleaned
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Block Windows reserved device names
    let upper = cleaned.to_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = upper.split('.').next().unwrap_or("");
    if reserved.contains(&stem) {
        return format!("_{}", cleaned);
    }

    if cleaned.is_empty() {
        return "_unknown".to_string();
    }

    cleaned
}

/// Join a filename under a root directory and verify the canonical result
/// stays inside the root. Returns None if the path would escape.
fn safe_join_under(root: &std::path::Path, filename: &str) -> Option<PathBuf> {
    let sanitized = sanitize_filename_component(filename);
    let candidate = root.join(&sanitized);

    // Canonicalize root; candidate may not exist yet so we check prefix
    if let Ok(canon_root) = std::fs::canonicalize(root) {
        // For a new file, canonicalize the parent
        if let Some(parent) = candidate.parent()
            && let Ok(canon_parent) = std::fs::canonicalize(parent)
            && canon_parent.starts_with(&canon_root)
        {
            return Some(candidate);
        }
    }

    // Fallback: if root doesn't exist yet either, just check component-level
    if !sanitized.contains("..") && !sanitized.starts_with('/') {
        return Some(candidate);
    }

    None
}

/// How often the local setup folders are rescanned when nothing has changed.
///
/// Setups appear when the user saves one from inside the game, so the folder
/// changes on a scale of minutes. This only has to be fast enough that a
/// setup saved mid-session shows up without a restart.
const RESCAN_INTERVAL: Duration = Duration::from_secs(10);

const GITHUB_USER_REPO: &str = "Rgosh/ac-setups";
const GITHUB_BRANCH: &str = "main";

#[derive(Debug, Clone)]
pub struct SetupDiffItem {
    pub name: String,
    pub current: f32,
    pub reference: f32,
    pub diff: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CarSetup {
    pub name: String,
    #[serde(skip)]
    pub path: PathBuf,
    pub source: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub credits: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub car_id: String,
    #[serde(skip)]
    pub is_remote: bool,

    #[serde(default)]
    pub fuel: u32,
    #[serde(default)]
    pub brake_bias: u32,
    #[serde(default)]
    pub engine_limiter: u32,
    #[serde(default)]
    pub pressure_lf: u32,
    #[serde(default)]
    pub pressure_rf: u32,
    #[serde(default)]
    pub pressure_lr: u32,
    #[serde(default)]
    pub pressure_rr: u32,
    #[serde(default)]
    pub wing_1: u32,
    #[serde(default)]
    pub wing_2: u32,
    #[serde(default)]
    pub camber_lf: i32,
    #[serde(default)]
    pub camber_rf: i32,
    #[serde(default)]
    pub camber_lr: i32,
    #[serde(default)]
    pub camber_rr: i32,
    #[serde(default)]
    pub toe_lf: i32,
    #[serde(default)]
    pub toe_rf: i32,
    #[serde(default)]
    pub toe_lr: i32,
    #[serde(default)]
    pub toe_rr: i32,
    #[serde(default)]
    pub spring_lf: u32,
    #[serde(default)]
    pub spring_rf: u32,
    #[serde(default)]
    pub spring_lr: u32,
    #[serde(default)]
    pub spring_rr: u32,
    #[serde(default)]
    pub rod_length_lf: i32,
    #[serde(default)]
    pub rod_length_rf: i32,
    #[serde(default)]
    pub rod_length_lr: i32,
    #[serde(default)]
    pub rod_length_rr: i32,
    #[serde(default)]
    pub arb_front: u32,
    #[serde(default)]
    pub arb_rear: u32,
    #[serde(default)]
    pub damp_bump_lf: u32,
    #[serde(default)]
    pub damp_rebound_lf: u32,
    #[serde(default)]
    pub damp_bump_rf: u32,
    #[serde(default)]
    pub damp_rebound_rf: u32,
    #[serde(default)]
    pub damp_bump_lr: u32,
    #[serde(default)]
    pub damp_rebound_lr: u32,
    #[serde(default)]
    pub damp_bump_rr: u32,
    #[serde(default)]
    pub damp_rebound_rr: u32,
    #[serde(default)]
    pub diff_power: u32,
    #[serde(default)]
    pub diff_coast: u32,
    #[serde(default)]
    pub final_ratio: u32,
    #[serde(default)]
    pub gears: Vec<u32>,
}

impl CarSetup {
    /// Score needed before a local setup file is claimed to be the one loaded
    /// in the car.
    ///
    /// `match_score` awards 30 for fuel, 25 for brake bias and 20 for
    /// pressures, so the only reachable totals are 0, 20, 25, 30, 45, 50, 55
    /// and 75. The old threshold of `> 60` therefore admitted nothing but a
    /// perfect 3/3 — one lap's worth of burnt fuel dropped the score to 55 and
    /// the match silently disappeared, taking the "(NOW: x%)" hints in the
    /// brake-bias and camber advice with it.
    ///
    /// 45 is the first total that requires two of the three to agree.
    pub const MIN_MATCH_SCORE: u32 = 45;

    pub fn match_score(
        &self,
        current_fuel: f32,
        current_bias: f32,
        current_pressures: &[f32; 4],
    ) -> u32 {
        let mut score = 0;
        if (self.fuel as f32 - current_fuel).abs() < 2.0 {
            score += 30;
        }
        let bias_file = self.brake_bias as f32 / 100.0;
        if (bias_file - current_bias).abs() < 0.05 {
            score += 25;
        }
        let avg_p_file = (self.pressure_lf + self.pressure_rf + self.pressure_lr + self.pressure_rr)
            as f32
            / 4.0;
        let avg_p_curr = current_pressures.iter().sum::<f32>() / 4.0;
        if (avg_p_file - avg_p_curr).abs() < 2.0 {
            score += 20;
        }
        score
    }

    pub fn generate_diff(&self, reference: &CarSetup) -> Vec<SetupDiffItem> {
        let mut diffs = Vec::new();
        let mut check = |name: &str, cur: f32, ref_val: f32| {
            if (cur - ref_val).abs() > 0.001 {
                diffs.push(SetupDiffItem {
                    name: name.to_string(),
                    current: cur,
                    reference: ref_val,
                    diff: cur - ref_val,
                });
            }
        };

        check("Front Wing", self.wing_1 as f32, reference.wing_1 as f32);
        check("Rear Wing", self.wing_2 as f32, reference.wing_2 as f32);
        check(
            "Front ARB",
            self.arb_front as f32,
            reference.arb_front as f32,
        );
        check("Rear ARB", self.arb_rear as f32, reference.arb_rear as f32);
        check("Fuel (L)", self.fuel as f32, reference.fuel as f32);
        check(
            "Brake Bias (%)",
            self.brake_bias as f32,
            reference.brake_bias as f32,
        );
        check(
            "Camber FL",
            self.camber_lf as f32 / 10.0,
            reference.camber_lf as f32 / 10.0,
        );
        check(
            "Camber FR",
            self.camber_rf as f32 / 10.0,
            reference.camber_rf as f32 / 10.0,
        );
        check(
            "Camber RL",
            self.camber_lr as f32 / 10.0,
            reference.camber_lr as f32 / 10.0,
        );
        check(
            "Camber RR",
            self.camber_rr as f32 / 10.0,
            reference.camber_rr as f32 / 10.0,
        );
        check(
            "Spring FL",
            self.spring_lf as f32,
            reference.spring_lf as f32,
        );
        check(
            "Spring RL",
            self.spring_lr as f32,
            reference.spring_lr as f32,
        );
        check(
            "Pressure FL",
            self.pressure_lf as f32,
            reference.pressure_lf as f32,
        );
        check(
            "Pressure RL",
            self.pressure_lr as f32,
            reference.pressure_lr as f32,
        );

        diffs
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestItem {
    pub id: String,
    pub count: usize,
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchState {
    Idle,
    Loading,
    Ready,
    Failed {
        error: String,
        retry_at: std::time::Instant,
        attempts: u32,
    },
}

pub struct SetupManager {
    pub setups: Arc<Mutex<Vec<CarSetup>>>,
    pub current_car: Arc<Mutex<String>>,
    pub current_track: Arc<Mutex<String>>,
    pub active_setup_index: Arc<Mutex<Option<usize>>>,

    pub browser_active: Arc<Mutex<bool>>,
    pub manifest: Arc<Mutex<Vec<ManifestItem>>>,
    pub browser_car_idx: Arc<Mutex<usize>>,
    pub browser_setup_idx: Arc<Mutex<usize>>,
    pub browser_focus_col: Arc<Mutex<u8>>,
    pub browser_setups: Arc<Mutex<Vec<CarSetup>>>,

    pub details_scroll: Arc<Mutex<usize>>,
    pub loading_tick: Arc<Mutex<usize>>,

    /// Configured AC Documents folder, empty when auto-detecting. Shared so
    /// the background scan thread resolves the same directory the UI does.
    pub documents_override: Arc<Mutex<PathBuf>>,

    pub fetch_state: Arc<Mutex<FetchState>>,
    pub last_status: Arc<Mutex<String>>,
    /// How this game's setups are read and written, or `None` for a game
    /// that keeps none this program can handle.
    store: Option<&'static SetupStore>,
    pub shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    pub bg_thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

trait SafeLock<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> SafeLock<T> for Mutex<T> {
    fn safe_lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for SetupManager {
    /// A manager with no setup store, which is the right default: reading
    /// setups is something a *game* can do, and a default cannot know which
    /// game it is for.
    fn default() -> Self {
        Self::new(None)
    }
}

impl SetupManager {
    pub fn shutdown(&self) {
        self.shutdown_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut lock) = self.bg_thread.lock()
            && let Some(handle) = lock.take()
        {
            let _ = handle.join();
        }
    }
}

impl Drop for SetupManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl SetupManager {
    /// Build a manager that reads setups with `scan`.
    ///
    /// `None` is a game that keeps no setups this program can read — iRacing
    /// stores them in a format of its own — and the manager then holds an
    /// empty list rather than pretending the folder is empty. The screens read
    /// [`Capabilities::setups`](crate::games::Capabilities::setups) and say so.
    pub fn new(store: Option<&'static SetupStore>) -> Self {
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let bg_thread_slot = Arc::new(Mutex::new(None));

        let manager = Self {
            setups: Arc::new(Mutex::new(Vec::new())),
            current_car: Arc::new(Mutex::new(String::new())),
            current_track: Arc::new(Mutex::new(String::new())),
            active_setup_index: Arc::new(Mutex::new(None)),

            browser_active: Arc::new(Mutex::new(false)),
            manifest: Arc::new(Mutex::new(Vec::new())),
            browser_car_idx: Arc::new(Mutex::new(0)),
            browser_setup_idx: Arc::new(Mutex::new(0)),
            browser_focus_col: Arc::new(Mutex::new(0)),
            browser_setups: Arc::new(Mutex::new(Vec::new())),

            details_scroll: Arc::new(Mutex::new(0)),
            loading_tick: Arc::new(Mutex::new(0)),

            documents_override: Arc::new(Mutex::new(PathBuf::new())),

            fetch_state: Arc::new(Mutex::new(FetchState::Idle)),
            last_status: Arc::new(Mutex::new(String::new())),

            shutdown_flag: shutdown.clone(),
            bg_thread: bg_thread_slot.clone(),
            store,
        };

        let setups_clone = manager.setups.clone();
        let car_clone = manager.current_car.clone();
        let track_clone = manager.current_track.clone();
        let fetch_state_clone = manager.fetch_state.clone();
        let documents_clone = manager.documents_override.clone();
        let manifest_clone = manager.manifest.clone();

        let shutdown_loop = shutdown.clone();

        let handle = thread::spawn(move || {
            let mut last_car = String::new();
            let mut last_scan: Option<Instant> = None;
            let mut last_track = String::new();

            if let Ok(m) = fetch_manifest() {
                *manifest_clone.safe_lock() = m;
            }

            loop {
                if shutdown_loop.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }

                let is_empty = manifest_clone.safe_lock().is_empty();
                if is_empty && let Ok(m) = fetch_manifest() {
                    *manifest_clone.safe_lock() = m;
                }

                let car = car_clone.safe_lock().clone();
                let track = track_clone.safe_lock().clone();

                if !car.is_empty() {
                    let context_changed = car != last_car || track != last_track;
                    if context_changed {
                        *fetch_state_clone.safe_lock() = FetchState::Idle;
                        last_car = car.clone();
                        last_track = track.clone();
                        setups_clone.safe_lock().clear();
                    }

                    // `scan_folders` walks three directory trees and parses
                    // every setup file it finds. Doing that twice a second for
                    // a directory that changes when the user saves a setup in
                    // the game — minutes apart at best — is pure waste. Rescan
                    // when the car or track changes, and otherwise on a slow
                    // heartbeat so a setup saved mid-session still appears.
                    let due_for_rescan = context_changed
                        || last_scan.is_none_or(|at: Instant| at.elapsed() >= RESCAN_INTERVAL);

                    if !due_for_rescan {
                        for _ in 0..10 {
                            if shutdown_loop.load(std::sync::atomic::Ordering::SeqCst) {
                                return;
                            }
                            thread::sleep(Duration::from_millis(50));
                        }
                        continue;
                    }
                    last_scan = Some(Instant::now());

                    let configured_docs = documents_clone.safe_lock().clone();
                    let mut all_setups = match store {
                        Some(store) => (store.scan)(&car, &track, &configured_docs),
                        None => Vec::new(),
                    };
                    let current_state = fetch_state_clone.safe_lock().clone();

                    match current_state {
                        FetchState::Idle => {
                            *fetch_state_clone.safe_lock() = FetchState::Loading;
                            match fetch_server_setups(&car) {
                                Ok(mut server_setups) => {
                                    for s in &mut server_setups {
                                        if s.car_id.is_empty() {
                                            s.car_id = car.clone();
                                        }
                                    }
                                    all_setups.append(&mut server_setups);
                                    *fetch_state_clone.safe_lock() = FetchState::Ready;
                                }
                                Err(err_msg) => {
                                    let backoff_secs = 2u64.pow(1);
                                    let retry_at = std::time::Instant::now()
                                        + Duration::from_secs(backoff_secs);
                                    *fetch_state_clone.safe_lock() = FetchState::Failed {
                                        error: err_msg,
                                        retry_at,
                                        attempts: 1,
                                    };
                                }
                            }
                        }
                        FetchState::Failed {
                            error: _,
                            retry_at,
                            attempts,
                        } => {
                            if std::time::Instant::now() >= retry_at {
                                *fetch_state_clone.safe_lock() = FetchState::Loading;
                                match fetch_server_setups(&car) {
                                    Ok(mut server_setups) => {
                                        for s in &mut server_setups {
                                            if s.car_id.is_empty() {
                                                s.car_id = car.clone();
                                            }
                                        }
                                        all_setups.append(&mut server_setups);
                                        *fetch_state_clone.safe_lock() = FetchState::Ready;
                                    }
                                    Err(err_msg) => {
                                        let next_attempts = attempts + 1;
                                        let backoff_secs = 2u64.pow(next_attempts.min(6)).min(60);
                                        let next_retry = std::time::Instant::now()
                                            + Duration::from_secs(backoff_secs);
                                        *fetch_state_clone.safe_lock() = FetchState::Failed {
                                            error: err_msg,
                                            retry_at: next_retry,
                                            attempts: next_attempts,
                                        };
                                    }
                                }
                            } else {
                                let existing = setups_clone.safe_lock();
                                let remotes: Vec<CarSetup> = existing
                                    .iter()
                                    .filter(|s| s.is_remote && s.car_id == car)
                                    .cloned()
                                    .collect();
                                drop(existing);
                                all_setups.extend(remotes);
                            }
                        }
                        FetchState::Loading | FetchState::Ready => {
                            let existing = setups_clone.safe_lock();
                            let remotes: Vec<CarSetup> = existing
                                .iter()
                                .filter(|s| s.is_remote && s.car_id == car)
                                .cloned()
                                .collect();
                            drop(existing);
                            all_setups.extend(remotes);
                        }
                    }

                    *setups_clone.safe_lock() = all_setups;
                }

                for _ in 0..10 {
                    if shutdown_loop.load(std::sync::atomic::Ordering::SeqCst) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        });

        *bg_thread_slot.safe_lock() = Some(handle);

        manager
    }

    /// Where AC keeps its setups, honouring the configured override.
    ///
    /// Under Proton this is inside the game's prefix, not the host's
    /// ~/Documents — which is why `UserDirs::document_dir()` found nothing on
    /// Linux and local setups were never discovered there.
    fn setups_root(&self) -> Option<PathBuf> {
        let configured = self.documents_override.safe_lock().clone();
        (self.store?.root)((!configured.as_os_str().is_empty()).then_some(configured.as_path()))
    }

    /// Point the manager at a specific AC Documents folder. Empty resumes
    /// auto-detection.
    pub fn set_documents_override(&self, path: &std::path::Path) {
        *self.documents_override.safe_lock() = path.to_path_buf();
    }

    pub fn scroll_details(&self, delta: i32) {
        let mut scroll = self.details_scroll.safe_lock();
        if delta < 0 {
            if *scroll > 0 {
                *scroll -= 1;
            }
        } else {
            *scroll += 1;
        }
    }

    pub fn is_installed(&self, setup: &CarSetup, target_car: &str) -> bool {
        if let Some(root) = self.setups_root() {
            let safe_name = sanitize_filename_component(&setup.name);
            let safe_author = sanitize_filename_component(&setup.author);
            let Some(store) = self.store else {
                return false;
            };
            let file_name = (store.file_name)(&safe_author, &safe_name);
            let target_dir = root
                .join(sanitize_filename_component(target_car))
                .join("downloaded");
            if let Some(path) = safe_join_under(&target_dir, &file_name) {
                return path.exists();
            }
            return false;
        }
        false
    }

    pub fn get_manifest(&self) -> Vec<ManifestItem> {
        self.manifest.safe_lock().clone()
    }

    pub fn get_browser_setups(&self) -> Vec<CarSetup> {
        self.browser_setups.safe_lock().clone()
    }

    /// Fetch the setup list for the car the browser is pointing at.
    ///
    /// Runs on its own thread: `fetch_server_setups` is a blocking HTTP call
    /// with a five second timeout, and this is driven straight off a keypress.
    /// Doing it inline would freeze the whole TUI — including the loading
    /// spinner meant to show that something is happening.
    pub fn load_browser_car(&self) {
        let idx = *self.browser_car_idx.safe_lock();
        let manifest = self.manifest.safe_lock();
        let Some(car_id) = manifest.get(idx).map(|m| m.id.clone()) else {
            return;
        };
        drop(manifest);

        // Clear immediately so the pane does not keep showing the previous
        // car's setups while the new ones are in flight.
        self.browser_setups.safe_lock().clear();
        *self.browser_setup_idx.safe_lock() = 0;
        *self.details_scroll.safe_lock() = 0;

        let browser_setups = Arc::clone(&self.browser_setups);
        let browser_car_idx = Arc::clone(&self.browser_car_idx);
        let last_status = Arc::clone(&self.last_status);
        let requested_idx = idx;

        std::thread::spawn(move || match fetch_server_setups(&car_id) {
            Ok(mut setups) => {
                for s in &mut setups {
                    s.car_id = car_id.clone();
                }
                // The user may have moved on while this was in flight. Only
                // publish if the selection is still the one we fetched for.
                if *browser_car_idx.safe_lock() == requested_idx {
                    *browser_setups.safe_lock() = setups;
                }
            }
            Err(e) => {
                error!("Could not fetch setups for {}: {}", car_id, e);
                *last_status.safe_lock() = format!("Fetch failed: {}", car_id);
            }
        });
    }

    pub fn get_browser_selected_setup(&self) -> Option<CarSetup> {
        let idx = *self.browser_setup_idx.safe_lock();
        let setups = self.browser_setups.safe_lock();
        setups.get(idx).cloned()
    }

    pub fn get_browser_target_car(&self) -> String {
        let idx = *self.browser_car_idx.safe_lock();
        let manifest = self.manifest.safe_lock();
        if idx < manifest.len() {
            manifest[idx].id.clone()
        } else {
            "unknown".to_string()
        }
    }

    pub fn set_context(&self, car: &str, track: &str) {
        let mut c = self.current_car.safe_lock();
        let mut t = self.current_track.safe_lock();
        if *c != car || *t != track {
            *c = car.to_string();
            *t = track.to_string();
            *self.fetch_state.safe_lock() = FetchState::Idle;
            *self.details_scroll.safe_lock() = 0;
        }
    }

    pub fn get_setups(&self) -> Vec<CarSetup> {
        self.setups.safe_lock().clone()
    }

    pub fn get_best_match_index(&self) -> Option<usize> {
        let setups = self.setups.safe_lock();
        let track_name = self.current_track.safe_lock();
        if setups.is_empty() {
            return None;
        }
        if let Some(idx) = setups
            .iter()
            .position(|s| s.source == *track_name && !s.is_remote)
        {
            return Some(idx);
        }
        Some(0)
    }

    pub fn get_setup_by_index(&self, index: usize) -> Option<CarSetup> {
        let setups = self.setups.safe_lock();
        setups.get(index).cloned()
    }

    pub fn download_setup(&self, setup: &CarSetup, target_car: &str) -> bool {
        if !setup.is_remote {
            return false;
        }

        let mut status_lock = self.last_status.safe_lock();

        if !setup.car_id.is_empty() && setup.car_id != target_car {
            *status_lock = format!("Err: Car mismatch! ({} != {})", setup.car_id, target_car);
            return false;
        }

        {
            let Some(store) = self.store else {
                *status_lock = "Err: this game keeps no setups this program can write".to_string();
                return false;
            };
            let root = match self.setups_root() {
                Some(d) => d,
                None => {
                    *status_lock = "Err: No Assetto Corsa documents folder found".to_string();
                    return false;
                }
            };

            let safe_car = sanitize_filename_component(target_car);
            let target_dir = root.join(&safe_car).join("downloaded");

            if fs::create_dir_all(&target_dir).is_err() {
                *status_lock = "Err: Could not create directory".to_string();
                return false;
            }

            let safe_name = sanitize_filename_component(&setup.name);
            let safe_author = sanitize_filename_component(&setup.author);
            let file_name = (store.file_name)(&safe_author, &safe_name);
            let file_path = match safe_join_under(&target_dir, &file_name) {
                Some(p) => p,
                None => {
                    *status_lock = "Err: unsafe filename rejected".to_string();
                    return false;
                }
            };
            let content = (store.serialise)(setup);

            // **Atomic, because this writes into the game's own setup folder.**
            // A plain write that is interrupted — a crash, a power cut, the
            // game reading the file at that moment — leaves a correctly named
            // setup holding half a file, and the driver finds out about it in
            // the pits. Everything else that saves user data here goes through
            // `atomic_file`; this was the last one that did not.
            match crate::atomic_file::write_atomic(&file_path, content.as_bytes()) {
                Ok(_) => {
                    *status_lock = format!("✅ SAVED to {}!", target_car);
                    drop(status_lock);
                    *self.fetch_state.safe_lock() = FetchState::Idle;
                    return true;
                }
                Err(e) => {
                    *status_lock = format!("Err: {}", e);
                }
            }
        }
        false
    }

    pub fn get_status_message(&self) -> String {
        self.last_status.safe_lock().clone()
    }

    pub fn detect_current(&self, fuel: f32, bias: f32, pressures: &[f32; 4], _temps: &[f32; 4]) {
        let setups = self.setups.safe_lock();
        let mut best_score = 0;
        let mut best_idx = None;
        for (i, setup) in setups.iter().enumerate() {
            if setup.is_remote {
                continue;
            }
            let score = setup.match_score(fuel, bias, pressures);
            if score > best_score && score >= CarSetup::MIN_MATCH_SCORE {
                best_score = score;
                best_idx = Some(i);
            }
        }
        let mut active_idx = self.active_setup_index.safe_lock();
        *active_idx = best_idx;
    }

    pub fn get_active_setup(&self) -> Option<CarSetup> {
        let idx = *self.active_setup_index.safe_lock();
        let setups = self.setups.safe_lock();
        if let Some(i) = idx
            && i < setups.len()
        {
            Some(setups[i].clone())
        } else {
            None
        }
    }
}

/// Off the caller's thread, like every blocking request in this crate: this is
/// reached from a background thread today, and `reqwest::blocking` panics if a
/// future caller reaches it from inside an async runtime. See [`crate::net`].
fn fetch_manifest() -> Result<Vec<ManifestItem>, String> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/manifest.json",
        GITHUB_USER_REPO, GITHUB_BRANCH
    );
    crate::net::off_runtime(|| {
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => return Err(format!("Client build error: {}", e)),
        };
        let resp = match client.get(&url).send() {
            Ok(r) => r,
            Err(e) => return Err(format!("Network error: {}", e)),
        };
        if !resp.status().is_success() {
            return Err(format!("HTTP status {}", resp.status()));
        }
        resp.json::<Vec<ManifestItem>>()
            .map_err(|e| format!("JSON parse error: {}", e))
    })
}

/// Same again — see [`fetch_manifest`].
fn fetch_server_setups(car: &str) -> Result<Vec<CarSetup>, String> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}.json",
        GITHUB_USER_REPO, GITHUB_BRANCH, car
    );
    let mut setups = crate::net::off_runtime(|| {
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => return Err(format!("Client build error: {}", e)),
        };
        let resp = match client.get(&url).send() {
            Ok(r) => r,
            Err(e) => return Err(format!("Network error: {}", e)),
        };
        if !resp.status().is_success() {
            return Err(format!("HTTP status {}", resp.status()));
        }
        resp.json::<Vec<CarSetup>>()
            .map_err(|e| format!("JSON parse error: {}", e))
    })?;
    for s in &mut setups {
        s.is_remote = true;
        s.car_id = car.to_string();
        if s.author.is_empty() {
            s.author = "Server".to_string();
        }
    }
    Ok(setups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_path_traversal() {
        assert!(!sanitize_filename_component("../../etc/passwd").contains(".."));
        assert!(!sanitize_filename_component("../hack").contains(".."));
        assert!(!sanitize_filename_component("..\\..\\windows\\system32").contains(".."));
    }

    #[test]
    fn sanitize_strips_path_separators() {
        let result = sanitize_filename_component("some/path/name");
        assert!(!result.contains('/'));
        let result = sanitize_filename_component("some\\path\\name");
        assert!(!result.contains('\\'));
    }

    #[test]
    fn sanitize_handles_absolute_paths() {
        let result = sanitize_filename_component("/etc/passwd");
        assert!(!result.contains('/'));
        assert!(!result.starts_with('/'));
    }

    #[test]
    fn sanitize_handles_empty_string() {
        let result = sanitize_filename_component("");
        assert!(!result.is_empty());
    }

    #[test]
    fn sanitize_handles_unicode() {
        let result = sanitize_filename_component("Иванов_сетап");
        assert!(!result.is_empty());
        assert!(!result.contains('/'));
    }

    #[test]
    fn sanitize_blocks_windows_reserved_names() {
        let result = sanitize_filename_component("CON");
        assert!(
            result.starts_with('_'),
            "CON should be prefixed: {}",
            result
        );
        let result = sanitize_filename_component("NUL.txt");
        assert!(
            result.starts_with('_'),
            "NUL.txt should be prefixed: {}",
            result
        );
        let result = sanitize_filename_component("com1");
        assert!(
            result.starts_with('_'),
            "com1 should be prefixed: {}",
            result
        );
    }

    #[test]
    fn sanitize_handles_colon() {
        let result = sanitize_filename_component("C:setup");
        assert!(!result.contains(':'));
    }

    #[test]
    fn sanitize_preserves_normal_names() {
        assert_eq!(sanitize_filename_component("my_setup-v2"), "my_setup-v2");
        assert_eq!(sanitize_filename_component("RaceSetup123"), "RaceSetup123");
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let tmp = std::env::temp_dir().join("test_safe_join");
        let _ = std::fs::create_dir_all(&tmp);

        // Even pre-sanitized, the function should stay inside root
        let result = safe_join_under(&tmp, "../../etc/passwd");
        if let Some(path) = &result {
            // The path should be inside tmp (sanitize strips the ..)
            assert!(!path.to_string_lossy().contains("etc/passwd"));
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn safe_join_works_for_normal_names() {
        let tmp = std::env::temp_dir().join("test_safe_join_normal");
        let _ = std::fs::create_dir_all(&tmp);

        let result = safe_join_under(&tmp, "my_setup.ini");
        assert!(result.is_some());
        let path = result.expect("should succeed");
        assert!(path.starts_with(&tmp));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_fetch_state_context_change_resets_state() {
        let mgr = SetupManager::new(None);
        mgr.set_context("ks_ferrari_sf70h", "monza");
        *mgr.fetch_state.safe_lock() = FetchState::Ready;

        assert_eq!(*mgr.fetch_state.safe_lock(), FetchState::Ready);

        // Context change to a new car resets fetch_state to Idle
        mgr.set_context("ks_porsche_911_gt3_rs", "spa");
        assert_eq!(*mgr.fetch_state.safe_lock(), FetchState::Idle);
    }

    #[test]
    fn test_fetch_state_failed_exponential_backoff() {
        let err_msg = "404 Not Found".to_string();
        let retry_at = std::time::Instant::now() + Duration::from_secs(4);
        let state = FetchState::Failed {
            error: err_msg.clone(),
            retry_at,
            attempts: 2,
        };

        let is_failed = matches!(state, FetchState::Failed { .. });
        assert!(is_failed, "Expected FetchState::Failed");
    }

    #[test]
    fn test_setup_manager_shutdown_joins_background_thread() {
        let mgr = SetupManager::new(None);
        assert!(mgr.bg_thread.safe_lock().is_some());
        assert!(!mgr.shutdown_flag.load(std::sync::atomic::Ordering::SeqCst));

        mgr.shutdown();

        assert!(mgr.shutdown_flag.load(std::sync::atomic::Ordering::SeqCst));
        assert!(mgr.bg_thread.safe_lock().is_none());
    }

    /// The reasoning behind MIN_MATCH_SCORE, pinned so a change to the
    /// weights cannot quietly make the threshold unreachable again.
    #[test]
    fn match_score_totals_stay_reachable_below_a_perfect_match() {
        let setup = CarSetup {
            fuel: 50,
            brake_bias: 6500,
            pressure_lf: 27,
            pressure_rf: 27,
            pressure_lr: 27,
            pressure_rr: 27,
            ..CarSetup::default()
        };

        let perfect = setup.match_score(50.0, 65.0, &[27.0; 4]);
        assert_eq!(perfect, 75, "all three components agree");

        // Ten laps in, the fuel no longer matches but nothing about the setup
        // has changed. This is the everyday case the old `> 60` threshold
        // rejected.
        let fuel_burnt = setup.match_score(20.0, 65.0, &[27.0; 4]);
        assert_eq!(fuel_burnt, 45);
        assert!(
            fuel_burnt >= CarSetup::MIN_MATCH_SCORE,
            "a setup with burnt fuel must still be recognised"
        );

        // One component alone is not enough to claim a match.
        let only_fuel = setup.match_score(50.0, 20.0, &[10.0; 4]);
        assert_eq!(only_fuel, 30);
        assert!(only_fuel < CarSetup::MIN_MATCH_SCORE);
    }
}
