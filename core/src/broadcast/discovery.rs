//! Finding the other copies of this program on the network.
//!
//! **Nobody should have to read an IP address out loud.** Typing one works and
//! is still there, but the ordinary case — two people in a house, or a driver
//! and a spare laptop — is two machines that can simply announce themselves and
//! be picked from a list.
//!
//! # Multicast to find, unicast to stream
//!
//! Discovery is a couple of hundred bytes every two seconds to a group every
//! copy joins. Telemetry is *not*: a reading goes to one address, the one
//! somebody chose. That split is the whole design and it is deliberate —
//! multicast is broadcast to a switch, so putting thirty readings a second on
//! it means every machine on the network handles the traffic whether or not it
//! wants any. Discovery is small, rare and idempotent; a stream is none of the
//! three.
//!
//! # What travels, and what does not
//!
//! An announcement says: a name somebody chose, whether they are driving or
//! watching, which port they listen on, the car, the track and the release.
//! **No telemetry, ever** — `an_announcement_says_nothing_about_the_lap` holds
//! that. It is an offer to be found, and a machine with no business here
//! should be able to read every packet on the group and learn a name and a
//! port.
//!
//! # Off is a real answer
//!
//! Announcing is a switch of its own. Somebody on a shared network — a hotel,
//! an office, a LAN party — may want to watch a friend without telling the
//! room their name, and the way to have that is not to send. It does not stop
//! them *seeing* anybody: listening is free and says nothing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

/// The group every copy joins.
///
/// 239.x.x.x is the administratively scoped range — routers do not forward it
/// off the local network, which is exactly the reach this wants: the people who
/// can already see each other's machines.
pub const GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);

/// The port announcements are sent to and heard on.
///
/// Not the port a session travels on. One is a group everybody shares; the
/// other is a socket one machine owns, and giving them the same number would
/// mean a watcher's own telemetry arriving in the discovery reader.
pub const PORT: u16 = 9002;

/// The magic an announcement carries.
pub const WHAT: &str = "pro-engineer/announce";

/// What the window called this before the two programs shared one protocol.
///
/// Accepted on the way in and never sent, for the same reason
/// [`super::session::WHAT_WAS`] is: a driver who updates one machine before
/// the other should not find that the two have stopped seeing each other.
pub const WHAT_WAS: &str = "rg-pro-engineer/announce";

/// The shape of an announcement.
pub const SCHEMA: u32 = 1;

/// How often a copy says it is here.
const EVERY: Duration = Duration::from_secs(2);

/// How long a machine stays in the list after its last announcement.
///
/// Three missed announcements. A laptop that was closed, a program that was
/// quit and a cable that was pulled all look the same from here, and all three
/// should leave the list rather than sit in it being unreachable.
const FORGET_AFTER: Duration = Duration::from_secs(7);

/// The most an announcement may be. They are a couple of hundred bytes; this is
/// the ceiling past which something is not ours.
const MAX_ANNOUNCEMENT: usize = 2048;

/// What this copy tells the network about itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Announcement {
    /// So a datagram from something else on this port is not mistaken for one
    /// of ours.
    pub what: String,
    /// Bumped when this shape changes, so an older copy is told rather than
    /// silently misread.
    pub schema: u32,
    /// Stable for as long as the program runs, so a machine that changes its
    /// name stays one entry rather than becoming two.
    pub id: String,
    /// What to show in the list.
    pub name: String,
    /// Driving, watching, or just here.
    pub role: Role,
    /// The port this copy listens for a session on, when it does. Zero is
    /// somebody nobody can send to, and a list should say as much.
    pub port: u16,
    /// What is being driven, when something is.
    pub car: String,
    pub track: String,
    /// This program's release, so a mismatch can be said out loud.
    pub version: String,
    /// Which front end this is — the terminal or the window.
    ///
    /// **Not for deciding anything**, and it must never become that: both
    /// speak the same protocol and either can watch either. It is there so a
    /// list of four entries on a LAN-party table can be read at a glance.
    #[serde(default)]
    pub front_end: String,
}

