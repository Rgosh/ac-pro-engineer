//! Layout tests for Assetto Corsa Competizione's shared-memory structs.
//!
//! Same argument as `shm_layout_tests.rs` makes for Assetto Corsa, and a
//! sharper one: ACC publishes under **the same three names** with a different
//! layout, so a struct transcribed from somebody's header file and never
//! checked would attach cleanly and read plausible nonsense. Every other test
//! in this workspace builds an `Acc*` value in Rust and reads it back, which
//! round-trips through whatever layout the struct declares and cannot disagree
//! with the game at all.
//!
//! # Where the bytes come from
//!
//! One session, recorded with `tools/record-session.sh` on 16 August 2026:
//! **Assetto Corsa Competizione under Proton, a Lamborghini Huracán GT3 EVO at
//! Spa, 337 seconds, 8376 samples, two laps.** These are the last samples of
//! that recording, whole 2048-byte mappings as the bridge mirrors them, and
//! `assetto-corsa-competizione-20260816-2051.txt` in the repository root is
//! the rest of it — what every four-byte word did over the whole session,
//! which is what identified the fields in the first place.
//!
//! # What makes these worth having
//!
//! A page of zeros pins nothing, because a wrong offset also reads zero. So
//! the assertions below are values, not sizes, and they are values that could
//! not have come from anywhere else: 4.24 litres a lap that 62 litres divides
//! by to give the 14.6 laps the next field holds; 520 °C front brakes against
//! 257 °C rear; a car named at offset 68 and its compounds at 688 and 754.
//!
//! The whole mapping is kept rather than the first `size_of` bytes, which buys
//! the one check a struct-sized capture cannot make: **everything past the end
//! of each struct is zero**, so the structs are not too short. That is the
//! question AC's own graphics capture could not answer for a year.

use ac_core::games::assetto_corsa_competizione::shm::page_is_ours;
use ac_core::games::assetto_corsa_competizione::structs::{AccGraphics, AccPhysics, AccStatic};
use ac_core::games::reading::{COORD_X, COORD_Y, COORD_Z, FL, FR, RL, RR};
use zerocopy::TryFromBytes;

/// The whole `acpmf_physics` mapping: 800 bytes of page, 1248 of nothing.
/// Braking hard in fourth at 166 km/h, two laps in.
const PHYSICS_MAPPING_HEX: &str = concat!(
    "e3fb010000000000a2cf523f00007842050000009b17000000000000b4e62543",
    "65ccc3c1a887973fcbf01bc2c67f07bcf17157bddccabebfc9ab913f0146733f",
    "447abe3e9091bd3e00000000000000000000000000000000700bda417eaad841",
    "464cd4411db1d3412820f542dcb9f9426063fb426063fb420000000000000000",
    "0000000000000000000000000000000000000000000000006348ae4242b1aa42",
    "e5fbaf42d567ae42000000000000000000000000000000007ee35a3c75e6633c",
    "9015863c16178f3c0000000000000000881f25404a65443c10da05bd00000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "000000000000000001000000000000000000000017ecba320000000000000000",
    "5a40d941abe0df416218323e5afcdcbb0b48dabc680b54bd0000000000000000",
    "000000000000000000000000000000000000000000000000000000008de2bc43",
    "08fdbb433d0048435e6e47430000803f00000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "000000005db595c4ca04be4149ee01c5d68595c42e91bd4140fd01c5c88795c4",
    "338cbd4191ca01c5305995c4331abd413cd901c59fdd233d3fcb7f3f4fa4393b",
    "30e8233d29cb7f3fae77553bbd11223d9dcc7f3f90a1b73a0f7e1d3d87cf7f3f",
    "21af123af262083f8063c2bc1e8f583f7009083fc8f3c4bccfc6583f91dd073f",
    "13e5b5bc9ae5583fe193083fae0cacbcff74583f5c8f423ff8f4b6bc660f223f",
    "ec2a38420000000000000000ca21000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "22bfb8bd408c98bd465df5bc9025f0bcb03ec23aed006b395678d7b9adbaad3a",
    "0000000000000000000000000000000000000000000000006348ae4242b1aa42",
    "e5fbaf42d567ae4242f4a7426737203f6737203fee604a3eee604a3e01000000",
    "010000000100e8410100e8410100e8410100e841000000420000004200000042",
    "0000004201000000000000000100000000000000d700823ec487bc3c00000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
);

