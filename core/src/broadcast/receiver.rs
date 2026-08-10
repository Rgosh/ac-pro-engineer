//! The other end of the UDP feed: watching someone else drive.
//!
//! `docs/ARCHITECTURE.md` calls this the requirement that decides the shape of
//! the whole design. A driver is on track and cannot look away; a friend runs
//! the same program, sets it to receive, and sees the driver's telemetry and
//! the engineer's advice in their own overlay.
//!
//! ```text
//! driver's machine                          friend's machine
//!   AC shared memory                          this receiver
//!         │                                         │
//!     core (analyse, advise)  ──[ UdpSink ]──►  local frame → panel
//! ```
//!
//! ## What arrives is the computed frame, not telemetry
//!
//! The architecture doc weighs sending samples and advising locally against
//! sending the finished frame, and chooses the second: far smaller, far
//! simpler, and the friend sees exactly what the driver's engineer is saying —
//! which is the point when the reason you are watching is to help the person
//! driving. The cost is that the viewer's own units and language do not apply,
//! because the sentences were written on the other machine.
//!
//! So the receiving side does no analysis at all. It parses a datagram back
//! into an [`OverlayFrame`] and hands it to whatever draws frames. Everything
//! downstream — the panel, the terminal — is unchanged, which is the test of
//! whether the sink/source boundary was drawn in the right place.
//!
//! ## What is deliberately not here yet
//!
//! **The sender's name does not reach the panel.** It arrives in the datagram
//! and this keeps it, but showing it needs bytes in the frame and the frame
//! cannot move without costing every Linux driver a bridge update. The
//! [`flags::REMOTE`] bit is set, which is free, so the panel can at least say
//! these are not your numbers. The name goes in whenever the struct next moves.
//!
//! **Nothing here crosses a NAT.** Two machines on one network is this and
//! nothing else; two machines behind two home routers needs a forwarded port or
//! a relay in the middle, which is infrastructure rather than code and is the
//! only part of the architecture doc's plan that is not a weekend.

use super::udp::{MAGIC, Message, SCHEMA_VERSION};
use crate::engineer::{Recommendation, Severity};
use crate::overlay::frame::{
    DEBRIEF_LAPS, DEBRIEF_LINES, DebriefLap, OverlayFrame, flags, severity,
};
use std::net::{SocketAddr, UdpSocket};
use tracing::{debug, warn};

/// The largest datagram worth reading.
///
/// A full message — eight advice lines and three laps of debrief with eight
/// lines each — is a few kilobytes; this is the ceiling a UDP datagram can be
/// anyway, so anything larger is not ours.
const MAX_DATAGRAM: usize = 65_535;

/// Why a datagram was thrown away.
///
/// Kept apart from "nothing arrived" because they mean different things to
/// somebody trying to work out why their screen is empty: nothing arriving is a
/// firewall or the wrong address, and a rejected datagram is a version mismatch
/// or another program on the same port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    /// Not one of ours — some other program shares the port.
    NotOurs,
    /// Ours, but written against a schema this build does not know.
    WrongSchema { theirs: u32, ours: u32 },
    /// Ours and the right schema, but not valid JSON.
    Malformed,
}

/// What one call to [`FrameReceiver::poll`] found.
#[derive(Debug)]
pub enum Received {
    /// A frame, and who sent it.
    Frame(Box<OverlayFrame>),
    /// A datagram arrived and was not usable.
    Rejected(Rejected),
    /// Nothing was waiting.
    Idle,
}

/// Listens for frames published by another machine's [`super::udp::UdpSink`].
pub struct FrameReceiver {
    socket: UdpSocket,
    buffer: Vec<u8>,
    /// Who sent the last frame we accepted, and the name they travel under.
    last_sender: Option<(SocketAddr, String)>,
    /// Sequence of the last frame accepted, so datagrams that overtook each
    /// other are dropped rather than drawn.
    last_sequence: Option<u32>,
    rejections: u64,
    warned_about_schema: bool,
}

