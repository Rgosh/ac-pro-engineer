//! A model as its tree, kept whole — read, changed, and written back.
//!
//! [`crate::scene`] reads a model to draw it: every mesh placed, every
//! transform applied, the tree gone. That is what a showroom wants and what
//! an editor cannot use — a car's wheels, suspension, steering wheel and
//! doors are *nodes the game moves by name*, and a model written back without
//! them is a model that does not drive.
//!
//! **Kept whole.** Every node keeps the bytes it was read from, and every
//! material too; one that nothing changed is written back exactly as it
//! came, byte for byte, whatever this crate understands of it. Only what was
//! changed — a node hidden, moved or added, a material or a texture
//! replaced — is written from what it now holds. A real car read and written
//! with nothing changed is the same file (`a_real_car_comes_back_as_it_was`,
//! against the cars of the developer's own install).
//!
//! **A tree with no recursion.** Nodes live in one list and name their
//! children by place in it, and every walk uses an explicit stack: a file
//! that claims a depth of a million is an error, not a crash.

use crate::reader::{Error, Reader, Result};
use crate::scene::{IDENTITY, Material, Matrix, Mesh, Scene, Texture, Vertex, multiply};

pub const DUMMY: u32 = 1;
pub const MESH: u32 = 2;
pub const SKINNED_MESH: u32 = 3;
const KNOWN_VERSIONS: [u32; 2] = [5, 6];

/// A texture as the file carries it, with the word written before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureEntry {
    pub kind: u32,
    pub texture: Texture,
}

/// A material, and the bytes it was read from while nothing has changed it.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialEntry {
    pub material: Material,
    raw: Option<Vec<u8>>,
}

/// What a mesh node holds, read once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshFacts {
    pub vertices: usize,
    pub triangles: usize,
    /// An index into [`Model::materials`].
    pub material: usize,
    pub visible: bool,
    pub skinned: bool,
    /// Where the material's number sits in the node's bytes.
    material_at: usize,
}

/// One node: a transform with children, or geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: u32,
    /// As text; [`Node::name_bytes`] are what is written.
    pub name: String,
    name_bytes: Vec<u8>,
    pub active: u8,
    /// Everything after the node's header and before its children.
    body: Vec<u8>,
    pub children: Vec<usize>,
    pub mesh: Option<MeshFacts>,
}

impl Node {
    /// A transform, empty.
    pub fn dummy(name: &str, transform: &Matrix) -> Node {
        let mut body = Vec::with_capacity(64);
        for row in transform {
            for cell in row {
                body.extend_from_slice(&cell.to_le_bytes());
            }
        }
        Node {
            kind: DUMMY,
            name: name.to_string(),
            name_bytes: name.as_bytes().to_vec(),
            active: 1,
            body,
            children: Vec::new(),
            mesh: None,
        }
    }

    /// Geometry, in its parent's space.
    pub fn mesh(mesh: &Mesh) -> Node {
        let mut body = Vec::new();
        body.extend_from_slice(&[1, u8::from(mesh.visible), u8::from(mesh.transparent)]);
        put_u32(&mut body, mesh.vertices.len() as u32);
        for v in &mesh.vertices {
            for x in v.position.iter().chain(&v.normal).chain(&v.uv) {
                put_f32(&mut body, *x);
            }
            // The tangent: along the texture's u, as near as one can say
            // without the neighbours — the game's shaders take any.
            for x in tangent(v.normal) {
                put_f32(&mut body, x);
            }
        }
        put_u32(&mut body, mesh.indices.len() as u32);
        for i in &mesh.indices {
            body.extend_from_slice(&i.to_le_bytes());
        }
        let material_at = body.len();
        put_u32(&mut body, mesh.material as u32);
        put_u32(&mut body, 0);
        put_f32(&mut body, mesh.lod_in);
        put_f32(&mut body, mesh.lod_out);
        let (centre, radius) = sphere(&mesh.vertices);
        for x in centre {
            put_f32(&mut body, x);
        }
        put_f32(&mut body, radius);
        body.push(1);
        Node {
            kind: MESH,
            name: mesh.name.clone(),
            name_bytes: mesh.name.as_bytes().to_vec(),
            active: 1,
            body,
            children: Vec::new(),
            mesh: Some(MeshFacts {
                vertices: mesh.vertices.len(),
                triangles: mesh.indices.len() / 3,
                material: mesh.material,
                visible: mesh.visible,
                skinned: false,
                material_at,
            }),
        }
    }

