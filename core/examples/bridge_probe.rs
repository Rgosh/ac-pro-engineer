//! Print what the application knows about `shm-bridge.exe`.
//!
//! Three pieces have to agree about a frame — the application, the panel, and
//! the bridge — and the bridge was the one that could not be inspected. Every
//! failure that cost an evening looked identical from the driver's seat: the
//! panel saying "waiting for AC Pro Engineer" with `/dev/shm` holding the file,
//! at the right size, with the application running. A bridge built before the
//! frame grew maps too few bytes and CSP silently refuses to open the mapping.
//!
//! ```text
//! cargo run -p ac_core --example bridge_probe
//! ```
//!
//! The report itself lives in `ac_core::overlay::diagnosis`, because the same
//! answer is on a screen in the application now — a driver who downloaded a
//! release cannot run a cargo example, and this was the only thing that could
//! answer their question. This is the same text, for anyone already in a
//! terminal.
//!
//! On Windows there is no bridge — the application creates the named mapping
//! itself — and this says so rather than reporting a missing component.

fn main() {
    println!("AC Pro Engineer — bridge probe");
    print!("{}", ac_core::overlay::diagnosis::report().to_plain_text());
}
