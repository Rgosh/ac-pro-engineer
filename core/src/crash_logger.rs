use std::fs::{self, File};
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::time::SystemTime;

pub fn init_crash_handler() {
    panic::set_hook(Box::new(|info| {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "Unknown location".to_string());

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };

        let report = format!(
            "====================================================\n\
             AC PRO ENGINEER - CRASH DIAGNOSTIC REPORT\n\
             ====================================================\n\
             Timestamp:  {}\n\
             OS Target:  {}\n\
             Location:   {}\n\
             Message:    {}\n\
             ====================================================\n\
             Backtrace:\n{:?}\n\
             ====================================================\n",
            timestamp,
            std::env::consts::OS,
            location,
            payload,
            std::backtrace::Backtrace::force_capture()
        );

        eprintln!("\nCRASH DETECTED! Writing report to disk...\n{}", report);

        let mut log_dir = PathBuf::from("logs");
        if !log_dir.exists() {
            drop(fs::create_dir_all(&log_dir));
        }
        if !log_dir.exists() {
            log_dir = PathBuf::from(".");
        }

        let file_path = log_dir.join(format!("crash_report_{}.log", timestamp));
        if let Ok(mut f) = File::create(&file_path) {
            drop(f.write_all(report.as_bytes()));
        }
    }));
}