    /// The node's own transform, for a dummy.
    pub fn transform(&self) -> Option<Matrix> {
        if self.kind != DUMMY {
            return None;
        }
        let mut m = IDENTITY;
        let mut words = self.body.chunks_exact(4);
        for row in &mut m {
            for cell in row.iter_mut() {
                let word = words.next()?;
                *cell = f32::from_le_bytes(word.try_into().ok()?);
            }
        }
        Some(m)
    }

    /// The geometry, in the node's parent's space.
    pub fn geometry(&self) -> Result<Option<Mesh>> {
        if self.kind == DUMMY {
            return Ok(None);
        }
        let mut reader = Reader::new(&self.body);
        parse_mesh(&mut reader, self.kind, &self.name, &IDENTITY).map(Some)
    }
}

/// A whole model, as its tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub version: u32,
    /// The word after the version, in version 6.
    pub extra: u32,
    pub textures: Vec<TextureEntry>,
    pub materials: Vec<MaterialEntry>,
    /// Every node; the first is the root.
    pub nodes: Vec<Node>,
}

/// Read a model as its tree.
pub fn read(bytes: &[u8]) -> Result<Model> {
    let mut reader = Reader::new(bytes);
    if bytes.get(..6) != Some(b"sc6969".as_slice()) {
        return Err(Error::NotAModel);
    }
    reader.skip(6)?;
    let version = reader.u32()?;
    if !KNOWN_VERSIONS.contains(&version) {
        return Err(Error::Version(version));
    }
    let extra = match version > 5 {
        true => reader.u32()?,
        false => 0,
    };
    let mut textures = Vec::new();
    for _ in 0..reader.u32()? {
        let kind = reader.u32()?;
        let name = reader.text()?;
        let size = reader.u32()? as usize;
        let bytes = reader.bytes(size)?.to_vec();
        textures.push(TextureEntry {
            kind,
            texture: Texture { name, bytes },
        });
    }
    let mut materials = Vec::new();
    for _ in 0..reader.u32()? {
        let from = reader.at();
        let material = parse_material(&mut reader)?;
        let raw = bytes.get(from..reader.at()).map(<[u8]>::to_vec);
        materials.push(MaterialEntry { material, raw });
    }
    // Depth first, an explicit stack of (parent, children left).
    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<(Option<usize>, usize)> = vec![(None, 1)];
    while let Some(top) = stack.last_mut() {
        if top.1 == 0 {
            stack.pop();
            continue;
        }
        top.1 -= 1;
        let parent = top.0;
        let at = reader.at();
        let kind = reader.u32()?;
        let length = reader.u32()? as usize;
        let name_bytes = reader.bytes(length)?.to_vec();
        let children = reader.u32()? as usize;
        let active = reader.u8()?;
        let from = reader.at();
        let mesh = match kind {
            DUMMY => {
                reader.skip(64)?;
                None
            }
            MESH | SKINNED_MESH => Some(mesh_facts(&mut reader, kind, from)?),
            other => return Err(Error::UnknownNode { at, kind: other }),
        };
        let body = bytes.get(from..reader.at()).unwrap_or_default().to_vec();
        let id = nodes.len();
        nodes.push(Node {
            kind,
            name: String::from_utf8_lossy(&name_bytes).into_owned(),
            name_bytes,
            active,
            body,
            children: Vec::new(),
            mesh,
        });
        if let Some(parent) = parent.and_then(|p| nodes.get_mut(p)) {
            parent.children.push(id);
        }
        if children > 0 {
            stack.push((Some(id), children));
        }
    }
    Ok(Model {
        version,
        extra,
        textures,
        materials,
        nodes,
    })
}

