//! What the game would trip on, found before the file is written.
//!
//! **A model the game cannot load closes the game** — not an error message,
//! not a car missing from a list: Assetto Corsa goes, mid-loading, taking
//! the session with it. Every rule here is one of those: an index past the
//! end of its vertices, a triangle with two corners, a number that is not a
//! number, a material or a texture named and not there, a picture that is
//! not one. The studio asks this before every save and writes nothing that
//! fails it.
//!
//! **Only what was changed.** A node or material nothing touched is the
//! game's own bytes, read and written back as they came; it loaded before
//! and it loads now. The checks run on everything all the same — a car whose
//! own file already breaks a rule is worth knowing about — but the studio
//! only refuses to write what it made.

use crate::tree::{DUMMY, Model};

/// Something the game would trip on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The node, material or texture it is about.
    pub at: String,
    pub what: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.at, self.what)
    }
}

/// The most corners a mesh can have: its triangles name them in sixteen bits.
pub const MOST_CORNERS: usize = 65_535;

/// Everything wrong with a model, in the order it is found.
pub fn check(model: &Model) -> Vec<Problem> {
    let mut found = Vec::new();
    let mut say = |at: &str, what: String| {
        found.push(Problem {
            at: at.to_string(),
            what,
        })
    };

    let texture_named = |name: &str| model.textures.iter().any(|t| t.texture.name == name);
    for entry in &model.textures {
        let texture = &entry.texture;
        if texture.bytes.is_empty() {
            say(&texture.name, "an empty picture".to_string());
            continue;
        }
        if texture.name.to_ascii_lowercase().ends_with(".dds")
            && let Some(why) = dds_problem(&texture.bytes)
        {
            say(&texture.name, why);
        }
    }

    for (k, entry) in model.materials.iter().enumerate() {
        let material = &entry.material;
        let at = if material.name.is_empty() {
            format!("material {k}")
        } else {
            material.name.clone()
        };
        if material.shader.trim().is_empty() {
            say(&at, "no shader".to_string());
        }
        for (slot, name) in &material.textures {
            if !texture_named(name) {
                say(
                    &at,
                    format!("its {slot} names {name}, which the model does not carry"),
                );
            }
        }
        for property in &material.properties {
            if !property.value.is_finite() || property.vector.iter().any(|v| !v.is_finite()) {
                say(&at, format!("{} is not a number", property.name));
            }
        }
    }

    for node in &model.nodes {
        if node.kind == DUMMY {
            continue;
        }
        let Some(facts) = node.mesh else { continue };
        if facts.material >= model.materials.len() {
            say(
                &node.name,
                format!(
                    "uses material {} of {}",
                    facts.material,
                    model.materials.len()
                ),
            );
        }
        let mesh = match node.geometry() {
            Ok(Some(mesh)) => mesh,
            Ok(None) => continue,
            Err(why) => {
                say(&node.name, format!("its geometry does not read: {why}"));
                continue;
            }
        };
        // The corners as the file has them: `geometry` leaves out a triangle
        // it cannot draw, and the game does not.
        let (corners, indices) = match node.raw_corners() {
            Ok(Some(raw)) => raw,
            _ => (mesh.vertices.len(), mesh.indices.clone()),
        };
        if corners == 0 || indices.is_empty() {
            say(&node.name, "a mesh with nothing in it".to_string());
            continue;
        }
        if corners > MOST_CORNERS {
            say(
                &node.name,
                format!("{corners} corners, more than {MOST_CORNERS}"),
            );
        }
        if indices.len() % 3 != 0 {
            say(
                &node.name,
                format!("{} corners named, not whole triangles", indices.len()),
            );
        }
        if let Some(beyond) = indices.iter().find(|i| usize::from(**i) >= corners) {
            say(
                &node.name,
                format!("a triangle names corner {beyond} of {corners}"),
            );
        }
        let broken = mesh.vertices.iter().any(|v| {
            v.position
                .iter()
                .chain(&v.normal)
                .chain(&v.uv)
                .any(|x| !x.is_finite())
        });
        if broken {
            say(&node.name, "a corner that is not a number".to_string());
        }
    }
    found
}

