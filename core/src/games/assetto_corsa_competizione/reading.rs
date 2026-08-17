//! Competizione's three pages, turned into a [`Reading`].
//!
//! Everything ACC-specific about the *meaning* of the numbers lives here, and
//! two of those meanings are not Assetto Corsa's even though the pages look
//! alike:
//!
//! * **the session table is different.** ACC numbers practice 0 where this
//!   project's Assetto Corsa table numbers it 1, so a table copied across
//!   would report every practice session as booking and every race as a
//!   hotlap. The capture settles it: session 0, with no lap count and no
//!   clock, is the practice session that was driven.
//! * **traction control and ABS are split across two pages.** The physics page
//!   says whether the system is cutting in right now; the graphics page says
//!   which level the driver has dialled in. Assetto Corsa keeps both on the
//!   physics page, so a conversion that only reads physics reports a GT3 with
//!   TC 3 and ABS 4 as having neither.
//!
//! What ACC does not publish is left at its default and declared absent in
//! [`CAPABILITIES`](super::CAPABILITIES) — tyre wear, the tread temperatures,
//! camber and the track length. None of them is zero; all of them are unknown.

use super::structs::{AccGraphics, AccPhysics, AccStatic};
use crate::games::reading::{Car, Fixed, Name, Reading, Session, SessionKind, Status};

/// ACC's `AC_STATUS`, which is Assetto Corsa's unchanged.
fn status_of(raw: i32) -> Status {
    match raw {
        1 => Status::Replay,
        2 => Status::Live,
        3 => Status::Paused,
        // 0 is `AC_OFF`, and so is anything unrecognised.
        _ => Status::Off,
    }
}

/// ACC's `AC_SESSION_TYPE`.
///
/// **Not the table this project uses for Assetto Corsa.** Only 0 is confirmed
/// by the capture — a practice session with no lap count and `sessionTimeLeft`
/// of −1 — and the rest come from ACC's published enum, which puts qualifying
/// at 1 and the race at 2. The two ACC added, hot stint and superpole, have no
/// counterpart in Assetto Corsa at all.
fn session_kind_of(raw: i32) -> SessionKind {
    match raw {
        0 => SessionKind::Practice,
        1 => SessionKind::Qualifying,
        2 => SessionKind::Race,
        3 => SessionKind::Hotlap,
        4 => SessionKind::TimeAttack,
        5 => SessionKind::Drift,
        6 => SessionKind::Drag,
        7 => SessionKind::HotStint,
        8 => SessionKind::Superpole,
        _ => SessionKind::Unknown,
    }
}

/// The car, from both pages it takes to describe one.
fn car_of(p: &AccPhysics, g: &AccGraphics) -> Car {
    Car {
        speed_kmh: p.speed_kmh,
        rpm: p.rpm,
        // ACC counts reverse as 0 and neutral as 1, the same as Assetto Corsa
        // and unlike one published binding, which documents 0 as neutral. The
        // recording holds eight distinct values from 0 up on a six-speed car.
        gear: p.gear - 1,
        throttle: p.gas,
        brake: p.brake,
        clutch: p.clutch,
        steer_angle: p.steer_angle,
        fuel_litres: p.fuel,

        acc_g: p.acc_g,

        wheel_slip: p.wheel_slip,
        // Not published: `wheel_load` is zero for the whole session, so the
        // load ratio on every screen is zero rather than wrong.
        wheel_load: p.wheel_load,
        tyre_pressure_psi: p.wheel_pressure,
        // Not published either, and this one is dangerous: the default is
        // zero and zero means "worn out" to every rule that reads it. That is
        // what `tyre_wear: false` in the capabilities is for.
        tyre_wear: p.tyre_wear,
        tyre_core_temp_c: p.tyre_core_temp,
        // The tread triplet is empty on ACC, which withholds the camber advice
        // and the tread-temperature band.
        tyre_temp_inner_c: p.tyre_temp_i,
        tyre_temp_middle_c: p.tyre_temp_m,
        tyre_temp_outer_c: p.tyre_temp_o,
        brake_temp_c: p.brake_temp,
        // What this game measures in place of tyre wear, and the reason the
        // brake-wear advice exists at all.
        brake_pad_mm: p.pad_life,
        brake_disc_mm: p.disc_life,
        camber_rad: p.camber_rad,
        suspension_travel: p.suspension_travel,
        ride_height_m: p.ride_height,

        brake_bias: p.brake_bias,
        air_temp_c: p.air_temp,
        road_temp_c: p.road_temp,

        // The level is on the graphics page and the intervention on the
        // physics page — see the note at the top of this file.
        tc: p.tc,
        tc_level: g.tc,
        tc_in_action: p.tc_in_action as f32,
        abs: p.abs,
        abs_level: g.abs,
        abs_in_action: p.abs_in_action as f32,

        // ACC measures the delta to its own reference lap on the graphics
        // page, in milliseconds, where AC has a float on the physics page.
        reference_delta_s: g.i_delta_lap_time as f32 / 1000.0,
        force_feedback: p.final_ff,
        pit_limiter: p.pit_limiter_on != 0,
    }
}