/// What a copy is doing, as far as anybody else needs to know.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Reading a game and willing to be watched.
    Driving,
    /// Listening for somebody else's session.
    Watching,
    /// Neither, but here.
    Idle,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Driving => "driving",
            Role::Watching => "watching",
            Role::Idle => "idle",
        }
    }
}

/// Somebody else, as this copy sees them.
#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub role: Role,
    /// Where to send a session: the address the datagram came from, and the
    /// port they said they listen on.
    ///
    /// **Their own idea of their address is not asked for.** A machine behind
    /// two interfaces does not know which one reached us; the source address of
    /// the packet that arrived does, because it is the one that worked.
    pub reachable_at: SocketAddr,
    pub car: String,
    pub track: String,
    pub version: String,
    pub front_end: String,
    pub last_seen: Instant,
}

impl Peer {
    /// What to put in the "send to" box.
    pub fn address(&self) -> String {
        self.reachable_at.to_string()
    }

    /// One line for a list.
    pub fn summary(&self) -> String {
        let what = match (self.car.is_empty(), self.track.is_empty()) {
            (false, false) => format!("{} at {}", self.car, self.track),
            (false, true) => self.car.clone(),
            _ => self.role.label().to_string(),
        };
        format!("{} · {}", self.address(), what)
    }

    /// Whether this peer can be sent to at all.
    ///
    /// Somebody who is not listening announces port zero. Offering to stream
    /// at them produces a link that never carries anything and a driver who
    /// cannot tell why.
    pub fn reachable(&self) -> bool {
        self.reachable_at.port() != 0
    }
}

/// The multicast half: announces this copy and collects everybody else's.
pub struct Discovery {
    socket: UdpSocket,
    /// The interfaces to announce out of: loopback, so copies on this machine
    /// find each other, and every address this machine has on the network.
    interfaces: Vec<Ipv4Addr>,
    to_group: SocketAddr,
    peers: HashMap<String, Peer>,
    last_announced: Option<Instant>,
    id: String,
    buffer: Vec<u8>,
}

