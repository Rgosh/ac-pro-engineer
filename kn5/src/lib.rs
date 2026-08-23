//! Reading the shape of a car out of Assetto Corsa's own model files.
//!
//! **Why this exists.** A telemetry screen that draws a car draws a generic
//! one, and every car it is drawn for is a different shape — a Miata and a GT3
//! share nothing but four wheels. The exact silhouette is on the disk of
//! everyone who owns the car, in the model the game itself renders, including
//! for a mod installed yesterday. Nothing has to be downloaded, bundled or
//! guessed, and none of Kunos's artwork is redistributed: it is read from the
//! copy the driver already has.
//!
//! **What it deliberately is not.** This is not a renderer and it does not
//! read textures, materials, normals or animations. It steps over all of them.
//! What comes out is a closed outline of a few hundred points, the car's
//! measurements in metres, and where its four wheels sit — a couple of
//! kilobytes that draw as one polyline, at any zoom, rotating with the car for
//! free. A 3D model rendered every frame would cost a hundred times that to
//! show something less readable.
//!
//! ```no_run
//! let bytes = std::fs::read("collider.kn5")?;
//! let model = kn5::read(&bytes)?;
//! let shape = kn5::shape(&model);
//! println!("{:.2} m long, {:.2} m wide", shape.length_m, shape.width_m);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Which file to read
//!
//! A car folder holds several. [`best_model`] picks, and the order is about
//! cost: `collider.kn5` is a few kilobytes and is the body the game itself
//! collides with, which is exactly the shape wanted; the lowest visual level
//! of detail is next; the full model is tens of megabytes and is the last
//! resort.
//!
//! ## What is confirmed and what is not
//!
//! Every offset here was read off real files from a real installation, the
//! same way this project pins a game's memory layout to a recording. Versions
//! 5 and 6 are what ships and are what this accepts; anything else is refused
//! rather than read as if it were one of them.
//!
//! Transforms are **local to a parent** and this crate does not compose them.
//! It does not need to: the wheels sit directly under the root, whose
//! transform is the identity in every car checked. A nested marker's position
//! is therefore relative and is not used.

mod model;
mod outline;
mod reader;

pub use model::{Marker, Mesh, Model, Point, read};
pub use outline::{Shape, shape};
pub use reader::{Error, Result};

use std::path::{Path, PathBuf};

/// The cheapest file in a car's folder that holds its shape.
///
/// Returns nothing when the folder holds none, which is a normal state: a car
/// may ship without a collider and without a level of detail, and a screen
/// that cannot draw its outline should say so rather than draw somebody
/// else's.
pub fn best_model(car_folder: &Path) -> Option<PathBuf> {
    models_by_size(car_folder)?.into_iter().next()
}

/// Every model in a car's folder, cheapest first.
///
/// **The collider sorts last however small it is.** It is a tenth the size of
/// anything else and it is the body the game itself collides with — but it is
/// *only* the body: no wheel nodes, so a shape read from it has nothing to
/// place them by.
fn models_by_size(car_folder: &Path) -> Option<Vec<PathBuf>> {
    let mut models: Vec<PathBuf> = std::fs::read_dir(car_folder)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "kn5") && path.is_file())
        .collect();
    // Smallest first, which is the lowest level of detail, which is the
    // cheapest file that still has the whole car in it.
    let is_collider = |path: &PathBuf| {
        path.file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("collider.kn5"))
    };
    models.sort_by_key(|path| {
        (
            is_collider(path),
            std::fs::metadata(path)
                .map(|data| data.len())
                .unwrap_or(u64::MAX),
        )
    });
    Some(models)
}

/// The largest model this will open looking for wheels.
///
/// A full car model is tens of megabytes and the lowest level of detail is
/// tens of kilobytes. Reading a bigger one is worth it when the smaller has no
/// wheel nodes — several cars' lowest level of detail is the body alone — but
/// not without a ceiling, or one badly built mod costs a driver a second of
/// disk on the screen that draws their car.
const LARGEST_TRIED: u64 = 12 << 20;