impl FrameReceiver {
    /// Listen on `address` for frames.
    ///
    /// Non-blocking, because this is polled from the same loop that draws: a
    /// receiver waiting on a socket is a terminal that has stopped repainting.
    pub fn bind(address: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(address)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            buffer: vec![0; MAX_DATAGRAM],
            last_sender: None,
            last_sequence: None,
            rejections: 0,
            warned_about_schema: false,
        })
    }

    /// Who we are hearing from, for the status line.
    pub fn sender(&self) -> Option<&(SocketAddr, String)> {
        self.last_sender.as_ref()
    }

    pub fn rejections(&self) -> u64 {
        self.rejections
    }

    /// Take the newest frame waiting, if any.
    ///
    /// Drains the socket rather than returning the first datagram: at ten a
    /// second into a loop that runs at sixty, a queue only ever means the
    /// reader fell behind, and the newest frame is the only one worth drawing.
    pub fn poll(&mut self) -> Received {
        let mut newest: Option<Box<OverlayFrame>> = None;
        let mut last_rejection = None;

        loop {
            let (size, from) = match self.socket.recv_from(&mut self.buffer) {
                Ok(pair) => pair,
                // Nothing left in the queue.
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => {
                    debug!(error = ?error, "Receiver socket error");
                    break;
                }
            };

            let Some(datagram) = self.buffer.get(..size) else {
                break;
            };
            match self.decode(datagram) {
                Ok(message) => {
                    // A datagram that overtook a newer one is stale. UDP does
                    // not promise order, and drawing an older frame after a
                    // newer one makes the lap counter go backwards.
                    let stale = self
                        .last_sequence
                        .is_some_and(|last| message.sequence < last);
                    if stale {
                        continue;
                    }
                    self.last_sequence = Some(message.sequence);
                    self.last_sender = Some((from, message.driver.clone()));
                    newest = Some(Box::new(to_frame(&message)));
                }
                Err(rejection) => {
                    self.rejections += 1;
                    // Logged once: a program sharing the port would otherwise
                    // write a line ten times a second for the whole session.
                    if let Rejected::WrongSchema { theirs, ours } = rejection
                        && !self.warned_about_schema
                    {
                        self.warned_about_schema = true;
                        warn!(
                            theirs,
                            ours, "Receiving from a sender on a different schema; ignoring it"
                        );
                    }
                    last_rejection = Some(rejection);
                }
            }
        }

        match (newest, last_rejection) {
            (Some(frame), _) => Received::Frame(frame),
            (None, Some(rejection)) => Received::Rejected(rejection),
            (None, None) => Received::Idle,
        }
    }

    /// Is this ours, and can this build read it?
    fn decode(&self, datagram: &[u8]) -> Result<Message, Rejected> {
        // The magic is checked before parsing rather than after, so a port
        // shared with an unrelated program costs a substring search and not a
        // full JSON parse ten times a second.
        let looks_like_ours = datagram
            .windows(MAGIC.len())
            .any(|window| window == MAGIC.as_bytes());
        if !looks_like_ours {
            return Err(Rejected::NotOurs);
        }

        let message: Message = serde_json::from_slice(datagram).map_err(|_| Rejected::Malformed)?;
        if message.magic != MAGIC {
            return Err(Rejected::NotOurs);
        }
        if message.schema != SCHEMA_VERSION {
            return Err(Rejected::WrongSchema {
                theirs: message.schema,
                ours: SCHEMA_VERSION,
            });
        }
        Ok(message)
    }
}

