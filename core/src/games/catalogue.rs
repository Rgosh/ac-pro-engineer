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
}
