use serde::Deserialize;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::process::Command;
use std::env;
use std::path::PathBuf;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
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
        Self { status: Arc::new(Mutex::new(UpdateStatus::Idle)) }
    }

    pub fn check_for_updates(&self) {
        let status = self.status.clone();
        *status.lock().unwrap() = UpdateStatus::Checking;
        thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(500));
            match reqwest::blocking::get(VERSION_URL) {
                Ok(resp) => {
                    if !resp.status().is_success() {
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
                        Err(_) => { *status.lock().unwrap() = UpdateStatus::Error("Invalid response".to_string()); }
                    }
                }
                Err(_) => { *status.lock().unwrap() = UpdateStatus::Error("Connection failed".to_string()); }
            }
        });
    }

    pub fn download_update(&self, info: RemoteVersion) {
        let status = self.status.clone();
        thread::spawn(move || {
            *status.lock().unwrap() = UpdateStatus::Downloading(0.0);
            
          
            let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("ac_pro_engineer.exe"));
            let exe_dir = current_exe.parent().unwrap_or_else(|| std::path::Path::new("."));
            let file_path = exe_dir.join("ac_pro_engineer_new.exe");
            let file_name_str = file_path.to_str().unwrap().to_string();

            match reqwest::blocking::get(&info.url) {
                Ok(mut resp) => {
                    if !resp.status().is_success() {
                        *status.lock().unwrap() = UpdateStatus::Error("Download failed".to_string());
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
                                            *status.lock().unwrap() = UpdateStatus::Error("Write error".to_string());
                                            return; 
                                        }
                                        downloaded += n as u64;
                                        if total_size > 0 {
                                            let pct = (downloaded as f32 / total_size as f32) * 100.0;
                                            *status.lock().unwrap() = UpdateStatus::Downloading(pct);
                                        }
                                    }
                                    Err(_) => { return; }
                                }
                            }
                            *status.lock().unwrap() = UpdateStatus::Downloaded(file_name_str);
                        },
                        Err(_) => { *status.lock().unwrap() = UpdateStatus::Error("File access error".to_string()); }
                    }
                }
                Err(_) => { *status.lock().unwrap() = UpdateStatus::Error("Connection lost".to_string()); }
            }
        });
    }

    pub fn restart_and_apply(&self, _new_file_name: &str) {
        let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("ac_pro_engineer.exe"));
        let exe_dir = current_exe.parent().unwrap_or_else(|| std::path::Path::new("."));
        
        let exe_path = current_exe.to_str().unwrap_or("ac_pro_engineer.exe");
 
        let new_exe = exe_dir.join("ac_pro_engineer_new.exe");
        let new_exe_str = new_exe.to_str().unwrap();
        
        let bat_path = exe_dir.join("updater.bat");


        let script = format!(
            "@echo off\r\n\
             chcp 65001 >nul\r\n\
             \r\n\
             :wait_close\r\n\
             timeout /t 1 /nobreak > NUL\r\n\
             \r\n\
             :: Пытаемся удалить старый файл. Если ошибка (занят) - повторяем\r\n\
             del \"{0}\" >nul 2>&1\r\n\
             if exist \"{0}\" goto wait_close\r\n\
             \r\n\
             :: Перемещаем новый файл на место старого\r\n\
             move /y \"{1}\" \"{0}\" >nul\r\n\
             \r\n\
             :: Запускаем обновленную программу\r\n\
             start \"\" \"{0}\"\r\n\
             \r\n\
             :: Удаляем этот батник и выходим\r\n\
             (goto) 2>nul & del \"%~f0\"\r\n\
             exit",
            exe_path,     
            new_exe_str  
        );

        if let Ok(mut file) = File::create(&bat_path) {
            let _ = file.write_all(script.as_bytes());
            drop(file);
            
            
            let _ = Command::new("cmd")
                .args(["/C", "start", "/MIN", "updater.bat"])
                .current_dir(exe_dir)
                .spawn();
                
            std::process::exit(0);
        }
    }
}