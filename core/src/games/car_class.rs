//! What kind of car this is, and the numbers that follow from it.
//!
//! A GT3 on carbon brakes runs its fronts at 520 °C and is fine; a road car at
//! 520 °C has boiled its fluid. A Formula car wants its tyres above 90 °C; the
//! same reading on a street tyre is the far side of its window. Until this
//! existed the engineer judged both against **one** band — 70–105 °C and a
//! single 800 °C brake ceiling — which is a band that is wrong for almost
//! every car and merely quiet for the rest. That is why the advice went so
//! silent: not because there was nothing to say, but because the thresholds
//! were nobody's.
//!
//! # Where the class comes from
//!
//! The car's own id, which both games publish and which is descriptive in
//! both: `lamborghini_huracan_gt3_evo`, `mercedes_amg_gt4`, `ks_ferrari_sf70h`,
//! `porsche_991ii_gt3_cup`. Assetto Corsa also ships `ui_car.json` with tags —
//! `gt3`, `#GT4`, `singleseater`, `vintage` — and those are used first where
//! they are there, because they are the game's own answer rather than a guess
//! about a string.
//!
//! **`Unknown` is a real answer**, and the important one: a mod car with a
//! name nobody has seen keeps the driver's own settings rather than being
//! forced into a class it may not belong to.
//!
//! # Where the numbers come from
//!
//! Published operating windows, not invention:
//!
//! * **GT3** — ACC's dry Pirellis work between 70 and 100 °C with peak grip at
//!   80–90 °C, and iRacing's GT3s are quoted the same 80–90 °C. The recording
//!   this project pins Competizione to shows cores of 85–98 °C over two laps at
//!   Spa, which sits exactly there.
//! * **Brakes, carbon** — GT3 practice is fronts at or under 600–650 °C and
//!   rears near 450 °C, with real carbon-ceramic discs plateauing at
//!   550–750 °C. The same recording shows 520 °C front against 257 °C rear,
//!   which is why the ceilings here are per axle: one number for all four
//!   corners is either too low for the fronts or blind to the rears.
//! * **Formula** — slicks in the 80–110 °C band, with the harder compounds
//!   higher still.
//! * **GT4 and touring** — around 80–90 °C, the same order as GT3 on softer
//!   rubber and less downforce.
//! * **Road and vintage** — the application's original 70–105 °C, which was
//!   chosen against Assetto Corsa's street cars and is right for them.
//!
//! Sources are listed in `docs/plan-0.4.0-car-classes.md` beside the table, so
//! a number can be argued with rather than merely disbelieved.

/// The kind of car, as far as an engineer's numbers are concerned.
///
/// Deliberately coarse. The point is not to catalogue motorsport — it is that
/// four groups of cars want four different answers to "is this tyre hot", and
/// a finer split would be more classes than there is evidence to fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarClass {
    /// Single-seaters: Formula 1, 2, 3, and anything else with wings and no
    /// roof. The hottest tyres and the strongest brakes.
    Formula,
    /// Le Mans prototypes and their like.
    Prototype,
    /// GTE, GT2 and the GT1-era cars: slicks and carbon, a shade harder worked
    /// than GT3.
    GrandTouringPro,
    /// GT3, the class most of this project's evidence comes from.
    Gt3,
    /// GT4, and one-make cup cars on the same kind of rubber.
    Gt4,
    /// Saloons and touring cars — TCR, the ACC challengers, the road-derived
    /// racers.
    TouringCar,
    /// Road cars, including the fast ones. Street or semi-slick tyres and
    /// steel brakes.
    Road,
    /// Anything old enough that its tyres and brakes are of another era.
    Vintage,
    /// Not recognised — a mod, a name nobody has seen, or nothing loaded yet.
    ///
    /// The default, and it means "keep the driver's own settings". Guessing a
    /// class for an unknown car is how a mod ends up judged against numbers
    /// that were never about it.
    #[default]
    Unknown,
}

