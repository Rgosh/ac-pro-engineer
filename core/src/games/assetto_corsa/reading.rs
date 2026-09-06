//! Assetto Corsa's three pages, turned into a [`Reading`].
//!
//! Everything AC-specific about the *meaning* of the numbers lives here: that
//! reverse is gear 0, that the session type is an integer with a table behind
//! it, that the tyre compound arrives as UTF-16 in a fixed array. Above this
//! file none of that is true of anything.

use super::structs::{AcGraphics, AcPhysics, AcStatic};
use crate::games::reading::{Car, FL, Fixed, Name, RL, Reading, Session, SessionKind, Status};

/// AC's tread temperatures, put on the car's own inner/outer scale.
///
/// **The two sides of the car are mirrored, and only one of them arrives the
/// way the field names read.** `tyreTempI` and `tyreTempO` are filled across
/// the contact patch in the *wheel's* frame, so the array that is the inner
/// edge on the right of the car is the outer edge on the left. Passed through
/// as named, every left-hand tyre had its two shoulders the wrong way round.
///
/// This is the same property of Assetto Corsa that
/// [`Engineer::camber_degrees`] already compensates for — it negates the
/// right-hand corners because the same setting reads -0.023 rad on the left
/// and +0.021 on the right. A quantity that has a direction across the car is
/// mirrored between the sides; the camber reader knew and this one did not.
///
/// What it cost, and it is not a display fault: every camber verdict is inner
/// minus outer, so on the left of the car the advice was the exact opposite of
/// the right thing to do. A driver reported it as "left side tyres have their
/// zones swapped, so inner is outer" and it survived the v0.4.5 fix to the car
/// drawing, which was a second and unrelated mirror.
///
/// The evidence is in the report: on one frame the game's own tyre app read
/// FL 96/98/102 outer-to-inner and FR 71/69/69 inner-to-outer — inner hottest
/// on both, which is what a cambered tyre does — while this read FL as inner
/// 96 and FR as inner 71. One side agreed and one was reversed.
///
/// Competizione needs none of this: it publishes zeros for all three, which
/// `Capabilities::tyre_edge_temps` already says.
///
/// [`Engineer::camber_degrees`]: crate::engineer::Engineer::camber_degrees
fn tread_across_the_car(inner: [f32; 4], outer: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    let (mut inner, mut outer) = (inner, outer);
    for wheel in [FL, RL] {
        std::mem::swap(&mut inner[wheel], &mut outer[wheel]);
    }
    (inner, outer)
}

/// AC's `AC_STATUS`.
fn status_of(raw: i32) -> Status {
    match raw {
        1 => Status::Replay,
        2 => Status::Live,
        3 => Status::Paused,
        // 0 is `AC_OFF`, and so is anything unrecognised: the menus are the
        // safe reading for a value this build has not seen.
        _ => Status::Off,
    }
}

/// AC's `AC_SESSION_TYPE`.
///
/// The table is the one the terminal has always used. `-1` is AC's own
/// "unknown", which it publishes in the menus, and it maps to
/// [`SessionKind::Unknown`] — which the fuel calculator treats as a session
/// with no finish, exactly as the integer comparison it replaces did.
fn session_kind_of(raw: i32) -> SessionKind {
    match raw {
        0 => SessionKind::Booking,
        1 => SessionKind::Practice,
        2 => SessionKind::Qualifying,
        3 => SessionKind::Race,
        4 => SessionKind::Hotlap,
        5 => SessionKind::TimeAttack,
        6 => SessionKind::Drift,
        7 => SessionKind::Drag,
        _ => SessionKind::Unknown,
    }
}

impl From<&AcPhysics> for Car {
    fn from(p: &AcPhysics) -> Self {
        let (tread_inner, tread_outer) = tread_across_the_car(p.tyre_temp_i, p.tyre_temp_o);
        Self {
            speed_kmh: p.speed_kmh,
            rpm: p.rpms,
            // AC counts reverse as 0 and neutral as 1.
            gear: p.gear - 1,
            throttle: p.gas,
            brake: p.brake,
            clutch: p.clutch,
            steer_angle: p.steer_angle,
            fuel_litres: p.fuel,

            acc_g: p.acc_g,

            wheel_slip: p.wheel_slip,
            wheel_load: p.wheel_load,
            tyre_pressure_psi: p.wheels_pressure,
            tyre_wear: p.tyre_wear,
            tyre_core_temp_c: p.tyre_core_temp,
            tyre_temp_inner_c: tread_inner,
            tyre_temp_middle_c: p.tyre_temp_m,
            tyre_temp_outer_c: tread_outer,
            brake_temp_c: p.brake_temp,
            // Not published by this game; `brake_wear` says so.
            brake_pad_mm: [0.0; 4],
            brake_disc_mm: [0.0; 4],
            camber_rad: p.camber_rad,
            suspension_travel: p.suspension_travel,
            ride_height_m: p.ride_height,

            brake_bias: p.brake_bias,
            air_temp_c: p.air_temp,
            road_temp_c: p.road_temp,

            tc: p.tc,
            tc_level: p.tc_level,
            tc_in_action: p.tc_in_action,
            abs: p.abs,
            abs_level: p.abs_level,
            abs_in_action: p.abs_in_action,

            damage: p.car_damage,
            tyres_off_track: p.number_of_tyres_out,
            reference_delta_s: p.performance_meter,
            force_feedback: p.final_ff,
            pit_limiter: p.pit_limiter_on != 0,
        }
    }
}

