//! Subscribing to the frame feed, with nothing to configure.
//!
//! **A program that wants the engineer's numbers asks for them.** Until this,
//! a dashboard, a stream overlay or a button box written against
//! [`udp`](super::udp) needed the driver to type its address into
//! `overlay.broadcast_to` — one address, set by hand, in a file the program
//! writing the dashboard does not own. That is the step at which people gave
//! up and read the game's memory themselves, re-implementing what the core
//! already worked out.
//!
//! Now the running program listens on [`PORT`]. A client sends one datagram:
//!
//! ```json
//! {"magic":"acpe","subscribe":{"hz":20,"name":"my dashboard"}}
//! ```
//!
//! and gets an answer back to the address it sent from —
//!
//! ```json
//! {"magic":"acpe","subscribed":true,"schema":1,"hz":20,"lease_s":10}
//! ```
//!
//! — then the same JSON messages [`udp`](super::udp) sends, at the rate it
//! asked for. It repeats the request every few seconds while it wants more
//! (a **lease**, [`LEASE`] long), so a client that crashes or closes is
//! dropped by itself and nothing has to say goodbye; one that leaves politely
//! sends `{"magic":"acpe","unsubscribe":true}`. A plain-text request,
//! `acpe subscribe 20`, works too, for a shell and `nc`.
//!
//! **Who may ask.** This machine always: what is sent never leaves it. The
//! local network only when the driver allowed it ([`Subscriptions::open`]'s
//! `allow_lan`) — telemetry about a person crosses a wire because they said
//! so. The internet never: a public address is not even answered, because an
//! answer to a forged sender is traffic aimed at somebody else. At most
//! [`MOST`] subscribers at once.

use super::udp::UdpSink;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// The port the running program listens on for subscriptions.
pub const PORT: u16 = 9003;

/// How long a subscription lasts without being asked for again.
pub const LEASE: Duration = Duration::from_secs(10);

/// The most subscribers at once.
pub const MOST: usize = 16;

/// The fastest a subscriber may ask for, in frames a second.
pub const FASTEST_HZ: f32 = 60.0;

/// What a request asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Frames, at this rate (0 is every frame, up to [`FASTEST_HZ`]), and
    /// what the client calls itself.
    Subscribe {
        hz: f32,
        name: String,
    },
    Unsubscribe,
}

#[derive(Deserialize)]
struct Asked {
    magic: String,
    #[serde(default)]
    subscribe: Option<Wants>,
    #[serde(default)]
    unsubscribe: bool,
}

#[derive(Deserialize, Default)]
struct Wants {
    #[serde(default)]
    hz: Option<f32>,
    #[serde(default)]
    name: Option<String>,
}

/// The rate a subscriber gets when it does not say: what a dashboard needs
/// and a spectator cannot tell apart from more.
pub const DEFAULT_HZ: f32 = 20.0;

/// Read a request: JSON, or `acpe subscribe [hz]` / `acpe unsubscribe`.
pub fn parse(datagram: &[u8]) -> Option<Request> {
    let text = std::str::from_utf8(datagram).ok()?.trim();
    let clamp = |hz: f32| match hz.is_finite() && hz > 0.0 {
        true => hz.min(FASTEST_HZ),
        false => DEFAULT_HZ,
    };
    if text.starts_with('{') {
        let asked: Asked = serde_json::from_str(text).ok()?;
        if asked.magic != super::udp::MAGIC {
            return None;
        }
        if asked.unsubscribe {
            return Some(Request::Unsubscribe);
        }
        let wants = asked.subscribe?;
        let name: String = wants
            .name
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_control())
            .take(48)
            .collect();
        return Some(Request::Subscribe {
            hz: clamp(wants.hz.unwrap_or(DEFAULT_HZ)),
            name,
        });
    }
    let mut words = text.split_whitespace();
    if words.next()? != super::udp::MAGIC {
        return None;
    }
    match words.next()? {
        "subscribe" => Some(Request::Subscribe {
            hz: clamp(
                words
                    .next()
                    .and_then(|hz| hz.parse().ok())
                    .unwrap_or(DEFAULT_HZ),
            ),
            name: String::new(),
        }),
        "unsubscribe" => Some(Request::Unsubscribe),
        _ => None,
    }
}

