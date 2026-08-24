//! What a circuit is, as far as the rest of the program is concerned.
//!
//! **Neutral on purpose.** Assetto Corsa keeps the real shape of a track on
//! disk — a rendered outline, the parameters to align world coordinates onto
//! it, the names its corners actually have, and the line the AI drives.
//! Competizione keeps none of it. So this is what a front end may ask for, and
//! a game that cannot answer says so by returning nothing rather than by
//! having a different shape of answer.
//!
//! Everything here is **additive**. The map drawn from the driver's own
//! coordinates has to keep working exactly as it does — that is what makes it
//! work on a circuit nobody has surveyed, which is most of what people drive.
//! A track file, where there is one, makes that map more exact; it never
//! becomes a requirement for having one.

use serde::{Deserialize, Serialize};

/// How long the circuit is, worked out from the car that just drove round it.
///
/// **Because one of the two games does not say.** Assetto Corsa publishes its
/// spline length and this is never needed there. Competizione publishes
/// `trackSPlineLength` as zero — pinned by the layout tests against a real
/// recorded session, so it is the game's behaviour and not a parsing mistake —
/// and the consequence reaches the driver: every answer denominated in metres
/// is withheld on the game most GT3 drivers are on. No "braking 14 m earlier",
/// no corner distances.
///
/// The measurement was already sitting there. Both games publish how far the
/// car has travelled since the session started, and both readers already put it
/// on the `Reading`, where nothing has ever read it. The distance between two
/// crossings of the line is the length of the lap between them.
///
/// # What this is not
///
/// It is **not** the game reporting a track length, and it must never be filed
/// as one. `Capabilities::track_length` stays false on a game that does not
/// publish it: that flag answers "does this game measure it", and the answer is
/// still no. This answers a different question — "has the car been round yet"
/// — and until it has, there is no length here rather than a plausible one.
#[derive(Debug, Clone, Copy, Default)]
pub struct MeasuredLength {
    /// The distance reading when the car last crossed the line.
    at_line_m: Option<f32>,
    /// Where the car was on the previous sample, so a wrap can be spotted.
    last_position: f32,
    /// The best answer so far, metres.
    metres: Option<f32>,
}

/// The shortest and longest circuit this will believe in, metres.
///
/// Anything outside is a session that was reset, a car teleported to the pits,
/// or a distance counter that started somewhere else — and a wrong track length
/// is worse than none, because every metre-denominated answer would quietly be
/// scaled by it. The narrowest real circuits are under a kilometre; the longest
/// in common use is the Nordschleife at 20.8 km.
const PLAUSIBLE_M: std::ops::RangeInclusive<f32> = 500.0..=30_000.0;

/// How far round the lap counts as "about to cross the line" and "just after
/// it". A wrap is a fall from the first to the second.
const NEAR_LINE: (f32, f32) = (0.9, 0.1);

impl MeasuredLength {
    /// Feed one sample: where the car is round the lap, 0..1, and how far it
    /// has travelled since the session started, in metres.
    ///
    /// Returns the length when a lap has just been completed and measured,
    /// so a caller can act on it the moment it arrives rather than polling.
    pub fn observe(&mut self, track_position: f32, distance_travelled_m: f32) -> Option<f32> {
        let previous = self.last_position;
        self.last_position = track_position;

        // Not a crossing.
        if !(previous > NEAR_LINE.0 && track_position < NEAR_LINE.1) {
            return None;
        }

        let previous_line = self.at_line_m.replace(distance_travelled_m);
        let covered = distance_travelled_m - previous_line?;
        if !PLAUSIBLE_M.contains(&covered) {
            // A lap that measured as impossible says nothing, and the crossing
            // is still recorded — the next lap is measured from here.
            return None;
        }
        self.metres = Some(covered);
        Some(covered)
    }

    /// The measured length, if the car has been round once.
    pub fn metres(&self) -> Option<f32> {
        self.metres
    }
}

#[cfg(test)]
mod measured_length_tests {
    use super::MeasuredLength;

