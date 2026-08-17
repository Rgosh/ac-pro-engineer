//! Assetto Corsa Competizione's three shared-memory pages.
//!
//! The names are ACC's own — `acpmf_physics`, `acpmf_graphics`, `acpmf_static`
//! — and so are the layouts, which is the dangerous part: **the names match
//! Assetto Corsa's and the layouts do not.** ACC's graphics page is 1588 bytes
//! where AC's is 360, because ACC publishes sixty cars' coordinates where AC
//! publishes the player's. Reading one game's page with the other's parser
//! produces plausible-looking numbers, which is why `shm::Memory` refuses to
//! attach unless the page says which contract it was written to.
//!
//! # Where these offsets come from
//!
//! A recording of a live session, not a header file somebody published:
//! `assetto-corsa-competizione-20260816-2051.txt` in the repository root is
//! 337 seconds and 8376 samples of a Lamborghini Huracán GT3 EVO at Spa, taken
//! with `tools/record-session.sh` under Proton. Every offset asserted below is
//! one that recording *proves* — the field moved, or held a value only that
//! field could hold.
//!
//! **Offsets the recording leaves at zero are not asserted**, because a wrong
//! offset also reads zero. They are declared, because the fields after them
//! would be in the wrong place otherwise, and they are commented as unproven.
//! That is the distinction this whole project turns on: "not measured" and
//! "measured as zero" are different answers.
//!
//! # What ACC does not publish
//!
//! Six of AC's arrays are dead here, and this matters more than it sounds:
//! `wheel_load`, `tyre_wear`, `tyre_dirty_level`, `camber_rad` and the three
//! tread temperatures `tyre_temp_i/m/o` all read zero for the whole session.
//! ACC measures a different set — brake pad and disc life, water temperature,
//! slip ratio and angle, the MFD — and the capability flags in `mod.rs` are
//! what stop the engineer reporting four unworn tyres as four destroyed ones.

use std::fmt::Display;
use std::fmt::Formatter;
use zerocopy::TryFromBytes;

/// Compile-time guarantee that these structs still match the sizes ACC
/// publishes.
///
/// Each is the last field's offset plus its size, and each was confirmed
/// against the recording: the physics page's last written byte is 795 of 800
/// (`abs_vibrations`, which reached ±1.0), the graphics page's is 1576 of 1588
/// (`strategy_tyre_set`), and the static page's is 754 of 820
/// (`wet_tyres_name`, "WH").
const _: () = {
    assert!(
        size_of::<AccPhysics>() == 800,
        "AccPhysics no longer matches ACC's SPageFilePhysics"
    );
    assert!(
        size_of::<AccGraphics>() == 1588,
        "AccGraphics no longer matches ACC's SPageFileGraphic"
    );
    assert!(
        size_of::<AccStatic>() == 820,
        "AccStatic no longer matches ACC's SPageFileStatic"
    );
};

