use crate::session_info::SessionInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayState {
    pub speed_kmh: i32,
    pub gear: i32,
    pub rpm: i32,
    pub max_rpm: i32,
    pub is_pit_limiter_on: bool,
    pub show_telemetry: bool,
    pub show_engineer: bool,
    pub engineer_messages: Vec<String>,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            speed_kmh: 0,
            gear: 0,
            rpm: 0,
            max_rpm: 8500,
            is_pit_limiter_on: false,
            show_telemetry: true,
            show_engineer: true,
            engineer_messages: Vec::new(),
        }
    }
}

impl OverlayState {
    pub fn update_from_session(&mut self, session: &SessionInfo) {
        if session.max_rpm > 0 {
            self.max_rpm = session.max_rpm;
        }
    }
}
