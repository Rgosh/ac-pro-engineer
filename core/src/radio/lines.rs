//! Everything the engineer can say on the radio, written once.
//!
//! **Written for the ear, not copied from the screen.** The advice a screen
//! shows is a measurement with its mechanism — "FL OVERHEATING: 112 °C
//! (I/M/O 108/113/115)" — and read aloud it is a robot. A line here is what an
//! engineer says on the radio about the same finding: what is happening, why,
//! and what to do, in a sentence or two. The numbers stay on the screen and in
//! the subtitle, where a driver can read them on the next straight.
//!
//! **This table is the voice bank's script.** `tools/voicebank` speaks every
//! line of it — `cargo run -p ac_core --example radio_lines` prints it in the
//! shape the voice bank reads — so a line added here is a line the voices
//! learn on the next build, and a line the pack has no clip for is a test
//! that fails, not a silence nobody notices.

/// How a line is said: its pace, and how hard the radio is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Style {
    /// About the car and the driving, said on a straight.
    Calm,
    /// Numbers: fuel, laps.
    Report,
    /// A best lap, a clean stint.
    Praise,
    /// Something to do now: box, a locking rear, fuel gone.
    Urgent,
}

impl Style {
    /// The word the voice bank's styles are named by.
    pub fn name(self) -> &'static str {
        match self {
            Style::Calm => "calm",
            Style::Report => "report",
            Style::Praise => "praise",
            Style::Urgent => "urgent",
        }
    }
}

/// What makes a line worth saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// A finding of the engineer's, matched on its category, the component it
    /// is about, and words of its mechanism — all in English, whatever language
    /// the screen is in (see [`crate::i18n::english_of`]).
    Advice {
        category: &'static str,
        component: Option<&'static str>,
        cause: Option<&'static str>,
    },
    /// Something the radio notices by itself.
    Event(Event),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The first frame of a session with a car on track.
    RadioCheck,
    /// A lap faster than every lap before it this session.
    PersonalBest,
    /// Fuel for `n` more laps, said every few laps.
    FuelLaps,
    /// The driver started drifting, and the engineer says it will stay quiet
    /// about grip — the same rule the advice already follows.
    Drifting,
}

/// One thing the engineer can say.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    /// The clip's name in the voice pack. A line with a value has one clip per
    /// value, named `id.value`.
    pub id: &'static str,
    pub style: Style,
    pub trigger: Trigger,
    /// What is said. `{n}` is a whole number, spoken in words; the voice bank
    /// makes one clip per value in [`Line::values`].
    pub said: &'static str,
    /// The values `{n}` can take, inclusive. `None` for a line with no value.
    pub values: Option<(u32, u32)>,
}

const fn advice(
    id: &'static str,
    style: Style,
    category: &'static str,
    component: Option<&'static str>,
    cause: Option<&'static str>,
    said: &'static str,
) -> Line {
    Line {
        id,
        style,
        trigger: Trigger::Advice {
            category,
            component,
            cause,
        },
        said,
        values: None,
    }
}

const fn event(id: &'static str, style: Style, event: Event, said: &'static str) -> Line {
    Line {
        id,
        style,
        trigger: Trigger::Event(event),
        said,
        values: None,
    }
}

use Style::{Calm, Praise, Report, Urgent};