impl From<&AcGraphics> for Session {
    fn from(g: &AcGraphics) -> Self {
        Self {
            status: status_of(g.status),
            kind: session_kind_of(g.session),

            completed_laps: g.completed_laps,
            total_laps: g.number_of_laps,
            position: g.position,

            current_lap_ms: g.i_current_time,
            last_lap_ms: g.i_last_time,
            best_lap_ms: g.i_best_time,
            session_time_left_ms: g.session_time_left,

            current_sector: g.current_sector_index,
            last_sector_ms: g.last_sector_time,

            track_position: g.normalized_car_position,
            distance_travelled_m: g.distance_traveled,
            car_position_m: g.car_coordinates,

            surface_grip: g.surface_grip,
            wind_speed_kmh: g.wind_speed,
            wind_direction_deg: g.wind_direction,

            fuel_per_lap: g.fuel_x_lap,
            compound: Name::new(&g.tyre_compound.to_string()),

            in_pit_lane: g.is_in_pit_lane != 0,
            tc_cut: g.tccut,
            engine_map: g.engine_map,
            // AC has no field for it, and the default is what that means:
            // valid because nothing said otherwise. `lap_validity: false` is
            // what stops that reading as a verdict.
            lap_is_valid: true,
        }
    }
}

impl From<&AcStatic> for Fixed {
    fn from(s: &AcStatic) -> Self {
        Self {
            car_model: s.car_model.to_string(),
            track: s.track.to_string(),
            track_config: s.track_configuration.to_string(),
            driver_name: s.player_nick.to_string(),
            sector_count: s.sector_count,
            max_rpm: s.max_rpm,
            max_fuel_litres: s.max_fuel,
            track_length_m: s.track_spline_length,
        }
    }
}

