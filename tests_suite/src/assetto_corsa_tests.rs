//! Layout tests for the Assetto Corsa shared-memory structs.
//!
//! Every other test in this workspace builds an `Ac*` value in Rust and reads
//! it back, so it round-trips through whatever layout the struct happens to
//! declare and cannot detect a mismatch with the game. `simulator.rs` has the
//! same blind spot: it *writes* the mapping using `size_of::<AcGraphics>()`.
//!
//! These tests instead parse bytes captured verbatim from a live
//! `/dev/shm/acpmf_graphics` (Assetto Corsa 1.16.4, shared-memory version 1.7,
//! Imola, abarth500) through the exact call the app uses — zerocopy's
//! `try_read_from_bytes`. If a field moves, the decoded values stop making
//! sense and these fail.
//!
//! This is what caught the bug they exist to prevent: the graphics struct had
//! ACC's layout (`activeCars` + `carCoordinates[60][3]` + `carID[60]` +
//! `playerCarID` + `penalty`), which is 964 bytes AC never writes, so every
//! field from `car_coordinates` onward read from the wrong offset.
//!
//! All three pages come from one AC run: the graphics page mid-lap, the
//! physics and static pages after returning to the menu, which is why the
//! latter two show a car sitting cold and stationary in the pits.
//!
//! # What this does not cover
//!
//! **Graphics offsets past 296.** The graphics capture is zero from 300 to the
//! end of the page, so the last fifteen fields are asserted by nothing.
//! Reading these tests as covering that page end to end would be a mistake —
//! see the note on the tail fields of `AcGraphics`. Settling it needs a
//! capture taken from lap 2 or later, with TC/ABS set and the headlights on.

use ac_core::games::assetto_corsa::structs::{AcGraphics, AcPhysics, AcStatic};
use ac_core::games::reading::{COORD_X, COORD_Y, COORD_Z};
use zerocopy::TryFromBytes;

/// First 360 bytes of `/dev/shm/acpmf_graphics`, mid-lap at Imola.
pub const GRAPHICS_PAGE_HEX: &str = concat!(
    "e9b70100020000000200000031003a00340033003a0033003900380000000000",
    "000000000000000000002d003a002d002d003a002d002d002d00000000000000",
    "00000000000000002d003a002d002d003a002d002d002d000000000000000000",
    "0000000000002000000000000000000000000000000000000000000000000000",
    "000000000000000007000000e6930100000000000000000013f1c9c72b8c4d44",
    "00000000000000000000000002000000530065006d00690073006c0069006300",
    "6b0073002000280053004d002900000000000000000000000000000000000000",
    "00000000000000000000000000000000000000000000803fa17c1e3eefa501c4",
    "d9a6a4c26389a1c300000000000000000000000000000000be00713f00000000",
    "0000204100000000ffffffff0000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000",
);

/// All 596 bytes of `/dev/shm/acpmf_physics`, car stopped in the pits.
pub const PHYSICS_PAGE_HEX: &str = concat!(
    "d4e0000000000000000000007322ec410100000052030000000000007691e439",
    "0000000000000000000000000000000000000000000000004f00b13c48fba63c",
    "f30f0b3d9156033d3fef5145947f5045694211456dba11456387e1418a99e041",
    "3465c841915dc7410000000000000000f8edcbb9ea4ac7b90000c8420000c842",
    "0000c8420000c842e14f863fd35f913ff63c8e3fe23c983f2b8ed941a1bfd341",
    "8478a041cc089a414554bdbc3270ac3cf88711bdd120073d80959f3da8e89e3d",
    "046bbf3d3aecbf3d00000000cdcccc3d6d13bd3e0134d0bbdad59a3a0011083f",
    "936150418467e7409c1dff4000000000936150410000000000000000ae47e13d",
    "00000000000000000000000000f61b3e00d8473e5f4fce0300000000a01a9f3f",
    "0000404100002041000000000000000000000000175693370000000000000000",
    "0000000000000000000000000000000000000000000000000000000000004041",
    "00004041000040410000404100000000963106425e800442d2d3af41305cac41",
    "92d004429f9600422f51b0417782a641edff0742d40701428a03b6414b2aa541",
    "00000000317428c41954a3c23a8099c2e2c728c47253a3c21e839ac25a3f28c4",
    "5958a3c22cc79dc2d19228c45757a3c290c99ec2c3b0843b76ff7f3fb9a4b5b7",
    "949ed03ab1ff7f3f72682bbb25d3b13bebfe7f3f1a2ff7ba2b5d0a3b72ff7f3f",
    "7f4567bbf245b93ebab5c2baaaa66ebfe456b83eef7745bbb0d46ebfa113ba3e",
    "ec6574bb3c7e6ebfa180b73eb4bf84bba9fd6ebf9a99593f0000000000000000",
    "0000000000000000000000006419000000000000",
);