impl Discovery {
    /// Join the group and start listening.
    ///
    /// `SO_REUSEADDR` before the bind, because two copies on one machine is
    /// exactly how somebody tries this out and both have to hold the same port.
    /// Failure is not fatal anywhere: a network that refuses multicast still
    /// carries a typed address, which is the path this only ever shortens.
    pub fn open() -> Result<Self, String> {
        use socket2::{Domain, Protocol, Socket, Type};

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|error| format!("cannot make a socket: {error}"))?;
        socket
            .set_reuse_address(true)
            .map_err(|error| format!("cannot share the port: {error}"))?;
        socket
            .bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, PORT)).into())
            .map_err(|error| format!("cannot bind {PORT}: {error}"))?;

        let socket: UdpSocket = socket.into();

        // **Joined on every interface, not on "any".** `INADDR_ANY` leaves the
        // choice to the routing table, which picks the one that reaches the
        // network — and two copies on *this* machine then never hear each
        // other. Measured, not guessed: with `ANY` neither of two local copies
        // saw the other; joining and announcing on loopback as well, both do,
        // and that is the case somebody tries first.
        let interfaces = interfaces();
        let joined = interfaces
            .iter()
            .filter(|address| socket.join_multicast_v4(&GROUP, address).is_ok())
            .count();
        if joined == 0 {
            return Err(format!("cannot join {GROUP} on any interface"));
        }
        socket
            .set_nonblocking(true)
            .map_err(|error| format!("cannot set non-blocking: {error}"))?;
        // **On**, and it has to be: this is how a second copy on the same
        // machine hears the first. Our own announcements come back too, and are
        // dropped by id.
        let _ = socket.set_multicast_loop_v4(true);

        Ok(Self {
            socket,
            interfaces,
            to_group: SocketAddrV4::new(GROUP, PORT).into(),
            peers: HashMap::new(),
            last_announced: None,
            id: fresh_id(),
            buffer: vec![0; MAX_ANNOUNCEMENT],
        })
    }

    /// This copy's own id, so its own announcements can be ignored.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Say we are here, if it is time, and take in whatever has arrived.
    ///
    /// `announce` is `None` for somebody who does not want to be found: they
    /// still see everybody else, which is the asymmetry a shared network wants.
    pub fn poll(&mut self, announce: Option<&Announcement>) {
        if let Some(mine) = announce {
            let due = self
                .last_announced
                .is_none_or(|when| when.elapsed() >= EVERY);
            if due && let Ok(bytes) = serde_json::to_vec(mine) {
                // One datagram per interface. A couple of hundred bytes every
                // two seconds each, and the alternative is choosing between
                // being found on this machine and being found on the network.
                for address in &self.interfaces {
                    let _ = set_interface(&self.socket, address);
                    let _ = self.socket.send_to(&bytes, self.to_group);
                }
                self.last_announced = Some(Instant::now());
            }
        }

        while let Ok((size, from)) = self.socket.recv_from(&mut self.buffer) {
            let datagram = self.buffer[..size].to_vec();
            self.accept(&datagram, from);
        }

        self.peers
            .retain(|_, peer| peer.last_seen.elapsed() < FORGET_AFTER);
    }

    /// Take one datagram. Public for the tests, which need no network to
    /// exercise the part that decides anything.
    pub fn accept(&mut self, datagram: &[u8], from: SocketAddr) {
        let Ok(heard) = serde_json::from_slice::<Announcement>(datagram) else {
            return;
        };
        if (heard.what != WHAT && heard.what != WHAT_WAS)
            || heard.schema != SCHEMA
            || heard.id == self.id
        {
            return;
        }
        // Their port, our idea of their address: the one the packet came from
        // is the one that reached us.
        let reachable_at = SocketAddr::new(from.ip(), heard.port);
        self.peers.insert(
            heard.id.clone(),
            Peer {
                id: heard.id,
                name: heard.name,
                role: heard.role,
                reachable_at,
                car: heard.car,
                track: heard.track,
                version: heard.version,
                front_end: heard.front_end,
                last_seen: Instant::now(),
            },
        );
    }

    /// Everybody heard from lately, driving first and then by name — a list
    /// that reorders itself while somebody is reading it is a list nobody can
    /// choose from.
    pub fn peers(&self) -> Vec<Peer> {
        let mut found: Vec<Peer> = self.peers.values().cloned().collect();
        found.sort_by(|a, b| {
            let rank = |role: Role| match role {
                Role::Driving => 0,
                Role::Watching => 1,
                Role::Idle => 2,
            };
            rank(a.role)
                .cmp(&rank(b.role))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.id.cmp(&b.id))
        });
        found
    }
}

/// Every address this machine has on a network, loopback aside.
///
/// **Asked of the routing table rather than of an interface list**: connecting
/// a UDP socket sends nothing and tells the kernel to choose the source
/// address it would use to reach that destination, which is the one a peer
/// will see. Two well-known private ranges and one public address, so the
/// answer is the interface a home network actually uses rather than whichever
/// happens to be listed first.
pub fn local_addresses() -> Vec<String> {
    ["10.255.255.255:1", "192.168.255.255:1", "8.8.8.8:80"]
        .iter()
        .filter_map(|probe| {
            let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
            socket.connect(probe).ok()?;
            let address = socket.local_addr().ok()?.ip();
            (!address.is_loopback() && !address.is_unspecified()).then(|| address.to_string())
        })
        .fold(Vec::new(), |mut found, address| {
            if !found.contains(&address) {
                found.push(address);
            }
            found
        })
}

/// Which interfaces to join and announce on.
///
/// Loopback first, because two copies on one machine is how somebody tries
/// this out and it has to work before any network does; then whatever
/// addresses this machine has on the network.
fn interfaces() -> Vec<Ipv4Addr> {
    let mut found = vec![Ipv4Addr::LOCALHOST];
    for address in local_addresses() {
        if let Ok(parsed) = address.parse::<Ipv4Addr>()
            && !found.contains(&parsed)
        {
            found.push(parsed);
        }
    }
    found
}

