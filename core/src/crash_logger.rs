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

        // Somewhere writable, in preference order. The old code used "logs"
        // relative to the working directory and fell back to "." — both of
        // which are unwritable when the app is launched from a shortcut, from
        // Explorer, or installed under Program Files. The report was then
        // dropped in silence, which is the worst possible outcome for a crash
        // report: the user is asked to send one that was never written.
        let candidates = [
            crate::config::app_dir().join("logs"),
            PathBuf::from("logs"),
            PathBuf::from("."),
        ];

        let mut written = false;
        for dir in candidates {
            drop(fs::create_dir_all(&dir));
            let file_path = dir.join(format!("crash_report_{}.log", timestamp));
            if let Ok(mut f) = File::create(&file_path)
                && f.write_all(report.as_bytes()).is_ok()
            {
                eprintln!("Crash report written to {}", file_path.display());
                written = true;
                break;
            }
        }

        if !written {
            eprintln!("Could not write a crash report anywhere; the trace above is all there is.");
        }
    }));
}
