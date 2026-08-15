//! One tick of a car on a track, in the shape the rest of the program works in.
//!
//! This is the boundary the folder-per-game layout was drawn for. Before it,
//! [`Source`](super::Source) said *whether* a game was running and the telemetry
//! came out beside it, through `physics()`, `graphics()` and `stat()` returning
//! Assetto Corsa's own three shared-memory pages. The engineer, the analyser and
//! five screens read AC's memory layout directly, which made the folder real as
//! a folder and nothing more: a second game meant either a second engineer or a
//! set of `if`s through the middle of the first one.
//!
//! So the game hands over a [`Reading`] and nothing above here knows what a
//! `SPageFilePhysics` is. Three rules the conversion has to keep, because each
//! is a bug this project has already shipped or nearly shipped:
//!
//! * **Units live in the name.** `air_temp_c`, `tyre_pressure_psi`,
//!   `session_time_left_ms`. A game that publishes bar converts on the way in.
//! * **Conventions are stated, not inherited.** [`Car::gear`] is −1 for reverse
//!   because that is what a human means, not because a simulator numbered it
//!   that way; AC's 0-is-reverse is translated at the boundary, once.
//! * **"Not measured" is not zero.** A field a game does not publish is left at
//!   its default *and declared absent* in [`Capabilities`](super::Capabilities).
//!   Nothing here can express the difference on its own — that is what the
//!   capability flags are for, and why they have to be consulted.

use std::fmt::{Debug, Display, Formatter};

/// Index of the front-left wheel in every `[f32; 4]` below.
pub const FL: usize = 0;
/// Index of the front-right wheel.
pub const FR: usize = 1;
/// Index of the rear-left wheel.
pub const RL: usize = 2;
/// Index of the rear-right wheel.
pub const RR: usize = 3;

/// Index of the world X axis in [`Session::car_position_m`].
pub const COORD_X: usize = 0;
/// Index of the world Y axis — altitude — in [`Session::car_position_m`].
///
/// A track map wants the ground plane, so it plots [`COORD_X`] against
/// [`COORD_Z`] and leaves this one alone. Reading altitude as a ground
/// coordinate is precisely what an early misread of ACC's layout did.
pub const COORD_Y: usize = 1;
/// Index of the world Z axis in [`Session::car_position_m`].
pub const COORD_Z: usize = 2;

/// A short name carried by value.
///
/// The obvious type here is `String`, and it is the wrong one: a reading is
/// pushed into a buffer thousands of entries long, once per tick, so a word
/// like "semislick" would be a heap allocation per frame per lap. Long names
/// are truncated rather than refused — this is a label on a screen, and a
/// compound whose name does not fit is still better than none.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Name {
    bytes: [u8; Self::CAPACITY],
    len: u8,
}

impl Name {
    /// Bytes, not characters. Enough for every tyre compound AC ships.
    pub const CAPACITY: usize = 32;

    pub fn new(text: &str) -> Self {
        // Truncated on a character boundary: cutting a multi-byte character in
        // half would make `as_str` unable to return the prefix at all.
        let mut end = text.len().min(Self::CAPACITY);
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }

