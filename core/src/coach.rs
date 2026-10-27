//! One thing to fix, and whether it got fixed.
//!
//! **A list of everything that went wrong is a list nobody works through.**
//! The corner table says where every tenth went and the braking analysis
//! names a fault at every corner that has one — both right, and both more
//! than anybody can act on between one lap and the next. A coach beside the
//! track does the opposite: picks the one corner that cost the most, says the
//! one thing to do differently there, and watches the next lap to see whether
//! it worked.
//!
//! That is all this does. After each lap it compares the lap with the
//! session's best, takes the corner that lost the most, and turns the
//! measurement into a sentence a driver can carry into the next lap — "brake
//! 12 m later into T5", not "T5: −0.31 s". Then it stays on that corner:
//!
//! * **fixed** when the corner costs half what it did, or under a twentieth —
//!   said, and the next corner is picked;
//! * **given up on** after four laps without that — also said, because
//!   hammering one corner for an evening is how people stop listening, and
//!   another corner is usually where the next tenth is.
//!
//! # What it decides
//!
//! Which corner to talk about, and in what words. The measurements are
//! [`crate::corners`] and [`crate::braking`]'s, unchanged — this is the
//! sentence on top, the same split the engineer and the debrief keep between
//! what was measured and what is said about it.

use crate::analyzer::LapData;
use crate::braking::{self, Fault};
use crate::corners::{self, CornerComparison};
use crate::i18n::{Translate, tr_fmt};

/// Below this a corner is not worth a coach's breath.
const WORTH_MS: i32 = 60;

/// How many laps a corner is kept in focus without improving.
const PATIENCE: usize = 4;

/// Where on the track two laps' corners count as the same corner.
const SAME_CORNER: f32 = 0.015;

/// Which part of the corner the time went in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Braking,
    Middle,
    Exit,
}

/// How the last thing worked out.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// A new corner, nothing to compare yet.
    New,
    /// The same corner, and what it cost last lap. `laps` is how many laps
    /// have already been driven on it since it was picked.
    Again { last_ms: i32, laps: usize },
    /// The previous corner is fixed: from what, to what.
    Fixed {
        corner: usize,
        from_ms: i32,
        to_ms: i32,
    },
    /// Four laps on the previous corner without it coming.
    MovedOn { corner: usize },
}

/// The one thing.
#[derive(Debug, Clone, PartialEq)]
pub struct Focus {
    /// The corner, as the corner table numbers it.
    pub corner: usize,
    /// What it cost against the best lap, in milliseconds.
    pub lost_ms: i32,
    pub phase: Option<Phase>,
    /// What to do: "Brake 12 m later into T5".
    pub instruction: String,
    /// The measurement behind it.
    pub because: String,
    /// How to know next lap whether it worked.
    pub check: String,
    pub progress: Progress,
}

impl Focus {
    /// What happened to the last one, as a sentence, when there is anything
    /// to say.
    pub fn progress_line(&self, ru: bool) -> Option<String> {
        let seconds = |ms: i32| format!("{:.2} s", ms as f32 / 1000.0);
        match &self.progress {
            Progress::New => None,
            Progress::Again { last_ms, laps } => Some(tr_fmt(
                "lap {0} on this corner: {1} last time, {2} now",
                ru,
                &[
                    &(laps + 2).to_string(),
                    &seconds(*last_ms),
                    &seconds(self.lost_ms),
                ],
            )),
            Progress::Fixed {
                corner,
                from_ms,
                to_ms,
            } => Some(tr_fmt(
                "T{0} fixed: {1} down to {2}",
                ru,
                &[&corner.to_string(), &seconds(*from_ms), &seconds(*to_ms)],
            )),
            Progress::MovedOn { corner } => Some(tr_fmt(
                "T{0} has not come in four laps — leave it for now",
                ru,
                &[&corner.to_string()],
            )),
        }
    }
}

impl Focus {
    /// The category every coaching line carries, so a front end can tell it
    /// from the engineer's own findings without matching on translated words.
    pub const CATEGORY: &'static str = "Coach";

