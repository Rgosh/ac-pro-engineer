//! Servers on the local network, which the public lobby never lists.
//!
//! **Asked the way Content Manager and the game ask**: one UDP datagram,
//! the byte `200`, broadcast to the ports servers usually listen on. A
//! server answers from its game port with three bytes — `200` again and its
//! HTTP port, little-endian — and its `GET /INFO` is the same object the
//! lobby lists, so a server found here is a [`Listing`] like any other and
//! every screen that draws one draws it.
//!
//! The ports are Content Manager's defaults for the same search; a server on
//! a port outside them is reached by address instead. The exchange is
//! written from how Content Manager does it and held by a stand-in server
//! in the tests; it has not yet been measured against a real `acServer`.

use super::lobby::Listing;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// The ranges searched, inclusive: Content Manager's own defaults.
pub const PORTS: &[(u16, u16)] = &[(9456, 9458), (9556, 9556), (9600, 9612), (9700, 9700)];

/// What is sent, and what an answer starts with.
pub const ASK: u8 = 200;

/// Every port searched, with any others a caller knows of — the port of a
/// server this machine runs itself, say.
pub fn ports(also: &[u16]) -> Vec<u16> {
    let mut all: Vec<u16> = PORTS.iter().flat_map(|&(a, b)| a..=b).collect();
    all.extend(also.iter().copied().filter(|p| *p > 0));
    all.sort_unstable();
    all.dedup();
    all
}

/// The HTTP port in an answer, when it is one.
pub fn answer(bytes: &[u8]) -> Option<u16> {
    match bytes {
        [ASK, low, high, ..] => Some(u16::from_le_bytes([*low, *high])).filter(|p| *p > 0),
        _ => None,
    }
}

/// Every server that answers within `wait`, as its address and HTTP port.
///
/// **Blocking, so called on a thread.** Sent to the broadcast address and to
/// this machine itself — a broadcast does not always come back to the
/// machine that sent it, and the server somebody runs on their own computer
/// is the one most likely to be looked for.
pub fn find(also: &[u16], wait: Duration) -> Vec<(IpAddr, u16)> {
    find_at(
        &[Ipv4Addr::BROADCAST, Ipv4Addr::LOCALHOST],
        &ports(also),
        wait,
    )
}

/// [`find`], asking these addresses on these ports.
pub fn find_at(at: &[Ipv4Addr], ports: &[u16], wait: Duration) -> Vec<(IpAddr, u16)> {
    let Ok(socket) = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))) else {
        return Vec::new();
    };
    let _ = socket.set_broadcast(true);
    for address in at {
        for &port in ports {
            let _ = socket.send_to(&[ASK], SocketAddr::from((*address, port)));
        }
    }
    let mut found: Vec<(IpAddr, u16)> = Vec::new();
    let until = Instant::now() + wait;
    let mut buffer = [0u8; 64];
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() || socket.set_read_timeout(Some(left)).is_err() {
            break;
        }
        match socket.recv_from(&mut buffer) {
            Ok((n, from)) => {
                if let Some(http) = answer(buffer.get(..n).unwrap_or_default()) {
                    let ip = match from.ip() {
                        // Asked on loopback: the same machine, by any name.
                        IpAddr::V4(v4) if v4.is_loopback() => IpAddr::V4(Ipv4Addr::LOCALHOST),
                        ip => ip,
                    };
                    if !found.contains(&(ip, http)) {
                        found.push((ip, http));
                    }
                }
            }
            Err(_) => break,
        }
    }
    found
}

/// A server's own account of itself, read from its HTTP port.
///
/// The `/INFO` object leaves `ip` empty — the server does not know the
/// address it is reached by — so it is filled in from the one asked.
pub fn info(ip: IpAddr, http: u16, timeout: Duration) -> Result<Listing, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|why| format!("could not make a client: {why}"))?;
    let text = client
        .get(format!("http://{ip}:{http}/INFO"))
        .send()
        .and_then(|answer| answer.error_for_status())
        .and_then(|answer| answer.text())
        .map_err(|why| why.to_string())?;
    read_info(&text, ip, http).ok_or_else(|| "the server's INFO did not read".to_string())
}

