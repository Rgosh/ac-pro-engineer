//! A car's physics: read, understood, and tuned — with the original kept.
//!
//! **What the car is, out of its own files.** Weight from `car.ini`, the
//! engine's torque curve from the file `engine.ini` names (`power.lut`), its
//! limiter and turbo, the gearbox and final drive from `drivetrain.ini`, the
//! driven wheels' radius from `tyres.ini`, the brakes from `brakes.ini` —
//! from the car's `data/` folder or from its `data.acd` archive, the same two
//! places [`super::car_data`] reads.
//!
//! **What follows from it is arithmetic and nothing else.** Power is torque
//! times revs; the speed at the limiter in each gear is the wheel's
//! circumference turned by the gearbox. Nothing here estimates a lap time or
//! a 0–100: those depend on grip, aero and the driver, and a number that
//! looks measured and is not is this project's worst kind of bug.
//!
//! # Tuning, and getting the car back
//!
//! [`Tune`] is a few numbers — weight, power, the limiter, every gear and
//! the final drive, brake torque and balance — always applied to the car's
//! **original** physics, never on top of a previous tune. [`apply`] writes
//! them:
//!
//! * a Kunos car's `data.acd` is unpacked to `data/` once and the archive
//!   set aside as `data.acd.pro-launcher-original` — the game reads `data/`
//!   when there is no archive beside it;
//! * a car with its own `data/` keeps each file it changes once, beside
//!   it, as `<file>.pro-launcher-original` — and if a `data.acd` sits
//!   beside the folder too, the archive is set aside the same way, because
//!   the game reads an archive before a folder and would never see the
//!   tune. The folder is never overwritten by an unpacked archive;
//! * a note, `data/pro-launcher-tune.json`, says which it was and what tune
//!   is on.
//!
//! [`restore`] undoes exactly that and nothing else. A car tuned here is a
//! different car to a server that checks its files, which is why the page
//! that offers this says so before anything is written.

use super::car_data;
use super::ini;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The note left in `data/` while a tune is on.
pub const NOTE: &str = "pro-launcher-tune.json";
/// What the archive is renamed to while its unpacked copy is in use.
pub const ARCHIVE_KEPT: &str = "data.acd.pro-launcher-original";
/// What a mod's own file is kept as before it is first changed.
const KEPT_SUFFIX: &str = ".pro-launcher-original";

/// Which wheels are driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Drive {
    Rear,
    Front,
    All,
}

/// A car's physics, as far as this reads them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Physics {
    /// `car.ini`'s total mass, kg.
    pub mass_kg: f32,
    /// The torque curve: revs, newton-metres — the file's own points.
    pub torque: Vec<(f32, f32)>,
    /// Where the engine is cut, rpm.
    pub limiter_rpm: f32,
    /// Full boost, as a fraction added to the curve; zero for no turbo.
    pub boost: f32,
    /// Forward gears, first first.
    pub gears: Vec<f32>,
    pub final_drive: f32,
    pub drive: Drive,
    /// The driven wheels' radius, metres.
    pub wheel_radius_m: f32,
    /// Brake torque at full pedal, Nm, and the share at the front.
    pub brake_torque: f32,
    pub brake_front: f32,
}

/// What is read from where — the files a tune changes.
const FILES: [&str; 5] = [
    "car.ini",
    "engine.ini",
    "drivetrain.ini",
    "brakes.ini",
    "tyres.ini",
];

/// A car's files, read through whichever form they are in — the original
/// ones when a tune is on.
struct Source {
    folder: PathBuf,
    archive: Option<(Vec<u8>, String)>,
}

impl Source {
    fn of(car_folder: &Path, original: bool) -> Option<Self> {
        let name = car_folder.file_name()?.to_str()?.to_string();
        // The archive set aside is the original only when the tune was
        // unpacked from it; set aside from beside a folder, it is not what
        // the folder's own originals are.
        let from_archive = on(car_folder).is_some_and(|note| note.unpacked);
        let kept = car_folder.join(ARCHIVE_KEPT);
        let archive = match (original && from_archive, std::fs::read(&kept)) {
            (true, Ok(bytes)) => Some((bytes, name)),
            _ => None,
        };
        Some(Source {
            folder: car_folder.to_path_buf(),
            archive,
        })
    }