/// Turn a received message back into the frame that produced it.
///
/// Not every field survives a round trip and that is deliberate: the settings
/// flags — which blocks the *sender* chose to draw — are theirs, and applying
/// them to the viewer's panel would let one person's preferences reach into
/// another's screen. Everything that is a measurement comes back; everything
/// that is a preference is left to the receiving side.
pub fn to_frame(message: &Message) -> OverlayFrame {
    let mut frame = OverlayFrame::empty();

    frame.sequence = message.sequence;
    frame.speed_kmh = message.speed_kmh;
    frame.rpm = message.rpm;
    frame.max_rpm = message.max_rpm;
    frame.gear = message.gear;
    frame.fuel_litres = message.fuel_litres;
    frame.fuel_laps_remaining = message.fuel_laps_remaining;
    frame.fuel_per_lap = message.fuel_per_lap;
    frame.delta_seconds = message.delta_seconds;
    frame.position = message.position;
    frame.lap_count = message.lap_count;
    frame.last_lap_ms = message.last_lap_ms;
    frame.best_lap_ms = message.best_lap_ms;
    frame.current_lap_ms = message.current_lap_ms;
    frame.stint_laps = message.stint_laps;
    frame.air_temp_c = message.air_temp_c;
    frame.road_temp_c = message.road_temp_c;
    frame.surface_grip = message.surface_grip;

    for (index, corner) in message.corners.iter().enumerate().take(4) {
        frame.tyre_pressure_psi[index] = corner.pressure_psi;
        frame.tyre_temp_c[index] = corner.temp_c;
        frame.tyre_temp_inner_c[index] = corner.temp_inner_c;
        frame.tyre_temp_outer_c[index] = corner.temp_outer_c;
        frame.tyre_wear_percent[index] = corner.wear_percent;
        frame.brake_temp_c[index] = corner.brake_temp_c;
        frame.tyre_laps_remaining[index] = corner.laps_remaining;
    }

    let advice: Vec<Recommendation> = message
        .advice
        .iter()
        .map(|line| recommendation(line.severity, &line.text))
        .collect();
    frame.set_messages(&advice);

    let laps: Vec<DebriefLap> = message
        .debrief
        .iter()
        .take(DEBRIEF_LAPS)
        .map(|lap| DebriefLap {
            lap_number: lap.lap_number,
            lap_time_ms: lap.lap_time_ms,
            sectors: [
                lap.sectors_ms.first().copied().unwrap_or(0),
                lap.sectors_ms.get(1).copied().unwrap_or(0),
                lap.sectors_ms.get(2).copied().unwrap_or(0),
            ],
            advice: lap
                .lines
                .iter()
                .map(|line| recommendation(line.severity, &line.text))
                .collect(),
        })
        .collect();
    frame.set_debrief(&laps, DEBRIEF_LINES);
    frame.set_sectors(&laps, [0; 3]);

    // The sender's measured state travels; the sender's preferences do not.
    frame.set_flag(flags::CONNECTED, message.flags & flags::CONNECTED != 0);
    frame.set_flag(flags::PIT_LIMITER, message.flags & flags::PIT_LIMITER != 0);
    frame.set_flag(
        flags::FUEL_WARNING,
        message.flags & flags::FUEL_WARNING != 0,
    );
    // These are not the viewer's numbers, and the panel has to be able to say so.
    frame.set_flag(flags::REMOTE, true);

    frame
}

