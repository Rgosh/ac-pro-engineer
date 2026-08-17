//! Assetto Corsa's three pages, turned into a [`Reading`].
//!
//! Everything AC-specific about the *meaning* of the numbers lives here: that
//! reverse is gear 0, that the session type is an integer with a table behind
//! it, that the tyre compound arrives as UTF-16 in a fixed array. Above this
//! file none of that is true of anything.

use super::structs::{AcGraphics, AcPhysics, AcStatic};
use crate::games::reading::{Car, Fixed, Name, Reading, Session, SessionKind, Status};

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
            tyre_temp_inner_c: p.tyre_temp_i,
            tyre_temp_middle_c: p.tyre_temp_m,
            tyre_temp_outer_c: p.tyre_temp_o,
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
    use crate::games::reading::FL;

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
            ..Default::default()
        });
        assert_eq!(car.speed_kmh, 214.0);
        assert_eq!(car.throttle, 0.75);
        assert_eq!(car.tyre_pressure_psi[FL], 26.8);
        assert_eq!(car.avg_tyre_temp_c(FL), 80.0);
        assert_eq!(car.force_feedback, 0.62);
        assert_eq!(car.reference_delta_s, -0.34);
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