fn parse_material(reader: &mut Reader<'_>) -> Result<Material> {
    let name = reader.text()?;
    let shader = reader.text()?;
    let blend = reader.u8()?;
    let alpha_tested = reader.u8()? != 0;
    let depth_mode = reader.u32()?;
    let mut properties = Vec::new();
    for _ in 0..reader.u32()? {
        let name = reader.text()?;
        let value = reader.f32()?;
        reader.skip(4 * 2)?;
        let vector = [reader.f32()?, reader.f32()?, reader.f32()?];
        reader.skip(4 * 4)?;
        properties.push(crate::scene::Property {
            name,
            value,
            vector,
        });
    }
    let mut textures = Vec::new();
    for _ in 0..reader.u32()? {
        let slot = reader.text()?;
        reader.u32()?;
        textures.push((slot, reader.text()?));
    }
    Ok(Material {
        name,
        shader,
        blend,
        alpha_tested,
        depth_mode,
        properties,
        textures,
    })
}

/// Step over a mesh's body, noting what it holds and where its material's
/// number is — `from` is where the body starts in the file.
fn mesh_facts(reader: &mut Reader<'_>, kind: u32, from: usize) -> Result<MeshFacts> {
    reader.u8()?; // casts shadows
    let visible = reader.u8()? != 0;
    reader.u8()?; // transparent
    let skinned = kind == SKINNED_MESH;
    if skinned {
        for _ in 0..reader.u32()? {
            reader.text()?;
            reader.skip(4 * 16)?;
        }
    }
    let vertices = reader.u32()? as usize;
    let each = 4 * 11 + if skinned { 4 * 8 } else { 0 };
    reader.skip(vertices.checked_mul(each).ok_or(Error::Truncated {
        at: reader.at(),
        wanted: usize::MAX,
        left: 0,
    })?)?;
    let indices = reader.u32()? as usize;
    reader.skip(indices.saturating_mul(2))?;
    let material_at = reader.at() - from;
    let material = reader.u32()? as usize;
    reader.u32()?; // layer
    // **A skinned mesh ends in eight bytes**; a plain one in its levels of
    // detail, bounding sphere and renderable flag. See `scene`.
    let renderable = match skinned {
        true => {
            reader.skip(8)?;
            true
        }
        false => {
            reader.skip(4 * 2 + 4 * 4)?;
            reader.u8()? != 0
        }
    };
    Ok(MeshFacts {
        vertices,
        triangles: indices / 3,
        material,
        visible: visible && renderable,
        skinned,
        material_at,
    })
}

/// A mesh's body read as geometry, placed by `parent`.
fn parse_mesh(reader: &mut Reader<'_>, kind: u32, name: &str, parent: &Matrix) -> Result<Mesh> {
    reader.u8()?; // casts shadows
    let visible = reader.u8()? != 0;
    let transparent = reader.u8()? != 0;
    let skinned = kind == SKINNED_MESH;
    if skinned {
        for _ in 0..reader.u32()? {
            reader.text()?;
            reader.skip(4 * 16)?;
        }
    }
    let count = reader.u32()? as usize;
    let mut vertices = Vec::with_capacity(count.min(1 << 20));
    for _ in 0..count {
        let p = [reader.f32()?, reader.f32()?, reader.f32()?];
        let n = [reader.f32()?, reader.f32()?, reader.f32()?];
        let uv = [reader.f32()?, reader.f32()?];
        reader.skip(4 * 3)?; // tangent
        if skinned {
            reader.skip(4 * 4 + 4 * 4)?;
        }
        vertices.push(Vertex {
            position: point(parent, p),
            normal: direction(parent, n),
            uv,
        });
    }
    let index_count = reader.u32()? as usize;
    let raw = reader.bytes(index_count.saturating_mul(2))?;
    let read: Vec<u16> = raw
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    // **Whole triangles or none.** A triangle naming a vertex that is not
    // there is left out whole; dropping one index shifts every triangle
    // after it by a corner.
    let indices = read
        .chunks_exact(3)
        .filter(|tri| tri.iter().all(|i| (*i as usize) < vertices.len()))
        .flatten()
        .copied()
        .collect();
    let material = reader.u32()? as usize;
    reader.u32()?; // layer
    let (lod_in, lod_out, renderable) = match skinned {
        true => {
            reader.skip(8)?;
            (0.0, 0.0, true)
        }
        false => {
            let lod_in = reader.f32()?;
            let lod_out = reader.f32()?;
            reader.skip(4 * 4)?; // bounding sphere
            (lod_in, lod_out, reader.u8()? != 0)
        }
    };
    Ok(Mesh {
        name: name.to_string(),
        vertices,
        indices,
        material,
        visible: visible && renderable,
        transparent,
        lod_in,
        lod_out,
    })
}