/// The whole `acpmf_graphics` mapping: 1588 bytes of page, 460 of nothing.
const GRAPHICS_MAPPING_HEX: &str = concat!(
    "ad9b0000020000000000000030003a00300034003a0039003100300000000000",
    "0000000000000000000032003a00340031003a00360037003500000000000000",
    "0000000000000000330035003700390031003a00320033003a00360034003700",
    "00000000000030003a00300034003a0039003100300000000000000000000000",
    "0000000001000000010000002e1300008b770200ffffff7f000080bf80f24e46",
    "000000000000000000000000000000006400720079005f0063006f006d007000",
    "6f0075006e006400000000000000000000000000000000000000000000000000",
    "000000000000000000000000000000000000000000000000b23f013d01000000",
    "677d95c41c91c0418cdc01c50000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000001000000000000000000000000000000",
    "0000000000000000000000000400000000000000030000000000000000000000",
    "0400000014ae87400000000000000000000000002539e0430000000018fcffff",
    "d037fbff0000000000000000000000002d003a002d002d003a002d002d002d00",
    "0000000000000000000000000000000000000000330035003700390031003a00",
    "320033003a003600340037000000000000000000ffffff7f000000002e130000",
    "0100000058f6694111042b042104220420042e00000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "000000000000000000000000ff00000005a0ff46000000000000000000000000",
    "0000000000000000000000000000000001000000000000000000000000000000",
    "00000000cdcccc41cdcccc416766c6416766c641010000000000000000000000",
    "0000000002000000020000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
);

/// The whole `acpmf_static` mapping: 820 bytes of page, 1228 of nothing.
const STATIC_MAPPING_HEX: &str = concat!(
    "31002e0039000000000000000000000000000000000000000000000000003100",
    "2e00370000000000000000000000000000000000000000000000000000000000",
    "010000006c0061006d0062006f0072006700680069006e0069005f0068007500",
    "72006100630061006e005f006700740033005f00650076006f00000000000000",
    "0000000000005300700061000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "000000000000000041006e006400720065006100000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000430061006c0064006100720065006c006c0069000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000000000000000030000000000000000000000ca210000",
    "0000f04200000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "000000000000803f010000000100000000000000000000000000000000000000",
    "00000000000000000000000074007200610063006b00200063006f006e006600",
    "6900670000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000073006b00",
    "69006e0000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "000000000000000018fcffff0000000044004800440032000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000005700480000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
);

fn decode_hex(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "hex must be byte-aligned");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect()
}

/// The bytes of one page, and the proof that the struct describing it is
/// neither too long nor too short.
///
/// Too long is caught by the mapping being smaller than the struct. Too short
/// is caught by anything non-zero *after* the struct ends: the bridge maps
/// 2048 bytes per page and a fresh mapping is zero-filled, so a byte the game
/// wrote past our idea of the end is a field we do not know about.
fn page<T>(hex: &str, what: &str) -> Vec<u8> {
    let mapping = decode_hex(hex);
    let size = size_of::<T>();
    assert!(
        mapping.len() >= size,
        "the captured {what} mapping is {} bytes and the struct wants {size}",
        mapping.len()
    );
    assert!(
        mapping[size..].iter().all(|byte| *byte == 0),
        "the {what} page has data past offset {size}, where the struct ends — \
         ACC writes a field this build does not know about"
    );
    mapping[..size].to_vec()
}

fn physics_page() -> Vec<u8> {
    page::<AccPhysics>(PHYSICS_MAPPING_HEX, "physics")
}

fn graphics_page() -> Vec<u8> {
    page::<AccGraphics>(GRAPHICS_MAPPING_HEX, "graphics")
}

fn static_page() -> Vec<u8> {
    page::<AccStatic>(STATIC_MAPPING_HEX, "static")
}

