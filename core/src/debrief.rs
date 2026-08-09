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
        [0, 1] => if ru { "Перед" } else { "Fronts" }.to_string(),
        [2, 3] => if ru { "Зад" } else { "Rears" }.to_string(),
        [0, 2] => if ru { "Левые" } else { "Left side" }.to_string(),
        [1, 3] => if ru { "Правые" } else { "Right side" }.to_string(),
        [0, 1, 2, 3] => if ru { "Все шины" } else { "All four" }.to_string(),
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
            if ru { "Шины" } else { "Tyres" },
            if ru { "Давление" } else { "Pressure" },
            Severity::Warning,
            format!(
                "{} {} {} ({} {})",
                corner_phrase(corners, ru),
                if high {
                    if ru { "перекачаны" } else { "over" }
                } else if ru {
                    "недокачаны"
                } else {
                    "under"
                },
                fmt.format_pressure(average),
                if ru { "цель" } else { "target" },
                fmt.format_pressure(target)
            ),
            if high {
                if ru {
                    "Спустить"
                } else {
                    "Take pressure out"
                }
            } else if ru {
                "Накачать"
            } else {
                "Put pressure in"
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
            if ru { "Шины" } else { "Tyres" },
            if ru {
                "Температура"
            } else {
                "Temperature"
            },
            if is_hot {
                Severity::Warning
            } else {
                Severity::Info
            },
            format!(
                "{} {} {}",
                corner_phrase(corners, ru),
                if is_hot {
                    if ru {
                        "перегрев"
                    } else {
                        "over temperature"
                    }
                } else if ru {
                    "холодные"
                } else {
                    "cold"
                },
                fmt.format_temp(average)
            ),
            if is_hot {
                if ru {
                    "Ниже давление / мягче стиль"
                } else {
                    "Less pressure / ease off"
                }
            } else if ru {
                "Выше давление / больше нагрузки"
            } else {
                "More pressure / work them harder"
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
        let where_ = match (axle, ru) {
            (0, true) => "Перед",
            (0, false) => "Front",
            (_, true) => "Зад",
            (_, false) => "Rear",
        };
        if spread > 12.0 {
            push(
                if ru { "Подвеска" } else { "Suspension" },
                if ru { "Развал" } else { "Camber" },
                Severity::Warning,
                format!(
                    "{where_}: {} (I-O: {})",
                    if ru {
                        "перегрев внутренней части"
                    } else {
                        "inner edge running hot"
                    },
                    fmt.format_temp_delta(spread)
                ),
                if ru {
                    "Меньше отриц. развала"
                } else {
                    "Less negative camber"
                }
                .to_string(),
            );
        } else if spread < 4.0 {
            push(
                if ru { "Подвеска" } else { "Suspension" },
                if ru { "Развал" } else { "Camber" },
                Severity::Info,
                format!(
                    "{where_}: {} (I-O: {})",
                    if spread < 0.0 {
                        if ru {
                            "внешняя часть горячее"
                        } else {
                            "outer edge hotter"
                        }
                    } else if ru {
                        "прогрев слишком равномерный"
                    } else {
                        "heated too evenly"
                    },
                    fmt.format_temp_delta(spread)
                ),
                if ru {
                    "Больше отриц. развала"
                } else {
                    "More negative camber"
                }
                .to_string(),
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
            if ru { "Тормоза" } else { "Brakes" },
            if ru {
                "Температура"
            } else {
                "Temperature"
            },
            Severity::Critical,
            format!(
                "{} {} {}",
                corner_phrase(&cooking, ru),
                if ru {
                    "перегрев"
                } else {
                    "overheating"
                },
                fmt.format_temp(peak)
            ),
            if ru {
                "Открыть воздуховоды"
            } else {
                "Open the brake ducts"
            }
            .to_string(),
        );
    }

    // --- how it was driven ------------------------------------------------
    //
    // The setup lines above are worth more than these, so they come first and
    // these fill whatever room is left.
    if lap.lockup_count > 2 {
        push(
            if ru { "Пилотаж" } else { "Driving" },
            if ru {
                "Торможение"
            } else {
                "Braking"
            },
            Severity::Info,
            format!(
                "{}: {}",
                if ru {
                    "Блокировки"
                } else {
                    "Lockups"
                },
                lap.lockup_count
            ),
            if ru {
                "Мягче на педаль / больше ABS"
            } else {
                "Ease onto the pedal / more ABS"
            }
            .to_string(),
        );
    }
    if lap.scrubbing_incidents > 2 {
        push(
            if ru { "Пилотаж" } else { "Driving" },
            if ru { "Руление" } else { "Steering" },
            Severity::Info,
            format!(
                "{}: {}x, {:.0}°",
                if ru {
                    "Перекрут руля"
                } else {
                    "Over-rotation"
                },
                lap.scrubbing_incidents,
                lap.max_steering_over_rotation
            ),
            if ru {
                "Меньше угла — шины скребут"
            } else {
                "Less steering — the tyres are scrubbing"
            }
            .to_string(),
        );
    }
    if lap.coasting_percent > 15.0 {
        push(
            if ru { "Пилотаж" } else { "Driving" },
            if ru { "Педали" } else { "Pedals" },
            Severity::Info,
            format!(
                "{} {:.0}%",
                if ru { "Накат" } else { "Coasting" },
                lap.coasting_percent
            ),
            if ru {
                "Раньше на газ после торможения"
            } else {
                "Get back on the throttle sooner"
            }
            .to_string(),
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