fn point(m: &Matrix, p: [f32; 3]) -> [f32; 3] {
    let mut out = [0.0; 3];
    for (k, o) in out.iter_mut().enumerate() {
        *o = p[0] * m[0][k] + p[1] * m[1][k] + p[2] * m[2][k] + m[3][k];
    }
    out
}

fn direction(m: &Matrix, n: [f32; 3]) -> [f32; 3] {
    let mut out = [0.0; 3];
    for (k, o) in out.iter_mut().enumerate() {
        *o = n[0] * m[0][k] + n[1] * m[1][k] + n[2] * m[2][k];
    }
    let length = (out[0] * out[0] + out[1] * out[1] + out[2] * out[2]).sqrt();
    if length > 1e-6 {
        for o in &mut out {
            *o /= length;
        }
    }
    out
}

/// Any direction square to a normal.
fn tangent(n: [f32; 3]) -> [f32; 3] {
    let other = match n[1].abs() < 0.9 {
        true => [0.0, 1.0, 0.0],
        false => [1.0, 0.0, 0.0],
    };
    let t = [
        other[1] * n[2] - other[2] * n[1],
        other[2] * n[0] - other[0] * n[2],
        other[0] * n[1] - other[1] * n[0],
    ];
    let l = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt().max(1e-9);
    [t[0] / l, t[1] / l, t[2] / l]
}

