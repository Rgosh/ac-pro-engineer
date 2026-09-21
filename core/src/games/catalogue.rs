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