    fn file(&self, name: &str, original: bool) -> Option<String> {
        if let Some((bytes, folder)) = &self.archive {
            return car_data::file_from_archive(bytes, folder, name);
        }
        if original {
            let kept = self
                .folder
                .join("data")
                .join(format!("{name}{KEPT_SUFFIX}"));
            if let Ok(text) = std::fs::read_to_string(kept) {
                return Some(text);
            }
        }
        car_data::data_file(&self.folder, name)
    }
}

fn number(text: &str, section: &str, key: &str) -> Option<f32> {
    ini::value(text, section, key)?
        .split_whitespace()
        .next()?
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

/// A `.lut`: `x|y` a line, comments after `;`.
pub fn lut(text: &str) -> Vec<(f32, f32)> {
    text.lines()
        .filter_map(|line| {
            let line = line.split([';', '#']).next()?.trim();
            let (x, y) = line.split_once('|')?;
            let (x, y) = (x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?);
            (x.is_finite() && y.is_finite()).then_some((x, y))
        })
        .collect()
}

impl Physics {
    /// The car as it is now — tuned or not.
    pub fn read(car_folder: &Path) -> Option<Physics> {
        Self::read_from(&Source::of(car_folder, false)?, false)
    }

    /// The car as it came, under any tune that is on.
    pub fn original(car_folder: &Path) -> Option<Physics> {
        Self::read_from(&Source::of(car_folder, true)?, true)
    }

    fn read_from(source: &Source, original: bool) -> Option<Physics> {
        let car = source.file("car.ini", original)?;
        let engine = source.file("engine.ini", original)?;
        let drivetrain = source.file("drivetrain.ini", original)?;
        let tyres = source.file("tyres.ini", original).unwrap_or_default();
        let brakes = source.file("brakes.ini", original).unwrap_or_default();

        let curve_name =
            ini::value(&engine, "HEADER", "POWER_CURVE").unwrap_or_else(|| "power.lut".to_string());
        let torque = lut(&source.file(&curve_name, original)?);
        if torque.len() < 2 {
            return None;
        }
        let mass_kg = number(&car, "BASIC", "TOTALMASS")?;
        let limiter_rpm = number(&engine, "ENGINE_DATA", "LIMITER")
            .unwrap_or_else(|| torque.last().map_or(7000.0, |p| p.0));
        // Each turbo's full boost, held to its wastegate where it has one.
        let boost: f32 = (0..4)
            .filter_map(|k| {
                let section = format!("TURBO_{k}");
                let max = number(&engine, &section, "MAX_BOOST")?;
                let gate = number(&engine, &section, "WASTEGATE").filter(|g| *g > 0.0);
                Some(gate.map_or(max, |g| max.min(g)))
            })
            .sum();
        let count = number(&drivetrain, "GEARS", "COUNT").unwrap_or(0.0) as usize;
        let gears: Vec<f32> = (1..=count.min(10))
            .filter_map(|g| number(&drivetrain, "GEARS", &format!("GEAR_{g}")))
            .collect();
        let final_drive = number(&drivetrain, "GEARS", "FINAL")?;
        let drive = match ini::value(&drivetrain, "TRACTION", "TYPE")
            .unwrap_or_default()
            .to_uppercase()
            .as_str()
        {
            "FWD" => Drive::Front,
            "AWD" | "AWD2" => Drive::All,
            _ => Drive::Rear,
        };
        let axle = match drive {
            Drive::Front => "FRONT",
            _ => "REAR",
        };
        let wheel_radius_m = number(&tyres, axle, "RADIUS")
            .filter(|r| (0.15..=0.6).contains(r))
            .unwrap_or(0.32);
        let physics = Physics {
            mass_kg,
            torque,
            limiter_rpm,
            boost: boost.clamp(0.0, 5.0),
            gears,
            final_drive,
            drive,
            wheel_radius_m,
            brake_torque: number(&brakes, "DATA", "MAX_TORQUE").unwrap_or(0.0),
            brake_front: number(&brakes, "DATA", "FRONT_SHARE").unwrap_or(0.0),
        };
        physics.believable().then_some(physics)
    }

    /// Whether these numbers came from a car: a mass a car has, a gearbox
    /// that turns, revs an engine reaches. An archive read with the wrong
    /// key would fail here rather than show a 12-tonne car with no gears.
    fn believable(&self) -> bool {
        (100.0..=20_000.0).contains(&self.mass_kg)
            && (1000.0..=25_000.0).contains(&self.limiter_rpm)
            && !self.gears.is_empty()
            && self.gears.iter().all(|g| *g > 0.0 && *g < 20.0)
            && self.final_drive > 0.0
            && self.final_drive < 20.0
    }

    /// Torque at `rpm` with full boost, from the curve's points.
    pub fn torque_at(&self, rpm: f32) -> f32 {
        let curve = &self.torque;
        let raw = match curve.iter().position(|p| p.0 >= rpm) {
            None => curve.last().map_or(0.0, |p| p.1),
            Some(0) => curve.first().map_or(0.0, |p| p.1),
            Some(i) => {
                let (a, b) = (curve[i - 1], curve[i]);
                let t = if b.0 > a.0 {
                    (rpm - a.0) / (b.0 - a.0)
                } else {
                    0.0
                };
                a.1 + (b.1 - a.1) * t
            }
        };
        raw * (1.0 + self.boost)
    }

    /// Torque and horsepower over the rev range, `steps` points from the
    /// curve's first revs to the limiter.
    pub fn curve(&self, steps: usize) -> Vec<(f32, f32, f32)> {
        let low = self.torque.first().map_or(1000.0, |p| p.0.max(500.0));
        let high = self.limiter_rpm.max(low + 1.0);
        (0..=steps.max(1))
            .map(|k| {
                let rpm = low + (high - low) * k as f32 / steps.max(1) as f32;
                let nm = self.torque_at(rpm);
                (rpm, nm, horsepower(nm, rpm))
            })
            .collect()
    }

    /// The most power and the most torque, and where.
    pub fn peaks(&self) -> Peaks {
        let points = self.curve(200);
        let power = points
            .iter()
            .copied()
            .fold(
                (0.0f32, 0.0f32),
                |best, (rpm, _, hp)| if hp > best.1 { (rpm, hp) } else { best },
            );
        let torque = points
            .iter()
            .copied()
            .fold(
                (0.0f32, 0.0f32),
                |best, (rpm, nm, _)| if nm > best.1 { (rpm, nm) } else { best },
            );
        Peaks {
            power_hp: power.1,
            power_rpm: power.0,
            torque_nm: torque.1,
            torque_rpm: torque.0,
            kg_per_hp: match power.1 > 0.0 {
                true => self.mass_kg / power.1,
                false => 0.0,
            },
        }
    }

    /// Road speed at `rpm` in gear `gear` (1 is first), km/h.
    pub fn speed_kmh(&self, gear: usize, rpm: f32) -> f32 {
        let Some(ratio) = gear.checked_sub(1).and_then(|g| self.gears.get(g)) else {
            return 0.0;
        };
        let wheel_rpm = rpm / (ratio * self.final_drive);
        wheel_rpm * std::f32::consts::TAU * self.wheel_radius_m * 60.0 / 1000.0
    }

    /// Each gear's speed at the limiter, km/h — what the gearbox allows, not
    /// what the car reaches: drag and grip decide that.
    pub fn gear_speeds(&self) -> Vec<f32> {
        (1..=self.gears.len())
            .map(|g| self.speed_kmh(g, self.limiter_rpm))
            .collect()
    }

    /// The car with a tune applied — the preview the page draws.
    pub fn tuned(&self, tune: &Tune) -> Physics {
        let tune = tune.settled(self);
        let mut out = self.clone();
        out.mass_kg = tune.mass_kg;
        for point in &mut out.torque {
            point.1 *= tune.power;
        }
        out.limiter_rpm = tune.limiter_rpm;
        out.gears.clone_from(&tune.gears);
        out.final_drive = tune.final_drive;
        out.brake_torque = self.brake_torque * tune.brakes;
        out.brake_front = tune.brake_front;
        out
    }
}

/// Horsepower from torque and revs.
pub fn horsepower(nm: f32, rpm: f32) -> f32 {
    nm * rpm / 7121.0
}

/// A car's peaks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peaks {
    pub power_hp: f32,
    pub power_rpm: f32,
    pub torque_nm: f32,
    pub torque_rpm: f32,
    pub kg_per_hp: f32,
}

