//! CPU thumbnails from downloaded item definitions/models/textures. No GPU or
//! Java is used by the baker. Dynamic/special item renderers remain separate.
use super::models::{Catalog, ResourceId, Variant};
use anyhow::{ensure, Context, Result};
use glam::{Mat4, Vec2, Vec3};
use image::{Rgba, RgbaImage};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
};
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub item: u32,
    pub model: Option<String>,
    pub skin: Option<String>,
}
impl Key {
    pub fn plain(item: u32) -> Self {
        Self {
            item,
            model: None,
            skin: None,
        }
    }
    pub fn stack(stack: &crate::net::items::Stack) -> Self {
        Self {
            item: stack.item,
            model: stack
                .component("minecraft:item_model")
                .and_then(|mut b| crate::net::codec::read_string(&mut b, 32767).ok()),
            skin: stack
                .component("minecraft:profile")
                .and_then(|b| profile_skin(b).ok().flatten()),
        }
    }
    pub fn cache_name(&self) -> String {
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(self.item.to_le_bytes());
        h.update(self.model.as_deref().unwrap_or(""));
        h.update([0]);
        h.update(self.skin.as_deref().unwrap_or(""));
        format!("item:{:x}", h.finalize())
    }
}
fn profile_skin(mut b: &[u8]) -> Result<Option<String>> {
    use crate::net::{
        codec::read_string,
        maps::{byte, bytes},
    };
    use base64::Engine;
    if byte(&mut b)? != 0 {
        bytes(&mut b, 16)?;
        read_string(&mut b, 16)?;
    } else {
        if byte(&mut b)? != 0 {
            read_string(&mut b, 16)?;
        }
        if byte(&mut b)? != 0 {
            bytes(&mut b, 16)?;
        }
    }
    let n = crate::net::items::tool::count(&mut b, 1024)?;
    let mut url = None;
    for _ in 0..n {
        let name = read_string(&mut b, 32767)?;
        let value = read_string(&mut b, 32767)?;
        if byte(&mut b)? != 0 {
            read_string(&mut b, 32767)?;
        }
        if name == "textures" {
            let raw = base64::engine::general_purpose::STANDARD.decode(value)?;
            let json: serde_json::Value = serde_json::from_slice(&raw)?;
            url = json["textures"]["SKIN"]["url"].as_str().map(str::to_owned);
        }
    }
    Ok(url)
}
pub struct Worker {
    send: SyncSender<Key>,
    receive: Receiver<(Key, Arc<RgbaImage>)>,
    pending: HashSet<Key>,
    cache: HashMap<Key, (Arc<RgbaImage>, u64)>,
    clock: u64,
}
impl Worker {
    pub fn new(cache: &Path) -> Result<Self> {
        let (send, requests) = mpsc::sync_channel(64);
        let (results, receive) = mpsc::sync_channel(64);
        let path = cache.to_owned();
        std::thread::Builder::new()
            .name("item-icons".into())
            .spawn(move || {
                let mut baker = Baker::new(&path);
                while let Ok(id) = requests.recv() {
                    let pixels = baker.bake_key(&id).unwrap_or_else(|_| missing());
                    if results.send((id, Arc::new(pixels))).is_err() {
                        break;
                    }
                }
            })
            .context("Starting item icon worker")?;
        Ok(Self {
            send,
            receive,
            pending: HashSet::new(),
            cache: HashMap::new(),
            clock: 0,
        })
    }
    pub fn get(&mut self, id: &Key) -> Option<Arc<RgbaImage>> {
        while let Ok((id, pixels)) = self.receive.try_recv() {
            self.pending.remove(&id);
            self.cache.insert(id, (pixels, self.clock));
            if self.cache.len() > 512 {
                if let Some(old) = self
                    .cache
                    .iter()
                    .min_by_key(|(_, (_, age))| *age)
                    .map(|(id, _)| id.clone())
                {
                    self.cache.remove(&old);
                }
            }
        }
        self.clock = self.clock.wrapping_add(1);
        if let Some((pixels, age)) = self.cache.get_mut(id) {
            *age = self.clock;
            return Some(pixels.clone());
        }
        if !self.pending.contains(id) && self.send.try_send(id.clone()).is_ok() {
            self.pending.insert(id.clone());
        }
        None
    }
}
pub fn missing() -> RgbaImage {
    RgbaImage::from_fn(64, 64, |x, y| {
        if (x / 16 + y / 16) % 2 == 0 {
            Rgba([255, 0, 255, 255])
        } else {
            Rgba([0, 0, 0, 255])
        }
    })
}
pub struct Baker {
    root: PathBuf,
    head_cache: PathBuf,
    catalog: Catalog,
    textures: HashMap<ResourceId, Arc<RgbaImage>>,
}
pub(super) fn node_models(
    node: &serde_json::Value,
    out: &mut Vec<(String, Vec<[f32; 3]>)>,
    depth: usize,
    context: &str,
) -> Result<()> {
    ensure!(depth < 32, "Item definition too deep");
    match node["type"].as_str().unwrap_or("") {
        "minecraft:model" => {
            let model = node["model"]
                .as_str()
                .context("Missing item model")?
                .to_owned();
            let tints = node["tints"]
                .as_array()
                .map(|t| {
                    t.iter()
                        .map(|t| {
                            let c = t["value"]
                                .as_i64()
                                .or_else(|| t["default"].as_i64())
                                .unwrap_or(if t["type"] == "minecraft:grass" {
                                    0x91bd59
                                } else {
                                    0xffffff
                                }) as u32;
                            [
                                ((c >> 16) & 255) as f32 / 255.,
                                ((c >> 8) & 255) as f32 / 255.,
                                (c & 255) as f32 / 255.,
                            ]
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push((model, tints));
        }
        "minecraft:condition" => node_models(&node["on_false"], out, depth + 1, context)?,
        "minecraft:range_dispatch" => {
            // Zero is the neutral GUI sample. Missing fallback is legal: choose
            // the greatest threshold <= value, just like RangeSelectItemModel.
            let selected = node["entries"]
                .as_array()
                .and_then(|entries| {
                    entries
                        .iter()
                        .filter(|e| e["threshold"].as_f64().is_some_and(|v| v <= 0.))
                        .max_by(|a, b| {
                            a["threshold"]
                                .as_f64()
                                .unwrap()
                                .total_cmp(&b["threshold"].as_f64().unwrap())
                        })
                })
                .map(|e| &e["model"])
                .unwrap_or(&node["fallback"]);
            node_models(selected, out, depth + 1, context)?;
        }
        "minecraft:select" => {
            let selected = if node["property"] == "minecraft:display_context" {
                node["cases"]
                    .as_array()
                    .and_then(|cases| {
                        cases.iter().find(|case| {
                            case["when"] == context
                                || case["when"]
                                    .as_array()
                                    .is_some_and(|values| values.iter().any(|v| v == context))
                        })
                    })
                    .map(|case| &case["model"])
            } else {
                None
            };
            node_models(
                selected.unwrap_or(&node["fallback"]),
                out,
                depth + 1,
                context,
            )?;
        }
        "minecraft:composite" => {
            for child in node["models"]
                .as_array()
                .context("Missing composite models")?
            {
                node_models(child, out, depth + 1, context)?;
            }
        }
        "minecraft:special" if node["model"]["type"] == "minecraft:player_head" => {
            out.push(("optimc:special/player".into(), Vec::new()));
        }
        "minecraft:special" if node["model"]["type"] == "minecraft:head" => {
            let kind = node["model"]["kind"]
                .as_str()
                .context("Missing skull kind")?;
            out.push((format!("optimc:special/{kind}"), Vec::new()));
        }
        "minecraft:empty" => {}
        _ => anyhow::bail!("Dynamic/special item renderer not implemented"),
    }
    Ok(())
}
impl Baker {
    pub fn new(cache: &Path) -> Self {
        Self {
            root: super::root(cache).join("resources/assets"),
            head_cache: super::root(cache).join("item-head-skins"),
            catalog: Catalog::new(cache),
            textures: HashMap::new(),
        }
    }
    fn texture(&mut self, id: &ResourceId) -> Result<Arc<RgbaImage>> {
        if let Some(image) = self.textures.get(id) {
            return Ok(image.clone());
        }
        let image = image::open(id.file(&self.root, "textures", "png"))?.to_rgba8();
        let side = image.width().min(image.height());
        ensure!(side > 0 && side <= 4096, "Invalid item texture");
        let image = Arc::new(image::imageops::crop_imm(&image, 0, 0, side, side).to_image());
        // Bounded decoded-texture cache; compiled model definitions are registry-bound.
        if self.textures.len() >= 512 {
            self.textures.clear();
        }
        self.textures.insert(id.clone(), image.clone());
        Ok(image)
    }
    pub fn bake(&mut self, id: u32) -> Result<RgbaImage> {
        self.bake_key(&Key::plain(id))
    }
    pub fn bake_key(&mut self, key: &Key) -> Result<RgbaImage> {
        let id = key.item;
        let name = super::builtins::get()?
            .name("minecraft:item", id)
            .context("Unknown item ID")?;
        let resource = ResourceId::parse(key.model.as_deref().unwrap_or(name))?;
        let definition: serde_json::Value =
            serde_json::from_slice(&std::fs::read(resource.file(&self.root, "items", "json"))?)?;
        let mut models = Vec::new();
        node_models(&definition["model"], &mut models, 0, "gui")?;
        let mut image = RgbaImage::new(64, 64);
        for (model, tints) in models {
            if let Some(kind) = model.strip_prefix("optimc:special/") {
                let pixels = self.head(kind, key)?;
                image::imageops::overlay(&mut image, &pixels, 0, 0);
                continue;
            }
            let model_id = ResourceId::parse(&model)?;
            let resolved = self.catalog.model(&model_id)?;
            if resolved.elements.is_empty() {
                let mut layers = 0;
                for layer in 0..5 {
                    let key = format!("layer{layer}");
                    if !resolved.textures.contains_key(&key) {
                        continue;
                    }
                    let material = resolved.material(&format!("#{key}"))?;
                    let texture = self.texture(&material.sprite)?;
                    let tint = tints.get(layer).copied().unwrap_or([1.; 3]);
                    let mut pixels = image::imageops::resize(
                        texture.as_ref(),
                        64,
                        64,
                        image::imageops::FilterType::Nearest,
                    );
                    for p in pixels.pixels_mut() {
                        for c in 0..3 {
                            p[c] = (f32::from(p[c]) * tint[c]).round() as u8;
                        }
                    }
                    image::imageops::overlay(&mut image, &pixels, 0, 0);
                    layers += 1;
                }
                ensure!(layers > 0, "Special item model without flat layers");
                continue;
            }
            let display = resolved.display.get("gui").cloned().unwrap_or_default();
            let rotation = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                display.rotation[0].to_radians(),
                display.rotation[1].to_radians(),
                display.rotation[2].to_radians(),
            );
            let transform = Mat4::from_translation(Vec3::from_array(display.translation) / 16.)
                * Mat4::from_quat(rotation)
                * Mat4::from_scale(Vec3::from_array(display.scale));
            let quads = self.catalog.bake(&Variant {
                model: model.clone(),
                x: 0,
                y: 0,
                uvlock: false,
                weight: 1,
            })?;
            let mut faces = Vec::new();
            for quad in quads.iter() {
                let points = quad
                    .points
                    .map(|p| transform.transform_point3(p - Vec3::splat(0.5)));
                let normal = (points[1] - points[0])
                    .cross(points[2] - points[0])
                    .normalize_or_zero();
                if normal.z <= 0. {
                    continue;
                }
                let texture = self.texture(&quad.material.sprite)?;
                let tint = if quad.tint_index >= 0 {
                    tints
                        .get(quad.tint_index as usize)
                        .copied()
                        .unwrap_or([1.; 3])
                } else {
                    [1.; 3]
                };
                let light = if resolved.gui_light == "front" || !quad.shade {
                    1.
                } else {
                    (0.4 + 0.6 * normal.dot(Vec3::new(-0.3, 0.8, 0.6).normalize()).max(0.)).min(1.)
                };
                faces.push((
                    points.iter().map(|p| p.z).sum::<f32>(),
                    points,
                    quad.uv,
                    texture,
                    tint.map(|c| c * light),
                ));
            }
            faces.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, points, uv, texture, tint) in faces {
                for tri in [[0, 1, 2], [0, 2, 3]] {
                    triangle(
                        &mut image,
                        tri.map(|i| points[i]),
                        tri.map(|i| uv[i]),
                        &texture,
                        tint,
                    );
                }
            }
        }
        Ok(image)
    }
}
fn triangle(
    out: &mut RgbaImage,
    points: [Vec3; 3],
    uv: [Vec2; 3],
    texture: &RgbaImage,
    tint: [f32; 3],
) {
    let p = points.map(|p| Vec2::new(32. + p.x * 64., 32. - p.y * 64.));
    let edge = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - a);
    let area = edge(p[0], p[1], p[2]);
    if area.abs() < 1e-6 {
        return;
    }
    let min = p
        .iter()
        .fold(Vec2::splat(f32::INFINITY), |a, p| a.min(*p))
        .floor()
        .max(Vec2::ZERO);
    let max = p
        .iter()
        .fold(Vec2::splat(f32::NEG_INFINITY), |a, p| a.max(*p))
        .ceil()
        .min(Vec2::splat(63.));
    for y in min.y as u32..=max.y as u32 {
        for x in min.x as u32..=max.x as u32 {
            let sample = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w = [
                edge(p[1], p[2], sample) / area,
                edge(p[2], p[0], sample) / area,
                edge(p[0], p[1], sample) / area,
            ];
            if w.iter().any(|w| *w < -1e-6) {
                continue;
            }
            let t = uv[0] * w[0] + uv[1] * w[1] + uv[2] * w[2];
            let tx = (t.x * texture.width() as f32).floor() as i32;
            let ty = (t.y * texture.height() as f32).floor() as i32;
            let mut c = *texture.get_pixel(
                tx.clamp(0, texture.width() as i32 - 1) as u32,
                ty.clamp(0, texture.height() as i32 - 1) as u32,
            );
            if c[3] == 0 {
                continue;
            }
            for a in 0..3 {
                c[a] = (f32::from(c[a]) * tint[a]).round() as u8;
            }
            use image::Pixel;
            out.get_pixel_mut(x, y).blend(&c);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires imported official item models/textures; CPU only"]
    fn official_flat_item_texels_and_block_gui_mesh_reach_cpu_icons() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let items = crate::net::item_defaults::get().unwrap();
        let id = |name: &str| items.iter().find(|i| i.name == name).unwrap().id;
        let mut baker = Baker::new(&cache);
        let icon = baker.bake(id("minecraft:diamond")).unwrap();
        let source = image::open(
            super::super::root(&cache).join("resources/assets/minecraft/textures/item/diamond.png"),
        )
        .unwrap()
        .to_rgba8();
        let expected =
            image::imageops::resize(&source, 64, 64, image::imageops::FilterType::Nearest);
        assert_eq!(icon, expected);
        let stone = baker.bake(id("minecraft:stone")).unwrap();
        assert!(stone.pixels().filter(|p| p[3] > 0).count() > 1000);
        assert!(stone.pixels().any(|p| p[3] == 0));
        let out = cache.join("inventory-icon-preview.png");
        let mut preview = RgbaImage::new(192, 64);
        image::imageops::overlay(&mut preview, &icon, 0, 0);
        image::imageops::overlay(&mut preview, &stone, 64, 0);
        image::imageops::overlay(
            &mut preview,
            &baker.bake(id("minecraft:iron_pickaxe")).unwrap(),
            128,
            0,
        );
        preview.save(out).unwrap();
    }
}

#[cfg(test)]
mod dynamic_tests {
    use super::*;
    #[test]
    #[ignore = "Requires imported official item models/textures; CPU only"]
    fn compass_and_clock_without_fallback_render_their_original_threshold_models() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut baker = Baker::new(&cache);
        let items = crate::net::item_defaults::get().unwrap();
        for name in [
            "minecraft:compass",
            "minecraft:recovery_compass",
            "minecraft:clock",
        ] {
            let item = items.iter().find(|i| i.name == name).unwrap();
            let image = baker.bake(item.id).unwrap();
            assert!(image.pixels().filter(|p| p[3] > 0).count() > 100);
            assert_ne!(image, missing());
            image
                .save(cache.join(format!(
                    "{}-icon-preview.png",
                    name.strip_prefix("minecraft:").unwrap()
                )))
                .unwrap();
        }
    }
}

impl Baker {
    fn skin(&self, url: &str) -> Result<RgbaImage> {
        use sha1::{Digest, Sha1};
        use std::io::Read;
        let mut url = reqwest::Url::parse(url)?;
        ensure!(
            matches!(
                url.host_str(),
                Some("textures.minecraft.net" | "skinsystem.ely.by" | "ely.by")
            ) && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none_or(|p| p == 80 || p == 443)
                && matches!(url.scheme(), "http" | "https"),
            "Unsupported head texture host"
        );
        url.set_scheme("https")
            .map_err(|_| anyhow::anyhow!("Invalid texture scheme"))?;
        url.set_port(None)
            .map_err(|_| anyhow::anyhow!("Invalid texture port"))?;
        let name = format!("{:x}.png", Sha1::digest(url.as_str().as_bytes()));
        let path = self.head_cache.join(name);
        if let Ok(raw) = std::fs::read(&path) {
            return crate::accounts::decode_skin(&raw);
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(4))
            .connect_timeout(std::time::Duration::from_secs(2))
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = client.get(url).send()?.error_for_status()?;
        let mut raw = Vec::new();
        response.take(1024 * 1024 + 1).read_to_end(&mut raw)?;
        ensure!(raw.len() <= 1024 * 1024, "Head PNG too large");
        let image = crate::accounts::decode_skin(&raw)?;
        std::fs::create_dir_all(&self.head_cache)?;
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, &raw)?;
        std::fs::rename(temporary, path)?;
        Ok(image)
    }
    fn head(&mut self, kind: &str, key: &Key) -> Result<RgbaImage> {
        let (layer, texture) = match kind {
            "player" => ("player_head", "entity/player/wide/steve"),
            "skeleton" => ("skeleton_skull", "entity/skeleton/skeleton"),
            "wither_skeleton" => ("wither_skeleton_skull", "entity/skeleton/wither_skeleton"),
            "creeper" => ("creeper_head", "entity/creeper/creeper"),
            "zombie" => ("zombie_head", "entity/zombie/zombie"),
            "piglin" => ("piglin_head", "entity/piglin/piglin"),
            "dragon" => ("dragon_skull", "entity/enderdragon/dragon"),
            _ => anyhow::bail!("Unknown skull model"),
        };
        let custom = if kind == "player" {
            key.skin.as_deref().and_then(|url| self.skin(url).ok())
        } else {
            None
        };
        let texture = match custom {
            Some(image) => image,
            None => image::open(ResourceId::parse(texture)?.file(&self.root, "textures", "png"))?
                .to_rgba8(),
        };
        let report = super::entity_models::get()?;
        let root = report
            .layers
            .get(&format!("minecraft:{layer}#main"))
            .context("Missing original skull layer")?;
        // Original template_skull GUI transform and player-head model transform.
        let display = self
            .catalog
            .model(&ResourceId::parse("item/template_skull")?)?
            .display
            .get("gui")
            .cloned()
            .unwrap_or_default();
        let rotation = glam::Quat::from_euler(
            glam::EulerRot::XYZ,
            display.rotation[0].to_radians(),
            display.rotation[1].to_radians(),
            display.rotation[2].to_radians(),
        );
        let transform = Mat4::from_translation(Vec3::from_array(display.translation) / 16.)
            * Mat4::from_quat(rotation)
            * Mat4::from_scale(Vec3::from_array(display.scale))
            * Mat4::from_translation(Vec3::new(0., -0.5, 0.))
            * Mat4::from_rotation_x(std::f32::consts::PI);
        let mut mesh = crate::world::Mesh::new(Vec3::ZERO, Vec3::ZERO, 0);
        super::entity_models::bake(root, transform, false, 0, [1.; 2], &mut mesh, &mut None)?;
        let mut faces: Vec<_> = mesh
            .vertices
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|q| q[0].normal[2] > 0.)
            .collect();
        faces.sort_by(|a, b| {
            a.iter()
                .map(|v| v.position[2])
                .sum::<f32>()
                .total_cmp(&b.iter().map(|v| v.position[2]).sum::<f32>())
        });
        let mut out = RgbaImage::new(64, 64);
        for q in faces {
            let normal = Vec3::from_array(q[0].normal);
            let light =
                (0.4 + 0.6 * normal.dot(Vec3::new(-0.3, 0.8, 0.6).normalize()).max(0.)).min(1.);
            for tri in [[0, 1, 2], [0, 2, 3]] {
                triangle(
                    &mut out,
                    tri.map(|i| Vec3::from_array(q[i].position)),
                    tri.map(|i| Vec2::from_array(q[i].uv)),
                    &texture,
                    [light; 3],
                );
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod skull_tests {
    use super::*;
    #[test]
    #[ignore = "Requires imported original skull PNGs; CPU only, no network"]
    fn official_special_skulls_and_cached_profile_skins_have_distinct_icons() {
        use sha1::{Digest, Sha1};
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut baker = Baker::new(&cache);
        let report = super::super::builtins::get().unwrap();
        for name in [
            "player_head",
            "creeper_head",
            "zombie_head",
            "skeleton_skull",
            "wither_skeleton_skull",
            "piglin_head",
            "dragon_head",
        ] {
            let id = (0..2000)
                .find(|&id| {
                    report.name("minecraft:item", id) == Some(format!("minecraft:{name}").as_str())
                })
                .unwrap();
            let pixels = baker.bake(id).unwrap_or_else(|e| panic!("{name}: {e:#}"));
            assert!(pixels.pixels().filter(|p| p[3] > 0).count() > 80, "{name}");
            assert!(!pixels.pixels().any(|p| p.0 == [255, 0, 255, 255]));
            pixels.save(cache.join(format!("icon-{name}.png"))).unwrap();
        }
        let root = std::env::temp_dir().join(format!("optimc-head-cache-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        baker.head_cache = root.clone();
        let id = (0..2000)
            .find(|&id| report.name("minecraft:item", id) == Some("minecraft:player_head"))
            .unwrap();
        let mut icons = Vec::new();
        for (path, color) in [("red", [230, 20, 20, 255]), ("green", [20, 230, 20, 255])] {
            let url = format!("https://textures.minecraft.net/texture/fixture-{path}");
            RgbaImage::from_pixel(64, 64, Rgba(color))
                .save(root.join(format!("{:x}.png", Sha1::digest(url.as_bytes()))))
                .unwrap();
            let key = Key {
                item: id,
                model: None,
                skin: Some(url),
            };
            icons.push(baker.bake_key(&key).unwrap());
        }
        assert_ne!(icons[0], icons[1]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