/// Every line, in the order the voice bank speaks them.
///
/// Where two lines share a category, the one with the narrower match comes
/// first: [`find`] takes the first that fits.
pub const LINES: &[Line] = &[
    // ── Events ───────────────────────────────────────────────────────────
    event(
        "radio.check",
        Report,
        Event::RadioCheck,
        "Radio check. Loud and clear.",
    ),
    event(
        "best.lap",
        Praise,
        Event::PersonalBest,
        "Good lap, mate. That's a new personal best.",
    ),
    event(
        "drift.quiet",
        Calm,
        Event::Drifting,
        "Copy, you're drifting. I'll keep quiet about grip.",
    ),
    Line {
        id: "fuel.laps",
        style: Report,
        trigger: Trigger::Event(Event::FuelLaps),
        said: "You've got fuel for {n} more laps.",
        values: Some((2, 40)),
    },
    // ── Brakes ───────────────────────────────────────────────────────────
    advice(
        "bias.rear",
        Urgent,
        "Bias",
        None,
        Some("rear axle"),
        "Rear's locking, careful! Too much braking at the back. Move the bias forward a click.",
    ),
    advice(
        "bias.front",
        Calm,
        "Bias",
        None,
        Some("front axle"),
        "You're locking the fronts. Too much of the braking is on the front axle. Move the bias back a click, and watch the next big stop.",
    ),
    advice(
        "brakes.hot",
        Calm,
        "Overheat",
        Some("Brakes"),
        None,
        "Brakes are cooking. More heat's going in than they can get rid of. Open the ducts a bit, or ease off the big stops.",
    ),
    advice(
        "brakes.wear",
        Calm,
        "Wear",
        Some("Brakes"),
        None,
        "The pads are getting thin. Go easy on the heavy stops, and we'll plan a stop.",
    ),
    // ── Tyres ────────────────────────────────────────────────────────────
    advice(
        "tyres.hot",
        Calm,
        "Overheat",
        None,
        None,
        "Tyres are overheating. They're taking more energy than they can shed. Calm the slides and look after them for a lap.",
    ),
    advice(
        "tyres.cold",
        Calm,
        "Temperature",
        None,
        None,
        "Tyres are still cold. There's not enough energy going in to bring them up. Lean on them a bit more through the corners.",
    ),
    advice(
        "pressure.low",
        Calm,
        "Pressure",
        None,
        Some("enough heat"),
        "Pressures are low. The tyre isn't building enough heat to reach its window. Add a little pressure, and check again in two laps.",
    ),
    advice(
        "pressure.high",
        Calm,
        "Pressure",
        None,
        Some("more pressure"),
        "Pressures are high. The tyre's building more than the cold setting allows for. Take some out, and check again in two laps.",
    ),
    advice(
        "tyres.fading",
        Calm,
        "Stint",
        None,
        None,
        "The tyres are going off. A little less grip every lap. Start thinking about when we box.",
    ),
    advice(
        "tyres.wear",
        Calm,
        "Wear",
        None,
        None,
        "The tyres are wearing. Look after them, or we'll have to box early.",
    ),
    advice(
        "camber",
        Calm,
        "Camber",
        None,
        None,
        "Camber's off. One edge of the tyre is doing all the work. The temperatures are on your screen.",
    ),
    advice(
        "rejoin.cold",
        Calm,
        "Rejoin",
        None,
        None,
        "Tyres cooled down while you were stopped. Take it easy for a lap and build the heat before you push.",
    ),
    // ── Balance and the car ──────────────────────────────────────────────
    advice(
        "balance.understeer",
        Calm,
        "Understeer",
        None,
        None,
        "Understeer at high speed. The front's running out of grip before the rear. Try a bit more front wing.",
    ),
    advice(
        "balance.oversteer",
        Calm,
        "Oversteer",
        None,
        None,
        "Oversteer at high speed. The rear lets go first. Try a bit more rear wing.",
    ),
    advice(
        "bottoming",
        Calm,
        "Bottoming",
        None,
        None,
        "We're bottoming out over the kerbs. The suspension's running out of travel. A bit more ride height, or stiffer springs.",
    ),
    advice(
        "rake",
        Calm,
        "Rake Loss",
        None,
        None,
        "The rear's squatting at high speed. The downforce is pulling the rake away. Stiffen the rear a touch.",
    ),
    advice(
        "ffb.clipping",
        Calm,
        "Clipping",
        None,
        None,
        "Your force feedback is clipping. Anything above the ceiling never reaches your hands. Bring the gain down a bit.",
    ),
    // ── Driving ──────────────────────────────────────────────────────────
    advice(
        "driving.coasting",
        Calm,
        "Time Loss",
        None,
        None,
        "You're coasting. The car's rolling where it should be braking or driving. Stay on one pedal or the other.",
    ),
    advice(
        "driving.overdriving",
        Calm,
        "Overdriving",
        None,
        None,
        "Too much steering. More lock than the corner will take, so the tyres just scrub. Unwind a bit and let the car turn.",
    ),
    // ── Fuel ─────────────────────────────────────────────────────────────
    advice(
        "fuel.short",
        Urgent,
        "Race Finish",
        None,
        None,
        "We're short on fuel for the finish. Lift and coast, save fuel now.",
    ),
    advice(
        "fuel.box",
        Urgent,
        "Fuel",
        None,
        None,
        "Fuel's critical. Box, box, box.",
    ),
];

/// The line for a piece of advice, if there is one.
///
/// Every argument is the English: [`crate::radio`] turns a Russian screen's
/// words back before asking, so one table serves both.
pub fn find(category: &str, component: &str, cause: &str) -> Option<&'static Line> {
    let category = category.trim();
    LINES.iter().find(|line| match line.trigger {
        Trigger::Advice {
            category: c,
            component: want_component,
            cause: want_cause,
        } => {
            c.eq_ignore_ascii_case(category)
                && want_component.is_none_or(|w| contains(component, w))
                && want_cause.is_none_or(|w| contains(cause, w))
        }
        Trigger::Event(_) => false,
    })
}

