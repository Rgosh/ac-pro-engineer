use crate::config::{AppConfig, Language};
use crate::games::{Capabilities, Car, Session};
use crate::i18n::{Translate, tr_fmt};
use crate::session_info::SessionInfo;
use crate::setup_manager::CarSetup;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, info};

#[derive(Debug, Serialize, Clone, Default)]
pub struct Recommendation {
    pub component: String,
    pub category: String,
    pub severity: Severity,
    pub message: String,
    pub action: String,
    pub parameters: Vec<Parameter>,
    /// The old hand-picked certainty, 0..1, still used for ordering.
    ///
    /// Superseded by [`Chain::evidence`] wherever a rule can count what it
    /// saw — see [`Recommendation::confidence_level`], which prefers the
    /// evidence and falls back to this.
    pub confidence: f32,
    /// Why this is happening, and how to know whether the fix worked.
    ///
    /// `Some` on every rule with a mechanism to state, which since v0.3.7 is
    /// all of them but the two fuel rules — see `analyze_strategy`, where the
    /// `None` is the finding rather than an omission.
    ///
    /// A chain whose [`Chain::evidence`] is *empty* is a third, deliberate
    /// state: the rule can explain itself but counts one whole-lap number
    /// rather than several corroborating observations, so
    /// [`Recommendation::confidence_level`] falls back to the hand-picked
    /// `confidence` beside it. One counter is one observation however large it
    /// gets.
    pub chain: Option<Chain>,
}

/// Evidence, mechanism, and a check for next time.
///
/// A flat finding says "front-right 96 °C". This says what produced it, what it
/// did, and — the field that matters most — **what to look at on the next run
/// to know whether the change worked**. That last one is what makes the advice
/// an engineer's rather than a paragraph: it commits the advice to being
/// checkable, and an unfalsifiable suggestion is not advice.
///
/// It also means the analysis stops being a list of independent checks. "FR
/// outer shoulder is hot" and "the car understeers in T4–T6" are two unrelated
/// findings until something states the link, and the value is entirely in the
/// link.
#[derive(Debug, Serialize, Clone, Default)]
pub struct Chain {
    /// The mechanism: "high lateral load through T4–T6".
    pub cause: String,
    /// The measurement it produced: "FR outer shoulder +11 °C".
    pub effect: String,
    /// What to look at next time: "FR I/M/O spread on the next lap".
    pub confirm: String,
    /// What was actually observed, and how much it agreed with itself.
    pub evidence: crate::confidence::Evidence,
}

