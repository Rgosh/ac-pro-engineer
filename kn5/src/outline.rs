//! A car seen from above, as a closed outline and a handful of measurements.
//!
//! **The outline is not a convex hull.** A car is waisted at the cabin and
//! widest at the arches, and a hull squares that off into a slab — the same
//! mistake the graphical front end already made once drawing its generic car,
//! and it is written down in its `CLAUDE.md` because it took a while to see.
//! What is used instead is the farthest point in each of a few dozen angular
//! slices around the middle, which keeps a waist and cannot self-intersect.
//!
//! Everything here is in the car's own metres, so a drawing can be to scale
//! and two cars can be compared without either being resized.

use crate::model::{Model, Point};

/// How many slices the outline is taken in.
///
/// Ninety-six is a point every four degrees: fine enough that an arch is a
/// curve rather than a facet, coarse enough that the whole shape is a few
/// hundred bytes and one polyline to draw.
const SLICES: usize = 96;

/// **Only the body.** A car's model carries its mirrors, its wing and its
/// aerial, and a silhouette that includes an aerial is a silhouette with a
/// spike on it. Anything above this, in metres, is left out.
const ROOF_M: f32 = 1.6;

/// A car, as much of it as is worth drawing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shape {
    /// The outline from above, closed, in metres. `x` is across the car and
    /// `y` is along it, nose towards positive.
    pub outline: Vec<[f32; 2]>,
    pub length_m: f32,
    pub width_m: f32,
    pub height_m: f32,
    /// Where each wheel sits, in wheel order — front left, front right, rear
    /// left, rear right — when the model names them. `None` on a model that
    /// does not, and then a drawing has to say so rather than place four
    /// wheels where they look right.
    pub wheels: Option<[[f32; 2]; 4]>,
}

impl Shape {
    pub fn wheelbase_m(&self) -> Option<f32> {
        let wheels = self.wheels?;
        Some((wheels[0][1] - wheels[2][1]).abs())
    }

    pub fn track_front_m(&self) -> Option<f32> {
        let wheels = self.wheels?;
        Some((wheels[0][0] - wheels[1][0]).abs())
    }

    pub fn track_rear_m(&self) -> Option<f32> {
        let wheels = self.wheels?;
        Some((wheels[2][0] - wheels[3][0]).abs())
    }
}

/// The names Assetto Corsa gives the four wheels, in wheel order.
///
/// The game's own arrays are front-left, front-right, rear-left, rear-right,
/// and every screen in this family of programs reads them in that order, so
/// the shape hands them over in it too rather than in the order the file
/// happens to list them.
const WHEEL_NODES: [&str; 4] = ["WHEEL_LF", "WHEEL_RF", "WHEEL_LR", "WHEEL_RR"];

/// What the suspension nodes are called, in the same order.
///
/// A second naming to look under, because a model may carry one and not the
/// other — and on every car checked that has both, the two sit at the same
/// place, which is what makes the fallback honest rather than approximate.
const SUSPENSION_NODES: [&str; 4] = ["SUSP_LF", "SUSP_RF", "SUSP_LR", "SUSP_RR"];

/// Work a car's shape out of a model.
pub fn shape(model: &Model) -> Shape {
    let all: Vec<&Point> = model.points().collect();
    if all.len() < 8 {
        return Shape::default();
    }
    // **Measured from the car's own bottom, not from zero.** Most models sit
    // on the ground with the origin at the axle line and a few do not — one
    // rally car's lowest level of detail sits high enough that filtering on
    // absolute height threw away everything but the floor and reported a
    // 1.34 m car.
    let floor = all.iter().map(|point| point.y).fold(f32::MAX, f32::min);
    let body: Vec<&Point> = all
        .into_iter()
        .filter(|point| point.y - floor <= ROOF_M)
        .collect();
    if body.len() < 8 {
        return Shape::default();
    }

    let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
    let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
    let (mut min_z, mut max_z) = (f32::MAX, f32::MIN);
    for point in &body {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
        min_z = min_z.min(point.z);
        max_z = max_z.max(point.z);
    }
    let centre = ((min_x + max_x) * 0.5, (min_z + max_z) * 0.5);

    // The farthest point in each slice. A slice with nothing in it is skipped
    // rather than filled: a gap in the outline is better than a point invented
    // to close it.
    let mut farthest: Vec<Option<(f32, [f32; 2])>> = vec![None; SLICES];
    for point in &body {
        let (dx, dz) = (point.x - centre.0, point.z - centre.1);
        let reach = dx.hypot(dz);
        if reach <= f32::EPSILON {
            continue;
        }
        let angle = dz.atan2(dx);
        let slice = (((angle + std::f32::consts::PI) / std::f32::consts::TAU) * SLICES as f32)
            as usize
            % SLICES;
        if farthest[slice].is_none_or(|(best, _)| reach > best) {
            farthest[slice] = Some((reach, [point.x, point.z]));
        }
    }

    Shape {
        outline: farthest.into_iter().flatten().map(|(_, at)| at).collect(),
        length_m: max_z - min_z,
        width_m: max_x - min_x,
        height_m: max_y - min_y,
        wheels: wheels(model),
    }
}

fn wheels(model: &Model) -> Option<[[f32; 2]; 4]> {
    named(model, &WHEEL_NODES)
        .filter(plausible)
        .or_else(|| named(model, &SUSPENSION_NODES).filter(plausible))
}

/// Whether four positions could be four wheels of one car.
///
/// **A transform in this format is local to its parent and this crate does not
/// compose them.** On nearly every car the wheel nodes sit directly under the
/// root, whose transform is the identity, so the local position is the real
/// one — but not on all of them: the Ferrari 312T nests its, and reading them
/// straight puts all four wheels in the middle of the car. Four wheels in the
/// wrong place is worse than none, so a set that is not the shape of a car is
/// refused and the drawing says the model did not name them.
fn plausible(wheels: &[[f32; 2]; 4]) -> bool {
    let wheelbase = (wheels[0][1] - wheels[2][1]).abs();
    let track = (wheels[0][0] - wheels[1][0]).abs();
    // Nothing on four wheels has a wheelbase under a metre and a half or a
    // track under three quarters of one.
    (1.5..4.5).contains(&wheelbase) && (0.75..2.5).contains(&track)
}

/// All four of a naming, or none of it. Three wheels is not a car.
fn named(model: &Model, nodes: &[&str; 4]) -> Option<[[f32; 2]; 4]> {
    let mut found = [[0.0_f32; 2]; 4];
    for (slot, name) in nodes.iter().enumerate() {
        let at = model.marker(name)?;
        found[slot] = [at.x, at.z];
    }
    Some(found)
}