/// Read a car's shape out of its folder.
///
/// Tries the models smallest first and stops at the first that has wheels in
/// it, because that is the one thing a cheaper file may be missing. Where
/// none has them the outline is still returned with no wheels, and a drawing
/// says so rather than placing four where they look right.
pub fn car_shape(car_folder: &Path) -> Option<Shape> {
    let mut best: Option<Shape> = None;
    for path in models_by_size(car_folder)? {
        if std::fs::metadata(&path).map(|data| data.len()).unwrap_or(0) > LARGEST_TRIED
            && best.is_some()
        {
            break;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(model) = read(&bytes) else {
            continue;
        };
        let found = shape(&model);
        // **A file that is not a car is skipped, not returned.** A folder may
        // hold a model of something else, and one rally car's lowest level of
        // detail is a fragment — a shape under two metres is not a car however
        // cheerfully it parsed.
        if found.outline.len() < 16
            || !(2.0..7.0).contains(&found.length_m)
            || !(1.2..2.6).contains(&found.width_m)
        {
            continue;
        }
        if found.wheels.is_some() {
            return Some(found);
        }
        best.get_or_insert(found);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_that_is_not_a_model_is_refused() {
        assert_eq!(read(b"not a car at all").unwrap_err(), Error::NotAModel);
    }

    #[test]
    fn a_version_nobody_has_checked_is_refused() {
        let mut bytes = b"sc6969".to_vec();
        bytes.extend_from_slice(&99u32.to_le_bytes());
        assert_eq!(read(&bytes).unwrap_err(), Error::Version(99));
    }

    /// **A truncated file must not panic.** A model is somebody else's data —
    /// a mod, a half-finished download — and a parser that indexes into it and
    /// trusts the result takes a driver's telemetry program down with it.
    #[test]
    fn a_file_that_stops_half_way_is_an_error_and_not_a_crash() {
        let mut bytes = b"sc6969".to_vec();
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // no textures
        bytes.extend_from_slice(&1u32.to_le_bytes()); // one material
        bytes.extend_from_slice(&40u32.to_le_bytes()); // whose name is 40 bytes
        // and then nothing
        assert!(matches!(read(&bytes), Err(Error::Truncated { .. })));
    }

    /// **Against a real installation, when there is one.** Every offset in
    /// this crate was read off real files, and a parser checked only against
    /// bytes the test wrote itself is a parser checked against its author's
    /// memory. Skipped where the game is absent, so this still passes on a
    /// build machine.
    #[test]
    fn every_installed_car_gives_a_shape_that_is_the_size_of_a_car() {
        let Some(cars) = installed_cars() else {
            return;
        };
        let mut read = 0;
        for folder in std::fs::read_dir(&cars).into_iter().flatten().flatten() {
            let folder = folder.path();
            if !folder.is_dir() {
                continue;
            }
            let Some(shape) = car_shape(&folder) else {
                continue;
            };
            read += 1;
            let name = folder.file_name().unwrap_or_default().to_string_lossy();
            // Nothing on four wheels is under two metres or over seven.
            assert!(
                (2.0..7.0).contains(&shape.length_m),
                "{name} came out {:.2} m long",
                shape.length_m
            );
            assert!(
                (1.2..2.6).contains(&shape.width_m),
                "{name} came out {:.2} m wide",
                shape.width_m
            );
            assert!(
                shape.outline.len() > 24,
                "{name} produced an outline of {} points",
                shape.outline.len()
            );
            if let Some(base) = shape.wheelbase_m() {
                assert!(
                    (1.6..4.0).contains(&base),
                    "{name} came out with a {base:.2} m wheelbase"
                );
            }
        }
        assert!(read > 0, "the game is installed and not one car was read");
    }

    fn installed_cars() -> Option<std::path::PathBuf> {
        let home = std::env::var_os("HOME")?;
        [
            ".local/share/Steam/steamapps/common/assettocorsa",
            ".steam/steam/steamapps/common/assettocorsa",
        ]
        .into_iter()
        .map(|under| std::path::Path::new(&home).join(under).join("content/cars"))
        .find(|path| path.is_dir())
    }
}
