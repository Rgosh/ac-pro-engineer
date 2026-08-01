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
fn create_shared_memory(name: &str, size: usize) -> Result<*mut u8, Box<dyn std::error::Error>> {
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

/// Lap duration in milliseconds (~30 seconds per lap = one full driving scenario cycle).
const LAP_DURATION_MS: i32 = 30_000;

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
        (*stat).track_spline_length = 5793.0;
        (*stat).car_model = "kunos_ferrari_488_gt3".into();
        (*stat).track = "monza".into();
        (*stat).player_nick = "Simulator_User".into();
    }

    println!(
        "Lap duration: {}s  |  Track: Monza  |  Car: Ferrari 488 GT3",
        LAP_DURATION_MS / 1000
    );
    println!("Simulation started. Press Ctrl+C to stop.\n");

    let start_time = Instant::now();
    let mut lap_start_time = Instant::now();
    let mut lap_count: i32 = 0;
    let mut last_completed_lap_time_ms: i32 = 0;
    let mut best_lap_time_ms: i32 = 0;

    let mut speed: f32 = 120.0;
    let mut dist: f32 = 0.0;
    let mut fuel: f32 = 50.0;
    let mut fuel_at_lap_start: f32 = 50.0;
    let mut fuel_per_lap: f32 = 0.0;

    loop {
        let total_elapsed = start_time.elapsed().as_secs_f32();
        let lap_elapsed_ms = lap_start_time.elapsed().as_millis() as i32;

        // ── Driving scenario — one full cycle per lap ──
        let lap_progress = (lap_elapsed_ms as f32) / (LAP_DURATION_MS as f32);
        let scenario_phase = lap_progress.fract(); // 0.0..1.0

        let (gas, brake, steer, gear, lat_g, lon_g, wheel_slip, scenario_name) =
            if scenario_phase < 0.27 {
                // Sector 1: Full throttle down the main straight
                let g = 4 + (speed / 50.0) as i32;
                (
                    1.0f32,
                    0.0f32,
                    0.0f32,
                    g.min(7),
                    0.0f32,
                    0.6f32,
                    [0.02, 0.02, 0.03, 0.03],
                    "S1: FULL THROTTLE (MAIN STRAIGHT)",
                )
            } else if scenario_phase < 0.40 {
                // Sector 1→2 transition: Heavy braking with FL lockup (driver error)
                (
                    0.0,
                    0.95,
                    -0.35,
                    2,
                    -1.4,
                    -1.8,
                    [0.45, 0.05, 0.04, 0.04],
                    "S1: DRIVER ERROR — FL LOCKUP",
                )
            } else if scenario_phase < 0.55 {
                // Sector 2: Tight chicane cornering
                (
                    0.35,
                    0.0,
                    0.48,
                    3,
                    2.35,
                    0.1,
                    [0.08, 0.09, 0.07, 0.07],
                    "S2: CHICANE APEX (MAX LAT-G)",
                )
            } else if scenario_phase < 0.72 {
                // Sector 2→3: Trail-braking into Ascari
                (
                    0.15,
                    0.40,
                    0.25,
                    3,
                    1.6,
                    -0.8,
                    [0.06, 0.07, 0.05, 0.06],
                    "S2: TRAIL BRAKING INTO ASCARI",
                )
            } else if scenario_phase < 0.82 {
                // Sector 3: Snap oversteer on Parabolica exit (driver error)
                (
                    0.90,
                    0.0,
                    -0.30,
                    4,
                    -1.90,
                    0.4,
                    [0.05, 0.05, 0.38, 0.35],
                    "S3: DRIVER ERROR — SNAP OVERSTEER",
                )
            } else {
                // Sector 3: Recovery & full throttle exit
                (
                    0.95,
                    0.0,
                    0.05,
                    5,
                    0.3,
                    0.4,
                    [0.03, 0.03, 0.04, 0.04],
                    "S3: CORNER EXIT & FULL THROTTLE",
                )
            };

        // ── Physics model ──
        if gas > 0.5 {
            speed = (speed + 2.8).min(295.0);
        } else if brake > 0.5 {
            speed = (speed - 4.5).max(75.0);
        } else {
            speed = (speed - 0.3).max(60.0);
        }

        let rpm =
            (3200.0 + (speed / 300.0) * 5600.0 + (total_elapsed * 120.0).sin() * 150.0) as i32;
        dist += (speed / 3.6) * 0.016;
        fuel = (fuel - 0.001f32).max(0.0f32);

        // ── Lap completion ──
        if lap_elapsed_ms >= LAP_DURATION_MS {
            lap_count += 1;

            // Calculate this lap's time with slight random variation
            let variation = ((total_elapsed * 7.3).sin() * 800.0) as i32;
            last_completed_lap_time_ms = LAP_DURATION_MS + variation;

            if best_lap_time_ms == 0 || last_completed_lap_time_ms < best_lap_time_ms {
                best_lap_time_ms = last_completed_lap_time_ms;
            }

            // Fuel per lap calculation
            fuel_per_lap = fuel_at_lap_start - fuel;
            fuel_at_lap_start = fuel;

            println!(
                "\n  ✅ LAP {} COMPLETED: {}:{:02}.{:03}  (best: {}:{:02}.{:03})  fuel/lap: {:.2}L",
                lap_count,
                last_completed_lap_time_ms / 60000,
                (last_completed_lap_time_ms % 60000) / 1000,
                last_completed_lap_time_ms % 1000,
                best_lap_time_ms / 60000,
                (best_lap_time_ms % 60000) / 1000,
                best_lap_time_ms % 1000,
                fuel_per_lap,
            );

            lap_start_time = Instant::now();
        }

        // ── Write to shared memory ──
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
            (*phys).wheel_slip = wheel_slip;
            (*phys).performance_meter = (total_elapsed * 1.3).sin() * 0.5;
            (*phys).air_temp = 23.0;
            (*phys).road_temp = 35.0;

            let temp_base = 82.0 + (speed / 280.0) * 14.0;
            (*phys).tyre_temp_i = [
                temp_base + 4.0,
                temp_base + 3.0,
                temp_base + 2.0,
                temp_base + 2.0,
            ];
            (*phys).tyre_temp_m = [temp_base + 1.0, temp_base, temp_base - 1.0, temp_base];
            (*phys).tyre_temp_o = [
                temp_base - 3.0,
                temp_base - 2.0,
                temp_base - 4.0,
                temp_base - 3.0,
            ];
            (*phys).wheels_pressure = [27.4, 27.6, 27.2, 27.4];
            (*phys).brake_temp = [480.0, 490.0, 390.0, 400.0];
            (*phys).ride_height = [25.0, 55.0];

            (*gfx).packet_id = (*gfx).packet_id.wrapping_add(1);
            (*gfx).status = 2; // AC_LIVE
            (*gfx).completed_laps = lap_count;
            (*gfx).position = 1;
            (*gfx).i_current_time = lap_elapsed_ms;
            (*gfx).i_last_time = last_completed_lap_time_ms;
            (*gfx).i_best_time = best_lap_time_ms;
            (*gfx).session_time_left = ((3600.0 - total_elapsed) * 1000.0).max(0.0);
            (*gfx).distance_traveled = dist;
            (*gfx).surface_grip = 0.98;
            (*gfx).fuel_x_lap = if fuel_per_lap > 0.0 {
                fuel_per_lap
            } else {
                1.8
            };
        }

        print!(
            "\r[{:02}:{:02}] Lap {} {:5.1}s | {:<40} | {:3.0} km/h | {:5} RPM | G{} | {:.1}L",
            (total_elapsed as u32) / 60,
            (total_elapsed as u32) % 60,
            lap_count + 1,
            lap_elapsed_ms as f32 / 1000.0,
            scenario_name,
            speed,
            rpm,
            gear,
            fuel,
        );
        io::stdout().flush().ok();

        thread::sleep(Duration::from_millis(16));
    }
}
