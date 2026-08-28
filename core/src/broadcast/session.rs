//! A whole session on the wire, so a watcher's screens are not a summary.
//!
//! [`udp`](super::udp) sends the computed *frame*: speed, four corners, the
//! sentences the engineer wrote. That is the right thing to send to a panel —
//! it is a panel's worth of data and it is what the panel draws. It is the
//! wrong thing to send to another copy of this program, and the difference is
//! not size. A frame has no coordinates in it, so a watcher cannot draw the
//! map; no throttle trace, so it cannot draw a lap; no capabilities, so it
//! cannot tell an unmeasured zero from a measured one. A spectator was handed
//! four sentences and a speed and could not ask a single one of the questions
//! this program exists to answer.
//!
//! So this sends the [`Reading`] — the same struct a game's reader produces,
//! field for field — and the receiving copy hands it to the same analysis a
//! local game feeds. Laps are detected there, traces are built there, the
//! engineer runs there. Every screen then works without knowing where the
//! numbers came from, and the watcher's own units and language apply, which
//! they cannot when the sentences were written on the other machine.
//!
//! ```text
//!   driver's machine                        watcher's machine
//!     game → Reading ──[ Sender ]──►──[ Listener ]──► the same analysis
//!                                                     → every screen
//! ```
//!
//! # Why this and the frame both exist
//!
//! They answer different questions and neither replaces the other:
//!
//! | | [`udp`](super::udp) | this |
//! |---|---|---|
//! | what travels | the finished frame | the reading |
//! | who reads it | the in-game panel, a relay, fifty lines of Python | another copy of this program |
//! | who analyses | the driver's machine | the watcher's |
//! | bytes a message | about 1 kB | about 2 kB |
//!
//! # Rules that are not negotiable
//!
//! * **UDP, and never a wait.** A watcher that stops reading must not be able
//!   to stall the tick that is also feeding the driver's own overlay.
//! * **Nothing here writes a record.** Somebody else's lap is not this
//!   driver's personal best. The rule lives with the caller — there is nothing
//!   in this module that could — and it is stated here because this is where
//!   somebody will come looking for it.

use crate::games::Reading;
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

/// So a datagram from something else on this port is not read as a session.
pub const WHAT: &str = "pro-engineer/session";

/// What the window called this before the two programs shared one protocol.
///
/// Accepted on the way in and never sent. A copy of the window from v0.4.2
/// speaks it, and a driver who updates one machine before the other should not
/// find that they have stopped being able to see each other.
pub const WHAT_WAS: &str = "rg-pro-engineer/reading";

/// Bumped when the shape below changes. A copy speaking another number is told
/// rather than half-understood.
pub const SCHEMA: u32 = 1;

/// The most a reading may be on the wire.
///
/// A tick is about two kilobytes; this is the ceiling past which something is
/// not ours. Not the fragmentation threshold: a reading crosses one Ethernet
/// frame comfortably and IP would reassemble it if it did not.
const MAX_DATAGRAM: usize = 64 * 1024;

/// One tick, addressed and numbered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub what: String,
    pub schema: u32,
    /// Who is driving, by the name they chose, for the strip that says whose
    /// numbers these are. A watcher with two feeds arriving cannot tell them
    /// apart without it.
    pub from: String,
    /// Counted by the sender, so a receiver can tell a frame that overtook
    /// another from one that is new, and can count the ones that never came.
    pub sequence: u64,
    pub reading: Reading,
}

/// The sending half: one reading a tick, to one address.
pub struct Sender {
    socket: UdpSocket,
    target: SocketAddr,
    name: String,
    sequence: u64,
    interval: Duration,
    last_sent: Option<Instant>,
    sent: u64,
    /// Sending is paused — the car is off track and the driver asked for that.
    held: bool,
}

