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
//! # What this does not cover
//!
//! Two gaps, both wanting a live session to close rather than more code:
//!
//! - **Offsets past 296.** The capture is zero from 300 to the end of the
//!   page, so the last fifteen fields are asserted by nothing. Reading these
//!   tests as covering the whole page would be a mistake — see the note on the
//!   tail fields of `AcGraphics`.
//! - **`AcPhysics` and `AcStatic`.** Both were checked by hand against the
//!   same session, but neither has a fixture here, so the round-trip blind
//!   spot described above still applies to them in full. Capturing
//!   `acpmf_physics` (596 bytes) and `acpmf_static` (688) alongside a
//!   lap-2 graphics page would close both gaps in one sitting.

use ac_core::ac_structs::{AcGraphics, COORD_X, COORD_Y, COORD_Z};
use zerocopy::TryFromBytes;

/// First 360 bytes of `/dev/shm/acpmf_graphics`, mid-lap at Imola.
const GRAPHICS_PAGE_HEX: &str = concat!(
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

fn decode_hex(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "hex must be byte-aligned");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn graphics_page() -> Vec<u8> {
    let bytes = decode_hex(GRAPHICS_PAGE_HEX);
    assert_eq!(
        bytes.len(),
        size_of::<AcGraphics>(),
        "captured page is {} bytes but AcGraphics is {} — the struct no longer \
         matches the size AC actually publishes",
        bytes.len(),
        size_of::<AcGraphics>()
    );
    bytes
}

/// AC pads its fixed-width `wchar_t` fields with NULs.
fn utf16_to_string(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

fn parse() -> AcGraphics {
    // The same call `SharedMemory::get` makes on the real mapping.
    AcGraphics::try_read_from_bytes(&graphics_page()).expect("real AC page must parse")
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