impl Recommendation {
    /// How sure this advice is.
    ///
    /// Evidence wins where a rule counted what it saw; the hand-picked score is
    /// the fallback for the rules that did not, and is mapped onto the same
    /// scale rather than pretending to be evidence.
    pub fn confidence_level(&self) -> crate::confidence::Confidence {
        match self.chain.as_ref() {
            Some(chain) if !chain.evidence.is_empty() => chain.evidence.confidence(),
            _ => crate::confidence::Confidence::from_score(self.confidence),
        }
    }
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct Parameter {
    pub name: String,
    pub current: f32,
    pub target: f32,
    pub unit: String,
}

/// The four corners, in the order every array in AC's physics page uses.
///
/// Was written out as a `match i { 0 => "FL", ... }` in six analysers, which
/// is six places to get the order wrong.
pub const CORNER_NAMES: [&str; 4] = ["FL", "FR", "RL", "RR"];

#[derive(Debug, Serialize, Clone, PartialEq, PartialOrd, Default)]
pub enum Severity {
    #[default]
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct DrivingStyle {
    pub smoothness: f32,
    pub aggression: f32,
    pub consistency: f32,
    pub trail_braking: f32,
    pub throttle_control: f32,
    pub prev_gas: f32,
    pub prev_brake: f32,
    pub prev_steer: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WizardPhase {
    Entry,
    Apex,
    Exit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WizardProblem {
    Understeer,
    Oversteer,
    Instability,
}

/// How many recent laps the measured fuel average runs over. Short enough to
/// track a changing pace, long enough that one cautious lap does not skew it.
const FUEL_HISTORY_LAPS: usize = 3;

pub struct Engineer {
    config: AppConfig,
    /// What the game feeding this engineer can actually measure.
    ///
    /// Starts at nothing measured, and every rule that rests on a measurement
    /// checks here before it says anything. A game is expected to announce
    /// itself through [`update_capabilities`](Engineer::update_capabilities)
    /// on the way in; until it does, the engineer withholds rather than reads
    /// a default as a reading.
    capabilities: Capabilities,
    history_size: usize,
    pub stats: EngineerStats,
    pub driving_style: DrivingStyle,
    pub wizard_phase: WizardPhase,
    pub wizard_problem: WizardProblem,
    alert_timers: HashMap<String, (Instant, Instant)>,
}

#[derive(Debug, Clone)]
pub struct EngineerStats {
    pub bottoming_frames: [u32; 4],
    pub lockup_frames_front: u32,
    pub lockup_frames_rear: u32,
    pub wheel_spin_frames: u32,
    pub traction_loss_frames: u32,
    pub oversteer_frames: u32,
    pub understeer_frames: u32,
    pub coasting_frames: u32,
    pub scrubbing_frames: u32,
    pub current_excess_steer: f32,
    pub total_frames: u32,
    pub ffb_clip_frames: u32,
    pub input_history: crate::RingBuffer<(f64, f64, f64, f64, f64)>,
    pub fuel_laps_remaining: f32,
    pub fuel_consumption_rate: f32,
    /// Fuel used on each of the last few completed laps, newest last.
    ///
    /// AC's own `fuel_x_lap` sits in the part of the graphics page that is
    /// not confirmed against a live capture (see the note in `games/assetto_corsa/structs.rs`),
    /// and it reads zero on lap one regardless. Everything on the strategy
    /// tab was gated on it being positive, so the tab showed "NO DATA" for
    /// the whole first lap and, if that offset is wrong, forever.
    pub recent_fuel_per_lap: Vec<f32>,
    /// Fuel level at the start of the current lap, to measure against.
    pub fuel_at_lap_start: f32,
    pub current_delta: f32,
    pub predicted_lap_time: f32,
    pub low_speed_rake: f32,
    pub high_speed_rake: f32,
    /// Inner-minus-outer surface temperature per corner, averaged over the
    /// frames where the tyre was actually loaded sideways.
    ///
    /// A tyre says nothing about camber on a straight. Upright, both edges run
    /// at the same temperature and the spread reads about zero — which the
    /// first version of `analyze_camber` read as "contact patch inefficient"
    /// and published on every straight, four corners at a time, filling the
    /// panel's eight advice lines with one wrong answer. Camber only shows in
    /// the spread while the tyre is loaded, so the sample is taken while the
    /// car is cornering and the verdict comes from the average.
    ///
    /// All four corners are sampled rather than the loaded pair: left-handers
    /// load the right tyres and right-handers the left, so over a lap of a
    /// circuit both sides get their turn. An oval would skew it.
    pub camber_spread: [f32; 4],
    /// Ticks of cornering behind `camber_spread`. Below `CAMBER_MIN_FRAMES`
    /// the average is one corner's worth of noise and nothing is published.
    pub camber_frames: u32,
    pub base_tyre_wear: [f32; 4],
    pub stint_laps: i32,
    pub last_lap_count: i32,
    pub tyre_laps_remaining: [f32; 4],
}

impl Default for EngineerStats {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineerStats {
    pub fn new() -> Self {
        Self {
            bottoming_frames: [0; 4],
            lockup_frames_front: 0,
            lockup_frames_rear: 0,
            wheel_spin_frames: 0,
            traction_loss_frames: 0,
            oversteer_frames: 0,
            understeer_frames: 0,
            coasting_frames: 0,
            scrubbing_frames: 0,
            current_excess_steer: 0.0,
            total_frames: 0,
            ffb_clip_frames: 0,
            input_history: crate::RingBuffer::new(300),
            fuel_laps_remaining: 0.0,
            fuel_consumption_rate: 0.0,
            recent_fuel_per_lap: Vec::new(),
            fuel_at_lap_start: 0.0,
            current_delta: 0.0,
            predicted_lap_time: 0.0,
            low_speed_rake: 0.0,
            high_speed_rake: 0.0,
            camber_spread: [0.0; 4],
            camber_frames: 0,
            base_tyre_wear: [100.0; 4],
            stint_laps: 0,
            last_lap_count: -1,
            // Negative means "not measured yet", and it has to be a value
            // laps remaining can never take — this was 99.0, which is also a
            // perfectly good answer, so a tyre with ninety-nine laps in it read
            // as no data.
            tyre_laps_remaining: [-1.0; 4],
        }
    }
}

impl Default for DrivingStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl DrivingStyle {
    pub fn new() -> Self {
        Self {
            smoothness: 50.0,
            aggression: 50.0,
            consistency: 50.0,
            trail_braking: 0.0,
            throttle_control: 50.0,
            prev_gas: 0.0,
            prev_brake: 0.0,
            prev_steer: 0.0,
        }
    }
}

impl Engineer {
    pub fn new(config: &AppConfig) -> Self {
        info!("Engineer module initialized.");
        Self {
            config: config.clone(),
            capabilities: Capabilities::default(),
            history_size: 600,
            stats: EngineerStats::new(),
            driving_style: DrivingStyle::new(),
            wizard_phase: WizardPhase::Entry,
            wizard_problem: WizardProblem::Understeer,
            alert_timers: HashMap::new(),
        }
    }

    pub fn update_config(&mut self, config: &AppConfig) {
        self.config = config.clone();
        self.stats.input_history.set_capacity(config.history_size);
    }

    /// Tell the engineer what the game it is reading can measure.
    ///
    /// Carried on every [`Reading`](crate::games::Reading), and passed on here
    /// once a tick beside the config. Nothing else needs to change when a
    /// second game arrives with a different answer.
    pub fn update_capabilities(&mut self, capabilities: Capabilities) {
        self.capabilities = capabilities;
    }

    /// What the engineer is currently willing to speak about.
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub fn update(&mut self, car: &Car, session: &Session, _info: &SessionInfo) {
        self.update_stats(car, session);
        self.analyze_driving_style(car);

        if self.stats.total_frames > self.history_size as u32 {
            debug!("Engineer history buffer reached limit, resetting counters.");
            self.reset_counters();
        }
    }

    fn update_stats(&mut self, car: &Car, session: &Session) {
        let dt_sec = (self.config.update_rate as f32 / 1000.0).clamp(0.001, 1.0);
        let ticks_norm = (dt_sec * 60.0).round().max(1.0) as u32;

        self.stats.total_frames += ticks_norm;

        if car.force_feedback.abs() > 0.98 {
            self.stats.ffb_clip_frames += ticks_norm;
        }

        if self.stats.total_frames.is_multiple_of(3) {
            let t = self.stats.total_frames as f64;
            self.stats.input_history.push((
                t,
                car.steer_angle as f64,
                car.throttle as f64,
                car.brake as f64,
                car.force_feedback as f64,
            ));
        }

        for i in 0..4 {
            if car.suspension_travel[i] < 0.005 {
                self.stats.bottoming_frames[i] += ticks_norm;
            }
        }

        let rake = car.ride_height_m[1] - car.ride_height_m[0];
        let rake_mm = rake * 1000.0;
        if car.speed_kmh > 50.0 && car.speed_kmh < 90.0 {
            if self.stats.low_speed_rake == 0.0 {
                self.stats.low_speed_rake = rake_mm;
            }
            self.stats.low_speed_rake = self.stats.low_speed_rake * 0.98 + rake_mm * 0.02;
        } else if car.speed_kmh > 160.0 {
            if self.stats.high_speed_rake == 0.0 {
                self.stats.high_speed_rake = rake_mm;
            }
            self.stats.high_speed_rake = self.stats.high_speed_rake * 0.98 + rake_mm * 0.02;
        }

        // acc_g is [lateral, vertical, longitudinal]. Half a g sideways is a
        // corner being driven; a lane change on a straight does not reach it,
        // and a straight is where the inner and outer edges of a correctly
        // cambered tyre read the same temperature.
        if self.capabilities.tyre_edge_temps && car.speed_kmh > 50.0 && car.acc_g[0].abs() > 0.5 {
            let first_sample = self.stats.camber_frames == 0;
            self.stats.camber_frames = self.stats.camber_frames.saturating_add(ticks_norm);
            for i in 0..4 {
                let spread = car.tyre_temp_inner_c[i] - car.tyre_temp_outer_c[i];
                if first_sample {
                    self.stats.camber_spread[i] = spread;
                } else {
                    self.stats.camber_spread[i] =
                        self.stats.camber_spread[i] * 0.98 + spread * 0.02;
                }
            }
        }

        let current_laps = session.completed_laps;
        if current_laps != self.stats.last_lap_count {
            self.record_fuel_for_completed_lap(car.fuel_litres);
            if self.stats.last_lap_count == -1 || current_laps == 0 || car.speed_kmh < 10.0 {
                self.stats.base_tyre_wear = car.tyre_wear;
                self.stats.stint_laps = 0;
            } else if self.capabilities.tyre_wear {
                self.stats.stint_laps += 1;
                for i in 0..4 {
                    let wear_used = self.stats.base_tyre_wear[i] - car.tyre_wear[i];
                    if wear_used > 0.0 && self.stats.stint_laps > 0 {
                        let wear_per_lap = wear_used / self.stats.stint_laps as f32;
                        // Laps until the tyre is *done*, not until it is two
                        // percent past the warning. That derivation is the same
                        // one that used to call a tyre at 93.9 % life critical.
                        let replacement_threshold = self
                            .config
                            .alerts
                            .wear_critical
                            .min(self.config.alerts.wear_warning)
                            .max(0.0);
                        let remaining_wear = car.tyre_wear[i] - replacement_threshold;
                        if wear_per_lap > 0.001 {
                            let laps = (remaining_wear / wear_per_lap).max(0.0);
                            self.stats.tyre_laps_remaining[i] =
                                if laps > 500.0 { 500.0 } else { laps };
                        }
                    }
                }
            }
            self.stats.last_lap_count = current_laps;
        }

        if car.speed_kmh > 30.0 {
            if (car.wheel_slip[0].abs() > 0.2 || car.wheel_slip[1].abs() > 0.2) && car.brake > 0.1 {
                self.stats.lockup_frames_front += ticks_norm;
            }
            if (car.wheel_slip[2].abs() > 0.2 || car.wheel_slip[3].abs() > 0.2) && car.brake > 0.1 {
                self.stats.lockup_frames_rear += ticks_norm;
            }
        }

        for i in 0..4 {
            if car.wheel_slip[i] > 0.15 && car.throttle > 0.3 && car.speed_kmh < 120.0 {
                self.stats.wheel_spin_frames += ticks_norm;
            }
        }

        if car.speed_kmh > 30.0 && car.throttle < 0.05 && car.brake < 0.05 {
            self.stats.coasting_frames += ticks_norm;
        }

        if car.speed_kmh > 40.0 {
            let front_slip = car.wheel_slip[0].max(car.wheel_slip[1]);
            let rear_slip = car.wheel_slip[2].max(car.wheel_slip[3]);

            if front_slip > 0.15 && front_slip > rear_slip + 0.05 && car.steer_angle.abs() > 0.15 {
                self.stats.understeer_frames += ticks_norm;
                self.stats.scrubbing_frames += ticks_norm;
                let excess = (car.steer_angle.abs() - 0.15) * 57.2958;
                if excess > self.stats.current_excess_steer {
                    self.stats.current_excess_steer = excess;
                }
            } else if rear_slip > 0.15 && rear_slip > front_slip + 0.05 {
                self.stats.oversteer_frames += ticks_norm;
            }
        } else if self.stats.scrubbing_frames > 0 && self.stats.scrubbing_frames < 45 {
            self.stats.scrubbing_frames = 0;
            self.stats.current_excess_steer = 0.0;
        }

        // Prefer AC's own figure when it is reporting one, and fall back to
        // what we measured ourselves otherwise. Either way the strategy tab
        // has something to work with from the second lap onward.
        let fuel_per_lap = if session.fuel_per_lap > 0.0 {
            Some(session.fuel_per_lap)
        } else {
            self.measured_fuel_per_lap()
        };
        match fuel_per_lap {
            Some(per_lap) if per_lap > 0.0 => {
                self.stats.fuel_consumption_rate = per_lap;
                self.stats.fuel_laps_remaining = car.fuel_litres / per_lap;
            }
            // Nothing to go on. Clear rather than leave the previous value
            // standing: it was never reset, so after a refuel or a session
            // change `analyze_strategy` could call BOX BOX BOX on a number
            // measured before the stop.
            _ => {
                self.stats.fuel_consumption_rate = 0.0;
                self.stats.fuel_laps_remaining = 0.0;
            }
        }

        self.stats.current_delta = car.reference_delta_s;

        if session.best_lap_ms > 0 {
            self.stats.predicted_lap_time =
                (session.best_lap_ms as f32 / 1000.0) + car.reference_delta_s;
        } else if session.last_lap_ms > 0 {
            self.stats.predicted_lap_time = session.last_lap_ms as f32 / 1000.0;
        }
    }

    fn analyze_driving_style(&mut self, car: &Car) {
        let gas_diff = (car.throttle - self.driving_style.prev_gas).abs();
        let brake_diff = (car.brake - self.driving_style.prev_brake).abs();
        let steer_diff = (car.steer_angle - self.driving_style.prev_steer).abs();

        let throttle_smoothness = (100.0 - (gas_diff * 1000.0)).clamp(0.0, 100.0);
        let brake_smoothness = (100.0 - (brake_diff * 1000.0)).clamp(0.0, 100.0);
        let steer_smoothness = (100.0 - (steer_diff * 500.0)).clamp(0.0, 100.0);

        self.driving_style.smoothness = 0.95 * self.driving_style.smoothness
            + 0.05 * (throttle_smoothness + brake_smoothness + steer_smoothness) / 3.0;

        self.driving_style.prev_gas = car.throttle;
        self.driving_style.prev_brake = car.brake;
        self.driving_style.prev_steer = car.steer_angle;

        // acc_g is [lateral, vertical, longitudinal]. This combined the
        // lateral and *vertical* axes, so it measured cornering plus the ~1 g
        // the car carries standing still, and never saw braking or
        // acceleration at all.
        let combined_g = (car.acc_g[0].powi(2) + car.acc_g[2].powi(2)).sqrt();
        self.driving_style.aggression =
            0.9 * self.driving_style.aggression + 0.1 * combined_g.min(2.5) / 2.5 * 100.0;

        if car.brake > 0.1 && car.steer_angle.abs() > 0.1 {
            self.driving_style.trail_braking =
                0.95 * self.driving_style.trail_braking + 0.05 * 100.0;
        } else {
            self.driving_style.trail_braking *= 0.98;
        }
    }

    /// Average fuel burn over the laps measured so far, if there are any.
    fn measured_fuel_per_lap(&self) -> Option<f32> {
        if self.stats.recent_fuel_per_lap.is_empty() {
            return None;
        }
        let sum: f32 = self.stats.recent_fuel_per_lap.iter().sum();
        Some(sum / self.stats.recent_fuel_per_lap.len() as f32)
    }

    /// Note how much fuel the lap that just ended consumed.
    ///
    /// A negative delta means the tank went up, i.e. a pit stop: the history
    /// is dropped rather than averaged, because burn measured across a refuel
    /// is meaningless.
    fn record_fuel_for_completed_lap(&mut self, fuel_now: f32) {
        let used = self.stats.fuel_at_lap_start - fuel_now;
        self.stats.fuel_at_lap_start = fuel_now;

        if used < 0.0 {
            self.stats.recent_fuel_per_lap.clear();
            return;
        }
        if used <= 0.0 || !used.is_finite() {
            return;
        }

        self.stats.recent_fuel_per_lap.push(used);
        if self.stats.recent_fuel_per_lap.len() > FUEL_HISTORY_LAPS {
            self.stats.recent_fuel_per_lap.remove(0);
        }
    }

    fn reset_counters(&mut self) {
        self.stats.bottoming_frames = [0; 4];
        self.stats.lockup_frames_front = 0;
        self.stats.lockup_frames_rear = 0;
        self.stats.wheel_spin_frames = 0;
        self.stats.traction_loss_frames = 0;
        self.stats.oversteer_frames = 0;
        self.stats.understeer_frames = 0;
        self.stats.coasting_frames = 0;
        self.stats.scrubbing_frames = 0;
        self.stats.current_excess_steer = 0.0;
        self.stats.total_frames = 0;
        self.stats.ffb_clip_frames = 0;
    }

    /// Forget everything measured about this stint. Called when the session
    /// changes underneath us.
    pub fn reset_fuel_tracking(&mut self) {
        self.stats.recent_fuel_per_lap.clear();
        self.stats.fuel_at_lap_start = 0.0;
        self.stats.fuel_laps_remaining = 0.0;
        self.stats.fuel_consumption_rate = 0.0;
    }

    fn check_hysteresis(&mut self, key: &str, active: bool) -> bool {
        let now = Instant::now();
        if active {
            let first_seen = if let Some(&(first, _)) = self.alert_timers.get(key) {
                first
            } else {
                now
            };
            self.alert_timers.insert(key.to_string(), (first_seen, now));
            return now.duration_since(first_seen) >= Duration::from_secs_f32(1.0);
        }

        if let Some(&(first_seen, last_seen)) = self.alert_timers.get(key)
            && now.duration_since(last_seen) < Duration::from_secs_f32(2.0)
        {
            return now.duration_since(first_seen) >= Duration::from_secs_f32(1.0);
        }

        self.alert_timers.remove(key);
        false
    }

    pub fn analyze_live(
        &mut self,
        car: &Car,
        session: &Session,
        setup: Option<&CarSetup>,
    ) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        self.analyze_tyre_pressure(car, session, &mut recommendations);
        // Both of these are built on measurements a game may simply not make,
        // and a missing measurement reads as zero — which is a confident
        // verdict about a car nobody drove. ACC publishes core tyre
        // temperature and no tread across it at all; the camber rule *is*
        // inner minus outer, and the temperature band is written against the
        // mean of the three. Neither has anything to fall back on that is the
        // same physical quantity, so neither runs.
        if self.capabilities.tyre_edge_temps {
            self.analyze_tyre_temperature(car, &mut recommendations);
            self.analyze_camber(car, &mut recommendations);
        }
        if self.capabilities.tyre_wear {
            self.analyze_tyre_wear(car, &mut recommendations);
        }
        self.analyze_suspension(car, &mut recommendations);
        self.analyze_brakes(car, &mut recommendations);
        self.analyze_brake_bias(setup, &mut recommendations);
        self.analyze_aero(car, &mut recommendations);

        self.analyze_driving_errors(&mut recommendations);
        self.analyze_strategy(car, session, &mut recommendations);
        self.analyze_ffb_clipping(car, &mut recommendations);

        recommendations.sort_by(|a, b| {
            b.severity
                .partial_cmp(&a.severity)
                .unwrap_or(Ordering::Equal)
                .then(
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(Ordering::Equal),
                )
        });

        recommendations
    }

    fn analyze_suspension(&mut self, _car: &Car, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        // Which corners, not merely whether. The loop used to break on the
        // first one over the threshold, which was enough to raise the alert and
        // not enough to say anything about it — and a car bottoming on one rear
        // corner is a different problem from one bottoming on both fronts.
        const BOTTOMING_FRAMES: u32 = 30;
        let grounded: Vec<usize> = (0..4)
            .filter(|i| self.stats.bottoming_frames[*i] > BOTTOMING_FRAMES)
            .collect();
        let bottoming_detected = !grounded.is_empty();
        if self.check_hysteresis("bottoming", bottoming_detected) && bottoming_detected {
            recs.push(Recommendation {
                component: "Suspension".tr(ru).to_string(),
                category: "Bottoming".tr(ru).to_string(),
                severity: Severity::Critical,
                message: "Chassis bottoming out!".tr(ru).to_string(),
                action: "Increase ride height or stiffness".tr(ru).to_string(),
                parameters: vec![],
                confidence: 0.95,
                chain: Some(Chain {
                    cause: "the suspension is running out of travel over kerbs and compressions"
                        .tr(ru)
                        .to_string(),
                    effect: match ru {
                        true => format!(
                            "{}: {}",
                            Self::corner_phrase(&grounded, ru),
                            Self::frames_phrase(
                                grounded
                                    .iter()
                                    .map(|i| self.stats.bottoming_frames[*i])
                                    .max()
                                    .unwrap_or(0),
                                self.stats.total_frames,
                                ru
                            )
                        ),
                        false => format!(
                            "{}: {}",
                            Self::corner_phrase(&grounded, ru),
                            Self::frames_phrase(
                                grounded
                                    .iter()
                                    .map(|i| self.stats.bottoming_frames[*i])
                                    .max()
                                    .unwrap_or(0),
                                self.stats.total_frames,
                                ru
                            )
                        ),
                    },
                    confirm: tr_fmt(
                        "the bottoming count on {0} over the same lap, once it is raised",
                        ru,
                        &[&Self::corner_phrase_mid(&grounded, ru)],
                    ),
                    // One observation per corner that grounded, measured past
                    // the threshold rather than from zero — a corner one frame
                    // over is not the same finding as one forty frames over.
                    evidence: crate::confidence::Evidence::from_values(
                        grounded
                            .iter()
                            .map(|i| (self.stats.bottoming_frames[*i] - BOTTOMING_FRAMES) as f32),
                    ),
                }),
            });
        }
    }

    fn analyze_aero(&mut self, car: &Car, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        if self.stats.high_speed_rake != 0.0
            && self.stats.low_speed_rake != 0.0
            && car.speed_kmh > 150.0
        {
            let rake_loss = self.stats.low_speed_rake - self.stats.high_speed_rake;
            if self.check_hysteresis("aero_rake", rake_loss > 10.0) && rake_loss > 10.0 {
                recs.push(Recommendation {
                    component: "Aerodynamics".tr(ru).to_string(),
                    category: "Rake Loss".to_string(),
                    severity: Severity::Warning,
                    message: tr_fmt(
                        "Rear dropping too much at high speed (-{0}mm)",
                        ru,
                        &[&format!("{rake_loss:.1}")],
                    ),
                    action: "Stiffen Rear Springs or add Packers".tr(ru).to_string(),
                    parameters: vec![],
                    confidence: 0.85,
                    chain: Some(Chain {
                        cause: "downforce is squatting the rear, and the rake goes with it"
                            .tr(ru)
                            .to_string(),
                        effect: tr_fmt(
                            "-{0} mm between low and high speed",
                            ru,
                            &[&format!("{rake_loss:.1}")],
                        ),
                        confirm: "the rake difference at the same speed next run out"
                            .tr(ru)
                            .to_string(),
                        // One measurement, taken at one speed. It is a finding,
                        // not four corroborating observations, and saying so is
                        // the point of leaving this empty.
                        evidence: crate::confidence::Evidence::new(),
                    }),
                });
            }
        }
    }

    pub fn get_wizard_advice(&self) -> Vec<String> {
        let is_ru = self.config.language == Language::Russian;
        let mut advice = Vec::new();

        match (&self.wizard_phase, &self.wizard_problem) {
            (WizardPhase::Entry, WizardProblem::Understeer) => {
                advice.push("Decrease Front Rebound".tr(is_ru).to_string());
                advice.push("Increase Rear Ride Height".tr(is_ru).to_string());
                advice.push("Move Brake Bias Rearwards".tr(is_ru).to_string());
            }
            (WizardPhase::Entry, WizardProblem::Oversteer) => {
                advice.push("Increase Front Rebound".tr(is_ru).to_string());
                advice.push("Move Brake Bias Forwards".tr(is_ru).to_string());
                advice.push("Increase Front Wing".tr(is_ru).to_string());
            }
            (WizardPhase::Apex, WizardProblem::Understeer) => {
                advice.push("Softer Front Springs".tr(is_ru).to_string());
                advice.push("Softer Front ARB".tr(is_ru).to_string());
                advice.push("More Front Camber".tr(is_ru).to_string());
            }
            (WizardPhase::Apex, WizardProblem::Oversteer) => {
                advice.push("Softer Rear Springs".tr(is_ru).to_string());
                advice.push("Softer Rear ARB".tr(is_ru).to_string());
                advice.push("Increase Front Ride Height".tr(is_ru).to_string());
            }
            (WizardPhase::Exit, WizardProblem::Understeer) => {
                advice.push("Increase Front Bump".tr(is_ru).to_string());
                advice.push("Stiffer Rear Springs".tr(is_ru).to_string());
                advice.push("Increase Diff Power".tr(is_ru).to_string());
            }
            (WizardPhase::Exit, WizardProblem::Oversteer) => {
                advice.push("Softer Rear Springs".tr(is_ru).to_string());
                advice.push("Decrease Rear Bump".tr(is_ru).to_string());
                advice.push("Decrease Diff Power".tr(is_ru).to_string());
                advice.push("Increase TC".tr(is_ru).to_string());
            }
            (_, WizardProblem::Instability) => {
                advice.push("Increase Downforce (Wings)".tr(is_ru).to_string());
                advice.push("More Rear Toe-In".tr(is_ru).to_string());
                advice.push("Stiffer Suspension Overall".tr(is_ru).to_string());
            }
        }
        advice
    }

    fn is_ru(&self) -> bool {
        self.config.language == Language::Russian
    }

    pub fn compare_setups_advice(&self, target: &CarSetup, reference: &CarSetup) -> Vec<String> {
        let mut advice = Vec::new();
        let ru = self.is_ru();

        let aero_diff =
            (target.wing_1 + target.wing_2) as i32 - (reference.wing_1 + reference.wing_2) as i32;
        if aero_diff != 0 {
            advice.push(tr_fmt("Aero: {0}", ru, &[&format!("{aero_diff:+}")]));
        }

        let camber_f_diff =
            (target.camber_lf + target.camber_rf) - (reference.camber_lf + reference.camber_rf);
        if camber_f_diff.abs() > 2 {
            advice.push(tr_fmt(
                "Front Camber: {0}",
                ru,
                &[&format!("{camber_f_diff:+}")],
            ));
        }

        let avg_p_target: f32 =
            (target.pressure_lf + target.pressure_rf + target.pressure_lr + target.pressure_rr)
                as f32
                / 4.0;
        let avg_p_ref: f32 = (reference.pressure_lf
            + reference.pressure_rf
            + reference.pressure_lr
            + reference.pressure_rr) as f32
            / 4.0;
        if (avg_p_target - avg_p_ref).abs() > 1.0 {
            advice.push(tr_fmt(
                "Tyre Press: {0} PSI",
                ru,
                &[&format!("{:+.1}", avg_p_target - avg_p_ref)],
            ));
        }

        if advice.is_empty() {
            advice.push("No major differences".tr(ru).to_string());
        }
        advice
    }

    fn analyze_ffb_clipping(&mut self, car: &Car, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        let clip_ratio = if self.stats.total_frames > 0 {
            self.stats.ffb_clip_frames as f32 / self.stats.total_frames as f32
        } else {
            0.0
        };

        let is_clipping = clip_ratio > 0.05 && car.speed_kmh > 10.0;

        if self.check_hysteresis("ffb_clip", is_clipping) && is_clipping {
            recs.push(Recommendation {
                component: "Force Feedback".tr(ru).to_string(),
                category: "Clipping".to_string(),
                severity: Severity::Warning,
                message: tr_fmt(
                    "FFB Clipping: {0}% of time",
                    ru,
                    &[&format!("{:.1}", clip_ratio * 100.0)],
                ),
                action: "Lower FFB Gain".tr(ru).to_string(),
                parameters: vec![Parameter {
                    name: "Clip Ratio".to_string(),
                    current: clip_ratio * 100.0,
                    target: 0.0,
                    unit: "%".to_string(),
                }],
                confidence: 1.0,
                chain: Some(Chain {
                    cause: "the signal is hitting its ceiling, and everything above it never reaches the wheel".tr(ru)
                    .to_string(),
                    effect: Self::frames_phrase(
                        self.stats.ffb_clip_frames,
                        self.stats.total_frames,
                        ru,
                    ),
                    // The one rule whose check is not "next lap": clipping
                    // answers to a slider, and the answer arrives in the corner
                    // after it is moved.
                    confirm: "the clipping share after lowering the gain — near zero through corners".tr(ru)
                    .to_string(),
                    evidence: crate::confidence::Evidence::new(),
                }),
            });
        }
    }

    /// Name a set of corners the way an engineer would say it out loud.
    ///
    /// Four separate lines saying the same thing about four wheels is four of
    /// the overlay's slots spent on one fact, and the driver reads "FL COLD /
    /// FR COLD / RL COLD / RR COLD" as noise rather than as "the tyres are not
    /// up to temperature yet". Which is what it means.
    /// A frame count with something to measure it against.
    ///
    /// The driving rules all count frames over a lap, and a bare "412 frames"
    /// means nothing to anybody: it is 7 seconds on one circuit and 12 on
    /// another. The share of the lap is the part a driver can act on, so it is
    /// said when the denominator exists and quietly left out when it does not —
    /// a percentage of nothing is the sort of confident zero this project keeps
    /// having to remove.
    fn frames_phrase(frames: u32, total: u32, ru: bool) -> String {
        if total == 0 {
            return tr_fmt("{0} frames", ru, &[&frames.to_string()]);
        }
        let share = frames as f32 / total as f32 * 100.0;
        tr_fmt(
            "{0} frames of the lap ({1} %)",
            ru,
            &[&frames.to_string(), &format!("{share:.0}")],
        )
    }

    fn corner_phrase(corners: &[usize], ru: bool) -> String {
        match corners {
            [] => String::new(),
            [only] => CORNER_NAMES[*only].to_string(),
            [0, 1] => "Fronts".tr(ru).to_string(),
            [2, 3] => "Rears".tr(ru).to_string(),
            [0, 2] => "Left side".tr(ru).to_string(),
            [1, 3] => "Right side".tr(ru).to_string(),
            [0, 1, 2, 3] => "All four".tr(ru).to_string(),
            many => many
                .iter()
                .map(|index| CORNER_NAMES[*index])
                .collect::<Vec<_>>()
                .join("/"),
        }
    }

    /// The same phrase, for the middle of a sentence rather than the start of
    /// one.
    ///
    /// `corner_phrase` is capitalised because every message begins with it.
    /// The chain's `confirm` does not — "the hot pressure on All four after two
    /// laps" is the sort of thing that reads as machine-generated, which is
    /// exactly what this advice is trying not to sound like.
    ///
    /// Lowercased only when the phrase is a word. `FL`, `RR` and `FL/RR` are
    /// names and stay as they are — a blanket `to_lowercase` would print "fl",
    /// which is worse than the capital it fixed.
    fn corner_phrase_mid(corners: &[usize], ru: bool) -> String {
        let phrase = Self::corner_phrase(corners, ru);
        if phrase.chars().any(|c| c.is_lowercase()) {
            let mut chars = phrase.chars();
            match chars.next() {
                Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
                None => phrase,
            }
        } else {
            phrase
        }
    }

    fn analyze_tyre_pressure(
        &mut self,
        car: &Car,
        session: &Session,
        recs: &mut Vec<Recommendation>,
    ) {
        let ru = self.is_ru();

        let compound_name = session.compound.to_string().to_lowercase();

        let class_name = if compound_name.contains("street")
            || compound_name.contains("sport")
            || compound_name.contains("eco")
            || compound_name.contains("semislick")
        {
            "Street"
        } else if compound_name.contains("wet") || compound_name.contains("rain") {
            "Wet"
        } else {
            "Racing"
        };
        let pressure_min = self
            .config
            .alerts
            .tyre_pressure_min
            .min(self.config.alerts.tyre_pressure_max);
        let pressure_max = self
            .config
            .alerts
            .tyre_pressure_min
            .max(self.config.alerts.tyre_pressure_max);
        let base_optimal = if self.config.target_tyre_pressure > 0.0 {
            self.config.target_tyre_pressure
        } else {
            (pressure_min + pressure_max) / 2.0
        };

        let grip_compensation = (1.0 - session.surface_grip.clamp(0.80, 1.0)) * 1.5;
        let optimal_pressure = base_optimal + grip_compensation;

        let mut low: Vec<usize> = Vec::new();
        let mut high: Vec<usize> = Vec::new();

        for i in 0..4 {
            let pressure = car.tyre_pressure_psi[i];
            let is_error = pressure < pressure_min || pressure > pressure_max;

            let key = format!("pres_{}", i);
            if !self.check_hysteresis(&key, is_error) || car.speed_kmh <= 10.0 || !is_error {
                continue;
            }

            if pressure < optimal_pressure {
                low.push(i);
            } else {
                high.push(i);
            }
        }

        // Units, at last. The temperature analyser has gone through the
        // formatter since it was written; this one printed raw psi, so anyone
        // working in bar read their pressures in one unit on the Dashboard and
        // another in the advice about them.
        let formatter = self.config.formatter();

        let mut push = |corners: &[usize], inflate: bool| {
            if corners.is_empty() {
                return;
            }
            let average = corners
                .iter()
                .map(|i| car.tyre_pressure_psi[*i])
                .sum::<f32>()
                / corners.len() as f32;
            let difference = (average - optimal_pressure).abs();

            recs.push(Recommendation {
                component: tr_fmt("Tyres ({0})", ru, &[class_name]),
                category: "Pressure".tr(ru).to_string(),
                // A pressure a full unit off the target changes how the car
                // turns; half of one is a setup working as intended.
                severity: if difference > 2.5 {
                    Severity::Warning
                } else {
                    Severity::Info
                },
                message: format!(
                    "{} {}: {} ({} {})",
                    Self::corner_phrase(corners, ru),
                    "pressure".tr(ru),
                    formatter.format_pressure(average),
                    "target".tr(ru),
                    formatter.format_pressure(optimal_pressure)
                ),
                action: if inflate {
                    "Inflate".tr(ru)
                } else {
                    "Deflate".tr(ru)
                }
                .to_string(),
                parameters: corners
                    .iter()
                    .map(|i| Parameter {
                        name: CORNER_NAMES[*i].to_string(),
                        current: car.tyre_pressure_psi[*i],
                        target: optimal_pressure,
                        unit: formatter.pressure_symbol().to_string(),
                    })
                    .collect(),
                confidence: 0.9,
                // Hot pressure is not something the driver sets. It is the cold
                // setting plus whatever the tyre has been made to absorb, so the
                // cause is on one side of that and the fix is on the other —
                // which is exactly what makes this worth stating rather than
                // printing a number against a target.
                chain: Some(Chain {
                    cause: if inflate {
                        "the tyre is not building enough heat to reach the window"
                    } else {
                        "the tyre is building more pressure than the cold setting allows for"
                    }
                    .tr(ru)
                    .to_string(),
                    effect: format!(
                        "{} {} ({} {})",
                        Self::corner_phrase(corners, ru),
                        formatter.format_pressure(average),
                        "target".tr(ru),
                        formatter.format_pressure(optimal_pressure)
                    ),
                    confirm: tr_fmt(
                        "the hot pressure on {0} after two laps at pace: {1} is the target",
                        ru,
                        &[
                            &Self::corner_phrase_mid(corners, ru),
                            &formatter.format_pressure(optimal_pressure),
                        ],
                    ),
                    // One observation per corner in the group, each a live
                    // reading rather than an average — so two corners agreeing
                    // is Medium and all four are High, which is the right shape
                    // for a measurement this direct.
                    evidence: crate::confidence::Evidence::from_values(
                        corners
                            .iter()
                            .map(|i| car.tyre_pressure_psi[*i] - optimal_pressure),
                    ),
                }),
            });
        };

        push(&low, true);
        push(&high, false);
    }

    fn analyze_tyre_wear(&mut self, car: &Car, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();

        // AC counts wear down from 100, so all four corners reading zero is not
        // four destroyed tyres — it is a session that has not published wear
        // yet. Without this the panel opens with four CRITICAL lines telling a
        // driver who just left the pits that every tyre is gone, which is the
        // kind of thing that gets an engineer ignored for the rest of the race.
        if car.tyre_wear.iter().all(|wear| *wear <= 0.0) {
            return;
        }

        let warning_threshold = self.config.alerts.wear_warning;
        // Its own threshold, not `warning - 2`. That derivation made a tyre at
        // 93.9 % life a CRITICAL "WORN OUT" with the default settings, which is
        // a tyre most of the way through its first stint.
        let critical_threshold = self
            .config
            .alerts
            .wear_critical
            .min(self.config.alerts.wear_warning);

        let mut worn: Vec<usize> = Vec::new();
        let mut critical: Vec<usize> = Vec::new();

        for (i, wear) in car.tyre_wear.iter().copied().enumerate() {
            let is_worn = wear < warning_threshold;
            // The hysteresis is still per corner: it is per-corner state, and
            // one wheel picking up a flat spot should not reset the timers on
            // the other three. Only the reporting is grouped.
            if !self.check_hysteresis(&format!("wear_{}", i), is_worn) || !is_worn {
                continue;
            }
            if wear < critical_threshold {
                critical.push(i);
            } else {
                worn.push(i);
            }
        }

        // Read off before the closure: it needs them for the chain, and the
        // closure must not hold a borrow of `self` while `recs` is being
        // written through.
        let stint_laps = self.stats.stint_laps;
        let laps_left = self.stats.tyre_laps_remaining;

        let mut push = |corners: &[usize], severity: Severity| {
            if corners.is_empty() {
                return;
            }
            let lowest = corners
                .iter()
                .map(|i| car.tyre_wear[*i])
                .fold(f32::MAX, f32::min);
            let where_ = Self::corner_phrase(corners, ru);
            let where_low = Self::corner_phrase_mid(corners, ru);
            let what = if severity == Severity::Critical {
                "WORN OUT"
            } else {
                "high wear"
            }
            .tr(ru);

            recs.push(Recommendation {
                component: "Tyres".tr(ru).to_string(),
                category: "Wear".tr(ru).to_string(),
                severity,
                message: format!("{where_} {what}: {lowest:.1}%"),
                action: "Box / Careful".tr(ru)
                .to_string(),
                parameters: corners
                    .iter()
                    .map(|i| Parameter {
                        name: format!("{} life", CORNER_NAMES[*i]),
                        current: car.tyre_wear[*i],
                        target: 100.0,
                        unit: "%".to_string(),
                    })
                    .collect(),
                confidence: 0.9,
                // Wear is the one finding whose cause is simply time: this set
                // has done these laps. What makes it worth chaining is the
                // check — a percentage means nothing to a driver deciding
                // whether to stop, and laps left does.
                chain: Some(Chain {
                    // A stint of zero laps is a real state rather than a missing
                    // number — a set can be past the warning before the first
                    // lap is complete — and "0 laps on this set" as the reason
                    // reads as a broken sentence rather than as the truth it is.
                    cause: if stint_laps > 0 {
                        tr_fmt("{0} laps on this set", ru, &[&stint_laps.to_string()])
                    } else {
                        "no complete lap on this set yet".tr(ru).to_string()
                    },
                    effect: format!("{where_} {lowest:.1}%"),
                    confirm: {
                        // From the worst corner in the group, not the average:
                        // a set is finished when one corner is.
                        let soonest = corners
                            .iter()
                            .map(|i| laps_left[*i])
                            .fold(f32::MAX, f32::min);
                        if soonest.is_finite() && soonest > 0.0 {
                            tr_fmt(
                                "the life on {0} at the end of the next lap: ~{1} laps left at this rate",
                                ru,
                                &[&where_low, &format!("{soonest:.0}")],
                            )
                        } else {
                            tr_fmt(
                                "the life on {0} at the end of the next lap",
                                ru,
                                &[&where_low],
                            )
                        }
                    },
                    // How far past the warning each corner is, rather than the
                    // life itself: four corners at 88–91 % are all a long way
                    // from zero and would agree trivially on the raw number.
                    evidence: crate::confidence::Evidence::from_values(
                        corners
                            .iter()
                            .map(|i| warning_threshold - car.tyre_wear[*i]),
                    ),
                }),
            });
        };

        push(&critical, Severity::Critical);
        push(&worn, Severity::Warning);
    }

    /// The camber a wheel is running, in degrees, in the car's frame rather
    /// than the wheel's own.
    ///
    /// AC publishes `camberRAD` per wheel, and the two sides mirror each other:
    /// an abarth500 sitting in the pits with the same setting on both front
    /// wheels reads -0.023 rad on the left and +0.021 on the right. Negating
    /// the right-hand corners puts all four on one scale, where negative means
    /// the top of the tyre leans in — which is what a driver means by camber.
    fn camber_degrees(car: &Car, corner: usize) -> f32 {
        let sign = if corner == 1 || corner == 3 {
            -1.0
        } else {
            1.0
        };
        car.camber_rad[corner].to_degrees() * sign
    }

    /// Cornering ticks needed before the camber average is worth publishing.
    /// 60 ticks is a second of load, which is one long corner or two short
    /// ones — enough that the average is not a single frame of noise.
    const CAMBER_MIN_FRAMES: u32 = 60;

    /// What the inner and outer edges of a tyre say about its camber.
    ///
    /// Judged on `stats.camber_spread`, which is only sampled while the car is
    /// cornering — see the field. The instantaneous spread this used to read
    /// is near zero on every straight, so it published four Info lines about
    /// nothing every time the car left a corner.
    fn analyze_camber(&self, car: &Car, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        if self.stats.camber_frames < Self::CAMBER_MIN_FRAMES {
            return;
        }
        let fmt = self.config.formatter();
        let ideal_spread = 8.0;

        let mut too_little: Vec<usize> = Vec::new();
        let mut too_much: Vec<usize> = Vec::new();
        for i in 0..4 {
            let spread = self.stats.camber_spread[i];
            if spread < 2.0 {
                too_little.push(i);
            } else if spread > 15.0 {
                too_much.push(i);
            }
        }

        // Four corners of one problem are one problem — the same grouping the
        // wear and pressure advice does, and for the same reason: the panel
        // shows eight lines, and four of them saying "FL", "FR", "RL", "RR"
        // about one setting crowds out everything else the engineer noticed.
        let mut push = |corners: &[usize], more_camber: bool| {
            if corners.is_empty() {
                return;
            }
            let where_ = Self::corner_phrase(corners, ru);
            // The worst corner of the group is the one worth naming: the
            // furthest from the window for too little camber, the hottest
            // inner edge for too much.
            let spread = corners
                .iter()
                .map(|i| self.stats.camber_spread[*i])
                .fold(f32::NAN, if more_camber { f32::min } else { f32::max });
            // AC publishes the camber it is running, so the advice can say what
            // to change *from*. The setup file cannot: `CAMBER_LF VALUE=-9` is
            // a step index into a range that lives inside the car's `data.acd`,
            // and printing it read as "now: -9" beside a car showing -1.3°.
            let now = corners
                .iter()
                .map(|i| Self::camber_degrees(car, *i))
                .sum::<f32>()
                / corners.len() as f32;
            let now_clause = if now.abs() > 0.05 {
                format!(" ({}: {now:.1}°)", "now".tr(ru))
            } else {
                String::new()
            };

            recs.push(Recommendation {
                component: "Suspension".tr(ru).to_string(),
                category: "Camber".tr(ru).to_string(),
                severity: if more_camber {
                    Severity::Info
                } else {
                    Severity::Warning
                },
                // The message hardcoded "C" while the parameter beside it
                // was labelled with the configured symbol, so with
                // Fahrenheit selected the user saw a Celsius number
                // labelled °F. A spread is a difference, so it converts
                // by scale only -- `format_temp` would add 32.
                message: format!(
                    "{where_} {} (I-O: {})",
                    if more_camber {
                        "contact patch inefficient"
                    } else {
                        "inner edge overheating"
                    }
                    .tr(ru),
                    fmt.format_temp_delta(spread)
                ),
                action: if more_camber {
                    tr_fmt(
                        "More neg. camber{0}. If maxed -> soften ARB",
                        ru,
                        &[&now_clause],
                    )
                } else {
                    tr_fmt(
                        "Less neg. camber{0}. If maxed -> stiffen ARB",
                        ru,
                        &[&now_clause],
                    )
                },
                parameters: corners
                    .iter()
                    .map(|i| Parameter {
                        name: format!("{} I-O", CORNER_NAMES[*i]),
                        current: fmt.temp_delta_val(self.stats.camber_spread[*i]),
                        target: fmt.temp_delta_val(ideal_spread),
                        unit: fmt.temp_symbol().to_string(),
                    })
                    .collect(),
                confidence: if more_camber { 0.7 } else { 0.8 },
                // The one rule that can say why, what it did, and how to know
                // whether the change worked — and the one the plan names,
                // because a camber verdict from a single cornering frame is
                // how this project learned to distrust its own certainty.
                //
                // The evidence is one observation per wheel in the group, each
                // already the mean of at least `CAMBER_MIN_FRAMES` frames of
                // load. That is what `averaged_over` is for: two settled
                // wheels are a finding, two frames are not, and the frame gate
                // above stops being a bolted-on precondition and becomes part
                // of how sure the advice says it is.
                chain: Some(Chain {
                    cause: if more_camber {
                        "the outer shoulder is not being loaded through corners"
                    } else {
                        "the inner shoulder is carrying the corner"
                    }
                    .tr(ru)
                    .to_string(),
                    effect: format!("{where_} I-O {}", fmt.format_temp_delta(spread)),
                    confirm: tr_fmt(
                        "the I/M/O spread on {0} next run out: {1} is the window",
                        ru,
                        &[&where_, &fmt.format_temp_delta(ideal_spread)],
                    ),
                    evidence: crate::confidence::Evidence::from_values(
                        corners.iter().map(|i| self.stats.camber_spread[*i]),
                    )
                    .averaged_over(self.stats.camber_frames),
                }),
            });
        };

        push(&too_much, false);
        push(&too_little, true);
    }

    fn analyze_tyre_temperature(&mut self, car: &Car, recs: &mut Vec<Recommendation>) {
        let min_temp = self
            .config
            .alerts
            .tyre_temp_min
            .min(self.config.alerts.tyre_temp_max);
        let max_temp = self
            .config
            .alerts
            .tyre_temp_min
            .max(self.config.alerts.tyre_temp_max);
        let ru = self.is_ru();

        if car.speed_kmh <= 100.0 {
            return;
        }

        let mut cold: Vec<usize> = Vec::new();
        let mut hot: Vec<usize> = Vec::new();

        for i in 0..4 {
            let temp = car.avg_tyre_temp_c(i);
            let out_of_band = temp < min_temp || temp > max_temp;
            // Same gate as the pressure and wear alerts. This ran on every
            // frame, so a tyre that stayed cold produced a fresh recommendation
            // dozens of times a second.
            if !self.check_hysteresis(&format!("tyre_temp_{}", i), out_of_band) {
                continue;
            }
            if temp < min_temp {
                cold.push(i);
            } else if temp > max_temp {
                hot.push(i);
            }
        }

        let formatter = self.config.formatter();

        if !cold.is_empty() {
            let average =
                cold.iter().map(|i| car.avg_tyre_temp_c(*i)).sum::<f32>() / cold.len() as f32;
            recs.push(Recommendation {
                component: "Tyres".tr(ru).to_string(),
                category: "Temperature".tr(ru).to_string(),
                severity: Severity::Warning,
                message: format!(
                    "{} {}: {}",
                    Self::corner_phrase(&cold, ru),
                    "COLD".tr(ru),
                    formatter.format_temp(average)
                ),
                action: "Warm tyres".tr(ru).to_string(),
                parameters: vec![],
                confidence: 0.95,
                chain: Some(Chain {
                    cause: "not enough energy is going into the tyre to bring it into its window"
                        .tr(ru)
                        .to_string(),
                    effect: format!(
                        "{} {} ({} {})",
                        Self::corner_phrase(&cold, ru),
                        formatter.format_temp(average),
                        "window from".tr(ru),
                        formatter.format_temp(min_temp)
                    ),
                    confirm: tr_fmt(
                        "the temperature on {0} after a lap at pace: the window starts at {1}",
                        ru,
                        &[
                            &Self::corner_phrase_mid(&cold, ru),
                            &formatter.format_temp(min_temp),
                        ],
                    ),
                    evidence: crate::confidence::Evidence::from_values(
                        cold.iter().map(|i| min_temp - car.avg_tyre_temp_c(*i)),
                    ),
                }),
            });
        }

        if !hot.is_empty() {
            let average =
                hot.iter().map(|i| car.avg_tyre_temp_c(*i)).sum::<f32>() / hot.len() as f32;
            recs.push(Recommendation {
                component: "Tyres".tr(ru).to_string(),
                category: "Overheat".tr(ru).to_string(),
                severity: Severity::Critical,
                message: format!(
                    "{} {}: {}",
                    Self::corner_phrase(&hot, ru),
                    "OVERHEATING".tr(ru),
                    formatter.format_temp(average)
                ),
                action: "Cool tyres".tr(ru).to_string(),
                parameters: vec![],
                confidence: 0.95,
                // Deliberately vague about the mechanism, because there are
                // three and this rule cannot tell them apart from a temperature
                // alone: too little pressure, too much slip, or a car simply
                // being asked for more than the compound has. Naming one would
                // be a guess dressed as a diagnosis; the check is the same
                // whichever it is.
                chain: Some(Chain {
                    cause: "the tyre is being given more energy than it can shed"
                        .tr(ru)
                        .to_string(),
                    effect: format!(
                        "{} {} ({} {})",
                        Self::corner_phrase(&hot, ru),
                        formatter.format_temp(average),
                        "window to".tr(ru),
                        formatter.format_temp(max_temp)
                    ),
                    confirm: tr_fmt(
                        "the temperature on {0} a lap after the change: the window ends at {1}",
                        ru,
                        &[
                            &Self::corner_phrase_mid(&hot, ru),
                            &formatter.format_temp(max_temp),
                        ],
                    ),
                    evidence: crate::confidence::Evidence::from_values(
                        hot.iter().map(|i| car.avg_tyre_temp_c(*i) - max_temp),
                    ),
                }),
            });
        }
    }

    fn analyze_brakes(&mut self, car: &Car, recs: &mut Vec<Recommendation>) {
        let max_temp = self.config.alerts.brake_temp_max;
        let ru = self.is_ru();

        let mut cooking: Vec<usize> = Vec::new();
        for i in 0..4 {
            // Gated the way the pressure and wear alerts already are. Without
            // it this pushed a fresh recommendation on every single frame the
            // brake was over temperature — dozens a second, burying every
            // other message in the list.
            let too_hot = car.brake_temp_c[i] > max_temp;
            if self.check_hysteresis(&format!("brake_temp_{}", i), too_hot) && too_hot {
                cooking.push(i);
            }
        }

        if cooking.is_empty() {
            return;
        }

        // Both front brakes overheating is one thing to say, not two. The
        // corner names are FL/FR/RL/RR the way every neighbouring alert says
        // them — this used to number them "Brake 1" to "Brake 4", the only
        // place in the application that did.
        let hottest = cooking
            .iter()
            .map(|i| car.brake_temp_c[*i])
            .fold(0.0_f32, f32::max);
        let formatter = self.config.formatter();

        recs.push(Recommendation {
            component: "Brakes".tr(ru).to_string(),
            category: "Overheat".tr(ru).to_string(),
            severity: Severity::Critical,
            message: format!(
                "{} {}: {}",
                Self::corner_phrase(&cooking, ru),
                "brakes cooking".tr(ru),
                formatter.format_temp(hottest)
            ),
            action: "Move bias / Cool down".tr(ru).to_string(),
            parameters: vec![],
            confidence: 1.0,
            chain: Some(Chain {
                cause: "more energy is going into the brakes than they can shed"
                    .tr(ru)
                    .to_string(),
                effect: format!(
                    "{} {} ({} {})",
                    Self::corner_phrase(&cooking, ru),
                    formatter.format_temp(hottest),
                    "ceiling".tr(ru),
                    formatter.format_temp(max_temp)
                ),
                confirm: tr_fmt(
                    "the peak on {0} through the next lap: {1} is the ceiling",
                    ru,
                    &[
                        &Self::corner_phrase_mid(&cooking, ru),
                        &formatter.format_temp(max_temp),
                    ],
                ),
                evidence: crate::confidence::Evidence::from_values(
                    cooking.iter().map(|i| car.brake_temp_c[*i] - max_temp),
                ),
            }),
        });
    }

    fn analyze_brake_bias(&self, setup: Option<&CarSetup>, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();
        let total_lockups = self.stats.lockup_frames_front + self.stats.lockup_frames_rear;

        if total_lockups > 20 {
            let current_bias_str = if let Some(s) = setup {
                tr_fmt(" (NOW: {0}%)", ru, &[&s.brake_bias.to_string()])
            } else {
                "".to_string()
            };

            if self.stats.lockup_frames_front > self.stats.lockup_frames_rear * 2 {
                recs.push(Recommendation {
                    component: "Brakes".tr(ru).to_string(),
                    category: "Bias".tr(ru).to_string(),
                    severity: Severity::Warning,
                    message: tr_fmt("FRONT Locking detected{0}", ru, &[&current_bias_str]),
                    action: "Move Bias REARWARDS".tr(ru).to_string(),
                    parameters: vec![],
                    confidence: 0.85,
                    // Evidence left empty on purpose, and it is not laziness.
                    // What this rule has is two whole-lap counters, and two
                    // counts of *different* things are not two observations of
                    // one — feeding them to `Evidence` would measure how much
                    // the front disagrees with the rear, which is the finding
                    // rather than the corroboration. An empty evidence makes
                    // `confidence_level` fall back to the score above, which is
                    // the honest answer here.
                    chain: Some(Chain {
                        cause: "too much of the braking is landing on the front axle"
                            .tr(ru)
                            .to_string(),
                        effect: tr_fmt(
                            "{0} frames of front lock against {1} at the rear",
                            ru,
                            &[
                                &self.stats.lockup_frames_front.to_string(),
                                &self.stats.lockup_frames_rear.to_string(),
                            ],
                        ),
                        confirm: "front lockups next run out, after moving the bias back"
                            .tr(ru)
                            .to_string(),
                        evidence: crate::confidence::Evidence::new(),
                    }),
                });
            } else if self.stats.lockup_frames_rear > self.stats.lockup_frames_front * 2 {
                recs.push(Recommendation {
                    component: "Brakes".tr(ru).to_string(),
                    category: "Bias".tr(ru).to_string(),
                    severity: Severity::Critical,
                    message: tr_fmt("REAR Locking (Danger!){0}", ru, &[&current_bias_str]),
                    action: "Move Bias FORWARDS".tr(ru).to_string(),
                    parameters: vec![],
                    confidence: 0.95,
                    chain: Some(Chain {
                        cause: "too much of the braking is landing on the rear axle"
                            .tr(ru)
                            .to_string(),
                        effect: tr_fmt(
                            "{0} frames of rear lock against {1} at the front",
                            ru,
                            &[
                                &self.stats.lockup_frames_rear.to_string(),
                                &self.stats.lockup_frames_front.to_string(),
                            ],
                        ),
                        confirm: "rear lockups next run out, after moving the bias forward"
                            .tr(ru)
                            .to_string(),
                        evidence: crate::confidence::Evidence::new(),
                    }),
                });
            }
        }
    }

    fn analyze_driving_errors(&mut self, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();

        let is_coasting = self.stats.coasting_frames > 60;
        if self.check_hysteresis("coast", is_coasting) && is_coasting {
            recs.push(Recommendation {
                component: "Driving".tr(ru).to_string(),
                category: "Time Loss".tr(ru).to_string(),
                severity: Severity::Info,
                message: "Excessive Coasting".tr(ru).to_string(),
                action: "Keep throttle or brake".tr(ru).to_string(),
                parameters: vec![],
                confidence: 0.7,
                // The whole-lap counters below all carry a chain with empty
                // evidence, and that is the design rather than a gap: one
                // counter is one observation however large it gets, and
                // `Evidence` exists to count observations that *corroborate*
                // each other. An empty one makes `confidence_level` fall back
                // to the score beside it, which is what these rules have always
                // been judged on. What they gain is the other three fields —
                // the mechanism, the measurement, and something to check.
                chain: Some(Chain {
                    cause: "the car is rolling unloaded where it should be braking or driving"
                        .tr(ru)
                        .to_string(),
                    effect: Self::frames_phrase(
                        self.stats.coasting_frames,
                        self.stats.total_frames,
                        ru,
                    ),
                    confirm: "the share of the next lap spent on neither pedal"
                        .tr(ru)
                        .to_string(),
                    evidence: crate::confidence::Evidence::new(),
                }),
            });
        }

        if self.stats.understeer_frames > 30 {
            recs.push(Recommendation {
                component: "Balance".tr(ru).to_string(),
                category: "Understeer".to_string(),
                severity: Severity::Warning,
                message: "High Speed Understeer".tr(ru).to_string(),
                action: "More Front Wing / Softer Front".tr(ru).to_string(),
                parameters: vec![],
                confidence: 0.85,
                chain: Some(Chain {
                    cause: "the front axle runs out of grip before the rear at speed"
                        .tr(ru)
                        .to_string(),
                    effect: Self::frames_phrase(
                        self.stats.understeer_frames,
                        self.stats.total_frames,
                        ru,
                    ),
                    // Deliberately not "the car will understeer less": whether
                    // this is the car or the driving is a different question,
                    // and `driver_vs_car` is what answers it over a stint.
                    confirm: "the understeer count next run out, after the change"
                        .tr(ru)
                        .to_string(),
                    evidence: crate::confidence::Evidence::new(),
                }),
            });
        }

        if self.stats.oversteer_frames > 30 {
            recs.push(Recommendation {
                component: "Balance".tr(ru).to_string(),
                category: "Oversteer".to_string(),
                severity: Severity::Warning,
                message: "High Speed Oversteer".tr(ru).to_string(),
                action: "More Rear Wing".tr(ru).to_string(),
                parameters: vec![],
                confidence: 0.85,
                chain: Some(Chain {
                    cause: "the rear axle runs out of grip before the front at speed"
                        .tr(ru)
                        .to_string(),
                    effect: Self::frames_phrase(
                        self.stats.oversteer_frames,
                        self.stats.total_frames,
                        ru,
                    ),
                    confirm: "the oversteer count next run out, after the change"
                        .tr(ru)
                        .to_string(),
                    evidence: crate::confidence::Evidence::new(),
                }),
            });
        }

        let is_scrubbing = self.stats.scrubbing_frames > 45;
        if self.check_hysteresis("scrubbing", is_scrubbing) && is_scrubbing {
            let excess = self.stats.current_excess_steer;
            recs.push(Recommendation {
                component: "Driving".tr(ru).to_string(),
                category: "Overdriving".tr(ru).to_string(),
                severity: Severity::Warning,
                message: tr_fmt(
                    "Steering over-rotated by {0}°! Tyres sliding.",
                    ru,
                    &[&format!("{excess:.0}")],
                ),
                action: tr_fmt(
                    "Reduce steering angle by {0}°",
                    ru,
                    &[&format!("{excess:.0}")],
                ),
                parameters: vec![],
                confidence: 0.95,
                chain: Some(Chain {
                    cause: "more steering angle than the corner will take, so the tyres scrub"
                        .tr(ru)
                        .to_string(),
                    effect: tr_fmt(
                        "{0}, worst excess {1}°",
                        ru,
                        &[
                            &Self::frames_phrase(
                                self.stats.scrubbing_frames,
                                self.stats.total_frames,
                                ru,
                            ),
                            &format!("{excess:.0}"),
                        ],
                    ),
                    confirm: "the over-rotation count through the same corners next lap"
                        .tr(ru)
                        .to_string(),
                    evidence: crate::confidence::Evidence::new(),
                }),
            });
            self.stats.scrubbing_frames = 0;
            self.stats.current_excess_steer = 0.0;
        }
    }

    /// Fuel, and the two rules that deliberately have no chain.
    ///
    /// Every other rule in this file now states a mechanism, a measurement and
    /// something to check next time. These two do not, and the empty `chain`
    /// below is the finding rather than an omission:
    ///
    /// * there is **no mechanism** to state. "The fuel will not last" is
    ///   arithmetic on what is in the tank, not a chain of cause and effect —
    ///   writing one would mean inventing a story for a subtraction.
    /// * there is **nothing to check next time**, because the check is the same
    ///   number a lap later, and it is already on the screen. `confirm` exists
    ///   to make advice falsifiable; advice that restates its own input is
    ///   falsified by looking at it.
    ///
    /// A `Chain` filled in here would be three fields of ceremony that made the
    /// advice look better researched than it is, and that is the opposite of
    /// what the field is for.
    fn analyze_strategy(&self, car: &Car, session: &Session, recs: &mut Vec<Recommendation>) {
        let ru = self.is_ru();

        if self.stats.fuel_laps_remaining < self.config.alerts.fuel_warning_laps
            && self.stats.fuel_laps_remaining > 0.0
        {
            recs.push(Recommendation {
                component: "Strategy".tr(ru).to_string(),
                category: "Fuel".tr(ru).to_string(),
                severity: Severity::Critical,
                message: tr_fmt(
                    "FUEL LOW: {0} laps",
                    ru,
                    &[&format!("{:.1}", self.stats.fuel_laps_remaining)],
                ),
                action: "BOX BOX BOX".to_string(),
                parameters: vec![],
                confidence: 1.0,
                chain: None,
            });
        }

        if (session.session_time_left_ms > 0.0 || session.total_laps > 0)
            && session.fuel_per_lap > 0.0
        {
            // Whole laps, not the display fraction: a timed race runs until
            // the leader completes the lap the clock ran out on, and the lap
            // already in progress still has to be finished.
            let laps_remaining_in_race = crate::session_info::SessionTiming::laps_to_fuel_for(
                session.session_time_left_ms,
                session.best_lap_ms,
                session.last_lap_ms,
                session.total_laps,
                session.completed_laps,
                session.track_position,
            );

            if laps_remaining_in_race > 0.0 {
                let fuel_needed = (laps_remaining_in_race * session.fuel_per_lap)
                    + self.config.fuel_safety_margin;
                let fuel_diff = car.fuel_litres - fuel_needed;

                if fuel_diff < -1.0 {
                    recs.push(Recommendation {
                        component: "Strategy".tr(ru).to_string(),
                        category: "Race Finish".tr(ru).to_string(),
                        severity: Severity::Warning,
                        message: tr_fmt("Short {0} L", ru, &[&format!("{:.1}", fuel_diff.abs())]),
                        action: "Save Fuel / Box".tr(ru).to_string(),
                        parameters: vec![Parameter {
                            name: "Need".to_string(),
                            current: car.fuel_litres,
                            target: fuel_needed,
                            unit: "L".to_string(),
                        }],
                        confidence: 0.8,
                        chain: None,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    /// The simulator publishes zero wear until a lap is done, and so does a
    /// real session before its first update. Neither means the tyres are gone.
    #[test]
    fn wear_of_zero_on_every_corner_is_no_data_rather_than_four_dead_tyres() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);

        let mut car = Car {
            tyre_wear: [0.0; 4],
            ..Default::default()
        };
        let mut recs = Vec::new();
        engineer.analyze_tyre_wear(&car, &mut recs);
        assert!(recs.is_empty(), "no wear data means no wear advice");

        // One corner reporting something plausible is data, and a corner at 40%
        // in that state is worth saying out loud. Alerts are held back for a
        // second before they are reported, so the timer is aged by hand here
        // rather than by sleeping through it.
        car.tyre_wear = [98.0, 97.0, 40.0, 96.0];
        let aged = std::time::Instant::now() - std::time::Duration::from_secs(2);
        engineer
            .alert_timers
            .insert("wear_2".to_string(), (aged, std::time::Instant::now()));

        let mut recs = Vec::new();
        engineer.analyze_tyre_wear(&car, &mut recs);
        assert!(
            recs.iter().any(|rec| rec.message.contains("RL")),
            "the worn corner is reported once there is data: {recs:?}"
        );
    }
    use super::{Engineer, Severity};
    use crate::config::{AppConfig, PressureUnit};
    use crate::games::{Capabilities, Car, Session};

    /// Age every alert timer past the one-second hold, so a test does not have
    /// to sleep through it.
    fn age_the_alerts(engineer: &mut Engineer) {
        let aged = std::time::Instant::now() - std::time::Duration::from_secs(2);
        let now = std::time::Instant::now();
        for key in [
            "wear_0",
            "wear_1",
            "wear_2",
            "wear_3",
            "tyre_temp_0",
            "tyre_temp_1",
            "tyre_temp_2",
            "tyre_temp_3",
            "pres_0",
            "pres_1",
            "pres_2",
            "pres_3",
            "brake_temp_0",
            "brake_temp_1",
            "brake_temp_2",
            "brake_temp_3",
        ] {
            engineer.alert_timers.insert(key.to_string(), (aged, now));
        }
    }

    /// Every rule that has a mechanism states one, and the two that do not say
    /// why in prose next to the `None`.
    ///
    /// Read off the source rather than off the output, and deliberately so: a
    /// behavioural test can only check the rules it manages to trigger, and the
    /// failure this guards against is a *new* rule added six months from now
    /// with `chain: None` copied from its neighbour. There is no telemetry that
    /// would provoke a rule nobody has written yet; there is a `grep`.
    #[test]
    fn no_rule_outside_the_fuel_pair_ships_without_an_explanation() {
        let source = include_str!("engineer.rs");
        let strategy = source
            .find("fn analyze_strategy")
            .expect("analyze_strategy is where the two exceptions live");
        // The function runs to the end of the `impl` block, which is the last
        // thing in it — everything after `mod tests` is this file's own tests
        // and has no rules in it.
        let tests = source.find("\nmod tests {").unwrap_or(source.len());

        let unexplained = source[..strategy].matches("chain: None").count();
        assert_eq!(
            unexplained, 0,
            "a rule before analyze_strategy has no chain. Either give it a \
             cause, an effect and something to check next run, or move it \
             beside the fuel rules and write down why it cannot have one."
        );

        let excused = source[strategy..tests].matches("chain: None").count();
        assert_eq!(
            excused, 2,
            "the fuel rules are the only two without a chain; if that changed, \
             the doc comment on analyze_strategy has to change with it"
        );
    }

    /// The rules a driver sees most can all say why, what, and what to check.
    ///
    /// The companion to the test above: that one proves nothing was left out,
    /// this one proves what was put in is not three empty strings.
    #[test]
    fn the_advice_a_driver_actually_sees_carries_a_usable_chain() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        age_the_alerts(&mut engineer);

        // Over pressure, over temperature, cooking brakes and a worn set, all
        // at once — four different analysers, four different shapes of chain.
        let car = Car {
            tyre_pressure_psi: [31.0, 31.2, 30.8, 31.1],
            tyre_core_temp_c: [120.0; 4],
            tyre_temp_inner_c: [120.0; 4],
            tyre_temp_middle_c: [120.0; 4],
            tyre_temp_outer_c: [120.0; 4],
            brake_temp_c: [900.0, 910.0, 880.0, 895.0],
            tyre_wear: [80.0, 81.0, 79.0, 82.0],
            speed_kmh: 180.0,
            ..Default::default()
        };
        let session = Session {
            surface_grip: 1.0,
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_pressure(&car, &session, &mut recs);
        engineer.analyze_tyre_temperature(&car, &mut recs);
        engineer.analyze_brakes(&car, &mut recs);
        engineer.analyze_tyre_wear(&car, &mut recs);

        assert!(
            recs.len() >= 4,
            "this state should trip pressure, temperature, brakes and wear: {recs:?}"
        );
        for rec in &recs {
            // Phrased as an assertion rather than an unwrap because the
            // workspace denies `clippy::panic`, and a test is not exempt from a
            // lint that exists so a release build cannot abort on a driver
            // mid-race.
            assert!(
                rec.chain.is_some(),
                "{} produces no chain at all",
                rec.message
            );
            let Some(chain) = rec.chain.as_ref() else {
                continue;
            };
            // Captured by default, and printed anyway on purpose: the
            // simulator's telemetry sits inside every window, so
            // `engineer_probe` never trips these rules and there is nowhere
            // else to *read* the sentences they produce. `cargo test --
            // --nocapture` is that place. A `confirm` that is grammatical and
            // useless is not something an assertion can catch.
            println!(
                "{}\n    why:   {}\n    seen:  {}\n    check: {}",
                rec.message, chain.cause, chain.effect, chain.confirm
            );
            assert!(
                !chain.cause.trim().is_empty(),
                "{} states no mechanism",
                rec.message
            );
            assert!(
                !chain.effect.trim().is_empty(),
                "{} states no measurement",
                rec.message
            );
            // The field the whole idea rests on. Advice nobody can check is not
            // advice, and an empty string here is the failure that would be
            // easiest to ship without noticing.
            assert!(
                !chain.confirm.trim().is_empty(),
                "{} gives nothing to check next run",
                rec.message
            );
        }
    }

    /// Drive `phys` through `update` often enough for the camber average to
    /// have something behind it.
    /// The four flags, one at a time: a game that does not measure a thing
    /// must not have a verdict about it.
    ///
    /// This is the half of `Capabilities` that did not exist. The type was
    /// declared, documented and checked against a real capture, and then
    /// consulted by nobody — so on Assetto Corsa, which measures all four,
    /// nothing ever went wrong, and on the first game that does not it would
    /// have read four defaults as four measurements.
    mod capabilities {
        use super::*;

        /// A car in enough trouble that both gated rules have something to say
        /// about it, and the ungated ones do too.
        fn a_car_in_trouble() -> Car {
            Car {
                speed_kmh: 180.0,
                tyre_pressure_psi: [20.0; 4],
                tyre_wear: [70.0; 4],
                brake_temp_c: [950.0; 4],
                // Cooking, and the inner edge far hotter than the outer: too
                // much negative camber as well as too much heat.
                tyre_temp_inner_c: [140.0; 4],
                tyre_temp_middle_c: [130.0; 4],
                tyre_temp_outer_c: [118.0; 4],
                acc_g: [1.2, 1.0, 0.0],
                ..Default::default()
            }
        }

        /// "Tyres/Overheat", "Brakes/Overheat" — the component as well as the
        /// category, because two different rules both call their finding an
        /// overheat and only one of them rests on a tread temperature.
        fn advice_about(engineer: &mut Engineer, car: &Car) -> Vec<String> {
            age_the_alerts(engineer);
            drive(engineer, car, 120);
            age_the_alerts(engineer);
            engineer
                .analyze_live(car, &Session::default(), None)
                .into_iter()
                .map(|rec| format!("{}/{}", rec.component, rec.category))
                .collect()
        }

        /// Both halves in one: everything measured says all of it, and nothing
        /// measured says only what does not depend on a measurement the game
        /// withheld. The brakes are in both lists, which is the control — a
        /// silent engineer would pass a test that only checked for absence.
        #[test]
        fn a_game_that_measures_nothing_still_says_what_it_can() {
            let config = AppConfig::default();
            let car = a_car_in_trouble();

            let mut complete = engineer_reading_a_complete_game(&config);
            let said = advice_about(&mut complete, &car);
            for expected in [
                "Tyres/Overheat",
                "Suspension/Camber",
                "Tyres/Wear",
                "Brakes/Overheat",
            ] {
                assert!(said.iter().any(|c| c == expected), "{expected}: {said:?}");
            }
            // The pressure rule names the compound it judged against —
            // "Tyres (Racing)" — so it is matched by what it found.
            assert!(said.iter().any(|c| c.ends_with("/Pressure")), "{said:?}");

            let mut blind = Engineer::new(&config);
            let said = advice_about(&mut blind, &car);
            assert!(
                said.iter().any(|c| c == "Brakes/Overheat"),
                "the brakes rest on no withheld measurement: {said:?}"
            );
            assert!(
                said.iter().any(|c| c.ends_with("/Pressure")),
                "nor does the pressure: {said:?}"
            );
            for withheld in ["Tyres/Overheat", "Suspension/Camber", "Tyres/Wear"] {
                assert!(
                    !said.iter().any(|c| c == withheld),
                    "{withheld} rests on a measurement this game does not make: {said:?}"
                );
            }
        }

        /// Tyre wear alone. The rule reads a percentage that counts down from
        /// 100, and an unpublished one reads as zero — which is exactly the
        /// "four tyres WORN OUT" this project has already shipped once.
        #[test]
        fn without_tyre_wear_there_is_no_wear_verdict() {
            let config = AppConfig::default();
            let car = a_car_in_trouble();

            let mut engineer = Engineer::new(&config);
            engineer.update_capabilities(Capabilities {
                tyre_wear: false,
                ..Capabilities::all()
            });
            let said = advice_about(&mut engineer, &car);
            assert!(!said.iter().any(|c| c == "Tyres/Wear"), "{said:?}");
            assert!(
                said.iter().any(|c| c == "Tyres/Overheat"),
                "only the wear flag was taken away: {said:?}"
            );
        }

        /// Tread temperature alone, which gates two rules: the camber advice is
        /// inner minus outer, and the temperature band is the mean of the three.
        /// ACC publishes core temperature and neither of those.
        #[test]
        fn without_tread_temperatures_there_is_no_camber_or_temperature_verdict() {
            let config = AppConfig::default();
            let car = a_car_in_trouble();

            let mut engineer = Engineer::new(&config);
            engineer.update_capabilities(Capabilities {
                tyre_edge_temps: false,
                ..Capabilities::all()
            });
            let said = advice_about(&mut engineer, &car);
            assert!(!said.iter().any(|c| c == "Suspension/Camber"), "{said:?}");
            assert!(!said.iter().any(|c| c == "Tyres/Overheat"), "{said:?}");
            assert!(
                said.iter().any(|c| c == "Tyres/Wear"),
                "only the tread flag was taken away: {said:?}"
            );
            assert!(
                said.iter().any(|c| c == "Brakes/Overheat"),
                "the brakes are measured and still cooking: {said:?}"
            );
        }

        /// The stat behind the camber verdict has to stop being collected too.
        ///
        /// Withholding only at the rule would leave `camber_spread` averaging
        /// zero minus zero over a whole stint — a number that reads as a
        /// perfectly cambered car rather than as no reading, and the moment a
        /// flag was flipped back on it would be believed.
        #[test]
        fn an_unmeasured_tread_leaves_no_camber_history_behind() {
            let config = AppConfig::default();
            let car = a_car_in_trouble();

            let mut engineer = Engineer::new(&config);
            engineer.update_capabilities(Capabilities {
                tyre_edge_temps: false,
                ..Capabilities::all()
            });
            drive(&mut engineer, &car, 120);
            assert_eq!(engineer.stats.camber_frames, 0);
            assert_eq!(engineer.stats.camber_spread, [0.0; 4]);
        }
    }

    /// An engineer reading a game that measures everything.
    ///
    /// Which is what Assetto Corsa does, and what every test in this file
    /// assumed before the capability flags gated anything. Spelled out rather
    /// than defaulted: the point of the flags is that a rule resting on a
    /// measurement has to be told the measurement exists.
    fn engineer_reading_a_complete_game(config: &AppConfig) -> Engineer {
        let mut engineer = Engineer::new(config);
        engineer.update_capabilities(Capabilities::all());
        engineer
    }

    fn drive(engineer: &mut Engineer, car: &Car, ticks: u32) {
        let session = Session::default();
        let info = crate::session_info::SessionInfo::default();
        for _ in 0..ticks {
            engineer.update(car, &session, &info);
        }
    }

    /// A tyre says nothing about camber while the car is upright: both edges
    /// run the same temperature, the spread reads zero, and the old check read
    /// that as "contact patch inefficient" — four Info lines, on every
    /// straight, in the eight the panel has.
    #[test]
    fn a_straight_is_not_a_camber_problem() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);

        let car = Car {
            speed_kmh: 240.0,
            acc_g: [0.0, 1.0, 0.0],
            tyre_temp_inner_c: [90.0; 4],
            tyre_temp_outer_c: [90.0; 4],
            ..Default::default()
        };
        drive(&mut engineer, &car, 600);

        let mut recs = Vec::new();
        engineer.analyze_camber(&car, &mut recs);
        assert!(recs.is_empty(), "ten seconds of straight line: {recs:?}");
    }

    /// The same spread, measured while the tyre is loaded, is a real answer —
    /// and it is one line for all four corners rather than four.
    #[test]
    fn four_corners_wanting_camber_are_one_piece_of_advice() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);

        let car = Car {
            speed_kmh: 160.0,
            acc_g: [1.2, 1.0, 0.0],
            // Outer edge hotter than inner: not enough negative camber.
            tyre_temp_inner_c: [90.0; 4],
            tyre_temp_outer_c: [94.0; 4],
            camber_rad: [
                (-1.5f32).to_radians(),
                1.5f32.to_radians(),
                (-1.5f32).to_radians(),
                1.5f32.to_radians(),
            ],
            ..Default::default()
        };
        drive(&mut engineer, &car, 120);

        let mut recs = Vec::new();
        engineer.analyze_camber(&car, &mut recs);

        assert_eq!(recs.len(), 1, "one fact, one line: {recs:?}");
        assert!(
            recs[0].message.contains("All four"),
            "and it says which corners: {}",
            recs[0].message
        );
        // The number in the advice is the camber AC reports, not the setup
        // file's step index — `CAMBER_LF VALUE=-9` used to be printed as
        // "now: -9" beside a car the game showed at -1.3°.
        assert!(
            recs[0].action.contains("-1.5°"),
            "the advice names the camber the car is running: {}",
            recs[0].action
        );
    }

    /// Inner edges cooking is the other direction, and it is a warning rather
    /// than a note.
    #[test]
    fn inner_edges_cooking_ask_for_less_camber() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);

        let car = Car {
            speed_kmh: 160.0,
            acc_g: [-1.2, 1.0, 0.0],
            tyre_temp_inner_c: [110.0, 110.0, 90.0, 90.0],
            tyre_temp_outer_c: [90.0, 90.0, 82.0, 82.0],
            ..Default::default()
        };
        drive(&mut engineer, &car, 120);

        let mut recs = Vec::new();
        engineer.analyze_camber(&car, &mut recs);

        assert_eq!(recs.len(), 1, "{recs:?}");
        assert!(
            recs[0].message.contains("Fronts"),
            "only the fronts are cooking: {}",
            recs[0].message
        );
        assert_eq!(recs[0].severity, Severity::Warning);
        assert!(
            recs[0].action.contains("Less neg. camber"),
            "{}",
            recs[0].action
        );
    }

    /// AC reports camber in each wheel's own frame, so the two sides mirror.
    /// Reading them raw makes a symmetric car look like it has +1.5° on the
    /// right, which is a car about to spin rather than a normal setup.
    #[test]
    fn the_two_sides_of_the_car_report_camber_on_one_scale() {
        let car = Car {
            camber_rad: [
                (-1.3f32).to_radians(),
                1.3f32.to_radians(),
                (-2.0f32).to_radians(),
                2.0f32.to_radians(),
            ],
            ..Default::default()
        };
        for corner in 0..4 {
            assert!(
                Engineer::camber_degrees(&car, corner) < 0.0,
                "corner {corner} reads positive on a car with negative camber"
            );
        }
        assert!((Engineer::camber_degrees(&car, 0) + 1.3).abs() < 0.01);
        assert!((Engineer::camber_degrees(&car, 1) + 1.3).abs() < 0.01);
    }

    /// Four corners of one problem used to be four recommendations, which is
    /// every slot the overlay has spent saying one thing — and "FL COLD / FR
    /// COLD / RL COLD / RR COLD" reads as noise rather than as "the tyres are
    /// not up to temperature".
    #[test]
    fn four_cold_tyres_are_one_piece_of_advice() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        age_the_alerts(&mut engineer);

        let car = Car {
            speed_kmh: 180.0,
            tyre_temp_inner_c: [55.0; 4],
            tyre_temp_middle_c: [55.0; 4],
            tyre_temp_outer_c: [55.0; 4],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_temperature(&car, &mut recs);

        assert_eq!(recs.len(), 1, "one fact, one line: {recs:?}");
        assert!(
            recs[0].message.contains("All four"),
            "and it says which corners: {}",
            recs[0].message
        );
    }

    /// Two of four is still one line, and it names the axle rather than
    /// listing wheels.
    #[test]
    fn two_hot_fronts_are_named_as_an_axle() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        age_the_alerts(&mut engineer);

        let car = Car {
            speed_kmh: 180.0,
            tyre_temp_inner_c: [130.0, 130.0, 90.0, 90.0],
            tyre_temp_middle_c: [130.0, 130.0, 90.0, 90.0],
            tyre_temp_outer_c: [130.0, 130.0, 90.0, 90.0],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_temperature(&car, &mut recs);

        assert_eq!(recs.len(), 1);
        assert!(
            recs[0].message.contains("Fronts"),
            "expected an axle, got: {}",
            recs[0].message
        );
    }

    /// Cold at the front and hot at the back is two different problems, and
    /// grouping must not merge them.
    #[test]
    fn cold_and_hot_stay_separate_problems() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        age_the_alerts(&mut engineer);

        let car = Car {
            speed_kmh: 180.0,
            tyre_temp_inner_c: [50.0, 50.0, 130.0, 130.0],
            tyre_temp_middle_c: [50.0, 50.0, 130.0, 130.0],
            tyre_temp_outer_c: [50.0, 50.0, 130.0, 130.0],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_temperature(&car, &mut recs);

        assert_eq!(recs.len(), 2, "{recs:?}");
        assert!(recs.iter().any(|rec| rec.message.contains("Fronts")));
        assert!(recs.iter().any(|rec| rec.message.contains("Rears")));
    }

    /// The threshold that made the engineer cry wolf. `wear_warning - 2` meant
    /// a tyre at 93.9 % life — most of the way through a first stint — came
    /// back as CRITICAL "WORN OUT".
    #[test]
    fn a_tyre_most_of_the_way_through_a_stint_is_not_critical() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        age_the_alerts(&mut engineer);

        let car = Car {
            tyre_wear: [93.0; 4],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_wear(&car, &mut recs);

        assert_eq!(recs.len(), 1, "{recs:?}");
        assert_eq!(
            recs[0].severity,
            Severity::Warning,
            "93% life is a warning, not a critical: {}",
            recs[0].message
        );

        // And below the critical threshold it still is one.
        let car = Car {
            tyre_wear: [70.0; 4],
            ..Default::default()
        };
        let mut recs = Vec::new();
        engineer.analyze_tyre_wear(&car, &mut recs);
        assert_eq!(recs[0].severity, Severity::Critical, "{recs:?}");
    }

    /// "Laps remaining" starts as "not measured", and 99.0 was a bad way to
    /// say it — ninety-nine laps is also a perfectly good answer, so a fresh
    /// set on a short track read as having no data at all.
    #[test]
    fn tyre_life_says_not_measured_with_a_value_laps_cannot_take() {
        let engineer = Engineer::new(&AppConfig::default());
        for corner in 0..4 {
            assert!(
                engineer.stats.tyre_laps_remaining[corner] < 0.0,
                "corner {corner} should start unmeasured"
            );
        }
    }

    /// The pressure advice printed raw psi while the temperature advice next
    /// to it went through the formatter, so anyone working in bar read their
    /// pressures in one unit on the Dashboard and another in the advice about
    /// them.
    #[test]
    fn pressure_advice_is_in_the_unit_the_driver_chose() {
        let config = AppConfig {
            pressure_unit: PressureUnit::Bar,
            ..AppConfig::default()
        };
        let mut engineer = engineer_reading_a_complete_game(&config);
        age_the_alerts(&mut engineer);

        let car = Car {
            speed_kmh: 120.0,
            tyre_pressure_psi: [20.0; 4],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_tyre_pressure(&car, &Session::default(), &mut recs);

        assert_eq!(
            recs.len(),
            1,
            "four under-inflated tyres are one line: {recs:?}"
        );
        assert!(
            recs[0].message.contains("bar"),
            "expected bar, got: {}",
            recs[0].message
        );
        assert!(
            !recs[0].message.contains("psi"),
            "and not psi: {}",
            recs[0].message
        );
    }

    /// Both front brakes over temperature is one thing to say. It also has to
    /// name the wheels the way every other alert does — this used to number
    /// them "Brake 1" to "Brake 4".
    #[test]
    fn cooking_brakes_are_grouped_and_named_by_corner() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        age_the_alerts(&mut engineer);

        let car = Car {
            brake_temp_c: [950.0, 950.0, 300.0, 300.0],
            ..Default::default()
        };

        let mut recs = Vec::new();
        engineer.analyze_brakes(&car, &mut recs);

        assert_eq!(recs.len(), 1, "{recs:?}");
        assert!(recs[0].message.contains("Fronts"), "{}", recs[0].message);
        assert!(
            !recs[0].message.contains("Brake 1"),
            "corners are named, not numbered: {}",
            recs[0].message
        );
    }

    /// What all of this is for: the overlay carries eight lines, and a session
    /// with several things wrong has to fit more than one of them in.
    #[test]
    fn several_problems_at_once_still_leave_room_for_each_other() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        age_the_alerts(&mut engineer);

        let car = Car {
            speed_kmh: 180.0,
            tyre_pressure_psi: [20.0; 4],
            tyre_wear: [70.0; 4],
            brake_temp_c: [950.0; 4],
            tyre_temp_inner_c: [130.0; 4],
            tyre_temp_middle_c: [130.0; 4],
            tyre_temp_outer_c: [130.0; 4],
            ..Default::default()
        };

        let recs = engineer.analyze_live(&car, &Session::default(), None);

        // Four distinct problems. Ungrouped this was sixteen lines, and the
        // four the overlay publishes were all about tyre temperature.
        for expected in ["pressure", "WORN OUT", "brakes cooking", "OVERHEATING"] {
            assert!(
                recs.iter().any(|rec| rec.message.contains(expected)),
                "'{expected}' did not survive into the top of the list: {:?}",
                recs.iter().map(|r| &r.message).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn tyre_pressure_alert_uses_updated_configuration() {
        let mut config = AppConfig::default();
        config.alerts.tyre_pressure_max = 31.0;
        let mut engineer = engineer_reading_a_complete_game(&config);
        let car = Car {
            speed_kmh: 60.0,
            tyre_pressure_psi: [30.0; 4],
            ..Default::default()
        };
        let session = Session::default();

        let recommendations = engineer.analyze_live(&car, &session, None);
        assert!(!recommendations.iter().any(|rec| rec.category == "Pressure"));

        config.alerts.tyre_pressure_max = 29.0;
        engineer.update_config(&config);

        let past = std::time::Instant::now() - std::time::Duration::from_secs(2);
        for i in 0..4 {
            engineer
                .alert_timers
                .insert(format!("pres_{}", i), (past, past));
        }

        let recommendations = engineer.analyze_live(&car, &session, None);
        assert!(recommendations.iter().any(|rec| rec.category == "Pressure"));
    }

    /// The brake and tyre-temperature alerts had no hysteresis, unlike the
    /// pressure and wear alerts, so they pushed a fresh recommendation on
    /// every frame the condition held.
    #[test]
    fn overheating_alerts_are_not_repeated_every_frame() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        let session = Session::default();
        let car = Car {
            speed_kmh: 150.0,
            brake_temp_c: [1500.0; 4],
            ..Default::default()
        };

        // The gate needs a second of sustained condition before it fires at
        // all, so the first burst produces nothing.
        let mut total = 0;
        for _ in 0..50 {
            total += engineer
                .analyze_live(&car, &session, None)
                .iter()
                .filter(|rec| rec.category == "Overheat")
                .count();
        }
        assert_eq!(
            total, 0,
            "an alert should need a sustained condition, not a single frame"
        );
    }

    /// AC's own fuel_x_lap sits in the unverified tail of the graphics page
    /// and reads zero on lap one regardless, so the whole strategy tab was
    /// gated on a field that may never be populated.
    #[test]
    fn fuel_estimate_falls_back_to_measured_consumption() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        let info = crate::session_info::SessionInfo::default();

        // AC reports nothing, as it does on lap one and as it would
        // permanently if that offset is wrong.
        let session = |laps| Session {
            completed_laps: laps,
            fuel_per_lap: 0.0,
            ..Default::default()
        };
        let car = |fuel| Car {
            fuel_litres: fuel,
            speed_kmh: 120.0,
            ..Default::default()
        };

        // Start the stint with a full tank.
        engineer.update(&car(50.0), &session(0), &info);
        assert_eq!(
            engineer.stats.fuel_laps_remaining, 0.0,
            "nothing measured yet, so no estimate is claimed"
        );

        // Two laps at 2.5 L each.
        engineer.update(&car(47.5), &session(1), &info);
        engineer.update(&car(45.0), &session(2), &info);

        assert!(
            (engineer.stats.fuel_consumption_rate - 2.5).abs() < 0.01,
            "measured burn, got {}",
            engineer.stats.fuel_consumption_rate
        );
        assert!(
            (engineer.stats.fuel_laps_remaining - 18.0).abs() < 0.1,
            "45 L at 2.5 L/lap is 18 laps, got {}",
            engineer.stats.fuel_laps_remaining
        );
    }

    /// Burn measured across a refuel is meaningless, and the stale estimate
    /// it produced could fire BOX BOX BOX on a car that had just filled up.
    #[test]
    fn refuelling_discards_the_measured_history() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        let info = crate::session_info::SessionInfo::default();
        let session = |laps| Session {
            completed_laps: laps,
            ..Default::default()
        };
        let car = |fuel| Car {
            fuel_litres: fuel,
            speed_kmh: 120.0,
            ..Default::default()
        };

        engineer.update(&car(20.0), &session(0), &info);
        engineer.update(&car(17.5), &session(1), &info);
        assert!(engineer.stats.fuel_laps_remaining > 0.0);

        // Pit stop: the tank goes up.
        engineer.update(&car(60.0), &session(2), &info);
        assert_eq!(
            engineer.stats.fuel_laps_remaining, 0.0,
            "the estimate is dropped rather than carried across the stop"
        );
    }