    /// One lap of Spa, sampled the way the game publishes it: the position
    /// wraps at the line and the distance keeps counting up.
    fn drive(measure: &mut MeasuredLength, laps: usize, length_m: f32) -> Vec<f32> {
        let mut travelled = 0.0;
        let mut measured = Vec::new();
        for _ in 0..laps {
            for step in 0..100 {
                let position = step as f32 / 100.0;
                travelled += length_m / 100.0;
                if let Some(metres) = measure.observe(position, travelled) {
                    measured.push(metres);
                }
            }
        }
        measured
    }

    #[test]
    fn nothing_is_known_until_the_car_has_been_round() {
        let mut measure = MeasuredLength::default();
        // Half a lap in: the line has been crossed once at most, and one
        // crossing measures nothing.
        drive(&mut measure, 1, 7_004.0);
        assert_eq!(measure.metres(), None);
    }

    #[test]
    fn a_second_crossing_measures_the_lap_between_them() {
        let mut measure = MeasuredLength::default();
        drive(&mut measure, 3, 7_004.0);
        let metres = measure.metres().expect("three laps is two measurements");
        assert!((metres - 7_004.0).abs() < 80.0, "measured {metres} m");
    }

    /// A session reset, a tow to the pits, or a distance counter that started
    /// somewhere else. A wrong length would silently scale every answer given
    /// in metres, which is worse than having none.
    #[test]
    fn an_impossible_lap_is_not_believed() {
        let mut measure = MeasuredLength::default();
        measure.observe(0.95, 1_000.0);
        measure.observe(0.05, 1_000.0);
        // Ninety kilometres later, the line again.
        measure.observe(0.95, 91_000.0);
        measure.observe(0.05, 91_000.0);
        assert_eq!(measure.metres(), None);
    }

    #[test]
    fn a_game_that_publishes_its_length_needs_none_of_this() {
        // The type is inert until it is fed; nothing here runs on Assetto
        // Corsa, where `track_length_m` arrives from the game itself.
        let measure = MeasuredLength::default();
        assert_eq!(measure.metres(), None);
    }
}

/// A named stretch of a circuit, as the track itself names it.
///
/// **This is the difference between "T7" and "Eau Rouge".** Corner detection
/// finds where the corners are from what the car did, which works everywhere;
/// this says what they are called, which only the track knows. The two are
/// matched by distance, so a track with no names loses the names and keeps
/// every number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    /// Where it begins and ends, as a fraction of the lap.
    pub from: f32,
    pub to: f32,
}

impl Section {
    /// Whether a place on the lap falls inside this section.
    pub fn holds(&self, distance: f32) -> bool {
        // A section that wraps the start line — a chicane at 0.98 to 0.02 —
        // is two ranges rather than an empty one.
        if self.from <= self.to {
            (self.from..=self.to).contains(&distance)
        } else {
            distance >= self.from || distance <= self.to
        }
    }
}

/// A stretch where the rear wing may be opened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrsZone {
    /// Where the gap to the car ahead is measured.
    pub detection: f32,
    pub start: f32,
    pub end: f32,
}

/// How world coordinates land on the track's own rendered outline.
///
/// The numbers are the game's, straight out of the file that ships beside the
/// image. Their meaning: a world point is shifted by the offsets, divided by
/// the scale factor, and the margin is the blank border the image was drawn
/// with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alignment {
    pub width: f32,
    pub height: f32,
    pub margin: f32,
    pub scale_factor: f32,
    pub x_offset: f32,
    pub z_offset: f32,
}

impl Alignment {
    /// Where a world position falls on the outline image, in pixels.
    pub fn place(&self, world_x: f32, world_z: f32) -> (f32, f32) {
        let scale = if self.scale_factor.abs() < f32::EPSILON {
            1.0
        } else {
            self.scale_factor
        };
        (
            (world_x + self.x_offset) / scale + self.margin,
            (world_z + self.z_offset) / scale + self.margin,
        )
    }
}

/// One point of a line the track itself carries.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinePoint {
    pub x: f32,
    /// Altitude. Carried because a track has elevation and a plot of it is
    /// worth having; no map uses it.
    pub y: f32,
    pub z: f32,
    /// Metres from the start line.
    pub metres: f32,
}

