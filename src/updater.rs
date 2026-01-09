use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

const GITHUB_OWNER: &str = "Rgosh";
const GITHUB_REPO: &str = "ac-pro-engineer";
const BINARY_NAME: &str = "ac_pro_engineer.exe";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpdateAvailable(RemoteVersion),
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
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Clone)]
pub struct Updater {
    pub status: Arc<Mutex<UpdateStatus>>,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
        }
    }

    pub fn check_for_updates(&self) {
        let status = self.status.clone();
        {
            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
            *lock = UpdateStatus::Checking;
        }

        thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .user_agent("AC-Pro-Engineer-Updater")
                .build()
                .unwrap_or_default();

            let url = format!(
                "https://api.github.com/repos/{}/{}/releases/latest",
                GITHUB_OWNER, GITHUB_REPO
            );

            match client.get(&url).send() {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                        *lock = UpdateStatus::Error(format!("GitHub API error: {}", resp.status()));
                        return;
                    }

                    match resp.json::<GitHubRelease>() {
                        Ok(release) => {
                            let remote_ver_str = release.tag_name.trim_start_matches('v');
                            let asset = release.assets.iter().find(|a| a.name == BINARY_NAME);

                            if let Some(asset) = asset {
                                let mut state = status.lock().unwrap_or_else(|e| e.into_inner());
                                if remote_ver_str != CURRENT_VERSION {
                                    *state = UpdateStatus::UpdateAvailable(RemoteVersion {
                                        version: remote_ver_str.to_string(),
                                        url: asset.browser_download_url.clone(),
                                        notes: release.body.unwrap_or_default(),
                                    });
                                } else {
                                    *state = UpdateStatus::NoUpdate;
                                }
                            } else {
                                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                                *lock = UpdateStatus::Error("Asset not found".to_string());
                            }
                        }
                        Err(e) => {
                            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                            *lock = UpdateStatus::Error(format!("Parse error: {}", e));
                        }
                    }
                }
                Err(e) => {
                    let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                    *lock = UpdateStatus::Error(format!("Network error: {}", e));
                }
            }
        });
    }

    pub fn download_update(&self, info: RemoteVersion) {
        let status = self.status.clone();
        thread::spawn(move || {
            {
                let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                *lock = UpdateStatus::Downloading(0.0);
            }

            let current_exe =
                env::current_exe().unwrap_or_else(|_| PathBuf::from("ac_pro_engineer.exe"));
            let exe_dir = current_exe
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            let file_path = exe_dir.join("ac_pro_engineer_new.exe");
            let file_name_str = file_path
                .to_str()
                .unwrap_or("ac_pro_engineer_new.exe")
                .to_string();

            let client = reqwest::blocking::Client::builder()
                .user_agent("AC-Pro-Engineer-Updater")
                .build()
                .unwrap_or_default();

            match client.get(&info.url).send() {
                Ok(mut resp) => {
                    if !resp.status().is_success() {
                        let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                        *lock = UpdateStatus::Error("Download failed".to_string());
                        return;
                    }
                    let total_size = resp.content_length().unwrap_or(0);
                    match File::create(&file_path) {
                        Ok(mut file) => {
                            let mut buffer = [0; 8192];
                            let mut downloaded: u64 = 0;
                            loop {
                                match resp.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if file.write_all(&buffer[..n]).is_err() {
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
                                    Err(_) => return,
                                }
                            }
                            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                            *lock = UpdateStatus::Downloaded(file_name_str);
                        }
                        Err(_) => {
                            let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                            *lock = UpdateStatus::Error("File access error".to_string());
                        }
                    }
                }
                Err(_) => {
                    let mut lock = status.lock().unwrap_or_else(|e| e.into_inner());
                    *lock = UpdateStatus::Error("Connection lost".to_string());
                }
            }
        });
    }

    pub fn restart_and_apply(&self, _new_file_name: &str) {
        let current_exe =
            env::current_exe().unwrap_or_else(|_| PathBuf::from("ac_pro_engineer.exe"));
        let exe_dir = current_exe
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        let exe_path = current_exe.to_str().unwrap_or("ac_pro_engineer.exe");
        let new_exe = exe_dir.join("ac_pro_engineer_new.exe");
        let new_exe_str = new_exe.to_str().unwrap_or("ac_pro_engineer_new.exe");
        let bat_path = exe_dir.join("updater.bat");

        let script = format!(
            "@echo off\r\n\
             chcp 65001 >nul\r\n\
             :wait_close\r\n\
             timeout /t 1 /nobreak > NUL\r\n\
             del \"{0}\" >nul 2>&1\r\n\
             if exist \"{0}\" goto wait_close\r\n\
             move /y \"{1}\" \"{0}\" >nul\r\n\
             start \"\" \"{0}\"\r\n\
             (goto) 2>nul & del \"%~f0\"\r\n\
             exit",
            exe_path, new_exe_str
        );

        if let Ok(mut file) = File::create(&bat_path) {
            let _res = file.write_all(script.as_bytes());
            drop(file);

            let _child = Command::new("cmd")
                .args(["/C", "start", "/MIN", "updater.bat"])
                .current_dir(exe_dir)
                .spawn();

            std::process::exit(0);
        }
    }
}
