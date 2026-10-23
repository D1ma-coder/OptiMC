//! Cached CPU item geometry, with alpha-contour extrusion for generated layers.
use super::{
    item_icons::node_models,
    models::{Catalog, DisplayTransform, ResourceId, Variant},
};
use crate::world::Mesh;
use anyhow::{ensure, Context, Result};
use glam::{Mat4, Vec3};
use image::RgbaImage;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub item: u32,
    pub model: Option<String>,
    pub left: bool,
}
impl Key {
    pub fn stack(stack: &crate::net::items::Stack, left: bool) -> Self {
        let model = stack
            .component("minecraft:item_model")
            .and_then(|mut b| crate::net::codec::read_string(&mut b, 32767).ok());
        Self {
            item: stack.item,
            model,
            left,
        }
    }
}
pub struct Model {
    pub mesh: Mesh,
    pub textures: Vec<RgbaImage>,
}
pub struct Worker {
    send: mpsc::SyncSender<Key>,
    receive: mpsc::Receiver<(Key, Option<Arc<Model>>)>,
    pending: HashSet<Key>,
    cache: HashMap<Key, (Option<Arc<Model>>, u64)>,
    clock: u64,
}
impl Worker {
    pub fn new(cache: &Path) -> Result<Self> {
        let (send, requests) = mpsc::sync_channel::<Key>(8);
        let (results, receive) = mpsc::sync_channel(8);
        let cache = cache.to_owned();
        std::thread::Builder::new()
            .name("held-item-models".into())
            .spawn(move || {
                let mut baker = Baker::new(&cache);
                while let Ok(key) = requests.recv() {
                    let model = baker
                        .bake(&key)
                        .map(Arc::new)
                        .map_err(|e| eprintln!("Held item {}: {e:#}", key.item))
                        .ok();
                    if results.send((key, model)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            send,
            receive,
            pending: HashSet::new(),
            cache: HashMap::new(),
            clock: 0,
        })
    }
    pub fn get(&mut self, key: &Key) -> Option<Arc<Model>> {
        while let Ok((key, model)) = self.receive.try_recv() {
            self.pending.remove(&key);
            self.cache.insert(key, (model, self.clock));
            if self.cache.len() > 64 {
                if let Some(key) = self
                    .cache
                    .iter()
                    .min_by_key(|(_, (_, age))| *age)
                    .map(|(key, _)| key.clone())
                {
                    self.cache.remove(&key);
                }
            }
        }
        self.clock = self.clock.wrapping_add(1);
        if let Some((model, age)) = self.cache.get_mut(key) {
            *age = self.clock;
            return model.clone();
        }
        if !self.pending.contains(key) && self.send.try_send(key.clone()).is_ok() {
            self.pending.insert(key.clone());
        }
        None
    }
}
pub fn display(d: &DisplayTransform, left: bool) -> Mat4 {
    let sign = if left { -1. } else { 1. };
    Mat4::from_translation(Vec3::from_array(d.translation) * Vec3::new(sign, 1., 1.) / 16.)
        * Mat4::from_quat(glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            d.rotation[0].to_radians(),
            (sign * d.rotation[1]).to_radians(),
            (sign * d.rotation[2]).to_radians(),
        ))
        * Mat4::from_scale(Vec3::from_array(d.scale))
        * Mat4::from_translation(Vec3::splat(-0.5))
}
/// The generator retains one side per nontransparent -> transparent transition.
pub fn contour(image: &RgbaImage) -> Vec<(u8, u32, u32)> {
    let (w, h) = image.dimensions();
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if image.get_pixel(x, y)[3] == 0 {
                continue;
            }
            for (side, dx, dy) in [(0, 0, -1), (1, 0, 1), (2, -1, 0), (3, 1, 0)] {
                let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                if xx < 0
                    || yy < 0
                    || xx >= w as i32
                    || yy >= h as i32
                    || image.get_pixel(xx as u32, yy as u32)[3] == 0
                {
                    out.push((side, x, y));
                }
            }
        }
    }
    out
}
fn generated(mesh: &mut Mesh, image: &RgbaImage, layer: u32, tint: [f32; 3]) {
    let (w, h) = (image.width() as f32, image.height() as f32);
    let (z, zz) = (7.5 / 16., 8.5 / 16.);
    mesh.quad_uv(
        [[0., 1., zz], [0., 0., zz], [1., 0., zz], [1., 1., zz]],
        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
        layer,
        tint,
    );
    mesh.quad_uv(
        [[1., 1., z], [1., 0., z], [0., 0., z], [0., 1., z]],
        [[1., 0.], [1., 1.], [0., 1.], [0., 0.]],
        layer,
        tint,
    );
    for (side, x, y) in contour(image) {
        let (x, y) = (x as f32, y as f32);
        let (a, b, c, d) = (x / w, (x + 1.) / w, 1. - (y + 1.) / h, 1. - y / h);
        let points = match side {
            0 => [[a, d, z], [a, d, zz], [b, d, zz], [b, d, z]],
            1 => [[a, c, zz], [a, c, z], [b, c, z], [b, c, zz]],
            2 => [[a, d, z], [a, c, z], [a, c, zz], [a, d, zz]],
            _ => [[b, d, zz], [b, c, zz], [b, c, z], [b, d, z]],
        };
        let (u, v, uu, vv) = ((x + 0.1) / w, (y + 0.1) / h, (x + 0.9) / w, (y + 0.9) / h);
        mesh.quad_uv(points, [[u, v], [u, vv], [uu, vv], [uu, v]], layer, tint);
    }
}
fn load(
    root: &Path,
    textures: &mut Vec<RgbaImage>,
    ids: &mut HashMap<ResourceId, u32>,
    id: &ResourceId,
) -> Result<u32> {
    if let Some(&layer) = ids.get(id) {
        return Ok(layer);
    }
    ensure!(textures.len() < 16, "Held item texture budget exceeded");
    let image = image::open(id.file(root, "textures", "png"))?.to_rgba8();
    let side = image.width().min(image.height());
    ensure!(side > 0 && side <= 256, "Invalid held texture");
    let image = image::imageops::crop_imm(&image, 0, 0, side, side).to_image();
    let layer = textures.len() as u32;
    textures.push(image);
    ids.insert(id.clone(), layer);
    Ok(layer)
}
pub struct Baker {
    root: PathBuf,
    catalog: Catalog,
}
impl Baker {
    pub fn new(cache: &Path) -> Self {
        Self {
            root: super::root(cache).join("resources/assets"),
            catalog: Catalog::new(cache),
        }
    }
    pub fn bake(&mut self, key: &Key) -> Result<Model> {
        let name = key
            .model
            .as_deref()
            .or_else(|| {
                super::builtins::get()
                    .ok()?
                    .name("minecraft:item", key.item)
            })
            .context("Unknown held item")?;
        let resource = ResourceId::parse(name)?;
        let definition: serde_json::Value =
            serde_json::from_slice(&std::fs::read(resource.file(&self.root, "items", "json"))?)?;
        let context = if key.left {
            "firstperson_lefthand"
        } else {
            "firstperson_righthand"
        };
        let mut models = Vec::new();
        node_models(&definition["model"], &mut models, 0, context)?;
        let mut mesh = Mesh::new(Vec3::splat(-2.), Vec3::splat(2.), 0);
        let mut textures = Vec::new();
        let mut texture_ids = HashMap::new();
        for (name, tints) in models {
            let model = self.catalog.model(&ResourceId::parse(&name)?)?;
            let transform = display(
                model
                    .display
                    .get(context)
                    .or_else(|| {
                        key.left
                            .then(|| model.display.get("firstperson_righthand"))
                            .flatten()
                    })
                    .unwrap_or(&DisplayTransform::default()),
                key.left,
            );
            let base = mesh.vertices.len();
            if model.elements.is_empty() {
                for layer in 0..5 {
                    let name = format!("layer{layer}");
                    if !model.textures.contains_key(&name) {
                        continue;
                    }
                    let material = model.material(&format!("#{name}"))?;
                    let texture = load(
                        &self.root,
                        &mut textures,
                        &mut texture_ids,
                        &material.sprite,
                    )?;
                    ensure!(
                        contour(&textures[texture as usize]).len() <= 16384,
                        "Generated item contour budget exceeded"
                    );
                    generated(
                        &mut mesh,
                        &textures[texture as usize],
                        texture,
                        tints.get(layer).copied().unwrap_or([1.; 3]),
                    );
                }
            } else {
                for quad in self
                    .catalog
                    .bake(&Variant {
                        model: name,
                        x: 0,
                        y: 0,
                        uvlock: false,
                        weight: 1,
                    })?
                    .iter()
                {
                    let layer = load(
                        &self.root,
                        &mut textures,
                        &mut texture_ids,
                        &quad.material.sprite,
                    )?;
                    let tint = if quad.tint_index >= 0 {
                        tints
                            .get(quad.tint_index as usize)
                            .copied()
                            .unwrap_or([1.; 3])
                    } else {
                        [1.; 3]
                    };
                    mesh.quad_uv(
                        quad.points.map(|p| p.to_array()),
                        quad.uv.map(|p| p.to_array()),
                        layer,
                        tint,
                    );
                }
            }
            ensure!(
                mesh.vertices.len() <= 131072,
                "Held item geometry budget exceeded"
            );
            let normal_matrix = glam::Mat3::from_mat4(transform).inverse().transpose();
            for v in &mut mesh.vertices[base..] {
                v.position = transform
                    .transform_point3(Vec3::from_array(v.position))
                    .to_array();
                v.normal = normal_matrix
                    .mul_vec3(Vec3::from_array(v.normal))
                    .normalize_or_zero()
                    .to_array();
                v.lod = 256;
            }
        }
        ensure!(!mesh.indices.is_empty(), "No regular held item geometry");
        mesh.tighten_bounds();
        Ok(Model { mesh, textures })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, io::Read};
    fn reference() -> serde_json::Value {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/held-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        serde_json::from_slice(&raw).unwrap()
    }
    #[test]
    fn display_transform_mirrors_left_hand_like_original_item_transform() {
        let d = DisplayTransform {
            rotation: [13., 47., -23.],
            translation: [1.25, -3., 2.],
            scale: [0.8, 0.7, 1.2],
        };
        for row in reference()["displays"].as_array().unwrap() {
            let expected = row["matrix"].as_array().unwrap();
            for (i, v) in display(&d, row["left"].as_bool().unwrap())
                .to_cols_array()
                .iter()
                .enumerate()
            {
                assert!(
                    (v - expected[i].as_f64().unwrap() as f32).abs() < 2e-6,
                    "{i}: {v}"
                );
            }
        }
    }
    #[test]
    fn alpha_holes_create_inner_contours_without_extruding_internal_shared_edges() {
        let image = RgbaImage::from_fn(3, 3, |x, y| {
            image::Rgba([255, 255, 255, if (x, y) == (1, 1) { 0 } else { 255 }])
        });
        let edges = contour(&image);
        assert_eq!(edges.len(), 16);
        let mut mesh = Mesh::new(Vec3::ZERO, Vec3::ONE, 0);
        generated(&mut mesh, &image, 0, [1.; 3]);
        assert_eq!(mesh.indices.len() / 6, 18);
        assert!(mesh
            .vertices
            .iter()
            .all(|v| Vec3::from_array(v.normal).is_normalized()));
    }
    #[test]
    #[ignore = "Requires imported official item resources; CPU only"]
    fn source_alpha_contours_and_full_models_compile_for_both_first_person_hands() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let root = super::super::root(&cache).join("resources/assets");
        let r = reference();
        for row in r["sprites"].as_array().unwrap() {
            let id = ResourceId::parse(row["texture"].as_str().unwrap()).unwrap();
            let image = image::open(id.file(&root, "textures", "png"))
                .unwrap()
                .to_rgba8();
            let expected: BTreeSet<_> = row["edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| {
                    (
                        match e[0].as_str().unwrap() {
                            "UP" => 0,
                            "DOWN" => 1,
                            "LEFT" => 2,
                            "RIGHT" => 3,
                            _ => unreachable!(),
                        },
                        e[1].as_u64().unwrap() as u32,
                        e[2].as_u64().unwrap() as u32,
                    )
                })
                .collect();
            assert_eq!(
                contour(&image).into_iter().collect::<BTreeSet<_>>(),
                expected,
                "{}",
                row["texture"]
            );
        }
        let mut baker = Baker::new(&cache);
        for name in [
            "minecraft:stone",
            "minecraft:diamond",
            "minecraft:iron_pickaxe",
            "minecraft:compass",
        ] {
            let item = crate::net::item_defaults::get()
                .unwrap()
                .iter()
                .find(|i| i.name == name)
                .unwrap();
            for left in [false, true] {
                let model = baker
                    .bake(&Key {
                        item: item.id,
                        model: None,
                        left,
                    })
                    .unwrap();
                assert!(!model.mesh.indices.is_empty());
                assert!(model
                    .mesh
                    .vertices
                    .iter()
                    .all(|v| Vec3::from_array(v.position).is_finite()
                        && Vec3::from_array(v.normal).is_finite()
                        && (v.layer as usize) < model.textures.len()));
            }
        }
    }
}