/// Whether an address may subscribe: this machine always, the local network
/// when allowed, nothing else.
pub fn may_subscribe(address: IpAddr, allow_lan: bool) -> bool {
    match address {
        IpAddr::V4(v4) => {
            v4.is_loopback() || (allow_lan && (v4.is_private() || v4.is_link_local()))
        }
        IpAddr::V6(v6) => {
            let local_net =
                (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80;
            v6.is_loopback()
                || (allow_lan && local_net)
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| may_subscribe(IpAddr::V4(v4), allow_lan))
        }
    }
}

/// A change to the sinks, for the broadcaster to make.
pub enum Change {
    /// A new subscriber: its sink, ready to add.
    Add(UdpSink),
    /// Drop the sink with this name.
    Drop(String),
}

/// The listening socket and who is subscribed.
pub struct Subscriptions {
    socket: UdpSocket,
    allow_lan: bool,
    game: String,
    driver: String,
    /// Each subscriber's address and when its lease runs out.
    leases: HashMap<SocketAddr, Instant>,
}

/// What every subscriber's sink is called, before its address.
pub const SINK_PREFIX: &str = "subscriber ";

/// What a subscriber's sink is called.
pub fn sink_name(address: SocketAddr) -> String {
    format!("{SINK_PREFIX}{address}")
}

impl Subscriptions {
    /// Listen for subscriptions: on every interface when the local network
    /// may subscribe, on this machine's loopback otherwise.
    pub fn open(
        port: u16,
        allow_lan: bool,
        game: impl Into<String>,
        driver: impl Into<String>,
    ) -> std::io::Result<Self> {
        let host = match allow_lan {
            true => "0.0.0.0",
            false => "127.0.0.1",
        };
        let socket = UdpSocket::bind((host, port))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            allow_lan,
            game: game.into(),
            driver: driver.into(),
            leases: HashMap::new(),
        })
    }

    /// Where it listens.
    pub fn listening_on(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// How many are subscribed.
    pub fn count(&self) -> usize {
        self.leases.len()
    }

    /// Take every request waiting, answer each, and say what changes.
    /// Never blocks.
    ///
    /// `has` says whether a sink of that name is still live: a subscriber's
    /// sink the broadcaster dropped for failing is made again when it renews,
    /// rather than leaving a lease with nothing behind it.
    pub fn poll(&mut self, now: Instant, has: impl Fn(&str) -> bool) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut buffer = [0u8; 2048];
        // A bounded number a call: a flood cannot hold the tick.
        for _ in 0..64 {
            let Ok((read, from)) = self.socket.recv_from(&mut buffer) else {
                break;
            };
            let Some(request) = buffer.get(..read).and_then(parse) else {
                continue;
            };
            let refuse = |socket: &UdpSocket, why: &str| {
                let answer = serde_json::json!({
                    "magic": super::udp::MAGIC, "subscribed": false, "reason": why,
                });
                let _ = socket.send_to(answer.to_string().as_bytes(), from);
            };
            match request {
                Request::Unsubscribe => {
                    if self.leases.remove(&from).is_some() {
                        changes.push(Change::Drop(sink_name(from)));
                    }
                }
                Request::Subscribe { hz, name } => {
                    if !may_subscribe(from.ip(), self.allow_lan) {
                        // Only an address that could have been allowed hears
                        // why not: a reply to a public one would be a reply
                        // to whoever's address a forged datagram carried.
                        if may_subscribe(from.ip(), true) {
                            refuse(
                                &self.socket,
                                "only this machine may subscribe, or the local network when the driver allows it",
                            );
                        }
                        continue;
                    }
                    let renewing = self.leases.contains_key(&from);
                    if !renewing && self.leases.len() >= MOST {
                        refuse(&self.socket, "too many subscribers");
                        continue;
                    }
                    self.leases.insert(from, now + LEASE);
                    if !renewing || !has(&sink_name(from)) {
                        match UdpSink::new(from, self.game.clone(), self.driver.clone(), hz) {
                            Ok(sink) => {
                                tracing::info!(%from, name, hz, "A subscriber asked for frames");
                                changes.push(Change::Add(sink.named(sink_name(from))));
                            }
                            Err(_) => {
                                self.leases.remove(&from);
                                continue;
                            }
                        }
                    }
                    let answer = serde_json::json!({
                        "magic": super::udp::MAGIC,
                        "subscribed": true,
                        "schema": super::udp::SCHEMA_VERSION,
                        "hz": hz,
                        "lease_s": LEASE.as_secs(),
                    });
                    let _ = self.socket.send_to(answer.to_string().as_bytes(), from);
                }
            }
        }
        // Leases that ran out: the subscriber went without saying.
        let expired: Vec<SocketAddr> = self
            .leases
            .iter()
            .filter(|(_, until)| **until <= now)
            .map(|(address, _)| *address)
            .collect();
        for address in expired {
            self.leases.remove(&address);
            changes.push(Change::Drop(sink_name(address)));
        }
        changes
    }
}

