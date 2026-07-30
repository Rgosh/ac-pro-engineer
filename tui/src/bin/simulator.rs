#![allow(unsafe_code)]

use ac_core::ac_structs::{AcGraphics, AcPhysics, AcStatic};
use std::io::{self, Write};
use std::mem::size_of;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_ALL_ACCESS, MapViewOfFile, PAGE_READWRITE,
};

#[cfg(target_os = "windows")]
fn create_shared_memory(
    name: &str,
    size: usize,
) -> Result<(HANDLE, *mut u8), Box<dyn std::error::Error>> {
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
            return Err("Failed to map view of file".into());
        }

        Ok((handle, ptr))
    }
}

#[cfg(not(target_os = "windows"))]
fn create_shared_memory(
    name: &str,
    size: usize,
) -> Result<*mut u8, Box<dyn std::error::Error>> {
    use std::fs::OpenOptions;

    let path = format!("/dev/shm/{}", name.replace("Local\\", ""));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;

    file.set_len(size as u64)?;

    let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };
    let ptr = mmap.as_ptr() as *mut u8;

    std::mem::forget(mmap);
    Ok(ptr)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== AC PRO ENGINEER: AUTOMATED TELEMETRY SIMULATOR ===");
    println!("Initializing shared memory...");

    #[cfg(target_os = "windows")]
    let (_h_phys, phys_ptr) = create_shared_memory("Local\\acpmf_physics", size_of::<AcPhysics>())?;
    #[cfg(target_os = "windows")]
    let (_h_gfx, gfx_ptr) = create_shared_memory("Local\\acpmf_graphics", size_of::<AcGraphics>())?;
    #[cfg(target_os = "windows")]
    let (_h_stat, stat_ptr) = create_shared_memory("Local\\acpmf_static", size_of::<AcStatic>())?;

    #[cfg(not(target_os = "windows"))]
    let phys_ptr = create_shared_memory("Local\\acpmf_physics", size_of::<AcPhysics>())?;
    #[cfg(not(target_os = "windows"))]
    let gfx_ptr = create_shared_memory("Local\\acpmf_graphics", size_of::<AcGraphics>())?;
    #[cfg(not(target_os = "windows"))]
    let stat_ptr = create_shared_memory("Local\\acpmf_static", size_of::<AcStatic>())?;

    let phys = phys_ptr as *mut AcPhysics;
    let gfx = gfx_ptr as *mut AcGraphics;
    let stat = stat_ptr as *mut AcStatic;

    unsafe {
        (*stat).max_rpm = 9000;
        (*stat).max_fuel = 120.0;
        (*stat).track_spline_length = 7004.0;
        (*stat).car_model = "kunos_ferrari_488_gt3".into();
        (*stat).track = "spa".into();
        (*stat).player_nick = "Simulator_User".into();
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
            (
                0.0,
                0.9,
                -0.4,
                2,
                -1.8,
                -1.2,
                "HEAVY BRAKING & TURN-IN",
            )
        } else if scenario_timer < 22.0 {
            (
                0.4,
                0.0,
                0.5,
                3,
                2.2,
                0.1,
                "APEX CORNERING (MAX G)",
            )
        } else {
            (
                0.85,
                0.0,
                0.1,
                5,
                0.5,
                0.3,
                "CORNER EXIT & FULL THROTTLE",
            )
        };

        if gas > 0.5 {
            speed = (speed + 2.5).min(285.0);
        } else if brake > 0.5 {
            speed = (speed - 4.0).max(60.0);
        }

        let rpm = (3000.0 + (speed / 300.0) * 5500.0 + (scenario_timer * 100.0).sin() * 200.0) as i32;
        dist += (speed / 3.6) * 0.016;
        fuel = (fuel - 0.0005f32).max(0.0f32);

        if lap_elapsed_ms > 120_000 {
            lap_count += 1;
            last_lap_time = Instant::now();
        }

        unsafe {
            (*phys).packet_id = (*phys).packet_id.wrapping_add(1);
            (*phys).gas = gas;
            (*phys).brake = brake;
            (*phys).fuel = fuel;
            (*phys).gear = gear;
            (*phys).rpms = rpm;
            (*phys).steer_angle = steer;
            (*phys).speed_kmh = speed;
            (*phys).acc_g = [lat_g, 0.0, lon_g];

            let temp_base = 80.0 + (speed / 280.0) * 15.0;
            (*phys).tyre_temp_i = [temp_base + 3.0, temp_base + 4.0, temp_base + 1.0, temp_base + 2.0];
            (*phys).tyre_temp_m = [temp_base, temp_base + 1.0, temp_base - 1.0, temp_base];
            (*phys).tyre_temp_o = [temp_base - 3.0, temp_base - 2.0, temp_base - 4.0, temp_base - 3.0];
            (*phys).wheels_pressure = [27.4, 27.8, 27.1, 27.3];
            (*phys).brake_temp = [450.0, 470.0, 380.0, 390.0];

            (*gfx).packet_id = (*gfx).packet_id.wrapping_add(1);
            (*gfx).status = 2; // AC_LIVE
            (*gfx).completed_laps = lap_count;
            (*gfx).position = 1;
            (*gfx).i_current_time = lap_elapsed_ms;
            (*gfx).i_last_time = 121452;
            (*gfx).i_best_time = 120890;
            (*gfx).session_time_left = (3600.0 - total_elapsed).max(0.0);
            (*gfx).distance_traveled = dist;
            (*gfx).surface_grip = 0.98;
        }

        print!(
            "\r[{:02}:{:02}] Scenario: {:<28} | Spd: {:3.0} km/h | RPM: {:5} | Gear: {} | Fuel: {:4.1}L",
            (total_elapsed as u32) / 60,
            (total_elapsed as u32) % 60,
            scenario_name,
            speed,
            rpm,
            gear,
            fuel
        );
        io::stdout().flush().ok();

        thread::sleep(Duration::from_millis(16));
    }
}