/// The offsets the recording actually proves, pinned one by one.
///
/// A size assertion says nothing about a field inserted and another removed,
/// which is exactly the shape of the mistake this project already made once —
/// AC's graphics struct carried ACC's layout and every field past
/// `car_coordinates` read 964 bytes late. So each of these is an offset where
/// the recording holds a number that could not have come from anywhere else.
const _: () = {
    use std::mem::offset_of;

    // Physics. Tyre pressures at 88 (27.8/27.3/26.8/26.6 psi), core
    // temperatures at 152 (92/88/90/88 °C), brake temperatures at 348
    // (520/509/257/256 °C — a GT3 on carbon), brake bias at 564 (0.76),
    // the rev limit at 588 (8650, which the static page repeats), water at
    // 712 (84 °C), brake pressure at 716 (0.76/0.76/0.24/0.24, the same split
    // as the bias), pad life at 740 (29 mm) and disc life at 756 (32 mm).
    assert!(offset_of!(AccPhysics, speed_kmh) == 28);
    assert!(offset_of!(AccPhysics, wheel_pressure) == 88);
    assert!(offset_of!(AccPhysics, tyre_core_temp) == 152);
    assert!(offset_of!(AccPhysics, air_temp) == 288);
    assert!(offset_of!(AccPhysics, brake_temp) == 348);
    assert!(offset_of!(AccPhysics, tyre_contact_point) == 420);
    assert!(offset_of!(AccPhysics, brake_bias) == 564);
    assert!(offset_of!(AccPhysics, current_max_rpm) == 588);
    assert!(offset_of!(AccPhysics, slip_ratio) == 640);
    assert!(offset_of!(AccPhysics, tyre_temp) == 696);
    assert!(offset_of!(AccPhysics, water_temp) == 712);
    assert!(offset_of!(AccPhysics, brake_pressure) == 716);
    assert!(offset_of!(AccPhysics, pad_life) == 740);
    assert!(offset_of!(AccPhysics, disc_life) == 756);
    assert!(offset_of!(AccPhysics, is_engine_running) == 780);

    // Graphics. This is where ACC diverges from Assetto Corsa and where the
    // 964 bytes come from: `active_cars` at 252 and `car_coordinates[60][3]`
    // at 256, where AC has the player's three coordinates and nothing else.
    assert!(offset_of!(AccGraphics, i_current_time) == 140);
    assert!(offset_of!(AccGraphics, tyre_compound) == 176);
    assert!(offset_of!(AccGraphics, normalized_car_position) == 248);
    assert!(offset_of!(AccGraphics, active_cars) == 252);
    assert!(offset_of!(AccGraphics, car_coordinates) == 256);
    assert!(offset_of!(AccGraphics, is_in_pit_lane) == 1236);
    assert!(offset_of!(AccGraphics, fuel_x_lap) == 1284);
    assert!(offset_of!(AccGraphics, is_valid_lap) == 1408);
    assert!(offset_of!(AccGraphics, fuel_estimated_laps) == 1412);
    assert!(offset_of!(AccGraphics, mfd_tyre_pressure_lf) == 1540);
    assert!(offset_of!(AccGraphics, strategy_tyre_set) == 1576);

    // Static. The strings are what pin this page: the car at 68, the track at
    // 134, the placeholder track configuration at 524 and the dry and wet
    // compound names at 688 and 754.
    assert!(offset_of!(AccStatic, car_model) == 68);
    assert!(offset_of!(AccStatic, track) == 134);
    assert!(offset_of!(AccStatic, sector_count) == 400);
    assert!(offset_of!(AccStatic, max_rpm) == 412);
    assert!(offset_of!(AccStatic, max_fuel) == 416);
    assert!(offset_of!(AccStatic, track_configuration) == 524);
    assert!(offset_of!(AccStatic, dry_tyres_name) == 688);
    assert!(offset_of!(AccStatic, wet_tyres_name) == 754);
};

/// A fixed-width UTF-16 field, NUL-padded, as the game writes it.
///
/// A wrapper rather than a bare `[u16; 33]` for two reasons: arrays longer
/// than 32 do not implement `Default`, and a name that arrives as raw units is
/// a name every caller has to decode by hand.
#[repr(C)]
#[derive(Debug, Clone, Copy, TryFromBytes)]
pub struct StringU16_33([u16; 33]);

impl Default for StringU16_33 {
    fn default() -> Self {
        Self([0u16; 33])
    }
}

impl Display for StringU16_33 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", read_acc_string(&self.0))
    }
}

impl From<&str> for StringU16_33 {
    fn from(value: &str) -> Self {
        let mut arr = [0u16; 33];
        for (i, c) in value.encode_utf16().enumerate() {
            if i < 32 {
                arr[i] = c;
            }
        }
        Self(arr)
    }
}

impl From<[u16; 33]> for StringU16_33 {
    fn from(value: [u16; 33]) -> Self {
        Self(value)
    }
}

/// How many cars ACC publishes coordinates and ids for, whatever the grid
/// actually holds.
pub const CAR_SLOTS: usize = 60;

/// Every car's world position, `[x, y, z]` in metres.
///
/// A wrapper for the same reason [`StringU16_33`] is one — an array of sixty
/// does not implement `Default` — and it earns its keep by being the only
/// place that knows a car id is an index into it.
#[repr(C)]
#[derive(Debug, Clone, Copy, TryFromBytes)]
pub struct CarPositions([[f32; 3]; CAR_SLOTS]);

