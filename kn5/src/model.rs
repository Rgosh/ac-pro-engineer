//! The tree a model file holds, and the walk that reads it.
//!
//! The format, confirmed byte by byte against files this machine has rather
//! than from memory:
//!
//! ```text
//! "sc6969"                     six bytes
//! version            u32       5 and 6 are what ships
//! [extra             u32]      only when the version is above 5
//! textures           u32       count, then each: kind u32, name, size u32, bytes
//! materials          u32       count, then each: name, shader, two flags,
//!                              depth mode u32, properties, texture slots
//! root node                    and its children, depth first
//! ```
//!
//! A node is a kind, a name, how many children follow it, and whether it is
//! active. A dummy carries a 4×4 transform; a mesh carries its vertices and
//! indices; a skinned mesh carries its bones first and fatter vertices.
//!
//! **Only two things are kept**: the positions of a mesh's vertices, and where
//! each dummy sits. Everything else is stepped over — this crate exists to
//! draw a car's outline from above, and a normal, a UV or a texture is weight
//! it would carry and never use.

use crate::reader::{Error, Reader, Result};

/// The kinds a node comes in.
const DUMMY: u32 = 1;
const MESH: u32 = 2;
const SKINNED_MESH: u32 = 3;

/// The versions this parser has been checked against.
const KNOWN_VERSIONS: [u32; 2] = [5, 6];

/// Bytes per vertex of a plain mesh: position, normal, texture coordinate,
/// tangent.
const VERTEX_BYTES: usize = 4 * 3 + 4 * 3 + 4 * 2 + 4 * 3;
/// A skinned vertex carries its bone weights and indices as well.
const SKINNED_VERTEX_BYTES: usize = VERTEX_BYTES + 4 * 4 + 4 * 4;

/// One point of a car, in the car's own metres.
///
/// **x is across, y is up, z is along** — which is Assetto Corsa's convention
/// and not this crate's choice. A top-down outline is therefore x against z,
/// and y is what says how tall the car is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// A named place in the car, with nothing hanging off it.
///
/// The wheels are these: `WHEEL_LF` and its three companions sit exactly where
/// the car's wheels are, which is how a drawing can put them there rather than
/// somewhere that looks about right.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub name: String,
    pub at: Point,
}

/// One piece of geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub name: String,
    pub vertices: Vec<Point>,
}

/// Everything read out of one file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub markers: Vec<Marker>,
}

impl Model {
    /// Every vertex of every mesh.
    pub fn points(&self) -> impl Iterator<Item = &Point> {
        self.meshes.iter().flat_map(|mesh| mesh.vertices.iter())
    }

    /// The place a named marker sits, if the model has one.
    ///
    /// Case-insensitive: the same node is `WHEEL_LF` in one car and
    /// `wheel_LF` in another, and a car is not missing a wheel because its
    /// author held shift.
    pub fn marker(&self, name: &str) -> Option<Point> {
        self.markers
            .iter()
            .find(|marker| marker.name.eq_ignore_ascii_case(name))
            .map(|marker| marker.at)
    }
}

/// Read a model out of bytes.
pub fn read(bytes: &[u8]) -> Result<Model> {
    let mut reader = Reader::new(bytes);
    if reader.skip(6).is_err() || &bytes[..6] != b"sc6969" {
        return Err(Error::NotAModel);
    }
    let version = reader.u32()?;
    if !KNOWN_VERSIONS.contains(&version) {
        return Err(Error::Version(version));
    }
    // A field that appears above version five and is zero in everything seen.
    if version > 5 {
        reader.u32()?;
    }

    skip_textures(&mut reader)?;
    skip_materials(&mut reader)?;

    let mut model = Model::default();
    node(&mut reader, &mut model)?;
    Ok(model)
}

fn skip_textures(reader: &mut Reader<'_>) -> Result<()> {
    for _ in 0..reader.u32()? {
        reader.u32()?;
        reader.text()?;
        let size = reader.u32()? as usize;
        reader.skip(size)?;
    }
    Ok(())
}

fn skip_materials(reader: &mut Reader<'_>) -> Result<()> {
    for _ in 0..reader.u32()? {
        reader.text()?; // name
        reader.text()?; // shader
        reader.u8()?; // alpha blend mode
        reader.u8()?; // alpha tested
        reader.u32()?; // depth mode
        for _ in 0..reader.u32()? {
            reader.text()?; // property name
            reader.f32()?; // value
            // The same value again as a 2-, 3- and 4-component vector, which
            // is how the format writes it whatever the property actually is.
            reader.skip(4 * (2 + 3 + 4))?;
        }
        for _ in 0..reader.u32()? {
            reader.text()?; // slot name
            reader.u32()?; // slot
            reader.text()?; // texture name
        }
    }
    Ok(())
}

/// One node and everything under it.
///
/// Written as a loop over an explicit stack rather than as recursion: a car is
/// only a few levels deep, but a corrupt file can claim any depth it likes,
/// and a parser that recurses on that is a stack overflow rather than an
/// error.
fn node(reader: &mut Reader<'_>, model: &mut Model) -> Result<()> {
    let mut todo = 1usize;
    while todo > 0 {
        todo -= 1;
        let at = reader.at();
        let kind = reader.u32()?;
        let name = reader.text()?;
        let children = reader.u32()? as usize;
        reader.u8()?; // active

        match kind {
            DUMMY => {
                // A 4×4 transform, of which the last row is where it sits.
                reader.skip(4 * 12)?;
                let x = reader.f32()?;
                let y = reader.f32()?;
                let z = reader.f32()?;
                reader.f32()?;
                model.markers.push(Marker {
                    name,
                    at: Point { x, y, z },
                });
            }
            MESH | SKINNED_MESH => {
                reader.skip(3)?; // casts shadows, visible, transparent
                let stride = if kind == MESH {
                    VERTEX_BYTES
                } else {
                    // Bones come first on a skinned mesh: a name and a
                    // transform each.
                    for _ in 0..reader.u32()? {
                        reader.text()?;
                        reader.skip(4 * 16)?;
                    }
                    SKINNED_VERTEX_BYTES
                };

                let count = reader.u32()? as usize;
                let mut vertices = Vec::with_capacity(count.min(1 << 16));
                for _ in 0..count {
                    let x = reader.f32()?;
                    let y = reader.f32()?;
                    let z = reader.f32()?;
                    reader.skip(stride - 4 * 3)?;
                    vertices.push(Point { x, y, z });
                }

                let indices = reader.u32()? as usize;
                reader.skip(indices * 2)?;
                reader.u32()?; // material
                reader.u32()?; // layer
                reader.f32()?; // lod in
                reader.f32()?; // lod out
                reader.skip(4 * 3 + 4)?; // bounding sphere and its radius
                reader.u8()?; // renderable

                model.meshes.push(Mesh { name, vertices });
            }
            other => return Err(Error::UnknownNode { at, kind: other }),
        }

        todo += children;
    }
    Ok(())
}
