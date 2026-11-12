//! The engineer's findings as a plan: what to change first, and what waits.
//!
//! **A list of correct findings is not yet advice.** "Pressures low",
//! "tyres cold" and "camber: outer shoulder hot" can all be true at once,
//! and a driver who changes all three after one stint learns nothing — the
//! pressure moved the temperature and the spread too, so the next run says
//! nothing about the other two. A race engineer changes one thing at a
//! time, in the order the car's physics run: pressures, then what the tyre
//! does at that pressure, then the car around it.
//!
//! So the findings are ordered that way (within severity — a critical
//! finding still comes first), and a finding that another one in the list
//! moves says so: the temperature verdict waits for the pressure, the
//! brakes' heat waits for the bias. The first change to make is marked as
//! the place to start. Nothing here decides a finding; it orders and links
//! what the rules found.

use crate::engineer::{Recommendation, Severity};
use crate::i18n::Translate;

/// A finding's kind, read from its category in either language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Rejoin,
    Pressure,
    TyreHeat,
    Camber,
    Bias,
    BrakeHeat,
    Aero,
    Suspension,
    Wear,
    Driving,
    Strategy,
    Wheel,
    Other,
}

fn kind(rec: &Recommendation) -> Kind {
    let is = |english: &str| rec.category == english || rec.category == english.tr(true);
    let brakes = rec.component == "Brakes" || rec.component == "Brakes".tr(true);
    match () {
        _ if is("Rejoin") => Kind::Rejoin,
        _ if is("Pressure") => Kind::Pressure,
        _ if is("Overheat") && brakes => Kind::BrakeHeat,
        _ if is("Temperature") || is("Overheat") => Kind::TyreHeat,
        _ if is("Camber") => Kind::Camber,
        _ if is("Bias") => Kind::Bias,
        _ if is("Rake Loss") => Kind::Aero,
        _ if is("Bottoming") => Kind::Suspension,
        _ if is("Wear") => Kind::Wear,
        _ if is("Time Loss") || is("Overdriving") => Kind::Driving,
        _ if is("Fuel") || is("Race Finish") => Kind::Strategy,
        _ if is("Clipping") => Kind::Wheel,
        _ => Kind::Other,
    }
}

/// What a finding waits for, when the list holds it: the kinds whose change
/// moves this one's measurement.
fn waits_for(this: Kind) -> &'static [Kind] {
    match this {
        Kind::TyreHeat | Kind::Camber | Kind::Wear => &[Kind::Pressure],
        Kind::BrakeHeat => &[Kind::Bias],
        _ => &[],
    }
}

fn note_for(waited: Kind, ru: bool) -> &'static str {
    match waited {
        Kind::Pressure => {
            "Pressure moves this too — change the pressure first, then judge this again after three laps.".tr(ru)
        }
        Kind::Bias => {
            "The brake bias decides which brakes work hardest — change the bias first, then look at this again.".tr(ru)
        }
        _ => "Change the finding above first.".tr(ru),
    }
}

fn start_here(ru: bool) -> &'static str {
    "Start here: one change at a time, then three clean laps before the next.".tr(ru)
}

/// Order the findings into a plan and link the ones that wait on another.
/// Idempotent: run twice, the notes are added once.
pub fn into_a_plan(recs: &mut [Recommendation], ru: bool) {
    recs.sort_by(|a, b| {
        b.severity
            .partial_cmp(&a.severity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(kind(a).cmp(&kind(b)))
            .then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    let present: Vec<Kind> = recs.iter().map(kind).collect();
    for rec in recs.iter_mut() {
        if let Some(waited) = waits_for(kind(rec)).iter().find(|k| present.contains(k)) {
            append(&mut rec.action, note_for(*waited, ru));
        }
    }
    // The first change to the car, when there is more than one to make.
    let changes = recs.iter().filter(|r| !r.parameters.is_empty()).count();
    if changes > 1
        && let Some(first) = recs
            .iter_mut()
            .find(|r| !r.parameters.is_empty() && r.severity != Severity::Info)
    {
        append(&mut first.action, start_here(ru));
    }
}

fn append(action: &mut String, note: &str) {
    if action.contains(note) {
        return;
    }
    if !action.is_empty() && !action.ends_with(' ') {
        action.push(' ');
    }
    action.push_str(note);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    use super::*;
    use crate::engineer::Parameter;

    fn rec(category: &str, component: &str, severity: Severity, change: bool) -> Recommendation {
        Recommendation {
            component: component.to_string(),
            category: category.to_string(),
            severity,
            action: "Do it.".to_string(),
            parameters: match change {
                true => vec![Parameter::default()],
                false => Vec::new(),
            },
            confidence: 0.8,
            ..Recommendation::default()
        }
    }

    /// **Pressure before what it moves**, within a severity; the
    /// temperature verdict says it waits; the first change says start here.
    #[test]
    fn the_findings_become_a_plan() {
        let mut recs = vec![
            rec("Temperature", "Tyres", Severity::Warning, true),
            rec("Camber", "Suspension", Severity::Warning, true),
            rec("Pressure", "Tyres (GT3)", Severity::Warning, true),
            rec("Time Loss", "Driving", Severity::Critical, false),
        ];
        into_a_plan(&mut recs, false);
        let order: Vec<&str> = recs.iter().map(|r| r.category.as_str()).collect();
        assert_eq!(
            order,
            vec!["Time Loss", "Pressure", "Temperature", "Camber"]
        );
        assert!(recs[1].action.contains("Start here"));
        assert!(recs[2].action.contains("change the pressure first"));
        assert!(recs[3].action.contains("change the pressure first"));
        // Run again: nothing is said twice.
        into_a_plan(&mut recs, false);
        assert_eq!(recs[2].action.matches("pressure first").count(), 1);
        assert_eq!(recs[1].action.matches("Start here").count(), 1);
    }

    /// The brakes' heat waits for the bias; tyre heat does not.
    #[test]
    fn brake_heat_waits_for_the_bias_in_either_language() {
        let mut recs = vec![
            rec(
                "Overheat".tr(true),
                "Brakes".tr(true),
                Severity::Warning,
                false,
            ),
            rec("Bias".tr(true), "Brakes".tr(true), Severity::Warning, true),
        ];
        into_a_plan(&mut recs, true);
        assert_eq!(recs[0].category, "Bias".tr(true));
        let note = "The brake bias decides which brakes work hardest — change the bias first, then look at this again.";
        assert!(recs[1].action.contains(note.tr(true)));
        // One change alone needs no "start here".
        let start = "Start here: one change at a time, then three clean laps before the next.";
        assert!(!recs[0].action.contains(start.tr(true)));
    }

    /// Nothing to link, nothing added.
    #[test]
    fn a_lone_finding_is_left_as_it_was() {
        let mut recs = vec![rec("Wear", "Tyres", Severity::Info, false)];
        into_a_plan(&mut recs, false);
        assert_eq!(recs[0].action, "Do it.");
    }
}
