use crate::config::Language;
use crate::games::reading::{COORD_X, COORD_Z};
use crate::games::{Car, Session};
use crate::records::TrackRecord;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use tracing::{debug, info};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LapData {
    pub lap_number: i32,
    pub lap_time_ms: i32,
    pub sectors: [i32; 3],
    pub valid: bool,

    pub car_model: String,
    pub track_name: String,
    /// The track's length in metres, from AC's own spline length.
    ///
    /// Carried on the lap rather than looked up, because a lap saved to a file
    /// is read back on a day when no track is loaded and `corners` still has to
    /// be able to say "14 m later" rather than "0.003 of a lap later". Zero
    /// means not known — laps saved before this existed have no value for it,
    /// and everything that reports metres refuses rather than inventing them.
    #[serde(default)]
    pub track_length_m: f32,
    /// Whether that length was measured from the car's own distance rather
    /// than published by the game.
    ///
    /// Competizione publishes none, so on that game it is worked out over a
    /// lap — and a screen reporting metres should be able to say which of the
    /// two it is showing, because afterwards both are an `f32`. Absent from
    /// laps saved before this existed, which is what `serde(default)` says.
    #[serde(default)]
    pub track_length_measured: bool,
    pub save_date: String,
    #[serde(default)]
    pub from_file: bool,

    pub air_temp: f32,
    pub road_temp: f32,
    pub track_grip: f32,
    pub timestamp: String,

    pub max_speed: f32,
    pub avg_speed: f32,
    /// Mean tyre pressure over the samples above the speed gate. `None`
    /// when the lap never got up to speed, so nothing was measured.
    pub avg_pressure: Option<f32>,
    pub min_corner_speed_avg: f32,
    pub fuel_used: f32,
    pub gear_shifts: i32,
    pub peak_lat_g: f32,
    pub peak_brake_g: f32,

    pub avg_tyre_temp: [f32; 4],
    pub max_brake_temp: [f32; 4],
    /// Mean absolute deviation from the target pressure, over the same
    /// samples as [`LapData::avg_pressure`]. `None` for the same reason.
    pub pressure_deviation: Option<f32>,
    pub suspension_travel_hist: [f32; 4],

    #[serde(default)]
    pub avg_wheels_pressure: [f32; 4],
    #[serde(default)]
    pub avg_tyre_temp_i: [f32; 4],
    #[serde(default)]
    pub avg_tyre_temp_m: [f32; 4],
    #[serde(default)]
    pub avg_tyre_temp_o: [f32; 4],
    #[serde(default)]
    pub avg_brake_temp: [f32; 4],
    #[serde(default)]
    pub avg_ride_height: [f32; 2],

    #[serde(default)]
    pub damper_histograms: [[f32; 4]; 4],

    pub throttle_smoothness: f32,
    pub steering_smoothness: f32,
    pub trail_braking_score: f32,
    pub coasting_percent: f32,
    pub pedal_overlap_percent: f32,
    pub full_throttle_percent: f32,
    pub grip_usage_percent: f32,

    pub oversteer_count: i32,
    pub understeer_count: i32,
    pub lockup_count: i32,

    pub scrubbing_incidents: i32,
    pub max_steering_over_rotation: f32,

    pub radar_stats: RadarStats,

    pub telemetry_trace: Vec<TelemetryPoint>,

    pub bounds_min_x: f32,
    pub bounds_max_x: f32,
    pub bounds_min_y: f32,
    pub bounds_max_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RadarStats {
    pub smoothness: f32,
    pub aggression: f32,
    pub consistency: f32,
    pub tyre_mgmt: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPoint {
    pub distance: f32,
    pub time_ms: i32,
    pub speed: f32,
    pub gas: f32,
    pub brake: f32,
    pub gear: i32,
    pub steer: f32,
    pub lat_g: f32,
    pub lon_g: f32,
    pub slip_avg: f32,
    /// Where the car was, in the track's own metres. **Defaulted**: added
    /// after the format shipped, so a lap saved before it has no such key.
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// Revs at this instant. **Defaulted, for the same reason, and it is the
    /// one that was missed.**
    ///
    /// A lap is a file on somebody's disk that outlives every release. This
    /// field was added to the struct and not to the format's rules, so every
    /// lap saved before it stopped loading — `not a saved lap: missing field
    /// \`rpms\`` — and the laps most worth keeping are the oldest ones.
    /// Reported from the outside, by a driver whose reference lap vanished.
    ///
    /// **The rule this file now follows:** a field added after the format
    /// shipped defaults; the ones that were there from the first version do
    /// not, so a genuinely wrong file still fails rather than loading as a lap
    /// of zeroes.
    #[serde(default)]
    pub rpms: i32,
    /// Everything else the game published at this instant.
    ///
    /// **Separate, and defaulted, on purpose.** The fields above are the ones
    /// every screen and every rule reads, and they were the whole of a sample
    /// until a front end wanted to plot suspension travel against distance and
    /// found there was nothing to plot. This is the rest of it — per corner
    /// where the car has four of something, and the handful of scalars that
    /// change through a lap.
    ///
    /// `serde(default)` because laps saved before it existed are still laps:
    /// they load, and everything drawn from this reads zero, which is why
    /// [`Detail::measured`] exists rather than a screen guessing.
    #[serde(default)]
    pub detail: Detail,
}

/// The rest of one instant, beyond what every screen reads.
///
/// Adds about two hundred bytes to a sample, which is a megabyte and a half
/// over a lap at sixty a second. That is the price of being able to plot any
/// channel the game publishes rather than the six somebody chose in advance,
/// and it is the right way round: a telemetry program that throws the data
/// away at the door cannot get it back.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Detail {
    /// False on a lap recorded before this existed, so a screen can tell a
    /// zero apart from a lap that never carried the field.
    pub measured: bool,

    pub tyre_core_temp_c: [f32; 4],
    pub tyre_temp_inner_c: [f32; 4],
    pub tyre_temp_middle_c: [f32; 4],
    pub tyre_temp_outer_c: [f32; 4],
    pub tyre_pressure_psi: [f32; 4],
    pub tyre_wear: [f32; 4],
    pub brake_temp_c: [f32; 4],
    pub brake_pad_mm: [f32; 4],
    pub wheel_load: [f32; 4],
    pub wheel_slip: [f32; 4],
    pub suspension_travel: [f32; 4],
    pub camber_rad: [f32; 4],

    /// Front and rear, in metres.
    pub ride_height_m: [f32; 2],
    pub vertical_g: f32,
    pub clutch: f32,
    pub fuel_litres: f32,
    pub brake_bias: f32,
    pub tc_in_action: f32,
    pub abs_in_action: f32,
    pub force_feedback: f32,
    pub air_temp_c: f32,
    pub road_temp_c: f32,
}

impl Detail {
    /// Everything of one reading that is not already on the point above it.
    pub fn of(car: &crate::games::Car) -> Self {
        Self {
            measured: true,
            tyre_core_temp_c: car.tyre_core_temp_c,
            tyre_temp_inner_c: car.tyre_temp_inner_c,
            tyre_temp_middle_c: car.tyre_temp_middle_c,
            tyre_temp_outer_c: car.tyre_temp_outer_c,
            tyre_pressure_psi: car.tyre_pressure_psi,
            tyre_wear: car.tyre_wear,
            brake_temp_c: car.brake_temp_c,
            brake_pad_mm: car.brake_pad_mm,
            wheel_load: car.wheel_load,
            wheel_slip: car.wheel_slip,
            suspension_travel: car.suspension_travel,
            camber_rad: car.camber_rad,
            ride_height_m: car.ride_height_m,
            vertical_g: car.acc_g[1],
            clutch: car.clutch,
            fuel_litres: car.fuel_litres,
            brake_bias: car.brake_bias,
            tc_in_action: car.tc_in_action,
            abs_in_action: car.abs_in_action,
            force_feedback: car.force_feedback,
            air_temp_c: car.air_temp_c,
            road_temp_c: car.road_temp_c,
        }
    }

    /// Between two instants, for resampling a lap onto an even grid.
    pub fn between(from: &Self, to: &Self, factor: f32) -> Self {
        let mix = |a: f32, b: f32| a + factor * (b - a);
        let corners = |a: &[f32; 4], b: &[f32; 4]| std::array::from_fn(|i| mix(a[i], b[i]));
        Self {
            // Only measured where both ends were: half of a reading nobody
            // took is still nobody's reading.
            measured: from.measured && to.measured,
            tyre_core_temp_c: corners(&from.tyre_core_temp_c, &to.tyre_core_temp_c),
            tyre_temp_inner_c: corners(&from.tyre_temp_inner_c, &to.tyre_temp_inner_c),
            tyre_temp_middle_c: corners(&from.tyre_temp_middle_c, &to.tyre_temp_middle_c),
            tyre_temp_outer_c: corners(&from.tyre_temp_outer_c, &to.tyre_temp_outer_c),
            tyre_pressure_psi: corners(&from.tyre_pressure_psi, &to.tyre_pressure_psi),
            tyre_wear: corners(&from.tyre_wear, &to.tyre_wear),
            brake_temp_c: corners(&from.brake_temp_c, &to.brake_temp_c),
            brake_pad_mm: corners(&from.brake_pad_mm, &to.brake_pad_mm),
            wheel_load: corners(&from.wheel_load, &to.wheel_load),
            wheel_slip: corners(&from.wheel_slip, &to.wheel_slip),
            suspension_travel: corners(&from.suspension_travel, &to.suspension_travel),
            camber_rad: corners(&from.camber_rad, &to.camber_rad),
            ride_height_m: std::array::from_fn(|i| mix(from.ride_height_m[i], to.ride_height_m[i])),
            vertical_g: mix(from.vertical_g, to.vertical_g),
            clutch: mix(from.clutch, to.clutch),
            fuel_litres: mix(from.fuel_litres, to.fuel_litres),
            brake_bias: mix(from.brake_bias, to.brake_bias),
            tc_in_action: mix(from.tc_in_action, to.tc_in_action),
            abs_in_action: mix(from.abs_in_action, to.abs_in_action),
            force_feedback: mix(from.force_feedback, to.force_feedback),
            air_temp_c: mix(from.air_temp_c, to.air_temp_c),
            road_temp_c: mix(from.road_temp_c, to.road_temp_c),
        }
    }
}

/// Shortest split treated as a real sector. Anything under a second is AC
/// reporting a partial or reset timer rather than a driven sector.
pub const MIN_VALID_SECTOR_MS: i32 = 1000;

/// How many samples make up one incident at the configured update rate.
///
/// The mistake counters are incremented once per sample, so the number of
/// samples an incident lasts depends on how often the app is sampling.
/// `at_60hz` is the run length the thresholds were tuned against; the result
/// is that same duration expressed in samples at `update_rate_ms`.
///
/// This is the same normalisation `Engineer::update_stats` already applies to
/// its own counters, which is why the engineer's numbers were stable across
/// update rates while the analyzer's were not.
fn samples_per_incident(at_60hz: i32, update_rate_ms: u64) -> i32 {
    const SIXTY_HZ_MS: f32 = 1000.0 / 60.0;
    let rate = (update_rate_ms as f32).max(1.0);
    ((at_60hz as f32 * SIXTY_HZ_MS / rate).round() as i32).max(1)
}

/// Replace an untouched coordinate bound with zero.
///
/// The bounds scan seeds min with `f32::MAX` and max with `f32::MIN`, so a lap
/// where no sample carried usable coordinates leaves those seeds in place.
/// They are perfectly finite, so a `is_finite` check does not catch them — the
/// sentinel value itself is the signal.
fn without_sentinel(value: f32) -> f32 {
    if value == f32::MAX || value == f32::MIN || !value.is_finite() {
        0.0
    } else {
        value
    }
}

/// One episode counted once, however long it lasts.
///
/// **Why this is not a per-sample counter.** It was one: every sample where a
/// tyre was sliding added one, and the total was divided by the number of
/// samples an incident lasts. That arithmetic is only correct if incidents
/// come in fixed-length pieces. They do not — a single slide held for half a
/// lap is one incident, and dividing counted it as hundreds of them. On
/// Competizione, where the slip figures sit above the threshold for most of a
/// lap, that produced "Oversteer: 1454x" and "understeer: 1105 a lap" for the
/// same lap, both at once: not two symptoms but one condition, counted per
/// sample.
///
/// So an episode is counted when the condition has held for `min_run` samples,
/// and cannot be counted again until it has been absent for as long. The run
/// length is the same duration the divisor used to encode, which is why
/// [`samples_per_incident`] is still what supplies it.
#[derive(Default)]
struct Episodes {
    count: i32,
    active: bool,
    run: i32,
}

impl Episodes {
    fn sample(&mut self, present: bool, min_run: i32) {
        if present == self.active {
            self.run = 0;
            return;
        }
        self.run += 1;
        if self.run < min_run {
            return;
        }
        // Held long enough to be a change of state rather than one noisy
        // sample. Only the start of an episode is counted; its end just arms
        // the next one.
        self.active = present;
        self.run = 0;
        if present {
            self.count += 1;
        }
    }
}

pub struct TelemetryTrace;

impl TelemetryTrace {
    /// Resample a telemetry trace by normalized distance (0.0 to 1.0) with spatial step (e.g. 0.002 = 500 samples per lap).
    pub fn resample_by_distance(points: &[TelemetryPoint], step: f32) -> Vec<TelemetryPoint> {
        if points.is_empty() {
            return Vec::new();
        }
        if points.len() == 1 || step <= 0.0 {
            return points.to_vec();
        }

        let mut sorted = points.to_vec();
        sorted.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut resampled = Vec::new();
        let mut target_dist = 0.0f32;
        let max_dist = sorted.last().map(|p| p.distance).unwrap_or(1.0).min(1.0);

        let mut idx = 0;
        while target_dist <= max_dist + 1e-4 && idx < sorted.len() - 1 {
            while idx < sorted.len() - 2 && sorted[idx + 1].distance < target_dist {
                idx += 1;
            }

            let p0 = &sorted[idx];
            let p1 = &sorted[idx + 1];

            let dist_diff = p1.distance - p0.distance;
            let factor = if dist_diff > 1e-6 {
                ((target_dist - p0.distance) / dist_diff).clamp(0.0, 1.0)
            } else {
                0.0
            };

            resampled.push(TelemetryPoint {
                distance: target_dist,
                time_ms: (p0.time_ms as f32 + factor * (p1.time_ms - p0.time_ms) as f32) as i32,
                speed: p0.speed + factor * (p1.speed - p0.speed),
                gas: p0.gas + factor * (p1.gas - p0.gas),
                brake: p0.brake + factor * (p1.brake - p0.brake),
                gear: if factor < 0.5 { p0.gear } else { p1.gear },
                steer: p0.steer + factor * (p1.steer - p0.steer),
                lat_g: p0.lat_g + factor * (p1.lat_g - p0.lat_g),
                lon_g: p0.lon_g + factor * (p1.lon_g - p0.lon_g),
                slip_avg: p0.slip_avg + factor * (p1.slip_avg - p0.slip_avg),
                x: p0.x + factor * (p1.x - p0.x),
                y: p0.y + factor * (p1.y - p0.y),
                rpms: if factor < 0.5 { p0.rpms } else { p1.rpms },
                detail: Detail::between(&p0.detail, &p1.detail, factor),
            });

            target_dist += step;
        }

        if resampled.is_empty() {
            sorted
        } else {
            resampled
        }
    }
}

pub struct LapComparison;

impl LapComparison {
    /// Time gained and lost against a reference lap, ready to plot.
    ///
    /// `(seconds into this lap, seconds behind the reference)`. Positive is
    /// slower, which is the sign every driver reads.
    ///
    /// **Paired by distance, not by index.** What was here resampled both laps
    /// onto a fixed grid and then compared sample *i* of one with sample *i* of
    /// the other — which is the same place on the track only if both traces
    /// happen to start at the same distance and hold the same number of
    /// samples. They do not: a lap recorded from a standing start and one
    /// joined at speed begin at different points, and the whole graph was then
    /// shifted by the difference with nothing to show that it was.
    ///
    /// [`crate::corners::delta_ms_at`] interpolates both laps at the same
    /// distance, and withholds an answer where either lap has nothing there —
    /// so a stretch the reference never covered is absent from the line rather
    /// than drawn flat.
    pub fn delta_over_time(
        current: &[TelemetryPoint],
        reference: &[TelemetryPoint],
    ) -> Vec<(f64, f64)> {
        current
            .iter()
            .filter_map(|point| {
                let delta = crate::corners::delta_ms_at(current, reference, point.distance)?;
                Some((point.time_ms as f64 / 1000.0, delta as f64 / 1000.0))
            })
            .collect()
    }
}

pub struct StandaloneAnalysis {
    pub is_perfect: bool,
    pub advices: Vec<Advice>,
}

#[derive(Debug, Clone)]
pub struct Advice {
    pub zone: String,
    pub problem: String,
    pub solution: String,
    pub severity: u8,
}

#[derive(Debug, Serialize, Clone)]
pub struct AnalysisResult {
    pub category: String,
    pub metric: String,
    pub value: f32,
    pub unit: String,
    pub recommendation: String,
}

pub struct TelemetryAnalyzer {
    pub laps: Vec<LapData>,
    pub best_lap_index: Option<usize>,
    pub best_sectors: [i32; 3],
    pub world_record: Option<TrackRecord>,
    pub reference_lap: Option<LapData>,
    /// The loaded track's length in metres, stamped onto every lap processed
    /// from here on. Set once when a session is recognised; zero until then.
    pub track_length_m: f32,
    /// Worked out from the car's own distance, for a game that does not publish
    /// a length. See [`crate::track::MeasuredLength`].
    measured_length: crate::track::MeasuredLength,
    /// Whether [`Self::track_length_m`] was measured here rather than reported
    /// by the game.
    ///
    /// A front end that says "7004 m" should be able to say where that came
    /// from, and nothing else can tell afterwards: both are an `f32` in metres.
    track_length_measured: bool,
}

pub type Analyzer = TelemetryAnalyzer;

impl Default for TelemetryAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryAnalyzer {
    pub fn new() -> Self {
        Self {
            laps: Vec::new(),
            best_lap_index: None,
            best_sectors: [i32::MAX, i32::MAX, i32::MAX],
            world_record: None,
            reference_lap: None,
            track_length_m: 0.0,
            measured_length: crate::track::MeasuredLength::default(),
            track_length_measured: false,
        }
    }

    /// Throw away everything that belonged to the last car and track.
    ///
    /// A driver going back to the menu and picking another car does not restart
    /// this program, and the shared memory stays mapped throughout — so without
    /// this, laps driven in a Miata sit in the same list as laps driven in a
    /// GT3, the best of them becomes the reference the other car is compared
    /// against, and the measured circuit length from the last track stamps
    /// itself onto laps of the new one.
    ///
    /// The world record goes too: it is looked up per car and per track, and is
    /// set again when the first lap of the new pairing finishes.
    pub fn start_new_session(&mut self) {
        self.laps.clear();
        self.best_lap_index = None;
        self.best_sectors = [i32::MAX, i32::MAX, i32::MAX];
        self.reference_lap = None;
        self.world_record = None;
        self.track_length_m = 0.0;
        self.track_length_measured = false;
        self.measured_length = crate::track::MeasuredLength::default();
    }

    pub fn set_world_record(&mut self, record: TrackRecord) {
        self.world_record = Some(record);
    }

    /// Tell the analyser how long the track is, so laps recorded from now on
    /// carry it. Ignores a value the game has not filled in yet, which it has
    /// not during the first frames of a session.
    ///
    /// **A published length wins over a measured one** and clears the mark: the
    /// game's own number is exact, and the measurement is only ever there
    /// because the game had nothing to say.
    pub fn set_track_length(&mut self, metres: f32) {
        if metres > 0.0 {
            self.track_length_m = metres;
            self.track_length_measured = false;
        }
    }

    /// Feed the lap-length measurement one sample.
    ///
    /// For the game that publishes no track length. Harmless on the one that
    /// does — a published length is never replaced — so a front end calls this
    /// every tick without asking which game it is on, which is the only way it
    /// stays correct when a third game arrives.
    ///
    /// Returns the length in metres on the tick a lap is first measured, so a
    /// caller can log it or say so on screen.
    pub fn observe_distance(
        &mut self,
        track_position: f32,
        distance_travelled_m: f32,
    ) -> Option<f32> {
        let measured = self
            .measured_length
            .observe(track_position, distance_travelled_m)?;
        if self.track_length_measured || self.track_length_m <= 0.0 {
            self.track_length_m = measured;
            self.track_length_measured = true;
            return Some(measured);
        }
        None
    }

    /// Whether the track length came from this program measuring it rather than
    /// from the game reporting it.
    pub fn track_length_measured(&self) -> bool {
        self.track_length_measured
    }

    // Ten parameters, all of them distinct lap facts the caller already holds.
    // Bundling them into a struct would only move the argument list to the call
    // site, so the lint is acknowledged rather than worked around. `expect`
    // rather than `allow`: if the signature is ever trimmed, this goes stale
    // loudly instead of lingering.
    #[expect(clippy::too_many_arguments)]
    pub fn process_lap(
        &mut self,
        lap_number: i32,
        lap_time_ms: i32,
        car_log: &[Car],
        session_log: &[Session],
        sectors: [i32; 3],
        car_name: String,
        track_name: String,
        target_pressure: f32,
        update_rate_ms: u64,
    ) {
        if car_log.is_empty() {
            return;
        }

        info!(
            "Processing Lap {} | Time: {}ms | Car: {}",
            lap_number, lap_time_ms, car_name
        );

        // sectors already computed by caller
        for (i, sector) in sectors.iter().enumerate() {
            if *sector > MIN_VALID_SECTOR_MS && *sector < self.best_sectors[i] {
                self.best_sectors[i] = *sector;
            }
        }

        let air_temp = car_log.first().map(|p| p.air_temp_c).unwrap_or(20.0);
        let road_temp = car_log.first().map(|p| p.road_temp_c).unwrap_or(20.0);
        let track_grip = session_log.first().map(|g| g.surface_grip).unwrap_or(1.0) * 100.0;
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        let save_date = chrono::Local::now().format("%Y-%m-%d").to_string();

        let max_speed = car_log.iter().map(|p| p.speed_kmh).fold(0.0, f32::max);
        let avg_speed = if !car_log.is_empty() {
            car_log.iter().map(|p| p.speed_kmh).sum::<f32>() / car_log.len() as f32
        } else {
            0.0
        };

        let start_fuel = car_log.first().map(|p| p.fuel_litres).unwrap_or(0.0);
        let end_fuel = car_log.last().map(|p| p.fuel_litres).unwrap_or(0.0);
        let fuel_used = (start_fuel - end_fuel).max(0.0);

        let mut coasting_frames = 0;
        let mut overlap_frames = 0;
        let mut full_throttle_frames = 0;
        let mut gear_shifts = 0;
        let mut prev_gear = car_log.first().map(|p| p.gear).unwrap_or(0);

        let mut trail_braking_score_acc = 0.0;
        let mut trail_braking_samples = 0.0;
        let mut grip_usage_acc = 0.0;
        let mut grip_samples = 0.0;

        let mut oversteer_c = Episodes::default();
        let mut understeer_c = Episodes::default();
        let mut lockup_c = Episodes::default();
        let mut scrubbing_c = Episodes::default();
        let steady = samples_per_incident(5, update_rate_ms);
        let steady_long = samples_per_incident(10, update_rate_ms);
        let mut max_over_rotation = 0.0_f32;

        let mut total_jerk = 0.0;
        let mut prev_acc = 0.0;
        let mut steer_jerk = 0.0;
        let mut prev_steer = 0.0;

        let mut peak_lat_g: f32 = 0.0;
        let mut peak_brake_g: f32 = 0.0;

        let mut max_brake_temp = [0.0; 4];
        let mut sum_tyre_temp = [0.0; 4];
        let mut sum_susp_travel = [0.0; 4];
        let mut press_sum = 0.0;
        let mut press_dev_acc = 0.0;
        let mut pressure_sample_frames = 0_u32;

        let mut sum_wheels_pressure = [0.0; 4];
        let mut sum_tyre_temp_i = [0.0; 4];
        let mut sum_tyre_temp_m = [0.0; 4];
        let mut sum_tyre_temp_o = [0.0; 4];
        let mut sum_brake_temp_avg = [0.0; 4];
        let mut sum_ride_height = [0.0; 2];

        let mut prev_susp_travel = car_log
            .first()
            .map(|p| p.suspension_travel)
            .unwrap_or([0.0; 4]);
        let mut damper_counts = [[0.0_f32; 4]; 4];
        let mut damper_total_moves = [0.0_f32; 4];

        let log_len = car_log.len() as f32;

        for p in car_log {
            if p.speed_kmh > 50.0 {
                pressure_sample_frames += 1;
            }

            let acc = p.acc_g[2];
            total_jerk += (acc - prev_acc).abs();
            prev_acc = acc;

            steer_jerk += (p.steer_angle - prev_steer).abs();
            prev_steer = p.steer_angle;

            let lat_g = p.acc_g[0];
            let lon_g = p.acc_g[2];

            if lat_g.abs() > peak_lat_g {
                peak_lat_g = lat_g.abs();
            }
            if lon_g < peak_brake_g {
                peak_brake_g = lon_g;
            }

            let combined_g = (lat_g.powi(2) + lon_g.powi(2)).sqrt();
            if combined_g > 0.5 {
                grip_usage_acc += combined_g;
                grip_samples += 1.0;
            }

            if p.gear != prev_gear {
                gear_shifts += 1;
                prev_gear = p.gear;
            }

            if p.throttle > 0.95 {
                full_throttle_frames += 1;
            }
            if p.speed_kmh > 30.0 && p.throttle < 0.05 && p.brake < 0.05 {
                coasting_frames += 1;
            }
            if p.throttle > 0.1 && p.brake > 0.1 {
                overlap_frames += 1;
            }

            if p.brake > 0.1 && p.steer_angle.abs() > 0.05 {
                let steer_factor = p.steer_angle.abs().min(1.0);
                let brake_ideal = (1.0 - steer_factor).max(0.0);
                let diff = (brake_ideal - p.brake).abs();
                trail_braking_score_acc += (1.0 - diff).max(0.0);
                trail_braking_samples += 1.0;
            }

            if p.speed_kmh > 20.0 {
                let slip_vals = p.wheel_slip;

                lockup_c.sample(
                    slip_vals.iter().any(|&s| s.abs() > 0.2) && p.brake > 0.5,
                    steady,
                );
                oversteer_c.sample(slip_vals[2].abs() > 0.3 || slip_vals[3].abs() > 0.3, steady);
                understeer_c.sample(slip_vals[0].abs() > 0.3 || slip_vals[1].abs() > 0.3, steady);

                let scrubbing = p.speed_kmh > 40.0
                    && p.steer_angle.abs() > 0.15
                    && (slip_vals[0] > 0.15 || slip_vals[1] > 0.15);
                scrubbing_c.sample(scrubbing, steady_long);
                if scrubbing {
                    let excess = (p.steer_angle.abs() - 0.15) * 57.2958;
                    if excess > max_over_rotation {
                        max_over_rotation = excess;
                    }
                }
            }

            for i in 0..4 {
                if p.brake_temp_c[i] > max_brake_temp[i] {
                    max_brake_temp[i] = p.brake_temp_c[i];
                }
                let t_avg = p.avg_tyre_temp_c(i);
                sum_tyre_temp[i] += t_avg;
                sum_susp_travel[i] += p.suspension_travel[i];

                if p.speed_kmh > 50.0 {
                    press_sum += p.tyre_pressure_psi[i];
                    press_dev_acc += (p.tyre_pressure_psi[i] - target_pressure).abs();
                }

                sum_wheels_pressure[i] += p.tyre_pressure_psi[i];
                sum_tyre_temp_i[i] += p.tyre_temp_inner_c[i];
                sum_tyre_temp_m[i] += p.tyre_temp_middle_c[i];
                sum_tyre_temp_o[i] += p.tyre_temp_outer_c[i];
                sum_brake_temp_avg[i] += p.brake_temp_c[i];

                let delta_travel = p.suspension_travel[i] - prev_susp_travel[i];
                let dt_sec = update_rate_ms as f32 / 1000.0;
                let vel_mm_s = (delta_travel / dt_sec) * 1000.0;

                if vel_mm_s.abs() > 2.0 {
                    damper_total_moves[i] += 1.0;
                    if vel_mm_s > 30.0 {
                        damper_counts[i][1] += 1.0;
                    } else if vel_mm_s > 2.0 {
                        damper_counts[i][0] += 1.0;
                    } else if vel_mm_s < -30.0 {
                        damper_counts[i][3] += 1.0;
                    } else if vel_mm_s < -2.0 {
                        damper_counts[i][2] += 1.0;
                    }
                }
                prev_susp_travel[i] = p.suspension_travel[i];
            }

            sum_ride_height[0] += p.ride_height_m[0];
            sum_ride_height[1] += p.ride_height_m[1];
        }

        let mut damper_histograms = [[0.0; 4]; 4];
        for i in 0..4 {
            let total = if damper_total_moves[i] > 0.0 {
                damper_total_moves[i]
            } else {
                1.0
            };
            damper_histograms[i][0] = (damper_counts[i][0] / total) * 100.0;
            damper_histograms[i][1] = (damper_counts[i][1] / total) * 100.0;
            damper_histograms[i][2] = (damper_counts[i][2] / total) * 100.0;
            damper_histograms[i][3] = (damper_counts[i][3] / total) * 100.0;
        }

        debug!("Damper Histograms calculated successfully");

        let throttle_smoothness = if log_len > 0.0 {
            (100.0 - (total_jerk / log_len) * 50.0).clamp(0.0, 100.0)
        } else {
            100.0
        };

        let steering_smoothness = if log_len > 0.0 {
            (100.0 - (steer_jerk / log_len) * 200.0).clamp(0.0, 100.0)
        } else {
            100.0
        };

        let trail_score = if trail_braking_samples > 0.0 {
            (trail_braking_score_acc / trail_braking_samples * 100.0).clamp(0.0, 100.0)
        } else {
            50.0
        };

        let grip_usage_percent = if grip_samples > 0.0 {
            ((grip_usage_acc / grip_samples) / 2.0 * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        let coasting_pct = if log_len > 0.0 {
            (coasting_frames as f32 / log_len) * 100.0
        } else {
            0.0
        };
        let overlap_pct = if log_len > 0.0 {
            (overlap_frames as f32 / log_len) * 100.0
        } else {
            0.0
        };
        let full_throttle_pct = if log_len > 0.0 {
            (full_throttle_frames as f32 / log_len) * 100.0
        } else {
            0.0
        };

        let safe_div_len = if log_len > 0.0 { log_len } else { 1.0 };

        let avg_tyre_temp = [
            sum_tyre_temp[0] / safe_div_len,
            sum_tyre_temp[1] / safe_div_len,
            sum_tyre_temp[2] / safe_div_len,
            sum_tyre_temp[3] / safe_div_len,
        ];

        let suspension_travel_hist = [
            sum_susp_travel[0] / safe_div_len,
            sum_susp_travel[1] / safe_div_len,
            sum_susp_travel[2] / safe_div_len,
            sum_susp_travel[3] / safe_div_len,
        ];

        // Pressure metrics only count samples above the speed gate, so a lap
        // that never got up to speed — an out-lap, a wet crawl, a spin and
        // recovery — has no samples at all. Dividing by a floor of 1 in that
        // case gave 0.0 psi and a deviation of 0.0, which then scored a
        // *perfect* 100 on tyre management: an out-lap rated better than a
        // hot lap. `None` means "not measured" and is rendered as a dash.
        let has_pressure_samples = pressure_sample_frames > 0;
        let pressure_sample_count = (pressure_sample_frames as f32 * 4.0).max(1.0);
        let pressure_deviation =
            has_pressure_samples.then(|| press_dev_acc / pressure_sample_count);
        let avg_pressure = has_pressure_samples.then(|| press_sum / pressure_sample_count);

        let avg_wheels_pressure = [
            sum_wheels_pressure[0] / safe_div_len,
            sum_wheels_pressure[1] / safe_div_len,
            sum_wheels_pressure[2] / safe_div_len,
            sum_wheels_pressure[3] / safe_div_len,
        ];
        let avg_tyre_temp_i = [
            sum_tyre_temp_i[0] / safe_div_len,
            sum_tyre_temp_i[1] / safe_div_len,
            sum_tyre_temp_i[2] / safe_div_len,
            sum_tyre_temp_i[3] / safe_div_len,
        ];
        let avg_tyre_temp_m = [
            sum_tyre_temp_m[0] / safe_div_len,
            sum_tyre_temp_m[1] / safe_div_len,
            sum_tyre_temp_m[2] / safe_div_len,
            sum_tyre_temp_m[3] / safe_div_len,
        ];
        let avg_tyre_temp_o = [
            sum_tyre_temp_o[0] / safe_div_len,
            sum_tyre_temp_o[1] / safe_div_len,
            sum_tyre_temp_o[2] / safe_div_len,
            sum_tyre_temp_o[3] / safe_div_len,
        ];
        let avg_brake_temp = [
            sum_brake_temp_avg[0] / safe_div_len,
            sum_brake_temp_avg[1] / safe_div_len,
            sum_brake_temp_avg[2] / safe_div_len,
            sum_brake_temp_avg[3] / safe_div_len,
        ];
        let avg_ride_height = [
            sum_ride_height[0] / safe_div_len,
            sum_ride_height[1] / safe_div_len,
        ];

        let aggro_score = (grip_usage_percent + full_throttle_pct) / 2.0;

        let consistency_score = if let Some(best_idx) = self.best_lap_index {
            if best_idx < self.laps.len() {
                let diff = (lap_time_ms - self.laps[best_idx].lap_time_ms).abs();
                (100.0 - (diff as f32 / 500.0) * 10.0).clamp(0.0, 100.0)
            } else {
                100.0
            }
        } else {
            100.0
        };

        // With nothing measured, sit at the neutral middle rather than claim a
        // perfect score.
        let tyre_score = match pressure_deviation {
            Some(dev) => (100.0 - dev * 20.0).clamp(0.0, 100.0),
            None => 50.0,
        };

        let radar = RadarStats {
            smoothness: (throttle_smoothness + steering_smoothness) / 2.0 / 100.0,
            aggression: aggro_score / 100.0,
            consistency: consistency_score / 100.0,
            tyre_mgmt: tyre_score / 100.0,
        };

        let mut trace = Vec::new();

        let step = 5;

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for (i, p) in car_log.iter().enumerate() {
            if i % step == 0 {
                let g = if i < session_log.len() {
                    &session_log[i]
                } else {
                    match session_log.last() {
                        Some(last) => last,
                        None => continue,
                    }
                };

                let x = g.car_position_m[COORD_X];
                let z = g.car_position_m[COORD_Z];

                if x.abs() > 0.1 || z.abs() > 0.1 {
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if z < min_y {
                        min_y = z;
                    }
                    if z > max_y {
                        max_y = z;
                    }
                }

                let wheel_slip = p.wheel_slip;
                let slip_avg = if !wheel_slip.is_empty() {
                    wheel_slip.iter().sum::<f32>() / wheel_slip.len() as f32
                } else {
                    0.0
                };

                trace.push(TelemetryPoint {
                    distance: g.track_position,
                    time_ms: g.current_lap_ms,
                    speed: p.speed_kmh,
                    gas: p.throttle,
                    brake: p.brake,
                    // The reading already counts reverse as −1; this used to
                    // subtract one itself, because what arrived here was AC's
                    // own numbering.
                    gear: p.gear,
                    steer: p.steer_angle,
                    lat_g: p.acc_g[0],
                    lon_g: p.acc_g[2],
                    slip_avg,
                    x,
                    y: z,
                    rpms: p.rpm,
                    detail: Detail::of(p),
                });
            }
        }

        trace.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(Ordering::Equal)
        });

        let corner_points: Vec<&TelemetryPoint> = trace
            .iter()
            .filter(|p| p.speed > 30.0 && p.lat_g.abs() > 0.5)
            .collect();

        let min_corner_speed_avg = if !corner_points.is_empty() {
            corner_points.iter().map(|p| p.speed).sum::<f32>() / corner_points.len() as f32
        } else {
            0.0
        };

        let lap_data = LapData {
            lap_number,
            lap_time_ms,
            sectors,
            // A lap is invalid if the game said so at any point during it, not
            // only at the flag: a game that reports track limits clears the
            // flag when they are exceeded and leaves it clear for the rest of
            // the lap, and a reading taken after the line would miss it.
            //
            // A game that never says — Assetto Corsa — leaves `lap_is_valid`
            // true on every sample, so this is `true` there exactly as it was
            // before. That is not a claim the lap was clean; it is the absence
            // of anything saying it was not, which is what
            // `Capabilities::lap_validity` distinguishes.
            valid: session_log.iter().all(|s| s.lap_is_valid),
            car_model: car_name,
            track_name,
            track_length_m: self.track_length_m,
            track_length_measured: self.track_length_measured,
            save_date,
            from_file: false,
            air_temp,
            road_temp,
            track_grip,
            timestamp,
            max_speed,
            avg_speed,
            avg_pressure,
            min_corner_speed_avg,
            fuel_used,
            gear_shifts,
            peak_lat_g,
            peak_brake_g: peak_brake_g.abs(),
            avg_tyre_temp,
            max_brake_temp,
            pressure_deviation,
            suspension_travel_hist,
            avg_wheels_pressure,
            avg_tyre_temp_i,
            avg_tyre_temp_m,
            avg_tyre_temp_o,
            avg_brake_temp,
            avg_ride_height,
            damper_histograms,
            throttle_smoothness,
            steering_smoothness,
            trail_braking_score: trail_score,
            coasting_percent: coasting_pct,
            pedal_overlap_percent: overlap_pct,
            full_throttle_percent: full_throttle_pct,
            grip_usage_percent,
            // Episodes, counted once each — see `Episodes`. The rate the app
            // sampled at is already accounted for in the run length these were
            // opened with, so there is nothing left to divide by, and a slide
            // held for half a lap is one of these rather than hundreds.
            oversteer_count: oversteer_c.count,
            understeer_count: understeer_c.count,
            lockup_count: lockup_c.count,
            scrubbing_incidents: scrubbing_c.count,
            max_steering_over_rotation: max_over_rotation,
            radar_stats: radar,
            telemetry_trace: trace,
            // Without a single usable coordinate these are still the
            // f32::MAX/f32::MIN sentinels the scan started from, and they get
            // serialised into the saved lap that way. The renderer guards
            // against them, but anything computing `max - min` off the file
            // gets -6.8e38. Collapse to zero, which reads as "no track map".
            bounds_min_x: without_sentinel(min_x),
            bounds_max_x: without_sentinel(max_x),
            bounds_min_y: without_sentinel(min_y),
            bounds_max_y: without_sentinel(max_y),
        };

        self.laps.push(lap_data);
        info!("Lap {} successfully added to telemetry stack.", lap_number);

        // **A lap the game called invalid is not a best lap.** The best
        // sectors have always filtered on this; the best *lap* did not, so on
        // a game that reports track limits — Competizione does, Assetto Corsa
        // never has — a cut lap became the reference every later lap was
        // compared against, and the consistency score's baseline. On a game
        // that never says, `valid` is true for every lap and this changes
        // nothing.
        //
        // If every lap so far was invalid there is no best lap, which is the
        // same answer the sectors give and is the honest one: there is nothing
        // yet that counted.
        let is_valid = self.laps.last().map(|lap| lap.valid).unwrap_or(false);
        if !is_valid {
            return;
        }
        if let Some(best_idx) = self.best_lap_index {
            if best_idx < self.laps.len() {
                if lap_time_ms < self.laps[best_idx].lap_time_ms && lap_time_ms > 10000 {
                    self.best_lap_index = Some(self.laps.len() - 1);
                }
            } else {
                self.best_lap_index = Some(self.laps.len() - 1);
            }
        } else if lap_time_ms > 10000 {
            self.best_lap_index = Some(self.laps.len() - 1);
        }
    }

    pub fn analyze_standalone(&self, lap: &LapData, _lang: &Language) -> StandaloneAnalysis {
        let mut advices = Vec::new();

        if lap.pressure_deviation.is_some_and(|dev| dev > 0.5)
            && let Some(avg_pressure) = lap.avg_pressure
        {
            let target = 27.5;
            let diff = avg_pressure - target;

            if diff > 0.5 {
                advices.push(Advice {
                    zone: "Tyres".into(),
                    problem: format!("Pressure High: {:.1} psi", avg_pressure),
                    solution: format!("Deflate tyres by {:.1} psi.", diff),
                    severity: 3,
                });
            } else if diff < -0.5 {
                advices.push(Advice {
                    zone: "Tyres".into(),
                    problem: format!("Pressure Low: {:.1} psi", avg_pressure),
                    solution: format!("Inflate tyres by {:.1} psi.", diff.abs()),
                    severity: 3,
                });
            }
        }

        if let Some(wr) = &self.world_record {
            let diff = (lap.lap_time_ms - wr.time_ms) as f32 / 1000.0;
            if diff > 5.0 {
                advices.push(Advice {
                    zone: "Pace".into(),
                    problem: format!("Off WR Pace by +{:.1}s", diff),
                    solution: "Focus on corner exit speed.".into(),
                    severity: 1,
                });
            }
        }

        if lap.track_grip < 96.0 {
            advices.push(Advice {
                zone: "Track".into(),
                problem: format!("Low Grip: {:.1}%", lap.track_grip),
                solution: "Brake earlier, smooth throttle.".into(),
                severity: 2,
            });
        }

        let max_b = lap.max_brake_temp.iter().cloned().fold(f32::MIN, f32::max);
        if max_b > 750.0 {
            let diff = max_b - 750.0;
            advices.push(Advice {
                zone: "Brakes".into(),
                problem: format!("Overheating: {:.0}°C (+{:.0})", max_b, diff),
                solution: "Open ducts or increase ABS.".into(),
                severity: 3,
            });
        }

        if lap.lockup_count > 0 {
            advices.push(Advice {
                zone: "Lockup".into(),
                problem: format!("{} Lockups detected", lap.lockup_count),
                solution: "Reduce peak pressure or bias rear.".into(),
                severity: 3,
            });
        }

        let front_fast_bump = (lap.damper_histograms[0][1] + lap.damper_histograms[1][1]) / 2.0;
        if front_fast_bump > 35.0 {
            advices.push(Advice {
                zone: "Suspension".into(),
                problem: format!("High Front Fast Bump ({:.0}%)", front_fast_bump),
                solution: "Suspension bottoming out over kerbs. Stiffen front Fast Bump.".into(),
                severity: 2,
            });
        }

        StandaloneAnalysis {
            is_perfect: advices.is_empty(),
            advices,
        }
    }

    pub fn predictive_lap_time_ms(
        &self,
        current_i_lap_time: i32,
        current_normalized_pos: f32,
    ) -> Option<i32> {
        if current_normalized_pos > 0.05
            && current_normalized_pos < 0.99
            && current_i_lap_time > 1000
        {
            let estimated = (current_i_lap_time as f32 / current_normalized_pos) as i32;
            Some(estimated)
        } else {
            None
        }
    }

    /// Fastest time set in each sector across every valid lap.
    ///
    /// `None` for a sector nothing usable has been recorded in — a lap whose
    /// split was never captured, or a slot a two-sector track never fills.
    /// That distinction is the point: a caller taking a plain minimum over the
    /// raw values picks up those zeroes and reports a best sector of 0.000.
    ///
    /// `> MIN_VALID_SECTOR_MS`, matching `process_lap`, so the two agree on
    /// what counts as a sector.
    pub fn best_sectors_ms(&self) -> [Option<i32>; 3] {
        let mut best = [None; 3];

        for lap in self.laps.iter().filter(|l| l.valid) {
            for (slot, sector) in best.iter_mut().zip(lap.sectors.iter()) {
                if *sector > MIN_VALID_SECTOR_MS && slot.is_none_or(|current| *sector < current) {
                    *slot = Some(*sector);
                }
            }
        }

        best
    }

    /// The lap that would result from stringing every best sector together.
    ///
    /// `None` until every sector has been set at least once, because a sum
    /// missing a term is not a lap time — it is a smaller number that looks
    /// like one.
    pub fn theoretical_best_lap_ms(&self) -> Option<i32> {
        let best = self.best_sectors_ms();
        if best.iter().any(Option::is_none) {
            return None;
        }
        Some(best.iter().flatten().sum())
    }
}

pub fn export_lap_to_csv(
    lap: &LapData,
    path: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // RPM, both G axes and average slip were dropped on the way out, even
    // though TelemetryPoint carries all four — so an exported lap could not
    // be used to look at engine usage, the friction circle or wheelspin, the
    // three things an external analysis tool is most often opened for.
    let mut content = String::with_capacity(lap.telemetry_trace.len() * 128);
    content.push_str(
        "\"Time\",\"Distance\",\"Speed\",\"RPM\",\"Steer\",\"Gas\",\"Brake\",\"Gear\",\
         \"Lat_G\",\"Lon_G\",\"Slip\",\"Pos_X\",\"Pos_Y\"\n",
    );
    content.push_str(
        "\"s\",\"fraction\",\"km/h\",\"rpm\",\"rad\",\"%\",\"%\",\"\",\
         \"g\",\"g\",\"\",\"m\",\"m\"\n",
    );

    for p in &lap.telemetry_trace {
        let time_sec = p.time_ms as f32 / 1000.0;
        let line = format!(
            "{:.3},{:.5},{:.1},{},{:.3},{:.2},{:.2},{},{:.3},{:.3},{:.3},{:.2},{:.2}\n",
            time_sec,
            p.distance,
            p.speed,
            p.rpms,
            p.steer,
            p.gas * 100.0,
            p.brake * 100.0,
            p.gear,
            p.lat_g,
            p.lon_g,
            p.slip_avg,
            p.x,
            p.y
        );
        content.push_str(&line);
    }

    crate::atomic_file::write_atomic(path, content.as_bytes())?;
    Ok(path.to_path_buf())
}

pub fn calculate_ghost_delta(
    best_lap: &LapData,
    progress: f32,
    current_lap_time_sec: f32,
) -> Option<f32> {
    if best_lap.telemetry_trace.is_empty() {
        return None;
    }

    let target_dist = progress * best_lap.telemetry_trace.last()?.distance;
    let best_point = best_lap.telemetry_trace.iter().min_by(|a, b| {
        (a.distance - target_dist)
            .abs()
            .partial_cmp(&(b.distance - target_dist).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    let best_time_sec = best_point.time_ms as f32 / 1000.0;
    Some(current_lap_time_sec - best_time_sec)
}

#[cfg(test)]
mod tests {
    use super::{
        Episodes, LapData, TelemetryAnalyzer, TelemetryPoint, TelemetryTrace, samples_per_incident,
        without_sentinel,
    };

    fn point(distance: f32, time_ms: i32, speed: f32) -> TelemetryPoint {
        TelemetryPoint {
            distance,
            time_ms,
            speed,
            gas: 0.0,
            brake: 0.0,
            gear: 3,
            steer: 0.0,
            lat_g: 0.0,
            lon_g: 0.0,
            slip_avg: 0.0,
            x: 0.0,
            y: 0.0,
            rpms: 6000,
            detail: Default::default(),
        }
    }
    use crate::games::{Car, Session};

    #[test]
    fn pressure_metrics_only_use_high_speed_samples() {
        let mut analyzer = TelemetryAnalyzer::new();
        let high_speed = Car {
            speed_kmh: 120.0,
            tyre_pressure_psi: [28.0; 4],
            ..Default::default()
        };
        let low_speed = Car {
            speed_kmh: 30.0,
            tyre_pressure_psi: [20.0; 4],
            ..Default::default()
        };

        analyzer.process_lap(
            1,
            90_000,
            &[high_speed, low_speed],
            &[Session::default()],
            [0, 0, 0],
            "test_car".to_string(),
            "test_track".to_string(),
            27.5,
            16,
        );

        let lap = analyzer.laps.last().expect("lap should be recorded");
        let avg = lap.avg_pressure.expect("a high-speed sample was measured");
        let dev = lap
            .pressure_deviation
            .expect("a high-speed sample was measured");
        assert!((avg - 28.0).abs() < f32::EPSILON);
        assert!((dev - 0.5).abs() < f32::EPSILON);
    }

    /// A lap that never got above the speed gate has nothing to measure. It
    /// used to report 0.0 psi and, because the deviation was also 0.0, a
    /// *perfect* tyre-management score — so an out-lap rated better than a
    /// hot lap.
    #[test]
    fn a_lap_with_no_high_speed_samples_reports_no_pressure() {
        let mut analyzer = TelemetryAnalyzer::new();
        let crawling = Car {
            speed_kmh: 30.0,
            tyre_pressure_psi: [20.0; 4],
            ..Default::default()
        };

        analyzer.process_lap(
            1,
            90_000,
            &[crawling, crawling],
            &[Session::default()],
            [0, 0, 0],
            "test_car".to_string(),
            "test_track".to_string(),
            27.5,
            16,
        );

        let lap = analyzer.laps.last().expect("lap should be recorded");
        assert_eq!(lap.avg_pressure, None);
        assert_eq!(lap.pressure_deviation, None);
        assert!(
            (lap.radar_stats.tyre_mgmt - 0.5).abs() < f32::EPSILON,
            "unmeasured should sit at neutral, not perfect: {}",
            lap.radar_stats.tyre_mgmt
        );
    }

    /// Mistake counts must mean the same thing whatever update rate the user
    /// picked, or laps recorded at different rates cannot be compared.
    /// The bug this replaced: a condition that holds counts once, not once
    /// per sample. Half a lap of sliding at 60 Hz used to arrive as hundreds
    /// of separate incidents, which is how one continuous slide was reported
    /// as "Oversteer: 1454x".
    #[test]
    fn one_long_slide_is_one_episode() {
        let mut episodes = Episodes::default();
        for _ in 0..5000 {
            episodes.sample(true, 5);
        }
        assert_eq!(episodes.count, 1);
    }

    /// Two slides with the car settled in between are two.
    #[test]
    fn a_second_episode_needs_the_condition_to_clear_first() {
        let mut episodes = Episodes::default();
        for _ in 0..50 {
            episodes.sample(true, 5);
        }
        for _ in 0..50 {
            episodes.sample(false, 5);
        }
        for _ in 0..50 {
            episodes.sample(true, 5);
        }
        assert_eq!(episodes.count, 2);
    }

    /// Resampling by distance is what makes a ghost comparison mean anything —
    /// two laps lined up metre by metre rather than second by second — and it
    /// had no test at all. `cargo mutants` found six separate edits to it that
    /// nothing noticed, including turning `-` into `+` in the interpolation.
    #[test]
    fn resampling_interpolates_between_the_points_it_was_given() {
        let trace = vec![
            point(0.0, 0, 100.0),
            point(0.5, 1000, 200.0),
            point(1.0, 2000, 100.0),
        ];
        let out = TelemetryTrace::resample_by_distance(&trace, 0.25);

        assert!(
            out.len() >= 4,
            "a quarter-lap step over a whole lap: {out:?}"
        );
        // Every sample sits on the step it was asked for, in order.
        for (i, p) in out.iter().enumerate() {
            assert!(
                (p.distance - i as f32 * 0.25).abs() < 1e-3,
                "sample {i} landed at {}",
                p.distance
            );
        }
        // Half way between the first two points is half way between their
        // speeds. This is the arithmetic the mutants rewrote unnoticed.
        let quarter = &out[1];
        assert!(
            (quarter.speed - 150.0).abs() < 0.5,
            "150 km/h half way from 100 to 200, got {}",
            quarter.speed
        );
        let half = &out[2];
        assert!(
            (half.speed - 200.0).abs() < 0.5,
            "the sample at the point itself is the point, got {}",
            half.speed
        );
    }

    /// The degenerate inputs, which a lap that never published a distance
    /// reaches: nothing to resample, and nothing to divide by.
    #[test]
    fn resampling_refuses_to_invent_a_trace() {
        assert!(TelemetryTrace::resample_by_distance(&[], 0.1).is_empty());
        let one = vec![point(0.3, 10, 90.0)];
        assert_eq!(
            TelemetryTrace::resample_by_distance(&one, 0.1).len(),
            1,
            "one point cannot be interpolated between"
        );
        assert_eq!(
            TelemetryTrace::resample_by_distance(&one, 0.0).len(),
            1,
            "a step of zero would never advance"
        );
    }

    /// A lap that published no coordinates leaves the bounds scan holding its
    /// seeds, and those are finite — so only the sentinel value itself says
    /// so. Both branches, because `cargo mutants` deleted the `!` and nothing
    /// failed.
    #[test]
    fn the_sentinel_bounds_become_zero_and_real_ones_survive() {
        assert_eq!(without_sentinel(f32::MAX), 0.0);
        assert_eq!(without_sentinel(f32::MIN), 0.0);
        assert_eq!(without_sentinel(f32::NAN), 0.0);
        assert_eq!(without_sentinel(f32::INFINITY), 0.0);
        assert_eq!(without_sentinel(-12.5), -12.5, "a real bound is kept");
        assert_eq!(without_sentinel(0.0), 0.0);
    }

    /// Exactly the run length is enough, and one sample short is not. Both
    /// halves matter: `cargo mutants` turned the `<` in `sample` into `<=`
    /// and every test above still passed, because none of them sat on the
    /// boundary. An episode that needs one extra sample is a different
    /// threshold than the one the comment claims.
    #[test]
    fn the_run_length_is_exact() {
        let mut just_short = Episodes::default();
        for _ in 0..4 {
            just_short.sample(true, 5);
        }
        assert_eq!(just_short.count, 0, "four samples is not five");

        let mut exactly = Episodes::default();
        for _ in 0..5 {
            exactly.sample(true, 5);
        }
        assert_eq!(exactly.count, 1, "five samples is five");
    }

    /// A single sample over the line is noise, not an episode — the run
    /// length is what tells them apart.
    #[test]
    fn a_flicker_shorter_than_the_run_length_is_not_an_episode() {
        let mut episodes = Episodes::default();
        for _ in 0..100 {
            episodes.sample(true, 5);
            episodes.sample(false, 5);
        }
        assert_eq!(episodes.count, 0);
    }

    #[test]
    fn samples_per_incident_holds_a_fixed_duration() {
        // 16 ms is the default and is close enough to 60 Hz that the run
        // length is unchanged.
        assert_eq!(samples_per_incident(5, 16), 5);
        // Half the rate, half as many samples for the same duration.
        assert_eq!(samples_per_incident(5, 33), 3);
        // Double the rate, twice as many.
        assert_eq!(samples_per_incident(5, 8), 10);
        // A very slow rate rounds below one sample per incident. This is a
        // divisor, so it floors at 1 rather than dividing by zero.
        assert_eq!(samples_per_incident(5, 1000), 1);
        // Zero cannot reach here — AppConfig::validate clamps update_rate to
        // 5..=1000 — but it must not divide by zero if it ever did.
        assert!(samples_per_incident(5, 0) >= 1);
    }

    /// A lap where AC published no coordinates leaves the bounds scan holding
    /// its f32::MAX/f32::MIN seeds, and those used to be serialised into the
    /// saved lap. Anything computing `max - min` off the file got -6.8e38.
    #[test]
    fn a_lap_with_no_coordinates_reports_zero_bounds() {
        let mut analyzer = TelemetryAnalyzer::new();
        let sample = Car {
            speed_kmh: 120.0,
            ..Default::default()
        };
        // Default graphics means car_coordinates is all zeroes, which the
        // scan skips as not-a-position.
        analyzer.process_lap(
            1,
            90_000,
            &[sample; 10],
            &[Session::default(); 10],
            [0, 0, 0],
            "test_car".to_string(),
            "test_track".to_string(),
            27.5,
            16,
        );

        let lap = analyzer.laps.last().expect("lap should be recorded");
        assert_eq!(lap.bounds_min_x, 0.0);
        assert_eq!(lap.bounds_max_x, 0.0);
        assert_eq!(lap.bounds_min_y, 0.0);
        assert_eq!(lap.bounds_max_y, 0.0);
    }

    /// A lap whose split was never captured leaves a zero in the array, and
    /// a two-sector track never fills the third slot at all. A plain minimum
    /// over the raw values picks those up and reports a best sector of 0.000,
    /// which is what the analysis tab used to show.
    #[test]
    fn best_sectors_ignore_uncaptured_splits() {
        let mut analyzer = TelemetryAnalyzer::new();
        analyzer.laps.push(LapData {
            valid: true,
            sectors: [30_000, 35_000, 30_500],
            ..Default::default()
        });
        // Second lap: S2 was never captured.
        analyzer.laps.push(LapData {
            valid: true,
            sectors: [29_000, 0, 31_000],
            ..Default::default()
        });

        let best = analyzer.best_sectors_ms();
        assert_eq!(best[0], Some(29_000), "the faster S1 of the two");
        assert_eq!(
            best[1],
            Some(35_000),
            "the zero is not a sector time, so S2 stays at the only real one"
        );
        assert_eq!(best[2], Some(30_500));

        assert_eq!(analyzer.theoretical_best_lap_ms(), Some(94_500));
    }

    /// A sum missing a term is not a lap time, it is a smaller number that
    /// looks like one.
    #[test]
    fn theoretical_best_needs_every_sector() {
        let mut analyzer = TelemetryAnalyzer::new();
        analyzer.laps.push(LapData {
            valid: true,
            sectors: [30_000, 35_000, 0],
            ..Default::default()
        });

        assert_eq!(analyzer.best_sectors_ms()[2], None);
        assert_eq!(analyzer.theoretical_best_lap_ms(), None);
    }

    /// **And an invalid lap is not the best lap either.**
    ///
    /// The sectors filtered on validity from the start; the best *lap* did
    /// not, so on a game that reports track limits a cut lap became the
    /// reference every later lap was compared against. It is also what the
    /// consistency score measures spread around, and what the analysis screen
    /// draws a ghost from.
    #[test]
    fn the_best_lap_skips_a_lap_the_game_invalidated() {
        let mut analyzer = TelemetryAnalyzer::new();
        let car = Car {
            speed_kmh: 120.0,
            ..Default::default()
        };
        let session = |valid| Session {
            lap_is_valid: valid,
            ..Default::default()
        };
        let drive = |analyzer: &mut TelemetryAnalyzer, number, time_ms, valid| {
            analyzer.process_lap(
                number,
                time_ms,
                &[car],
                &[session(valid)],
                [0, 0, 0],
                "test_car".to_string(),
                "test_track".to_string(),
                27.5,
                16,
            );
        };

        drive(&mut analyzer, 1, 92_000, true);
        assert_eq!(analyzer.best_lap_index, Some(0));

        // Quicker, and it never counted.
        drive(&mut analyzer, 2, 89_000, false);
        assert_eq!(
            analyzer.best_lap_index,
            Some(0),
            "a lap the game called invalid became the reference"
        );

        // Quicker and clean: this one is the best.
        drive(&mut analyzer, 3, 90_500, true);
        assert_eq!(analyzer.best_lap_index, Some(2));
    }

    /// A driver who goes back to the menu and picks another car does not
    /// restart the program, and everything here belonged to the car they left.
    #[test]
    fn a_new_car_starts_from_nothing() {
        let mut analyzer = TelemetryAnalyzer::new();
        analyzer.set_track_length(7_004.0);
        analyzer.process_lap(
            1,
            92_000,
            &[Car {
                speed_kmh: 120.0,
                ..Default::default()
            }],
            &[Session::default()],
            [30_000, 31_000, 31_000],
            "gt3".to_string(),
            "spa".to_string(),
            27.5,
            16,
        );
        assert_eq!(analyzer.best_lap_index, Some(0));
        assert_eq!(analyzer.best_sectors_ms()[0], Some(30_000));

        analyzer.start_new_session();

        assert!(analyzer.laps.is_empty());
        assert_eq!(analyzer.best_lap_index, None);
        assert_eq!(analyzer.best_sectors_ms()[0], None);
        assert!(analyzer.reference_lap.is_none());
        assert!(analyzer.world_record.is_none());
        assert_eq!(
            analyzer.track_length_m, 0.0,
            "the previous circuit's length would stamp itself onto laps of the new one"
        );
        assert!(!analyzer.track_length_measured());
    }

    /// Assetto Corsa never reports validity, so every lap there is valid and
    /// the rule above is invisible on it. Worth a test of its own: gating on a
    /// field a game does not fill is how a working feature goes silent.
    #[test]
    fn a_game_that_never_reports_validity_still_gets_a_best_lap() {
        let mut analyzer = TelemetryAnalyzer::new();
        analyzer.process_lap(
            1,
            92_000,
            &[Car {
                speed_kmh: 120.0,
                ..Default::default()
            }],
            // `Session::default()` is what a game that says nothing produces.
            &[Session::default()],
            [0, 0, 0],
            "test_car".to_string(),
            "test_track".to_string(),
            27.5,
            16,
        );
        assert_eq!(analyzer.best_lap_index, Some(0));
    }

    /// Invalid laps do not contribute a best sector.
    #[test]
    fn best_sectors_skip_invalid_laps() {
        let mut analyzer = TelemetryAnalyzer::new();
        analyzer.laps.push(LapData {
            valid: true,
            sectors: [30_000, 35_000, 30_500],
            ..Default::default()
        });
        analyzer.laps.push(LapData {
            valid: false,
            sectors: [1_000_000, 2_000, 2_000],
            ..Default::default()
        });

        assert_eq!(analyzer.best_sectors_ms()[1], Some(35_000));
    }

    #[test]
    fn test_resample_empty_or_partial_trace() {
        use super::*;
        let empty: Vec<TelemetryPoint> = vec![];
        let resampled_empty = TelemetryTrace::resample_by_distance(&empty, 0.1);
        assert!(resampled_empty.is_empty());

        let single = vec![TelemetryPoint {
            distance: 0.5,
            time_ms: 1000,
            speed: 100.0,
            rpms: 0,
            gas: 1.0,
            brake: 0.0,
            gear: 3,
            steer: 0.0,
            lat_g: 0.0,
            lon_g: 0.0,
            slip_avg: 0.0,
            x: 0.0,
            y: 0.0,
            detail: Default::default(),
        }];
        let resampled_single = TelemetryTrace::resample_by_distance(&single, 0.1);
        assert_eq!(resampled_single.len(), 1);
    }

    #[test]
    fn test_identical_laps_different_sampling_rates_produce_zero_delta() {
        use super::*;
        // Lap A: 10 samples (10 Hz)
        let lap_a: Vec<TelemetryPoint> = (0..=10)
            .map(|i| {
                let dist = i as f32 / 10.0;
                let time_ms = (dist * 60_000.0) as i32; // 60s lap
                TelemetryPoint {
                    distance: dist,
                    time_ms,
                    speed: 150.0,
                    rpms: 0,
                    gas: 1.0,
                    brake: 0.0,
                    gear: 4,
                    steer: 0.0,
                    lat_g: 0.0,
                    lon_g: 0.0,
                    slip_avg: 0.0,
                    x: 0.0,
                    y: 0.0,
                    detail: Default::default(),
                }
            })
            .collect();

        // Lap B: 100 samples (100 Hz) for exact same speed profile
        let lap_b: Vec<TelemetryPoint> = (0..=100)
            .map(|i| {
                let dist = i as f32 / 100.0;
                let time_ms = (dist * 60_000.0) as i32; // 60s lap
                TelemetryPoint {
                    distance: dist,
                    time_ms,
                    speed: 150.0,
                    rpms: 0,
                    gas: 1.0,
                    brake: 0.0,
                    gear: 4,
                    steer: 0.0,
                    lat_g: 0.0,
                    lon_g: 0.0,
                    slip_avg: 0.0,
                    x: 0.0,
                    y: 0.0,
                    detail: Default::default(),
                }
            })
            .collect();

        let delta = LapComparison::delta_over_time(&lap_a, &lap_b);
        assert!(!delta.is_empty());
        for (_time, dt) in delta {
            assert!(
                dt.abs() < 0.05,
                "Delta should be ~0.0s for identical performance, got {}",
                dt
            );
        }
    }

    #[test]
    fn test_lap_comparison_slower_lap_has_positive_delta() {
        use super::*;
        // Lap Fast: 60s
        let lap_fast: Vec<TelemetryPoint> = (0..=10)
            .map(|i| {
                let dist = i as f32 / 10.0;
                TelemetryPoint {
                    distance: dist,
                    time_ms: (dist * 60_000.0) as i32,
                    speed: 200.0,
                    rpms: 0,
                    gas: 1.0,
                    brake: 0.0,
                    gear: 5,
                    steer: 0.0,
                    lat_g: 0.0,
                    lon_g: 0.0,
                    slip_avg: 0.0,
                    x: 0.0,
                    y: 0.0,
                    detail: Default::default(),
                }
            })
            .collect();

        // Lap Slow: 65s (5s slower)
        let lap_slow: Vec<TelemetryPoint> = (0..=10)
            .map(|i| {
                let dist = i as f32 / 10.0;
                TelemetryPoint {
                    distance: dist,
                    time_ms: (dist * 65_000.0) as i32,
                    speed: 185.0,
                    rpms: 0,
                    gas: 0.9,
                    brake: 0.0,
                    gear: 5,
                    steer: 0.0,
                    lat_g: 0.0,
                    lon_g: 0.0,
                    slip_avg: 0.0,
                    x: 0.0,
                    y: 0.0,
                    detail: Default::default(),
                }
            })
            .collect();

        let delta = LapComparison::delta_over_time(&lap_slow, &lap_fast);
        let final_delta = delta.last().expect("should have points").1;
        assert!(
            (final_delta - 5.0).abs() < 0.2,
            "Expected ~+5.0s delta at finish, got {}",
            final_delta
        );
    }
}

#[cfg(test)]
mod measured_track_length_tests {
    use super::TelemetryAnalyzer;

    /// One lap of a circuit, sampled the way a game publishes it.
    fn lap(analyzer: &mut TelemetryAnalyzer, from_m: f32, length_m: f32) -> f32 {
        let mut travelled = from_m;
        for step in 0..100 {
            travelled += length_m / 100.0;
            analyzer.observe_distance(step as f32 / 100.0, travelled);
        }
        travelled
    }

    /// Competizione publishes no track length, so everything reported in metres
    /// is withheld on it. Two crossings of the line say how long the circuit is.
    #[test]
    fn a_game_that_publishes_nothing_gets_a_measured_length() {
        let mut analyzer = TelemetryAnalyzer::new();
        analyzer.set_track_length(0.0); // what ACC reports

        // The line is crossed at the *start* of a lap, so the first lap of a
        // session ends with one crossing recorded and nothing measured — a
        // session joined halfway down the straight has no length to report.
        let after_one = lap(&mut analyzer, 0.0, 7_004.0);
        let after_two = lap(&mut analyzer, after_one, 7_004.0);
        assert_eq!(
            analyzer.track_length_m, 0.0,
            "one crossing measures nothing, and a guess would be worse than none"
        );

        lap(&mut analyzer, after_two, 7_004.0);
        assert!(
            (analyzer.track_length_m - 7_004.0).abs() < 80.0,
            "measured {} m",
            analyzer.track_length_m
        );
        assert!(
            analyzer.track_length_measured(),
            "a front end has to be able to say where the number came from"
        );
    }

    /// Assetto Corsa's own spline length is exact. A measurement taken from the
    /// car must never replace it — and the mark has to say so, or a screen
    /// would report the game's own figure as something this program worked out.
    #[test]
    fn a_published_length_is_never_replaced_by_a_measured_one() {
        let mut analyzer = TelemetryAnalyzer::new();
        analyzer.set_track_length(7_004.0);

        let after_one = lap(&mut analyzer, 0.0, 6_800.0);
        let after_two = lap(&mut analyzer, after_one, 6_800.0);
        lap(&mut analyzer, after_two, 6_800.0);

        assert_eq!(analyzer.track_length_m, 7_004.0);
        assert!(!analyzer.track_length_measured());
    }

    /// The session recognises the track a few frames in, which on Assetto Corsa
    /// arrives after the car has already been driving. A length that was
    /// measured first has to step aside for the published one.
    #[test]
    fn a_published_length_arriving_late_takes_over() {
        let mut analyzer = TelemetryAnalyzer::new();
        let after_one = lap(&mut analyzer, 0.0, 6_800.0);
        let after_two = lap(&mut analyzer, after_one, 6_800.0);
        lap(&mut analyzer, after_two, 6_800.0);
        assert!(analyzer.track_length_measured());

        analyzer.set_track_length(7_004.0);
        assert_eq!(analyzer.track_length_m, 7_004.0);
        assert!(!analyzer.track_length_measured());
    }
}
