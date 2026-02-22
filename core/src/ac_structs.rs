use std::fmt::Display;
use std::fmt::Formatter;
use tracing::info;
use zerocopy::TryFromBytes;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, TryFromBytes)]
pub struct AcPhysics {
    pub packet_id: i32,
    pub gas: f32,
    pub brake: f32,
    pub fuel: f32,
    pub gear: i32,
    pub rpms: i32,
    pub steer_angle: f32,
    pub speed_kmh: f32,

    pub velocity: [f32; 3],
    pub acc_g: [f32; 3],

    pub wheel_slip: [f32; 4],
    pub wheel_load: [f32; 4],
    pub wheels_pressure: [f32; 4],
    pub wheel_angular_speed: [f32; 4],
    pub tyre_wear: [f32; 4],
    pub tyre_dirty_level: [f32; 4],
    pub tyre_core_temp: [f32; 4],
    pub camber_rad: [f32; 4],
    pub suspension_travel: [f32; 4],
    pub drs: f32,
    pub tc: f32,
    pub heading: f32,
    pub pitch: f32,
    pub roll: f32,
    pub cg_height: f32,
    pub car_damage: [f32; 5],
    pub number_of_tyres_out: i32,
    pub pit_limiter_on: i32,
    pub abs: f32,
    pub kers_charge: f32,
    pub kers_input: f32,
    pub auto_shifter_on: i32,
    pub ride_height: [f32; 2],
    pub turbo_boost: f32,
    pub ballast: f32,
    pub air_density: f32,
    pub air_temp: f32,
    pub road_temp: f32,
    pub local_angular_vel: [f32; 3],
    pub final_ff: f32,
    pub performance_meter: f32,
    pub engine_brake: i32,
    pub ers_recovery_level: i32,
    pub ers_power_level: i32,
    pub ers_heat_charging: i32,
    pub ers_is_charging: i32,
    pub kers_current_kj: f32,
    pub drs_available: i32,
    pub drs_enabled: i32,
    pub brake_temp: [f32; 4],
    pub clutch: f32,
    pub tyre_temp_i: [f32; 4],
    pub tyre_temp_m: [f32; 4],
    pub tyre_temp_o: [f32; 4],
    pub is_ai_controlled: i32,
    pub tyre_contact_point: [[f32; 3]; 4],
    pub tyre_contact_normal: [[f32; 3]; 4],
    pub tyre_contact_heading: [[f32; 3]; 4],
    pub brake_bias: f32,
    pub local_velocity: [f32; 3],

    pub abs_level: i32,
    pub tc_level: i32,
    pub tc_in_action: f32,
    pub abs_in_action: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, TryFromBytes)]
pub struct StringU16_33([u16; 33]);

impl Default for StringU16_33 {
    fn default() -> Self {
        Self([0u16; 33])
    }
}

impl Display for StringU16_33 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = read_ac_string(&self.0);
        info!("Format name: '{name}'");
        write!(f, "{}", name)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, TryFromBytes)]
pub struct CarCoordinates([[f32; 3]; 60]);

impl Default for CarCoordinates {
    fn default() -> Self {
        Self([[0f32; 3]; 60])
    }
}

impl CarCoordinates {
    pub fn get(&self, first: usize, second: usize) -> f32 {
        self.0[first][second]
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, TryFromBytes)]
pub struct CarId([i32; 60]);

