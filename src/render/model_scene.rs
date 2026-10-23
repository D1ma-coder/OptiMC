//! Visual QA and geometry path for the official block-model pipeline.
use crate::{
    assets::models::{Catalog, Properties, ResourceId, Variant},
    world::Mesh,
};
use anyhow::{ensure, Context, Result};
use glam::Vec3;
use std::{collections::BTreeMap, path::Path, sync::Arc};
#[derive(Clone)]
pub struct Preview {
    pub id: String,
    pub properties: Properties,
    pub direct: bool,
}
pub struct ModelScene {
    pub meshes: Vec<Arc<Mesh>>,
    pub texture_pixels: Vec<u8>,
}
impl ModelScene {
    pub fn from_remote(cache: &Path, world: &crate::world::remote::RemoteWorld) -> Result<Self> {
        let mut mesher = crate::world::remote_mesh::RemoteMesher::new(cache);
        let (meshes, stats) = mesher.update(world)?;
        ensure!(
            !meshes.is_empty(),
            "Received world has no visible block-model geometry"
        );
        println!(
            "Server snapshot: {} sections, {} quads",
            stats.rebuilt_sections, stats.quads
        );
        Ok(Self {
            meshes,
            texture_pixels: mesher.texture_pixels().to_vec(),
        })
    }
    pub fn load(cache: &Path, request: &Preview) -> Result<Self> {
        let mut catalog = Catalog::new(cache);
        let quads = if request.direct {
            catalog
                .bake(&Variant {
                    model: request.id.clone(),
                    x: 0,
                    y: 0,
                    uvlock: false,
                    weight: 1,
                })?
                .to_vec()
        } else {
            let id = ResourceId::parse(&request.id)?;
            let state = catalog.blockstate(&id)?;
            let variants = state.selected(&request.properties, 0)?;
            ensure!(
                !variants.is_empty(),
                "No matching blockstate for {} {:?}; supply --properties",
                id,
                request.properties
            );
            let mut quads = Vec::new();
            for variant in variants {
                quads.extend(catalog.bake(variant)?.iter().cloned());
            }
            quads
        };
        ensure!(
            !quads.is_empty(),
            "Model has no cuboid geometry (may use an entity/special renderer)"
        );
        let mut textures = BTreeMap::new();
        let mut pixels = Vec::new();
        let mut meshes = Vec::new();
        let mut solid = Mesh::new(Vec3::ZERO, Vec3::ONE, 0);
        for quad in quads {
            let id = &quad.material.sprite;
            if !textures.contains_key(id) {
                let image = if id.path == "missingno" {
                    image::RgbaImage::from_fn(16, 16, |x, y| {
                        image::Rgba(if (x < 8) == (y < 8) {
                            [0, 0, 0, 255]
                        } else {
                            [255, 0, 255, 255]
                        })
                    })
                } else {
                    image::open(catalog.texture_file(id))
                        .with_context(|| format!("Loading {id}"))?
                        .to_rgba8()
                };
                // Animation decoding is separate. Preview uses the first frame;
                // each image occupies one independent array layer, with no atlas bleed.
                let frame = image.width().min(image.height());
                let image = image::imageops::crop_imm(&image, 0, 0, frame, frame).to_image();
                let alpha = image.pixels().any(|p| p[3] != 0 && p[3] != 255);
                let layer = 7 + textures.len() as u32;
                textures.insert(id.clone(), (layer, alpha));
                pixels.extend_from_slice(
                    image::imageops::resize(&image, 64, 64, image::imageops::FilterType::Nearest)
                        .as_raw(),
                );
            }
            let (layer, alpha) = textures[id];
            let translucent = quad.material.force_translucent || alpha;
            let tint = if quad.tint_index >= 0 {
                [0.46, 0.77, 0.27]
            } else {
                [1.; 3]
            };
            let shade = crate::world::classic_shade(quad.normal, quad.shade);
            if translucent {
                let mut mesh = Mesh::new(Vec3::ZERO, Vec3::ONE, 0);
                mesh.translucent = true;
                mesh.quad_uv_shaded(
                    quad.points.map(|v| v.to_array()),
                    quad.uv.map(|v| v.to_array()),
                    layer | 0x8000_0000,
                    tint,
                    shade,
                );
                mesh.tighten_bounds();
                meshes.push(Arc::new(mesh));
            } else {
                solid.quad_uv_shaded(
                    quad.points.map(|v| v.to_array()),
                    quad.uv.map(|v| v.to_array()),
                    layer,
                    tint,
                    shade,
                );
            }
        }
        if !solid.indices.is_empty() {
            solid.tighten_bounds();
            meshes.insert(0, Arc::new(solid));
        }
        println!(
            "Official model: {} meshes, {} quads, {} textures",
            meshes.len(),
            meshes.iter().map(|m| m.indices.len() / 6).sum::<usize>(),
            textures.len()
        );
        Ok(Self {
            meshes,
            texture_pixels: pixels,
        })
    }
}
