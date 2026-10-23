//! Official baked ModelPart data exported at development time. Special renderers
//! and state-dependent layers/animations still need their own runtime paths.
use crate::{
    assets::{builtins, models::ResourceId},
    world::Mesh,
};
use anyhow::{ensure, Context, Result};
use flate2::read::GzDecoder;
use glam::{Mat4, Vec3};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{collections::BTreeMap, io::Read, path::Path, sync::OnceLock};
#[derive(Clone, Deserialize)]
pub struct Part {
    pub transform: [f32; 9],
    pub visible: bool,
    pub skip_draw: bool,
    pub quads: Vec<[[f32; 5]; 4]>,
    pub children: BTreeMap<String, Part>,
}
#[derive(Deserialize)]
pub struct Definition {
    pub width: f32,
    pub height: f32,
    pub root: Option<Part>,
    pub texture: Option<String>,
    pub scale: Option<[f32; 16]>,
}
#[derive(Deserialize)]
pub struct Report {
    pub layers: BTreeMap<String, Part>,
    pub entities: BTreeMap<String, Definition>,
}
pub struct Geometry {
    pub kind: u32,
    pub mesh: Mesh,
    pub head_pivot: Option<Vec3>,
    pub humanoid: Option<super::humanoid_attack::Skeleton>,
}
pub struct Compiled {
    pub models: Vec<Geometry>,
    pub pixels: Vec<u8>,
    pub texture_count: u32,
}
fn check(part: &Part, depth: usize) -> Result<()> {
    ensure!(
        depth <= 64
            && part
                .transform
                .iter()
                .all(|v| v.is_finite() && v.abs() < 10000.),
        "Invalid entity part transform"
    );
    ensure!(
        part.quads
            .iter()
            .flatten()
            .flatten()
            .all(|v| v.is_finite() && v.abs() < 10000.),
        "Invalid entity quad"
    );
    for child in part.children.values() {
        check(child, depth + 1)?;
    }
    Ok(())
}
pub fn get() -> Result<&'static Report> {
    static DATA: OnceLock<Result<Report, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        (|| -> Result<Report> {
            let mut bytes = Vec::new();
            GzDecoder::new(include_bytes!("../../data/26.2/entities.json.gz").as_slice())
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 16 * 1024 * 1024,
                "Entity model report too large"
            );
            let reference: serde_json::Value =
                serde_json::from_str(include_str!("../../data/26.2/reference.json"))?;
            ensure!(
                format!("{:x}", Sha1::digest(&bytes))
                    == reference["sha1"]["entities.json"]
                        .as_str()
                        .context("Missing entity model hash")?,
                "Entity report hash mismatch"
            );
            let report: Report = serde_json::from_slice(&bytes)?;
            ensure!(report.entities.len() == 158, "Wrong entity catalog size");
            for part in report.layers.values() {
                check(part, 0)?;
            }
            for definition in report.entities.values() {
                ensure!(
                    definition.width.is_finite()
                        && definition.height.is_finite()
                        && definition.width >= 0.
                        && definition.height >= 0.,
                    "Invalid entity dimensions"
                );
                if let Some(part) = &definition.root {
                    check(part, 0)?;
                }
            }
            Ok(report)
        })()
        .map_err(|e| format!("{e:#}"))
    })
    .as_ref()
    .map_err(|e| anyhow::anyhow!(e.clone()))
}
impl Report {
    pub fn compile(&self, cache: &Path) -> Result<Compiled> {
        self.compile_selected(cache, None, None)
    }
    /// Compile only a model that is present in the live world, keeping texture
    /// uploads independent of the size of the full entity catalog.
    pub fn compile_kind(&self, cache: &Path, kind: u32) -> Result<Compiled> {
        self.compile_selected(cache, Some(kind), None)
    }
    pub fn compile_armor_stand(
        &self,
        cache: &Path,
        kind: u32,
        pose: &crate::net::metadata::ArmorStand,
    ) -> Result<Compiled> {
        self.compile_selected(cache, Some(kind), Some(pose))
    }
    fn compile_selected(
        &self,
        cache: &Path,
        selected: Option<u32>,
        armor: Option<&crate::net::metadata::ArmorStand>,
    ) -> Result<Compiled> {
        let selected_name =
            selected.and_then(|kind| builtins::get().ok()?.name("minecraft:entity_type", kind));
        let textures: BTreeMap<_, _> = self
            .entities
            .iter()
            .filter(|(name, _)| selected.is_none() || selected_name == Some(name.as_str()))
            .filter_map(|(_, e)| e.texture.as_ref())
            .map(|t| (t.clone(), ()))
            .collect();
        let mut sprites = BTreeMap::new();
        let mut pixels = Vec::new();
        for (layer, id) in textures.keys().enumerate() {
            let id_parsed = ResourceId::parse(id)?;
            let path = cache
                .join("26.2/resources/assets")
                .join(id_parsed.namespace)
                .join(id_parsed.path);
            let image = image::open(&path)
                .with_context(|| format!("Entity texture {}", path.display()))?
                .to_rgba8();
            let (w, h) = image.dimensions();
            ensure!(
                w > 0 && h > 0 && w <= 256 && h <= 256,
                "Entity texture exceeds 256x256"
            );
            let blended = image.pixels().any(|p| p[3] != 0 && p[3] != 255);
            // Pad instead of resampling: preserve every original texel, including
            // non-square and 192-pixel textures; UVs address only their own area.
            let padded = image::RgbaImage::from_fn(256, 256, |x, y| {
                *image.get_pixel(x.min(w - 1), y.min(h - 1))
            });
            pixels.extend_from_slice(padded.as_raw());
            sprites.insert(
                id,
                (layer as u32, [w as f32 / 256., h as f32 / 256.], blended),
            );
        }
        let mut models = Vec::new();
        for kind in 0..158 {
            if selected.is_some_and(|wanted| wanted != kind) {
                continue;
            }
            let name = builtins::get()?
                .name("minecraft:entity_type", kind)
                .context("Missing entity ID")?;
            let definition = &self.entities[name];
            let (Some(root), Some(texture)) = (&definition.root, &definition.texture) else {
                continue;
            };
            let posed;
            let root = if let Some(pose) = armor {
                posed = armor_root(pose)?;
                &posed
            } else {
                root
            };
            let (layer, uv, blended) = sprites[texture];
            let scale =
                Mat4::from_cols_array(&definition.scale.unwrap_or(Mat4::IDENTITY.to_cols_array()));
            ensure!(scale.is_finite(), "Invalid renderer scale");
            let base = Mat4::from_scale(Vec3::new(-1., -1., 1.))
                * scale
                * Mat4::from_translation(Vec3::new(0., -1.501, 0.));
            let mut mesh = Mesh::new(Vec3::ZERO, Vec3::ZERO, 0);
            mesh.translucent = blended;
            let mut head_pivot = None;
            bake(
                root,
                base,
                false,
                layer | if blended { 0x80000000 } else { 0 },
                uv,
                &mut mesh,
                &mut head_pivot,
            )?;
            let humanoid = if armor.is_none()
                && root.visible
                && root.quads.is_empty()
                && super::humanoid_attack::NAMES
                    .iter()
                    .all(|name| root.children.contains_key(*name))
            {
                let root_matrix = base * super::humanoid_attack::matrix(root.transform);
                let parts = super::humanoid_attack::NAMES.map(|name| root.children[name].transform);
                // Re-bake once with part IDs, preserving the exact neutral mesh.
                mesh.vertices.clear();
                mesh.indices.clear();
                head_pivot = None;
                for (name, child) in &root.children {
                    let start = mesh.vertices.len();
                    bake(
                        child,
                        root_matrix,
                        name == "head",
                        layer | if blended { 0x80000000 } else { 0 },
                        uv,
                        &mut mesh,
                        &mut head_pivot,
                    )?;
                    if let Some(index) = super::humanoid_attack::NAMES
                        .iter()
                        .position(|n| *n == name)
                    {
                        for vertex in &mut mesh.vertices[start..] {
                            vertex.lod = index as u32 + 1;
                        }
                    }
                }
                // bake(child) does not visit the head through its parent.
                head_pivot = Some(root_matrix.transform_point3(
                    Vec3::from_array([parts[0][0], parts[0][1], parts[0][2]]) / 16.,
                ));
                Some(super::humanoid_attack::Skeleton {
                    root: root_matrix,
                    parts,
                })
            } else {
                None
            };
            mesh.tighten_bounds();
            models.push(Geometry {
                kind,
                mesh,
                humanoid,
                head_pivot: if armor.is_some() { None } else { head_pivot },
            });
        }
        Ok(Compiled {
            models,
            pixels,
            texture_count: textures.len() as u32,
        })
    }
}
fn armor_root(pose: &crate::net::metadata::ArmorStand) -> Result<Part> {
    let reference = super::signs::reference()?;
    let mut root: Part = serde_json::from_value(
        reference[if pose.flags & 1 != 0 {
            "armor_small"
        } else {
            "armor_big"
        }]
        .clone(),
    )?;
    for (name, part) in &mut root.children {
        let index = match name.as_str() {
            "head" => Some(0),
            "body" | "left_body_stick" | "right_body_stick" | "shoulder_stick" => Some(1),
            "left_arm" => Some(2),
            "right_arm" => Some(3),
            "left_leg" => Some(4),
            "right_leg" => Some(5),
            _ => None,
        };
        if let Some(index) = index {
            for axis in 0..3 {
                part.transform[3 + axis] = pose.poses[index][axis] * 0.017453292;
            }
        }
        if name == "left_arm" || name == "right_arm" {
            part.visible = pose.flags & 4 != 0;
        }
        if name == "base_plate" {
            part.visible = pose.flags & 8 == 0;
            part.transform[4] = -pose.yaw * 0.017453292;
        }
    }
    if let Some(hat) = root
        .children
        .get_mut("head")
        .and_then(|head| head.children.get_mut("hat"))
    {
        hat.visible = false;
    }
    Ok(root)
}
pub(crate) fn bake(
    part: &Part,
    parent: Mat4,
    head: bool,
    layer: u32,
    uv_scale: [f32; 2],
    mesh: &mut Mesh,
    pivot: &mut Option<Vec3>,
) -> Result<()> {
    if !part.visible {
        return Ok(());
    }
    let t = part.transform;
    let matrix = parent
        * Mat4::from_translation(Vec3::new(t[0], t[1], t[2]) / 16.)
        * Mat4::from_rotation_z(t[5])
        * Mat4::from_rotation_y(t[4])
        * Mat4::from_rotation_x(t[3])
        * Mat4::from_scale(Vec3::new(t[6], t[7], t[8]));
    if !part.skip_draw {
        for quad in &part.quads {
            let points = quad.map(|p| {
                matrix
                    .transform_point3(Vec3::new(p[0], p[1], p[2]) / 16.)
                    .to_array()
            });
            let uv = quad.map(|p| [p[3] * uv_scale[0], p[4] * uv_scale[1]]);
            mesh.quad_uv(points, uv, layer, [1.; 3]);
            let len = mesh.vertices.len();
            for vertex in &mut mesh.vertices[len - 4..] {
                vertex.lod = head as u32;
            }
        }
    }
    for (name, child) in &part.children {
        let is_head = name == "head" && pivot.is_none();
        if is_head {
            let c = child.transform;
            *pivot = Some(matrix.transform_point3(Vec3::new(c[0], c[1], c[2]) / 16.));
        }
        bake(child, matrix, head || is_head, layer, uv_scale, mesh, pivot)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_entity_catalog_contains_layers_dimensions_and_base_living_models() {
        let report = get().unwrap();
        assert_eq!(report.layers.len(), 358);
        assert_eq!(report.entities.len(), 158);
        assert_eq!(
            report
                .entities
                .values()
                .filter(|e| e.root.is_some() && e.texture.is_some())
                .count(),
            90
        );
        assert_eq!(report.entities["minecraft:creeper"].width, 0.6);
        assert!(report.entities["minecraft:player"].root.is_none());
    }
    #[test]
    #[ignore = "Requires imported official entity PNG resources"]
    fn all_base_living_geometry_uses_original_texels_and_valid_outward_triangles() {
        let compiled = get()
            .unwrap()
            .compile(&Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache"))
            .unwrap();
        assert_eq!(compiled.models.len(), 90);
        assert_eq!(compiled.texture_count, 87);
        for geometry in &compiled.models {
            assert!(
                !geometry.mesh.indices.is_empty(),
                "Empty entity kind {}",
                geometry.kind
            );
            assert!(geometry.mesh.vertices.iter().all(|v| v
                .position
                .into_iter()
                .all(f32::is_finite)
                && v.uv.into_iter().all(f32::is_finite)));
        }
        let creeper = compiled.models.iter().find(|m| m.kind == 32).unwrap();
        assert!(creeper.head_pivot.is_some());
        assert!(creeper.mesh.vertices.iter().any(|v| v.lod == 1));
        assert!(
            creeper.mesh.max.y > 1.5 && creeper.mesh.max.y < 1.8,
            "{:?}",
            creeper.mesh.max
        );
    }
}

#[cfg(test)]
mod armor_tests {
    use super::*;
    #[test]
    fn armor_flags_small_layer_and_all_part_rotations_match_original_setup_anim() {
        let reference = crate::assets::signs::reference().unwrap();
        for sample in reference["armor_samples"].as_array().unwrap() {
            let pose = crate::net::metadata::ArmorStand {
                flags: sample["flags"].as_u64().unwrap() as u8,
                yaw: 37.,
                poses: [
                    [15., 25., -5.],
                    [10., -20., 30.],
                    [-45., 12., 8.],
                    [30., 15., -20.],
                    [9., 3., -5.],
                    [-15., 8., 17.],
                ],
            };
            let actual = armor_root(&pose).unwrap();
            let expected: Part = serde_json::from_value(sample["root"].clone()).unwrap();
            fn compare(a: &Part, b: &Part) {
                assert_eq!(a.visible, b.visible);
                assert_eq!(a.children.len(), b.children.len());
                for (x, y) in a.transform.iter().zip(b.transform) {
                    assert!((x - y).abs() < 0.000001, "{x} != {y}");
                }
                for (name, part) in &a.children {
                    compare(part, &b.children[name]);
                }
            }
            compare(&actual, &expected);
        }
    }
}