/// Rebuild one line of advice. The sentence was written on the sender's
/// machine, in the sender's language, and travels as text for exactly that
/// reason.
fn recommendation(level: u32, text: &str) -> Recommendation {
    Recommendation {
        severity: match level {
            severity::CRITICAL => Severity::Critical,
            severity::WARNING => Severity::Warning,
            _ => Severity::Info,
        },
        message: text.to_string(),
        confidence: 1.0,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broadcast::udp::message;

    fn advice_line(text: &str) -> Recommendation {
        Recommendation {
            severity: Severity::Warning,
            message: text.to_string(),
            confidence: 1.0,
            ..Default::default()
        }
    }

    fn sent_frame() -> OverlayFrame {
        let mut frame = OverlayFrame::empty();
        frame.sequence = 5150;
        frame.speed_kmh = 214.5;
        frame.gear = 5;
        frame.rpm = 7400;
        frame.lap_count = 7;
        frame.best_lap_ms = 91_380;
        frame.stint_laps = 7;
        frame.tyre_pressure_psi = [26.8, 27.0, 26.4, 26.6];
        frame.tyre_temp_inner_c = [92.0, 91.0, 88.0, 87.0];
        frame.set_flag(flags::CONNECTED, true);
        frame.set_flag(flags::PIT_LIMITER, true);
        frame.set_messages(&[advice_line("Fronts over 28.4 psi")]);
        let laps = [DebriefLap {
            lap_number: 12,
            lap_time_ms: 91_234,
            sectors: [28_540, 31_120, 31_574],
            advice: vec![advice_line("Rears cold")],
        }];
        frame.set_debrief(&laps, DEBRIEF_LINES);
        // Both, the way the application does: the sectors come from the
        // analyser and the advice from the engineer, so they are two calls and
        // a producer that makes only the first publishes zeroed sectors.
        frame.set_sectors(&laps, [28_400, 31_000, 31_500]);
        frame
    }

    /// The whole point: what the driver's engineer said reaches the friend's
    /// screen unchanged.
    #[test]
    fn a_frame_survives_the_round_trip() {
        let sent = sent_frame();
        let json = serde_json::to_vec(&message(&sent, "assetto_corsa", "Rgosh"))
            .expect("the message serialises");
        let parsed: Message = serde_json::from_slice(&json).expect("and parses back");
        let got = to_frame(&parsed);

        assert_eq!(got.sequence, 5150);
        assert_eq!(got.gear, 5);
        assert_eq!(got.lap_count, 7);
        assert_eq!(got.best_lap_ms, 91_380);
        assert_eq!(got.stint_laps, 7);
        assert!((got.speed_kmh - 214.5).abs() < 0.01);
        assert!((got.tyre_pressure_psi[0] - 26.8).abs() < 0.01);
        assert!((got.tyre_temp_inner_c[0] - 92.0).abs() < 0.01);

        assert_eq!(got.message_count, 1);
        assert_eq!(got.debrief_lap_count, 1);
        assert_eq!(got.debrief_lap_number[0], 12);
        assert_eq!(got.debrief_sector_ms[1], 31_120);
    }

    /// The viewer has to be able to tell somebody else's lap counter from their
    /// own, or the bug report is about telemetry not matching the game.
    #[test]
    fn a_received_frame_says_it_is_not_yours() {
        let parsed: Message = serde_json::from_slice(
            &serde_json::to_vec(&message(&sent_frame(), "assetto_corsa", "Rgosh"))
                .expect("the message serialises"),
        )
        .expect("and parses back");

        let got = to_frame(&parsed);
        assert!(got.flags & flags::REMOTE != 0);
        assert!(got.flags & flags::CONNECTED != 0, "the sender was on track");
        assert!(got.flags & flags::PIT_LIMITER != 0);
    }

    /// One person's choice of which blocks to draw must not reach into
    /// another's panel.
    #[test]
    fn the_senders_preferences_stay_on_the_senders_machine() {
        let mut sent = sent_frame();
        sent.set_flag(flags::SHOW_TELEMETRY, true);
        sent.set_flag(flags::RUSSIAN, true);

        let parsed: Message = serde_json::from_slice(
            &serde_json::to_vec(&message(&sent, "assetto_corsa", "Rgosh"))
                .expect("the message serialises"),
        )
        .expect("and parses back");
        let got = to_frame(&parsed);

        assert_eq!(
            got.flags & flags::SHOW_TELEMETRY,
            0,
            "which blocks to draw is the viewer's business"
        );
        assert_eq!(
            got.flags & flags::RUSSIAN,
            0,
            "and so is what language to draw them in"
        );
    }

    fn receiver() -> (FrameReceiver, SocketAddr) {
        let receiver = FrameReceiver::bind(
            "127.0.0.1:0"
                .parse()
                .expect("a well-formed loopback address"),
        )
        .expect("bind an ephemeral socket");
        let address = receiver
            .socket
            .local_addr()
            .expect("a bound socket has an address");
        (receiver, address)
    }

    fn send(to: SocketAddr, bytes: &[u8]) {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("bind an ephemeral socket");
        socket.send_to(bytes, to).expect("loopback always accepts");
    }

    #[test]
    fn nothing_waiting_is_not_an_error() {
        let (mut receiver, _) = receiver();
        assert!(matches!(receiver.poll(), Received::Idle));
    }

    /// A port shared with an unrelated program must not fill the log or look
    /// like a broken sender.
    #[test]
    fn somebody_elses_datagram_is_told_apart_from_ours() {
        let (mut receiver, address) = receiver();
        send(address, b"a totally unrelated program's packet");
        std::thread::sleep(std::time::Duration::from_millis(50));

        assert!(matches!(
            receiver.poll(),
            Received::Rejected(Rejected::NotOurs)
        ));
        assert_eq!(receiver.rejections(), 1);
    }

    /// A sender on a newer schema is refused rather than half-read.
    #[test]
    fn a_sender_on_another_schema_is_refused() {
        let (mut receiver, address) = receiver();
        let mut wire = message(&sent_frame(), "assetto_corsa", "Rgosh");
        wire.schema = SCHEMA_VERSION + 1;
        send(
            address,
            &serde_json::to_vec(&wire).expect("the message serialises"),
        );
        std::thread::sleep(std::time::Duration::from_millis(50));

        let Received::Rejected(Rejected::WrongSchema { theirs, ours }) = receiver.poll() else {
            unreachable!("a different schema is not readable")
        };
        assert_eq!(theirs, SCHEMA_VERSION + 1);
        assert_eq!(ours, SCHEMA_VERSION);
    }

    /// Ten a second into a loop running at sixty means a queue only ever forms
    /// when the reader fell behind, and then only the newest frame matters.
    #[test]
    fn a_queue_is_drained_to_the_newest_frame() {
        let (mut receiver, address) = receiver();
        for sequence in [1u32, 2, 3] {
            let mut frame = sent_frame();
            frame.sequence = sequence;
            send(
                address,
                &serde_json::to_vec(&message(&frame, "assetto_corsa", "Rgosh"))
                    .expect("the message serialises"),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(50));

        let Received::Frame(frame) = receiver.poll() else {
            unreachable!("three datagrams were sent")
        };
        assert_eq!(frame.sequence, 3, "the newest, not the first");
        assert!(
            matches!(receiver.poll(), Received::Idle),
            "and nothing left"
        );
    }

    /// UDP does not promise order, and a lap counter that goes backwards reads
    /// as the application being broken.
    #[test]
    fn a_datagram_that_overtook_a_newer_one_is_dropped() {
        let (mut receiver, address) = receiver();
        let mut newer = sent_frame();
        newer.sequence = 900;
        send(
            address,
            &serde_json::to_vec(&message(&newer, "assetto_corsa", "Rgosh"))
                .expect("the message serialises"),
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(matches!(receiver.poll(), Received::Frame(_)));

        let mut older = sent_frame();
        older.sequence = 400;
        send(
            address,
            &serde_json::to_vec(&message(&older, "assetto_corsa", "Rgosh"))
                .expect("the message serialises"),
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            matches!(receiver.poll(), Received::Idle),
            "an older frame is not a frame"
        );
    }

    #[test]
    fn the_sender_is_remembered_for_the_status_line() {
        let (mut receiver, address) = receiver();
        send(
            address,
            &serde_json::to_vec(&message(&sent_frame(), "assetto_corsa", "Rgosh"))
                .expect("the message serialises"),
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(matches!(receiver.poll(), Received::Frame(_)));

        let (_, name) = receiver.sender().expect("a frame was accepted");
        assert_eq!(name, "Rgosh");
    }
}