impl Sender {
    /// Aim at an address.
    ///
    /// Nothing is connected — UDP has no connection — so this cannot fail
    /// because the far end is absent, which is the point: a watcher may start
    /// after the driver has already gone out. A name is resolved rather than
    /// parsed, so `friend-pc:9001` works.
    ///
    /// `rate_hz` of zero means every tick.
    pub fn open(target: &str, name: &str, rate_hz: f32) -> Result<Self, String> {
        let address = target
            .trim()
            .to_socket_addrs()
            .map_err(|error| format!("{target}: {error}"))?
            .next()
            .ok_or_else(|| format!("{target}: no address"))?;
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|error| format!("cannot open a socket: {error}"))?;
        socket
            .set_nonblocking(true)
            .map_err(|error| format!("cannot set non-blocking: {error}"))?;
        Ok(Self {
            socket,
            target: address,
            name: name.trim().to_string(),
            sequence: 0,
            interval: if rate_hz > 0.0 {
                Duration::from_secs_f32(1.0 / rate_hz)
            } else {
                Duration::ZERO
            },
            last_sent: None,
            sent: 0,
            held: false,
        })
    }

    /// Send this tick, if it is time.
    ///
    /// **Never waits.** A non-blocking socket that would block drops the
    /// datagram, which is the right answer for telemetry: the next one is
    /// already better than the one that did not fit.
    pub fn send(&mut self, reading: &Reading) {
        if self.held {
            return;
        }
        let due = self
            .last_sent
            .is_none_or(|when| when.elapsed() >= self.interval);
        if !due {
            return;
        }
        self.sequence += 1;
        let envelope = Envelope {
            what: WHAT.to_string(),
            schema: SCHEMA,
            from: self.name.clone(),
            sequence: self.sequence,
            reading: reading.clone(),
        };
        if let Ok(bytes) = serde_json::to_vec(&envelope)
            && self.socket.send_to(&bytes, self.target).is_ok()
        {
            self.sent += 1;
        }
        self.last_sent = Some(Instant::now());
    }

    /// The address this link is aimed at, for the line that says so.
    pub fn target(&self) -> String {
        self.target.to_string()
    }

    /// Pause or resume sending, without closing the socket.
    ///
    /// For "only while I am on track": a session spent in the menus otherwise
    /// sends a reading thirty times a second saying nothing is happening, and
    /// a watcher cannot tell a driver in the pits from one who has quit.
    pub fn hold(&mut self, held: bool) {
        self.held = held;
    }

    pub fn sent(&self) -> u64 {
        self.sent
    }
}

/// What a screen needs to know about a link, taken in one look.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Link {
    /// The address being listened on, so it can be read out to the driver.
    pub listening_on: String,
    /// Who is sending, by the name they chose.
    pub from: Option<String>,
    /// Nothing has arrived for a while. Distinct from never having heard
    /// anybody, which is what an absent `from` says.
    pub quiet: bool,
    /// Something arrived and was not usable.
    pub trouble: Option<&'static str>,
    /// Readings a second actually arriving, measured rather than configured.
    pub rate_hz: f32,
    /// How many have been accepted since this link opened.
    pub seen: u64,
    /// How old the picture is, in milliseconds.
    pub age_ms: u32,
    /// Readings the sender numbered and this never saw.
    pub lost: u64,
}

/// The receiving half: somebody else's session, arriving.
pub struct Listener {
    socket: UdpSocket,
    buffer: Vec<u8>,
    listening_on: String,
    from: Option<String>,
    last_arrival: Option<Instant>,
    seen: u64,
    lost: u64,
    last_sequence: Option<u64>,
    rate_from: Option<(Instant, u64)>,
    rate_hz: f32,
    trouble: Option<&'static str>,
    quiet_after: Duration,
}

