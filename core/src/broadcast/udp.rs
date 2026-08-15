//! The frame as JSON over UDP, for anything that is not the in-game panel.
//!
//! A second front end on the same machine, a friend on another, a relay for a
//! championship — all the same sink with a different address. Shared memory is
//! the right transport for the panel and the wrong one for everything else: it
//! needs a bridge under Proton, it cannot cross a machine, and nothing outside
//! this project knows the byte layout.
//!
//! UDP rather than TCP, and JSON rather than the struct:
//!
//! * **UDP**, because a lost frame costs nothing — another arrives in
//!   milliseconds — and because a subscriber that stops reading must not be
//!   able to block the tick that is also feeding the driver's own overlay. A
//!   TCP send buffer filling up does exactly that.
//! * **JSON**, because the point is that *anything* can subscribe. A byte
//!   layout would mean every client re-implementing `#[repr(C)]` field order,
//!   which is the thing that has cost this project the most evenings. Fifty
//!   lines of Python should be able to read this.
//!
//! Lap and debrief data are in every message rather than sent as events. A
//! dropped event would be gone; a dropped frame is replaced by the next one.

use super::Sink;
use crate::overlay::frame::{DEBRIEF_LAPS, DEBRIEF_LINES, MESSAGE_SLOTS, OverlayFrame};
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// What every datagram starts with, so a receiver on a shared port can tell
/// ours from somebody else's without parsing further.
pub const MAGIC: &str = "acpe";

/// Schema version, carried in every message.
///
/// Its own number, not the frame's: a receiver written against this JSON does
/// not care that the struct behind it grew a field, and should not be told to
/// upgrade because the panel's wire format moved. It changes when a *key*
/// changes meaning or disappears.
pub const SCHEMA_VERSION: u32 = 1;

/// The size at which a message has stopped being a message and become a bug.
///
/// Not enforced by truncating — something over this is to be fixed rather than
/// silently cut in half — but worth a warning, and worth knowing that the
/// debrief is the part that grows.
///
/// **This is not the fragmentation threshold, and deliberately so.** A datagram
/// stops fitting one Ethernet frame at 1472 bytes and one Tailscale frame at
/// about 1252, and the measured sizes sit either side of that:
///
/// | what is on the wire | bytes | one Ethernet frame | one mesh-VPN frame |
/// |---|---|---|---|
/// | no advice yet | 953 | yes | yes |
/// | four lines of live advice | 1188 | yes | yes |
/// | eight full lines plus three laps of debrief | 3971 | no, 3 fragments | no, 4 |
///
/// So the stream a spectator watches is one packet, and the burst after a lap
/// is three or four. IP reassembles them; losing any one fragment discards the
/// whole datagram, which on a LAN is nothing and across a VPN is an occasional
/// missed update that the next one at 10 Hz replaces a tenth of a second later.
/// Warning at the fragmentation point instead would fire after every lap for a
/// thing that works, and a warning nobody can act on is a warning nobody reads.
const SAFE_DATAGRAM_BYTES: usize = 8192;

/// What a datagram may be and still cross one Ethernet frame: 1500 less 20 of
/// IP header and 8 of UDP.
#[cfg(test)]
const ETHERNET_PAYLOAD: usize = 1472;

/// The same for a mesh VPN, which tunnels and so has less room: Tailscale and
/// ZeroTier both default to a 1280-byte MTU.
#[cfg(test)]
const MESH_VPN_PAYLOAD: usize = 1252;

#[derive(Serialize, Deserialize)]
pub struct Corner {
    pub pressure_psi: f32,
    pub temp_c: f32,
    pub temp_inner_c: f32,
    pub temp_outer_c: f32,
    pub wear_percent: f32,
    pub brake_temp_c: f32,
    pub laps_remaining: f32,
}

#[derive(Serialize, Deserialize)]
pub struct Advice {
    pub severity: u32,
    pub text: String,
}

#[derive(Serialize, Deserialize)]
pub struct DebriefLapOut {
    pub lap_number: u32,
    pub lap_time_ms: u32,
    pub sectors_ms: Vec<u32>,
    pub lines: Vec<Advice>,
}

