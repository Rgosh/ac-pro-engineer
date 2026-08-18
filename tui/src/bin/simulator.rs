#![allow(unsafe_code)]

use ac_core::games::assetto_corsa::structs::{AcGraphics, AcPhysics, AcStatic};
use ac_core::games::assetto_corsa_competizione::structs::{
    AccGraphics, AccPhysics, AccStatic, CarPositions,
};
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

    // `set_len` to the size the file already has changes nothing, so a page
    // from an earlier run keeps every byte this one does not overwrite — and
    // this writes the fields it cares about, not the whole struct. The same
    // omission in the bridge is how a Huracán at Spa was reported as a Ferrari
    // at Monza.
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut file = &file;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&vec![0u8; size])?;
        file.flush()?;
    }

    let mmap = unsafe { memmap2::MmapMut::map_mut(&file)? };
    let ptr = mmap.as_ptr() as *mut u8;

    std::mem::forget(mmap);
    Ok(ptr)
}

/// Set by the interrupt handler; the loop reads it and leaves.
#[cfg(not(target_os = "windows"))]
static STOPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The pages this writes, so the cleanup names the same ones the creation did.
#[cfg(not(target_os = "windows"))]
const PAGE_NAMES: [&str; 3] = ["acpmf_physics", "acpmf_graphics", "acpmf_static"];

