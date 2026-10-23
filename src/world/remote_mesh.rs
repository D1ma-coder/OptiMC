//! Official block-model geometry for client-owned server chunks. No generated
//! terrain is substituted for absent chunks. Fluid/special renderers and vanilla
//! lighting/biome tint remain separate, unfinished rendering paths.
use super::{
    remote::{RemoteWorld, StoredSection},
    Mesh,
};
use crate::assets::{
    builtins,
    models::{
        random::{position_seed, ModelRandom},
        BakedQuad, Catalog, Direction, ResourceId, Variant,
    },
};
use anyhow::{ensure, Context, Result};
use glam::Vec3;
use std::{collections::HashMap, path::Path, sync::Arc};
mod chests;
mod entities;
mod frames;
mod signs;
pub const MAP_TILE_VERSION_FLAG: u64 = 1 << 63;

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];
const STEPS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];
#[derive(Clone, Copy)]
struct Sprite {
    layer: u32,
    opaque: bool,
    blended: bool,
}
struct Quad {
    baked: BakedQuad,
    layer: u32,
    blended: bool,
}
struct Part {
    quads: Vec<Quad>,
    opaque_faces: u8,
}
struct Group {
    parts: Vec<(Arc<Part>, u32)>,
    weight: i32,
}
struct StateModel {
    groups: Vec<Group>,
    multipart: bool,
    opaque_faces: u8,
    unculled: bool,
}
#[derive(Clone)]
struct Stamp {
    sections: [Option<Arc<StoredSection>>; 7],
}
impl Stamp {
    fn same(&self, other: &Self) -> bool {
        self.sections
            .iter()
            .zip(&other.sections)
            .all(|(a, b)| match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            })
    }
}
struct Cached {
    stamp: Stamp,
    visibility: u64,
    meshes: Vec<Arc<Mesh>>,
}
#[derive(Default, Debug)]
pub struct Statistics {
    pub rebuilt_sections: usize,
    pub reused_sections: usize,
    pub quads: usize,
}
pub struct RemoteMesher {
    catalog: Catalog,
    sprites: HashMap<ResourceId, Sprite>,
    pixels: Vec<u8>,
    texture_versions: Vec<u64>,
    frames: frames::Cache,
    signs: signs::Cache,
    chests: chests::Cache,
    entities: entities::Cache,
    cache_path: std::path::PathBuf,
    parts: HashMap<(String, i32, i32, bool), Arc<Part>>,
    states: HashMap<u32, Arc<StateModel>>,
    sections: HashMap<(i32, usize, i32), Cached>,
    dimension: Option<(String, i32, u32)>,
    terrain_columns: HashMap<(i32, i32), Arc<super::remote::Column>>,
    terrain_meshes: Vec<Arc<Mesh>>,
    terrain_quads: usize,
    terrain_ready: bool,
    visibility: Arc<crate::mods::sodium::Graph>,
}
impl RemoteMesher {
    pub fn new(cache: &Path) -> Self {
        Self {
            catalog: Catalog::new(cache),
            sprites: HashMap::new(),
            pixels: Vec::new(),
            texture_versions: Vec::new(),
            frames: frames::Cache::default(),
            signs: signs::Cache::default(),
            chests: chests::Cache::default(),
            entities: entities::Cache::default(),
            cache_path: cache.to_owned(),
            parts: HashMap::new(),
            states: HashMap::new(),
            sections: HashMap::new(),
            dimension: None,
            terrain_columns: HashMap::new(),
            terrain_meshes: Vec::new(),
            terrain_quads: 0,
            terrain_ready: false,
            visibility: Arc::new(Default::default()),
        }
    }
    /// Additional 64×64 texture-array layers, in stable order starting at 7,
    /// matching the renderer's six base block textures and player skin.
    pub fn texture_pixels(&self) -> &[u8] {
        &self.pixels
    }
    pub fn texture_versions(&self) -> &[u64] {
        &self.texture_versions
    }
    fn sprite(&mut self, id: &ResourceId) -> Result<Sprite> {
        if let Some(sprite) = self.sprites.get(id) {
            return Ok(*sprite);
        }
        let image = if id.path == "missingno" {
            image::RgbaImage::from_fn(16, 16, |x, y| {
                image::Rgba(if (x < 8) == (y < 8) {
                    [0, 0, 0, 255]
                } else {
                    [255, 0, 255, 255]
                })
            })
        } else {
            image::open(self.catalog.texture_file(id))
                .with_context(|| format!("Loading {id}"))?
                .to_rgba8()
        };
        let size = image.width().min(image.height());
        ensure!(size > 0, "Empty sprite {id}");
        let image = image::imageops::crop_imm(&image, 0, 0, size, size).to_image();
        let sprite = Sprite {
            layer: 7 + (self.pixels.len() / 16384) as u32,
            opaque: image.pixels().all(|p| p[3] == 255),
            blended: image.pixels().any(|p| p[3] != 0 && p[3] != 255),
        };
        self.pixels.extend_from_slice(
            image::imageops::resize(&image, 64, 64, image::imageops::FilterType::Nearest).as_raw(),
        );
        self.sprites.insert(id.clone(), sprite);
        self.texture_versions.push(1);
        Ok(sprite)
    }
    fn part(&mut self, variant: &Variant) -> Result<Arc<Part>> {
        let key = (variant.model.clone(), variant.x, variant.y, variant.uvlock);
        if let Some(part) = self.parts.get(&key) {
            return Ok(part.clone());
        }
        let quads = self.catalog.bake(variant)?;
        let mut part = Part {
            quads: Vec::with_capacity(quads.len()),
            opaque_faces: 0,
        };
        for baked in quads.iter() {
            let sprite = self.sprite(&baked.material.sprite)?;
            let blended = sprite.blended || baked.material.force_translucent;
            if sprite.opaque && !blended {
                for (index, direction) in DIRECTIONS.iter().enumerate() {
                    if full_face(baked, *direction) {
                        part.opaque_faces |= 1 << index;
                    }
                }
            }
            part.quads.push(Quad {
                baked: baked.clone(),
                layer: sprite.layer,
                blended,
            });
        }
        let part = Arc::new(part);
        self.parts.insert(key, part.clone());
        Ok(part)
    }
    fn state(&mut self, id: u32) -> Result<Arc<StateModel>> {
        if let Some(state) = self.states.get(&id) {
            return Ok(state.clone());
        }
        let state = builtins::get()?
            .state(id)
            .with_context(|| format!("Unknown block state ID {id}"))?;
        let dispatch = self.catalog.blockstate(&ResourceId::parse(&state.block)?)?;
        let mut model = StateModel {
            groups: Vec::new(),
            multipart: !dispatch.multipart.is_empty(),
            opaque_faces: 0,
            unculled: false,
        };
        for choices in dispatch.choices(&state.properties)? {
            let mut group = Group {
                parts: Vec::new(),
                weight: 0,
            };
            let mut guaranteed_faces = 63;
            for variant in choices.all() {
                ensure!(
                    variant.weight > 0 && variant.weight <= i32::MAX as u32,
                    "Invalid variant weight"
                );
                group.weight = group
                    .weight
                    .checked_add(variant.weight as i32)
                    .context("Model weights overflow")?;
                let part = self.part(variant)?;
                model.unculled |= part.quads.iter().any(|q| q.baked.cullface.is_none());
                guaranteed_faces &= part.opaque_faces;
                group.parts.push((part, variant.weight));
            }
            ensure!(group.weight > 0, "Empty model variants");
            model.opaque_faces |= guaranteed_faces;
            model.groups.push(group);
        }
        let model = Arc::new(model);
        self.states.insert(id, model.clone());
        Ok(model)
    }
    /// Reuse each section only while its own data and its six face neighbours
    /// remain identical. Missing-to-present neighbours also invalidate borders.
    pub fn update(&mut self, world: &RemoteWorld) -> Result<(Vec<Arc<Mesh>>, Statistics)> {
        self.update_while(world, || true)
    }
    pub fn update_while(
        &mut self,
        world: &RemoteWorld,
        proceed: impl Fn() -> bool,
    ) -> Result<(Vec<Arc<Mesh>>, Statistics)> {
        let dimension = world.dimension.as_ref().context("No server dimension")?;
        let key = (
            world
                .spawn
                .as_ref()
                .map(|s| s.dimension.clone())
                .unwrap_or_else(|| dimension.name.clone()),
            dimension.min_y,
            dimension.height,
        );
        if self.dimension.as_ref() != Some(&key) {
            self.sections.clear();
            self.terrain_columns.clear();
            self.terrain_meshes.clear();
            self.terrain_ready = false;
            self.dimension = Some(key);
        }
        ensure!(proceed(), "Server meshing cancelled");
        // Entity/map/metadata updates do not invalidate terrain. Compare immutable
        // column identities before constructing seven-neighbour section stamps,
        // sorting columns or rebuilding the portal graph.
        if self.terrain_ready
            && self.terrain_columns.len() == world.chunks.len()
            && self.terrain_columns.iter().all(|(key, column)| {
                world
                    .chunks
                    .get(key)
                    .is_some_and(|other| Arc::ptr_eq(column, other))
            })
        {
            return Ok((
                self.terrain_meshes.clone(),
                Statistics {
                    reused_sections: self.sections.len(),
                    quads: self.terrain_quads,
                    ..Default::default()
                },
            ));
        }
        // A cancelled/failed partial rebuild must never publish an old cache.
        self.terrain_ready = false;
        let old = std::mem::take(&mut self.sections);
        let mut retained = HashMap::new();
        let mut meshes = Vec::new();
        let mut stats = Statistics::default();
        let mut columns: Vec<_> = world.chunks.keys().copied().collect();
        columns.sort_unstable();
        for (x, z) in columns {
            for section in 0..world.chunks[&(x, z)].sections.len() {
                ensure!(proceed(), "Server meshing cancelled");
                let key = (x, section, z);
                let stamp = stamp(world, x, section, z);
                let cached =
                    if let Some(cached) = old.get(&key).filter(|old| old.stamp.same(&stamp)) {
                        stats.reused_sections += 1;
                        Cached {
                            stamp,
                            meshes: cached.meshes.clone(),
                            visibility: cached.visibility,
                        }
                    } else {
                        stats.rebuilt_sections += 1;
                        Cached {
                            stamp,
                            meshes: self.build(world, x, section, z)?,
                            visibility: self.section_visibility(world, x, section, z)?,
                        }
                    };
                stats.quads += cached
                    .meshes
                    .iter()
                    .map(|m| m.indices.len() / 6)
                    .sum::<usize>();
                meshes.extend(cached.meshes.iter().cloned());
                retained.insert(key, cached);
            }
        }
        let visibility = retained
            .iter()
            .map(|(&(x, section, z), cached)| {
                (
                    [x, section as i32 + dimension.min_y.div_euclid(16), z],
                    cached.visibility,
                )
            })
            .collect();
        if self.visibility.sections != visibility {
            self.visibility = Arc::new(crate::mods::sodium::Graph {
                sections: visibility,
            });
        }
        self.sections = retained;
        self.terrain_columns.clone_from(&world.chunks);
        self.terrain_meshes.clone_from(&meshes);
        self.terrain_quads = stats.quads;
        self.terrain_ready = true;
        Ok((meshes, stats))
    }
    pub fn visibility(&self) -> Arc<crate::mods::sodium::Graph> {
        self.visibility.clone()
    }
    fn section_visibility(
        &mut self,
        world: &RemoteWorld,
        x: i32,
        section: usize,
        z: i32,
    ) -> Result<u64> {
        let blocks = &world.chunks[&(x, z)].sections[section].blocks;
        if let super::remote::Values::Uniform { value, .. } = blocks {
            return Ok(if self.state(*value)?.opaque_faces == 63 {
                0
            } else {
                crate::mods::sodium::ALL
            });
        }
        let mut opaque = [0u64; 64];
        let mut states = HashMap::new();
        for i in 0..4096 {
            let id = blocks.get(i).context("Invalid section block index")?;
            let solid = if let Some(&solid) = states.get(&id) {
                solid
            } else {
                let solid = self.state(id)?.opaque_faces == 63;
                states.insert(id, solid);
                solid
            };
            if solid {
                opaque[i / 64] |= 1 << (i % 64);
            }
        }
        Ok(crate::mods::sodium::resolve(&opaque)
            .into_iter()
            .fold(0, |a, b| a | b))
    }
    fn build(
        &mut self,
        world: &RemoteWorld,
        cx: i32,
        section: usize,
        cz: i32,
    ) -> Result<Vec<Arc<Mesh>>> {
        let origin = [
            cx * 16,
            world.dimension.as_ref().unwrap().min_y + section as i32 * 16,
            cz * 16,
        ];
        let min = Vec3::from_array(origin.map(|v| v as f32));
        let mut solid = Mesh::new(min, min + Vec3::splat(16.), 0);
        solid.section = Some([cx, origin[1].div_euclid(16), cz]);
        let mut blended = solid.clone();
        blended.translucent = true;
        let blocks = &world.chunks[&(cx, cz)].sections[section].blocks;
        if matches!(blocks, super::remote::Values::Uniform { value: 0, .. }) {
            return Ok(Vec::new());
        }
        for index in 0..4096 {
            let id = blocks.get(index).context("Invalid section storage")?;
            if id == 0 {
                continue;
            }
            let model = self.state(id)?;
            if model.groups.is_empty() {
                continue;
            }
            let local = [
                (index % 16) as i32,
                (index / 256) as i32,
                (index / 16 % 16) as i32,
            ];
            let position = std::array::from_fn(|i| origin[i] + local[i]);
            let translation = Vec3::from_array(position.map(|v| v as f32));
            let mut hidden = 0u8;
            for (face, step) in STEPS.iter().enumerate() {
                if let Some(neighbor) = world.block(
                    position[0] + step[0],
                    position[1] + step[1],
                    position[2] + step[2],
                ) {
                    if neighbor != 0 && self.state(neighbor)?.opaque_faces & (1 << (face ^ 1)) != 0
                    {
                        hidden |= 1 << face;
                    }
                }
            }
            if hidden == 63 && !model.unculled {
                continue;
            }
            let mut random = ModelRandom::new(position_seed(position));
            let multipart_seed = model.multipart.then(|| random.next_long());
            for group in &model.groups {
                if let Some(seed) = multipart_seed {
                    random = ModelRandom::new(seed);
                }
                let mut ticket = random.bounded(group.weight) as u32;
                let part = group
                    .parts
                    .iter()
                    .find_map(|(part, weight)| {
                        if ticket < *weight {
                            Some(part)
                        } else {
                            ticket -= weight;
                            None
                        }
                    })
                    .context("Invalid weighted model selection")?;
                for quad in &part.quads {
                    if quad
                        .baked
                        .cullface
                        .is_some_and(|face| hidden & (1 << face as u8) != 0)
                    {
                        continue;
                    }
                    let q = &quad.baked;
                    let shade = super::classic_shade(q.normal, q.shade);
                    let tint = if q.tint_index >= 0 {
                        [0.46, 0.77, 0.27]
                    } else {
                        [1.; 3]
                    };
                    let target = if quad.blended {
                        &mut blended
                    } else {
                        &mut solid
                    };
                    target.quad_uv_shaded(
                        q.points.map(|p| (p + translation).to_array()),
                        q.uv.map(|p| p.to_array()),
                        quad.layer | if quad.blended { 0x8000_0000 } else { 0 },
                        tint,
                        shade,
                    );
                }
            }
        }
        Ok([solid, blended]
            .into_iter()
            .filter(|m| !m.indices.is_empty())
            .map(|mut m| {
                m.tighten_bounds();
                m.polygon_lods = super::detail::variants(&m);
                Arc::new(m)
            })
            .collect())
    }
}
fn stamp(world: &RemoteWorld, x: i32, section: usize, z: i32) -> Stamp {
    let get = |x, z, y: i32| {
        if y < 0 {
            None
        } else {
            world
                .chunks
                .get(&(x, z))
                .and_then(|c| c.sections.get(y as usize))
                .cloned()
        }
    };
    let y = section as i32;
    Stamp {
        sections: [
            get(x, z, y),
            get(x, z, y - 1),
            get(x, z, y + 1),
            get(x, z - 1, y),
            get(x, z + 1, y),
            get(x - 1, z, y),
            get(x + 1, z, y),
        ],
    }
}
fn full_face(quad: &BakedQuad, face: Direction) -> bool {
    if quad.normal.dot(face.normal()) < 0.9999 {
        return false;
    }
    let axis = if face.normal().x != 0. {
        0
    } else if face.normal().y != 0. {
        1
    } else {
        2
    };
    let plane = if face.normal()[axis] > 0. { 1. } else { 0. };
    if quad
        .points
        .iter()
        .any(|p| (p[axis] - plane).abs() > 0.00001)
    {
        return false;
    }
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    // Require the four exact unit-square corners, not just its bounding box.
    [(0., 0.), (0., 1.), (1., 0.), (1., 1.)]
        .iter()
        .all(|(a, b)| {
            quad.points
                .iter()
                .any(|p| (p[u] - a).abs() < 0.00001 && (p[v] - b).abs() < 0.00001)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{
        chunk::{Chunk, Section},
        registry::Dimension,
    };
    fn world() -> RemoteWorld {
        let mut world = RemoteWorld {
            dimension: Some(Dimension {
                name: "minecraft:overworld".into(),
                min_y: 0,
                height: 32,
                logical_height: 32,
                skylight: true,
                ceiling: false,
            }),
            ..RemoteWorld::default()
        };
        for x in 0..2 {
            world
                .insert(Chunk {
                    block_entities: vec![],
                    x,
                    z: 0,
                    sections: (0..2)
                        .map(|_| Section {
                            non_air: 0,
                            fluids: 0,
                            blocks: vec![0; 4096],
                            biomes: vec![0; 64],
                        })
                        .collect(),
                })
                .unwrap();
        }
        world
    }
    #[test]
    #[ignore = "Explicit CPU-only terrain scheduling benchmark"]
    fn benchmark_unchanged_terrain() {
        let mut world = world();
        let column = world.chunks[&(0, 0)].clone();
        world.chunks = (0..421).map(|x| ((x, 0), column.clone())).collect();
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        mesher.update(&world).unwrap();
        let iterations = 200;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            mesher.terrain_ready = false;
            std::hint::black_box(mesher.update(std::hint::black_box(&world)).unwrap());
        }
        let section_path = start.elapsed();
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            std::hint::black_box(mesher.update(std::hint::black_box(&world)).unwrap());
        }
        let column_path = start.elapsed();
        println!("CPU terrain: 421 unchanged columns, 842 empty sections, {iterations} updates; section path {section_path:?}; column fast path {column_path:?}");
        // Timing is diagnostic, never a flaky performance assertion.
    }
    #[test]
    fn unchanged_columns_skip_section_stamps_and_keep_visibility_identity() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        let mut world = world();
        let (_, initial) = mesher.update(&world).unwrap();
        assert_eq!(initial.rebuilt_sections, 4);
        let graph = mesher.visibility();
        let calls = std::cell::Cell::new(0);
        let (_, repeated) = mesher
            .update_while(&world, || {
                calls.set(calls.get() + 1);
                true
            })
            .unwrap();
        assert_eq!(calls.get(), 1, "fast path must not visit each section");
        assert_eq!(repeated.reused_sections, 4);
        assert!(Arc::ptr_eq(&graph, &mesher.visibility()));
        assert!(mesher.update_while(&world, || false).is_err());
        world.forget(&0i64.to_be_bytes()).unwrap();
        let (_, removed) = mesher.update(&world).unwrap();
        assert_eq!(removed.rebuilt_sections, 2);
        world.chunks.clear();
        assert!(mesher.update(&world).unwrap().0.is_empty());
        assert!(mesher.visibility().sections.is_empty());
    }
    #[test]
    #[ignore = "Requires imported official 26.2 resources; CPU only"]
    fn official_tiled_section_has_four_cached_polygon_lods_and_keeps_full_geometry() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        let mut world = world();
        let stone = builtins::get()
            .unwrap()
            .default_state("minecraft:cobblestone")
            .unwrap();
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    world.set_block(x, y, z, stone);
                }
            }
        }
        let (meshes, _) = mesher.update(&world).unwrap();
        assert_eq!(meshes.len(), 1);
        let mesh = &meshes[0];
        assert_eq!(mesh.indices.len() / 6, 1536);
        assert_eq!(
            mesh.polygon_lods
                .iter()
                .map(|m| m.indices.len() / 6)
                .collect::<Vec<_>>(),
            vec![384, 96, 24, 6]
        );
        for level in &mesh.polygon_lods {
            assert_eq!(level.min, mesh.min);
            assert_eq!(level.max, mesh.max);
            assert_eq!(level.section, mesh.section);
            assert!(level
                .vertices
                .iter()
                .all(|v| v.layer >= 7 && v.origin == [0.; 3]));
        }
        let (again, stats) = mesher.update(&world).unwrap();
        assert_eq!(stats.rebuilt_sections, 0);
        assert!(Arc::ptr_eq(&meshes[0], &again[0]));
    }
    #[test]
    #[ignore = "Requires imported official 26.2 resources"]
    fn official_server_sections_cull_boundaries_and_invalidate_only_dependencies() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        let mut world = world();
        let stone = builtins::get()
            .unwrap()
            .default_state("minecraft:stone")
            .unwrap();
        for [x, y, z] in [[15, 15, 0], [16, 15, 0], [15, 16, 0]] {
            world.set_block(x, y, z, stone);
        }
        let (first, stats) = mesher.update(&world).unwrap();
        assert_eq!(stats.rebuilt_sections, 4);
        assert_eq!(stats.quads, 14);
        assert!(first.iter().flat_map(|m| &m.vertices).all(|v| v.layer >= 7));
        let (second, stats) = mesher.update(&world).unwrap();
        assert_eq!(stats.reused_sections, 4);
        assert_eq!(stats.rebuilt_sections, 0);
        assert!(first.iter().zip(&second).all(|(a, b)| Arc::ptr_eq(a, b)));
        world.set_block(16, 15, 0, 0);
        let (_, stats) = mesher.update(&world).unwrap();
        assert_eq!(stats.quads, 10);
        assert_eq!(stats.rebuilt_sections, 3);
        assert_eq!(stats.reused_sections, 1);
        world.forget(&1i64.to_be_bytes()).unwrap();
        let (_, stats) = mesher.update(&world).unwrap();
        assert_eq!(stats.rebuilt_sections, 2);
        assert_eq!(stats.quads, 10);
        world.dimension.as_mut().unwrap().name = "minecraft:the_nether".into();
        let (_, stats) = mesher.update(&world).unwrap();
        assert_eq!(stats.rebuilt_sections, 2);
        assert_eq!(stats.reused_sections, 0);
    }
    #[test]
    #[ignore = "Requires imported official 26.2 resources"]
    fn all_official_states_compile_and_transparent_neighbors_do_not_hide_stone() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        let ids = builtins::get().unwrap();
        for id in 0..ids.block_states.len() as u32 {
            mesher.state(id).unwrap();
        }
        let stone = ids.default_state("minecraft:stone").unwrap();
        let glass = ids.default_state("minecraft:glass").unwrap();
        let stairs = ids.default_state("minecraft:oak_stairs").unwrap();
        assert_eq!(mesher.state(stone).unwrap().opaque_faces, 63);
        assert_eq!(mesher.state(glass).unwrap().opaque_faces, 0);
        assert_ne!(mesher.state(stairs).unwrap().opaque_faces, 63);
        let mut world = world();
        world.set_block(15, 15, 0, stone);
        world.set_block(16, 15, 0, glass);
        let (meshes, stats) = mesher.update(&world).unwrap();
        // Stone keeps its side through glass; glass can drop its opaque-covered side.
        assert_eq!(stats.quads, 11);
        assert!(meshes.iter().any(|m| m.translucent));
        assert!(meshes.iter().any(|m| !m.translucent));
        assert_eq!(mesher.texture_pixels().len() % 16384, 0);
        println!(
            "Remote model catalog: {} states, {} parts, {} sprites",
            mesher.states.len(),
            mesher.parts.len(),
            mesher.sprites.len()
        );
    }
}