/// One published message. Everything a front end needs to draw a full panel.
#[derive(Serialize, Deserialize)]
pub struct Message {
    /// Always `"acpe"`, so a receiver on a shared port can tell our datagrams
    /// from somebody else's without parsing further.
    pub magic: String,
    pub schema: u32,
    /// The application's release, so a receiver can say "that driver is on an
    /// older build" rather than mis-drawing a field it does not have.
    pub app_version: String,
    /// Which simulator produced this.
    pub game: String,
    /// Who is driving. Empty when the sender did not set one.
    ///
    /// The reason this is here rather than assumed: a receiver watching a
    /// championship has several of these arriving at once and has to be able to
    /// tell them apart, and a friend watching one driver has to be able to see
    /// *whose* numbers are on their screen. A panel drawing someone else's lap
    /// count with no name on it produces a bug report about telemetry that does
    /// not match the game.
    pub driver: String,
    /// Monotonic, from the frame. A receiver can drop what arrives out of order.
    pub sequence: u32,

    pub speed_kmh: f32,
    pub rpm: i32,
    pub max_rpm: i32,
    pub gear: i32,
    pub fuel_litres: f32,
    pub fuel_laps_remaining: f32,
    pub fuel_per_lap: f32,
    pub delta_seconds: f32,
    pub position: i32,
    pub lap_count: i32,
    pub last_lap_ms: i32,
    pub best_lap_ms: i32,
    pub current_lap_ms: i32,
    pub stint_laps: u32,
    pub air_temp_c: f32,
    pub road_temp_c: f32,
    pub surface_grip: f32,
    pub flags: u32,

    pub corners: Vec<Corner>,
    pub advice: Vec<Advice>,
    pub debrief: Vec<DebriefLapOut>,
}

