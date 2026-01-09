use crate::session_info::SessionInfo;
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

const CLIENT_ID: &str = "119876543210987654";

pub struct DiscordClient {
    client: Option<DiscordIpcClient>,
    last_update: std::time::Instant,
    is_connected: bool,
}

impl DiscordClient {
    pub fn new() -> Self {
        let mut client = DiscordIpcClient::new(CLIENT_ID).ok();
        let mut connected = false;

        if let Some(c) = &mut client {
            if c.connect().is_ok() {
                connected = true;
            }
        }

        Self {
            client,
            last_update: std::time::Instant::now(),
            is_connected: connected,
        }
    }

    pub fn update(&mut self, is_connected: bool, session_info: &SessionInfo, delta: f32) {
        if !self.is_connected || self.last_update.elapsed() < std::time::Duration::from_secs(2) {
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

            let payload = activity::Activity::new()
                .details(&details)
                .state(&state)
                .assets(
                    activity::Assets::new()
                        .large_image("logo_large")
                        .large_text("AC Pro Engineer")
                        .small_image("status_icon")
                        .small_text(&small_text),
                );

            if client.set_activity(payload).is_err() {
                self.is_connected = false;
            }

            self.last_update = std::time::Instant::now();
        }
    }
}