    /// acc_g is [lateral, vertical, longitudinal]. Aggression used indices 0
    /// and 1, so it summed cornering with the ~1 g of gravity the car carries
    /// even parked, and never looked at braking or acceleration.
    #[test]
    fn aggression_ignores_the_vertical_axis() {
        let config = AppConfig::default();
        let mut engineer = engineer_reading_a_complete_game(&config);
        let session = Session::default();
        let info = crate::session_info::SessionInfo::default();

        // Straight line, steady speed, 1 g down. Nothing aggressive here.
        let cruising = Car {
            speed_kmh: 120.0,
            acc_g: [0.0, 1.0, 0.0],
            ..Default::default()
        };
        for _ in 0..200 {
            engineer.update(&cruising, &session, &info);
        }
        let cruising_aggression = engineer.driving_style.aggression;
        assert!(
            cruising_aggression < 5.0,
            "vertical g must not register as aggression, got {cruising_aggression}"
        );

        // Hard braking. This is the case the old formula could not see at all,
        // because longitudinal g lives at index 2.
        let braking = Car {
            speed_kmh: 120.0,
            acc_g: [0.0, 1.0, -1.8],
            ..Default::default()
        };
        for _ in 0..200 {
            engineer.update(&braking, &session, &info);
        }
        assert!(
            engineer.driving_style.aggression > cruising_aggression + 20.0,
            "braking must raise aggression, got {}",
            engineer.driving_style.aggression
        );
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TyreCornerAdjustment {
    pub corner_name: String,
    pub current_psi: f32,
    pub recommended_delta_psi: f32,
    pub temp_spread_c: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TyrePressureOptimizer {
    pub corners: [TyreCornerAdjustment; 4],
}

impl TyrePressureOptimizer {
    pub fn calculate(car: &Car, target_psi: f32) -> Self {
        let labels = ["FL", "FR", "RL", "RR"];
        let mut corners = Vec::with_capacity(4);

        for (i, label) in labels.iter().enumerate() {
            let p_psi = car.tyre_pressure_psi[i];
            let t_i = car.tyre_temp_inner_c[i];
            let t_o = car.tyre_temp_outer_c[i];
            let spread = t_i - t_o;
            let p_delta = target_psi - p_psi;

            let rec_delta = if p_delta.abs() > 0.5 {
                (p_delta * 10.0).round() / 10.0
            } else if spread > 12.0 {
                -0.3
            } else if spread < -8.0 {
                0.3
            } else {
                0.0
            };

            corners.push(TyreCornerAdjustment {
                corner_name: (*label).to_string(),
                current_psi: p_psi,
                recommended_delta_psi: rec_delta,
                temp_spread_c: spread,
            });
        }

        Self {
            corners: [
                corners[0].clone(),
                corners[1].clone(),
                corners[2].clone(),
                corners[3].clone(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdPressureEstimate {
    pub target_hot_psi: f32,
    pub recommended_cold_psi: f32,
    pub delta_temp_psi: f32,
    pub delta_grip_psi: f32,
}

pub struct ColdPressureCalculator;

impl ColdPressureCalculator {
    pub fn calculate(
        target_hot_psi: f32,
        ambient_temp_c: f32,
        track_grip: f32,
    ) -> ColdPressureEstimate {
        let temp_diff = (85.0 - ambient_temp_c.clamp(0.0, 50.0)).max(0.0);
        let delta_temp_psi = temp_diff * 0.08;
        let grip_norm = track_grip.clamp(0.80, 1.0);
        let delta_grip_psi = (1.0 - grip_norm) * 1.2;

        let recommended_cold_psi = (target_hot_psi - delta_temp_psi - delta_grip_psi).max(15.0);

        ColdPressureEstimate {
            target_hot_psi,
            recommended_cold_psi: (recommended_cold_psi * 10.0).round() / 10.0,
            delta_temp_psi: (delta_temp_psi * 10.0).round() / 10.0,
            delta_grip_psi: (delta_grip_psi * 10.0).round() / 10.0,
        }
    }
}
