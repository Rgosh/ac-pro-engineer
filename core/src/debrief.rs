//! What the engineer says about a lap that is over.
//!
//! The live engineer in [`crate::engineer`] answers "what is happening"; this
//! answers "what happened", from a [`LapData`] the analyser has finished with.
//! The two are different questions and want different thresholds: a tyre 3 °C
//! outside its window for one corner is noise live and is worth a line in a
//! debrief, because the number here is already an average over a whole lap.
//!
//! This existed before, inside `render_sector_advice` in the terminal's
//! Engineer tab — five hundred lines where the analysis and the ratatui spans
//! were the same code. That made it unusable anywhere else, which is why the
//! in-game panel had live advice and nothing after a lap: there was no way to
//! get at the sentences without drawing them into a terminal. It is data now,
//! and both the terminal and the overlay render the same values.

use crate::analyzer::LapData;
use crate::config::AppConfig;
use crate::engineer::{Recommendation, Severity};
use crate::i18n::Translate;

/// The corners, in the order every array in AC's physics page uses.
const CORNER_NAMES: [&str; 4] = ["FL", "FR", "RL", "RR"];

/// Name a group of corners the way a person would.
///
/// Same shape as the live engineer's version and for the same reason: four
/// lines that each say one wheel is four lines about one problem.
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