impl Default for CarId {
    fn default() -> Self {
        Self([0i32; 60])
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, TryFromBytes)]
pub struct AcGraphics {
    pub packet_id: i32,
    pub status: i32,
    pub session: i32,
    pub current_time: [u16; 15],
    pub last_time: [u16; 15],
    pub best_time: [u16; 15],
    pub split: [u16; 15],
    pub completed_laps: i32,
    pub position: i32,
    pub i_current_time: i32,
    pub i_last_time: i32,
    pub i_best_time: i32,
    pub session_time_left: f32,
    pub distance_traveled: f32,
    pub is_in_pit: i32,
    pub current_sector_index: i32,
    pub last_sector_time: i32,
    pub number_of_laps: i32,
    pub tyre_compound: StringU16_33,
    pub replay_time_multiplier: f32,
    pub normalized_car_position: f32,
    pub active_cars: i32,
    pub car_coordinates: CarCoordinates,
    pub car_id: CarId,
    pub player_car_id: i32,
    pub penalty_time: f32,
    pub flag: i32,
    pub penalty: i32,
    pub ideal_line_on: i32,
    pub is_in_pit_lane: i32,
    pub surface_grip: f32,
    pub mandatory_pit_done: i32,
    pub wind_speed: f32,
    pub wind_direction: f32,
    pub is_setup_menu_visible: i32,
    pub main_display_index: i32,
    pub secondary_display_index: i32,
    pub tc: i32,
    pub tccut: i32,
    pub engine_map: i32,
    pub abs: i32,
    pub fuel_x_lap: f32,
    pub rain_lights: i32,
    pub flashing_lights: i32,
    pub lights_stage: i32,
    pub exhaust_temperature: f32,
    pub wiper_lv: i32,
    pub driver_stint_total_time_left: i32,
    pub driver_stint_time_left: i32,
    pub rain_tyres: i32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, TryFromBytes)]
pub struct AcStatic {
    pub sm_version: [u16; 15],
    pub ac_version: [u16; 15],
    pub number_of_sessions: i32,
    pub num_cars: i32,
    pub car_model: StringU16_33,
    pub track: StringU16_33,
    pub player_name: StringU16_33,
    pub player_surname: StringU16_33,
    pub player_nick: StringU16_33,
    pub sector_count: i32,
    pub max_torque: f32,
    pub max_power: f32,
    pub max_rpm: i32,
    pub max_fuel: f32,
    pub suspension_max_travel: [f32; 4],
    pub tyre_radius: [f32; 4],
    pub max_turbo_boost: f32,
    pub deprecated_1: f32,
    pub deprecated_2: f32,
    pub penalties_enabled: i32,
    pub aid_fuel_rate: f32,
    pub aid_tire_rate: f32,
    pub aid_mechanical_damage: f32,
    pub allow_tyre_blankets: i32,
    pub aid_stability: f32,
    pub aid_auto_clutch: i32,
    pub aid_auto_blip: i32,
    pub has_drs: i32,
    pub has_ers: i32,
    pub has_kers: i32,
    pub kers_max_j: f32,
    pub engine_brake_settings_count: i32,
    pub ers_power_controller_count: i32,
    pub track_spline_length: f32,
    pub track_configuration: StringU16_33,
    pub ers_max_j: f32,
    pub is_timed_race: i32,
    pub has_extra_lap: i32,
    pub car_skin: StringU16_33,
    pub reversed_grid_positions: i32,
    pub pit_window_start: i32,
    pub pit_window_end: i32,
    pub is_online: i32,
}

pub fn read_ac_string(src: &[u16]) -> String {
    let len = src.iter().position(|&c| c == 0).unwrap_or(src.len());
    String::from_utf16_lossy(&src[..len])
}

impl AcPhysics {
    pub fn get_tyre_load_ratio(&self, wheel_index: usize) -> f32 {
        if wheel_index < 4 {
            let total = self.wheel_load.iter().sum::<f32>();
            if total > 1.0 {
                self.wheel_load[wheel_index] / total
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    pub fn get_avg_tyre_temp(&self, wheel_index: usize) -> f32 {
        if wheel_index < 4 {
            (self.tyre_temp_i[wheel_index]
                + self.tyre_temp_m[wheel_index]
                + self.tyre_temp_o[wheel_index])
                / 3.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ac_struct() {
        assert_eq!(size_of::<AcGraphics>(), 1320);
        assert_eq!(size_of::<AcPhysics>(), 596);
        assert_eq!(size_of::<AcStatic>(), 688);
    }
}
