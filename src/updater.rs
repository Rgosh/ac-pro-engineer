use serde::Deserialize;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::process::Command;
use std::env;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

// IP внутри кода останется (программа должна знать, куда стучаться),
// но мы скроем его отображение в интерфейсе.
const VERSION_URL: &str = "http://93.115.21.32:5555/version.json"; 

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
        *status.lock().unwrap() = UpdateStatus::Checking;

        thread::spawn(move || {
            // Убрали println!, который выводил IP в консоль
            thread::sleep(std::time::Duration::from_millis(500));

            match reqwest::blocking::get(VERSION_URL) {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        // Вместо кода ошибки показываем общее сообщение
                        *status.lock().unwrap() = UpdateStatus::Error("Server unavailable".to_string());
                        return;
                    }

                    match resp.json::<RemoteVersion>() {
                        Ok(info) => {
                            let mut state = status.lock().unwrap();
                            if info.version != CURRENT_VERSION {
                                *state = UpdateStatus::UpdateAvailable(info);
                            } else {
                                *state = UpdateStatus::NoUpdate;
                            }
                        },
                        Err(_) => {
                            // Скрываем детали ошибки парсинга
                            *status.lock().unwrap() = UpdateStatus::Error("Invalid server response".to_string());
                        }
                    }
                }
                Err(_) => {
                    // ГЛАВНОЕ ИСПРАВЛЕНИЕ:
                    // Раньше тут было `format!("Connect Fail: {}", e)`, и `e` содержало IP.
                    // Теперь пишем просто "Connection failed".
                    *status.lock().unwrap() = UpdateStatus::Error("Connection failed".to_string());
                }
            }
        });
    }

    pub fn download_update(&self, info: RemoteVersion) {
        let status = self.status.clone();
        thread::spawn(move || {
            *status.lock().unwrap() = UpdateStatus::Downloading(0.0);
            let file_name = "ac_pro_engineer_new.exe";
            
            match reqwest::blocking::get(&info.url) {
                Ok(mut resp) => {
                    if !resp.status().is_success() {
                        *status.lock().unwrap() = UpdateStatus::Error("Download failed".to_string());
                        return;
                    }
                    let total_size = resp.content_length().unwrap_or(0);
                    match File::create(file_name) {
                        Ok(mut file) => {
                            let mut buffer = [0; 8192];
                            let mut downloaded: u64 = 0;
                            loop {
                                match resp.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if file.write_all(&buffer[..n]).is_err() { 
                                            *status.lock().unwrap() = UpdateStatus::Error("Disk write error".to_string());
                                            return; 
                                        }
                                        downloaded += n as u64;
                                        if total_size > 0 {
                                            let pct = (downloaded as f32 / total_size as f32) * 100.0;
                                            *status.lock().unwrap() = UpdateStatus::Downloading(pct);
                                        }
                                    }
                                    Err(_) => {
                                        *status.lock().unwrap() = UpdateStatus::Error("Download interrupted".to_string());
                                        return;
                                    }
                                }
                            }
                            *status.lock().unwrap() = UpdateStatus::Downloaded(file_name.to_string());
                        },
                        Err(_) => { *status.lock().unwrap() = UpdateStatus::Error("File access error".to_string()); }
                    }
                }
                Err(_) => { *status.lock().unwrap() = UpdateStatus::Error("Connection lost".to_string()); }
            }
        });
    }

    pub fn restart_and_apply(&self, new_file_name: &str) {
        let current_exe = env::current_exe().unwrap();
        let current_exe_name = current_exe.file_name().unwrap().to_str().unwrap();
        let current_dir = env::current_dir().unwrap();

        let script = format!(
            "@echo off\r\n\
             chcp 65001 >nul\r\n\
             timeout /t 2 /nobreak > NUL\r\n\
             del \"{0}\"\r\n\
             move \"{1}\" \"{0}\"\r\n\
             start \"\" \"{0}\"\r\n\
             del \"%~f0\"\r\n\
             exit",
            current_exe_name,
            new_file_name
        );

        if let Ok(mut file) = File::create("updater.bat") {
            let _ = file.write_all(script.as_bytes());
            drop(file);
            let _ = Command::new("cmd")
                .args(["/C", "start", "updater.bat"])
                .current_dir(&current_dir)
                .spawn();
            std::process::exit(0);
        }
    }
}