    /// The one thing, as a finding the engineer's list can carry.
    ///
    /// **So every place that already draws findings draws this**: the
    /// terminal's engineer list, the window, the in-game panel, the overlay
    /// and a watcher's screens. A warning when the corner costs three tenths
    /// or more, otherwise information — it is a lesson, not an alarm.
    pub fn as_recommendation(&self, ru: bool) -> crate::engineer::Recommendation {
        crate::engineer::Recommendation {
            component: "Coach".tr(ru).to_string(),
            category: Self::CATEGORY.to_string(),
            severity: if self.lost_ms >= 300 {
                crate::engineer::Severity::Warning
            } else {
                crate::engineer::Severity::Info
            },
            message: self.instruction.clone(),
            action: self.check.clone(),
            parameters: Vec::new(),
            confidence: 0.8,
            chain: Some(crate::engineer::Chain {
                cause: self.because.clone(),
                effect: self
                    .progress_line(ru)
                    .unwrap_or_else(|| self.because.clone()),
                confirm: self.check.clone(),
                evidence: Default::default(),
            }),
        }
    }
}

/// The lap to learn from: one somebody chose, when it is of this car at
/// this track, and the session's own best otherwise.
///
/// **A chosen lap is the whole point of choosing one.** A driver who loads a
/// faster friend's lap, or one from the library, wants to be told where *that*
/// lap was quicker — not where their own best was. But a lap of another car,
/// or another circuit, is not a reference for anything here, and is passed
/// over rather than compared corner by corner with a different track.
pub fn reference<'a>(
    laps: &'a [LapData],
    best: Option<usize>,
    chosen: Option<&'a LapData>,
) -> Option<&'a LapData> {
    let latest = laps.last()?;
    chosen
        .filter(|lap| {
            !lap.telemetry_trace.is_empty()
                && lap.lap_time_ms > 0
                && lap.car_model == latest.car_model
                && lap.track_name == latest.track_name
        })
        .or_else(|| best.and_then(|at| laps.get(at)))
}

/// The corner being worked on.
#[derive(Debug, Clone)]
struct Working {
    corner: usize,
    apex: f32,
    first_ms: i32,
    last_ms: i32,
    laps: usize,
}

/// Keeps one corner in focus across laps.
#[derive(Debug, Clone, Default)]
pub struct Coach {
    working: Option<Working>,
}

impl Coach {
    /// Forget the corner — a new car or a new track.
    pub fn forget(&mut self) {
        self.working = None;
    }

    /// Look at a finished lap against the reference, and say the one thing.
    ///
    /// `None` when there is nothing to compare — no trace, the reference is
    /// this lap — or nothing worth a word: a lap within a twentieth of the
    /// reference at every corner is a lap to leave alone.
    pub fn after_lap(&mut self, lap: &LapData, reference: &LapData, ru: bool) -> Option<Focus> {
        if lap.telemetry_trace.is_empty()
            || reference.telemetry_trace.is_empty()
            || (lap.lap_number == reference.lap_number && lap.lap_time_ms == reference.lap_time_ms)
        {
            return None;
        }
        let mine = corners::detect(&lap.telemetry_trace);
        let theirs = corners::detect(&reference.telemetry_trace);
        let split = corners::decompose(
            &lap.telemetry_trace,
            &reference.telemetry_trace,
            &mine,
            &theirs,
        );
        let report = braking::look(
            &split,
            &lap.telemetry_trace,
            &reference.telemetry_trace,
            lap.track_length_m,
        );
        self.choose(&split, &report, lap, reference, ru)
    }

    fn choose(
        &mut self,
        split: &corners::Decomposition,
        report: &braking::Report,
        lap: &LapData,
        reference: &LapData,
        ru: bool,
    ) -> Option<Focus> {
        let mut progress = Progress::New;

        // The corner already in focus, if it is still on this lap.
        if let Some(working) = self.working.clone() {
            let now = split
                .sections
                .iter()
                .find(|section| (section.corner.apex - working.apex).abs() < SAME_CORNER);
            match now {
                Some(section)
                    if section.delta_ms <= (working.first_ms / 2).max(WORTH_MS / 2)
                        || section.delta_ms < 50 =>
                {
                    progress = Progress::Fixed {
                        corner: working.corner,
                        from_ms: working.first_ms,
                        to_ms: section.delta_ms.max(0),
                    };
                    self.working = None;
                }
                Some(section) if working.laps + 1 < PATIENCE => {
                    let focus = describe(
                        section,
                        split,
                        report,
                        lap,
                        reference,
                        Progress::Again {
                            last_ms: working.last_ms,
                            laps: working.laps,
                        },
                        ru,
                    );
                    self.working = Some(Working {
                        last_ms: section.delta_ms,
                        laps: working.laps + 1,
                        ..working
                    });
                    return Some(focus);
                }
                _ => {
                    progress = Progress::MovedOn {
                        corner: working.corner,
                    };
                    self.working = None;
                }
            }
        }

        // A new corner: the one that cost the most, if it cost enough — and
        // not the one just given up on.
        let given_up = match &progress {
            Progress::MovedOn { corner } => Some(*corner),
            _ => None,
        };
        let worst = split
            .losses_over(WORTH_MS as f32 / 1000.0)
            .into_iter()
            .find(|section| Some(section.corner.number) != given_up)?;
        self.working = Some(Working {
            corner: worst.corner.number,
            apex: worst.corner.apex,
            first_ms: worst.delta_ms,
            last_ms: worst.delta_ms,
            laps: 0,
        });
        Some(describe(worst, split, report, lap, reference, progress, ru))
    }
}

