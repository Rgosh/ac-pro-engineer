//! The whole of a model: everything [`crate::read`] steps over.
//!
//! [`crate::read`] keeps a car's outline and its wheel markers, and nothing
//! else, on purpose — it is for drawing a car from above. A showroom needs
//! the rest: every mesh with its normals and texture coordinates, placed by
//! the transforms of the nodes above it; every material with its shader, its
//! numbers and which texture fills which slot; and the textures themselves,
//! as the bytes the file carries (DDS, sometimes PNG or JPEG). This reads all
//! of it, from the same byte layout `model.rs` documents, and decodes none of
//! the textures — that is a renderer's business.
//!
//! [`write`] is the inverse, for tests and for a model made by hand: a reader
//! nobody can feed a known file is a reader nobody has checked.

use crate::reader::Result;
use crate::tree::{DUMMY, MESH};

/// A 4×4 transform, row-major, row vectors — translation in the last row,
/// the way the file writes it.
pub type Matrix = [[f32; 4]; 4];

/// The transform that moves nothing.
pub const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// `a` then `b`, for row vectors: a point is moved by `a` first.
pub fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 4];
    for (row, out_row) in out.iter_mut().enumerate() {
        for (column, cell) in out_row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[row][k] * b[k][column]).sum();
        }
    }
    out
}

/// A texture the file carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    pub name: String,
    /// As stored: DDS mostly, PNG or JPEG sometimes.
    pub bytes: Vec<u8>,
}

/// One number of a material — the format writes every property as a scalar
/// and again as vectors of two, three and four.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: String,
    pub value: f32,
    pub vector: [f32; 3],
}

/// How a material draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub name: String,
    /// `ksPerPixel`, `ksPerPixelNM`, `ksPerPixelMultiMap`, `ksWindscreen`…
    pub shader: String,
    /// Blended rather than opaque: glass, decals.
    pub blend: u8,
    /// Cut out at an alpha threshold: grilles, meshes.
    pub alpha_tested: bool,
    pub depth_mode: u32,
    pub properties: Vec<Property>,
    /// Slot name (`txDiffuse`, `txNormal`, `txMaps`…) to texture name.
    pub textures: Vec<(String, String)>,
}

impl Material {
    /// A property's scalar, or `otherwise` when the material has none.
    pub fn value(&self, name: &str, otherwise: f32) -> f32 {
        self.properties
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .map_or(otherwise, |p| p.value)
    }

    /// The texture in a slot, by name.
    pub fn texture(&self, slot: &str) -> Option<&str> {
        self.textures
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case(slot))
            .map(|(_, t)| t.as_str())
    }
}

/// One vertex as a renderer wants it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// One piece of geometry, already placed: its vertices are in the model's
/// own space, every transform above it applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    /// An index into [`Scene::materials`].
    pub material: usize,
    /// Whether the file marks it visible and renderable.
    pub visible: bool,
    pub transparent: bool,
    /// Levels of detail it belongs to: drawn between these distances.
    pub lod_in: f32,
    pub lod_out: f32,
}

/// Everything in one file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub textures: Vec<Texture>,
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh>,
}

impl Scene {
    /// The corners of the box everything visible fits in.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        let mut any = false;
        for mesh in self.meshes.iter().filter(|m| m.visible) {
            for v in &mesh.vertices {
                any = true;
                for k in 0..3 {
                    low[k] = low[k].min(v.position[k]);
                    high[k] = high[k].max(v.position[k]);
                }
            }
        }
        any.then_some((low, high))
    }

    /// A texture the file carries, by name, any case.
    pub fn texture(&self, name: &str) -> Option<&Texture> {
        self.textures
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }
}

/// Read a whole model: its tree read whole, then drawn — see `tree`.
pub fn read(bytes: &[u8]) -> Result<Scene> {
    crate::tree::read(bytes)?.scene()
}

/// One node of a model to be written.
#[derive(Debug, Clone)]
pub enum Node {
    /// A transform with children.
    Dummy {
        name: String,
        transform: Matrix,
        children: Vec<Node>,
    },
    /// Geometry, in its parent's space.
    Mesh(Mesh),
}