impl From<&AccGraphics> for Session {
    fn from(g: &AccGraphics) -> Self {
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
            // The player's own car out of the sixty ACC publishes. A car id
            // outside the array is not a position of zero, it is no position
            // at all, so it stays at the default rather than reading a
            // neighbour's.
            car_position_m: g.car_coordinates.of(g.player_car_id).unwrap_or_default(),

            // Not published. ACC says how the track is instead, as one of a
            // handful of named states, and turning that into a fraction would
            // be inventing a measurement.
            surface_grip: g.surface_grip,
            wind_speed_kmh: g.wind_speed,
            wind_direction_deg: g.wind_direction,

            fuel_per_lap: g.fuel_x_lap,
            compound: Name::new(&g.tyre_compound.to_string()),

            in_pit_lane: g.is_in_pit_lane != 0,
            tc_cut: g.tc_cut,
            engine_map: g.engine_map,
            // The gap Assetto Corsa leaves: this game says whether the lap
            // being driven still counts, so a lap over the limits stops being
            // analysed as a clean one.
            lap_is_valid: g.is_valid_lap != 0,
        }
    }
}

impl From<&AccStatic> for Fixed {
    fn from(s: &AccStatic) -> Self {
        Self {
            car_model: s.car_model.to_string(),
            track: s.track.to_string(),
            // ACC writes the literal placeholder "track config" here and never
            // fills it, so it is dropped rather than shown to a driver as the
            // name of the layout they are on.
            track_config: String::new(),
            // ACC leaves the nickname empty and fills the two real names.
            driver_name: format!(
                "{} {}",
                s.player_name.to_string().trim(),
                s.player_surname.to_string().trim()
            )
            .trim()
            .to_string(),
            sector_count: s.sector_count,
            max_rpm: s.max_rpm,
            max_fuel_litres: s.max_fuel,
            // **Not published.** Zero is how `LapData` already spells "no
            // track length", so everything that would report metres says so
            // rather than inventing one.
            track_length_m: s.track_spline_length,
        }
    }
}