/// Read a NUL-padded fixed byte array as a string.
fn text(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub(crate) fn message(frame: &OverlayFrame, game: &str, driver: &str) -> Message {
    let corners = (0..4)
        .map(|i| Corner {
            pressure_psi: frame.tyre_pressure_psi[i],
            temp_c: frame.tyre_temp_c[i],
            temp_inner_c: frame.tyre_temp_inner_c[i],
            temp_outer_c: frame.tyre_temp_outer_c[i],
            wear_percent: frame.tyre_wear_percent[i],
            brake_temp_c: frame.brake_temp_c[i],
            laps_remaining: frame.tyre_laps_remaining[i],
        })
        .collect();

    let advice = (0..MESSAGE_SLOTS)
        .take(frame.message_count as usize)
        .map(|i| Advice {
            severity: frame.message_severity[i],
            text: text(&frame.messages[i]),
        })
        .filter(|line| !line.text.is_empty())
        .collect();

    let debrief = (0..DEBRIEF_LAPS)
        .take(frame.debrief_lap_count as usize)
        .map(|lap| DebriefLapOut {
            lap_number: frame.debrief_lap_number[lap],
            lap_time_ms: frame.debrief_lap_time_ms[lap],
            sectors_ms: (0..3)
                .map(|s| frame.debrief_sector_ms[lap * 3 + s])
                .collect(),
            lines: (0..DEBRIEF_LINES)
                .take(frame.debrief_line_count[lap] as usize)
                .map(|line| {
                    let slot = lap * DEBRIEF_LINES + line;
                    Advice {
                        severity: frame.debrief_severity[slot],
                        text: text(&frame.debrief[slot]),
                    }
                })
                .filter(|line| !line.text.is_empty())
                .collect(),
        })
        .collect();

    Message {
        magic: MAGIC.to_string(),
        schema: SCHEMA_VERSION,
        app_version: text(&frame.app_version),
        game: game.to_string(),
        driver: driver.to_string(),
        sequence: frame.sequence,
        speed_kmh: frame.speed_kmh,
        rpm: frame.rpm,
        max_rpm: frame.max_rpm,
        gear: frame.gear,
        fuel_litres: frame.fuel_litres,
        fuel_laps_remaining: frame.fuel_laps_remaining,
        fuel_per_lap: frame.fuel_per_lap,
        delta_seconds: frame.delta_seconds,
        position: frame.position,
        lap_count: frame.lap_count,
        last_lap_ms: frame.last_lap_ms,
        best_lap_ms: frame.best_lap_ms,
        current_lap_ms: frame.current_lap_ms,
        stint_laps: frame.stint_laps,
        air_temp_c: frame.air_temp_c,
        road_temp_c: frame.road_temp_c,
        surface_grip: frame.surface_grip,
        flags: frame.flags,
        corners,
        advice,
        debrief,
    }
}

/// Sends every frame to one address as JSON.
pub struct UdpSink {
    socket: UdpSocket,
    target: SocketAddr,
    name: String,
    game: String,
    driver: String,
    interval: Duration,
    warned_about_size: bool,
}

impl UdpSink {
    /// Bind an ephemeral local port and aim at `target`.
    ///
    /// Nothing is connected: UDP has no connection, so this cannot fail because
    /// the far end is absent, which is the point — a friend can start their
    /// receiver after the driver has already gone out.
    pub fn new(
        target: SocketAddr,
        game: impl Into<String>,
        driver: impl Into<String>,
        rate_hz: f32,
    ) -> std::io::Result<Self> {
        // 0.0.0.0:0 — any interface, any free port. The receiver replies to
        // nothing, so the local port is of no interest to anyone.
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        // Never block the tick, whatever the network is doing.
        socket.set_nonblocking(true)?;

        let interval = if rate_hz > 0.0 {
            Duration::from_secs_f32(1.0 / rate_hz)
        } else {
            Duration::ZERO
        };

        Ok(Self {
            socket,
            target,
            name: format!("udp {target}"),
            game: game.into(),
            driver: driver.into(),
            interval,
            warned_about_size: false,
        })
    }
}

/// How many bytes this frame would put on the wire.
///
/// For `share_probe`, and for anyone checking a link before they rely on it:
/// the number decides whether a spectator receives one packet or a handful of
/// fragments, and nothing on loopback can tell the difference.
pub fn payload_size(frame: &OverlayFrame, game: &str, driver: &str) -> usize {
    serde_json::to_vec(&message(frame, game, driver))
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

impl Sink for UdpSink {
    fn name(&self) -> &str {
        &self.name
    }

    fn min_interval(&self) -> Duration {
        self.interval
    }

    fn publish(&mut self, frame: &OverlayFrame) -> std::io::Result<()> {
        let payload = serde_json::to_vec(&message(frame, &self.game, &self.driver))
            .map_err(std::io::Error::other)?;

        if payload.len() > SAFE_DATAGRAM_BYTES && !self.warned_about_size {
            self.warned_about_size = true;
            tracing::warn!(
                bytes = payload.len(),
                "A published message is past {SAFE_DATAGRAM_BYTES} bytes and will fragment"
            );
        }

        match self.socket.send_to(&payload, self.target) {
            Ok(_) => Ok(()),
            // Nobody is listening on the far end. On a connectionless socket
            // this is normal — the receiver has not started, or has stopped —
            // and it is not worth counting as a failure that eventually drops
            // the sink.
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineer::{Recommendation, Severity};

    fn advice_line(text: &str) -> Recommendation {
        Recommendation {
            component: String::new(),
            category: String::new(),
            severity: Severity::Warning,
            message: text.to_string(),
            action: String::new(),
            parameters: Vec::new(),
            confidence: 1.0,
            chain: None,
        }
    }

    /// A receiver has to be able to read this without knowing anything about
    /// `#[repr(C)]`, which is the entire reason it is JSON.
    #[test]
    fn a_frame_becomes_readable_json() {
        let mut frame = OverlayFrame::empty();
        frame.speed_kmh = 214.0;
        frame.gear = 5;
        frame.sequence = 42;
        frame.tyre_pressure_psi = [26.8, 27.0, 26.4, 26.6];
        frame.set_messages(&[advice_line("Fronts over 28.4 psi")]);

        let json = serde_json::to_string(&message(&frame, "assetto_corsa", "Rgosh"))
            .expect("the message serialises");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("and parses back");

        assert_eq!(parsed["magic"], "acpe");
        assert_eq!(parsed["schema"], SCHEMA_VERSION);
        assert_eq!(parsed["game"], "assetto_corsa");
        assert_eq!(parsed["driver"], "Rgosh");
        assert_eq!(parsed["sequence"], 42);
        assert_eq!(parsed["gear"], 5);
        assert_eq!(parsed["corners"][0]["pressure_psi"], 26.8);
        assert_eq!(parsed["advice"][0]["text"], "Fronts over 28.4 psi");
    }

    /// Empty slots are not empty strings in the output. A receiver iterating
    /// `advice` should get the lines that exist and nothing else.
    #[test]
    fn unused_slots_do_not_travel() {
        let mut frame = OverlayFrame::empty();
        frame.set_messages(&[advice_line("one"), advice_line("two")]);

        let value: serde_json::Value = serde_json::from_str(
            &serde_json::to_string(&message(&frame, "g", "d"))
                .expect("a well-formed message round-trips"),
        )
        .expect("the message serialises");

        assert_eq!(
            value["advice"]
                .as_array()
                .expect("a well-formed message round-trips")
                .len(),
            2
        );
    }

    /// Sending to an address with nothing behind it is the normal state at the
    /// moment a driver goes out and their friend has not opened the receiver
    /// yet. It must not count as a failure.
    #[test]
    fn nobody_listening_is_not_a_failure() {
        let mut sink = UdpSink::new(
            "127.0.0.1:59999"
                .parse()
                .expect("a well-formed message round-trips"),
            "assetto_corsa",
            "",
            0.0,
        )
        .expect("bind an ephemeral socket");

        let frame = OverlayFrame::empty();
        for _ in 0..5 {
            sink.publish(&frame).expect("no listener is not an error");
        }
    }

    /// The stream a spectator actually watches fits **one** packet.
    ///
    /// This is the claim the site makes about watching a friend over a mesh
    /// VPN, and it is the one worth pinning: while a driver is on track the
    /// message carries live advice and no debrief, and at that size it crosses
    /// a network as a single datagram with nothing to reassemble and nothing to
    /// lose halfway.
    ///
    /// A field added carelessly to the live part would push it over, and the
    /// symptom would not be an error — it would be a spectator whose screen
    /// stutters on a lossy link, which is the sort of thing that gets blamed on
    /// the network for a month.
    #[test]
    fn the_live_stream_crosses_a_network_in_one_packet() {
        let mut frame = OverlayFrame::empty();
        // Four lines is a busy moment: pressures, temperatures, brakes, wear.
        let typical = "Fronts over 28.4 psi (target 27.5)";
        frame.set_messages(&(0..4).map(|_| advice_line(typical)).collect::<Vec<_>>());

        let bytes = serde_json::to_vec(&message(&frame, "assetto_corsa", "somebody"))
            .expect("the message serialises");
        assert!(
            bytes.len() <= MESH_VPN_PAYLOAD,
            "live advice is {} bytes, past the {MESH_VPN_PAYLOAD} a mesh VPN carries \
             in one frame — a spectator would be reassembling fragments the whole \
             session",
            bytes.len()
        );
        assert!(bytes.len() <= ETHERNET_PAYLOAD);
    }

    /// And the burst after a lap does not, which is a fact rather than a fault.
    ///
    /// The debrief is three laps of eight lines and it fragments — three frames
    /// on Ethernet, four through a tunnel. That is fine and this test exists to
    /// say it is *known* fine: IP reassembles, a lost fragment costs one update
    /// out of ten a second, and the alternative is a protocol with sequencing
    /// and retries for a spectator view.
    ///
    /// The ceiling below is the one that means "something is wrong", not the
    /// one where fragmentation begins.
    #[test]
    fn a_full_message_fits_one_datagram() {
        let mut frame = OverlayFrame::empty();
        let long = "x".repeat(63);
        let lines: Vec<_> = (0..MESSAGE_SLOTS).map(|_| advice_line(&long)).collect();
        frame.set_messages(&lines);

        let laps: Vec<_> = (0..DEBRIEF_LAPS)
            .map(|index| crate::overlay::frame::DebriefLap {
                lap_number: index as u32,
                lap_time_ms: 91_234,
                sectors: [28_540, 31_120, 31_574],
                advice: (0..DEBRIEF_LINES).map(|_| advice_line(&long)).collect(),
            })
            .collect();
        frame.set_debrief(&laps, DEBRIEF_LINES);

        let bytes = serde_json::to_vec(&message(&frame, "assetto_corsa", "somebody"))
            .expect("the message serialises");
        assert!(
            bytes.len() < SAFE_DATAGRAM_BYTES,
            "a full message is {} bytes, past the {SAFE_DATAGRAM_BYTES} a datagram \
             should stay under",
            bytes.len()
        );
    }
}
