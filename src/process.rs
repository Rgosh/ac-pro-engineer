use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use std::mem::size_of;

pub fn is_process_running(target_name: &str) -> bool {
    unsafe {
        let snapshot: HANDLE = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };

        if snapshot == INVALID_HANDLE_VALUE { return false; }

        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry.szExeFile.iter().position(|&x| x == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);

                if name.eq_ignore_ascii_case(target_name) {
                    let _ = CloseHandle(snapshot);
                    return true;
                }
                if Process32NextW(snapshot, &mut entry).is_err() { break; }
            }
        }
        let _ = CloseHandle(snapshot);
        false
    }
}

pub fn get_process_id(process_name: &str) -> Option<u32> {
    unsafe {
        let snapshot: HANDLE = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return None,
        };

        if snapshot == INVALID_HANDLE_VALUE { return None; }

        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry.szExeFile.iter().position(|&x| x == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);

                if name.eq_ignore_ascii_case(process_name) {
                    let _ = CloseHandle(snapshot);
                    return Some(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry).is_err() { break; }
            }
        }
        let _ = CloseHandle(snapshot);
        None
    }
}