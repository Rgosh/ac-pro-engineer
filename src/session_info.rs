#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub car_name: String,
    pub track_name: String,
    pub track_config: String,
    pub player_name: String,
    pub session_type: String,
    pub lap_count: i32,
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