/// The sentence for one corner.
fn describe(
    section: &CornerComparison,
    split: &corners::Decomposition,
    report: &braking::Report,
    lap: &LapData,
    reference: &LapData,
    progress: Progress,
    ru: bool,
) -> Focus {
    let number = section.corner.number;
    let corner = format!("T{number}");
    let seconds = |ms: i32| format!("{:.2} s", ms as f32 / 1000.0);

    // Which part of the corner, from the clock read at entry, apex and exit.
    let index = split
        .sections
        .iter()
        .position(|candidate| candidate.corner.number == number);
    let section_end = index
        .and_then(|at| split.sections.get(at + 1))
        .map(|next| next.corner.entry)
        .unwrap_or(f32::MAX);
    let inside = section.inside(
        &lap.telemetry_trace,
        &reference.telemetry_trace,
        section_end,
    );
    let worst_part = inside.as_ref().and_then(|inside| inside.worst());
    let phase = worst_part.map(|(part, _)| match part {
        "braking" => Phase::Braking,
        "through the middle" => Phase::Middle,
        _ => Phase::Exit,
    });

    // Whose lap this is measured against: the session's own best, or a lap
    // somebody chose to learn from — loaded from a file or the library.
    let against = if reference.from_file {
        "the reference lap"
    } else {
        "your best lap"
    }
    .tr(ru)
    .to_string();

    let fault = report
        .corners
        .iter()
        .find(|at| at.number == number)
        .and_then(|at| at.fault.map(|fault| (fault, at)));
    let track_m = lap.track_length_m;

    let instruction = match (fault, phase) {
        // The braking analysis named it: say what it found, in the units a
        // driver can feel.
        (Some((Fault::Early, at)), _) => match at.brake_delta_m.filter(|m| *m < -2.0) {
            Some(metres) => tr_fmt(
                "Brake {0} m later into {1}",
                ru,
                &[&format!("{:.0}", -metres), &corner],
            ),
            None => tr_fmt("Brake later into {0}", ru, &[&corner]),
        },
        (Some((Fault::Soft, at)), _) => match at.decel_delta_g.filter(|g| *g < 0.0) {
            Some(g) => tr_fmt(
                "Brake harder into {0} — {1} g short of {2}",
                ru,
                &[&corner, &format!("{:.2}", -g), &against],
            ),
            None => tr_fmt("Brake harder into {0}", ru, &[&corner]),
        },
        (Some((Fault::Released, _)), _) => tr_fmt(
            "Ease off the brake into {0} rather than letting go in one movement",
            ru,
            &[&corner],
        ),
        (Some((Fault::Trailed, at)), _) => match at.trail_delta_ms.filter(|ms| *ms > 0) {
            Some(ms) => tr_fmt(
                "Release the brake {0} sooner in {1}",
                ru,
                &[&seconds(ms), &corner],
            ),
            None => tr_fmt("Release the brake sooner in {0}", ru, &[&corner]),
        },
        (Some((Fault::OverSlowed, _)), _) | (None, Some(Phase::Middle)) => {
            match section
                .speed_deltas()
                .map(|(_, apex, _)| apex)
                .filter(|kmh| *kmh < -1.0)
            {
                Some(kmh) => tr_fmt(
                    "Carry {0} km/h more through the middle of {1}",
                    ru,
                    &[&format!("{:.0}", -kmh), &corner],
                ),
                None => tr_fmt("Carry more speed through the middle of {0}", ru, &[&corner]),
            }
        }
        (None, Some(Phase::Exit)) => match section.throttle_delta_s().filter(|s| *s > 0.05) {
            Some(late) => tr_fmt(
                "Back on the throttle {0} sooner out of {1}",
                ru,
                &[&format!("{late:.2} s"), &corner],
            ),
            None => match section
                .speed_deltas()
                .map(|(_, _, exit)| exit)
                .filter(|kmh| *kmh < -1.0)
            {
                Some(kmh) => tr_fmt(
                    "Leave {0} {1} km/h faster — the straight after it is where it is paid for",
                    ru,
                    &[&corner, &format!("{:.0}", -kmh)],
                ),
                None => tr_fmt("Get on the power earlier out of {0}", ru, &[&corner]),
            },
        },
        (None, Some(Phase::Braking)) => {
            match section.braking_delta_m(track_m).filter(|m| *m < -2.0) {
                Some(metres) => tr_fmt(
                    "Brake {0} m later into {1}",
                    ru,
                    &[&format!("{:.0}", -metres), &corner],
                ),
                None => tr_fmt("Arrive at {0} with more speed", ru, &[&corner]),
            }
        }
        (None, None) => tr_fmt(
            "{0} is where the lap went — compare it with your best on the corner view",
            ru,
            &[&corner],
        ),
    };

    let because = match worst_part {
        Some((part, ms)) => tr_fmt(
            "{0} cost {1} against {2} — {3} of it {4}",
            ru,
            &[
                &corner,
                &seconds(section.delta_ms),
                &against,
                &seconds(ms),
                part.tr(ru),
            ],
        ),
        None => tr_fmt(
            "{0} cost {1} against {2}",
            ru,
            &[&corner, &seconds(section.delta_ms), &against],
        ),
    };
    let check = tr_fmt(
        "next lap: {0} under {1} lost means it worked",
        ru,
        &[&corner, &seconds((section.delta_ms / 2).max(50))],
    );

    Focus {
        corner: number,
        lost_ms: section.delta_ms,
        phase,
        instruction,
        because,
        check,
        progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::TelemetryPoint;
    use crate::corners::{Corner, Decomposition};

    fn corner(number: usize, apex: f32) -> Corner {
        Corner {
            number,
            direction: corners::Direction::Left,
            entry: apex - 0.02,
            apex,
            exit: apex + 0.02,
            entry_speed: 150.0,
            min_speed: 90.0,
            exit_speed: 140.0,
            peak_lat_g: 1.5,
            brake_point: None,
            braking: None,
            throttle_point: None,
            throttle_delay_ms: None,
            entry_time_ms: 0,
            exit_time_ms: 0,
        }
    }

    fn section(number: usize, apex: f32, delta_ms: i32) -> CornerComparison {
        CornerComparison {
            corner: corner(number, apex),
            reference: Some(corner(number, apex)),
            delta_ms,
        }
    }

    fn split(deltas: &[(usize, f32, i32)]) -> Decomposition {
        Decomposition {
            sections: deltas
                .iter()
                .map(|(number, apex, ms)| section(*number, *apex, *ms))
                .collect(),
            opening_ms: 0,
            total_ms: deltas.iter().map(|(_, _, ms)| ms).sum(),
        }
    }

    fn lap(number: i32) -> LapData {
        LapData {
            lap_number: number,
            lap_time_ms: 90_000,
            telemetry_trace: vec![point(0.0), point(1.0)],
            ..Default::default()
        }
    }

    fn point(distance: f32) -> TelemetryPoint {
        TelemetryPoint {
            distance,
            time_ms: (distance * 90_000.0) as i32,
            speed: 0.0,
            gas: 0.0,
            brake: 0.0,
            gear: 0,
            steer: 0.0,
            lat_g: 0.0,
            lon_g: 0.0,
            slip_avg: 0.0,
            x: 0.0,
            y: 0.0,
            rpms: 0,
            detail: crate::analyzer::Detail::default(),
        }
    }

    /// **The corner that cost the most, and only it.**
    #[test]
    fn the_worst_corner_is_the_one_thing() {
        let mut coach = Coach::default();
        let found = coach
            .choose(
                &split(&[(1, 0.1, 40), (2, 0.3, 310), (3, 0.6, 120)]),
                &braking::Report::default(),
                &lap(3),
                &lap(1),
                false,
            )
            .expect("a corner worth a word");
        assert_eq!(found.corner, 2);
        assert_eq!(found.lost_ms, 310);
        assert_eq!(found.progress, Progress::New);
        assert!(
            found.because.starts_with("T2 cost 0.31 s"),
            "{}",
            found.because
        );
        assert!(found.check.contains("under 0.16 s"), "{}", found.check);
    }

    /// Nothing worth saying is said as nothing.
    #[test]
    fn a_clean_lap_has_no_one_thing() {
        let mut coach = Coach::default();
        assert!(
            coach
                .choose(
                    &split(&[(1, 0.1, 40), (2, 0.3, 20)]),
                    &braking::Report::default(),
                    &lap(3),
                    &lap(1),
                    false,
                )
                .is_none()
        );
    }

    /// **It stays on the corner, and says when it is fixed.**
    #[test]
    fn a_corner_is_followed_until_it_is_fixed() {
        let mut coach = Coach::default();
        let report = braking::Report::default();
        coach.choose(
            &split(&[(2, 0.3, 300), (3, 0.6, 150)]),
            &report,
            &lap(3),
            &lap(1),
            false,
        );

        let again = coach
            .choose(
                &split(&[(2, 0.3, 240), (3, 0.6, 150)]),
                &report,
                &lap(4),
                &lap(1),
                false,
            )
            .expect("still on T2");
        assert_eq!(again.corner, 2);
        assert_eq!(
            again.progress,
            Progress::Again {
                last_ms: 300,
                laps: 0
            }
        );
        assert_eq!(
            again.progress_line(false).as_deref(),
            Some("lap 2 on this corner: 0.30 s last time, 0.24 s now")
        );

        let next = coach
            .choose(
                &split(&[(2, 0.3, 120), (3, 0.6, 150)]),
                &report,
                &lap(5),
                &lap(1),
                false,
            )
            .expect("on to the next");
        assert_eq!(next.corner, 3, "T2 is fixed, so T3 is next");
        assert_eq!(
            next.progress,
            Progress::Fixed {
                corner: 2,
                from_ms: 300,
                to_ms: 120
            }
        );
    }

    /// Four laps without it coming, and it moves on — to another corner.
    #[test]
    fn a_corner_that_will_not_come_is_left() {
        let mut coach = Coach::default();
        let report = braking::Report::default();
        let stuck = split(&[(2, 0.3, 300), (3, 0.6, 150)]);
        for number in 3..7 {
            let found = coach
                .choose(&stuck, &report, &lap(number), &lap(1), false)
                .expect("a corner");
            assert_eq!(found.corner, 2, "lap {number}");
        }
        let moved = coach
            .choose(&stuck, &report, &lap(7), &lap(1), false)
            .expect("another corner");
        assert_eq!(moved.corner, 3);
        assert_eq!(moved.progress, Progress::MovedOn { corner: 2 });
    }

    /// The best lap is not compared with itself.
    #[test]
    fn the_reference_is_not_its_own_lesson() {
        let mut coach = Coach::default();
        assert!(coach.after_lap(&lap(1), &lap(1), false).is_none());
    }

    /// **A chosen lap of this car and track is learned from; another car's
    /// is not.**
    #[test]
    fn a_chosen_reference_is_used_when_it_is_the_same_car_and_track() {
        let mut mine = vec![lap(1), lap(2)];
        for lap in &mut mine {
            lap.car_model = "bmw_z4_gt3".to_string();
            lap.track_name = "spa".to_string();
        }
        let mut friend = lap(9);
        friend.car_model = "bmw_z4_gt3".to_string();
        friend.track_name = "spa".to_string();
        friend.from_file = true;
        assert_eq!(
            reference(&mine, Some(0), Some(&friend)).map(|lap| lap.lap_number),
            Some(9)
        );

        friend.track_name = "monza".to_string();
        assert_eq!(
            reference(&mine, Some(0), Some(&friend)).map(|lap| lap.lap_number),
            Some(1),
            "another circuit is not a reference here"
        );
        assert_eq!(
            reference(&mine, Some(0), None).map(|lap| lap.lap_number),
            Some(1)
        );
    }

    /// And the sentence says whose lap it was.
    #[test]
    fn the_sentence_names_the_reference() {
        let mut coach = Coach::default();
        let mut friend = lap(1);
        friend.from_file = true;
        let found = coach
            .choose(
                &split(&[(2, 0.3, 300)]),
                &braking::Report::default(),
                &lap(3),
                &friend,
                false,
            )
            .expect("a corner");
        assert!(
            found.because.contains("against the reference lap"),
            "{}",
            found.because
        );
    }

    /// The braking analysis's finding becomes an instruction in metres.
    #[test]
    fn an_early_brake_is_said_in_metres() {
        let mut coach = Coach::default();
        let report = braking::Report {
            corners: vec![braking::AtCorner {
                number: 2,
                delta_ms: 300,
                entry_loss_ms: Some(250),
                start: None,
                reference_start: None,
                brake_delta_m: Some(-12.4),
                decel_delta_g: None,
                trail_delta_ms: None,
                min_speed_delta: None,
                peak_decel_g: None,
                reference_decel_g: None,
                fault: Some(Fault::Early),
            }],
        };
        let found = coach
            .choose(&split(&[(2, 0.3, 300)]), &report, &lap(3), &lap(1), false)
            .expect("a corner");
        assert_eq!(found.instruction, "Brake 12 m later into T2");
    }
}