        let mut bytes = [0u8; Self::CAPACITY];
        bytes[..end].copy_from_slice(&text.as_bytes()[..end]);
        Self {
            bytes,
            len: end as u8,
        }
    }

    pub fn as_str(&self) -> &str {
        // Only ever filled from a `&str` cut on a boundary, so this cannot fail.
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for Name {
    fn default() -> Self {
        Self {
            bytes: [0u8; Self::CAPACITY],
            len: 0,
        }
    }
}

impl Display for Name {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Debug for Name {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl From<&str> for Name {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

/// Whether the game is actually driving a car right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    /// In the menus, or nothing published yet.
    #[default]
    Off,
    Replay,
    Live,
    Paused,
}

impl Status {
    /// Anything other than sitting in the menus.
    ///
    /// The check this replaces was `gfx.status != 0`, which is the same
    /// question asked in the shape of whichever simulator answered it.
    pub fn is_on_track(self) -> bool {
        self != Status::Off
    }
}

/// What kind of session is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionKind {
    #[default]
    Unknown,
    Booking,
    Practice,
    Qualifying,
    Race,
    Hotlap,
    TimeAttack,
    Drift,
    Drag,
}

impl SessionKind {
    /// The name to print. English; the interface translates it.
    pub fn label(self) -> &'static str {
        match self {
            SessionKind::Unknown => "Unknown",
            SessionKind::Booking => "Booking",
            SessionKind::Practice => "Practice",
            SessionKind::Qualifying => "Qualifying",
            SessionKind::Race => "Race",
            SessionKind::Hotlap => "Hotlap",
            SessionKind::TimeAttack => "Time Attack",
            SessionKind::Drift => "Drift",
            SessionKind::Drag => "Drag",
        }
    }

    /// Practice, qualifying, booking — a session with no finish to fuel for.
    ///
    /// The fuel calculator invents a five-lap run for these, because a
    /// practice session has no lap count and its clock says nothing about how
    /// long the driver intends to stay out.
    pub fn has_no_finish(self) -> bool {
        matches!(
            self,
            SessionKind::Unknown
                | SessionKind::Booking
                | SessionKind::Practice
                | SessionKind::Qualifying
        )
    }
}

/// What the car is doing this instant.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Car {
    pub speed_kmh: f32,
    pub rpm: i32,
    /// −1 reverse, 0 neutral, 1 first, and up.
    ///
    /// AC numbers reverse 0 and neutral 1; that is translated on the way in, so
    /// nothing above here has to subtract one and every place that forgot to is
    /// a place that printed the wrong gear.
    pub gear: i32,
    /// 0..1.
    pub throttle: f32,
    /// 0..1.
    pub brake: f32,
    /// 0..1.
    pub clutch: f32,
    /// Radians, signed; negative is left on AC's convention.
    pub steer_angle: f32,
    pub fuel_litres: f32,

    /// `[lateral, vertical, longitudinal]`, in g.
    pub acc_g: [f32; 3],

    pub wheel_slip: [f32; 4],
    pub wheel_load: [f32; 4],
    pub tyre_pressure_psi: [f32; 4],
    /// Percent of tread left: **100 is a new tyre and it counts down.**
    ///
    /// A game that publishes wear the other way round inverts it here rather
    /// than teaching the engineer a second convention — four tyres reported as
    /// worn out when they were new is a verdict this project has shipped.
    pub tyre_wear: [f32; 4],
    pub tyre_core_temp_c: [f32; 4],
    /// Surface temperature across the tread. A game that publishes only the
    /// middle leaves the other two at zero and says so through
    /// [`Capabilities::tyre_edge_temps`](super::Capabilities::tyre_edge_temps);
    /// the camber advice is built entirely on inner minus outer and must not
    /// run on a guess.
    pub tyre_temp_inner_c: [f32; 4],
    pub tyre_temp_middle_c: [f32; 4],
    pub tyre_temp_outer_c: [f32; 4],
    pub brake_temp_c: [f32; 4],
    pub camber_rad: [f32; 4],
    pub suspension_travel: [f32; 4],
    /// Front and rear, in metres.
    pub ride_height_m: [f32; 2],

    pub brake_bias: f32,
    pub air_temp_c: f32,
    pub road_temp_c: f32,

    /// Traction control and ABS: the level the car is set to, and how hard the
    /// system is working this instant.
    pub tc: f32,
    pub tc_level: i32,
    pub tc_in_action: f32,
    pub abs: f32,
    pub abs_level: i32,
    pub abs_in_action: f32,

    /// The game's own delta to its reference lap, in seconds.
    pub reference_delta_s: f32,
    /// Force feedback output, 0..1. Above 1 the wheel is clipping.
    pub force_feedback: f32,
    pub pit_limiter: bool,
}

impl Car {
    /// Mean of the three tread temperatures for one wheel.
    ///
    /// Zero for an index past the fourth wheel rather than a panic: this is
    /// called from draw code, and a wrong number on a screen beats taking the
    /// application down mid-session.
    pub fn avg_tyre_temp_c(&self, wheel: usize) -> f32 {
        if wheel < 4 {
            (self.tyre_temp_inner_c[wheel]
                + self.tyre_temp_middle_c[wheel]
                + self.tyre_temp_outer_c[wheel])
                / 3.0
        } else {
            0.0
        }
    }