/// Everything a game could tell us about the circuit that is loaded.
///
/// Every field is optional and absent means absent — a track with no
/// `sections.ini` has no names, not a list of empty ones.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackData {
    /// The folder it was read from, so a screen can say where a name came
    /// from and a cache can be keyed on it.
    pub source: String,
    /// The rendered outline that ships with the track, if there is one. A path
    /// rather than the pixels: decoding a PNG is the front end's business and
    /// the core has no image decoder.
    pub outline: Option<std::path::PathBuf>,
    pub alignment: Option<Alignment>,
    pub sections: Vec<Section>,
    pub drs: Vec<DrsZone>,
    /// The line the game's own AI drives. **Not an optimal line** — it is a
    /// line that gets round, and saying otherwise would be inventing a
    /// measurement. Useful as a reference and labelled as what it is.
    pub ai_line: Vec<LinePoint>,
}

impl TrackData {
    /// Whether anything at all was found.
    pub fn is_empty(&self) -> bool {
        self.outline.is_none()
            && self.alignment.is_none()
            && self.sections.is_empty()
            && self.drs.is_empty()
            && self.ai_line.is_empty()
    }

    /// What the track calls the corner at this place on the lap.
    pub fn name_at(&self, distance: f32) -> Option<&str> {
        self.sections
            .iter()
            .find(|section| section.holds(distance))
            .map(|section| section.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_section_that_wraps_the_line_still_holds_both_ends() {
        let chicane = Section {
            name: "Chicane".to_string(),
            from: 0.97,
            to: 0.03,
        };
        assert!(chicane.holds(0.99));
        assert!(chicane.holds(0.01));
        assert!(!chicane.holds(0.5));
    }

    #[test]
    fn a_track_with_no_files_names_nothing() {
        let nothing = TrackData::default();
        assert!(nothing.is_empty());
        assert_eq!(nothing.name_at(0.5), None);
    }
}

/// What a game knows about the car that is loaded.
///
/// The same shape of answer as [`TrackData`] and for the same reasons: only
/// Assetto Corsa keeps any of it, every field is optional, and a game with
/// nothing to say returns an empty one rather than a differently shaped
/// something.
///
/// **The curves are the reason this exists.** A rev counter is a number; the
/// engine's own torque and power against revs is what says whether a driver is
/// short-shifting, and it ships beside every car including one installed from
/// a forum yesterday.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CarData {
    /// As the car calls itself — "Mazda Miata NA" rather than
    /// `ks_mazda_miata`.
    pub name: String,
    pub brand: String,
    /// The game's own class word, which is not this program's car class: it is
    /// free text and a mod may put anything in it.
    pub class: String,
    pub tags: Vec<String>,
    /// Power, weight and the rest, as the car writes them — strings, because
    /// that is what they are in the file and parsing "197+km/h" into a number
    /// would be inventing precision.
    pub specs: Vec<(String, String)>,
    /// Newton-metres against revs.
    pub torque: Vec<(f32, f32)>,
    /// Brake horsepower against revs.
    pub power: Vec<(f32, f32)>,
    /// The brand badge that ships with the car.
    pub badge: Option<std::path::PathBuf>,
    /// The car's real outline, in its own metres, out of the model the game
    /// renders. `None` where the car ships no model this can read.
    ///
    /// **Not serialised.** It is derived from a file on disk and re-read in a
    /// moment, and writing a few hundred points into every saved lap would be
    /// storing a car's shape in a record of a lap.
    #[serde(skip)]
    pub shape: Option<CarShape>,
}

/// A car seen from above, to scale.
///
/// Re-exported rather than redeclared: the parser already has exactly this
/// shape and a second copy of it here would be two definitions to keep in
/// step. Front ends name this and never the parser.
pub type CarShape = kn5::Shape;

impl CarData {
    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
            && self.torque.is_empty()
            && self.power.is_empty()
            && self.shape.is_none()
    }

    /// The revs the engine makes most torque at.
    pub fn torque_peak(&self) -> Option<(f32, f32)> {
        self.torque
            .iter()
            .copied()
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The revs it makes most power at, which is not the same place and is the
    /// one that decides where to change gear.
    pub fn power_peak(&self) -> Option<(f32, f32)> {
        self.power
            .iter()
            .copied()
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }
}
