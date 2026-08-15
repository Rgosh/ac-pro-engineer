//! Rich presence, and the rule that it may never hold anything up.
//!
//! This talks to a socket owned by another program on the user's desktop. It
//! is the least important thing in the application and the one with the most
//! ways to be slow: Discord may be absent, starting up, logged out, or busy,
//! and none of that is the driver's problem.

use crate::session_info::SessionInfo;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CLIENT_ID: &str = "119876543210987654";

/// How long to leave Discord alone after a failed connection.
///
/// Long enough that a machine without Discord is not retrying constantly, short
/// enough that starting Discord after the application picks it up within a
/// stint.
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// How often the presence is refreshed once connected.
const UPDATE_EVERY: Duration = Duration::from_secs(2);

pub struct DiscordClient {
    client: Option<DiscordIpcClient>,
    last_update: Instant,
    is_connected: bool,
    /// When connecting may next be attempted. `None` once connected.
    next_attempt: Option<Instant>,
    start_time: i64,
}

impl Default for DiscordClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscordClient {
    /// Build the client without touching the socket.
    ///
    /// It used to connect here, which put a blocking IPC handshake with
    /// another desktop application in the path of `AppState::new` — so the
    /// terminal's startup, and every test that builds an app state, waited on
    /// whether the user happened to have Discord open. With Discord running
    /// that turned a 0.4 second test suite into a six minute one, and it was
    /// invisible until somebody's Discord was open while the suite ran.
    ///
    /// Connecting is now something the tick does, on its own schedule, and
    /// gives up on for a minute at a time.
    pub fn new() -> Self {
        let start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            client: DiscordIpcClient::new(CLIENT_ID).ok(),
            // Far enough in the past that the first tick may connect at once.
            last_update: Instant::now() - UPDATE_EVERY,
            is_connected: false,
            next_attempt: Some(Instant::now()),
            start_time: start,
        }
    }

    /// Whether the presence is currently reaching Discord.
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    /// Try to connect, at most once every [`RETRY_AFTER`].
    ///
    /// Returns whether there is a live connection to write to.
    fn ensure_connected(&mut self) -> bool {
        if self.is_connected {
            return true;
        }
        match self.next_attempt {
            Some(when) if Instant::now() < when => return false,
            None => return false,
            _ => {}
        }

        let connected = self
            .client
            .as_mut()
            .is_some_and(|client| client.connect().is_ok());

        self.is_connected = connected;
        self.next_attempt = if connected {
            None
        } else {
            Some(Instant::now() + RETRY_AFTER)
        };
        connected
    }

    pub fn update(&mut self, is_connected: bool, session_info: &SessionInfo, delta: f32) {
        if self.last_update.elapsed() < UPDATE_EVERY {
            return;
        }
        // Stamped before the attempt, not after it: a connection that fails
        // slowly must not be retried on the very next tick.
        self.last_update = Instant::now();

        if !self.ensure_connected() {
            return;
        }

        if let Some(client) = &mut self.client {
            let details = if is_connected {
                if session_info.car_name == "-" {
                    "In Pit / Idle".to_string()
                } else {
                    format!("Driving {}", session_info.car_name)
                }
            } else {
                "In Menu".to_string()
            };

            let state = if is_connected {
                format!(
                    "{} | Lap {}",
                    session_info.track_name, session_info.lap_count
                )
            } else {
                "Analyzing Telemetry".to_string()
            };

            let small_text = if is_connected {
                format!("Delta: {:+.3}", delta)
            } else {
                format!("v{}", crate::updater::CURRENT_VERSION)
            };

            let mut activity = activity::Activity::new()
                .details(&details)
                .state(&state)
                .assets(
                    activity::Assets::new()
                        .large_image("logo_large")
                        .large_text("AC Pro Engineer")
                        .small_image("status_icon")
                        .small_text(&small_text),
                );

            if is_connected {
                activity = activity.timestamps(activity::Timestamps::new().start(self.start_time));
            }

            if client.set_activity(activity).is_err() {
                // Discord went away — a quit, a restart, a logout. Fall back to
                // the same patient retry rather than writing to a dead socket
                // sixty times a second.
                self.is_connected = false;
                self.next_attempt = Some(Instant::now() + RETRY_AFTER);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property that matters: constructing this touches nothing.
    ///
    /// Timed rather than asserted structurally, because the failure it guards
    /// against is a blocking handshake with whatever the user has running —
    /// which no amount of reading the fields would reveal.
    #[test]
    fn building_the_client_never_waits_on_discord() {
        let started = Instant::now();
        let client = DiscordClient::new();
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "construction took {:?}",
            started.elapsed()
        );
        assert!(!client.is_connected());
    }

    /// A machine with no Discord must not attempt a connection on every tick.
    ///
    /// The first update may try — and on a machine that *does* run Discord it
    /// will succeed, which is fine — but a failure has to buy an hour of quiet,
    /// not one frame of it.
    #[test]
    fn a_refused_connection_is_not_retried_every_frame() {
        let mut client = DiscordClient::new();
        let info = SessionInfo::default();

        client.update(false, &info, 0.0);
        if client.is_connected() {
            // Discord is running on this machine, so there is nothing to say
            // about the back-off; the connection is what was wanted.
            return;
        }

        let scheduled = client
            .next_attempt
            .expect("a failed attempt schedules the next one");
        assert!(
            scheduled > Instant::now() + RETRY_AFTER / 2,
            "the retry is a minute away, not a frame"
        );
    }

    /// The throttle applies to the whole body, including the connect attempt.
    #[test]
    fn updates_are_throttled_to_their_interval() {
        let mut client = DiscordClient::new();
        let info = SessionInfo::default();

        client.update(false, &info, 0.0);
        let before = client.last_update;
        client.update(false, &info, 0.0);
        assert_eq!(
            before, client.last_update,
            "a second update inside the interval does nothing at all"
        );
    }
}