/// The three pages as one reading.
///
/// The capabilities are left at their default — nothing measured — and filled
/// in by [`Source::poll`](crate::games::Source::poll), which is the only place
/// that speaks for the game as a whole.
pub fn reading_of(physics: &AccPhysics, graphics: &AccGraphics, stat: &AccStatic) -> Reading {
    Reading {
        car: car_of(physics, graphics),
        session: graphics.into(),
        fixed: stat.into(),
        capabilities: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::reading::{COORD_X, COORD_Z, FL};

    /// The translation that is invisible in a test using the same literal on
    /// both sides, and which cost three screens a gear during the Assetto
    /// Corsa refactor.
    #[test]
    fn reverse_and_neutral_come_out_the_way_a_driver_means_them() {
        let gear = |raw| {
            car_of(
                &AccPhysics {
                    gear: raw,
                    ..Default::default()
                },
                &AccGraphics::default(),
            )
            .gear
        };
        assert_eq!(gear(0), -1, "reverse");
        assert_eq!(gear(1), 0, "neutral");
        assert_eq!(gear(2), 1, "first");
        // The recording's last sample: fourth gear at 166 km/h.
        assert_eq!(gear(5), 4);
    }

    /// ACC's table is not Assetto Corsa's, and the capture is what says so:
    /// the session that was driven published 0 and was practice.
    #[test]
    fn the_session_table_is_competiziones_own() {
        let kind = |raw| {
            Session::from(&AccGraphics {
                session: raw,
                ..Default::default()
            })
            .kind
        };
        assert_eq!(kind(0), SessionKind::Practice, "the recorded session");
        assert_eq!(kind(1), SessionKind::Qualifying);
        assert_eq!(kind(2), SessionKind::Race);
        assert_eq!(kind(7), SessionKind::HotStint);
        assert_eq!(kind(8), SessionKind::Superpole);
        assert_eq!(kind(-1), SessionKind::Unknown);

        // Practice and qualifying have no finish to fuel for; the race and the
        // two ACC formats that run to a flag do.
        assert!(kind(0).has_no_finish());
        assert!(kind(1).has_no_finish());
        assert!(!kind(2).has_no_finish());
        assert!(!kind(7).has_no_finish());
    }

    /// Both pages describe one car's electronics, and reading only the physics
    /// page reports a GT3 with the aids switched off.
    #[test]
    fn the_aids_come_from_the_page_that_knows_the_level() {
        let car = car_of(
            &AccPhysics {
                tc: 1.0,
                abs: 1.0,
                ..Default::default()
            },
            &AccGraphics {
                tc: 3,
                abs: 4,
                ..Default::default()
            },
        );
        assert_eq!(car.tc_level, 3, "the level the driver dialled in");
        assert_eq!(car.abs_level, 4);
        assert_eq!(car.tc, 1.0, "and whether it is cutting in this instant");
    }

    /// Sixty cars are published and one of them is the driver's.
    #[test]
    fn the_players_own_car_is_the_one_that_is_reported() {
        let mut graphics = AccGraphics {
            active_cars: 3,
            player_car_id: 2,
            ..Default::default()
        };
        graphics.car_coordinates[0] = [1.0, 2.0, 3.0];
        graphics.car_coordinates[2] = [-1195.9, 24.1, -2077.8];

        let session = Session::from(&graphics);
        assert!((session.car_position_m[COORD_X] - -1195.9).abs() < 0.1);
        assert!((session.car_position_m[COORD_Z] - -2077.8).abs() < 0.1);
    }

    /// A car id outside the array is no position at all rather than a
    /// neighbour's, which is the mistake that produced a track map of one
    /// stationary dot.
    #[test]
    fn an_impossible_car_id_reports_no_position() {
        for id in [-1, 60, i32::MAX] {
            let session = Session::from(&AccGraphics {
                player_car_id: id,
                ..Default::default()
            });
            assert_eq!(session.car_position_m, [0.0; 3], "car id {id}");
        }
    }

    /// Values are carried across without being reinterpreted, and the ones ACC
    /// does not publish stay at their default rather than being filled in.
    #[test]
    fn the_numbers_arrive_where_their_names_say() {
        let car = car_of(
            &AccPhysics {
                speed_kmh: 252.73,
                wheel_pressure: [27.77, 27.32, 26.76, 26.56],
                brake_temp: [519.7, 509.2, 257.2, 256.1],
                brake_bias: 0.76,
                ..Default::default()
            },
            &AccGraphics::default(),
        );
        assert_eq!(car.speed_kmh, 252.73);
        assert_eq!(car.tyre_pressure_psi[FL], 27.77);
        assert_eq!(car.brake_temp_c[FL], 519.7);
        assert_eq!(car.brake_bias, 0.76);
        assert_eq!(car.tyre_wear, [0.0; 4], "ACC does not publish tyre wear");
        assert_eq!(car.camber_rad, [0.0; 4], "nor camber");
    }

    /// ACC leaves the nickname empty and fills the two real name fields, and
    /// writes a placeholder where the track layout would be.
    #[test]
    fn the_driver_is_named_and_the_layout_placeholder_is_not_shown() {
        let fixed = Fixed::from(&AccStatic {
            car_model: "lamborghini_huracan_gt3_evo".into(),
            track: "Spa".into(),
            player_name: "Andrea".into(),
            player_surname: "Caldarelli".into(),
            track_configuration: "track config".into(),
            ..Default::default()
        });
        assert_eq!(fixed.driver_name, "Andrea Caldarelli");
        assert_eq!(fixed.track, "Spa");
        assert_eq!(fixed.track_config, "");
        assert_eq!(
            fixed.track_length_m, 0.0,
            "ACC publishes no track length, and zero is how that is said"
        );
    }
}