/// What a [`Client`] knows about its subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum Standing {
    /// Asked, and no answer yet — the program is not running, or not
    /// listening for subscriptions.
    Asking,
    /// Subscribed, at this rate.
    Subscribed { hz: f32 },
    /// Refused, and why.
    Refused(String),
}

/// The other end, for a program written in Rust: subscribe, keep the lease,
/// and read the newest message.
///
/// ```no_run
/// use ac_core::broadcast::subscribe::Client;
/// let mut feed = Client::local(20.0, "my dashboard")?;
/// loop {
///     if let Some(message) = feed.poll() {
///         println!("{:.0} km/h, gear {}", message.speed_kmh, message.gear);
///     }
///     std::thread::sleep(std::time::Duration::from_millis(16));
/// }
/// # Ok::<(), std::io::Error>(())
/// ```
///
/// Never blocks: [`Client::poll`] takes what has arrived and returns, so it
/// sits in a drawing loop as it is. It asks again every third of a lease,
/// which is also what finds the program when it starts after the client.
pub struct Client {
    socket: UdpSocket,
    server: SocketAddr,
    hz: f32,
    name: String,
    asked_at: Option<Instant>,
    standing: Standing,
    last_sequence: Option<u32>,
    last_heard: Option<Instant>,
    buffer: Vec<u8>,
}

impl Client {
    /// Subscribe to the program on this machine.
    pub fn local(hz: f32, name: &str) -> std::io::Result<Self> {
        Self::to(SocketAddr::from(([127, 0, 0, 1], PORT)), hz, name)
    }