/// All 688 bytes of `/dev/shm/acpmf_static`, same run.
///
/// The player-name fields are AC's out-of-the-box `Player`, so there is
/// nothing personal in here to leak into the repository.
pub const STATIC_PAGE_HEX: &str = concat!(
    "31002e0037000000000000000000000000000000000000000000000000003100",
    "2e00310036002e00340000000000000000000000000000000000000001000000",
    "0800000061006200610072007400680035003000300000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "00000000000069006d006f006c00610000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "000000000000000050006c006100790065007200000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000050006c00610079006500720000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000000050006c0061007900650072000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000000000000000030000000000bc42ac98e34764190000",
    "00000c4200000000000000007b14ae3d7b14ae3dd6c59d3ed6c59d3ed6c59d3e",
    "d6c59d3ed7a3b03f0000000000000000010000000000803f0000803f0000803f",
    "0000000000000000010000000000000000000000000000000000000000000000",
    "0000000000000000dc0098450000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000030005f00",
    "770068006900740065005f00730063006f007200700069006f006e0000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "00000000000000000000000000000000",
);

fn decode_hex(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "hex must be byte-aligned");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect()
}

/// Decodes a captured page and holds it against the struct that claims to
/// describe it, before any field is read.
fn page<T>(hex: &str, what: &str) -> Vec<u8> {
    let bytes = decode_hex(hex);
    assert_eq!(
        bytes.len(),
        size_of::<T>(),
        "captured {what} page is {} bytes but the struct is {} — it no longer \
         matches the size AC actually publishes",
        bytes.len(),
        size_of::<T>()
    );
    bytes
}

fn graphics_page() -> Vec<u8> {
    page::<AcGraphics>(GRAPHICS_PAGE_HEX, "graphics")
}

fn physics_page() -> Vec<u8> {
    page::<AcPhysics>(PHYSICS_PAGE_HEX, "physics")
}

fn static_page() -> Vec<u8> {
    page::<AcStatic>(STATIC_PAGE_HEX, "static")
}