/// **Why the simulator has to clean up after itself.**
///
/// The pages live in `/dev/shm` and outlive the process that made them — until
/// a reboot, or until something else unlinks them. A simulator killed with
/// Ctrl+C therefore leaves a complete, valid-looking session sitting there: the
/// right shared-memory version, a car, a track. The application attaches to it
/// and reports a car nobody is driving.
///
/// That is not hypothetical. It is where "Competizione says I am in a Ferrari
/// 488 at Monza" came from, on a machine whose driver was in a Huracán at Spa:
/// the physics and graphics pages were live, mirrored by the bridge every few
/// milliseconds, and the static page — written once a session — was still this
/// program's, from a run that had ended long before.
///
/// Only an atomic store happens in the handler, which is one of the few things
/// that is safe to do inside one.
#[cfg(not(target_os = "windows"))]
fn stop_on_interrupt() {
    extern "C" fn handler(_signal: libc::c_int) {
        STOPPING.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    // SAFETY: installing a handler that does nothing but store to an atomic.
    unsafe {
        let handler = handler as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}

/// Remove the pages, so nothing reads them as a session tomorrow.
#[cfg(not(target_os = "windows"))]
fn remove_pages() {
    for name in PAGE_NAMES {
        let path = format!("/dev/shm/{name}");
        match std::fs::remove_file(&path) {
            Ok(()) => println!("Removed {path}"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => eprintln!("Could not remove {path}: {error}"),
        }
    }
}

/// Lap duration in milliseconds (~30 seconds per lap = one full driving scenario cycle).
const LAP_DURATION_MS: i32 = 30_000;

/// Which game this run is standing in for.
///
/// The simulator writes a game's pages, so it has to be one game or the other:
/// the two use the same three mapping names with different layouts, and a
/// reader that attached to the wrong one would get numbers rather than an
/// error. That is exactly what it is useful for here — the version at the top
/// of the static page is what the readers refuse on, and this writes it.
#[derive(Clone, Copy, PartialEq)]
enum Stand {
    AssettoCorsa,
    Competizione,
}

/// One tick of the scenario, before it is written in either game's shape.
///
/// Extracted so the two page layouts are two *writers* of the same drive
/// rather than two scenarios that have to be kept in step — which they would
/// not be, and the difference would look like a bug in whichever game was
/// looked at second.
struct Frame {
    gas: f32,
    brake: f32,
    steer: f32,
    gear: i32,
    rpm: i32,
    speed: f32,
    fuel: f32,
    lat_g: f32,
    lon_g: f32,
    wheel_slip: [f32; 4],
    tyre_core: [f32; 4],
    tyre_i: [f32; 4],
    tyre_m: [f32; 4],
    tyre_o: [f32; 4],
    pressures: [f32; 4],
    brake_temp: [f32; 4],
    camber: [f32; 4],
    wear: [f32; 4],
    pad_life: [f32; 4],
    disc_life: [f32; 4],
    suspension_travel: [f32; 4],
    delta: f32,
    lap_count: i32,
    lap_ms: i32,
    last_lap_ms: i32,
    best_lap_ms: i32,
    time_left_ms: f32,
    distance: f32,
    lap_fraction: f32,
    position: [f32; 3],
    fuel_per_lap: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== AC PRO ENGINEER: AUTOMATED TELEMETRY SIMULATOR ===");

    // One flag, and it changes which game's bytes come out. Named after the
    // ids in `games::registry` so there is nothing to translate.
    let stand = match std::env::args().nth(1).as_deref() {
        Some("acc") | Some("assetto_corsa_competizione") => Stand::Competizione,
        Some(other) if other != "ac" && other != "assetto_corsa" => {
            eprintln!("Unknown game {other:?}. Use `ac` (the default) or `acc`.");
            std::process::exit(2);
        }
        _ => Stand::AssettoCorsa,
    };
    println!(
        "Standing in for {}",
        match stand {
            Stand::AssettoCorsa => "Assetto Corsa",
            Stand::Competizione => "Assetto Corsa Competizione",
        }
    );
    // Brake wear moves too slowly to see in a demo: a whole stint takes a
    // couple of millimetres off. This starts the set nearly finished so the
    // advice that reads it can actually be looked at.
    let worn_brakes = std::env::var("SIM_WORN_BRAKES").is_ok();
    let (pad_start, disc_start) = if worn_brakes {
        (9.0f32, 28.5f32)
    } else {
        (29.0f32, 32.0f32)
    };

    println!("Initializing shared memory...");

    // The mappings are sized for the game being stood in for. A reader maps
    // its own struct's worth and refuses anything shorter, so a Competizione
    // run writing Assetto Corsa's 596 bytes would be refused by the kernel on
    // Windows and by `SharedMemory::get` everywhere else.
    let (physics_bytes, graphics_bytes, static_bytes) = match stand {
        Stand::AssettoCorsa => (
            size_of::<AcPhysics>(),
            size_of::<AcGraphics>(),
            size_of::<AcStatic>(),
        ),
        Stand::Competizione => (
            size_of::<AccPhysics>(),
            size_of::<AccGraphics>(),
            size_of::<AccStatic>(),
        ),
    };

    #[cfg(target_os = "windows")]
    let (_h_phys, phys_ptr) = create_shared_memory("Local\\acpmf_physics", physics_bytes)?;
    #[cfg(target_os = "windows")]
    let (_h_gfx, gfx_ptr) = create_shared_memory("Local\\acpmf_graphics", graphics_bytes)?;
    #[cfg(target_os = "windows")]
    let (_h_stat, stat_ptr) = create_shared_memory("Local\\acpmf_static", static_bytes)?;

    #[cfg(not(target_os = "windows"))]
    let phys_ptr = create_shared_memory("Local\\acpmf_physics", physics_bytes)?;
    #[cfg(not(target_os = "windows"))]
    let gfx_ptr = create_shared_memory("Local\\acpmf_graphics", graphics_bytes)?;
    #[cfg(not(target_os = "windows"))]
    let stat_ptr = create_shared_memory("Local\\acpmf_static", static_bytes)?;

    let phys = phys_ptr as *mut AcPhysics;
    let gfx = gfx_ptr as *mut AcGraphics;
    let stat = stat_ptr as *mut AcStatic;

    let acc_phys = phys_ptr as *mut AccPhysics;
    let acc_gfx = gfx_ptr as *mut AccGraphics;
    let acc_stat = stat_ptr as *mut AccStatic;

    // The static page, written once. Its first field is the shared-memory
    // version, and it is what tells the two games apart — a simulator that
    // left it empty would be readable by both, which is the one thing the
    // real games are not.
    unsafe {
        match stand {
            Stand::AssettoCorsa => {
                for (slot, unit) in (*stat).sm_version.iter_mut().zip("1.7".encode_utf16()) {
                    *slot = unit;
                }
                (*stat).max_rpm = 9000;
                (*stat).max_fuel = 120.0;
                (*stat).track_spline_length = 5793.0;
                (*stat).car_model = "kunos_ferrari_488_gt3".into();
                (*stat).track = "monza".into();
                (*stat).player_nick = "Simulator_User".into();
            }
            Stand::Competizione => {
                for (slot, unit) in (*acc_stat).sm_version.iter_mut().zip("1.9".encode_utf16()) {
                    *slot = unit;
                }
                (*acc_stat).max_rpm = 9000;
                (*acc_stat).max_fuel = 120.0;
                // No track length: ACC does not publish one, and a simulator
                // that invented one would hide every place that has to cope.
                (*acc_stat).sector_count = 3;
                (*acc_stat).car_model = "ferrari_488_gt3_evo".into();
                (*acc_stat).track = "monza".into();
                (*acc_stat).player_name = "Simulator".into();
                (*acc_stat).player_surname = "User".into();
                (*acc_stat).dry_tyres_name = "DHE".into();
                (*acc_stat).wet_tyres_name = "WH".into();
            }
        }
    }

    println!(
        "Lap duration: {}s  |  Track: Monza  |  Car: Ferrari 488 GT3",
        LAP_DURATION_MS / 1000
    );
    println!("Simulation started. Press Ctrl+C to stop.\n");

    // Registered here rather than at the top of main: there is nothing worth
    // cleaning up until the pages exist.
    #[cfg(not(target_os = "windows"))]
    stop_on_interrupt();

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

        // ── One tick, in whichever game's shape ──
        let temp_base = 82.0 + (speed / 280.0) * 14.0;
        let laps_done = total_elapsed / (LAP_DURATION_MS as f32 / 1000.0);
        // Trace a closed loop so the track map has something to plot. x/z are
        // the ground plane and y the altitude; the shape matches the one the
        // TUI draws for demo mode so the two look alike side by side.
        let angle = scenario_phase * std::f32::consts::TAU;

        let frame = Frame {
            gas,
            brake,
            steer,
            gear,
            rpm,
            speed,
            fuel,
            lat_g,
            lon_g,
            wheel_slip,
            tyre_core: [temp_base + 1.0, temp_base, temp_base - 1.0, temp_base],
            tyre_i: [
                temp_base + 4.0,
                temp_base + 3.0,
                temp_base + 2.0,
                temp_base + 2.0,
            ],
            tyre_m: [temp_base + 1.0, temp_base, temp_base - 1.0, temp_base],
            tyre_o: [
                temp_base - 3.0,
                temp_base - 2.0,
                temp_base - 4.0,
                temp_base - 3.0,
            ],
            pressures: [27.4, 27.6, 27.2, 27.4],
            brake_temp: [480.0, 490.0, 390.0, 400.0],
            // AC reports camber per wheel, in the wheel's own frame, so the
            // two sides mirror: the same negative camber reads negative on the
            // left and positive on the right. Left at zero, everything that
            // reads this field showed a car with no camber at all.
            camber: [
                (-1.3f32).to_radians(),
                1.3f32.to_radians(),
                (-2.0f32).to_radians(),
                2.0f32.to_radians(),
            ],
            // Wear counts down from 100 the way AC publishes it, rears faster
            // than fronts. Leaving it at zero made every consumer read four
            // destroyed tyres — the numbers a simulator does not write are the
            // ones its users end up debugging.
            wear: [
                (100.0 - laps_done * 0.9).max(0.0),
                (100.0 - laps_done * 0.9).max(0.0),
                (100.0 - laps_done * 1.2).max(0.0),
                (100.0 - laps_done * 1.2).max(0.0),
            ],
            // Millimetres of pad and disc left, which is what Competizione
            // publishes in place of tyre wear.
            // A GT3 stint takes a millimetre or two off the pads, so the rate
            // here is the real one — which means a demo run never reaches the
            // warning. `SIM_WORN_BRAKES=1` starts the set most of the way
            // through instead, which is the only way to see that advice
            // without driving for an hour.
            pad_life: [(pad_start - laps_done * 0.04).max(0.0); 4],
            disc_life: [(disc_start - laps_done * 0.02).max(0.0); 4],
            // Metres of travel left, which both games publish and neither
            // writer used to. Zero is not "no reading" to the engineer — it is
            // a car on its bump stops, and thirty frames of it is a CRITICAL
            // "chassis bottoming out". The demo raised it on every lap, which
            // is the kind of false alarm that teaches a driver to ignore the
            // panel. It squats a little under load, the way a real one does.
            suspension_travel: [
                (0.030 - lon_g.abs() * 0.004).max(0.001),
                (0.030 - lon_g.abs() * 0.004).max(0.001),
                (0.034 - lon_g.abs() * 0.005).max(0.001),
                (0.034 - lon_g.abs() * 0.005).max(0.001),
            ],
            delta: (total_elapsed * 1.3).sin() * 0.5,
            lap_count,
            lap_ms: lap_elapsed_ms,
            last_lap_ms: last_completed_lap_time_ms,
            best_lap_ms: best_lap_time_ms,
            time_left_ms: ((3600.0 - total_elapsed) * 1000.0).max(0.0),
            distance: dist,
            lap_fraction: scenario_phase,
            position: [
                400.0 * angle.cos() + 50.0 * (2.0 * angle).cos(),
                0.0,
                250.0 * angle.sin() + 30.0 * (3.0 * angle).sin(),
            ],
            fuel_per_lap: if fuel_per_lap > 0.0 {
                fuel_per_lap
            } else {
                1.8
            },
        };

        unsafe {
            match stand {
                Stand::AssettoCorsa => write_assetto_corsa(phys, gfx, &frame),
                Stand::Competizione => write_competizione(acc_phys, acc_gfx, &frame),
            }
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

        #[cfg(not(target_os = "windows"))]
        if STOPPING.load(std::sync::atomic::Ordering::SeqCst) {
            println!("\nStopping.");
            break;
        }
    }

    // Only reachable on the platform that has something to remove; on Windows
    // the loop above never ends and the section dies with the process.
    #[cfg(not(target_os = "windows"))]
    remove_pages();

    #[allow(unreachable_code)]
    Ok(())
}

/// Assetto Corsa's two live pages.
///
/// # Safety
///
/// `phys` and `gfx` must point at mappings at least as large as the structs.
unsafe fn write_assetto_corsa(phys: *mut AcPhysics, gfx: *mut AcGraphics, f: &Frame) {
    unsafe {
        (*phys).packet_id = (*phys).packet_id.wrapping_add(1);
        (*phys).gas = f.gas;
        (*phys).brake = f.brake;
        (*phys).fuel = f.fuel;
        // AC counts reverse as 0 and neutral as 1, so a scenario gear of 4 is
        // written as 5. The reader subtracts it again.
        (*phys).gear = f.gear + 1;
        (*phys).rpms = f.rpm;
        (*phys).steer_angle = f.steer;
        (*phys).speed_kmh = f.speed;
        (*phys).acc_g = [f.lat_g, 0.0, f.lon_g];
        (*phys).wheel_slip = f.wheel_slip;
        (*phys).performance_meter = f.delta;
        (*phys).air_temp = 23.0;
        (*phys).road_temp = 35.0;
        (*phys).tyre_core_temp = f.tyre_core;
        (*phys).tyre_temp_i = f.tyre_i;
        (*phys).tyre_temp_m = f.tyre_m;
        (*phys).tyre_temp_o = f.tyre_o;
        (*phys).wheels_pressure = f.pressures;
        (*phys).camber_rad = f.camber;
        (*phys).brake_temp = f.brake_temp;
        (*phys).tyre_wear = f.wear;
        (*phys).suspension_travel = f.suspension_travel;
        // Metres, which is what AC publishes — the UI multiplies by 1000 to
        // show millimetres. Writing 25.0 here meant the demo displayed a
        // 25000mm ride height.
        (*phys).ride_height = [0.025, 0.055];

        (*gfx).packet_id = (*gfx).packet_id.wrapping_add(1);
        (*gfx).status = 2; // AC_LIVE
        (*gfx).completed_laps = f.lap_count;
        (*gfx).position = 1;
        (*gfx).i_current_time = f.lap_ms;
        (*gfx).i_last_time = f.last_lap_ms;
        (*gfx).i_best_time = f.best_lap_ms;
        (*gfx).session_time_left = f.time_left_ms;
        (*gfx).distance_traveled = f.distance;
        (*gfx).normalized_car_position = f.lap_fraction;
        (*gfx).surface_grip = 0.98;
        (*gfx).car_coordinates = f.position;
        (*gfx).fuel_x_lap = f.fuel_per_lap;
    }
}

/// Competizione's two live pages: the same drive, in the layout that game
/// publishes.
///
/// Three differences are the point of having this at all, and each is a place
/// the reader has to do something Assetto Corsa's does not:
///
/// * the player's world position is one slot of a sixty-car array;
/// * the aids are split — the level on the graphics page, the intervention on
///   the physics page;
/// * the arrays ACC does not publish are **left at zero**, because that is
///   what the game does, and the capability flags are what stop them being
///   read as measurements.
///
/// # Safety
///
/// `phys` and `gfx` must point at mappings at least as large as the structs.
unsafe fn write_competizione(phys: *mut AccPhysics, gfx: *mut AccGraphics, f: &Frame) {
    unsafe {
        (*phys).packet_id = (*phys).packet_id.wrapping_add(1);
        (*phys).gas = f.gas;
        (*phys).brake = f.brake;
        (*phys).fuel = f.fuel;
        (*phys).gear = f.gear + 1;
        (*phys).rpm = f.rpm;
        (*phys).steer_angle = f.steer;
        (*phys).speed_kmh = f.speed;
        (*phys).acc_g = [f.lat_g, 0.0, f.lon_g];
        (*phys).wheel_slip = f.wheel_slip;
        (*phys).air_temp = 23.0;
        (*phys).road_temp = 35.0;
        (*phys).tyre_core_temp = f.tyre_core;
        (*phys).tyre_temp = f.tyre_core;
        (*phys).wheel_pressure = f.pressures;
        (*phys).brake_temp = f.brake_temp;
        (*phys).suspension_travel = f.suspension_travel;
        (*phys).brake_bias = 0.66;
        (*phys).water_temp = 88.0;
        (*phys).pad_life = f.pad_life;
        (*phys).disc_life = f.disc_life;
        (*phys).current_max_rpm = 9000;
        (*phys).is_engine_running = 1;
        // Deliberately untouched: `tyre_wear`, `camber_rad`, `wheel_load` and
        // the tread triplet. The game leaves them zero and so does this.

        (*gfx).packet_id = (*gfx).packet_id.wrapping_add(1);
        (*gfx).status = 2; // AC_LIVE
        (*gfx).session = 2; // ACC numbers the race 2
        (*gfx).completed_laps = f.lap_count;
        (*gfx).position = 1;
        (*gfx).i_current_time = f.lap_ms;
        (*gfx).i_last_time = f.last_lap_ms;
        (*gfx).i_best_time = f.best_lap_ms;
        (*gfx).session_time_left = f.time_left_ms;
        (*gfx).distance_traveled = f.distance;
        (*gfx).normalized_car_position = f.lap_fraction;
        (*gfx).active_cars = 1;
        (*gfx).player_car_id = 0;
        // Written whole rather than by index: indexing through a raw
        // pointer would take a reference to the array first, which is not
        // allowed here and is not needed.
        let mut cars = CarPositions::default();
        cars[0] = f.position;
        (*gfx).car_coordinates = cars;
        (*gfx).fuel_x_lap = f.fuel_per_lap;
        (*gfx).fuel_estimated_laps = if f.fuel_per_lap > 0.0 {
            f.fuel / f.fuel_per_lap
        } else {
            0.0
        };
        (*gfx).tyre_compound = "dry_compound".into();
        (*gfx).tc = 3;
        (*gfx).abs = 4;
        (*gfx).is_valid_lap = 1;
        (*gfx).track_grip_status = 1;
        (*gfx).current_tyre_set = 2;
        (*gfx).mfd_tyre_pressure_lf = 27.5;
        (*gfx).mfd_tyre_pressure_rf = 27.5;
        (*gfx).mfd_tyre_pressure_lr = 27.0;
        (*gfx).mfd_tyre_pressure_rr = 27.0;
        // Not published by the game: `surface_grip`, wind, and the tread
        // temperatures. See `track_grip_status` above, which is what ACC says
        // instead.
    }
}