/// What is wrong with a DDS file's head, if anything.
///
/// The game hands the file to the graphics card as it is: a header that lies
/// about its size or its format is read past the end, and that is a crash in
/// the driver, not a texture that looks wrong.
pub(crate) fn dds_problem(bytes: &[u8]) -> Option<String> {
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(u32::from_le_bytes)
    };
    if bytes.get(..4) != Some(b"DDS ".as_slice()) || word(4) != Some(124) {
        return Some("a .dds that is not DDS".to_string());
    }
    let (Some(height), Some(width)) = (word(12), word(16)) else {
        return Some("a DDS too short for its header".to_string());
    };
    if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
        return Some(format!("a DDS of {width} × {height}"));
    }
    let levels = word(28).unwrap_or(0).max(1);
    let flags = word(80).unwrap_or(0);
    let four = bytes.get(84..88).unwrap_or(&[]);
    let bits = word(88).unwrap_or(0);
    // Bytes in one level, for the formats the studio writes and the ones a
    // car most often carries.
    let size_of = |w: u32, h: u32| -> Option<u64> {
        let (w, h) = (u64::from(w.max(1)), u64::from(h.max(1)));
        let blocks = |w: u64, h: u64| w.div_ceil(4) * h.div_ceil(4);
        match four {
            b"DXT1" => Some(blocks(w, h) * 8),
            b"DXT3" | b"DXT5" | b"ATI2" | b"BC5U" => Some(blocks(w, h) * 16),
            _ if flags & 0x40 != 0 && bits % 8 == 0 && bits > 0 => {
                Some(w * h * u64::from(bits / 8))
            }
            _ => None,
        }
    };
    let mut needed = 128u64;
    if four == b"DX10" {
        return None;
    }
    for level in 0..levels.min(16) {
        let (w, h) = ((width >> level).max(1), (height >> level).max(1));
        needed += size_of(w, h)?;
    }
    (needed > bytes.len() as u64).then(|| {
        format!(
            "a DDS of {width} × {height} in {levels} levels needs {needed} bytes and has {}",
            bytes.len()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{IDENTITY, Material, Mesh, Texture, Vertex};
    use crate::tree::{Model, Node};

    fn quad() -> Mesh {
        let v = |x: f32, z: f32| Vertex {
            position: [x, 0.0, z],
            normal: [0.0, 1.0, 0.0],
            uv: [x, z],
        };
        Mesh {
            name: "quad".to_string(),
            vertices: vec![v(0.0, 0.0), v(1.0, 0.0), v(1.0, 1.0), v(0.0, 1.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            visible: true,
            transparent: false,
            lod_in: 0.0,
            lod_out: 0.0,
        }
    }

    fn dds(width: u32, height: u32) -> Vec<u8> {
        let mut out = b"DDS ".to_vec();
        let mut word = |v: u32| out.extend_from_slice(&v.to_le_bytes());
        word(124);
        word(0x1 | 0x2 | 0x4 | 0x1000);
        word(height);
        word(width);
        word(0);
        word(0);
        word(1);
        for _ in 0..11 {
            word(0);
        }
        word(32);
        word(0x41);
        word(0);
        word(32);
        for m in [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000] {
            word(m);
        }
        for _ in 0..5 {
            word(0);
        }
        out.extend(std::iter::repeat_n(255u8, (width * height * 4) as usize));
        out
    }

    fn model_with(mesh: &Mesh, texture: Vec<u8>) -> Model {
        let mut model = Model::new("root");
        model.put_texture(Texture {
            name: "t.dds".to_string(),
            bytes: texture,
        });
        model.add_material(Material {
            name: "m".to_string(),
            shader: "ksPerPixel".to_string(),
            blend: 0,
            alpha_tested: false,
            depth_mode: 0,
            properties: Vec::new(),
            textures: vec![("txDiffuse".to_string(), "t.dds".to_string())],
        });
        model.add(0, Node::mesh(mesh));
        model
    }

    #[test]
    fn a_sound_model_has_nothing_wrong() {
        let model = model_with(&quad(), dds(4, 4));
        assert_eq!(check(&model), Vec::new());
        // And again after a trip through the file.
        let back = crate::tree::read(&model.write()).unwrap();
        assert_eq!(check(&back), Vec::new());
        let _ = IDENTITY;
    }

    #[test]
    fn a_corner_past_the_end_is_found() {
        let mut mesh = quad();
        mesh.indices[5] = 9;
        let problems = check(&model_with(&mesh, dds(4, 4)));
        assert!(
            problems.iter().any(|p| p.what.contains("names corner 9")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_broken_triangle_and_a_nan_are_found() {
        let mut mesh = quad();
        mesh.indices.pop();
        mesh.vertices[1].normal = [f32::NAN, 0.0, 0.0];
        let problems = check(&model_with(&mesh, dds(4, 4)));
        assert!(
            problems
                .iter()
                .any(|p| p.what.contains("not whole triangles"))
        );
        assert!(problems.iter().any(|p| p.what.contains("not a number")));
    }

    #[test]
    fn a_missing_texture_and_a_short_dds_are_found() {
        let mut model = model_with(&quad(), dds(8, 8));
        model.textures[0].texture.bytes.truncate(200);
        model.materials[0].material.textures[0].1 = "gone.dds".to_string();
        let problems = check(&model);
        assert!(
            problems.iter().any(|p| p.what.contains("does not carry")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.what.contains("needs")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_mesh_on_a_material_that_is_not_there_is_found() {
        let mut mesh = quad();
        mesh.material = 3;
        let problems = check(&model_with(&mesh, dds(4, 4)));
        assert!(
            problems.iter().any(|p| p.what.contains("uses material 3")),
            "{problems:?}"
        );
    }
}
