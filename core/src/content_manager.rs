//! The car catalogue, once it has been read.
//!
//! A cache and a lookup, and nothing about any game's file layout: the scan
//! itself lives in the game's own folder — `games/assetto_corsa/content.rs` —
//! and hands over [`CarSpecs`]. This module used to do the walking, which meant
//! the neutral half of the program knew that cars live in `content/cars` and
//! that their specs are in a JSON file spelling horsepower "bhp".

use crate::games::catalogue::CarSpecs;

#[derive(Debug, Clone, Default)]
pub struct ContentManager {
    pub cars: Vec<CarSpecs>,
}

impl ContentManager {
    /// An empty catalogue, for a machine with no game installed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Hold a catalogue somebody else has read.
    pub fn from_cars(cars: Vec<CarSpecs>) -> Self {
        Self { cars }
    }

    pub fn get_car_specs(&self, car_id: &str) -> Option<&CarSpecs> {
        self.cars.iter().find(|c| c.id == car_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(id: &str) -> CarSpecs {
        CarSpecs {
            id: id.to_string(),
            name: id.to_string(),
            brand: "Brand".to_string(),
            description: String::new(),
            class: "street".to_string(),
            power: "300bhp".to_string(),
            torque: "400Nm".to_string(),
            weight: "1200kg".to_string(),
            year: Some(2020),
            power_hp: 300.0,
            weight_kg: 1200.0,
        }
    }

    #[test]
    fn a_car_is_found_by_the_id_the_game_uses() {
        let manager = ContentManager::from_cars(vec![car("ks_ferrari_sf70h")]);
        assert!(manager.get_car_specs("ks_ferrari_sf70h").is_some());
        assert!(manager.get_car_specs("something_else").is_none());
    }

    /// No game installed, so no catalogue — and every lookup says so rather
    /// than the manager refusing to exist.
    #[test]
    fn an_empty_catalogue_answers_rather_than_failing() {
        assert!(ContentManager::new().get_car_specs("anything").is_none());
    }
}