/// AC pads its fixed-width `wchar_t` fields with NULs.
fn utf16_to_string(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

// All three go through the same call `SharedMemory::get` makes on the real
// mapping — zerocopy's `try_read_from_bytes`.
fn parse() -> AcGraphics {
    AcGraphics::try_read_from_bytes(&graphics_page()).expect("real AC page must parse")
}

fn parse_physics() -> AcPhysics {
    AcPhysics::try_read_from_bytes(&physics_page()).expect("real AC page must parse")
}

fn parse_static() -> AcStatic {
    AcStatic::try_read_from_bytes(&static_page()).expect("real AC page must parse")
}

#[test]
fn graphics_page_size_matches_the_struct() {
    // Deliberately separate from the parse tests: if AC's page and the struct
    // disagree in size, that alone is the bug, whatever the fields decode to.
    let _ = graphics_page();
}

#[test]
fn graphics_scalars_decode_to_live_session_values() {
    let g = parse();
    assert_eq!(g.packet_id, 112617);
    assert_eq!(g.status, 2, "AC_LIVE");
    assert_eq!(g.session, 2, "AC_RACE");
    assert_eq!(g.position, 7);
    assert_eq!(g.number_of_laps, 2);
    assert_eq!(g.completed_laps, 0);
    assert_eq!(g.replay_time_multiplier, 1.0);
}

/// `normalized_car_position` (0..1 around the lap) and `distance_traveled`
/// (metres) are 92 bytes apart and independently written, so dividing one by
/// the other has to land in circuit territory — which it cannot do if either
/// field is misaligned.
///
/// The quotient is an over-estimate, not the track length: `distanceTraveled`
/// accumulates from the grid box while `normalizedCarPosition` is measured
/// from the start/finish line, so the head start inflates it. This capture
/// gives 822 m / 0.1548 = 5312 m at Imola, which is 4.9 km. Hence the wide
/// band below — it is checking the order of magnitude, not the circuit.
#[test]
fn lap_fraction_and_distance_imply_a_real_track_length() {
    let g = parse();
    assert!(
        (0.0..=1.0).contains(&g.normalized_car_position),
        "normalized_car_position = {} is not a lap fraction",
        g.normalized_car_position
    );

    let implied_track_length = g.distance_traveled / g.normalized_car_position;
    assert!(
        (3_000.0..=8_000.0).contains(&implied_track_length),
        "implied track length {implied_track_length:.0} m is not a circuit \
         ({} m travelled at lap fraction {})",
        g.distance_traveled,
        g.normalized_car_position
    );
}

/// `current_time` is a formatted string and `i_current_time` the same value in
/// milliseconds. They are 108 bytes apart, so agreeing is strong evidence the
/// whole front half is aligned rather than coincidentally plausible.
#[test]
fn formatted_and_numeric_lap_time_agree() {
    let g = parse();
    assert_eq!(g.i_current_time, 103_398);
    assert_eq!(utf16_to_string(&g.current_time), "1:43:398");
}

#[test]
fn tyre_compound_decodes_as_utf16() {
    assert_eq!(parse().tyre_compound.to_string(), "Semislicks (SM)");
}

/// The regression itself. Under the old ACC layout `car_coordinates` started 4
/// bytes late, so x read the altitude, y read z, and z read a neighbouring
/// car's x — which in a single-car session was 0.0.
#[test]
fn car_coordinates_are_the_players_world_position() {
    let c = parse().car_coordinates;

    // Imola is roughly 1km across and sits near y=0; a misread lands orders of
    // magnitude away or exactly zero.
    assert!(
        (-1000.0..=1000.0).contains(&c[COORD_X]) && c[COORD_X].abs() > 1.0,
        "x = {} is not a plausible world coordinate",
        c[COORD_X]
    );
    assert!(
        (-1000.0..=1000.0).contains(&c[COORD_Z]) && c[COORD_Z].abs() > 1.0,
        "z = {} is not a plausible world coordinate",
        c[COORD_Z]
    );
    assert!((c[COORD_X] - -518.59).abs() < 0.1, "x = {}", c[COORD_X]);
    assert!((c[COORD_Y] - -82.33).abs() < 0.1, "y = {}", c[COORD_Y]);
    assert!((c[COORD_Z] - -323.07).abs() < 0.1, "z = {}", c[COORD_Z]);
}

/// Fields after `car_coordinates` are the ones the 964-byte overshoot pushed
/// off the end of the page, where they read a constant 0.0. `surface_grip`
/// feeds the cold-pressure calculator and `wind_speed` the strategy tab, so a
/// silent zero is worse than a crash.
///
/// `is_setup_menu_visible` is the last field this capture can speak for. It
/// reads -1, and a fresh mapping is zero-filled, so AC demonstrably writes at
/// least this far — which matters, because the published Kunos struct stops at
/// `wind_direction` and would have the page end at 296. Everything after it is
/// zero here and proves nothing either way.
#[test]
fn fields_after_the_coordinates_are_not_silently_zero() {
    let g = parse();

    assert!(
        (g.surface_grip - 0.9414).abs() < 0.001,
        "surface_grip = {} (0.0 means it is reading past the page again)",
        g.surface_grip
    );
    assert!(
        (g.wind_speed - 10.0).abs() < 0.001,
        "wind_speed = {}",
        g.wind_speed
    );
    assert_eq!(g.wind_direction, 0.0);
    assert_eq!(g.is_setup_menu_visible, -1);

    // Quiet in this capture, but they must still land inside the page.
    assert_eq!(g.penalty_time, 0.0);
    assert_eq!(g.flag, 0, "AC_NO_FLAG");
    assert_eq!(g.is_in_pit_lane, 0);
    assert_eq!(g.mandatory_pit_done, 0);
    assert_eq!(g.rain_tyres, 0);
}

#[test]
fn physics_page_size_matches_the_struct() {
    let _ = physics_page();
}

#[test]
fn static_page_size_matches_the_struct() {
    let _ = static_page();
}

/// Captured with the car parked, which is a state every field has to be
/// consistent with at once: no throttle, no brake, idling, not moving.
#[test]
fn physics_decodes_a_car_stationary_in_the_pits() {
    let p = parse_physics();
    assert_eq!(p.packet_id, 57556);
    assert_eq!(p.gas, 0.0);
    assert_eq!(p.brake, 0.0);
    assert_eq!(p.gear, 1, "AC counts neutral as gear 1");
    assert_eq!(p.rpms, 850, "idling");
    assert!(p.speed_kmh < 0.01, "speed_kmh = {}", p.speed_kmh);
    assert_eq!(p.tyre_wear, [100.0; 4], "AC counts 100 as unworn");
}

/// The abarth500 is front-engined, so the front wheels must carry the heavier
/// share. `wheel_load` is at offset 72 and the total has to come out as a real
/// car's mass — a misalignment breaks the split, the magnitude, or both.
#[test]
fn physics_weight_distribution_is_a_front_engined_car() {
    let p = parse_physics();
    let front = p.wheel_load[0] + p.wheel_load[1];
    let rear = p.wheel_load[2] + p.wheel_load[3];

    assert!(
        front > rear,
        "front axle {front:.0} N should outweigh the rear {rear:.0} N"
    );

    let mass_kg = (front + rear) / 9.81;
    assert!(
        (700.0..=1500.0).contains(&mass_kg),
        "{mass_kg:.0} kg is not a car ({front:.0} N front, {rear:.0} N rear)"
    );
}

/// Three independent temperature readings of a car that has been sitting:
/// ambient at 288, road at 292 and the brakes at 348. They only agree on a
/// cold garage if all three are aligned.
#[test]
fn physics_temperatures_agree_across_the_struct() {
    let p = parse_physics();
    assert_eq!(p.air_temp, 12.0);
    assert_eq!(p.road_temp, 10.0);

    for (i, t) in p.brake_temp.iter().enumerate() {
        assert!(
            (t - 12.0).abs() < 1.0,
            "brake {i} at {t} °C, expected ambient on a car that has not moved"
        );
    }

    // tyre_core_temp (152) and the i/m/o triplet (368/384/400) measure the
    // same four tyres 216 bytes apart, so the warmer pair must be the same.
    assert!(
        p.tyre_core_temp[0] > p.tyre_core_temp[2] && p.tyre_temp_m[0] > p.tyre_temp_m[2],
        "core and surface disagree on which axle is warmer: {:?} vs {:?}",
        p.tyre_core_temp,
        p.tyre_temp_m
    );
}

#[test]
fn static_versions_decode_as_utf16() {
    let s = parse_static();
    assert_eq!(utf16_to_string(&s.sm_version), "1.7");
    assert_eq!(utf16_to_string(&s.ac_version), "1.16.4");
}

/// `track` sits at offset 134 and `track_spline_length` at 520. Naming the
/// circuit and measuring it are 386 bytes apart, so agreeing rules out an
/// accidental alignment of the front of the struct.
#[test]
fn static_track_name_and_length_agree() {
    let s = parse_static();
    assert_eq!(s.track.to_string(), "imola");
    assert_eq!(s.sector_count, 3);
    assert!(
        (s.track_spline_length - 4864.0).abs() < 10.0,
        "spline length {} m does not match Imola's 4.9 km",
        s.track_spline_length
    );
}

/// Same argument for the car: named at 68, specified from 404 onward, and
/// skinned at 604 — which is 84 bytes from the end of the page.
#[test]
fn static_car_name_and_specs_agree() {
    let s = parse_static();
    assert_eq!(s.car_model.to_string(), "abarth500");
    assert_eq!(s.max_rpm, 6500);
    assert_eq!(s.max_fuel, 35.0);
    assert!(
        (s.tyre_radius[0] - 0.308).abs() < 0.001,
        "tyre radius {} m",
        s.tyre_radius[0]
    );
    assert!(s.max_turbo_boost > 0.0, "the abarth500 is turbocharged");
    assert_eq!(s.car_skin.to_string(), "0_white_scorpion");
}

/// The check no single-page fixture can make. These came from one AC run, so
/// the pages have to agree with each other — and they are three separate
/// structs, so a layout error in any one of them shows up here.
#[test]
fn the_pages_agree_with_each_other() {
    let p = parse_physics();
    let s = parse_static();
    let g = parse();

    assert!(
        p.fuel > 0.0 && p.fuel <= s.max_fuel,
        "{} L on board does not fit the {} L tank",
        p.fuel,
        s.max_fuel
    );
    assert!(
        p.rpms <= s.max_rpm,
        "{} rpm exceeds the {} rpm limit",
        p.rpms,
        s.max_rpm
    );
    assert!(
        g.distance_traveled < s.track_spline_length,
        "{} m travelled on lap 1 of a {} m circuit",
        g.distance_traveled,
        s.track_spline_length
    );
}

/// A page one field longer or shorter must be refused rather than parsed from
/// whatever happens to be adjacent.
#[test]
fn a_wrong_sized_page_does_not_parse() {
    let good = graphics_page();

    let mut short = good.clone();
    short.pop();
    assert!(AcGraphics::try_read_from_bytes(&short).is_err());

    let mut long = good;
    long.extend_from_slice(&[0u8; 4]);
    assert!(AcGraphics::try_read_from_bytes(&long).is_err());
}

/// Formatting one of AC's fixed-width UTF-16 fields must not consume it.
///
/// It reads through a raw array and stops at the first NUL, and the bug this
/// guards against is a `Display` that advances something: the second read
/// would come back short, and the car would lose its name on the second frame.
///
/// Lived in the neutral core tests until the second game arrived, which is
/// where the rule came from: a test that names `AcStatic` is a test about
/// Assetto Corsa.
#[test]
fn formatting_a_fixed_width_name_has_no_side_effects() {
    use ac_core::games::assetto_corsa::structs::StringU16_33;

    let name = StringU16_33::from("ks_ferrari_488_gt3");
    assert_eq!(format!("{name}"), "ks_ferrari_488_gt3");
    assert_eq!(
        format!("{name}"),
        "ks_ferrari_488_gt3",
        "reading it once must not change it"
    );
}