/// The numbers a tune changes, always against the original car.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tune {
    pub mass_kg: f32,
    /// The torque curve scaled by this: 1.1 is ten per cent more.
    pub power: f32,
    pub limiter_rpm: f32,
    pub gears: Vec<f32>,
    pub final_drive: f32,
    /// Brake torque scaled by this.
    pub brakes: f32,
    pub brake_front: f32,
}

impl Tune {
    /// No change: the car as it is.
    pub fn of(car: &Physics) -> Tune {
        Tune {
            mass_kg: car.mass_kg,
            power: 1.0,
            limiter_rpm: car.limiter_rpm,
            gears: car.gears.clone(),
            final_drive: car.final_drive,
            brakes: 1.0,
            brake_front: car.brake_front,
        }
    }

    /// Held to what the car can take: half to twice its weight and power,
    /// the limiter within a third of where it was, ratios a gearbox has.
    pub fn settled(&self, car: &Physics) -> Tune {
        let within = |v: f32, lo: f32, hi: f32, otherwise: f32| match v.is_finite() {
            true => v.clamp(lo, hi),
            false => otherwise,
        };
        let mut gears = car.gears.clone();
        for (g, wanted) in gears.iter_mut().zip(&self.gears) {
            *g = within(*wanted, 0.3, 8.0, *g);
        }
        Tune {
            mass_kg: within(
                self.mass_kg,
                car.mass_kg * 0.5,
                car.mass_kg * 2.0,
                car.mass_kg,
            ),
            power: within(self.power, 0.5, 2.0, 1.0),
            limiter_rpm: within(
                self.limiter_rpm,
                car.limiter_rpm * 0.7,
                car.limiter_rpm * 1.3,
                car.limiter_rpm,
            ),
            gears,
            final_drive: within(self.final_drive, 1.5, 9.0, car.final_drive),
            brakes: within(self.brakes, 0.5, 2.0, 1.0),
            brake_front: within(self.brake_front, 0.3, 0.8, car.brake_front),
        }
    }

