use serde::{Deserialize, Serialize};

/// Session timing helper. All times are stored in **milliseconds**
/// to match the Assetto Corsa shared-memory API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub car_name: String,
    pub track_name: String,
    pub track_config: String,
    pub player_name: String,
    pub session_type: String,
    pub lap_count: i32,
    /// Remaining session time in **milliseconds** (AC API convention).
    pub session_time_left: f32,
    pub max_rpm: i32,
    pub max_fuel: f32,
}

impl Default for SessionInfo {
    fn default() -> Self {
        Self {
            car_name: "-".to_string(),
            track_name: "-".to_string(),
            track_config: "-".to_string(),
            player_name: "-".to_string(),
            session_type: "-".to_string(),
            lap_count: 0,
            session_time_left: 0.0,
            max_rpm: 8000,
            max_fuel: 100.0,
        }
    }
}

/// Shared timing utilities for both UI and engine strategy calculations.
/// All inputs are in **milliseconds** to match AC shared memory fields.
pub struct SessionTiming;

impl SessionTiming {
    /// Calculate remaining laps in the session.
    ///
    /// - `session_time_left_ms`: remaining session time in milliseconds (from `AcGraphics::session_time_left`)
    /// - `best_lap_time_ms`: best lap time in milliseconds (from `AcGraphics::i_best_time`)
    /// - `last_lap_time_ms`: last lap time in milliseconds (from `AcGraphics::i_last_time`)
    /// - `number_of_laps`: total laps if lap-limited race (from `AcGraphics::number_of_laps`)
    /// - `completed_laps`: completed laps (from `AcGraphics::completed_laps`)
    /// - `normalized_car_position`: track position 0.0..1.0 (from `AcGraphics::normalized_car_position`)
    ///
    /// Returns estimated remaining laps (fractional).
    pub fn remaining_laps(
        session_time_left_ms: f32,
        best_lap_time_ms: i32,
        last_lap_time_ms: i32,
        number_of_laps: i32,
        completed_laps: i32,
        normalized_car_position: f32,
    ) -> f32 {
        // Lap-limited race
        if number_of_laps > 0 {
            return (number_of_laps as f32 - completed_laps as f32 - normalized_car_position)
                .max(0.0);
        }

        // Time-limited race
        if session_time_left_ms > 0.0 {
            let lap_time_ms = if best_lap_time_ms > 0 {
                best_lap_time_ms as f32
            } else if last_lap_time_ms > 0 {
                last_lap_time_ms as f32
            } else {
                120_000.0 // 2 minute fallback
            };

            if lap_time_ms > 0.0 {
                return session_time_left_ms / lap_time_ms;
            }
        }

        0.0
    }

    /// Format session time left (in milliseconds) as "MM:SS" string.
    pub fn format_time_left_ms(session_time_left_ms: f32) -> String {
        let total_sec = (session_time_left_ms / 1000.0).max(0.0) as u32;
        let minutes = total_sec / 60;
        let seconds = total_sec % 60;
        format!("{}:{:02}", minutes, seconds)
    }

    /// Format session time left (in milliseconds) as "{X} min" string.
    pub fn format_time_left_minutes(session_time_left_ms: f32) -> String {
        let minutes = session_time_left_ms / 60_000.0;
        format!("{:.1} min", minutes)
    }
}
