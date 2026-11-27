//! The engineer on a stream: a page for OBS's browser source.
//!
//! ```text
//! http://127.0.0.1:7878/
//! ```
//!
//! **Every stream with the engineer on it shows the engineer.** A streamer
//! adds one browser source in OBS — the address above, transparent — and
//! their viewers see what the engineer says on the radio as a subtitle, with
//! the speed, the gear and the delta beside it, and the program's name in
//! the corner. Nothing is sent anywhere: the page is served by this program
//! on this machine, to OBS on this machine.
//!
//! **A sink like any other** ([`super::Sink`]): the tick hands over the frame
//! and never waits. The frame is kept as the same JSON the UDP sink sends;
//! a thread of its own answers OBS's requests — the page once, the frame a
//! few times a second — and the tick never sees a socket.
//!
//! Only the loopback address is bound: a page that showed a driver's
//! telemetry to anybody on the network who guessed the port is not a page
//! anybody asked for.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::Sink;
use crate::overlay::frame::OverlayFrame;

/// Where OBS finds the page unless told otherwise.
pub const DEFAULT_PORT: u16 = 7878;

/// The page OBS draws. Transparent, so only the subtitle and the chip are
/// over the stream; it asks for the frame five times a second.
const PAGE: &str = include_str!("obs.html");

/// Serves the stream overlay and keeps the newest frame for it.
pub struct ObsSink {
    latest: Arc<Mutex<Vec<u8>>>,
    stop: Arc<AtomicBool>,
    address: SocketAddr,
    game: String,
    driver: String,
}

impl ObsSink {
    /// Listen on the loopback address at `port`.
    pub fn start(port: u16, game: &str, driver: &str) -> std::io::Result<ObsSink> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let address = listener.local_addr()?;
        let latest = Arc::new(Mutex::new(b"{}".to_vec()));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, stopped) = (Arc::clone(&latest), Arc::clone(&stop));
        std::thread::Builder::new()
            .name("obs".to_string())
            .spawn(move || serve(&listener, &shared, &stopped))?;
        Ok(ObsSink {
            latest,
            stop,
            address,
            game: game.to_string(),
            driver: driver.to_string(),
        })
    }

    /// The address to give OBS.
    pub fn url(&self) -> String {
        format!("http://{}/", self.address)
    }
}

impl Sink for ObsSink {
    fn name(&self) -> &str {
        "obs"
    }

    fn publish(&mut self, frame: &OverlayFrame) -> std::io::Result<()> {
        let json = serde_json::to_vec(&super::udp::message(frame, &self.game, &self.driver))
            .map_err(std::io::Error::other)?;
        *self.latest.lock().unwrap_or_else(|p| p.into_inner()) = json;
        Ok(())
    }

    /// A viewer cannot tell ten frames a second from sixty, and the page asks
    /// five times a second.
    fn min_interval(&self) -> Duration {
        Duration::from_millis(100)
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // The accept loop is blocked on a connection; this is one.
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(200));
    }
}

impl Drop for ObsSink {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn serve(listener: &TcpListener, latest: &Mutex<Vec<u8>>, stop: &AtomicBool) {
    for stream in listener.incoming() {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let Ok(mut stream) = stream else { continue };
        // Two seconds to say what it wants: OBS asks at once, and a machine
        // busy with the game can be slow to — a request read too early was
        // answered as if it were empty, and the answer arrived before the
        // question had been sent.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        // **Read to the end of the request's head**, not one read's worth: a
        // request may arrive in pieces, and answering the first piece closes
        // the connection under the rest of it.
        let mut request = Vec::with_capacity(512);
        let mut chunk = [0u8; 512];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") && request.len() < 8192 {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => request.extend_from_slice(chunk.get(..n).unwrap_or(&[])),
            }
        }
        if request.is_empty() {
            continue;
        }
        let head = String::from_utf8_lossy(&request);
        let path = head.split_whitespace().nth(1).unwrap_or("/");
        let path = path.split('?').next().unwrap_or("/");
        let (status, kind, body): (&str, &str, Vec<u8>) = match path {
            "/" | "/index.html" => (
                "200 OK",
                "text/html; charset=utf-8",
                PAGE.as_bytes().to_vec(),
            ),
            "/frame.json" => (
                "200 OK",
                "application/json",
                latest.lock().unwrap_or_else(|p| p.into_inner()).clone(),
            ),
            _ => ("404 Not Found", "text/plain", b"not here".to_vec()),
        };
        let header = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::overlay::frame::flags;

    fn get(address: &str, path: &str) -> String {
        let mut stream = TcpStream::connect(address).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out
    }

    /// **OBS gets the page and then the frame**, and the frame carries what
    /// the engineer is saying.
    #[test]
    fn obs_gets_the_page_and_the_line() {
        let mut sink = ObsSink::start(0, "assetto_corsa", "Rgosh").unwrap();
        let address = sink.address.to_string();
        let page = get(&address, "/");
        assert!(page.starts_with("HTTP/1.1 200 OK"));
        assert!(page.contains("frame.json"), "the page asks for the frame");

        let mut frame = OverlayFrame::empty();
        frame.speed_kmh = 212.0;
        frame.set_flag(flags::CONNECTED, true);
        frame.set_radio(Some(("Box this lap.", "fuel 1.2 laps", true)));
        sink.publish(&frame).unwrap();
        let json = get(&address, "/frame.json?t=1");
        assert!(json.contains("\"said\":\"Box this lap.\""), "{json}");
        assert!(json.contains("\"critical\":true"), "{json}");

        assert!(get(&address, "/anything-else").starts_with("HTTP/1.1 404"));
        sink.shutdown();
    }

    /// Nobody on the network can read a driver's telemetry off this.
    #[test]
    fn only_this_machine_can_reach_it() {
        let sink = ObsSink::start(0, "g", "d").unwrap();
        assert!(sink.address.ip().is_loopback());
        assert!(sink.url().starts_with("http://127.0.0.1:"));
    }
}
