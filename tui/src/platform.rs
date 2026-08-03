#[cfg(target_os = "linux")]
pub mod linux;

/// Relaunching into a terminal, for when the app is started from a file
/// manager or a desktop entry rather than from a shell.
///
/// Linux only: on Windows the binary is built as a console subsystem
/// application, so double-clicking it already opens a console window.
#[cfg(target_os = "linux")]
pub mod relaunch;
