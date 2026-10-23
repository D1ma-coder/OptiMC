//! Server sign text, original bitmap glyphs and sign renderer transformations.
use super::*;
use crate::{
    net::{block_entity::BlockEntity, nbt::Tag},
    text::{RichText, StyledChar},
};
use glam::Mat4;
#[derive(Clone)]
struct Glyph {
    page: usize,
    rect: [u32; 4],
    factor: f32,
    advance: f32,
    offset: f32,
}
#[derive(Default)]
pub(super) struct Cache {
    font: Option<Font>,
    tiles: HashMap<(usize, u32, u32), u32>,
    meshes: HashMap<[i32; 3], (Arc<BlockEntity>, u32, Arc<Mesh>)>,
}
struct Font {
    pages: Vec<image::RgbaImage>,
    glyphs: HashMap<char, Glyph>,
}
impl Font {
    fn load(cache: &Path) -> Result<Self> {
        let dir = crate::assets::root(cache).join("gui");
        let definition: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("font.json"))?)?;
        let mut font = Self {
            pages: vec![],
            glyphs: HashMap::new(),
        };
        for provider in definition["providers"]
            .as_array()
            .context("Missing bitmap providers")?
        {
            if provider["type"] != "bitmap" {
                continue;
            }
            let file = provider["file"]
                .as_str()
                .context("Missing font page")?
                .rsplit('/')
                .next()
                .unwrap();
            let image = image::open(dir.join(file))?.to_rgba8();
            let rows = provider["chars"].as_array().context("Missing glyph rows")?;
            let columns = rows[0]
                .as_str()
                .context("Invalid glyph row")?
                .chars()
                .count() as u32;
            ensure!(columns > 0 && !rows.is_empty(), "Empty font page");
            let cw = image.width() / columns;
            let ch = image.height() / rows.len() as u32;
            ensure!(cw > 0 && ch > 0, "Empty glyph cell");
            let factor = provider["height"].as_f64().unwrap_or(8.) as f32 / ch as f32;
            for (y, row) in rows.iter().enumerate() {
                for (x, c) in row
                    .as_str()
                    .context("Invalid glyph row")?
                    .chars()
                    .enumerate()
                {
                    if c == '\0' || c == ' ' {
                        continue;
                    }
                    let mut ink = 0;
                    for gx in 0..cw {
                        if (0..ch).any(|gy| {
                            image.get_pixel(x as u32 * cw + gx, y as u32 * ch + gy)[3] != 0
                        }) {
                            ink = gx + 1;
                        }
                    }
                    if ink == 0 {
                        continue;
                    }
                    font.glyphs.entry(c).or_insert(Glyph {
                        page: font.pages.len(),
                        rect: [x as u32 * cw, y as u32 * ch, ink, ch],
                        factor,
                        advance: (ink as f32 * factor + 1.).floor(),
                        offset: 7. - provider["ascent"].as_f64().unwrap_or(7.) as f32,
                    });
                }
            }
            font.pages.push(image);
        }
        Ok(font)
    }
    fn advance(&self, c: StyledChar) -> f32 {
        (if c.ch == ' ' {
            4.
        } else {
            self.glyphs.get(&c.ch).map_or(6., |g| g.advance)
        }) + if c.style.bold { 1. } else { 0. }
    }
}
fn angle(state: &crate::assets::builtins::BlockState) -> f32 {
    if let Some(rotation) = state
        .properties
        .get("rotation")
        .and_then(|s| s.parse::<f32>().ok())
    {
        rotation * 22.5
    } else {
        match state.properties.get("facing").map(String::as_str) {
            Some("south") => 0.,
            Some("west") => 90.,
            Some("north") => 180.,
            _ => 270.,
        }
    }
}
fn text_matrix(state: &crate::assets::builtins::BlockState, front: bool) -> Mat4 {
    let hanging = state.block.contains("hanging_sign");
    let mut matrix =
        Mat4::from_translation(Vec3::new(0.5, if hanging { 0.9375 } else { 0.5 }, 0.5))
            * Mat4::from_rotation_y(-angle(state).to_radians());
    if hanging {
        matrix *= Mat4::from_translation(Vec3::new(0., -0.3125, 0.));
    } else if state.block.contains("wall_sign") {
        matrix *= Mat4::from_translation(Vec3::new(0., -0.3125, -0.4375));
    }
    if !front {
        matrix *= Mat4::from_rotation_y(std::f32::consts::PI);
    }
    let (offset, scale) = if hanging {
        (Vec3::new(0., -0.32, 0.073), 0.0140625)
    } else {
        (Vec3::new(0., 0.33333334, 0.046666667), 0.010416667)
    };
    matrix * Mat4::from_translation(offset) * Mat4::from_scale(Vec3::new(scale, -scale, scale))
}
fn dye(name: &str) -> Result<u32> {
    let reference = crate::assets::signs::reference()?;
    Ok(reference["dyes"][name].as_i64().unwrap_or(-16777216) as u32 & 0xffffff)
}
impl RemoteMesher {
    fn sign_glyph(
        &mut self,
        mesh: &mut Mesh,
        glyph: &Glyph,
        at: [f32; 2],
        matrix: Mat4,
        color: [f32; 3],
        italic: bool,
    ) -> Result<()> {
        let [sx, sy, w, h] = glyph.rect;
        for ty in sy / 64..=(sy + h - 1) / 64 {
            for tx in sx / 64..=(sx + w - 1) / 64 {
                let key = (glyph.page, tx, ty);
                let layer = if let Some(&layer) = self.signs.tiles.get(&key) {
                    layer
                } else {
                    let page = &self.signs.font.as_ref().unwrap().pages[glyph.page];
                    let layer = 7 + (self.pixels.len() / 16384) as u32;
                    for y in 0..64 {
                        for x in 0..64 {
                            self.pixels.extend_from_slice(&if tx * 64 + x < page.width()
                                && ty * 64 + y < page.height()
                            {
                                page.get_pixel(tx * 64 + x, ty * 64 + y).0
                            } else {
                                [0; 4]
                            });
                        }
                    }
                    self.texture_versions.push(MAP_TILE_VERSION_FLAG | 1);
                    self.signs.tiles.insert(key, layer);
                    layer
                };
                let x0 = sx.max(tx * 64);
                let x1 = (sx + w).min((tx + 1) * 64);
                let y0 = sy.max(ty * 64);
                let y1 = (sy + h).min((ty + 1) * 64);
                let xy = [[x0, y1], [x1, y1], [x1, y0], [x0, y0]];
                let points = xy.map(|[x, y]| {
                    let gy = (y - sy) as f32 * glyph.factor;
                    let skew = if italic { 1. - gy * 0.25 } else { 0. };
                    matrix
                        .transform_point3(Vec3::new(
                            at[0] + (x - sx) as f32 * glyph.factor + skew,
                            at[1] + gy,
                            0.,
                        ))
                        .to_array()
                });
                mesh.quad_uv(
                    points,
                    xy.map(|[x, y]| [(x - tx * 64) as f32 / 64., (y - ty * 64) as f32 / 64.]),
                    layer,
                    color,
                );
            }
        }
        Ok(())
    }
    pub fn sign_meshes(&mut self, world: &RemoteWorld) -> Result<Vec<Arc<Mesh>>> {
        let mut result = Vec::new();
        let old = std::mem::take(&mut self.signs.meshes);
        let builtin = builtins::get()?;
        for (position, entity) in &world.block_entities {
            if !matches!(
                builtin.name("minecraft:block_entity_type", entity.kind),
                Some("minecraft:sign" | "minecraft:hanging_sign")
            ) {
                continue;
            }
            let Some(state_id) = world.block(position[0], position[1], position[2]) else {
                continue;
            };
            let Some(state) = builtin
                .state(state_id)
                .filter(|s| s.block.ends_with("_sign"))
            else {
                continue;
            };
            if let Some((previous, id, mesh)) = old
                .get(position)
                .filter(|(previous, id, _)| Arc::ptr_eq(previous, entity) && *id == state_id)
            {
                self.signs
                    .meshes
                    .insert(*position, (previous.clone(), *id, mesh.clone()));
                result.push(mesh.clone());
                continue;
            }
            if self.signs.font.is_none() {
                self.signs.font = Some(Font::load(&self.cache_path)?);
            }
            let origin = Vec3::new(position[0] as f32, position[1] as f32, position[2] as f32);
            let low = Vec3::from_array(std::array::from_fn(|i| {
                (position[i] as f64 - origin[i] as f64) as f32
            }));
            let mut mesh = Mesh::new(origin, origin + Vec3::ONE, 0);
            for (front, key) in [(true, "front_text"), (false, "back_text")] {
                let Some(side) = entity.data.get(key) else {
                    continue;
                };
                let Some(Tag::List { values, .. }) = side.get("messages") else {
                    continue;
                };
                let glowing = side
                    .get("has_glowing_text")
                    .and_then(Tag::integer)
                    .unwrap_or(0)
                    != 0;
                let rgb = dye(&side
                    .get("color")
                    .and_then(Tag::string)
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_else(|| "black".into()))?;
                let base = [
                    (rgb >> 16 & 255) as f32 / 255.,
                    (rgb >> 8 & 255) as f32 / 255.,
                    (rgb & 255) as f32 / 255.,
                ]
                .map(|c| {
                    if glowing {
                        c
                    } else {
                        (c * 255. * 0.4).floor() / 255.
                    }
                });
                let hanging = state.block.contains("hanging_sign");
                let height = if hanging { 9. } else { 10. };
                let width = if hanging { 60. } else { 90. };
                let matrix = Mat4::from_translation(low) * text_matrix(state, front);
                for (row, message) in values.iter().take(4).enumerate() {
                    let text = RichText::from_json(&crate::net::component::json_tag(message))?;
                    let lines = text.lines(width, |c| self.signs.font.as_ref().unwrap().advance(c));
                    let Some(line) = lines.first() else {
                        continue;
                    };
                    let mut x = -(line
                        .iter()
                        .map(|c| self.signs.font.as_ref().unwrap().advance(*c))
                        .sum::<f32>()
                        .ceil() as i32
                        / 2) as f32;
                    for c in line {
                        let font = self.signs.font.as_ref().unwrap();
                        let advance = font.advance(*c);
                        let glyph = font.glyphs.get(&c.ch).cloned();
                        if let Some(glyph) = glyph {
                            let color = c
                                .style
                                .color
                                .map(|rgb| {
                                    [
                                        (rgb >> 16 & 255) as f32 / 255.,
                                        (rgb >> 8 & 255) as f32 / 255.,
                                        (rgb & 255) as f32 / 255.,
                                    ]
                                })
                                .unwrap_or(base);
                            let y = row as f32 * height - 2. * height + glyph.offset;
                            self.sign_glyph(
                                &mut mesh,
                                &glyph,
                                [x, y],
                                matrix,
                                color,
                                c.style.italic,
                            )?;
                            if c.style.bold {
                                self.sign_glyph(
                                    &mut mesh,
                                    &glyph,
                                    [x + 1., y],
                                    matrix,
                                    color,
                                    c.style.italic,
                                )?;
                            }
                        }
                        x += advance;
                    }
                }
            }
            for vertex in &mut mesh.vertices {
                vertex.origin = origin.to_array();
                vertex.lod = 1024 | 256;
            }
            mesh.tighten_bounds();
            let mesh = Arc::new(mesh);
            result.push(mesh.clone());
            self.signs
                .meshes
                .insert(*position, (entity.clone(), state_id, mesh));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_original_sign_text_transform_and_dye_color_matches() {
        let r = crate::assets::signs::reference().unwrap();
        for row in r["matrices"].as_array().unwrap() {
            let kind = row["kind"].as_str().unwrap();
            let mut properties = std::collections::BTreeMap::new();
            properties.insert(
                "rotation".into(),
                ((row["angle"].as_f64().unwrap() / 22.5) as u32).to_string(),
            );
            let state = crate::assets::builtins::BlockState {
                block: match kind {
                    "WALL" => "minecraft:oak_wall_sign",
                    "HANGING" => "minecraft:oak_hanging_sign",
                    _ => "minecraft:oak_sign",
                }
                .into(),
                properties,
                is_default: false,
            };
            let actual = text_matrix(&state, row["front"].as_bool().unwrap()).to_cols_array();
            for (i, expected) in row["matrix"].as_array().unwrap().iter().enumerate() {
                assert!(
                    (actual[i] - expected.as_f64().unwrap() as f32).abs() < 0.000001,
                    "{kind}: component {i}"
                );
            }
        }
        assert_eq!(dye("black").unwrap(), 0);
        assert_eq!(dye("red").unwrap(), 0xff0000);
        assert_eq!(dye("white").unwrap(), 0xffffff);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use base64::Engine;
    #[test]
    #[ignore = "Requires imported official bitmap font; CPU only"]
    fn server_sign_text_builds_both_faces_and_reuses_then_removes_meshes() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(
                crate::assets::signs::reference().unwrap()["update"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
        let entity = crate::net::block_entity::update(&bytes).unwrap();
        let state = builtins::get()
            .unwrap()
            .default_state("minecraft:oak_sign")
            .unwrap();
        let mut blocks = vec![0; 4096];
        blocks[(10 * 16 + 15) * 16 + 1] = state;
        let mut world = RemoteWorld {
            dimension: Some(crate::net::registry::Dimension {
                name: "minecraft:overworld".into(),
                min_y: -32,
                height: 16,
                logical_height: 16,
                skylight: true,
                ceiling: false,
            }),
            ..Default::default()
        };
        world
            .insert(crate::net::chunk::Chunk {
                x: -2,
                z: 2,
                sections: vec![crate::net::chunk::Section {
                    non_air: 1,
                    fluids: 0,
                    blocks,
                    biomes: vec![0; 64],
                }],
                block_entities: vec![entity],
            })
            .unwrap();
        let mut mesher = RemoteMesher::new(&cache);
        let meshes = mesher.sign_meshes(&world).unwrap();
        assert_eq!(meshes.len(), 1);
        let mesh = &meshes[0];
        assert!(!mesh.vertices.is_empty());
        assert!(mesh.vertices.iter().any(|v| v.normal[2] > 0.9));
        assert!(mesh.vertices.iter().any(|v| v.normal[2] < -0.9));
        assert!(mesh
            .vertices
            .iter()
            .all(|v| v.lod & 1024 != 0 && v.origin == [-31., -22., 47.]));
        let bytes = mesher.pixels.len();
        let again = mesher.sign_meshes(&world).unwrap();
        assert!(Arc::ptr_eq(&again[0], mesh));
        assert_eq!(mesher.pixels.len(), bytes);
        world.set_block(-31, -22, 47, 0);
        assert!(mesher.sign_meshes(&world).unwrap().is_empty());
    }
}