/// Write a model in the format [`read`] and the game read: version 6, the
/// textures and materials given, and the tree under one root.
pub fn write(textures: &[Texture], materials: &[Material], root: &Node) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"sc6969");
    put_u32(&mut out, 6);
    put_u32(&mut out, 0);
    put_u32(&mut out, textures.len() as u32);
    for texture in textures {
        put_u32(&mut out, 1);
        put_text(&mut out, &texture.name);
        put_u32(&mut out, texture.bytes.len() as u32);
        out.extend_from_slice(&texture.bytes);
    }
    put_u32(&mut out, materials.len() as u32);
    for material in materials {
        put_text(&mut out, &material.name);
        put_text(&mut out, &material.shader);
        out.push(material.blend);
        out.push(u8::from(material.alpha_tested));
        put_u32(&mut out, material.depth_mode);
        put_u32(&mut out, material.properties.len() as u32);
        for p in &material.properties {
            put_text(&mut out, &p.name);
            put_f32(&mut out, p.value);
            for v in [p.value, p.value] {
                put_f32(&mut out, v);
            }
            for v in p.vector {
                put_f32(&mut out, v);
            }
            for v in [p.vector[0], p.vector[1], p.vector[2], p.value] {
                put_f32(&mut out, v);
            }
        }
        put_u32(&mut out, material.textures.len() as u32);
        for (k, (slot, name)) in material.textures.iter().enumerate() {
            put_text(&mut out, slot);
            put_u32(&mut out, k as u32);
            put_text(&mut out, name);
        }
    }
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node {
            Node::Dummy {
                name,
                transform,
                children,
            } => {
                put_u32(&mut out, DUMMY);
                put_text(&mut out, name);
                put_u32(&mut out, children.len() as u32);
                out.push(1);
                for row in transform {
                    for cell in row {
                        put_f32(&mut out, *cell);
                    }
                }
                // Depth first, in order: the last child is pushed first.
                stack.extend(children.iter().rev());
            }
            Node::Mesh(mesh) => {
                put_u32(&mut out, MESH);
                put_text(&mut out, &mesh.name);
                put_u32(&mut out, 0);
                out.push(1);
                out.extend_from_slice(&[1, u8::from(mesh.visible), u8::from(mesh.transparent)]);
                put_u32(&mut out, mesh.vertices.len() as u32);
                for v in &mesh.vertices {
                    for x in v.position.iter().chain(&v.normal).chain(&v.uv) {
                        put_f32(&mut out, *x);
                    }
                    for _ in 0..3 {
                        put_f32(&mut out, 0.0);
                    }
                }
                put_u32(&mut out, mesh.indices.len() as u32);
                for i in &mesh.indices {
                    out.extend_from_slice(&i.to_le_bytes());
                }
                put_u32(&mut out, mesh.material as u32);
                put_u32(&mut out, 0);
                put_f32(&mut out, mesh.lod_in);
                put_f32(&mut out, mesh.lod_out);
                for _ in 0..4 {
                    put_f32(&mut out, 0.0);
                }
                out.push(1);
            }
        }
    }
    out
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_text(out: &mut Vec<u8>, text: &str) {
    put_u32(out, text.len() as u32);
    out.extend_from_slice(text.as_bytes());
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tree::SKINNED_MESH;

    fn triangle(material: usize) -> Mesh {
        Mesh {
            name: "body".to_string(),
            vertices: vec![
                Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                },
                Vertex {
                    position: [1.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [1.0, 0.0],
                },
                Vertex {
                    position: [0.0, 0.0, 1.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
            material,
            visible: true,
            transparent: false,
            lod_in: 0.0,
            lod_out: 0.0,
        }
    }

    /// **A skinned part does not throw out everything after it.** It ends
    /// in eight bytes where a plain mesh has its level of detail, sphere and
    /// renderable flag; read as a plain one, the next node was read out of
    /// step and the car failed — "the file wants 1065353213 bytes", which is
    /// a float 1.0 taken for a length.
    #[test]
    fn a_skinned_part_is_read_and_the_part_after_it_too() {
        let mesh_bytes = write(&[], &[], &Node::Mesh(triangle(0)));
        let header = 6 + 4 + 4 + 4 + 4;
        let plain = &mesh_bytes[header..];
        let mut skinned = Vec::new();
        put_u32(&mut skinned, SKINNED_MESH);
        put_text(&mut skinned, "belt");
        put_u32(&mut skinned, 0); // children
        skinned.push(1);
        skinned.extend_from_slice(&[1, 1, 0]);
        put_u32(&mut skinned, 1); // one bone
        put_text(&mut skinned, "bone");
        for _ in 0..16 {
            put_f32(&mut skinned, 0.0);
        }
        put_u32(&mut skinned, 3);
        for k in 0..3 {
            for x in [k as f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0] {
                put_f32(&mut skinned, x);
            }
            for _ in 0..3 + 4 + 4 {
                put_f32(&mut skinned, 0.0);
            }
        }
        put_u32(&mut skinned, 3);
        for i in [0u16, 1, 2] {
            skinned.extend_from_slice(&i.to_le_bytes());
        }
        put_u32(&mut skinned, 0); // material
        put_u32(&mut skinned, 0); // layer
        skinned.extend_from_slice(&[0; 8]);

        let root = Node::Dummy {
            name: "root".into(),
            transform: IDENTITY,
            children: vec![Node::Mesh(triangle(0))],
        };
        let mut file = write(&[], &[], &root);
        // Three children: the plain mesh written, the skinned one, another.
        let children_at = header + 4 + 4 + "root".len();
        file[children_at..children_at + 4].copy_from_slice(&3u32.to_le_bytes());
        file.extend_from_slice(&skinned);
        file.extend_from_slice(plain);

        let scene = read(&file).expect("reads past the skinned part");
        assert_eq!(scene.meshes.len(), 3);
        assert_eq!(scene.meshes[1].name, "belt");
        assert_eq!(scene.meshes[1].indices, vec![0, 1, 2]);
        assert_eq!(scene.meshes[2].name, "body", "the part after it is whole");
        assert_eq!(scene.meshes[2].vertices[1].position, [1.0, 0.0, 0.0]);
    }

    /// **What is written is what is read**, transforms composed down the
    /// tree: a mesh under two dummies lands where both put it.
    #[test]
    fn a_written_model_reads_back_placed() {
        let lift = |y: f32| {
            let mut m = IDENTITY;
            m[3][1] = y;
            m
        };
        let mut turn = IDENTITY; // a quarter turn about y: x -> -z
        turn[0] = [0.0, 0.0, -1.0, 0.0];
        turn[2] = [1.0, 0.0, 0.0, 0.0];
        let root = Node::Dummy {
            name: "root".into(),
            transform: lift(1.0),
            children: vec![
                Node::Dummy {
                    name: "turned".into(),
                    transform: turn,
                    children: vec![Node::Mesh(triangle(0))],
                },
                Node::Mesh(triangle(0)),
            ],
        };
        let material = Material {
            name: "paint".into(),
            shader: "ksPerPixel".into(),
            blend: 0,
            alpha_tested: false,
            depth_mode: 0,
            properties: vec![Property {
                name: "ksSpecularEXP".into(),
                value: 50.0,
                vector: [50.0; 3],
            }],
            textures: vec![("txDiffuse".into(), "paint.dds".into())],
        };
        let texture = Texture {
            name: "paint.dds".into(),
            bytes: b"DDS ...".to_vec(),
        };
        let bytes = write(
            std::slice::from_ref(&texture),
            std::slice::from_ref(&material),
            &root,
        );
        let scene = read(&bytes).unwrap();

        assert_eq!(scene.textures, vec![texture]);
        assert_eq!(scene.materials[0].value("ksspecularexp", 0.0), 50.0);
        assert_eq!(scene.materials[0].texture("txDiffuse"), Some("paint.dds"));
        assert_eq!(scene.meshes.len(), 2);
        // Under "turned" then "root": (1,0,0) turned to (0,0,-1), lifted by 1.
        let turned = &scene.meshes[0].vertices[1].position;
        assert!(
            (turned[0]).abs() < 1e-6
                && (turned[1] - 1.0).abs() < 1e-6
                && (turned[2] + 1.0).abs() < 1e-6,
            "{turned:?}"
        );
        // Directly under root: only lifted.
        assert_eq!(scene.meshes[1].vertices[1].position, [1.0, 1.0, 0.0]);
        assert_eq!(scene.meshes[1].indices, vec![0, 1, 2]);
        let (low, high) = scene.bounds().unwrap();
        assert_eq!(low[1], 1.0);
        assert_eq!(high[0], 1.0);
        // The outline reader agrees about where things are not, at least:
        // it reads the same file without error.
        assert!(crate::read(&bytes).is_ok());
    }

    /// A file cut short is an error, never a panic.
    #[test]
    fn a_truncated_model_is_an_error() {
        let bytes = write(&[], &[], &Node::Mesh(triangle(0)));
        for cut in [3, 10, bytes.len() / 2, bytes.len() - 1] {
            assert!(read(&bytes[..cut]).is_err(), "cut at {cut}");
        }
    }
}
