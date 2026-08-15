//! Take the bytes a simulator is publishing right now, ready to paste into a
//! layout test.
//!
//! This is the tool the second game needs before a line of its code is
//! written. `tests_suite/src/shm_layout_tests.rs` pins Assetto Corsa's offsets
//! against bytes captured verbatim from a live mapping, and that is the only
//! reason the ACC-shaped graphics struct was ever caught — every other test in
//! the workspace builds a value in Rust and reads it back, so it round-trips
//! through whatever layout the struct happens to declare and cannot detect a
//! mismatch with the game at all.
//!
//! So: run the game, run this, and paste what comes out.
//!
//! ```text
//! cargo run -p ac_core --example capture_pages
//! cargo run -p ac_core --example capture_pages -- acpmf_physics acpmf_graphics
//! ```
//!
//! # Taking a capture that is worth having
//!
//! The existing Assetto Corsa capture cannot speak for the last fifteen fields
//! of its graphics page, because everything past offset 300 was zero when it
//! was taken. A page of zeros pins nothing: every wrong offset also reads
//! zero. This tool says how far into each page the last non-zero byte is, so
//! that is known before the game is closed rather than a year later.
//!
//! To get a capture that settles a whole page:
//!
//! * be **on track and past the first lap** — several fields stay zero until a
//!   lap has been completed, `fuel_x_lap` among them;
//! * have the systems **on and not at their defaults** — traction control and
//!   ABS set to something other than zero, headlights on, a pit limiter used;
//! * take it **mid-lap**, not in the pits, so speeds, temperatures and
//!   positions are all non-zero;
//! * take the static page too, which only settles once a session has loaded.
//!
//! # On Linux
//!
//! The game is a Windows process under Proton and publishes into the prefix,
//! so `shm-bridge.exe` has to be running to mirror the pages into `/dev/shm`.
//! The bridge maps 2048 bytes per page, which is larger than any page Assetto
//! Corsa or Competizione writes — but it is the bridge's number, not the
//! game's, so **the length printed here is the mapping's, not necessarily the
//! struct's.** The trailing-zero count is what says where the game stopped
//! writing.

use std::fmt::Write as _;

/// The pages Assetto Corsa and Competizione both publish, under the same
/// names. Which is exactly why a second game needs a discriminator: the names
/// match and the layouts do not.
const DEFAULT_PAGES: &[&str] = &["acpmf_physics", "acpmf_graphics", "acpmf_static"];

/// How many bytes to map on Windows, where a mapping does not carry its size
/// the way a file on a tmpfs does.
#[cfg(target_os = "windows")]
const WINDOWS_MAP_BYTES: usize = 4096;

fn main() {
    let names: Vec<String> = std::env::args().skip(1).collect();
    let names: Vec<&str> = if names.is_empty() {
        DEFAULT_PAGES.to_vec()
    } else {
        names.iter().map(String::as_str).collect()
    };

    let mut any = false;
    for name in &names {
        match read_page(name) {
            Some(bytes) => {
                any = true;
                report(name, &bytes);
            }
            None => {
                println!("{name}: not published — is the game running?");
                #[cfg(not(target_os = "windows"))]
                println!("  On Linux the bridge has to be running too: bridge_probe says.");
                println!();
            }
        }
    }

    if !any {
        println!("Nothing captured. Nothing here is a failure — a game that is not");
        println!("running publishes nothing, which is the normal state.");
    }
}

#[cfg(not(target_os = "windows"))]
fn read_page(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("/dev/shm/{name}")).ok()
}

#[cfg(target_os = "windows")]
fn read_page(name: &str) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Memory::{FILE_MAP_READ, MapViewOfFile, OpenFileMappingW};
    use windows::core::HSTRING;

    let full = format!("Local\\{name}");
    unsafe {
        let handle = OpenFileMappingW(FILE_MAP_READ.0, false, &HSTRING::from(full)).ok()?;
        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, WINDOWS_MAP_BYTES);
        let ptr = view.Value as *const u8;
        if ptr.is_null() {
            let _ = CloseHandle(handle);
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr, WINDOWS_MAP_BYTES).to_vec();
        let _ = CloseHandle(handle);
        Some(bytes)
    }
}

/// The last byte the game actually wrote something into.
///
/// Everything past it is either padding or a field the session never filled,
/// and a capture cannot tell those apart — which is why this is printed rather
/// than assumed.
fn last_non_zero(bytes: &[u8]) -> Option<usize> {
    bytes.iter().rposition(|byte| *byte != 0)
}

fn report(name: &str, bytes: &[u8]) {
    println!("── {name} ─────────────────────────────────────────");
    println!("mapped {} bytes", bytes.len());

    match last_non_zero(bytes) {
        Some(last) => {
            let tail = bytes.len() - last - 1;
            println!("last non-zero byte at offset {last}, {tail} zero bytes after it");
            if tail > 64 {
                println!(
                    "  ⚠ this capture pins nothing past offset {last}: a wrong offset \
                     in that tail also reads zero."
                );
                println!(
                    "    Drive a lap, turn TC/ABS and the lights on, and take it again \
                     mid-lap."
                );
            }
        }
        None => {
            println!("  ⚠ the whole page is zero. The mapping exists and the game has not");
            println!("    written to it — on Linux that is usually the bridge running");
            println!("    before the game has published.");
        }
    }

    println!();
    println!("Paste into tests_suite/src/shm_layout_tests.rs:");
    println!();
    println!("const {}_PAGE_HEX: &str = concat!(", const_name(name));
    print!("{}", hex_block(bytes));
    println!(");");
    println!();
}

/// `acpmf_graphics` becomes `GRAPHICS`, which is what the existing constants
/// are called.
fn const_name(name: &str) -> String {
    name.strip_prefix("acpmf_")
        .unwrap_or(name)
        .to_uppercase()
        .replace(['-', '.'], "_")
}

/// Thirty-two bytes a line, in the shape the layout tests already use.
fn hex_block(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(32) {
        let mut line = String::with_capacity(64);
        for byte in chunk {
            let _ = write!(line, "{byte:02x}");
        }
        let _ = writeln!(out, "    \"{line}\",");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_of_zeros_has_no_last_byte() {
        assert_eq!(last_non_zero(&[0u8; 64]), None);
    }

    #[test]
    fn the_last_written_byte_is_found_past_a_run_of_zeros() {
        let mut page = vec![0u8; 64];
        page[9] = 7;
        assert_eq!(last_non_zero(&page), Some(9));
    }

    /// The hex has to be pasteable as-is, which means the same 32 bytes a line
    /// the existing captures use.
    #[test]
    fn the_hex_is_thirty_two_bytes_a_line() {
        let block = hex_block(&[0xabu8; 70]);
        let lines: Vec<&str> = block.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "    \"".to_string() + &"ab".repeat(32) + "\",");
        assert_eq!(lines[2], "    \"".to_string() + &"ab".repeat(6) + "\",");
    }

    #[test]
    fn the_constant_is_named_after_the_page() {
        assert_eq!(const_name("acpmf_graphics"), "GRAPHICS");
        assert_eq!(const_name("something_else"), "SOMETHING_ELSE");
    }
}
