//! Read the engineer's feed from a program of your own — the whole of it.
//!
//! ```bash
//! cargo run -p ac_core --example feed
//! ```
//!
//! Subscribes to Pro Engineer on this machine (nothing to configure there:
//! it answers on `broadcast::subscribe::PORT` while it runs), keeps the
//! subscription alive, and prints a line per message. Start it before or
//! after the program; it finds it either way. With an address it asks
//! another machine on the network, whose driver has to have allowed that:
//!
//! ```bash
//! cargo run -p ac_core --example feed -- 192.168.1.42:9003
//! ```
//!
//! No game to hand? `feed_demo` answers the same way with made-up numbers.

use ac_core::broadcast::subscribe::{Client, PORT, Standing};
use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let server: SocketAddr = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], PORT)));
    let mut feed = Client::to(server, 10.0, "feed example")?;
    let mut out = std::io::stdout().lock();
    writeln!(out, "asking {server} for the feed…")?;
    let mut said = feed.standing().clone();
    loop {
        if let Some(message) = feed.poll() {
            let advice = message
                .advice
                .first()
                .map(|line| line.text.as_str())
                .unwrap_or("—");
            // `?`, so piping into `head` ends the program rather than panicking.
            writeln!(
                out,
                "{:>5.0} km/h  gear {:>2}  lap {:>3}  fuel {:>5.1} l  {:+.3} s  | {advice}",
                message.speed_kmh,
                message.gear,
                message.lap_count,
                message.fuel_litres,
                message.delta_seconds,
            )?;
        }
        if feed.standing() != &said {
            said = feed.standing().clone();
            match &said {
                Standing::Subscribed { hz } => writeln!(out, "subscribed, {hz} a second")?,
                Standing::Refused(why) => writeln!(out, "refused: {why}")?,
                Standing::Asking => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