/// The operating windows one class expects, in the units the reading carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassWindow {
    /// Tyre temperature, °C: below is cold, above is overheating. Whichever
    /// measurement the game publishes — the core or the tread mean — is
    /// judged against this, and the two are close enough on a working tyre
    /// that one band covers both. What is *not* close enough is a GT3 against
    /// a road car, which is what this type is for.
    pub tyre_c: (f32, f32),
    /// What the front brakes may reach, °C.
    pub brake_front_max_c: f32,
    /// ...and the rears, which run far cooler on anything with a forward
    /// brake bias. One ceiling for all four is either too low for the fronts
    /// or blind to the rears.
    pub brake_rear_max_c: f32,
    /// Hot tyre pressure to aim for, psi, where the class has a customary one.
    pub hot_pressure_psi: f32,
}

impl CarClass {
    /// What this class calls normal.
    pub const fn window(self) -> ClassWindow {
        match self {
            // Slicks in the 80–110 band; carbon brakes that live above what a
            // GT3 would call cooked.
            CarClass::Formula => ClassWindow {
                tyre_c: (85.0, 110.0),
                brake_front_max_c: 900.0,
                brake_rear_max_c: 800.0,
                hot_pressure_psi: 21.0,
            },
            CarClass::Prototype => ClassWindow {
                tyre_c: (80.0, 105.0),
                brake_front_max_c: 750.0,
                brake_rear_max_c: 600.0,
                hot_pressure_psi: 24.0,
            },
            CarClass::GrandTouringPro => ClassWindow {
                tyre_c: (80.0, 100.0),
                brake_front_max_c: 700.0,
                brake_rear_max_c: 500.0,
                hot_pressure_psi: 27.0,
            },
            // The class with the most evidence behind it: 70–100 working,
            // 80–90 at peak, and the fronts at 600–650 before it matters.
            CarClass::Gt3 => ClassWindow {
                tyre_c: (75.0, 100.0),
                brake_front_max_c: 650.0,
                brake_rear_max_c: 500.0,
                hot_pressure_psi: 27.5,
            },
            CarClass::Gt4 => ClassWindow {
                tyre_c: (75.0, 95.0),
                brake_front_max_c: 600.0,
                brake_rear_max_c: 450.0,
                hot_pressure_psi: 27.0,
            },
            CarClass::TouringCar => ClassWindow {
                tyre_c: (70.0, 95.0),
                brake_front_max_c: 550.0,
                brake_rear_max_c: 450.0,
                hot_pressure_psi: 28.0,
            },
            // The application's original band, which was chosen against
            // Assetto Corsa's street cars and is right for them.
            CarClass::Road | CarClass::Unknown => ClassWindow {
                tyre_c: (70.0, 105.0),
                brake_front_max_c: 500.0,
                brake_rear_max_c: 400.0,
                hot_pressure_psi: 27.5,
            },
            // Crossplies and drums do not want to be hot, and there is no
            // sense in a modern ceiling on brakes that fade at half of it.
            CarClass::Vintage => ClassWindow {
                tyre_c: (60.0, 90.0),
                brake_front_max_c: 400.0,
                brake_rear_max_c: 350.0,
                hot_pressure_psi: 26.0,
            },
        }
    }