impl CarPositions {
    /// One car's position, or `None` for an id outside the grid.
    ///
    /// A negative id is not car zero and an id past the end is not the last
    /// car: both mean "no car", and returning a neighbour's coordinates would
    /// put somebody else's dot on the track map.
    pub fn of(&self, car_id: i32) -> Option<[f32; 3]> {
        usize::try_from(car_id)
            .ok()
            .and_then(|index| self.0.get(index))
            .copied()
    }

    pub fn as_slice(&self) -> &[[f32; 3]] {
        &self.0
    }
}

impl Default for CarPositions {
    fn default() -> Self {
        Self([[0.0; 3]; CAR_SLOTS])
    }
}

impl std::ops::Index<usize> for CarPositions {
    type Output = [f32; 3];

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for CarPositions {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

/// The id of each car in the same sixty slots.
#[repr(C)]
#[derive(Debug, Clone, Copy, TryFromBytes)]
pub struct CarIds([i32; CAR_SLOTS]);

impl CarIds {
    pub fn as_slice(&self) -> &[i32] {
        &self.0
    }
}

impl Default for CarIds {
    fn default() -> Self {
        Self([0; CAR_SLOTS])
    }
}

/// Everything up to and including the first NUL.
pub fn read_acc_string(src: &[u16]) -> String {
    let len = src.iter().position(|&c| c == 0).unwrap_or(src.len());
    String::from_utf16_lossy(&src[..len])
}

/// ACC's `SPageFilePhysics`, 800 bytes.
///
/// The first 580 bytes are AC's layout field for field, which is why the two
/// are so easy to confuse; everything from `p2p_activations` onward is ACC's
/// own, and is where the interesting measurements live.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, TryFromBytes)]
pub struct AccPhysics {
    pub packet_id: i32,
    pub gas: f32,
    pub brake: f32,
    pub fuel: f32,
    /// **0 is reverse and 1 is neutral**, as in Assetto Corsa. The recording
    /// holds eight distinct values starting at 0 for a six-speed GT3 with a
    /// reverse, and the last sample reads 5 at 166 km/h and 6043 rpm — fourth
    /// gear. One published binding documents 0 as neutral; the bytes disagree.
    pub gear: i32,
    pub rpm: i32,
    pub steer_angle: f32,
    pub speed_kmh: f32,

    pub velocity: [f32; 3],
    pub acc_g: [f32; 3],

    pub wheel_slip: [f32; 4],
    /// Not published — zero for the whole session.
    pub wheel_load: [f32; 4],
    pub wheel_pressure: [f32; 4],
    pub wheel_angular_speed: [f32; 4],
    /// Not published. ACC publishes brake wear instead — see [`Self::pad_life`]
    /// — and the tyre-wear capability is false because of this field.
    pub tyre_wear: [f32; 4],
    /// Not published.
    pub tyre_dirty_level: [f32; 4],
    pub tyre_core_temp: [f32; 4],
    /// Not published, which is what withholds the camber advice.
    pub camber_rad: [f32; 4],
    pub suspension_travel: [f32; 4],
    /// Not published on a GT3; ACC has no DRS car.
    pub drs: f32,
    pub tc: f32,
    pub heading: f32,
    pub pitch: f32,
    pub roll: f32,
    /// Not published.
    pub cg_height: f32,
    /// Front, rear, left, right and centre. Zero for this session, which had
    /// no contact — declared, not proven.
    pub car_damage: [f32; 5],
    /// Not published.
    pub number_of_tyres_out: i32,
    pub pit_limiter_on: i32,
    pub abs: f32,
    /// Not published.
    pub kers_charge: f32,
    /// Not published.
    pub kers_input: f32,
    pub auto_shifter_on: i32,
    /// Not published.
    pub ride_height: [f32; 2],
    pub turbo_boost: f32,
    /// Not published.
    pub ballast: f32,
    /// Not published.
    pub air_density: f32,
    pub air_temp: f32,
    pub road_temp: f32,
    pub local_angular_vel: [f32; 3],
    pub final_ff: f32,
    /// Not published. AC's delta to its reference lap; ACC has
    /// [`AccGraphics::i_delta_lap_time`] instead.
    pub performance_meter: f32,
    /// Not published.
    pub engine_brake: i32,
    /// Not published.
    pub ers_recovery_level: i32,
    /// Not published.
    pub ers_power_level: i32,
    /// Not published.
    pub ers_heat_charging: i32,
    /// Not published.
    pub ers_is_charging: i32,
    /// Not published.
    pub kers_current_kj: f32,
    /// Not published.
    pub drs_available: i32,
    /// Not published.
    pub drs_enabled: i32,
    pub brake_temp: [f32; 4],
    pub clutch: f32,
    /// Not published. See [`Self::tyre_temp`], which is the one ACC fills.
    pub tyre_temp_i: [f32; 4],
    /// Not published.
    pub tyre_temp_m: [f32; 4],
    /// Not published.
    pub tyre_temp_o: [f32; 4],
    /// Not published.
    pub is_ai_controlled: i32,
    /// World position of each wheel's contact patch, `[x, y, z]` in metres.
    /// The front-left agrees with [`AccGraphics::car_coordinates`] to within a
    /// wheelbase, which is what pins both.
    pub tyre_contact_point: [[f32; 3]; 4],
    pub tyre_contact_normal: [[f32; 3]; 4],
    pub tyre_contact_heading: [[f32; 3]; 4],
    pub brake_bias: f32,
    pub local_velocity: [f32; 3],

    // ── ACC's own, from here on. AC's page ends at 596. ──
    /// Push-to-pass, for the categories that have it. Not published on GT3.
    pub p2p_activations: i32,
    /// Not published on GT3.
    pub p2p_status: i32,
    /// The rev limit in force now, which a GT3's engine map can move. Reads
    /// 8650 here, the same as the static page's `max_rpm`.
    pub current_max_rpm: i32,
    /// Self-aligning torque per wheel. Not published.
    pub mz: [f32; 4],
    /// Longitudinal force per wheel. Not published.
    pub fx: [f32; 4],
    /// Lateral force per wheel. Not published.
    pub fy: [f32; 4],
    pub slip_ratio: [f32; 4],
    pub slip_angle: [f32; 4],
    /// Declared `int` by ACC's own header, unlike AC's float. Zero throughout
    /// this session, so the type is taken from the header rather than proven.
    pub tc_in_action: i32,
    /// Declared `int`; see [`Self::tc_in_action`].
    pub abs_in_action: i32,
    /// Zero here — nothing was damaged.
    pub suspension_damage: [f32; 4],
    /// Core tyre temperature again, byte for byte identical to
    /// [`Self::tyre_core_temp`] 544 bytes earlier. Two fields agreeing across
    /// that distance is strong evidence both are aligned.
    pub tyre_temp: [f32; 4],
    pub water_temp: f32,
    /// Per-corner brake pressure, 0..1. Reads 0.76/0.76/0.24/0.24 under
    /// braking, which is [`Self::brake_bias`] applied front to rear.
    pub brake_pressure: [f32; 4],
    pub front_brake_compound: i32,
    pub rear_brake_compound: i32,
    /// Brake pad thickness in millimetres, per wheel. 29 mm here.
    ///
    /// **This is ACC's answer to tyre wear**, which it does not publish: for a
    /// GT3 stint the pads are the consumable that decides the race.
    pub pad_life: [f32; 4],
    /// Brake disc thickness in millimetres, per wheel. 32 mm here.
    pub disc_life: [f32; 4],
    pub ignition_on: i32,
    pub starter_engine_on: i32,
    pub is_engine_running: i32,
    /// Four vibration channels the game feeds to a wheel or a seat.
    pub kerb_vibration: f32,
    pub slip_vibrations: f32,
    pub g_vibrations: f32,
    pub abs_vibrations: f32,
}

/// ACC's `SPageFileGraphic`, 1588 bytes.
///
/// AC's is 360. The difference starts at offset 252 and it is the single most
/// dangerous property of this pair of games.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, TryFromBytes)]
pub struct AccGraphics {
    pub packet_id: i32,
    pub status: i32,
    /// ACC's `AC_SESSION_TYPE`, which is **not** the table this project uses
    /// for Assetto Corsa — 0 is practice here. See `reading.rs`.
    pub session: i32,
    pub current_time: [u16; 15],
    pub last_time: [u16; 15],
    pub best_time: [u16; 15],
    pub split: [u16; 15],
    pub completed_laps: i32,
    pub position: i32,
    pub i_current_time: i32,
    /// `i32::MAX` until there is a lap to report, not zero.
    pub i_last_time: i32,
    /// `i32::MAX` until there is a lap to report.
    pub i_best_time: i32,
    /// −1.0 in a session with no clock.
    pub session_time_left: f32,
    pub distance_traveled: f32,
    /// Never moved in the recording even while `is_in_pit_lane` did, so this
    /// is declared rather than proven.
    pub is_in_pit: i32,
    pub current_sector_index: i32,
    pub last_sector_time: i32,
    /// 0 in a session with no lap count.
    pub number_of_laps: i32,
    /// `dry_compound` or `wet_compound` — **not** one of Assetto Corsa's
    /// compound names, which is why the pressure bands keyed off those names
    /// fall through to the default on this game.
    pub tyre_compound: StringU16_33,
    /// Not published.
    pub replay_time_multiplier: f32,
    pub normalized_car_position: f32,

    /// How many cars the arrays below hold. **This field and the array after
    /// it are what AC does not have.**
    pub active_cars: i32,
    /// Every car's world position, `[x, y, z]` in metres, the player's at
    /// `player_car_id`. 720 bytes where AC has 12.
    pub car_coordinates: CarPositions,
    pub car_id: CarIds,
    pub player_car_id: i32,
    pub penalty_time: f32,
    pub flag: i32,
    /// ACC's penalty enum. AC has no such field, and assuming it did is half
    /// of the 964-byte shift.
    pub penalty: i32,
    pub ideal_line_on: i32,
    pub is_in_pit_lane: i32,
    /// Not published — zero throughout, where AC reports 0.94 on a green
    /// track. ACC says the same thing through [`Self::track_grip_status`].
    pub surface_grip: f32,
    pub mandatory_pit_done: i32,
    /// Not published.
    pub wind_speed: f32,
    /// Not published.
    pub wind_direction: f32,
    /// Not published.
    pub is_setup_menu_visible: i32,
    pub main_display_index: i32,
    pub secondary_display_index: i32,
    pub tc: i32,
    pub tc_cut: i32,
    pub engine_map: i32,
    pub abs: i32,
    /// Litres per lap, **a float**: 4.24 in the recording, which is a GT3
    /// figure and which `fuel_estimated_laps` divides 62 litres by to get
    /// 14.6. One published binding declares this an `i32`.
    pub fuel_x_lap: f32,
    pub rain_lights: i32,
    pub flashing_lights: i32,
    pub lights_stage: i32,
    pub exhaust_temperature: f32,
    pub wiper_stage: i32,
    /// −1000 where there is no stint limit.
    pub driver_stint_total_time_left: i32,
    pub driver_stint_time_left: i32,
    pub rain_tyres: i32,
    pub session_index: i32,
    pub used_fuel: f32,
    pub delta_lap_time: [u16; 15],
    pub i_delta_lap_time: i32,
    pub estimated_lap_time: [u16; 15],
    pub i_estimated_lap_time: i32,
    pub is_delta_positive: i32,
    pub i_split: i32,
    /// Whether the lap being driven still counts.
    ///
    /// Assetto Corsa never says this, so the analyser treats every lap as
    /// valid. ACC does say it.
    pub is_valid_lap: i32,
    /// Laps left in the tank on the game's own measurement.
    pub fuel_estimated_laps: f32,
    /// The track status as a word, in the game's language — "БЫСТР." in the
    /// recording, which is a reminder that this is a label and not a key.
    pub track_status: StringU16_33,
    pub missing_mandatory_pits: i32,
    /// Time of day in seconds.
    pub clock: f32,
    pub direction_lights_left: i32,
    pub direction_lights_right: i32,
    /// Yellow anywhere, then per sector.
    pub global_yellow: i32,
    pub global_yellow_1: i32,
    pub global_yellow_2: i32,
    pub global_yellow_3: i32,
    pub global_white: i32,
    pub global_green: i32,
    pub global_chequered: i32,
    pub global_red: i32,
    /// What the driver has dialled into the multi-function display, which is
    /// the half of a pit stop no other game here can report.
    pub mfd_tyre_set: i32,
    pub mfd_fuel_to_add: f32,
    pub mfd_tyre_pressure_lf: f32,
    pub mfd_tyre_pressure_rf: f32,
    pub mfd_tyre_pressure_lr: f32,
    pub mfd_tyre_pressure_rr: f32,
    pub track_grip_status: i32,
    pub rain_intensity: i32,
    pub rain_intensity_in_10min: i32,
    pub rain_intensity_in_30min: i32,
    pub current_tyre_set: i32,
    pub strategy_tyre_set: i32,
    /// Milliseconds to the car ahead. Zero throughout a single-car session,
    /// so declared rather than proven.
    pub gap_ahead: i32,
    /// Milliseconds to the car behind. Declared, not proven.
    pub gap_behind: i32,
}

/// ACC's `SPageFileStatic`, 820 bytes.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, TryFromBytes)]
pub struct AccStatic {
    /// The shared-memory contract this page was written to: **"1.9"** on ACC
    /// where Assetto Corsa writes "1.7". It is the field that tells the two
    /// games apart — see `shm::Memory`.
    pub sm_version: [u16; 15],
    pub ac_version: [u16; 15],
    /// Not published.
    pub number_of_sessions: i32,
    pub num_cars: i32,
    pub car_model: StringU16_33,
    pub track: StringU16_33,
    pub player_name: StringU16_33,
    pub player_surname: StringU16_33,
    /// Not published — empty in the recording, where AC repeats the name.
    pub player_nick: StringU16_33,
    pub sector_count: i32,
    /// Not published.
    pub max_torque: f32,
    /// Not published.
    pub max_power: f32,
    pub max_rpm: i32,
    pub max_fuel: f32,
    /// Not published.
    pub suspension_max_travel: [f32; 4],
    /// Not published, which settles nothing about whether it is a scalar or
    /// four: the field is empty either way.
    pub tyre_radius: [f32; 4],
    /// Not published.
    pub max_turbo_boost: f32,
    /// Not published.
    pub deprecated_1: f32,
    /// Not published.
    pub deprecated_2: f32,
    /// Not published.
    pub penalties_enabled: i32,
    /// Not published.
    pub aid_fuel_rate: f32,
    /// Not published.
    pub aid_tire_rate: f32,
    /// Not published.
    pub aid_mechanical_damage: f32,
    /// Not published.
    pub allow_tyre_blankets: i32,
    pub aid_stability: f32,
    pub aid_auto_clutch: i32,
    /// Not published.
    pub aid_auto_blip: i32,
    /// Not published.
    pub has_drs: i32,
    /// Not published.
    pub has_ers: i32,
    /// Not published.
    pub has_kers: i32,
    /// Not published.
    pub kers_max_j: f32,
    /// Not published.
    pub engine_brake_settings_count: i32,
    /// Not published.
    pub ers_power_controller_count: i32,
    /// **Not published**, and the consequence is that there is no track length
    /// on this game: everything that would report metres has to say "not
    /// measured" rather than invent one.
    pub track_spline_length: f32,
    /// The literal string "track config" — a placeholder ACC writes and never
    /// fills. Useless as a value, and excellent as proof of alignment.
    pub track_configuration: StringU16_33,
    /// Not published.
    pub ers_max_j: f32,
    /// Not published.
    pub is_timed_race: i32,
    /// Not published.
    pub has_extra_lap: i32,
    /// The literal string "skin", the same kind of placeholder.
    pub car_skin: StringU16_33,
    /// Not published.
    pub reversed_grid_positions: i32,
    /// Not published.
    pub pit_window_start: i32,
    /// −1000 where there is no pit window.
    pub pit_window_end: i32,
    pub is_online: i32,
    /// The dry compound this car runs, "DHD2" here.
    pub dry_tyres_name: StringU16_33,
    /// The wet compound, "WH" here. **Two-byte aligned rather than four**, so
    /// it starts at 754 and not 756 — reading it four bytes in loses the "W",
    /// which is the sort of mistake that looks like a game bug.
    pub wet_tyres_name: StringU16_33,
}

impl crate::memory::Versioned for AccPhysics {
    fn packet_id(&self) -> i32 {
        self.packet_id
    }
}

impl crate::memory::Versioned for AccGraphics {
    fn packet_id(&self) -> i32 {
        self.packet_id
    }
}