impl Listener {
    /// Start listening, if there is an address to listen on.
    ///
    /// `0.0.0.0:9001` hears the network; `127.0.0.1:9001` hears only this
    /// machine, which is what somebody trying it out with two copies wants.
    /// An empty address is not an error and not a link: it is nobody having
    /// asked for one.
    pub fn open(address: &str, quiet_after_s: f32) -> Result<Option<Self>, String> {
        let wanted = address.trim();
        if wanted.is_empty() {
            return Ok(None);
        }
        let socket_address = wanted
            .to_socket_addrs()
            .map_err(|error| format!("{wanted}: {error}"))?
            .next()
            .ok_or_else(|| format!("{wanted}: no address"))?;
        let socket =
            UdpSocket::bind(socket_address).map_err(|error| format!("{wanted}: {error}"))?;
        socket
            .set_nonblocking(true)
            .map_err(|error| format!("{wanted}: {error}"))?;
        let listening_on = socket
            .local_addr()
            .map(|address| address.to_string())
            .unwrap_or_else(|_| socket_address.to_string());
        Ok(Some(Self {
            socket,
            buffer: vec![0; MAX_DATAGRAM],
            listening_on,
            from: None,
            last_arrival: None,
            seen: 0,
            lost: 0,
            last_sequence: None,
            rate_from: None,
            rate_hz: 0.0,
            trouble: None,
            quiet_after: Duration::from_secs_f32(quiet_after_s.max(0.5)),
        }))
    }

    /// Everything that has arrived since the last look, in the order it was
    /// sent.
    ///
    /// **Every one of them, not the newest.** A lap's line, its traces and its
    /// splits are built out of the readings themselves, so one dropped because
    /// a fresher had arrived is a hole in the picture rather than a saving.
    pub fn poll(&mut self) -> Vec<Reading> {
        let mut arrived = Vec::new();
        while let Ok((size, _)) = self.socket.recv_from(&mut self.buffer) {
            let datagram = &self.buffer[..size];
            let Ok(envelope) = serde_json::from_slice::<Envelope>(datagram) else {
                self.trouble = Some("something arrived on this port that is not a session");
                continue;
            };
            if envelope.what != WHAT && envelope.what != WHAT_WAS {
                self.trouble = Some("something arrived on this port that is not a session");
                continue;
            }
            if envelope.schema != SCHEMA {
                self.trouble = Some("the sender speaks a different version of this protocol");
                continue;
            }
            self.trouble = None;
            // **Counted, not ordered.** A sequence that went backwards is a
            // datagram that overtook another on the way, which is normal and
            // is not worth reordering for at thirty a second; a gap is a
            // reading that never came, and that is worth saying out loud on
            // the screen that reports the link.
            if let Some(last) = self.last_sequence
                && envelope.sequence > last + 1
            {
                self.lost += envelope.sequence - last - 1;
            }
            self.last_sequence = Some(envelope.sequence);
            self.from = Some(envelope.from);
            self.seen += 1;
            self.last_arrival = Some(Instant::now());
            arrived.push(envelope.reading);
        }
        self.measure_rate();
        arrived
    }

