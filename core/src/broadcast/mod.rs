//! The core publishes; anything at all subscribes.
//!
//! One producer, many sinks, and the producer never waits for any of them. The
//! application computes a frame per tick and hands it here; where it goes is a
//! list of sinks and not a property of the application.
//!
//! What that buys, in the order it was asked for:
//!
//! * the in-game panel reads a shared-memory mapping, as it always has —
//!   [`shm::SharedMemorySink`]
//! * anything else on the machine reads UDP on localhost, which is a socket and
//!   fifty lines rather than a mapping and a bridge — [`udp::UdpSink`]
//! * a friend on another machine, or a relay for a championship, is the same
//!   UDP sink with a different address
//!
//! ## The rule that shapes all of it
//!
//! **A sink is allowed to be slow, to fail, and to go away, and the tick must
//! not notice.** A remote peer on a bad connection, a relay that is down, a
//! subscriber that stopped reading — none of those may stall the loop that is
//! also feeding the driver's own overlay. So `publish` takes what it is given
//! and returns; a sink that errs is logged and, if it keeps erring, dropped.
//!
//! ## What travels
//!
//! The computed frame, not raw telemetry. The driver's machine reads its game,
//! analyses, advises, and sends the *result* — so a receiver renders and does
//! nothing else, and needs to know nothing about which simulator produced it.
//! That is what makes the format game-neutral without any work: an
//! [`OverlayFrame`] has speed, gear, four corners, lap times and advice text in
//! it, and no `AcPhysics` anywhere.

pub mod receiver;
pub mod shm;
pub mod udp;

use crate::overlay::frame::OverlayFrame;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Somewhere a frame can go.
pub trait Sink: Send {
    /// For logs and for the diagnostics screen.
    fn name(&self) -> &str;

    /// Take this frame. Must not block for long — see the rule above.
    fn publish(&mut self, frame: &OverlayFrame) -> std::io::Result<()>;

    /// How often this sink wants a frame, at most.
    ///
    /// The local mapping wants every one: it is a memcpy and the panel reads
    /// whatever is there. A remote subscriber does not — a spectator stops
    /// noticing above about ten a second, and twenty cars at tick rate is
    /// megabytes a second at a relay. Overriding this is how a sink says so.
    fn min_interval(&self) -> Duration {
        Duration::ZERO
    }

    /// Last words, if the sink has any. Called on a clean shutdown so a panel
    /// hides at once rather than waiting out its liveness timeout.
    fn shutdown(&mut self) {}
}

/// How many consecutive failures before a sink is dropped.
///
/// Not one: a UDP send can fail transiently while an interface comes back, and
/// tearing the sink down for that would mean a driver loses their friend's feed
/// over a hiccup. Not unbounded either — a sink erring every tick writes a log
/// line every 16 ms.
const FAILURES_BEFORE_DROP: u32 = 60;

struct Entry {
    sink: Box<dyn Sink>,
    last_sent: Option<Instant>,
    failures: u32,
}

/// The producer side: hand it a frame, forget about it.
#[derive(Default)]
pub struct Broadcaster {
    entries: Vec<Entry>,
}

impl Broadcaster {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a sink. Order is not significant; nothing here depends on another.
    pub fn add(&mut self, sink: Box<dyn Sink>) {
        info!(sink = sink.name(), "Broadcasting to a new sink");
        self.entries.push(Entry {
            sink,
            last_sent: None,
            failures: 0,
        });
    }

    /// Which sinks are live, for the diagnostics screen.
    pub fn sink_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.sink.name().to_string())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Send a frame to every sink that is due one.
    ///
    /// Never returns an error: there is no useful reaction here to a sink
    /// failing, and the caller is a telemetry loop that must keep running for
    /// the sake of every other sink.
    pub fn publish(&mut self, frame: &OverlayFrame) {
        let now = Instant::now();

        self.entries.retain_mut(|entry| {
            let interval = entry.sink.min_interval();
            if let Some(last) = entry.last_sent
                && now.duration_since(last) < interval
            {
                return true;
            }

            match entry.sink.publish(frame) {
                Ok(()) => {
                    entry.last_sent = Some(now);
                    entry.failures = 0;
                    true
                }
                Err(error) => {
                    entry.failures += 1;
                    // Logged once at each end rather than every tick: a sink
                    // failing sixty times a second would be the only thing in
                    // the log file.
                    if entry.failures == 1 {
                        warn!(sink = entry.sink.name(), error = ?error, "Sink failed");
                    }
                    if entry.failures >= FAILURES_BEFORE_DROP {
                        warn!(
                            sink = entry.sink.name(),
                            "Sink failed {FAILURES_BEFORE_DROP} times running; dropping it"
                        );
                        return false;
                    }
                    true
                }
            }
        });
    }

    /// Tell every sink we are going away.
    pub fn shutdown(&mut self) {
        for entry in &mut self.entries {
            entry.sink.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Counting {
        sent: Arc<AtomicU32>,
        fail: bool,
        interval: Duration,
    }

    impl Sink for Counting {
        fn name(&self) -> &str {
            "counting"
        }
        fn publish(&mut self, _frame: &OverlayFrame) -> std::io::Result<()> {
            if self.fail {
                return Err(std::io::Error::other("no"));
            }
            self.sent.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        fn min_interval(&self) -> Duration {
            self.interval
        }
    }

    fn counting(sent: Arc<AtomicU32>, fail: bool, interval: Duration) -> Box<dyn Sink> {
        Box::new(Counting {
            sent,
            fail,
            interval,
        })
    }

    /// The whole point: one sink failing must not cost another its frames.
    #[test]
    fn a_failing_sink_does_not_starve_the_others() {
        let good = Arc::new(AtomicU32::new(0));
        let mut broadcaster = Broadcaster::new();
        broadcaster.add(counting(Arc::new(AtomicU32::new(0)), true, Duration::ZERO));
        broadcaster.add(counting(good.clone(), false, Duration::ZERO));

        let frame = OverlayFrame::empty();
        for _ in 0..10 {
            broadcaster.publish(&frame);
        }

        assert_eq!(good.load(Ordering::Relaxed), 10);
    }

    /// A sink that never recovers is eventually let go, or it writes a log line
    /// every sixteen milliseconds for the rest of the session.
    #[test]
    fn a_sink_that_never_recovers_is_dropped() {
        let mut broadcaster = Broadcaster::new();
        broadcaster.add(counting(Arc::new(AtomicU32::new(0)), true, Duration::ZERO));

        let frame = OverlayFrame::empty();
        for _ in 0..FAILURES_BEFORE_DROP {
            broadcaster.publish(&frame);
        }

        assert!(
            broadcaster.is_empty(),
            "a sink failing {FAILURES_BEFORE_DROP} times running should have gone"
        );
    }

    /// A remote sink asks for a slower rate, and gets it. Twenty cars at tick
    /// rate is megabytes a second at a relay; a spectator cannot tell.
    #[test]
    fn a_sink_can_ask_for_a_slower_rate() {
        let slow = Arc::new(AtomicU32::new(0));
        let mut broadcaster = Broadcaster::new();
        broadcaster.add(counting(slow.clone(), false, Duration::from_secs(3600)));

        let frame = OverlayFrame::empty();
        for _ in 0..50 {
            broadcaster.publish(&frame);
        }

        assert_eq!(
            slow.load(Ordering::Relaxed),
            1,
            "the first frame goes, and nothing else for an hour"
        );
    }
}
