#![allow(unsafe_code)]

use std::io::{self, Write};
use std::mem::size_of;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
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
    let current_scenario = Arc::new(AtomicU8::new(1));
    let scenario_clone = current_scenario.clone();

    thread::spawn(move || {
        let (_h_phys, phys_ptr) =
            create_shared_memory("Local\\acpmf_physics", size_of::<AcPhysics>());
        let (_h_gfx, gfx_ptr) =
            create_shared_memory("Local\\acpmf_graphics", size_of::<AcGraphics>());
        let (_h_stat, stat_ptr) =
            create_shared_memory("Local\\acpmf_static", size_of::<AcStatic>());

        let phys = phys_ptr as *mut AcPhysics;
        let gfx = gfx_ptr as *mut AcGraphics;
        let stat = stat_ptr as *mut AcStatic;

        unsafe {
            (*stat).max_rpm = 8500;
            (*stat).max_fuel = 120.0;
            (*stat).track_spline_length = 7004.0;

            let car_name = "ks_ferrari_488_gt3".encode_utf16().collect::<Vec<u16>>();
            for (i, &c) in car_name.iter().enumerate() {
                (*stat).car_model[i] = c;
            }

            let track_name = "spa".encode_utf16().collect::<Vec<u16>>();
            for (i, &c) in track_name.iter().enumerate() {
                (*stat).track[i] = c;
            }
        }

        let start_time = Instant::now();
        let mut last_lap_time = Instant::now();
        let mut lap_count = 0;
        let mut base_fuel = 100.0;

        loop {
            let scenario = scenario_clone.load(Ordering::Relaxed);
            let elapsed = start_time.elapsed().as_secs_f32();
            let lap_elapsed = last_lap_time.elapsed().as_millis() as i32;

            unsafe {
                let cycle = (elapsed % 15.0) / 15.0;
                let is_accelerating = cycle < 0.5;
                let is_braking = cycle >= 0.5 && cycle < 0.7;
                let is_cornering = cycle >= 0.7;

                (*gfx).status = 2;
                (*gfx).session = 1;
                (*gfx).i_current_time = lap_elapsed;
                (*gfx).completed_laps = lap_count;
                (*gfx).fuel_x_lap = 3.5;

                if is_accelerating {
                    (*phys).gas = 1.0;
                    (*phys).brake = 0.0;
                    (*phys).speed_kmh = 50.0 + (cycle * 400.0);
                    (*phys).rpms = 4000 + ((cycle * 15.0) % 1.0 * 4500.0) as i32;
                    (*phys).gear = 1 + (cycle * 10.0) as i32;
                    (*phys).acc_g[2] = 0.8;
                    (*phys).acc_g[0] = 0.0;
                    (*phys).wheel_angular_speed = [80.0, 80.0, 80.0, 80.0];
                    (*phys).wheel_slip = [0.0; 4];
                    (*phys).suspension_travel = [0.05; 4];
                } else if is_braking {
                    (*phys).gas = 0.0;
                    (*phys).brake = 1.0;
                    (*phys).speed_kmh = 250.0 - ((cycle - 0.5) * 800.0).max(50.0);
                    (*phys).rpms = 6000;
                    (*phys).gear = 2;
                    (*phys).acc_g[2] = -1.5;
                    (*phys).acc_g[0] = 0.0;
                    (*phys).suspension_travel = [0.01, 0.01, 0.08, 0.08];
                } else if is_cornering {
                    (*phys).gas = 0.5;
                    (*phys).brake = 0.0;
                    (*phys).speed_kmh = 120.0;
                    (*phys).gear = 3;
                    (*phys).rpms = 5000;
                    (*phys).acc_g[2] = 0.0;
                    (*phys).acc_g[0] = 1.8;
                    (*phys).steer_angle = 45.0;
                    (*phys).suspension_travel = [0.02, 0.08, 0.02, 0.08];
                }

                match scenario {
                    1 => {
                        (*phys).wheels_pressure = [27.3, 27.4, 27.2, 27.3];
                        (*phys).tyre_core_temp = [80.0, 81.0, 82.0, 81.0];
                        base_fuel -= 0.001;
                    }
                    2 => {
                        (*phys).wheels_pressure = [29.5, 29.8, 30.1, 29.9];
                        (*phys).tyre_core_temp = [110.0, 112.0, 108.0, 109.0];
                        (*phys).brake_temp = [800.0, 820.0, 750.0, 740.0];
                        base_fuel -= 0.001;
                    }
                    3 => {
                        (*phys).wheels_pressure = [27.3; 4];
                        (*phys).tyre_core_temp = [85.0; 4];

                        if is_braking {
                            (*phys).wheel_angular_speed[0] = 0.0;
                            (*phys).wheel_angular_speed[1] = 0.0;
                        } else if is_accelerating {
                            (*phys).wheel_slip[2] = 0.8;
                            (*phys).wheel_slip[3] = 0.8;
                        } else if is_cornering {
                            (*phys).suspension_travel[0] = 0.00;
                            (*phys).suspension_travel[1] = 0.00;
                        }
                        base_fuel -= 0.002;
                    }
                    4 => {
                        base_fuel = 3.0;
                        (*phys).wheels_pressure = [27.3; 4];
                        (*phys).tyre_core_temp = [80.0; 4];
                    }
                    _ => {}
                }

                (*phys).fuel = base_fuel;

                if lap_elapsed > 60_000 {
                    (*gfx).i_last_time = lap_elapsed;
                    (*gfx).i_best_time = 59_500;
                    lap_count += 1;
                    last_lap_time = Instant::now();
                }
            }

            thread::sleep(Duration::from_millis(16));
        }
    });

    println!("\n=== AC TELEMETRY SIMULATOR ===");
    println!("Shared Memory created successfully.");

    loop {
        println!("\nSelect simulation scenario:");
        println!("1. Normal Driving (Green numbers)");
        println!("2. Overheating (Red numbers, Hot Tyres)");
        println!("3. Mistakes (TRIGGERS ENGINEER: Lockups, Wheelspin, Bottoming)");
        println!("4. Low Fuel (Force Pit Stop warning)");
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if let Ok(choice) = input.trim().parse::<u8>() {
            if (1..=4).contains(&choice) {
                current_scenario.store(choice, Ordering::Relaxed);
                println!(">>> SCENARIO SWITCHED TO: {}", choice);
            }
        }
    }
}
