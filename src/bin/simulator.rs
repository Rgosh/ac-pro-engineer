#![allow(unsafe_code)]

use std::io::{self, Write};
use std::mem::size_of;
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE,
};

#[path = "../ac_structs.rs"]
mod ac_structs;
use ac_structs::{AcGraphics, AcPhysics, AcStatic};

fn create_shared_memory(name: &str, size: usize) -> (HANDLE, *mut u8) {
    use std::os::windows::ffi::OsStrExt;
    let mut wide_name: Vec<u16> = std::ffi::OsStr::new(name).encode_wide().collect();
    wide_name.push(0);

    unsafe {
        let handle = CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            None,
            PAGE_READWRITE,
            0,
            size as u32,
            windows::core::PCWSTR(wide_name.as_ptr()),
        )
        .expect("Failed to create file mapping");

        let mapped_view = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size);
        let ptr = mapped_view.Value as *mut u8;

        if ptr.is_null() {
            panic!("Failed to map view of file");
        }

        (handle, ptr)
    }
}

fn main() {
    println!("\n=== AC PRO ENGINEER: AUTOMATED TELEMETRY SIMULATOR ===");
    println!("Initializing shared memory...");

    let (_h_phys, phys_ptr) = create_shared_memory("Local\\acpmf_physics", size_of::<AcPhysics>());
    let (_h_gfx, gfx_ptr) = create_shared_memory("Local\\acpmf_graphics", size_of::<AcGraphics>());
    let (_h_stat, stat_ptr) = create_shared_memory("Local\\acpmf_static", size_of::<AcStatic>());

    let phys = phys_ptr as *mut AcPhysics;
    let gfx = gfx_ptr as *mut AcGraphics;
    let stat = stat_ptr as *mut AcStatic;

    unsafe {
        (*stat).max_rpm = 9000;
        (*stat).max_fuel = 120.0;
        (*stat).track_spline_length = 7004.0;

        let car_str = "kunos_ferrari_488_gt3";
        for (i, c) in car_str.encode_utf16().enumerate() {
            if i < 32 {
                (*stat).car_model[i] = c;
            }
        }

        let track_str = "spa";
        for (i, c) in track_str.encode_utf16().enumerate() {
            if i < 32 {
                (*stat).track[i] = c;
            }
        }

        let player_str = "Simulator_User";
        for (i, c) in player_str.encode_utf16().enumerate() {
            if i < 32 {
                (*stat).player_nick[i] = c;
            }
        }
    }

    println!("Simulation started. Press Ctrl+C to stop.");
    println!("Generating dynamic telemetry data...\n");

    let start_time = Instant::now();
    let mut last_lap_time = Instant::now();
    let mut lap_count = 0;

    let mut speed: f32 = 0.0;
    let mut dist = 0.0;
    let mut fuel = 50.0;

    loop {
        let now = Instant::now();
        let total_elapsed = now.duration_since(start_time).as_secs_f32();
        let lap_elapsed_ms = last_lap_time.elapsed().as_millis() as i32;

        let scenario_timer = total_elapsed % 30.0;

        let (gas, brake, steer, gear, lat_g, lon_g, scenario_name) = if scenario_timer < 10.0 {
            (
                1.0,
                0.0,
                0.0,
                4 + (speed / 50.0) as i32,
                0.0,
                0.5,
                "ACCELERATION",
            )
        } else if scenario_timer < 15.0 {
            (0.0, 1.0, 0.0, 2, 0.0, -1.2, "HEAVY BRAKING")
        } else if scenario_timer < 25.0 {
            let s_factor = (scenario_timer - 15.0).sin();
            (
                0.6,
                0.0,
                s_factor * 45.0,
                3,
                s_factor * 1.5,
                0.1,
                "CORNERING",
            )
        } else {
            (0.0, 1.0, 10.0, 1, 0.2, -0.8, "ERROR SIMULATION")
        };

        if gas > 0.0 {
            speed += gas * 1.5;
        }
        if brake > 0.0 {
            speed -= brake * 3.0;
        }
        speed = speed.clamp(0.0, 280.0);
        dist += speed / 3.6 * 0.016;

        unsafe {
            (*gfx).status = 2;
            (*gfx).session = 3;
            (*gfx).completed_laps = lap_count;
            (*gfx).i_current_time = lap_elapsed_ms;
            (*gfx).i_last_time = 135000;
            (*gfx).i_best_time = 134500;
            (*gfx).normalized_car_position = (dist % 7004.0) / 7004.0;

            let compound = [0u16; 33];
            (*gfx).tyre_compound = compound;

            (*phys).speed_kmh = speed;
            (*phys).gas = gas;
            (*phys).brake = brake;
            (*phys).steer_angle = steer / 360.0;
            (*phys).gear = gear.min(6);
            (*phys).rpms = (3000.0 + speed * 20.0).min(9000.0) as i32;
            (*phys).fuel = fuel;

            (*phys).acc_g = [lat_g, 0.0, lon_g];

            let brake_heat = if brake > 0.0 { 10.0 } else { -2.0 };
            for i in 0..4 {
                (*phys).brake_temp[i] = ((*phys).brake_temp[i] + brake_heat).clamp(150.0, 800.0);
            }

            let aero_squat = (speed / 300.0).powi(2) * 0.030;
            let brake_dive = if brake > 0.0 { 0.015 } else { 0.0 };
            let lat_roll = lat_g * 0.010;
            let base_travel = 0.050;

            (*phys).suspension_travel[0] = base_travel + aero_squat + brake_dive + lat_roll;
            (*phys).suspension_travel[1] = base_travel + aero_squat + brake_dive - lat_roll;
            (*phys).suspension_travel[2] = base_travel + aero_squat - (brake_dive * 0.5) + lat_roll;
            (*phys).suspension_travel[3] = base_travel + aero_squat - (brake_dive * 0.5) - lat_roll;

            (*phys).ride_height[0] = (0.080 - (*phys).suspension_travel[0]).max(0.001);
            (*phys).ride_height[1] = (0.090 - (*phys).suspension_travel[2]).max(0.001);

            for i in 0..4 {
                let is_front = i < 2;
                let is_left = i % 2 == 0;
                let lat_load = if is_left { -lat_g } else { lat_g };

                let core_t =
                    80.0 + (speed * 0.05) + (brake * 20.0 * if is_front { 1.0 } else { 0.3 });
                (*phys).tyre_core_temp[i] = core_t;

                let camber_heat = 5.0;
                let corner_heat = if lat_load > 0.0 { lat_load * 10.0 } else { 0.0 };

                (*phys).tyre_temp_i[i] = core_t + camber_heat;
                (*phys).tyre_temp_m[i] = core_t;
                (*phys).tyre_temp_o[i] = core_t - camber_heat + corner_heat;

                let base_psi = 26.0;
                let temp_factor = (core_t - 20.0) * 0.1;
                (*phys).wheels_pressure[i] = base_psi + temp_factor;

                if scenario_timer > 25.0 && brake > 0.5 {
                    (*phys).wheel_slip[i] = 5.0;
                    (*phys).wheel_angular_speed[i] = 0.0;
                } else {
                    (*phys).wheel_slip[i] = 0.1;
                    (*phys).wheel_angular_speed[i] = speed / 2.0;
                }
            }

            if lap_elapsed_ms > 135000 {
                last_lap_time = Instant::now();
                lap_count += 1;
                fuel -= 2.5;
            }
        }

        if (total_elapsed * 10.0) as i32 % 10 == 0 {
            print!(
                "\r[SIMULATOR] Mode: {:<20} | Spd: {:>3.0} km/h | Fuel: {:>4.1} L | Lap: {}   ",
                scenario_name, speed, fuel, lap_count
            );
            io::stdout().flush().unwrap();
        }

        thread::sleep(Duration::from_millis(16));
    }
}