/// The line for an event.
pub fn for_event(event: Event) -> Option<&'static Line> {
    LINES
        .iter()
        .find(|line| line.trigger == Trigger::Event(event))
}

fn contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// A whole number as the engineer says it: 11 → "eleven".
pub fn spoken_number(n: u32) -> String {
    const ONES: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    match n {
        0..=19 => ONES.get(n as usize).copied().unwrap_or("").to_string(),
        20..=99 => {
            let tens = TENS.get((n / 10) as usize).copied().unwrap_or("");
            match n % 10 {
                0 => tens.to_string(),
                ones => format!("{tens}-{}", ONES.get(ones as usize).copied().unwrap_or("")),
            }
        }
        _ => n.to_string(),
    }
}

/// A line's text with its value in, the way the voice bank speaks it.
pub fn spoken(line: &Line, value: Option<u32>) -> String {
    match value {
        Some(n) => line.said.replace("{n}", &spoken_number(n)),
        None => line.said.to_string(),
    }
}

/// The clip's name for a line and its value.
pub fn clip_key(line: &Line, value: Option<u32>) -> String {
    match value {
        Some(n) => format!("{}.{n}", line.id),
        None => line.id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn every_id_is_unique() {
        let mut ids: Vec<&str> = LINES.iter().map(|l| l.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "two lines share an id");
    }

    #[test]
    fn a_value_line_has_a_value_and_only_then() {
        for line in LINES {
            assert_eq!(
                line.said.contains("{n}"),
                line.values.is_some(),
                "{}: `{{n}}` and `values` must come together",
                line.id
            );
        }
    }

    #[test]
    fn the_narrower_line_wins() {
        let rear = find(
            "Bias",
            "Brakes",
            "too much of the braking is landing on the rear axle",
        );
        assert_eq!(rear.map(|l| l.id), Some("bias.rear"));
        let front = find(
            "Bias",
            "Brakes",
            "too much of the braking is landing on the front axle",
        );
        assert_eq!(front.map(|l| l.id), Some("bias.front"));
        let brakes = find("Overheat", "Brakes", "more energy is going into the brakes");
        assert_eq!(brakes.map(|l| l.id), Some("brakes.hot"));
        let tyres = find("Overheat", "Tyres", "the tyre is being given more energy");
        assert_eq!(tyres.map(|l| l.id), Some("tyres.hot"));
        assert_eq!(find("Wear", "TYRES", "").map(|l| l.id), Some("tyres.wear"));
        assert!(
            find("Drift", "DRIFT", "").is_none(),
            "drift coaching stays on screen"
        );
    }

    #[test]
    fn numbers_are_said_in_words() {
        assert_eq!(spoken_number(11), "eleven");
        assert_eq!(spoken_number(40), "forty");
        assert_eq!(spoken_number(23), "twenty-three");
        let fuel = for_event(Event::FuelLaps).unwrap();
        assert_eq!(spoken(fuel, Some(11)), "You've got fuel for eleven more laps.");
        assert_eq!(clip_key(fuel, Some(11)), "fuel.laps.11");
    }

    /// **Every finding the engineer can make has a line**, or the radio says
    /// nothing about it. Read out of the engineer's own source rather than
    /// listed again here, so a rule added tomorrow fails this tomorrow.
    #[test]
    fn every_category_the_engineer_uses_has_a_line() {
        let sources = [include_str!("../engineer.rs"), include_str!("../stint.rs")];
        let mut missing = Vec::new();
        for source in sources {
            for (k, _) in source.match_indices("category: \"") {
                let after = source.get(k + "category: \"".len()..).unwrap_or("");
                let category = after.split('"').next().unwrap_or("");
                if category != "Drift" && find(category, "", "").is_none() {
                    // A category whose lines all need a component or a cause
                    // still has a line — `find` above asked with neither.
                    let narrower = LINES.iter().any(|l| {
                        matches!(l.trigger, Trigger::Advice { category: c, .. } if c == category)
                    });
                    if !narrower {
                        missing.push(category.to_string());
                    }
                }
            }
        }
        // The engineer's either-or categories, written as `if` arms.
        for category in ["Understeer", "Oversteer"] {
            assert!(find(category, "", "").is_some(), "{category} has no line");
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "categories with no radio line: {missing:?}"
        );
    }
}