/// Everything worth saying about one finished lap, most severe first.
///
/// Ordered rather than merely collected: the panel draws the first few and the
/// terminal draws them all, so which line is first decides what a driver reads
/// at a glance.
pub fn debrief(lap: &LapData, config: &AppConfig) -> Vec<Recommendation> {
    let ru = config.language == crate::config::Language::Russian;
    let fmt = config.formatter();
    let alerts = &config.alerts;
    let mut out: Vec<Recommendation> = Vec::new();

    let mut push =
        |component: &str, category: &str, severity: Severity, message: String, action: String| {
            out.push(Recommendation {
                component: component.to_string(),
                category: category.to_string(),
                severity,
                message,
                action,
                parameters: Vec::new(),
                confidence: 0.9,
                chain: None,
            });
        };

    // --- pressures -------------------------------------------------------
    //
    // Averaged over the lap, so the target is the middle of the configured
    // band rather than either edge of it.
    let target = (alerts.tyre_pressure_min + alerts.tyre_pressure_max) / 2.0;
    let mut over: Vec<usize> = Vec::new();
    let mut under: Vec<usize> = Vec::new();
    for corner in 0..4 {
        let psi = lap.avg_wheels_pressure[corner];
        // A lap that published no pressure at all is not a lap with flat
        // tyres. The live engineer learned this about wear; the same zero
        // arrives here from a session that ended before the analyser had
        // anything to average.
        if psi <= 0.0 {
            continue;
        }
        if psi > alerts.tyre_pressure_max {
            over.push(corner);
        } else if psi < alerts.tyre_pressure_min {
            under.push(corner);
        }
    }
    for (corners, high) in [(&over, true), (&under, false)] {
        if corners.is_empty() {
            continue;
        }
        let average = corners
            .iter()
            .map(|c| lap.avg_wheels_pressure[*c])
            .sum::<f32>()
            / corners.len() as f32;
        push(
            "Tyres".tr(ru),
            "Pressure".tr(ru),
            Severity::Warning,
            format!(
                "{} {} {} ({} {})",
                corner_phrase(corners, ru),
                if high { "over".tr(ru) } else { "under".tr(ru) },
                fmt.format_pressure(average),
                "target".tr(ru),
                fmt.format_pressure(target)
            ),
            if high {
                "Take pressure out".tr(ru)
            } else {
                "Put pressure in".tr(ru)
            }
            .to_string(),
        );
    }

    // --- tyre temperature window -----------------------------------------
    let mut cold: Vec<usize> = Vec::new();
    let mut hot: Vec<usize> = Vec::new();
    for corner in 0..4 {
        let temp = lap.avg_tyre_temp[corner];
        if temp <= 0.0 {
            continue;
        }
        if temp > alerts.tyre_temp_max {
            hot.push(corner);
        } else if temp < alerts.tyre_temp_min {
            cold.push(corner);
        }
    }
    for (corners, is_hot) in [(&hot, true), (&cold, false)] {
        if corners.is_empty() {
            continue;
        }
        let average =
            corners.iter().map(|c| lap.avg_tyre_temp[*c]).sum::<f32>() / corners.len() as f32;
        push(
            "Tyres".tr(ru),
            "Temperature".tr(ru),
            if is_hot {
                Severity::Warning
            } else {
                Severity::Info
            },
            format!(
                "{} {} {}",
                corner_phrase(corners, ru),
                if is_hot {
                    "over temperature".tr(ru)
                } else {
                    "cold".tr(ru)
                },
                fmt.format_temp(average)
            ),
            if is_hot {
                "Less pressure / ease off".tr(ru)
            } else {
                "More pressure / work them harder".tr(ru)
            }
            .to_string(),
        );
    }

    // --- camber, per axle -------------------------------------------------
    //
    // Inner minus outer, keeping the sign, averaged across the axle. The sign
    // is the whole message: an outer edge hotter than the inner one is a car
    // short of negative camber, and reading the magnitude alone gives exactly
    // the opposite advice. Per axle rather than per corner because a lap has
    // corners both ways and one wheel on its own reads the track's handedness
    // as a setup problem.
    for (axle, pair) in [(0usize, [0usize, 1usize]), (1, [2, 3])] {
        let spreads: Vec<f32> = pair
            .iter()
            .filter(|c| lap.avg_tyre_temp_i[**c] > 0.0 && lap.avg_tyre_temp_o[**c] > 0.0)
            .map(|c| lap.avg_tyre_temp_i[*c] - lap.avg_tyre_temp_o[*c])
            .collect();
        if spreads.is_empty() {
            continue;
        }
        let spread = spreads.iter().sum::<f32>() / spreads.len() as f32;
        let where_ = if axle == 0 { "Front" } else { "Rear" }.tr(ru);
        if spread > 12.0 {
            push(
                "Suspension".tr(ru),
                "Camber".tr(ru),
                Severity::Warning,
                format!(
                    "{where_}: {} (I-O: {})",
                    "inner edge running hot".tr(ru),
                    fmt.format_temp_delta(spread)
                ),
                "Less negative camber".tr(ru).to_string(),
            );
        } else if spread < 4.0 {
            push(
                "Suspension".tr(ru),
                "Camber".tr(ru),
                Severity::Info,
                format!(
                    "{where_}: {} (I-O: {})",
                    if spread < 0.0 {
                        "outer edge hotter".tr(ru)
                    } else {
                        "heated too evenly".tr(ru)
                    },
                    fmt.format_temp_delta(spread)
                ),
                "More negative camber".tr(ru).to_string(),
            );
        }
    }

    // --- brakes -----------------------------------------------------------
    let mut cooking: Vec<usize> = Vec::new();
    for corner in 0..4 {
        if lap.max_brake_temp[corner] > alerts.brake_temp_max {
            cooking.push(corner);
        }
    }
    if !cooking.is_empty() {
        let peak = cooking
            .iter()
            .map(|c| lap.max_brake_temp[*c])
            .fold(f32::MIN, f32::max);
        push(
            "Brakes".tr(ru),
            "Temperature".tr(ru),
            Severity::Critical,
            format!(
                "{} {} {}",
                corner_phrase(&cooking, ru),
                "overheating".tr(ru),
                fmt.format_temp(peak)
            ),
            "Open the brake ducts".tr(ru).to_string(),
        );
    }

    // --- balance and ride height ------------------------------------------
    //
    // Ported from the terminal's Engineer tab, which had these and the panel
    // did not. `avg_ride_height` is [front, rear]: AC publishes no per-corner
    // height, and a left-versus-right roll check used to live here comparing
    // two numbers that are the same by construction — always exactly 0.0, so
    // the warning was unreachable.
    //
    // **The height is in metres and the threshold is in millimetres.** It used
    // to be compared as it stands against 15.0, and an ordinary ride height of
    // 0.06 m clears `> 0.0` and is comfortably under 15, so every lap that
    // published a height at all was warned for bottoming out — and told the
    // driver it was at "0 mm", which is the same number printed with the same
    // mistake. It went unseen because the only fixture with ride heights in it
    // holds 25.0 and 55.0, both above the threshold by accident.
    const BOTTOMING_MM: f32 = 15.0;
    let heights_mm = lap.avg_ride_height.map(|height| height * 1000.0);
    let bottoming = heights_mm
        .iter()
        .any(|height| *height > 0.0 && *height < BOTTOMING_MM);
    if bottoming {
        let lowest = heights_mm
            .iter()
            .copied()
            .filter(|height| *height > 0.0)
            .fold(f32::MAX, f32::min);
        push(
            "Aero".tr(ru),
            "Ride height".tr(ru),
            Severity::Warning,
            format!("{} ({:.0} mm)", "Bottoming out".tr(ru), lowest),
            "Raise the ride height / stiffer springs".tr(ru).to_string(),
        );
    }

    // Counted over the lap, so a single moment does not make a verdict. Two of
    // each is noise in a car being driven near the limit.
    if lap.oversteer_count > lap.understeer_count && lap.oversteer_count > 2 {
        push(
            "Balance".tr(ru),
            "Oversteer".tr(ru),
            Severity::Info,
            format!("{}: {}x", "Oversteer".tr(ru), lap.oversteer_count),
            "Softer rear ARB / more rear wing".tr(ru).to_string(),
        );
    } else if lap.understeer_count > lap.oversteer_count && lap.understeer_count > 2 {
        push(
            "Balance".tr(ru),
            "Understeer".tr(ru),
            Severity::Info,
            format!("{}: {}x", "Understeer".tr(ru), lap.understeer_count),
            "Softer front ARB / more front wing".tr(ru).to_string(),
        );
    }

    // --- how it was driven ------------------------------------------------
    //
    // The setup lines above are worth more than these, so they come first and
    // these fill whatever room is left.
    if lap.lockup_count > 2 {
        push(
            "Driving".tr(ru),
            "Braking".tr(ru),
            Severity::Info,
            format!("{}: {}", "Lockups".tr(ru), lap.lockup_count),
            "Ease onto the pedal / more ABS".tr(ru).to_string(),
        );
    }
    if lap.scrubbing_incidents > 2 {
        push(
            "Driving".tr(ru),
            "Steering".tr(ru),
            Severity::Info,
            format!(
                "{}: {}x, {:.0}°",
                "Over-rotation".tr(ru),
                lap.scrubbing_incidents,
                lap.max_steering_over_rotation
            ),
            "Less steering — the tyres are scrubbing".tr(ru).to_string(),
        );
    }
    if lap.coasting_percent > 15.0 {
        push(
            "Driving".tr(ru),
            "Pedals".tr(ru),
            Severity::Info,
            format!("{} {:.0}%", "Coasting".tr(ru), lap.coasting_percent),
            "Get back on the throttle sooner".tr(ru).to_string(),
        );
    }

    // Most severe first, and stable within a severity so the same lap always
    // reads the same way.
    out.sort_by(|a, b| {
        b.severity
            .partial_cmp(&a.severity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::LapData;

    /// A lap with nothing in it. Every average is zero, which is a session
    /// that published nothing rather than a car with flat, frozen tyres — the
    /// same mistake the live engineer made about wear for eleven releases.
    #[test]
    fn an_empty_lap_says_nothing() {
        let lap = LapData::default();
        let advice = debrief(&lap, &AppConfig::default());
        assert!(advice.is_empty(), "{advice:?}");
    }

    /// The pressure block had no test of its own, and `cargo mutants` walked
    /// straight through it: the band edges, the midpoint the target is taken
    /// from and the average printed back all survived being rewritten.
    #[test]
    fn pressure_advice_uses_the_band_edges_and_reports_the_average() {
        let config = AppConfig::default();
        let (min, max) = (
            config.alerts.tyre_pressure_min,
            config.alerts.tyre_pressure_max,
        );

        // Sitting exactly on an edge is inside the band, not outside it.
        let on_the_edge = LapData {
            avg_wheels_pressure: [max, max, min, min],
            ..Default::default()
        };
        let advice = debrief(&on_the_edge, &config);
        assert!(
            !advice.iter().any(|r| r.category.contains("Pressure")),
            "the edges are in the band: {advice:?}"
        );

        // Over on both fronts, and the number quoted is their mean rather than
        // either one of them.
        let over = LapData {
            avg_wheels_pressure: [max + 1.0, max + 3.0, min + 0.5, min + 0.5],
            ..Default::default()
        };
        let advice = debrief(&over, &config);
        let line = advice
            .iter()
            .find(|r| r.category.contains("Pressure"))
            .expect("two fronts over the band produce a pressure line");
        let mean = ((max + 1.0) + (max + 3.0)) / 2.0;
        assert!(
            line.message.contains(&format!("{mean:.1}")),
            "the mean of the two that were over, not one of them: {}",
            line.message
        );
        assert!(
            line.message.to_lowercase().contains("front"),
            "named as an axle: {}",
            line.message
        );
    }

    /// The three driving lines and their thresholds, which nothing pinned:
    /// lockups and over-rotation are episode counts with the same noise floor
    /// as the balance rule, and coasting is a share of the lap.
    #[test]
    fn the_driving_lines_have_the_thresholds_they_claim() {
        let config = AppConfig::default();
        let clean = LapData {
            avg_wheels_pressure: [27.5; 4],
            avg_tyre_temp: [90.0; 4],
            max_brake_temp: [500.0; 4],
            avg_ride_height: [0.062, 0.078],
            ..Default::default()
        };

        let quiet = LapData {
            lockup_count: 2,
            scrubbing_incidents: 2,
            coasting_percent: 15.0,
            ..clean.clone()
        };
        let advice = debrief(&quiet, &config);
        assert!(
            !advice.iter().any(|r| r.component.contains("Driving")),
            "sitting on every threshold is under all of them: {advice:?}"
        );

        let busy = LapData {
            lockup_count: 3,
            scrubbing_incidents: 3,
            coasting_percent: 18.0,
            max_steering_over_rotation: 12.0,
            ..clean
        };
        let advice = debrief(&busy, &config);
        let driving: Vec<_> = advice
            .iter()
            .filter(|r| r.component.contains("Driving"))
            .collect();
        assert_eq!(driving.len(), 3, "one line each: {advice:?}");
        assert!(
            driving.iter().all(|r| r.severity == Severity::Info),
            "how it was driven never outranks what the car did"
        );
    }

    /// Which way the car is out of balance, and the threshold under it.
    /// `cargo mutants` could turn the `>` between the two counts into `<` —
    /// swapping the advice for its opposite — and nothing failed.
    #[test]
    fn the_balance_line_names_the_end_that_is_actually_loose() {
        let config = AppConfig::default();
        let base = LapData {
            avg_wheels_pressure: [27.5; 4],
            avg_tyre_temp: [90.0; 4],
            max_brake_temp: [500.0; 4],
            avg_ride_height: [0.062, 0.078],
            ..Default::default()
        };

        let loose_rear = LapData {
            oversteer_count: 9,
            understeer_count: 3,
            ..base.clone()
        };
        let advice = debrief(&loose_rear, &config);
        let line = advice
            .iter()
            .find(|r| r.category.contains("steer"))
            .expect("nine episodes of oversteer is a balance line");
        assert!(
            line.message.contains("Oversteer"),
            "the rear is the loose end: {}",
            line.message
        );
        assert!(
            line.action.contains("rear"),
            "and the fix is at the rear: {}",
            line.action
        );

        let pushing = LapData {
            oversteer_count: 3,
            understeer_count: 9,
            ..base.clone()
        };
        let advice = debrief(&pushing, &config);
        let line = advice
            .iter()
            .find(|r| r.category.contains("steer"))
            .expect("nine episodes of understeer is a balance line");
        assert!(
            line.message.contains("Understeer"),
            "the front is the one letting go: {}",
            line.message
        );

        // Two of anything is a car being driven near the limit, and the two
        // counts being equal says nothing about balance either way.
        let noise = LapData {
            oversteer_count: 2,
            understeer_count: 1,
            ..base.clone()
        };
        assert!(
            !debrief(&noise, &config)
                .iter()
                .any(|r| r.category.contains("steer")),
            "two is the noise floor"
        );
        let even = LapData {
            oversteer_count: 9,
            understeer_count: 9,
            ..base
        };
        assert!(
            !debrief(&even, &config)
                .iter()
                .any(|r| r.category.contains("steer")),
            "as much of one as the other is not a verdict"
        );
    }

    /// **The bug this test exists for shipped.** `avg_ride_height` is in
    /// metres and the threshold was written in millimetres, so an ordinary
    /// 60 mm of ride height compared as 0.06 against 15 and every lap was
    /// warned for bottoming out — at "0 mm", the same mistake printed back.
    #[test]
    fn an_ordinary_ride_height_is_not_bottoming_out() {
        let config = AppConfig::default();
        let ordinary = LapData {
            avg_wheels_pressure: [27.5; 4],
            avg_tyre_temp: [90.0; 4],
            max_brake_temp: [500.0; 4],
            // 62 mm front, 78 mm rear, as metres — what Assetto Corsa
            // publishes for a car sitting normally.
            avg_ride_height: [0.062, 0.078],
            ..Default::default()
        };
        assert!(
            !debrief(&ordinary, &config)
                .iter()
                .any(|r| r.category.contains("Ride height")),
            "60 mm is not the floor"
        );
    }

    /// And it still fires when the car really is on the deck, in the units it
    /// prints.
    #[test]
    fn a_car_on_the_floor_is_reported_in_millimetres() {
        let config = AppConfig::default();
        let scraping = LapData {
            avg_wheels_pressure: [27.5; 4],
            avg_tyre_temp: [90.0; 4],
            max_brake_temp: [500.0; 4],
            avg_ride_height: [0.009, 0.070],
            ..Default::default()
        };
        let advice = debrief(&scraping, &config);
        let line = advice
            .iter()
            .find(|r| r.category.contains("Ride height"))
            .expect("9 mm at the front is bottoming out");
        assert!(
            line.message.contains("9 mm"),
            "the lowest corner, in millimetres: {}",
            line.message
        );
    }

    /// A game that publishes no ride height says nothing, rather than
    /// reporting zero as a car welded to the tarmac.
    #[test]
    fn an_unpublished_ride_height_is_not_bottoming_out() {
        let config = AppConfig::default();
        let unmeasured = LapData {
            avg_wheels_pressure: [27.5; 4],
            avg_tyre_temp: [90.0; 4],
            max_brake_temp: [500.0; 4],
            avg_ride_height: [0.0, 0.0],
            ..Default::default()
        };
        assert!(
            !debrief(&unmeasured, &config)
                .iter()
                .any(|r| r.category.contains("Ride height")),
            "Competizione publishes none of this"
        );
    }

    /// The camber rule is the one that has shipped wrong verdicts before, and
    /// its guards were untested: a corner with no tread reading must not join
    /// the axle average, and the threshold is a spread of more than twelve
    /// degrees, not twelve.
    #[test]
    fn camber_ignores_a_corner_with_no_tread_reading() {
        let config = AppConfig::default();

        // The front left is cooking on its inner edge; the front right
        // published nothing. The axle verdict must come from the one corner
        // that was measured, not from an average with a zero in it.
        let half_measured = LapData {
            avg_wheels_pressure: [27.5; 4],
            avg_tyre_temp: [90.0; 4],
            avg_tyre_temp_i: [104.0, 0.0, 92.0, 92.0],
            avg_tyre_temp_o: [86.0, 0.0, 88.0, 88.0],
            max_brake_temp: [500.0; 4],
            ..Default::default()
        };
        let advice = debrief(&half_measured, &config);
        let camber: Vec<_> = advice
            .iter()
            .filter(|r| r.category.contains("Camber"))
            .collect();
        assert_eq!(camber.len(), 1, "one axle, one line: {advice:?}");
        assert!(
            camber[0].message.to_lowercase().contains("front"),
            "the axle that had a reading: {}",
            camber[0].message
        );

        // A game that publishes no tread temperature at all says nothing,
        // rather than reporting a spread of zero as perfect camber.
        let unmeasured = LapData {
            avg_wheels_pressure: [27.5; 4],
            avg_tyre_temp: [90.0; 4],
            avg_tyre_temp_i: [0.0; 4],
            avg_tyre_temp_o: [0.0; 4],
            max_brake_temp: [500.0; 4],
            ..Default::default()
        };
        assert!(
            !debrief(&unmeasured, &config)
                .iter()
                .any(|r| r.category.contains("Camber")),
            "no tread temperatures is not a camber verdict"
        );
    }

    /// The temperature window, which is the same shape as the pressure one
    /// and was equally untested: edges inside the band, the mean reported,
    /// and a hot tyre outranking a cold one in severity.
    #[test]
    fn temperature_advice_uses_the_band_edges_and_ranks_hot_above_cold() {
        let config = AppConfig::default();
        let (min, max) = (config.alerts.tyre_temp_min, config.alerts.tyre_temp_max);

        let on_the_edge = LapData {
            avg_tyre_temp: [max, max, min, min],
            avg_wheels_pressure: [27.5; 4],
            ..Default::default()
        };
        let advice = debrief(&on_the_edge, &config);
        assert!(
            !advice.iter().any(|r| r.category.contains("Temperature")),
            "the edges are in the band: {advice:?}"
        );

        let both_ends = LapData {
            avg_tyre_temp: [max + 10.0, max + 20.0, min - 10.0, min - 10.0],
            avg_wheels_pressure: [27.5; 4],
            ..Default::default()
        };
        let advice = debrief(&both_ends, &config);
        let temps: Vec<_> = advice
            .iter()
            .filter(|r| r.category.contains("Temperature"))
            .collect();
        assert_eq!(
            temps.len(),
            2,
            "one line for the hot pair, one for the cold"
        );
        assert_eq!(
            temps[0].severity,
            Severity::Warning,
            "overheating outranks being cold"
        );
        let mean = ((max + 10.0) + (max + 20.0)) / 2.0;
        assert!(
            temps[0].message.contains(&format!("{mean:.0}")),
            "the mean of the two that were hot: {}",
            temps[0].message
        );
    }

    /// Zero is not a flat tyre. A session that ended before the analyser had
    /// anything to average leaves zeros, and they must not become advice.
    #[test]
    fn an_unpublished_pressure_is_not_a_flat_tyre() {
        let lap = LapData {
            avg_wheels_pressure: [0.0; 4],
            avg_tyre_temp: [90.0; 4],
            ..Default::default()
        };
        let advice = debrief(&lap, &AppConfig::default());
        assert!(
            !advice.iter().any(|r| r.category.contains("Pressure")),
            "{advice:?}"
        );
    }

    fn healthy_lap() -> LapData {
        LapData {
            avg_wheels_pressure: [27.5; 4],
            avg_tyre_temp: [90.0; 4],
            avg_tyre_temp_i: [92.0; 4],
            avg_tyre_temp_o: [84.0; 4],
            max_brake_temp: [500.0; 4],
            ..Default::default()
        }
    }

    #[test]
    fn a_good_lap_is_quiet() {
        let advice = debrief(&healthy_lap(), &AppConfig::default());
        assert!(advice.is_empty(), "{advice:?}");
    }

    /// The bug this module was written around: `abs()` on the spread sent a
    /// car short of camber to the branch that takes camber out.
    #[test]
    fn an_outer_edge_running_hot_asks_for_more_camber() {
        let mut lap = healthy_lap();
        // Outer 13 °C hotter than the inner edge, on the front axle only.
        lap.avg_tyre_temp_i[0] = 84.0;
        lap.avg_tyre_temp_i[1] = 84.0;
        lap.avg_tyre_temp_o[0] = 97.0;
        lap.avg_tyre_temp_o[1] = 97.0;

        let advice = debrief(&lap, &AppConfig::default());
        let camber: Vec<_> = advice.iter().filter(|r| r.category == "Camber").collect();
        assert_eq!(camber.len(), 1, "one axle, one line: {advice:?}");
        assert!(camber[0].message.contains("Front"), "{}", camber[0].message);
        assert!(
            camber[0].action.contains("More negative camber"),
            "an outer edge that is hotter wants MORE camber, not less: {}",
            camber[0].action
        );
    }

    #[test]
    fn an_inner_edge_cooking_asks_for_less() {
        let mut lap = healthy_lap();
        lap.avg_tyre_temp_i[2] = 105.0;
        lap.avg_tyre_temp_i[3] = 105.0;
        lap.avg_tyre_temp_o[2] = 85.0;
        lap.avg_tyre_temp_o[3] = 85.0;

        let advice = debrief(&lap, &AppConfig::default());
        let camber: Vec<_> = advice.iter().filter(|r| r.category == "Camber").collect();
        assert_eq!(camber.len(), 1, "{advice:?}");
        assert!(camber[0].message.contains("Rear"));
        assert!(camber[0].action.contains("Less negative camber"));
    }

    /// Four corners of one problem are one line here too.
    #[test]
    fn four_over_inflated_tyres_are_one_line() {
        let mut lap = healthy_lap();
        lap.avg_wheels_pressure = [31.0; 4];

        let advice = debrief(&lap, &AppConfig::default());
        let pressure: Vec<_> = advice.iter().filter(|r| r.category == "Pressure").collect();
        assert_eq!(pressure.len(), 1, "{advice:?}");
        assert!(
            pressure[0].message.contains("All four"),
            "{}",
            pressure[0].message
        );
    }

    /// Cooking brakes outrank a note about coasting, whatever order they were
    /// found in.
    #[test]
    fn the_worst_thing_is_first() {
        let mut lap = healthy_lap();
        lap.coasting_percent = 40.0;
        lap.max_brake_temp = [900.0; 4];

        let advice = debrief(&lap, &AppConfig::default());
        assert!(!advice.is_empty());
        assert_eq!(advice[0].severity, Severity::Critical, "{advice:?}");
    }
}
