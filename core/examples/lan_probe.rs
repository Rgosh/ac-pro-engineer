//! Does the network work — and if it does not, which half.
//!
//! Sharing a session is four things that can each fail on their own: this
//! machine's addresses, the multicast group two copies find each other on, the
//! port a session arrives at, and whether anything is actually being sent. A
//! driver whose friend's screen stays empty can see none of them from inside
//! the program, so this prints all four.
//!
//! ```bash
//! # on the machine that is watching, or on both while trying it out
//! cargo run -p ac_core --example lan_probe
//!
//! # on the machine that is driving, aimed at the other one
//! cargo run -p ac_core --example lan_probe -- send 192.168.1.42:9001
//! ```
//!
//! With no arguments it announces itself, listens on the group, listens for a
//! session on the usual port, and reports for twenty seconds. That is the
//! whole of what the LAN tab does, with nothing else on screen — so a run of
//! this beside a run of the program answers "is it the network or is it me".
//!
//! **Two copies on one machine is a real test**, not a special case: the group
//! is joined on loopback as well as on every network address, precisely so
//! that somebody can try the feature on one desk before trusting it at a LAN
//! party.

use ac_core::broadcast::discovery::{Announcement, Discovery, Role, SCHEMA, WHAT, local_addresses};
use ac_core::broadcast::session::{Listener, Sender};
use ac_core::games::{Reading, Status};
use ac_core::lan::PORT;
use std::time::{Duration, Instant};

/// A reading that changes, so a watcher can see it is live rather than stuck.
fn made_up(step: u32) -> Reading {
    let round = (step % 400) as f32 / 400.0;
    let mut reading = Reading::default();
    reading.session.status = Status::Live;
    reading.session.track_position = round;
    reading.session.car_position_m = [round * 900.0, 0.0, round * -400.0];
    reading.session.completed_laps = (step / 400) as i32;
    reading.car.speed_kmh = 80.0 + round * 180.0;
    reading.car.rpm = 4000 + (round * 4000.0) as i32;
    reading.car.tyre_core_temp_c = [88.0, 87.0, 90.0, 89.0];
    reading.fixed.car_model = "lan_probe".to_string();
    reading.fixed.track = "nowhere".to_string();
    reading
}

fn send_to(target: &str) {
    let mut sender = match Sender::open(target, "lan_probe", 30.0) {
        Ok(sender) => sender,
        Err(why) => {
            eprintln!("cannot send to {target}: {why}");
            std::process::exit(1);
        }
    };
    // **Announcing as well as sending**, because that is what the program
    // does: a driver who is streaming is also the entry a friend picks out of
    // a list. Without it this half of the probe would demonstrate the stream
    // and quietly leave the half people actually have trouble with untested.
    let mut finder = Discovery::open().ok();
    println!(
        "sending to {} at 30 a second{} — ctrl-c to stop\n",
        sender.target(),
        match finder.is_some() {
            true => ", and announcing on the group",
            false => ", but this network refuses multicast",
        }
    );
    let started = Instant::now();
    let mut step = 0;
    while started.elapsed() < Duration::from_secs(60) {
        sender.send(&made_up(step));
        if let Some(finder) = finder.as_mut() {
            let mine = announcement(finder.id(), Role::Driving, 0);
            finder.poll(Some(&mine));
        }
        step += 1;
        std::thread::sleep(Duration::from_millis(5));
        if step % 200 == 0 {
            println!("  {} readings sent", sender.sent());
        }
    }
}

/// What this probe says about itself on the group.
///
/// `port` is what it listens on, and zero is honest for the sending half: it
/// has no listener, and a peer nobody can send to should say so rather than
/// offering a link that would carry nothing.
fn announcement(id: &str, role: Role, port: u16) -> Announcement {
    Announcement {
        what: WHAT.to_string(),
        schema: SCHEMA,
        id: id.to_string(),
        name: "lan_probe".to_string(),
        role,
        port,
        car: String::new(),
        track: String::new(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        front_end: "probe".to_string(),
    }
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    if let Some(first) = arguments.next() {
        let target = arguments.next().unwrap_or_else(|| format!("127.0.0.1:{PORT}"));
        if first == "send" {
            send_to(&target);
            return;
        }
        // `lan_probe 192.168.1.42:9001` means the same thing: it is what
        // somebody types first.
        send_to(&first);
        return;
    }

    println!("── this machine ─────────────────────────────────────────");
    let addresses = local_addresses();
    if addresses.is_empty() {
        println!("  no address on any network — only this machine can reach this copy");
    }
    for address in &addresses {
        println!("  {address}  — this is what a friend on the LAN sends to");
    }

    println!("\n── the port a session arrives on ────────────────────────");
    let listen = format!("0.0.0.0:{PORT}");
    let mut listener = match Listener::open(&listen, 3.0) {
        Ok(Some(listener)) => {
            println!("  listening on {listen}");
            Some(listener)
        }
        Ok(None) => None,
        Err(why) => {
            // The ordinary failure, and the one nothing else can report: the
            // program itself is already holding the port.
            println!("  cannot listen: {why}");
            println!("  something else has the port — is Pro Engineer already running?");
            None
        }
    };

    println!("\n── the group copies find each other on ──────────────────");
    let mut finder = match Discovery::open() {
        Ok(finder) => {
            println!("  joined — announcing as \"lan_probe\" every two seconds");
            Some(finder)
        }
        Err(why) => {
            println!("  {why}");
            println!("  this network refuses multicast; typing an address still works");
            None
        }
    };

    println!("\n── twenty seconds ──────────────────────────────────────");
    let started = Instant::now();
    let mut listed = 0;
    let mut reported_session = false;
    while started.elapsed() < Duration::from_secs(20) {
        if let Some(finder) = finder.as_mut() {
            let mine = announcement(finder.id(), Role::Watching, PORT);
            finder.poll(Some(&mine));
            let peers = finder.peers();
            if peers.len() != listed {
                listed = peers.len();
                println!("  {listed} copies on this network:");
                for peer in &peers {
                    println!(
                        "    {:<16} {:<22} {:<9} {} {}",
                        peer.name,
                        if peer.reachable() {
                            peer.address()
                        } else {
                            "not listening".to_string()
                        },
                        peer.role.label(),
                        peer.front_end,
                        peer.version
                    );
                }
            }
        }

        if let Some(listener) = listener.as_mut() {
            let arrived = listener.poll();
            let link = listener.link();
            if !arrived.is_empty() && !reported_session {
                reported_session = true;
                println!(
                    "  a session is arriving from {}",
                    link.from.as_deref().unwrap_or("somebody unnamed")
                );
            }
            if reported_session && link.seen % 300 == 0 && !arrived.is_empty() {
                println!(
                    "    {} readings, {:.0} a second, {} ms old, {} lost — {:.0} km/h",
                    link.seen,
                    link.rate_hz,
                    link.age_ms,
                    link.lost,
                    arrived.last().map(|r| r.car.speed_kmh).unwrap_or_default()
                );
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    println!("\n── what that means ─────────────────────────────────────");
    if listed == 0 {
        println!("  nobody else was found. Start the program, or this probe, on the");
        println!("  other machine; if that still finds nothing, the network is blocking");
        println!("  multicast and the address above has to be typed in by hand.");
    } else {
        println!("  {listed} found — the list on the LAN tab will have them too.");
    }
    if reported_session {
        println!("  and a session arrived, which is the whole path working.");
    } else {
        println!("  no session arrived on {PORT}. That is expected unless somebody was");
        println!("  sending: run `lan_probe send <this machine's address>` on the other one.");
    }
}