/// The three pages as one reading.
///
/// The capabilities are left at their default — nothing measured — and filled
/// in by [`Source::poll`](crate::games::Source::poll), which is the only place
/// that speaks for the game as a whole.
pub fn reading_of(physics: &AcPhysics, graphics: &AcGraphics, stat: &AcStatic) -> Reading {
    Reading {
        car: physics.into(),
        session: graphics.into(),
        fixed: stat.into(),
        capabilities: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::reading::{FL, FR, RR};

    /// The left of the car has its shoulders the right way round.
    ///
    /// **The frame is a real one**, from the report that found this: the
    /// game's own tyre app, on the same lap, printed the left tyres
    /// outer-to-inner and the right ones inner-to-outer, and read
    ///
    /// ```text
    ///   FL  O 96  M 98  I 102        FR  I 71  M 69  O 69
    ///   RL  O 82  M 82  I 83         RR  I 70  M 69  O 68
    /// ```
    ///
    /// so the arrays AC had filled were `tyre_temp_i = [96, 71, 82, 70]` and
    /// `tyre_temp_o = [102, 69, 83, 68]`. Passed through as named, this made
    /// the front-left's inner edge 96 °C and its outer 102 — the opposite of
    /// what the game was showing beside it, and the opposite of the
    /// front-right on the same car on the same lap.
    ///
    /// The assertion that matters is the last one: a tyre with negative camber
    /// runs its **inner** edge hotter, and it does so on both sides of the
    /// car. One side saying otherwise is the signature of this bug and the
    /// only thing here that would survive the numbers being replaced.
    #[test]
    fn the_left_of_the_car_has_its_shoulders_the_right_way_round() {
        let car = Car::from(&AcPhysics {
            tyre_temp_i: [96.0, 71.0, 82.0, 70.0],
            tyre_temp_m: [98.0, 69.0, 82.0, 69.0],
            tyre_temp_o: [102.0, 69.0, 83.0, 68.0],
            ..Default::default()
        });

        assert_eq!(car.tyre_temp_inner_c, [102.0, 71.0, 83.0, 70.0]);
        assert_eq!(car.tyre_temp_outer_c, [96.0, 69.0, 82.0, 68.0]);
        // The middle is the middle whichever way round the shoulders are.
        assert_eq!(car.tyre_temp_middle_c, [98.0, 69.0, 82.0, 69.0]);

        for corner in [FL, FR, crate::games::reading::RL, RR] {
            assert!(
                car.tyre_temp_inner_c[corner] > car.tyre_temp_outer_c[corner],
                "corner {corner} came out with its outer edge hotter, which is \
                 what a mirrored side looks like: inner {}, outer {}",
                car.tyre_temp_inner_c[corner],
                car.tyre_temp_outer_c[corner]
            );
        }
    }

    /// The swap is the two left-hand corners and nothing else.
    #[test]
    fn only_the_left_hand_corners_are_turned_round() {
        let (inner, outer) = tread_across_the_car([1.0, 2.0, 3.0, 4.0], [10.0, 20.0, 30.0, 40.0]);
        assert_eq!(inner, [10.0, 2.0, 30.0, 4.0]);
        assert_eq!(outer, [1.0, 20.0, 3.0, 40.0]);
    }

    /// The one translation a reader is most likely to forget, and the one that
    /// puts a wrong number on the screen rather than crashing: AC's neutral is
    /// 1 and its reverse is 0.
    #[test]
    fn reverse_and_neutral_come_out_the_way_a_driver_means_them() {
        let gear = |raw| {
            Car::from(&AcPhysics {
                gear: raw,
                ..Default::default()
            })
            .gear
        };
        assert_eq!(gear(0), -1, "reverse");
        assert_eq!(gear(1), 0, "neutral");
        assert_eq!(gear(2), 1, "first");
        assert_eq!(gear(8), 7);
    }

    /// A flag arrives as an integer and leaves as a yes or no.
    #[test]
    fn the_pit_limiter_stops_being_an_integer() {
        let limiter = |raw| {
            Car::from(&AcPhysics {
                pit_limiter_on: raw,
                ..Default::default()
            })
            .pit_limiter
        };
        assert!(!limiter(0));
        assert!(limiter(1));
    }

    /// Values are carried across without being reinterpreted — the point of
    /// the conversion is the naming, not arithmetic.
    #[test]
    fn the_numbers_arrive_where_their_names_say() {
        let car = Car::from(&AcPhysics {
            speed_kmh: 214.0,
            gas: 0.75,
            wheels_pressure: [26.8, 27.0, 26.1, 26.3],
            tyre_temp_i: [90.0; 4],
            tyre_temp_m: [80.0; 4],
            tyre_temp_o: [70.0; 4],
            final_ff: 0.62,
            performance_meter: -0.34,
            car_damage: [12.5, 0.0, 3.25, 0.0, 1.0],
            number_of_tyres_out: 2,
            ..Default::default()
        });
        assert_eq!(car.speed_kmh, 214.0);
        assert_eq!(car.throttle, 0.75);
        assert_eq!(car.tyre_pressure_psi[FL], 26.8);
        assert_eq!(car.avg_tyre_temp_c(FL), 80.0);
        assert_eq!(car.force_feedback, 0.62);
        assert_eq!(car.reference_delta_s, -0.34);
        // Five zones in the order the game publishes them, carried through
        // rather than stopping at the reader — which is where they stopped
        // until now, on both games.
        assert_eq!(car.damage, [12.5, 0.0, 3.25, 0.0, 1.0]);
        assert_eq!(car.tyres_off_track, 2);
    }

    /// The session table is the one the terminal printed before this file
    /// existed, and "unknown" — which AC publishes in the menus as −1 — must
    /// stay a session with no finish, or the fuel calculator changes what it
    /// says in the garage.
    #[test]
    fn the_session_table_is_unchanged_and_unknown_has_no_finish() {
        let kind = |raw| {
            Session::from(&AcGraphics {
                session: raw,
                ..Default::default()
            })
            .kind
        };
        assert_eq!(kind(0).label(), "Booking");
        assert_eq!(kind(3), SessionKind::Race);
        assert_eq!(kind(5).label(), "Time Attack");
        assert_eq!(kind(-1), SessionKind::Unknown);

        for raw in -1..3 {
            assert!(kind(raw).has_no_finish(), "session {raw} has no finish");
        }
        for raw in 3..8 {
            assert!(!kind(raw).has_no_finish(), "session {raw} finishes");
        }
    }

    /// `AC_OFF` is the reading for the menus and for anything unrecognised.
    #[test]
    fn the_status_says_whether_there_is_a_car() {
        let status = |raw| {
            Session::from(&AcGraphics {
                status: raw,
                ..Default::default()
            })
            .status
        };
        assert!(!status(0).is_on_track());
        assert_eq!(status(2), Status::Live);
        assert!(status(1).is_on_track(), "a replay is still a car on track");
        assert!(!status(99).is_on_track());
    }

    /// The compound arrives as UTF-16 in a fixed array and leaves as something
    /// that can be compared and printed.
    #[test]
    fn the_compound_becomes_a_name() {
        let session = Session::from(&AcGraphics {
            tyre_compound: "Semislick".into(),
            ..Default::default()
        });
        assert_eq!(session.compound.as_str(), "Semislick");
    }
}
