//! What a game knows about a car before anybody drives it.
//!
//! The counterpart to [`Reading`](super::Reading): that is the shape telemetry
//! arrives in, this is the shape the catalogue does. Every simulator ships a
//! list of cars with a power figure and a weight somewhere; where it keeps them
//! and what it calls them is the game folder's business.

use serde::{Deserialize, Serialize};

/// One car, as the game describes it.
///
/// The strings are kept beside the numbers on purpose. `power` is whatever the
/// car's author typed — "552bhp", "560 hp @ 8250" — and it is what a driver
/// recognises; `power_hp` is that same figure dug out for arithmetic. Throwing
/// the original away would make the screens read worse than the game's own.
/// What a car actually lets a driver change.
///
/// **Because advice about a part the car has not got is worse than silence.**
/// It was answered until now by guessing from the class, and a class is a
/// guess: a 90s Miata is tagged `street` and `#small sports`, which this
/// project deliberately reads as "unrecognised" rather than pressing a car
/// into a box — and unrecognised was on the side that has a wing. So a car
/// with four adjustments in the whole of its setup screen was told to add
/// front wing and stiffen an anti-roll bar it does not have.
///
/// `setup.ini` is the car's own answer and the same one its setup screen is
/// built from: a section per adjustment, and no section where there is no
/// adjustment. Read it and the guess is not needed.
///
/// `None` for a car whose data cannot be read, which still means "ask the
/// class" — the guess is a poor answer and no answer at all is worse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Adjustables {
    /// A wing, front or rear: `WING_0`, `WING_1`, …
    pub wing: bool,
    /// `ARB_FRONT` or `ARB_REAR`.
    pub anti_roll_bar: bool,
    /// `SPRING_RATE_*`.
    pub springs: bool,
    /// `DAMP_*`, of any speed or direction.
    pub dampers: bool,
    /// `DIFF_POWER`, `DIFF_COAST` or `DIFF_PRELOAD`.
    pub differential: bool,
    /// `FRONT_BIAS`.
    pub brake_bias: bool,
    /// `ROD_LENGTH_*`, which is what AC calls ride height.
    pub ride_height: bool,
}

/// One thing a driver can pick to drive on.
///
/// **A layout is a track here, not a track with a note beside it.** Half the
/// circuits installed on the machine this was written on have more than one —
/// Barcelona has a grand prix and a moto layout, and they are different
/// lengths with different corners. A list that offered "Barcelona" and then
/// asked a second question would be a list that cannot be scrolled and picked
/// from, which is the whole job.
///
/// `config` is empty for a circuit that has only one layout, because that is
/// what the game's own configuration expects in that case — not the string
/// "default", and not the track's own name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackListing {
    /// The folder under `content/tracks`.
    pub id: String,
    /// The layout under it, or empty when the track has only one.
    pub config: String,
    /// What the track calls itself, with the layout named when there is one.
    pub name: String,
    pub country: String,
    /// Metres, or zero where the track does not say.
    pub length_m: u32,
    /// How many cars can start, or zero where the track does not say.
    pub pitboxes: u32,
}

impl TrackListing {
    /// What the game is told to load.
    ///
    /// Two strings rather than one: `race.ini` carries the track and the
    /// layout on separate lines, and joining them into a path here would mean
    /// taking them apart again there.
    pub fn as_game_asks(&self) -> (&str, &str) {
        (&self.id, &self.config)
    }
}

/// One livery a car can be driven in.
///
/// **The most visible choice there is.** It is what the car looks like on
/// screen and in every screenshot anybody takes, and the game writes it into
/// the session by folder name — so a launcher that does not offer it either
/// writes nothing and takes whatever the game picks, or writes a name it
/// guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkinListing {
    /// The folder under `skins`, which is what the session is told.
    pub id: String,
    /// What it calls itself, or the folder tidied up when it does not say.
    pub name: String,
    /// The racing number, where the livery carries one.
    pub number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarSpecs {
    pub id: String,
    pub name: String,
    pub brand: String,
    pub description: String,
    pub class: String,
    pub power: String,
    pub torque: String,
    pub weight: String,
    pub year: Option<i32>,
    pub power_hp: f32,
    pub weight_kg: f32,
    /// The hot pressure the car itself was built around, front and rear, psi.
    ///
    /// Out of the car's own `tyres.ini` — see
    /// [`assetto_corsa::car_data`](crate::games::assetto_corsa::car_data).
    /// `None` where the game does not ship one, or ships it in a form this
    /// could not read: the class table answers then, exactly as before this
    /// field existed.
    #[serde(default)]
    pub ideal_pressure: Option<(f32, f32)>,
    /// Which adjustments the car's setup screen actually offers.
    ///
    /// Out of the car's own `setup.ini` — see
    /// [`Adjustables`].
    /// `None` where it could not be read, and the class is guessed from then
    /// as it always was.
    #[serde(default)]
    pub adjustable: Option<Adjustables>,
}