    /// The arriving rate, over a window rather than between two frames.
    ///
    /// Two consecutive arrivals give a number that swings between 12 and 900
    /// and is unreadable; a second of them gives one somebody can act on.
    fn measure_rate(&mut self) {
        let (since, count) = *self.rate_from.get_or_insert((Instant::now(), self.seen));
        let elapsed = since.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.rate_hz = (self.seen - count) as f32 / elapsed.as_secs_f32();
            self.rate_from = Some((Instant::now(), self.seen));
        }
    }

    /// What to say about the link.
    pub fn link(&self) -> Link {
        let quiet = self
            .last_arrival
            .is_none_or(|when| when.elapsed() > self.quiet_after);
        Link {
            listening_on: self.listening_on.clone(),
            from: self.from.clone(),
            quiet,
            trouble: self.trouble,
            rate_hz: if quiet { 0.0 } else { self.rate_hz },
            seen: self.seen,
            age_ms: self
                .last_arrival
                .map(|when| when.elapsed().as_millis().min(60_000) as u32)
                .unwrap_or(0),
            lost: self.lost,
        }
    }

    /// The address actually bound, which is not always the one asked for:
    /// port zero means "any free one", and a screen has to be able to say
    /// which one it got.
    pub fn listening_on(&self) -> &str {
        &self.listening_on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The whole feature, end to end, over a real socket**: a driver's
    /// reading, sent, received, and identical on the far side — which is what
    /// makes every screen work rather than one tab.
    #[test]
    fn a_reading_sent_by_a_driver_arrives_whole() {
        let mut listener = Listener::open("127.0.0.1:0", 3.0)
            .expect("an ephemeral port")
            .expect("a link");
        let to = listener.listening_on().to_string();

        let mut reading = Reading::default();
        reading.car.speed_kmh = 213.5;
        reading.car.tyre_temp_inner_c = [95.0, 92.0, 88.0, 87.0];
        reading.session.completed_laps = 7;
        reading.session.car_position_m = [120.0, 3.0, -40.0];
        reading.session.compound = "semislick".into();
        reading.fixed.car_model = "ferrari_488_gt3_evo".to_string();
        reading.capabilities.tyre_edge_temps = true;

        let mut sender = Sender::open(&to, "Kimi", 0.0).expect("a sender");
        sender.send(&reading);

        let mut arrived = Vec::new();
        for _ in 0..50 {
            arrived = listener.poll();
            if !arrived.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }

        let heard = arrived.into_iter().next().expect("the reading that was sent");
        assert_eq!(
            heard, reading,
            "a watcher's screens are only as good as this"
        );
        assert_eq!(heard.session.compound.as_str(), "semislick");
        let link = listener.link();
        assert!(!link.quiet);
        assert_eq!(link.from.as_deref(), Some("Kimi"));
        assert_eq!(link.seen, 1);
        assert_eq!(sender.sent(), 1);
    }

    /// The coordinates are the two fields nothing else would miss: a map with
    /// no line on it is not an error anywhere, it is an empty panel.
    #[test]
    fn the_map_has_somewhere_to_draw() {
        let mut listener = Listener::open("127.0.0.1:0", 3.0)
            .expect("an ephemeral port")
            .expect("a link");
        let to = listener.listening_on().to_string();
        let mut sender = Sender::open(&to, "Kimi", 0.0).expect("a sender");

        let mut reading = Reading::default();
        reading.session.car_position_m = [120.0, 3.0, -40.0];
        reading.session.track_position = 0.42;
        sender.send(&reading);

        let mut arrived = Vec::new();
        for _ in 0..50 {
            arrived = listener.poll();
            if !arrived.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let heard = arrived.into_iter().next().expect("a reading");
        assert_eq!(heard.session.car_position_m, [120.0, 3.0, -40.0]);
        assert!((heard.session.track_position - 0.42).abs() < f32::EPSILON);
    }

    /// A rate is a rate: at ten a second, a tick every couple of milliseconds
    /// must not put out five hundred.
    #[test]
    fn the_rate_is_honoured() {
        let mut sender = Sender::open("127.0.0.1:9199", "x", 10.0).expect("a sender");
        let reading = Reading::default();
        for _ in 0..50 {
            sender.send(&reading);
        }
        assert_eq!(sender.sent(), 1, "one went out; the rest were not due");
    }

    /// Nobody asked for a link, so there is not one — and that is not an error
    /// to report on a screen.
    #[test]
    fn nothing_listens_unless_an_address_was_given() {
        assert!(Listener::open("", 3.0).expect("empty is not an error").is_none());
        assert!(Listener::open("   ", 3.0).expect("blank is blank").is_none());
    }

    /// A datagram from something else on the port is thrown away with a reason
    /// somebody can read, rather than parsed into a session of zeroes.
    #[test]
    fn somebody_elses_datagram_is_not_a_session() {
        let mut listener = Listener::open("127.0.0.1:0", 3.0)
            .expect("an ephemeral port")
            .expect("a link");
        let to: SocketAddr = listener
            .listening_on()
            .parse()
            .expect("the address it bound");
        let noise = UdpSocket::bind("0.0.0.0:0").expect("a socket to be a nuisance from");
        noise.send_to(b"{\"hello\":1}", to).expect("one datagram of nonsense");

        for _ in 0..50 {
            if listener.link().trouble.is_some() {
                break;
            }
            let _ = listener.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(listener.link().trouble.is_some(), "and it says what happened");
        assert_eq!(listener.link().seen, 0);
    }
}
