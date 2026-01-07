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