    /// This wheel's share of the load on all four, 0..1.
    pub fn tyre_load_ratio(&self, wheel: usize) -> f32 {
        if wheel < 4 {
            let total = self.wheel_load.iter().sum::<f32>();
            if total > 1.0 {
                self.wheel_load[wheel] / total
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

/// Where that car is in the session, and how the session stands.
///
/// Copied into a per-lap buffer once a tick, so everything here is by value —
/// see [`Name`] for why the tyre compound is not a `String`.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Session {
    pub status: Status,
    pub kind: SessionKind,

    pub completed_laps: i32,
    /// Total laps in a lap-limited race, 0 when the session is timed.
    pub total_laps: i32,
    pub position: i32,

    pub current_lap_ms: i32,
    pub last_lap_ms: i32,
    pub best_lap_ms: i32,
    pub session_time_left_ms: f32,

    pub current_sector: i32,
    /// The sector just finished, in milliseconds.
    pub last_sector_ms: i32,

    /// Where the car is around the lap, 0..1.
    pub track_position: f32,
    /// Metres covered since the session started.
    pub distance_travelled_m: f32,
    /// World position, `[x, y, z]` in metres — see [`COORD_X`].
    pub car_position_m: [f32; 3],

    /// 0..1. Below 1 the track is green, dirty or wet.
    pub surface_grip: f32,
    pub wind_speed_kmh: f32,
    pub wind_direction_deg: f32,

    /// The game's own measured burn, litres per lap. Zero until it has a
    /// completed lap to measure, which is why the fuel advice is gated on it.
    pub fuel_per_lap: f32,
    pub compound: Name,

    pub in_pit_lane: bool,
    /// The car's dash settings, where the game publishes them.
    pub tc_cut: i32,
    pub engine_map: i32,
}

/// What does not change while the session runs.
///
/// Read once on connecting and refreshed with every reading, because a game
/// that changes car or track without closing is a game that would otherwise
/// keep reporting the previous one.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Fixed {
    pub car_model: String,
    pub track: String,
    pub track_config: String,
    pub driver_name: String,
    /// How many sectors this track publishes. Two-sector mod tracks exist, and
    /// assuming three put a split in the wrong place on every one of them.
    pub sector_count: i32,
    pub max_rpm: i32,
    pub max_fuel_litres: f32,
    /// Lap distance in metres, so a braking point can be reported in metres
    /// rather than as a fraction of a lap.
    pub track_length_m: f32,
}

/// Everything one tick of a simulator has to say.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Reading {
    pub car: Car,
    pub session: Session,
    pub fixed: Fixed,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A compound name longer than the buffer is cut, not lost, and cut where
    /// the bytes allow — a name ending mid-character would come back empty.
    ///
    /// The `é` is deliberately the thirty-second byte: it is two bytes long, so
    /// a cut at the capacity lands inside it and the truncation has to step
    /// back rather than write half a character.
    #[test]
    fn a_long_name_is_truncated_rather_than_refused() {
        let long = format!("{}é and more", "a".repeat(Name::CAPACITY - 1));
        let name = Name::new(&long);
        assert_eq!(name.as_str().len(), Name::CAPACITY - 1);
        assert!(long.starts_with(name.as_str()));
    }

    #[test]
    fn a_short_name_survives_intact() {
        assert_eq!(Name::new("semislick").as_str(), "semislick");
        assert!(Name::default().is_empty());
    }

    /// The mean is over the three tread temperatures, and a wheel that does not
    /// exist reads zero instead of taking the process down.
    #[test]
    fn the_tyre_average_covers_the_tread_and_stops_at_four_wheels() {
        let car = Car {
            tyre_temp_inner_c: [90.0; 4],
            tyre_temp_middle_c: [80.0; 4],
            tyre_temp_outer_c: [70.0; 4],
            ..Default::default()
        };
        assert_eq!(car.avg_tyre_temp_c(FL), 80.0);
        assert_eq!(car.avg_tyre_temp_c(9), 0.0);
    }

    /// Load share is a fraction of the total, and an unloaded car — every lap
    /// before the physics start, and every replay frame — is zero rather than
    /// a division by nothing.
    #[test]
    fn the_load_ratio_is_a_share_and_a_still_car_has_none() {
        let car = Car {
            wheel_load: [300.0, 300.0, 200.0, 200.0],
            ..Default::default()
        };
        assert!((car.tyre_load_ratio(FL) - 0.3).abs() < 1e-6);
        assert_eq!(Car::default().tyre_load_ratio(FL), 0.0);
    }
}
