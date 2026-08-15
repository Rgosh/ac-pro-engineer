//! Send a frame to yourself and print what came back out.
//!
//! `broadcast_to` and `receive_from` are the feature a driver uses to let a
//! friend watch, and the thing worth checking is not that a datagram arrived —
//! it is that **the engineer's sentences survive the trip**. A spectator who
//! gets numbers has a dashboard; one who gets "T3 cost 0.34 s — 14 m late on
//! the brakes" has an engineer, and that is the whole claim.
//!
//! So this runs the real engineer against a crafted physics state, publishes
//! the frame through the real [`UdpSink`], reads it with the real
//! [`FrameReceiver`], and prints the advice off the far end.
//!
//! ```bash
//! cargo run -p ac_core --example share_probe
//! ```
//!
//! Loopback by default, which proves everything except the network. That is
//! deliberate: the wire is the part that does not need testing, and a probe
//! that needs two machines is a probe nobody runs. Point it somewhere else to
//! test a real link — a machine on the LAN, or a Tailscale address to check the
//! path across the internet:
//!
//! ```bash
//! cargo run -p ac_core --example share_probe -- 100.64.0.2:9001
//! ```
//!
//! With an address given it only sends, since the far end is doing the
//! listening. Run it there with no argument to watch.

use ac_core::broadcast::Sink;
use ac_core::broadcast::receiver::{FrameReceiver, Received};
use ac_core::broadcast::udp::UdpSink;
use ac_core::config::AppConfig;
use ac_core::engineer::Engineer;
use ac_core::games::{Car, Session};
use ac_core::overlay::frame::{MESSAGE_SLOTS, OverlayFrame, flags};
use ac_core::session_info::SessionInfo;
use std::net::SocketAddr;
use std::thread::sleep;
use std::time::Duration;

/// A car in enough trouble that several rules have something to say.
fn troubled_car() -> Car {
    Car {
        tyre_pressure_psi: [31.0, 31.2, 30.8, 31.1],
        tyre_core_temp_c: [120.0; 4],
        tyre_temp_inner_c: [124.0; 4],
        tyre_temp_middle_c: [120.0; 4],
        tyre_temp_outer_c: [112.0; 4],
        brake_temp_c: [900.0, 910.0, 880.0, 895.0],
        tyre_wear: [80.0, 81.0, 79.0, 82.0],
        speed_kmh: 180.0,
        fuel_litres: 4.0,
        ..Default::default()
    }
}

fn advice_of(frame: &OverlayFrame) -> Vec<String> {
    (0..MESSAGE_SLOTS)
        .filter_map(|slot| {
            let bytes = &frame.messages[slot];
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            let text = String::from_utf8_lossy(&bytes[..end]).to_string();
            (!text.is_empty()).then_some(text)
        })
        .collect()
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let target: SocketAddr = match arguments.next() {
        Some(given) => given.parse().unwrap_or_else(|error| {
            eprintln!("{given} is not an address: {error}");
            std::process::exit(1);
        }),
        None => "127.0.0.1:9001"
            .parse()
            .unwrap_or_else(|_| unreachable!("a literal address")),
    };
    let loopback = target.ip().is_loopback();

    // Only bind a receiver when we are also the far end. Binding the port a
    // remote receiver is listening on would take it from them.
    let mut receiver = loopback.then(|| {
        FrameReceiver::bind(target).unwrap_or_else(|error| {
            eprintln!("cannot listen on {target}: {error}");
            eprintln!("something else is using the port — is the application running?");
            std::process::exit(1);
        })
    });

    let config = AppConfig::default();
    let mut engineer = Engineer::new(&config);
    // The car below is an Assetto Corsa car, so the engineer is told what that
    // game measures — otherwise the tyre verdicts this probe exists to send
    // are withheld before they ever reach the wire.
    engineer.update_capabilities(ac_core::games::assetto_corsa::CAPABILITIES);
    let car = troubled_car();
    let session = Session {
        surface_grip: 1.0,
        ..Default::default()
    };
    let info = SessionInfo::default();

    // Every alert waits a second of the condition actually holding before it is
    // said — one odd frame is not a finding. That is wall-clock time inside the
    // engineer, so the probe has to spend it: ask once to start the timers,
    // wait, then ask again. Skipping this is what made the first run of this
    // probe report an empty frame and look like a broken feed.
    for _ in 0..30 {
        engineer.update(&car, &session, &info);
    }
    let _ = engineer.analyze_live(&car, &session, None);
    print!("holding the alerts for their second... ");
    std::io::Write::flush(&mut std::io::stdout()).ok();
    sleep(Duration::from_millis(1200));
    for _ in 0..30 {
        engineer.update(&car, &session, &info);
    }
    let recommendations = engineer.analyze_live(&car, &session, None);
    println!("done\n");

    let mut frame = OverlayFrame::empty();
    frame.speed_kmh = car.speed_kmh;
    frame.tyre_pressure_psi = car.tyre_pressure_psi;
    frame.set_flag(flags::CONNECTED, true);
    frame.set_messages(&recommendations);

    println!("what the engineer said, before it went anywhere:");
    for line in advice_of(&frame) {
        println!("  {line}");
    }
    if recommendations.is_empty() {
        println!("  (nothing — the crafted state tripped no rule, which is a bug in this probe)");
    }

    let mut sink = UdpSink::new(target, "assetto_corsa", "probe", 60.0).unwrap_or_else(|error| {
        eprintln!("cannot open a sending socket: {error}");
        std::process::exit(1);
    });
    // How big it is on the wire matters more than it sounds: a datagram over
    // about 1472 bytes is split by IP into fragments, and losing any one of
    // them discards the whole thing. Nothing on loopback ever notices, so the
    // number is printed rather than trusted.
    let bytes = ac_core::broadcast::udp::payload_size(&frame, "assetto_corsa", "probe");
    println!(
        "\non the wire: {bytes} bytes — {} in one Ethernet frame, {} in one mesh-VPN frame",
        if bytes <= 1472 { "fits" } else { "FRAGMENTS" },
        if bytes <= 1252 { "fits" } else { "FRAGMENTS" },
    );
    println!("sending to {target}");
    if let Err(error) = sink.publish(&frame) {
        eprintln!("send failed: {error}");
        std::process::exit(1);
    }

    let Some(receiver) = receiver.as_mut() else {
        println!("sent. Run this with no argument on the far end to see it arrive.");
        return;
    };

    // A datagram to ourselves is not instant, and the receiver is deliberately
    // non-blocking — a spectator's terminal must keep repainting whether or not
    // anything arrived.
    for _ in 0..50 {
        match receiver.poll() {
            Received::Frame(arrived) => {
                let lines = advice_of(&arrived);
                println!("\nwhat came back out the far end:");
                for line in &lines {
                    println!("  {line}");
                }
                let sent = advice_of(&frame);
                println!(
                    "\n{} of {} lines survived the trip; speed {} km/h",
                    lines.len(),
                    sent.len(),
                    arrived.speed_kmh
                );
                if lines == sent {
                    println!(
                        "the sentences are identical — a spectator sees the engineer, not numbers"
                    );
                } else {
                    eprintln!("THE ADVICE CHANGED IN FLIGHT");
                    std::process::exit(1);
                }
                return;
            }
            Received::Rejected(why) => {
                eprintln!("a datagram arrived and was refused: {why:?}");
                std::process::exit(1);
            }
            Received::Idle => sleep(Duration::from_millis(20)),
        }
    }
    eprintln!("nothing arrived within a second");
    std::process::exit(1);
}