    /// Subscribe to the program at `server` — another machine on the local
    /// network, when its driver allowed that.
    pub fn to(server: SocketAddr, hz: f32, name: &str) -> std::io::Result<Self> {
        let host: IpAddr = match server.ip() {
            IpAddr::V4(v4) if v4.is_loopback() => [127, 0, 0, 1].into(),
            IpAddr::V4(_) => [0, 0, 0, 0].into(),
            IpAddr::V6(_) => std::net::Ipv6Addr::UNSPECIFIED.into(),
        };
        let socket = UdpSocket::bind((host, 0))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            server,
            hz,
            name: name.to_string(),
            asked_at: None,
            standing: Standing::Asking,
            last_sequence: None,
            last_heard: None,
            buffer: vec![0; 65_535],
        })
    }

    /// Where the subscription stands.
    pub fn standing(&self) -> &Standing {
        &self.standing
    }

    /// How long since a message last arrived, when one has.
    pub fn quiet_for(&self) -> Option<Duration> {
        self.last_heard.map(|at| at.elapsed())
    }

    /// Ask again if a third of the lease has gone, then take the newest
    /// message waiting.
    pub fn poll(&mut self) -> Option<super::udp::Message> {
        let now = Instant::now();
        if self
            .asked_at
            .is_none_or(|at| now.duration_since(at) >= LEASE / 3)
        {
            let ask = serde_json::json!({
                "magic": super::udp::MAGIC,
                "subscribe": { "hz": self.hz, "name": self.name },
            });
            // Nothing listening yet is not an error: the program may start
            // after this does, and the next ask finds it.
            let _ = self.socket.send_to(ask.to_string().as_bytes(), self.server);
            self.asked_at = Some(now);
        }
        let mut newest = None;
        while let Ok((read, from)) = self.socket.recv_from(&mut self.buffer) {
            let Some(datagram) = self.buffer.get(..read) else {
                break;
            };
            // The answer comes from the port asked; the frames from another
            // socket of the same program, so only the machine is checked.
            if from.ip() != self.server.ip() && !from.ip().is_loopback() {
                continue;
            }
            if let Ok(answer) = serde_json::from_slice::<Answer>(datagram)
                && answer.magic == super::udp::MAGIC
                && let Some(subscribed) = answer.subscribed
            {
                self.standing = match subscribed {
                    true => Standing::Subscribed {
                        hz: answer.hz.unwrap_or(self.hz),
                    },
                    false => Standing::Refused(answer.reason.unwrap_or_default()),
                };
                continue;
            }
            if let Ok(message) = super::receiver::decode(datagram) {
                if self
                    .last_sequence
                    .is_some_and(|last| message.sequence < last)
                {
                    continue;
                }
                self.last_sequence = Some(message.sequence);
                self.last_heard = Some(now);
                newest = Some(message);
            }
        }
        newest
    }

    /// Say goodbye, so the program stops sending now rather than when the
    /// lease runs out. Also done on drop.
    pub fn leave(&mut self) {
        let bye = serde_json::json!({ "magic": super::udp::MAGIC, "unsubscribe": true });
        let _ = self.socket.send_to(bye.to_string().as_bytes(), self.server);
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.leave();
    }
}