/// ACC pads its fixed-width `wchar_t` fields with NULs.
fn utf16_to_string(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

// All three go through the same call `SharedMemory::get` makes on the real
// mapping — zerocopy's `try_read_from_bytes`.
fn parse_physics() -> AccPhysics {
    AccPhysics::try_read_from_bytes(&physics_page()).expect("a real ACC page must parse")
}

fn parse_graphics() -> AccGraphics {
    AccGraphics::try_read_from_bytes(&graphics_page()).expect("a real ACC page must parse")
}

fn parse_static() -> AccStatic {
    AccStatic::try_read_from_bytes(&static_page()).expect("a real ACC page must parse")
}

/// Deliberately separate from the parsing tests: if the page and the struct
/// disagree about where the page ends, that alone is the bug, whatever the
/// fields decode to.
#[test]
fn each_page_ends_where_its_struct_ends() {
    let _ = physics_page();
    let _ = graphics_page();
    let _ = static_page();
}

/// The last sample of the recording: braking hard in fourth gear, two laps in,
/// on a hot afternoon at Spa. Every one of these has to be consistent with
/// that one moment at once.
#[test]
fn physics_decodes_a_car_braking_in_fourth() {
    let p = parse_physics();
    assert_eq!(p.packet_id, 130_019);
    assert_eq!(p.gas, 0.0, "off the throttle");
    assert!(p.brake > 0.8, "hard on the brakes: {}", p.brake);
    assert_eq!(p.gear, 5, "ACC counts reverse as 0, so 5 is fourth");
    assert_eq!(p.rpm, 6043);
    assert!(
        (p.speed_kmh - 165.9).abs() < 0.1,
        "speed_kmh = {}",
        p.speed_kmh
    );
    assert_eq!(p.fuel, 62.0);
    assert_eq!(p.is_engine_running, 1, "and the engine is running");
}

/// GT3 carbon brakes, and the reason the thresholds chosen for Assetto Corsa's
/// road cars are the first thing that will lie on this game: the fronts run at
/// 380 °C where they sit, and reached 520 °C during the session.
#[test]
fn brake_temperatures_are_a_gt3s_and_the_fronts_are_hotter() {
    let p = parse_physics();
    let front = (p.brake_temp[FL] + p.brake_temp[FR]) / 2.0;
    let rear = (p.brake_temp[RL] + p.brake_temp[RR]) / 2.0;

    assert!(
        (300.0..=600.0).contains(&front),
        "front brakes at {front} °C is not a GT3 under braking"
    );
    assert!(
        front > rear + 100.0,
        "fronts {front} °C should run far hotter than rears {rear} °C"
    );
}

/// Brake bias at 564 and brake pressure at 716 are 152 bytes apart and
/// measure the same thing from two directions: 76% of the pressure goes to the
/// front axle, and the bias reads 0.76.
#[test]
fn brake_bias_and_per_corner_pressure_agree() {
    let p = parse_physics();
    assert!((p.brake_bias - 0.76).abs() < 0.001, "bias {}", p.brake_bias);

    let front = p.brake_pressure[FL];
    let rear = p.brake_pressure[RL];
    let share = front / (front + rear);
    assert!(
        (share - p.brake_bias).abs() < 0.02,
        "pressure split {share:.3} does not match bias {}",
        p.brake_bias
    );
}

/// The fields ACC publishes and Assetto Corsa does not, all of them past
/// offset 580 where AC's page has already ended. Brake wear is the one that
/// matters: ACC withholds tyre wear and measures the pads instead.
#[test]
fn competiziones_own_measurements_are_there() {
    let p = parse_physics();

    assert_eq!(p.current_max_rpm, 8650, "the rev limit in force");
    assert!(
        (80.0..=110.0).contains(&p.water_temp),
        "water at {} °C is not a warmed-up engine",
        p.water_temp
    );
    assert!(
        p.pad_life.iter().all(|mm| (mm - 29.0).abs() < 0.001),
        "pad thickness in mm: {:?}",
        p.pad_life
    );
    assert!(
        p.disc_life.iter().all(|mm| (mm - 32.0).abs() < 0.001),
        "disc thickness in mm: {:?}",
        p.disc_life
    );
    assert_eq!(p.front_brake_compound, 1);
    assert_eq!(p.rear_brake_compound, 1);

    // Slip ratio and angle are small and negative under braking in a straight
    // line, which is what the car was doing.
    assert!(p.slip_ratio.iter().all(|s| (-0.5..=0.0).contains(s)));
    assert!(p.slip_angle.iter().all(|a| a.abs() < 0.05));
}

/// The six arrays ACC leaves empty, asserted as empty on purpose.
///
/// This is the test that decides two capability flags, so it has to fail if a
/// later build of the game starts filling them — at which point the flags are
/// wrong and the engineer is withholding advice it could give.
#[test]
fn the_arrays_competizione_does_not_publish_are_all_zero() {
    let p = parse_physics();
    assert_eq!(p.wheel_load, [0.0; 4]);
    assert_eq!(p.tyre_wear, [0.0; 4], "and this is why tyre_wear is false");
    assert_eq!(p.tyre_dirty_level, [0.0; 4]);
    assert_eq!(p.camber_rad, [0.0; 4]);
    assert_eq!(p.tyre_temp_i, [0.0; 4], "no tread temperatures either");
    assert_eq!(p.tyre_temp_m, [0.0; 4]);
    assert_eq!(p.tyre_temp_o, [0.0; 4]);
}

/// `tyre_core_temp` at 152 and `tyre_temp` at 696 are the same four numbers
/// 544 bytes apart. Nothing else in the page repeats itself like that, so both
/// being aligned is the only way they can agree.
#[test]
fn the_two_tyre_temperature_arrays_are_the_same_four_tyres() {
    let p = parse_physics();
    assert_eq!(p.tyre_core_temp, p.tyre_temp);
    assert!(
        p.tyre_core_temp.iter().all(|t| (70.0..=110.0).contains(t)),
        "working temperature: {:?}",
        p.tyre_core_temp
    );
}

/// The regression that gave this project its scar, from the other side. AC's
/// graphics page has the player's three coordinates at 252; ACC has a car
/// count there and sixty cars' worth after it, and that is the 964 bytes.
#[test]
fn the_car_coordinates_are_an_array_and_the_player_is_in_it() {
    let g = parse_graphics();
    assert_eq!(g.active_cars, 1, "a single-car practice session");
    assert_eq!(g.player_car_id, 0);

    let me = g.car_coordinates.of(g.player_car_id).expect("the player");
    // Spa sits a few hundred metres up and the pit straight is a long way from
    // the origin; a misread lands at zero or orders of magnitude away.
    assert!(
        (-1300.0..=-1100.0).contains(&me[COORD_X]),
        "x = {}",
        me[COORD_X]
    );
    assert!((0.0..=100.0).contains(&me[COORD_Y]), "y = {}", me[COORD_Y]);
    assert!(
        (-2200.0..=-2000.0).contains(&me[COORD_Z]),
        "z = {}",
        me[COORD_Z]
    );

    // Nobody else is on track, so every other slot is empty. That is also what
    // makes the check above meaningful: it cannot be another car's position.
    assert!(
        g.car_coordinates.as_slice()[1..]
            .iter()
            .all(|car| *car == [0.0; 3])
    );
}

/// The graphics and physics pages are two structs written by one game at one
/// instant, so the player's car has to be in the same place in both — within a
/// wheelbase, since one is the car and the other is its front-left contact
/// patch.
#[test]
fn the_two_pages_put_the_car_in_the_same_place() {
    let g = parse_graphics();
    let p = parse_physics();

    let car = g.car_coordinates.of(g.player_car_id).expect("the player");
    let wheel = p.tyre_contact_point[FL];
    for axis in [COORD_X, COORD_Y, COORD_Z] {
        assert!(
            (car[axis] - wheel[axis]).abs() < 5.0,
            "axis {axis}: the car is at {} and its front-left wheel at {}",
            car[axis],
            wheel[axis]
        );
    }
}

/// Formatted and numeric lap times are 128 bytes apart and independently
/// written, so agreeing is strong evidence the front of the page is aligned.
#[test]
fn formatted_and_numeric_lap_times_agree() {
    let g = parse_graphics();
    assert_eq!(g.i_current_time, 4910);
    assert_eq!(utf16_to_string(&g.current_time), "0:04:910");
    assert_eq!(g.i_last_time, 161_675);
    assert_eq!(utf16_to_string(&g.last_time), "2:41:675");
    assert_eq!(g.completed_laps, 1);
    assert_eq!(
        g.i_best_time,
        i32::MAX,
        "ACC says 'no best lap' with i32::MAX, not zero"
    );
}

/// `fuel_x_lap` is a **float**, and the field 128 bytes after it is what
/// proves it: 62 litres in the tank divided by 4.24 a lap is the 14.6 laps
/// `fuel_estimated_laps` holds. Read as the `i32` one published binding
/// declares, it would be 1082633748.
#[test]
fn fuel_per_lap_is_a_float_and_the_game_agrees_with_the_arithmetic() {
    let g = parse_graphics();
    let p = parse_physics();

    assert!((g.fuel_x_lap - 4.24).abs() < 0.01, "{} L/lap", g.fuel_x_lap);
    let implied = p.fuel / g.fuel_x_lap;
    assert!(
        (implied - g.fuel_estimated_laps).abs() < 0.1,
        "{} L at {} L/lap is {implied:.1} laps, and the game says {}",
        p.fuel,
        g.fuel_x_lap,
        g.fuel_estimated_laps
    );
}

/// Everything past offset 1300 — the half of the graphics page that has no
/// counterpart in Assetto Corsa at all. Each of these is a feature ACC makes
/// possible, and each is at an offset a wrong struct would have missed.
#[test]
fn the_tail_of_the_graphics_page_is_competiziones_own() {
    let g = parse_graphics();

    assert_eq!(g.is_valid_lap, 1, "the lap being driven still counts");
    assert_eq!(g.i_split, 4910);
    assert_eq!(g.tc, 3, "the levels the driver dialled in");
    assert_eq!(g.abs, 4);
    assert_eq!(g.current_tyre_set, 2);
    assert_eq!(g.strategy_tyre_set, 2);
    assert_eq!(g.track_grip_status, 1, "AC_FAST");
    assert_eq!(g.rain_intensity, 0, "no rain, now or forecast");
    assert_eq!(g.rain_intensity_in_10min, 0);
    assert_eq!(g.rain_intensity_in_30min, 0);
    assert_eq!(g.global_green, 1);
    assert_eq!(g.global_red, 0);

    // What the driver has set up for the next stop, which is the field that
    // makes setup attribution answerable on this game and no other.
    for (corner, dialled, expected) in [
        ("LF", g.mfd_tyre_pressure_lf, 25.6),
        ("RF", g.mfd_tyre_pressure_rf, 25.6),
        ("LR", g.mfd_tyre_pressure_lr, 24.8),
        ("RR", g.mfd_tyre_pressure_rr, 24.8),
    ] {
        assert!(
            (dialled - expected).abs() < 0.01,
            "{corner} set to {dialled} psi, expected {expected}"
        );
    }

    // A label in whatever language the game is running in, which is a reminder
    // that it is a label and not a key to match on.
    assert!(
        !utf16_to_string(&{
            let mut units = [0u16; 33];
            let text = g.track_status.to_string();
            for (slot, unit) in units.iter_mut().zip(text.encode_utf16()) {
                *slot = unit;
            }
            units
        })
        .is_empty(),
        "the track status is a non-empty word"
    );

    // Time of day, and a session with no clock.
    assert!((g.clock - 32_720.0).abs() < 1.0, "clock = {}", g.clock);
    assert_eq!(g.session_time_left, -1.0);
}

/// The static page is pinned by its strings, which are the least likely thing
/// in a page to be at the right offset by accident: the car at 68, the track
/// at 134, and the two compound names at 688 and 754.
#[test]
fn the_static_page_names_the_car_the_track_and_the_compounds() {
    let s = parse_static();

    assert_eq!(utf16_to_string(&s.sm_version), "1.9", "ACC's contract");
    assert_eq!(s.car_model.to_string(), "lamborghini_huracan_gt3_evo");
    assert_eq!(s.track.to_string(), "Spa");
    assert_eq!(s.player_name.to_string(), "Andrea");
    assert_eq!(s.player_surname.to_string(), "Caldarelli");
    assert_eq!(s.dry_tyres_name.to_string(), "DHD2");
    assert_eq!(
        s.wet_tyres_name.to_string(),
        "WH",
        "and it starts at 754, not 756 — two-byte alignment, so reading it \
         four bytes in loses the W"
    );

    assert_eq!(s.sector_count, 3);
    assert_eq!(s.max_rpm, 8650);
    assert_eq!(s.max_fuel, 120.0);
}

/// **There is no track length on this game.** Every one of these reads zero,
/// and the consequence is a decision rather than a bug: anything that would
/// report metres has to say "not measured".
#[test]
fn the_static_page_publishes_no_geometry() {
    let s = parse_static();
    assert_eq!(s.track_spline_length, 0.0);
    assert_eq!(s.tyre_radius, [0.0; 4]);
    assert_eq!(s.suspension_max_travel, [0.0; 4]);
    assert_eq!(s.max_torque, 0.0);
    assert_eq!(s.max_power, 0.0);
}

/// The check no single page can make: these came from one session, so they
/// have to agree with each other, and they are three separate structs.
#[test]
fn the_pages_agree_with_each_other() {
    let p = parse_physics();
    let g = parse_graphics();
    let s = parse_static();

    assert!(
        p.fuel > 0.0 && p.fuel <= s.max_fuel,
        "{} L on board does not fit the {} L tank",
        p.fuel,
        s.max_fuel
    );
    assert!(
        p.rpm <= s.max_rpm && p.current_max_rpm == s.max_rpm,
        "{} rpm against a {} limit, and the physics page says {}",
        p.rpm,
        s.max_rpm,
        p.current_max_rpm
    );
    assert!(
        g.completed_laps > 0 && g.distance_traveled > 10_000.0,
        "two laps of Spa is more than 10 km: {} m over {} laps",
        g.distance_traveled,
        g.completed_laps
    );
}

/// A page one field longer or shorter must be refused rather than parsed from
/// whatever happens to be adjacent.
#[test]
fn a_wrong_sized_page_does_not_parse() {
    let good = graphics_page();

    let mut short = good.clone();
    short.pop();
    assert!(AccGraphics::try_read_from_bytes(&short).is_err());

    let mut long = good;
    long.extend_from_slice(&[0u8; 4]);
    assert!(AccGraphics::try_read_from_bytes(&long).is_err());
}

/// The discriminator, against the real thing: Competizione's own pages are
/// accepted.
///
/// The other direction — Assetto Corsa's captured bytes refused by this
/// reader — is in `assetto_corsa_tests.rs`, where AC's capture lives.
#[test]
fn competiziones_own_pages_are_recognised() {
    assert!(page_is_ours(&parse_static()).is_ok());
}

// ── The discriminator, with both games' real bytes ───────────────────────────
//
// The whole reason this pair of games needs one: `acpmf_physics` means
// Assetto Corsa on one machine and Competizione on the next, and on Linux it
// means whichever of them published last into the same file. The mapped size
// is no help — the bridge maps 2048 bytes per page whatever wrote them — so
// the check is on what the page says about itself.

/// AC's captured static page, padded the way the bridge's 2048-byte mapping
/// pads it, so ACC's longer struct has something to read.
///
/// This is not a contrivance: on Linux it is exactly what ACC's reader would
/// find if it attached while Assetto Corsa was the game publishing. (On
/// Windows the game creates a 688-byte mapping and the request for 820 fails
/// in the kernel, which is a second gate this one does not need.)
fn assetto_corsas_static_page_padded_to_ours() -> Vec<u8> {
    let mut bytes = decode_hex(crate::assetto_corsa_tests::STATIC_PAGE_HEX);
    assert!(
        bytes.len() < size_of::<AccStatic>(),
        "Assetto Corsa's static page used to be shorter than Competizione's"
    );
    bytes.resize(size_of::<AccStatic>(), 0);
    bytes
}

/// Assetto Corsa's bytes, in Competizione's reader.
///
/// Note what happens without the check: the page *parses*. Every assertion
/// below the refusal is what the reader would have believed.
#[test]
fn assetto_corsas_pages_are_refused_by_competiziones_reader() {
    let bytes = assetto_corsas_static_page_padded_to_ours();
    let stat = AccStatic::try_read_from_bytes(&bytes)
        .expect("this is the point: the wrong page parses cleanly");

    let refusal = page_is_ours(&stat).expect_err("Assetto Corsa is not Competizione");
    assert!(refusal.contains("1.7"), "{refusal}");

    // What it would have read: AC's `ac_version` where ACC has none of its
    // fields, and a car whose name lands right because the front of the two
    // pages agrees — which is precisely why a plausibility check on one field
    // would not have been enough.
    assert_eq!(stat.car_model.to_string(), "abarth500");
    assert_eq!(stat.max_rpm, 6500, "and nothing about this looks wrong");
}

/// And the other way round: Competizione's bytes in Assetto Corsa's reader,
/// which is the direction that actually shipped once — the AC graphics struct
/// carried ACC's layout and every field past `car_coordinates` read 964 bytes
/// late.
#[test]
fn competiziones_pages_are_refused_by_assetto_corsas_reader() {
    use ac_core::games::assetto_corsa::shm::page_is_ours as ac_page_is_ours;
    use ac_core::games::assetto_corsa::structs::AcStatic;

    // AC's reader maps `size_of::<AcStatic>()` bytes and reads them, which on
    // a Competizione mapping is the first 688 of 820.
    let bytes = &static_page()[..size_of::<AcStatic>()];
    let stat = AcStatic::try_read_from_bytes(bytes).expect("the wrong page parses cleanly");

    let refusal = ac_page_is_ours(&stat).expect_err("Competizione is not Assetto Corsa");
    assert!(refusal.contains("1.9"), "{refusal}");

    // The car and track names land, because both games write them at the same
    // offsets. Everything past the aids does not — `track_spline_length` reads
    // ACC's empty one, and a lap distance divided by it is a division by zero.
    assert_eq!(stat.car_model.to_string(), "lamborghini_huracan_gt3_evo");
    assert_eq!(stat.track_spline_length, 0.0);
}
