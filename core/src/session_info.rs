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
    /// Every argument is a field of [`Session`](crate::games::Session):
    ///
    /// - `session_time_left_ms`: remaining session time in milliseconds
    /// - `best_lap_time_ms`: best lap time in milliseconds
    /// - `last_lap_time_ms`: last lap time in milliseconds
    /// - `number_of_laps`: `total_laps`, non-zero only in a lap-limited race
    /// - `completed_laps`: completed laps
    /// - `normalized_car_position`: `track_position`, 0.0..1.0 around the lap
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

    /// Laps that must actually be fuelled for, as a whole number.
    ///
    /// [`Self::remaining_laps`] returns a fraction, which is the right thing
    /// for a readout but the wrong thing for a fuel target in two ways.
    ///
    /// A timed race does not stop when the clock hits zero — it stops when the
    /// leader next *completes* a lap, so the last lap has to be paid for in
    /// full. And the fraction ignores the lap already in progress, which still
    /// needs the fuel to finish.
    ///
    /// Both errors point the same way: under-fuelling. Rounding up is the safe
    /// direction, and the configured safety margin is applied on top of this.
    pub fn laps_to_fuel_for(
        session_time_left_ms: f32,
        best_lap_time_ms: i32,
        last_lap_time_ms: i32,
        number_of_laps: i32,
        completed_laps: i32,
        normalized_car_position: f32,
    ) -> f32 {
        let remaining = Self::remaining_laps(
            session_time_left_ms,
            best_lap_time_ms,
            last_lap_time_ms,
            number_of_laps,
            completed_laps,
            normalized_car_position,
        );
        if remaining <= 0.0 {
            return 0.0;
        }

        // A lap-limited race already counts down whole laps and subtracts the
        // current position, so adding the rest of this lap back gets the
        // whole-lap count. A timed race gets rounded up to the lap the leader
        // finishes on.
        if number_of_laps > 0 {
            (remaining + normalized_car_position).ceil()
        } else {
            remaining.ceil()
        }
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

#[cfg(test)]
mod tests {
    use super::SessionTiming;

    /// The display fraction is what it always was.
    #[test]
    fn remaining_laps_still_reports_a_fraction() {
        // 5 minutes left, 90 second laps.
        let laps = SessionTiming::remaining_laps(300_000.0, 90_000, 0, 0, 0, 0.0);
        assert!((laps - 3.333).abs() < 0.01);
    }

    /// A timed race runs until the leader completes the lap the clock ran out
    /// on, so 3.33 laps of clock means four laps of fuel.
    #[test]
    fn fuel_target_rounds_a_timed_race_up() {
        let laps = SessionTiming::laps_to_fuel_for(300_000.0, 90_000, 0, 0, 0, 0.0);
        assert_eq!(laps, 4.0);
    }

    /// Half way around lap 16 of 20: four full laps plus the rest of this one,
    /// which is five laps of fuel, not the 4.5 the fraction reports.
    #[test]
    fn fuel_target_includes_the_lap_in_progress() {
        assert_eq!(
            SessionTiming::remaining_laps(0.0, 80_000, 0, 20, 15, 0.5),
            4.5
        );
        assert_eq!(
            SessionTiming::laps_to_fuel_for(0.0, 80_000, 0, 20, 15, 0.5),
            5.0
        );
    }

    /// A finished race needs no fuel, and must not round 0 up to 1.
    #[test]
    fn fuel_target_is_zero_at_the_end() {
        assert_eq!(
            SessionTiming::laps_to_fuel_for(0.0, 80_000, 0, 20, 20, 0.0),
            0.0
        );
    }
}