/// A sphere every vertex is inside: the middle of the box, and the farthest.
fn sphere(vertices: &[Vertex]) -> ([f32; 3], f32) {
    if vertices.is_empty() {
        return ([0.0; 3], 0.0);
    }
    let mut low = [f32::MAX; 3];
    let mut high = [f32::MIN; 3];
    for v in vertices {
        for k in 0..3 {
            low[k] = low[k].min(v.position[k]);
            high[k] = high[k].max(v.position[k]);
        }
    }
    let centre = [0, 1, 2].map(|k| (low[k] + high[k]) * 0.5);
    let radius = vertices
        .iter()
        .map(|v| {
            let d = [0, 1, 2].map(|k| v.position[k] - centre[k]);
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        })
        .fold(0.0f32, f32::max);
    (centre, radius)
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_text(out: &mut Vec<u8>, text: &[u8]) {
    put_u32(out, text.len() as u32);
    out.extend_from_slice(text);
}

/// A material as the file writes it.
pub fn material_bytes(material: &Material) -> Vec<u8> {
    let mut out = Vec::new();
    put_text(&mut out, material.name.as_bytes());
    put_text(&mut out, material.shader.as_bytes());
    out.push(material.blend);
    out.push(u8::from(material.alpha_tested));
    put_u32(&mut out, material.depth_mode);
    put_u32(&mut out, material.properties.len() as u32);
    for p in &material.properties {
        put_text(&mut out, p.name.as_bytes());
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
        put_text(&mut out, slot.as_bytes());
        put_u32(&mut out, k as u32);
        put_text(&mut out, name.as_bytes());
    }
    out
}

impl Model {
    /// A model of nothing but a root, to build on.
    pub fn new(root: &str) -> Model {
        Model {
            version: 6,
            extra: 0,
            textures: Vec::new(),
            materials: Vec::new(),
            nodes: vec![Node::dummy(root, &IDENTITY)],
        }
    }

    /// Write it, as the game reads it.
    pub fn write(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"sc6969");
        put_u32(&mut out, self.version);
        if self.version > 5 {
            put_u32(&mut out, self.extra);
        }
        put_u32(&mut out, self.textures.len() as u32);
        for entry in &self.textures {
            put_u32(&mut out, entry.kind);
            put_text(&mut out, entry.texture.name.as_bytes());
            put_u32(&mut out, entry.texture.bytes.len() as u32);
            out.extend_from_slice(&entry.texture.bytes);
        }
        put_u32(&mut out, self.materials.len() as u32);
        for entry in &self.materials {
            match &entry.raw {
                Some(raw) => out.extend_from_slice(raw),
                None => out.extend_from_slice(&material_bytes(&entry.material)),
            }
        }
        let mut stack = vec![0usize];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            put_u32(&mut out, node.kind);
            put_text(&mut out, &node.name_bytes);
            put_u32(&mut out, node.children.len() as u32);
            out.push(node.active);
            out.extend_from_slice(&node.body);
            // Depth first, in order: the last child is pushed first.
            stack.extend(node.children.iter().rev());
        }
        out
    }

    /// Every node reachable from the root, depth first, with its depth.
    pub fn walk(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut stack = vec![(0usize, 0usize)];
        while let Some((id, depth)) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            out.push((id, depth));
            stack.extend(node.children.iter().rev().map(|c| (*c, depth + 1)));
        }
        out
    }

    /// Each node's parent, by place.
    pub fn parents(&self) -> Vec<Option<usize>> {
        let mut out = vec![None; self.nodes.len()];
        for (id, node) in self.nodes.iter().enumerate() {
            for child in &node.children {
                if let Some(slot) = out.get_mut(*child) {
                    *slot = Some(id);
                }
            }
        }
        out
    }

    /// Where a node is in the model's own space: every transform from the
    /// root down to it, its own included.
    pub fn world(&self, id: usize) -> Matrix {
        let parents = self.parents();
        let mut chain = vec![id];
        let mut at = id;
        while let Some(Some(up)) = parents.get(at) {
            if chain.len() > self.nodes.len() {
                break;
            }
            chain.push(*up);
            at = *up;
        }
        chain.iter().fold(IDENTITY, |m, k| {
            match self.nodes.get(*k).and_then(Node::transform) {
                Some(local) => multiply(&m, &local),
                None => m,
            }
        })
    }

    /// The first node of this name, any case.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.walk().into_iter().map(|(id, _)| id).find(|id| {
            self.nodes
                .get(*id)
                .is_some_and(|n| n.name.eq_ignore_ascii_case(name))
        })
    }

    /// Move a dummy: its own transform replaced.
    pub fn set_transform(&mut self, id: usize, transform: &Matrix) -> bool {
        let Some(node) = self.nodes.get_mut(id).filter(|n| n.kind == DUMMY) else {
            return false;
        };
        node.body.clear();
        for row in transform {
            for cell in row {
                node.body.extend_from_slice(&cell.to_le_bytes());
            }
        }
        true
    }

    /// Show or hide a mesh — in the game as here.
    pub fn set_visible(&mut self, id: usize, visible: bool) -> bool {
        let Some(node) = self.nodes.get_mut(id) else {
            return false;
        };
        let (Some(facts), Some(flag)) = (node.mesh.as_mut(), node.body.get_mut(1)) else {
            return false;
        };
        *flag = u8::from(visible);
        facts.visible = visible;
        true
    }

    /// Draw a mesh with another material.
    pub fn set_material(&mut self, id: usize, material: usize) -> bool {
        if material >= self.materials.len() {
            return false;
        }
        let Some(node) = self.nodes.get_mut(id) else {
            return false;
        };
        let Some(facts) = node.mesh.as_mut() else {
            return false;
        };
        let at = facts.material_at;
        let Some(slot) = node.body.get_mut(at..at + 4) else {
            return false;
        };
        slot.copy_from_slice(&(material as u32).to_le_bytes());
        facts.material = material;
        true
    }

    /// Take a node, and everything under it, out of the model.
    pub fn remove(&mut self, id: usize) -> bool {
        if id == 0 {
            return false;
        }
        let mut found = false;
        for node in &mut self.nodes {
            let before = node.children.len();
            node.children.retain(|c| *c != id);
            found |= node.children.len() != before;
        }
        found
    }

    /// Drop every node nothing reaches any more — what [`Model::remove`]
    /// left behind — so a model edited many times does not carry every
    /// version of itself. Places change; the old place of each node, its
    /// new one or `None`.
    pub fn compact(&mut self) -> Vec<Option<usize>> {
        let order: Vec<usize> = self.walk().into_iter().map(|(id, _)| id).collect();
        let mut map = vec![None; self.nodes.len()];
        for (new, old) in order.iter().enumerate() {
            if let Some(slot) = map.get_mut(*old) {
                *slot = Some(new);
            }
        }
        let mut old = std::mem::take(&mut self.nodes);
        self.nodes = order
            .iter()
            .filter_map(|id| {
                old.get_mut(*id)
                    .map(|n| std::mem::replace(n, Node::dummy("", &IDENTITY)))
            })
            .collect();
        for node in &mut self.nodes {
            node.children = node
                .children
                .iter()
                .filter_map(|c| map.get(*c).copied().flatten())
                .collect();
        }
        map
    }

    /// A node put under `parent`; its place in the list.
    pub fn add(&mut self, parent: usize, node: Node) -> Option<usize> {
        let id = self.nodes.len();
        self.nodes.get_mut(parent)?.children.push(id);
        self.nodes.push(node);
        Some(id)
    }

    /// A material added; its place.
    pub fn add_material(&mut self, material: Material) -> usize {
        self.materials.push(MaterialEntry {
            material,
            raw: None,
        });
        self.materials.len() - 1
    }

    /// A material changed: written from what it holds from now on.
    pub fn material_mut(&mut self, k: usize) -> Option<&mut Material> {
        let entry = self.materials.get_mut(k)?;
        entry.raw = None;
        Some(&mut entry.material)
    }

    /// A texture put in, over one of the same name if there is one.
    pub fn put_texture(&mut self, texture: Texture) {
        match self
            .textures
            .iter_mut()
            .find(|t| t.texture.name.eq_ignore_ascii_case(&texture.name))
        {
            Some(entry) => entry.texture = texture,
            None => self.textures.push(TextureEntry { kind: 1, texture }),
        }
    }

    /// Drawn as a showroom draws it: every mesh reachable from the root,
    /// placed by every transform above it.
    pub fn scene(&self) -> Result<Scene> {
        let mut scene = Scene {
            textures: self.textures.iter().map(|t| t.texture.clone()).collect(),
            materials: self.materials.iter().map(|m| m.material.clone()).collect(),
            meshes: Vec::new(),
        };
        let mut stack = vec![(0usize, IDENTITY)];
        let mut steps = 0usize;
        while let Some((id, parent)) = stack.pop() {
            steps += 1;
            if steps > self.nodes.len() * 2 + 2 {
                break;
            }
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let here = match node.transform() {
                Some(local) => multiply(&local, &parent),
                None => parent,
            };
            if node.kind != DUMMY {
                let mut reader = Reader::new(&node.body);
                scene
                    .meshes
                    .push(parse_mesh(&mut reader, node.kind, &node.name, &parent)?);
            }
            stack.extend(node.children.iter().rev().map(|c| (*c, here)));
        }
        Ok(scene)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::scene;

    fn quad(name: &str, material: usize) -> Mesh {
        let v = |x: f32, y: f32| Vertex {
            position: [x, y, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [x, y],
        };
        Mesh {
            name: name.to_string(),
            vertices: vec![v(0.0, 0.0), v(1.0, 0.0), v(0.0, 1.0), v(1.0, 1.0)],
            indices: vec![0, 1, 2, 1, 3, 2],
            material,
            visible: true,
            transparent: false,
            lod_in: 0.0,
            lod_out: 0.0,
        }
    }

    fn material(name: &str) -> Material {
        Material {
            name: name.to_string(),
            shader: "ksPerPixel".to_string(),
            blend: 0,
            alpha_tested: false,
            depth_mode: 0,
            properties: vec![],
            textures: vec![("txDiffuse".to_string(), "paint.dds".to_string())],
        }
    }

    fn built() -> Model {
        let mut model = Model::new("root");
        model.add_material(material("paint"));
        model.add_material(material("glass"));
        model.put_texture(Texture {
            name: "paint.dds".to_string(),
            bytes: b"DDS ".to_vec(),
        });
        let mut moved = IDENTITY;
        moved[3] = [0.0, 2.0, 0.0, 1.0];
        let wheel = model.add(0, Node::dummy("WHEEL_LF", &moved)).unwrap();
        model.add(wheel, Node::mesh(&quad("rim", 1))).unwrap();
        model.add(0, Node::mesh(&quad("body", 0))).unwrap();
        model
    }

    /// **Written and read again, the same model** — and the same file: a
    /// model nothing changed is written back byte for byte.
    #[test]
    fn a_model_comes_back_as_it_was() {
        let bytes = built().write();
        let again = read(&bytes).unwrap();
        assert_eq!(again.write(), bytes);
        let names: Vec<&str> = again
            .walk()
            .iter()
            .map(|(id, _)| again.nodes[*id].name.as_str())
            .collect();
        assert_eq!(names, ["root", "WHEEL_LF", "rim", "body"]);
        // What the showroom reads is the same either way.
        assert_eq!(scene::read(&bytes).unwrap(), again.scene().unwrap());
    }

    /// **Placed by the tree**: the rim under a wheel two metres up is two
    /// metres up, and moving the wheel moves it.
    #[test]
    fn the_tree_places_what_is_under_it() {
        let mut model = built();
        let scene = model.scene().unwrap();
        let rim = scene.meshes.iter().find(|m| m.name == "rim").unwrap();
        assert_eq!(rim.vertices[0].position, [0.0, 2.0, 0.0]);
        let wheel = model.find("wheel_lf").unwrap();
        let mut higher = IDENTITY;
        higher[3] = [1.0, 3.0, 0.0, 1.0];
        assert!(model.set_transform(wheel, &higher));
        let again = read(&model.write()).unwrap().scene().unwrap();
        let rim = again.meshes.iter().find(|m| m.name == "rim").unwrap();
        assert_eq!(rim.vertices[0].position, [1.0, 3.0, 0.0]);
        assert_eq!(model.world(wheel)[3], [1.0, 3.0, 0.0, 1.0]);
    }

    /// **Hidden, given another material, taken out** — and the game reads
    /// what was asked.
    #[test]
    fn nodes_are_hidden_repainted_and_removed() {
        let mut model = built();
        let body = model.find("body").unwrap();
        assert!(model.set_visible(body, false));
        assert!(model.set_material(body, 1));
        assert!(!model.set_material(body, 9));
        let rim = model.find("rim").unwrap();
        let wheel = model.find("WHEEL_LF").unwrap();
        assert!(model.remove(wheel));
        assert!(!model.remove(0), "never the root");
        let again = read(&model.write()).unwrap();
        assert!(again.find("rim").is_none(), "gone with its wheel: {rim}");
        let scene = again.scene().unwrap();
        assert_eq!(scene.meshes.len(), 1);
        assert!(!scene.meshes[0].visible);
        assert_eq!(scene.meshes[0].material, 1);
        // A changed material is written from what it holds now.
        let mut model = again;
        model.material_mut(0).unwrap().name = "PAINT_NEW".to_string();
        let back = read(&model.write()).unwrap();
        assert_eq!(back.materials[0].material.name, "PAINT_NEW");
    }

    /// **Compacted, the same model**: what was removed is gone from the
    /// list, what was kept writes the same file.
    #[test]
    fn a_compacted_model_writes_the_same() {
        let mut model = built();
        let wheel = model.find("WHEEL_LF").unwrap();
        model.remove(wheel);
        let before = model.write();
        let map = model.compact();
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(map[wheel], None);
        assert_eq!(model.write(), before);
        assert_eq!(model.find("body"), Some(1));
    }

    /// **A real car comes back as it was**: every car in the folder named
    /// by `KN5_CARS` read and written with nothing changed is the same
    /// file, and draws the same. Not run without real cars.
    #[test]
    #[ignore]
    fn a_real_car_comes_back_as_it_was() {
        let Ok(folder) = std::env::var("KN5_CARS") else {
            return;
        };
        for entry in std::fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("kn5") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let model = read(&bytes).unwrap();
            assert!(model.write() == bytes, "{} changed", path.display());
            let meshes = model.scene().unwrap().meshes.len();
            assert!(meshes > 10, "{}: {meshes} meshes", path.display());
        }
    }
}
