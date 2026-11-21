//! A fair grid: the game's drivers ballasted to the car you drive.
//!
//! **Weight is the one equaliser that is exact.** Ballast is mass the game
//! adds to a car, and a car's power-to-weight is its power over its mass —
//! so the ballast that brings a quicker car down to yours is arithmetic, not
//! a guess. The game's restrictor is left alone: what it does to power
//! depends on the engine, and a grid balanced by a guess is not balanced.
//!
//! A car slower than yours cannot be made quicker by ballast, and a car
//! quicker than the most ballast can bring down stays quicker; both are said,
//! not hidden, so the driver knows what kind of race it will be.

/// The most ballast the game takes on a car, kg.
pub const MOST_BALLAST_KG: f32 = 300.0;

/// What one car is given, and what it is then.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Share {
    pub ballast_kg: u32,
    /// Its kilograms per horsepower with that ballast.
    pub kg_per_hp: f32,
    /// How it then compares with yours.
    pub then: Against,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Against {
    /// Within two per cent of your car.
    Even,
    /// Still quicker: the most ballast was not enough.
    Quicker,
    /// Slower than your car to begin with; ballast cannot help it.
    Slower,
    /// Its power or weight is not known, so it is left as it is.
    Unknown,
}

/// The ballast that brings each car to `yours` — power in horsepower and
/// mass in kilograms — or as near as the game allows.
pub fn fair(yours: (f32, f32), theirs: &[(f32, f32)]) -> Vec<Share> {
    let known = |(hp, kg): (f32, f32)| hp.is_finite() && kg.is_finite() && hp > 1.0 && kg > 1.0;
    let target = match known(yours) {
        true => yours.1 / yours.0,
        false => {
            return theirs
                .iter()
                .map(|_| Share {
                    ballast_kg: 0,
                    kg_per_hp: 0.0,
                    then: Against::Unknown,
                })
                .collect();
        }
    };
    theirs
        .iter()
        .map(|&(hp, kg)| {
            if !known((hp, kg)) {
                return Share {
                    ballast_kg: 0,
                    kg_per_hp: 0.0,
                    then: Against::Unknown,
                };
            }
            let wanted = (target * hp - kg).max(0.0);
            let ballast = wanted.min(MOST_BALLAST_KG).round();
            let ratio = (kg + ballast) / hp;
            let then = match ratio / target {
                r if r < 0.98 => Against::Quicker,
                r if r > 1.02 => Against::Slower,
                _ => Against::Even,
            };
            Share {
                ballast_kg: ballast as u32,
                kg_per_hp: ratio,
                then,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quicker_cars_are_brought_down_to_yours_and_no_further() {
        // Yours: 300 hp, 1200 kg — 4 kg/hp.
        let shares = fair(
            (300.0, 1200.0),
            &[
                (350.0, 1200.0),
                (300.0, 1200.0),
                (250.0, 1300.0),
                (600.0, 1000.0),
                (0.0, 900.0),
            ],
        );
        // 350 hp wants 1400 kg: 200 of ballast, even.
        assert_eq!(shares[0].ballast_kg, 200);
        assert_eq!(shares[0].then, Against::Even);
        assert_eq!(shares[1].ballast_kg, 0);
        assert_eq!(shares[1].then, Against::Even);
        // Slower to begin with: nothing to add.
        assert_eq!(shares[2].ballast_kg, 0);
        assert_eq!(shares[2].then, Against::Slower);
        // Would want 1400 kg of ballast: given the most, and said to be quicker.
        assert_eq!(shares[3].ballast_kg, 300);
        assert_eq!(shares[3].then, Against::Quicker);
        assert_eq!(shares[4].then, Against::Unknown);
    }

    #[test]
    fn an_unknown_car_of_yours_changes_nothing() {
        let shares = fair((0.0, 1200.0), &[(500.0, 1000.0)]);
        assert_eq!(shares[0].ballast_kg, 0);
        assert_eq!(shares[0].then, Against::Unknown);
    }
}