/// `/INFO` as a listing, with the address it was asked at.
pub fn read_info(json: &str, ip: IpAddr, http: u16) -> Option<Listing> {
    let mut listing: Listing = serde_json::from_str(json).ok()?;
    listing.ip = ip.to_string();
    if listing.http == 0 {
        listing.http = http;
    }
    if listing.race == 0 {
        listing.race = listing.port;
    }
    Some(listing)
}

/// Every server on the local network, as listings. Blocking.
pub fn discover(also: &[u16], wait: Duration) -> Vec<Listing> {
    find(also, wait)
        .into_iter()
        .filter_map(|(ip, http)| info(ip, http, Duration::from_secs(2)).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ports_are_content_manager_s_and_an_answer_names_the_http_port() {
        let all = ports(&[9600, 9811, 0]);
        assert_eq!(all.first(), Some(&9456));
        assert!(all.contains(&9612) && all.contains(&9700) && all.contains(&9811));
        assert_eq!(all.iter().filter(|p| **p == 9600).count(), 1);
        assert_eq!(answer(&[200, 0x91, 0x1F]), Some(8081));
        assert_eq!(answer(&[200]), None);
        assert_eq!(answer(&[199, 1, 1]), None);
        assert_eq!(answer(&[200, 0, 0]), None);
    }

    /// **A stand-in server on this machine is found, and its INFO read**:
    /// a UDP socket answering the ask the way `acServer` does, and an HTTP
    /// port serving the lobby's object.
    #[test]
    fn a_server_on_this_machine_is_found_and_read() {
        use std::io::{Read, Write};
        let Ok(game) = UdpSocket::bind("127.0.0.1:0") else {
            return;
        };
        let Ok(web) = std::net::TcpListener::bind("127.0.0.1:0") else {
            return;
        };
        let game_port = game.local_addr().map(|a| a.port()).unwrap_or(0);
        let http = web.local_addr().map(|a| a.port()).unwrap_or(0);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 16];
            if let Ok((n, from)) = game.recv_from(&mut buffer)
                && buffer.get(..n) == Some(&[ASK][..])
            {
                let [low, high] = http.to_le_bytes();
                let _ = game.send_to(&[ASK, low, high], from);
            }
        });
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = web.accept() {
                let mut request = [0u8; 512];
                let _ = stream.read(&mut request);
                let body = format!(
                    r#"{{"ip":"","port":{game_port},"tport":{game_port},"cport":{http},
                       "name":"Garage server","clients":1,"maxclients":8,"track":"ks_vallelunga-club_circuit",
                       "cars":["ks_mazda_mx5_cup"],"session":1,"sessiontypes":[1,3],"durations":[1800,12],
                       "timeleft":1700,"pass":false,"pickup":true,"timed":false}}"#
                );
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let found = find_at(&[Ipv4Addr::LOCALHOST], &[game_port], Duration::from_secs(2));
        assert_eq!(found, vec![(IpAddr::V4(Ipv4Addr::LOCALHOST), http)]);
        let Ok(listing) = info(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            http,
            Duration::from_secs(2),
        ) else {
            unreachable!("the stand-in's INFO must read");
        };
        assert_eq!(listing.ip, "127.0.0.1");
        assert_eq!(listing.shown_name(), "Garage server");
        assert_eq!(listing.race, game_port);
        assert_eq!(
            listing
                .planned()
                .iter()
                .map(|p| p.length)
                .collect::<Vec<_>>(),
            vec![
                super::super::lobby::Length::Minutes(30),
                super::super::lobby::Length::Laps(12)
            ]
        );
    }
}