#[derive(Deserialize)]
struct Answer {
    magic: String,
    #[serde(default)]
    subscribed: Option<bool>,
    #[serde(default)]
    hz: Option<f32>,
    #[serde(default)]
    reason: Option<String>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn a_request_reads_as_json_or_as_words() {
        assert_eq!(
            parse(br#"{"magic":"acpe","subscribe":{"hz":30,"name":"dash\nboard"}}"#),
            Some(Request::Subscribe {
                hz: 30.0,
                name: "dashboard".into()
            })
        );
        assert_eq!(
            parse(br#"{"magic":"acpe","subscribe":{}}"#),
            Some(Request::Subscribe {
                hz: DEFAULT_HZ,
                name: String::new()
            })
        );
        assert_eq!(
            parse(br#"{"magic":"acpe","subscribe":{"hz":1000}}"#),
            Some(Request::Subscribe {
                hz: FASTEST_HZ,
                name: String::new()
            })
        );
        assert_eq!(
            parse(b"acpe subscribe 5"),
            Some(Request::Subscribe {
                hz: 5.0,
                name: String::new()
            })
        );
        assert_eq!(parse(b"acpe unsubscribe\n"), Some(Request::Unsubscribe));
        assert_eq!(
            parse(br#"{"magic":"acpe","unsubscribe":true}"#),
            Some(Request::Unsubscribe)
        );
        for not in [
            &b"hello"[..],
            b"{}",
            br#"{"magic":"other","subscribe":{}}"#,
            b"acpe dance",
            b"\xff\xfe",
        ] {
            assert_eq!(parse(not), None);
        }
    }

    #[test]
    fn this_machine_always_the_network_when_allowed_the_internet_never() {
        let v4 = |a, b, c, d| IpAddr::V4(Ipv4Addr::new(a, b, c, d));
        assert!(may_subscribe(v4(127, 0, 0, 1), false));
        assert!(!may_subscribe(v4(192, 168, 1, 20), false));
        assert!(may_subscribe(v4(192, 168, 1, 20), true));
        assert!(may_subscribe(v4(10, 0, 0, 5), true));
        assert!(!may_subscribe(v4(8, 8, 8, 8), true));
        assert!(may_subscribe(IpAddr::V6(Ipv6Addr::LOCALHOST), false));
        assert!(!may_subscribe(
            IpAddr::V6("2001:db8::1".parse().unwrap()),
            true
        ));
        assert!(may_subscribe(IpAddr::V6("fe80::1".parse().unwrap()), true));
    }

    /// **The whole exchange on this machine**: subscribe, the answer, a
    /// renewal that adds nothing, a goodbye, and a lease that runs out.
    #[test]
    fn a_client_subscribes_renews_leaves_and_lapses() {
        let Ok(mut subs) = Subscriptions::open(0, false, "assetto_corsa", "Driver") else {
            return;
        };
        let at = subs.listening_on().unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        let start = Instant::now();
        let ask = |text: &[u8]| {
            client.send_to(text, at).unwrap();
            std::thread::sleep(Duration::from_millis(30));
        };

        ask(br#"{"magic":"acpe","subscribe":{"hz":10,"name":"test"}}"#);
        let changes = subs.poll(start, |_| false);
        assert!(matches!(changes.as_slice(), [Change::Add(_)]));
        let mut answer = [0u8; 512];
        let read = client.recv(&mut answer).unwrap();
        let said: serde_json::Value = serde_json::from_slice(&answer[..read]).unwrap();
        assert_eq!(said["subscribed"], true);
        assert_eq!(said["lease_s"], LEASE.as_secs());
        assert_eq!(subs.count(), 1);

        // A renewal keeps the one sink.
        ask(b"acpe subscribe 10");
        assert!(
            subs.poll(start + Duration::from_secs(5), |_| true)
                .is_empty()
        );
        let _ = client.recv(&mut answer);

        // A renewal after the broadcaster dropped the sink makes it again.
        ask(b"acpe subscribe 10");
        assert!(matches!(
            subs.poll(start + Duration::from_secs(5), |_| false)
                .as_slice(),
            [Change::Add(_)]
        ));
        let _ = client.recv(&mut answer);

        // Leaving says so.
        ask(br#"{"magic":"acpe","unsubscribe":true}"#);
        assert!(matches!(
            subs.poll(start + Duration::from_secs(6), |_| true)
                .as_slice(),
            [Change::Drop(_)]
        ));
        assert_eq!(subs.count(), 0);

        // Subscribed again and gone quiet: dropped when the lease runs out.
        ask(b"acpe subscribe");
        assert_eq!(
            subs.poll(start + Duration::from_secs(7), |_| false).len(),
            1
        );
        let _ = client.recv(&mut answer);
        let lapsed = subs.poll(
            start + Duration::from_secs(7) + LEASE + Duration::from_secs(1),
            |_| true,
        );
        assert!(matches!(lapsed.as_slice(), [Change::Drop(_)]));
    }

    /// **End to end, as somebody else's program sees it**: a client asks,
    /// the broadcaster answers and sends frames at the next publish, and a
    /// client that leaves is no longer sent anything.
    #[test]
    fn a_client_asks_and_the_frames_arrive() {
        let mut broadcaster = super::super::Broadcaster::new();
        let Ok(at) = broadcaster.accept_subscriptions(0, false, "assetto_corsa", "Driver") else {
            return;
        };
        let mut client = Client::to(at, 0.0, "test").unwrap();
        let mut frame = crate::overlay::frame::OverlayFrame::empty();
        frame.speed_kmh = 214.0;

        let mut got = None;
        for sequence in 1..200u32 {
            frame.sequence = sequence;
            broadcaster.publish(&frame);
            if let Some(message) = client.poll() {
                got = Some(message);
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let message = got.expect("a frame arrives after subscribing");
        assert_eq!(message.speed_kmh, 214.0);
        assert_eq!(message.game, "assetto_corsa");
        assert!(matches!(client.standing(), Standing::Subscribed { .. }));
        assert_eq!(broadcaster.subscribers(), Some(1));
        assert!(
            broadcaster
                .sink_names()
                .iter()
                .any(|name| name.starts_with(SINK_PREFIX))
        );

        client.leave();
        std::thread::sleep(Duration::from_millis(30));
        broadcaster.publish(&frame);
        assert_eq!(broadcaster.subscribers(), Some(0));
        assert!(broadcaster.sink_names().is_empty());

        broadcaster.stop_subscriptions();
        assert!(broadcaster.is_empty());
    }
}