    /// How it is said on a screen.
    pub const fn label(self) -> &'static str {
        match self {
            CarClass::Formula => "Formula",
            CarClass::Prototype => "Prototype",
            CarClass::GrandTouringPro => "GTE / GT2",
            CarClass::Gt3 => "GT3",
            CarClass::Gt4 => "GT4",
            CarClass::TouringCar => "Touring",
            CarClass::Road => "Road",
            CarClass::Vintage => "Vintage",
            CarClass::Unknown => "Unknown",
        }
    }

    /// Whether this class was recognised at all.
    ///
    /// What it gates: an unknown car keeps the driver's own thresholds. It is
    /// the same distinction the capability flags draw — not knowing and
    /// knowing a default are different answers.
    pub const fn is_known(self) -> bool {
        !matches!(self, CarClass::Unknown)
    }

    /// Work out the class from the car's id and, where a game supplies them,
    /// its own tags.
    ///
    /// Tags win: they are the game's answer rather than a guess about a
    /// string. Assetto Corsa ships them in `ui_car.json` (`gt3`, `#GT4`,
    /// `singleseater`, `vintage`); Competizione has none, and does not need
    /// them — every one of its car ids says what it is.
    pub fn identify(car_id: &str, tags: &[String]) -> Self {
        let tagged: Vec<String> = tags.iter().map(|tag| tag.to_lowercase()).collect();
        let has = |needle: &str| tagged.iter().any(|tag| tag.contains(needle));

        // Order matters throughout: `gt3` is a substring of nothing here, but
        // `gt4` and `gt3` both contain `gt`, and a cup car's id contains its
        // base class as well.
        if has("singleseater") || has("formula") || has("open wheel") {
            return CarClass::Formula;
        }
        if has("prototype") || has("lmp") {
            return CarClass::Prototype;
        }
        if has("gt4") {
            return CarClass::Gt4;
        }
        if has("gte") || has("gt2") || has("gt1") {
            return CarClass::GrandTouringPro;
        }
        if has("gt3") {
            return CarClass::Gt3;
        }
        if has("vintage") {
            return CarClass::Vintage;
        }
        if has("touring") || has("tcr") {
            return CarClass::TouringCar;
        }

        Self::from_id(car_id)
    }

    /// The class a car's id alone gives away.
    ///
    /// Both games name their cars descriptively, and Competizione's are
    /// exhaustively so — `lamborghini_huracan_gt3_evo2`,
    /// `mercedes_amg_gt4`, `porsche_992_gt3_cup`, `bmw_m2_cs_racing`. This is
    /// the whole classifier for that game and the fallback for the other.
    pub fn from_id(car_id: &str) -> Self {
        let id = car_id.to_lowercase();

        // Cup and one-make racers first: their ids carry the base class too,
        // so `porsche_992_gt3_cup` would otherwise read as a GT3 — which it
        // is not, on softer rubber and half the downforce.
        if id.contains("_cup") || id.contains("cup_") || id.contains("_st") && id.contains("audi") {
            return CarClass::Gt4;
        }
        if id.contains("gt4") {
            return CarClass::Gt4;
        }
        if id.contains("gte") || id.contains("gt2") || id.contains("_gt1") {
            return CarClass::GrandTouringPro;
        }
        if id.contains("gt3") {
            return CarClass::Gt3;
        }
        // Assetto Corsa's single-seaters, which have no common word in their
        // names: `ks_ferrari_sf70h`, `ks_ferrari_sf15t`, `lotus_exos_125`,
        // `ks_lotus_98t`, `dallara_f312`, `tatuusfa1`.
        if id.contains("formula")
            || id.contains("_sf7")
            || id.contains("_sf1")
            || id.contains("exos")
            || id.contains("dallara")
            || id.contains("tatuus")
            || id.contains("f2004")
            || id.contains("98t")
        {
            return CarClass::Formula;
        }
        if id.contains("lmp") || id.contains("_p1") || id.contains("prototype") {
            return CarClass::Prototype;
        }
        // Competizione's touring challengers, and the TCR-shaped things in
        // Assetto Corsa.
        if id.contains("cs_racing") || id.contains("tcr") || id.contains("_st_") {
            return CarClass::TouringCar;
        }

        CarClass::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Competizione's ids classify themselves, which is the whole reason that
    /// game needs no car catalogue on disk.
    #[test]
    fn competiziones_own_names_say_what_they_are() {
        for (id, expected) in [
            ("lamborghini_huracan_gt3_evo", CarClass::Gt3),
            ("ferrari_296_gt3", CarClass::Gt3),
            ("porsche_991ii_gt3_r", CarClass::Gt3),
            ("mercedes_amg_gt4", CarClass::Gt4),
            ("alpine_a110_gt4", CarClass::Gt4),
            ("porsche_991ii_gt3_cup", CarClass::Gt4),
            ("bmw_m2_cs_racing", CarClass::TouringCar),
            ("ferrari_296_gt2", CarClass::GrandTouringPro),
        ] {
            assert_eq!(CarClass::from_id(id), expected, "{id}");
        }
    }

    /// Assetto Corsa's, including the single-seaters, whose names share no
    /// common word at all.
    #[test]
    fn assetto_corsas_names_mostly_do_too() {
        for (id, expected) in [
            ("ks_ferrari_488_gt3", CarClass::Gt3),
            ("bmw_z4_gt3", CarClass::Gt3),
            ("ks_ferrari_sf70h", CarClass::Formula),
            ("ks_ferrari_sf15t", CarClass::Formula),
            ("lotus_exos_125", CarClass::Formula),
            ("tatuusfa1", CarClass::Formula),
            ("ks_audi_r18_etron_quattro", CarClass::Unknown),
            ("abarth500", CarClass::Unknown),
        ] {
            assert_eq!(CarClass::from_id(id), expected, "{id}");
        }
    }

    /// A game's own tags beat a guess about a string, and Assetto Corsa has
    /// them.
    #[test]
    fn the_games_own_tags_win() {
        let tags = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        assert_eq!(
            CarClass::identify("abarth500", &tags(&["street", "hot hatchback"])),
            CarClass::Unknown,
            "a road car with no class tag stays unknown rather than being guessed"
        );
        assert_eq!(
            CarClass::identify("ks_mazda_787b", &tags(&["#Vintage GT", "race"])),
            CarClass::Vintage
        );
        assert_eq!(
            CarClass::identify("some_mod_car", &tags(&["singleseater", "gp"])),
            CarClass::Formula,
            "a mod that carries the game's tags is classified by them"
        );
        assert_eq!(
            CarClass::identify("some_mod_car", &[]),
            CarClass::Unknown,
            "and one that carries nothing is not"
        );
    }

    /// The windows have to be windows: a minimum below a maximum, and the
    /// fronts allowed to run hotter than the rears on every class.
    #[test]
    fn every_window_is_the_right_way_round() {
        for class in [
            CarClass::Formula,
            CarClass::Prototype,
            CarClass::GrandTouringPro,
            CarClass::Gt3,
            CarClass::Gt4,
            CarClass::TouringCar,
            CarClass::Road,
            CarClass::Vintage,
            CarClass::Unknown,
        ] {
            let window = class.window();
            assert!(
                window.tyre_c.0 < window.tyre_c.1,
                "{}: {:?}",
                class.label(),
                window.tyre_c
            );
            assert!(
                window.brake_front_max_c >= window.brake_rear_max_c,
                "{}: fronts do the work",
                class.label()
            );
            assert!(
                (15.0..=45.0).contains(&window.hot_pressure_psi),
                "{}: {} psi is not a tyre pressure",
                class.label(),
                window.hot_pressure_psi
            );
        }
    }

    /// The one that matters for Competizione: the recording this project pins
    /// that game to has to sit inside the class it belongs to.
    ///
    /// 85–98 °C cores and 520 °C fronts against 257 °C rears, from a Huracán
    /// GT3 EVO at Spa. A window that called any of that an alarm would be a
    /// window that cries wolf for a whole stint.
    #[test]
    fn the_recorded_gt3_session_sits_inside_the_gt3_window() {
        let window = CarClass::Gt3.window();
        for core in [85.3, 87.1, 87.9, 92.5, 98.0] {
            assert!(
                core > window.tyre_c.0 && core < window.tyre_c.1,
                "{core} °C is normal running for a GT3 and must not be an alert"
            );
        }
        assert!(519.7 < window.brake_front_max_c, "the fronts peaked at 520");
        assert!(257.2 < window.brake_rear_max_c, "and the rears at 257");
    }
}