/// Choose the interface the next multicast send leaves by.
///
/// `std` has no such option and `socket2` does — the whole of why that
/// dependency is here. The socket is borrowed, never owned: building a
/// `socket2::Socket` that owns the descriptor and dropping it would close the
/// socket this is about to send on.
#[cfg(unix)]
fn set_interface(socket: &UdpSocket, address: &Ipv4Addr) -> std::io::Result<()> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let borrowed =
        std::mem::ManuallyDrop::new(unsafe { socket2::Socket::from_raw_fd(socket.as_raw_fd()) });
    borrowed.set_multicast_if_v4(address)
}

#[cfg(windows)]
fn set_interface(socket: &UdpSocket, address: &Ipv4Addr) -> std::io::Result<()> {
    use std::os::windows::io::{AsRawSocket, FromRawSocket};
    let borrowed = std::mem::ManuallyDrop::new(unsafe {
        socket2::Socket::from_raw_socket(socket.as_raw_socket())
    });
    borrowed.set_multicast_if_v4(address)
}

/// An id that is stable for this run and unlike anybody else's.
///
/// The clock and the process, which is enough: it has to survive a rename, not
/// a determined collision.
fn fresh_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);
    format!("{:x}-{:x}", std::process::id(), now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn announcement(id: &str, name: &str, role: Role, port: u16) -> Announcement {
        Announcement {
            what: WHAT.to_string(),
            schema: SCHEMA,
            id: id.to_string(),
            name: name.to_string(),
            role,
            port,
            car: "bmw_z4_gt3".to_string(),
            track: "spa".to_string(),
            version: "0.4.5".to_string(),
            front_end: "terminal".to_string(),
        }
    }

    /// A discovery that never touches the group, for the half that decides
    /// things. The network is exercised by the round trip at the end.
    fn quiet() -> Discovery {
        Discovery {
            socket: UdpSocket::bind("127.0.0.1:0").expect("an ephemeral socket"),
            interfaces: vec![Ipv4Addr::LOCALHOST],
            to_group: SocketAddrV4::new(GROUP, PORT).into(),
            peers: HashMap::new(),
            last_announced: None,
            id: "me".to_string(),
            buffer: vec![0; MAX_ANNOUNCEMENT],
        }
    }

    fn from(address: &str) -> SocketAddr {
        address.parse().expect("a well-formed address")
    }

    /// **Where to send is where it came from.** A machine behind two
    /// interfaces does not know which of its addresses reached us; the source
    /// of the packet does, because it is the one that worked.
    #[test]
    fn a_peer_is_reachable_where_its_packet_came_from() {
        let mut discovery = quiet();
        let heard = announcement("them", "Kimi", Role::Driving, 9001);
        discovery.accept(
            &serde_json::to_vec(&heard).expect("serialise"),
            from("192.168.1.42:51000"),
        );

        let peers = discovery.peers();
        assert_eq!(peers.len(), 1);
        // Their listening port, our idea of their address — not the ephemeral
        // port their announcement happened to leave from.
        assert_eq!(peers[0].address(), "192.168.1.42:9001");
        assert_eq!(peers[0].name, "Kimi");
        assert!(peers[0].reachable());
    }

    /// Somebody who is not listening cannot be sent to, and the list has to be
    /// able to say so rather than offering a link that carries nothing.
    #[test]
    fn a_peer_with_no_port_is_not_reachable() {
        let mut discovery = quiet();
        discovery.accept(
            &serde_json::to_vec(&announcement("them", "Ann", Role::Idle, 0)).expect("serialise"),
            from("192.168.1.42:51000"),
        );
        assert!(!discovery.peers()[0].reachable());
    }

    #[test]
    fn our_own_announcement_is_not_somebody_else() {
        let mut discovery = quiet();
        let mine = announcement("me", "me", Role::Driving, 9001);
        discovery.accept(
            &serde_json::to_vec(&mine).expect("serialise"),
            from("127.0.0.1:51000"),
        );
        assert!(
            discovery.peers().is_empty(),
            "a copy that lists itself offers to stream to itself"
        );
    }

    /// The window before v0.4.5 announced under its own name. A driver who
    /// updates one machine first still finds the other.
    #[test]
    fn the_name_the_window_used_before_is_still_heard() {
        let mut discovery = quiet();
        let mut older = announcement("them", "Kimi", Role::Driving, 9001);
        older.what = WHAT_WAS.to_string();
        discovery.accept(
            &serde_json::to_vec(&older).expect("serialise"),
            from("192.168.1.42:51000"),
        );
        assert_eq!(discovery.peers().len(), 1);
    }

    /// Anything else on the port, and any release that speaks a different
    /// shape, is ignored rather than half-read.
    #[test]
    fn a_datagram_that_is_not_ours_is_ignored() {
        let mut discovery = quiet();
        discovery.accept(b"hello?", from("192.168.1.9:5000"));
        discovery.accept(b"{\"what\":\"something-else\"}", from("192.168.1.9:5000"));

        let mut wrong_schema = announcement("them", "Kimi", Role::Driving, 9001);
        wrong_schema.schema = SCHEMA + 1;
        discovery.accept(
            &serde_json::to_vec(&wrong_schema).expect("serialise"),
            from("192.168.1.9:5000"),
        );

        assert!(discovery.peers().is_empty());
    }

    #[test]
    fn hearing_from_somebody_twice_keeps_one_entry() {
        let mut discovery = quiet();
        let first = announcement("them", "Kimi", Role::Driving, 9001);
        let mut renamed = first.clone();
        renamed.name = "Kimi Räikkönen".to_string();

        discovery.accept(
            &serde_json::to_vec(&first).expect("serialise"),
            from("192.168.1.42:51000"),
        );
        discovery.accept(
            &serde_json::to_vec(&renamed).expect("serialise"),
            from("192.168.1.42:51000"),
        );

        let peers = discovery.peers();
        assert_eq!(
            peers.len(),
            1,
            "an id is what identifies somebody, not a name"
        );
        assert_eq!(peers[0].name, "Kimi Räikkönen");
    }

    /// Drivers first: the list is read by somebody looking for a session to
    /// watch, and everybody else is noise until they are not.
    #[test]
    fn the_list_puts_drivers_first() {
        let mut discovery = quiet();
        for (id, name, role) in [
            ("c", "Zoe", Role::Idle),
            ("b", "Ann", Role::Watching),
            ("a", "Yuri", Role::Driving),
        ] {
            discovery.accept(
                &serde_json::to_vec(&announcement(id, name, role, 9001)).expect("serialise"),
                from("192.168.1.42:51000"),
            );
        }
        let names: Vec<String> = discovery.peers().into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["Yuri", "Ann", "Zoe"]);
    }

    /// **An announcement carries nothing about anybody's driving.** A machine
    /// with no business here should be able to read every packet on the group
    /// and learn a name, a role and a port.
    #[test]
    fn an_announcement_says_nothing_about_the_lap() {
        let text = serde_json::to_string(&announcement("them", "Kimi", Role::Driving, 9001))
            .expect("serialise");
        for secret in ["speed", "lap_time", "tyre", "fuel", "advice", "brake"] {
            assert!(
                !text.contains(secret),
                "an announcement carried {secret}: {text}"
            );
        }
    }

    /// The one test that uses a real socket: a group, two copies, and one of
    /// them finding the other.
    ///
    /// Multicast needs a route, and a machine in a container may not have one —
    /// so a failure to join is skipped rather than failed. What must not happen
    /// is joining and then not hearing.
    #[test]
    fn two_copies_on_one_machine_find_each_other() {
        let (mut driver, mut watcher) = match (Discovery::open(), Discovery::open()) {
            (Ok(one), Ok(two)) => (one, two),
            // No multicast here. Typing an address still works, which is the
            // path this only ever shortens.
            _ => return,
        };

        let mut mine = announcement(driver.id(), "Kimi", Role::Driving, 9001);
        mine.id = driver.id().to_string();

        for _ in 0..40 {
            driver.poll(Some(&mine));
            watcher.poll(None);
            if !watcher.peers().is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
            // `poll` only announces every couple of seconds; the loop above
            // would otherwise wait one out.
            driver.last_announced = None;
        }

        let found = watcher.peers();
        assert_eq!(found.len(), 1, "the driver announced and was not found");
        assert_eq!(found[0].name, "Kimi");
        assert_eq!(found[0].reachable_at.port(), 9001);
    }
}