    /// Whether this changes anything at all.
    pub fn is_nothing(&self, car: &Physics) -> bool {
        *self == Tune::of(car)
    }
}

/// What the note in `data/` says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// The car shipped a `data.acd` and it was unpacked here.
    pub unpacked: bool,
    /// A `data.acd` beside the car's own folder was set aside so the game
    /// reads the folder.
    #[serde(default)]
    pub archive_aside: bool,
    pub tune: Tune,
}

/// The tune on this car, if one is.
pub fn on(car_folder: &Path) -> Option<Note> {
    let text = std::fs::read_to_string(car_folder.join("data").join(NOTE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write a tune into the car, against its original physics.
pub fn apply(car_folder: &Path, tune: &Tune) -> Result<Note, String> {
    let original = Physics::original(car_folder).ok_or("this car's physics could not be read")?;
    let tune = tune.settled(&original);
    let data = car_folder.join("data");
    let note_before = on(car_folder);
    let archive = car_folder.join("data.acd");

    // From an archive only when there is no folder of the car's own: a
    // folder beside an archive is what somebody who unpacked it uses, and
    // it is never written over.
    let unpacked = match &note_before {
        Some(note) => note.unpacked,
        None => archive.is_file() && !data.is_dir(),
    };
    let archive_aside = match &note_before {
        Some(note) => note.archive_aside,
        None => !unpacked && archive.is_file(),
    };
    if note_before.is_none() && unpacked {
        let bytes = std::fs::read(&archive).map_err(|e| e.to_string())?;
        let files = car_data::unpack_all(car_folder, &bytes)
            .ok_or("this car's data.acd could not be unpacked")?;
        for (name, body) in &files {
            if name.contains(['/', '\\']) || name.starts_with('.') {
                continue;
            }
            crate::atomic_file::write_atomic(&data.join(name), body).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&archive, car_folder.join(ARCHIVE_KEPT)).map_err(|e| e.to_string())?;
    }

    // Each file changed is the original's text with its numbers replaced.
    let source = Source::of(car_folder, true).ok_or("no car folder")?;
    let file = |name: &str| source.file(name, true).ok_or(format!("{name} is missing"));
    let mut changed: Vec<(String, String)> = Vec::new();

    let car = ini::with_value(
        &file("car.ini")?,
        "BASIC",
        "TOTALMASS",
        &format!("{:.1}", tune.mass_kg),
    );
    changed.push(("car.ini".into(), car));

    let engine_text = file("engine.ini")?;
    let curve_name = ini::value(&engine_text, "HEADER", "POWER_CURVE")
        .unwrap_or_else(|| "power.lut".to_string());
    let engine = ini::with_value(
        &engine_text,
        "ENGINE_DATA",
        "LIMITER",
        &format!("{:.0}", tune.limiter_rpm),
    );
    changed.push(("engine.ini".into(), engine));
    let curve: String = lut(&file(&curve_name)?)
        .iter()
        .map(|(rpm, nm)| format!("{rpm}|{:.2}\n", nm * tune.power))
        .collect();
    changed.push((curve_name, curve));

    let mut drivetrain = file("drivetrain.ini")?;
    for (g, ratio) in tune.gears.iter().enumerate() {
        drivetrain = ini::with_value(
            &drivetrain,
            "GEARS",
            &format!("GEAR_{}", g + 1),
            &format!("{ratio:.3}"),
        );
    }
    drivetrain = ini::with_value(
        &drivetrain,
        "GEARS",
        "FINAL",
        &format!("{:.3}", tune.final_drive),
    );
    changed.push(("drivetrain.ini".into(), drivetrain));

    if let Ok(brakes) = file("brakes.ini") {
        let brakes = ini::with_value(
            &brakes,
            "DATA",
            "MAX_TORQUE",
            &format!("{:.0}", original.brake_torque * tune.brakes),
        );
        let brakes = ini::with_value(
            &brakes,
            "DATA",
            "FRONT_SHARE",
            &format!("{:.3}", tune.brake_front),
        );
        changed.push(("brakes.ini".into(), brakes));
    }

    for (name, text) in &changed {
        let at = data.join(name);
        // A mod's own file is kept once, the first time it is changed.
        if !unpacked {
            let kept = data.join(format!("{name}{KEPT_SUFFIX}"));
            if at.is_file() && !kept.exists() {
                std::fs::copy(&at, &kept).map_err(|e| e.to_string())?;
            }
        }
        crate::atomic_file::write_atomic(&at, text.as_bytes()).map_err(|e| e.to_string())?;
    }
    // The archive beside a folder goes aside last, once the folder holds
    // the tune: until then the game still reads the car as it came.
    if archive_aside && archive.is_file() {
        std::fs::rename(&archive, car_folder.join(ARCHIVE_KEPT)).map_err(|e| e.to_string())?;
    }
    let note = Note {
        unpacked,
        archive_aside,
        tune,
    };
    let text = serde_json::to_string_pretty(&note).map_err(|e| e.to_string())?;
    crate::atomic_file::write_atomic(&data.join(NOTE), text.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(note)
}

/// Put the car back as it came, undoing exactly what [`apply`] did.
pub fn restore(car_folder: &Path) -> Result<(), String> {
    let Some(note) = on(car_folder) else {
        return Ok(());
    };
    let data = car_folder.join("data");
    if note.unpacked {
        let kept = car_folder.join(ARCHIVE_KEPT);
        if !kept.is_file() {
            return Err("the car's own data.acd is not where it was kept".to_string());
        }
        // The archive back first: if removing the folder fails part way,
        // the game reads the archive and the car is still the original.
        std::fs::rename(&kept, car_folder.join("data.acd")).map_err(|e| e.to_string())?;
        std::fs::remove_dir_all(&data).map_err(|e| e.to_string())?;
        return Ok(());
    }
    for entry in std::fs::read_dir(&data)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(own) = name.strip_suffix(KEPT_SUFFIX) {
            std::fs::rename(entry.path(), data.join(own)).map_err(|e| e.to_string())?;
        }
    }
    std::fs::remove_file(data.join(NOTE)).map_err(|e| e.to_string())?;
    let kept = car_folder.join(ARCHIVE_KEPT);
    if note.archive_aside && kept.is_file() {
        std::fs::rename(&kept, car_folder.join("data.acd")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The files a tune touches, for whoever has to explain it.
pub fn files() -> &'static [&'static str] {
    &FILES
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const CAR: &str = "[HEADER]\nVERSION=2\n[BASIC]\nTOTALMASS=1200 ; with driver\n";
    const ENGINE: &str = "[HEADER]\nVERSION=1\nPOWER_CURVE=power.lut\n[ENGINE_DATA]\nLIMITER=7000\n[TURBO_0]\nMAX_BOOST=0.5\nWASTEGATE=0.4\n";
    const POWER: &str = "1000|200\n4000|300\n7000|250 ; top\n";
    const DRIVETRAIN: &str = "[TRACTION]\nTYPE=RWD\n[GEARS]\nCOUNT=3\nGEAR_R=-3.0\nGEAR_1=3.0\nGEAR_2=2.0\nGEAR_3=1.0\nFINAL=4.0\n";
    const TYRES: &str = "[FRONT]\nRADIUS=0.30\n[REAR]\nRADIUS=0.33\n";
    const BRAKES: &str = "[DATA]\nMAX_TORQUE=2000\nFRONT_SHARE=0.62\n";

    fn files() -> Vec<(&'static str, &'static str)> {
        vec![
            ("car.ini", CAR),
            ("engine.ini", ENGINE),
            ("power.lut", POWER),
            ("drivetrain.ini", DRIVETRAIN),
            ("tyres.ini", TYRES),
            ("brakes.ini", BRAKES),
        ]
    }

    fn scratch(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("acpe-physics-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        at.join(name)
    }

    fn loose_car(name: &str) -> PathBuf {
        let car = scratch(name);
        for (file, text) in files() {
            crate::atomic_file::write_atomic(&car.join("data").join(file), text.as_bytes())
                .unwrap();
        }
        car
    }

    fn packed_car(name: &str) -> PathBuf {
        let car = scratch(name);
        let owned: Vec<(String, Vec<u8>)> = files()
            .iter()
            .map(|(n, t)| (n.to_string(), t.as_bytes().to_vec()))
            .collect();
        let archive = car_data::pack_for_test(name, &owned);
        crate::atomic_file::write_atomic(&car.join("data.acd"), &archive).unwrap();
        car
    }

    #[test]
    fn a_car_is_read_and_the_arithmetic_follows() {
        let car = loose_car("ks_test");
        let p = Physics::read(&car).unwrap();
        assert_eq!(p.mass_kg, 1200.0);
        assert_eq!(p.limiter_rpm, 7000.0);
        assert!((p.boost - 0.4).abs() < 1e-6, "held to the wastegate");
        assert_eq!(p.gears, vec![3.0, 2.0, 1.0]);
        assert_eq!(p.drive, Drive::Rear);
        assert_eq!(p.wheel_radius_m, 0.33, "the driven wheels' radius");
        // 300 Nm at 4000 with 40 % boost is 420 Nm.
        assert!((p.torque_at(4000.0) - 420.0).abs() < 0.01);
        // Third gear at 7000: 7000 / 4 = 1750 wheel rpm; × 2π × 0.33 m × 60 / 1000.
        let expected = 1750.0 * std::f32::consts::TAU * 0.33 * 60.0 / 1000.0;
        assert!((p.gear_speeds()[2] - expected).abs() < 0.01);
        let peaks = p.peaks();
        assert!(peaks.power_hp > 0.0 && peaks.kg_per_hp > 0.0);
        assert!((peaks.torque_nm - 420.0).abs() < 1.0);
    }

    /// **A Kunos car's archive is read, tuned, and given back byte for
    /// byte** — the archive kept aside, the folder removed, nothing left.
    #[test]
    fn an_archived_car_is_tuned_and_given_back() {
        let car = packed_car("ks_packed");
        let before = std::fs::read(car.join("data.acd")).unwrap();
        let p = Physics::read(&car).unwrap();
        let mut tune = Tune::of(&p);
        tune.power = 1.2;
        tune.mass_kg = 1100.0;
        tune.final_drive = 3.5;
        let note = apply(&car, &tune).unwrap();
        assert!(note.unpacked);
        assert!(!car.join("data.acd").exists());
        assert!(car.join(ARCHIVE_KEPT).is_file());
        let now = Physics::read(&car).unwrap();
        assert_eq!(now.mass_kg, 1100.0);
        assert_eq!(now.final_drive, 3.5);
        assert!((now.torque_at(4000.0) - 420.0 * 1.2).abs() < 0.1);
        // The original is still readable under the tune.
        assert_eq!(Physics::original(&car).unwrap(), p);

        // A second tune is against the original, not on top of the first.
        let mut again = Tune::of(&p);
        again.power = 1.1;
        apply(&car, &again).unwrap();
        assert!((Physics::read(&car).unwrap().torque_at(4000.0) - 420.0 * 1.1).abs() < 0.1);
        assert_eq!(Physics::read(&car).unwrap().mass_kg, 1200.0);

        restore(&car).unwrap();
        assert_eq!(std::fs::read(car.join("data.acd")).unwrap(), before);
        assert!(!car.join("data").exists());
        assert!(on(&car).is_none());
    }

    /// A mod's loose files keep their originals beside them, comments and
    /// all, and come back exactly.
    #[test]
    fn a_mod_s_own_files_come_back_exactly() {
        let car = loose_car("mod_loose");
        let p = Physics::read(&car).unwrap();
        let mut tune = Tune::of(&p);
        tune.gears = vec![3.2, 2.1, 1.1];
        tune.brake_front = 0.58;
        apply(&car, &tune).unwrap();
        let text = std::fs::read_to_string(car.join("data/drivetrain.ini")).unwrap();
        assert!(text.contains("GEAR_1=3.200"), "{text}");
        assert!(
            text.contains("GEAR_R=-3.0"),
            "the rest of the file is untouched"
        );
        restore(&car).unwrap();
        for (file, original) in files() {
            assert_eq!(
                std::fs::read_to_string(car.join("data").join(file)).unwrap(),
                original
            );
        }
        assert!(!car.join("data").join(NOTE).exists());
    }

    /// **A folder beside an archive is tuned in place and never written
    /// over**: the archive goes aside so the game reads the folder, and
    /// both come back as they were — even an archive that cannot be read.
    #[test]
    fn a_folder_beside_an_archive_is_tuned_in_place() {
        let car = loose_car("mod_both");
        crate::atomic_file::write_atomic(&car.join("data.acd"), b"not an archive").unwrap();
        let p = Physics::read(&car).unwrap();
        let mut tune = Tune::of(&p);
        tune.mass_kg = 1000.0;
        let note = apply(&car, &tune).unwrap();
        assert!(!note.unpacked && note.archive_aside);
        assert!(
            !car.join("data.acd").exists(),
            "the game reads the folder now"
        );
        assert_eq!(Physics::read(&car).unwrap().mass_kg, 1000.0);
        assert_eq!(
            Physics::original(&car).unwrap(),
            p,
            "the folder's own originals"
        );
        restore(&car).unwrap();
        assert_eq!(
            std::fs::read(car.join("data.acd")).unwrap(),
            b"not an archive"
        );
        assert_eq!(
            std::fs::read_to_string(car.join("data/car.ini")).unwrap(),
            CAR
        );
    }

    /// Nonsense is held to what a car can take.
    #[test]
    fn a_wild_tune_is_held_in_range() {
        let p = Physics::read(&loose_car("wild")).unwrap();
        let wild = Tune {
            mass_kg: 1.0,
            power: f32::NAN,
            limiter_rpm: 99_999.0,
            gears: vec![50.0, -1.0],
            final_drive: 0.0,
            brakes: 9.0,
            brake_front: 1.0,
        }
        .settled(&p);
        assert_eq!(wild.mass_kg, 600.0);
        assert_eq!(wild.power, 1.0);
        assert_eq!(wild.limiter_rpm, 7000.0 * 1.3);
        assert_eq!(wild.gears, vec![8.0, 0.3, 1.0], "the third keeps its own");
        assert_eq!(wild.final_drive, 1.5);
        assert_eq!(wild.brakes, 2.0);
        assert_eq!(wild.brake_front, 0.8);
    }

    /// Files that do not describe a car are no car at all.
    #[test]
    fn noise_is_not_a_car() {
        let car = scratch("noise");
        crate::atomic_file::write_atomic(&car.join("data/car.ini"), b"[BASIC]\nTOTALMASS=99999\n")
            .unwrap();
        crate::atomic_file::write_atomic(&car.join("data/engine.ini"), b"").unwrap();
        crate::atomic_file::write_atomic(&car.join("data/power.lut"), b"1|1\n2|2\n").unwrap();
        crate::atomic_file::write_atomic(&car.join("data/drivetrain.ini"), b"[GEARS]\nFINAL=4\n")
            .unwrap();
        assert!(Physics::read(&car).is_none());
    }
}
