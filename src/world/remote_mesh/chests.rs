//! Official ChestModel layers and PNGs, including double/trapped/copper/ender.
use super::*;
use crate::net::{block_event, SessionReport};
use glam::Mat4;
#[derive(Default)]
pub(super) struct Cache {
    textures: HashMap<String, u32>,
    meshes: HashMap<[i32; 3], (u32, u32, Arc<Mesh>)>,
}
pub(crate) fn matrix(facing: &str) -> Mat4 {
    let yaw: f32 = match facing {
        "south" => 0.,
        "west" => 90.,
        "north" => 180.,
        "east" => 270.,
        _ => 0.,
    };
    Mat4::from_translation(Vec3::new(0.5, 0., 0.5))
        * Mat4::from_rotation_y(-yaw.to_radians())
        * Mat4::from_translation(Vec3::new(-0.5, 0., -0.5))
}
fn texture(name: &str) -> &str {
    match name
        .trim_start_matches("minecraft:")
        .trim_start_matches("waxed_")
    {
        "trapped_chest" => "trapped",
        "ender_chest" => "ender",
        "copper_chest" => "copper",
        "exposed_copper_chest" => "copper_exposed",
        "weathered_copper_chest" => "copper_weathered",
        "oxidized_copper_chest" => "copper_oxidized",
        _ => "normal",
    }
}
fn openness(report: &SessionReport, p: [i32; 3], now: std::time::Instant) -> f32 {
    report
        .block_events
        .lids
        .get(&p)
        .map_or(0., |lid| lid.openness(now))
}
impl RemoteMesher {
    pub fn chest_meshes(&mut self, report: &SessionReport) -> Result<Vec<Arc<Mesh>>> {
        self.chest_meshes_at(report, std::time::Instant::now())
    }
    fn chest_meshes_at(
        &mut self,
        report: &SessionReport,
        now: std::time::Instant,
    ) -> Result<Vec<Arc<Mesh>>> {
        let builtin = builtins::get()?;
        let old = std::mem::take(&mut self.chests.meshes);
        let mut result = Vec::new();
        for position in report.world.block_entities.keys() {
            let Some(id) = report.world.block(position[0], position[1], position[2]) else {
                continue;
            };
            let Some(state) = builtin.state(id).filter(|s| block_event::chest(&s.block)) else {
                continue;
            };
            let facing = state
                .properties
                .get("facing")
                .map_or("north", String::as_str);
            let half = state
                .properties
                .get("type")
                .map_or("single", String::as_str);
            let mut open = openness(report, *position, now);
            if half != "single" {
                let (x, z) = match facing {
                    "north" => (1, 0),
                    "south" => (-1, 0),
                    "west" => (0, -1),
                    "east" => (0, 1),
                    _ => (0, 0),
                };
                let sign = if half == "left" { 1 } else { -1 };
                let neighbour = [position[0] + x * sign, position[1], position[2] + z * sign];
                if report
                    .world
                    .block(neighbour[0], neighbour[1], neighbour[2])
                    .and_then(|id| builtin.state(id))
                    .is_some_and(|other| {
                        other.block == state.block
                            && other.properties.get("facing") == state.properties.get("facing")
                            && other.properties.get("type").is_some_and(|value| {
                                value == if half == "left" { "right" } else { "left" }
                            })
                    })
                {
                    open = open.max(openness(report, neighbour, now));
                }
            }
            if let Some((_, _, mesh)) = old
                .get(position)
                .filter(|(old_id, old_open, _)| *old_id == id && *old_open == open.to_bits())
            {
                self.chests
                    .meshes
                    .insert(*position, (id, open.to_bits(), mesh.clone()));
                result.push(mesh.clone());
                continue;
            }
            let half = match half {
                "left" => "left",
                "right" => "right",
                _ => "single",
            };
            let texture = format!(
                "{}{}",
                texture(&state.block),
                if half == "single" {
                    String::new()
                } else {
                    format!("_{half}")
                }
            );
            let layer = if let Some(&layer) = self.chests.textures.get(&texture) {
                layer
            } else {
                let path = self
                    .cache_path
                    .join("26.2/resources/assets/minecraft/textures/entity/chest")
                    .join(format!("{texture}.png"));
                let image = image::open(path)?.to_rgba8();
                ensure!(
                    image.dimensions() == (64, 64),
                    "Unexpected chest PNG dimensions"
                );
                let layer = 7 + (self.pixels.len() / 16384) as u32;
                self.pixels.extend_from_slice(image.as_raw());
                self.texture_versions.push(1);
                self.chests.textures.insert(texture, layer);
                layer
            };
            let name = match half {
                "left" => "minecraft:double_chest_left#main",
                "right" => "minecraft:double_chest_right#main",
                _ => "minecraft:chest#main",
            };
            let mut part = crate::assets::entity_models::get()?.layers[name].clone();
            let eased = 1. - (1. - open) * (1. - open) * (1. - open);
            for name in ["lid", "lock"] {
                part.children
                    .get_mut(name)
                    .context("Missing original chest part")?
                    .transform[3] = -eased * std::f32::consts::FRAC_PI_2;
            }
            let origin = Vec3::from_array(position.map(|v| v as f32));
            let low = glam::DVec3::from_array(position.map(|v| v as f64));
            let low = (low - origin.as_dvec3()).as_vec3();
            let mut mesh = Mesh::new(origin, origin + Vec3::ONE, 0);
            crate::assets::entity_models::bake(
                &part,
                Mat4::from_translation(low) * matrix(facing),
                false,
                layer,
                [1., 1.],
                &mut mesh,
                &mut None,
            )?;
            for v in &mut mesh.vertices {
                v.origin = origin.to_array();
                v.lod = 1024;
                v.shade = super::super::classic_shade(v.normal.into(), true);
            }
            mesh.tighten_bounds();
            let mesh = Arc::new(mesh);
            self.chests
                .meshes
                .insert(*position, (id, open.to_bits(), mesh.clone()));
            result.push(mesh);
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> crate::world::remote::RemoteWorld {
        let mut world = crate::world::remote::RemoteWorld {
            dimension: Some(crate::net::registry::Dimension {
                name: "minecraft:overworld".into(),
                min_y: 0,
                height: 16,
                logical_height: 16,
                skylight: true,
                ceiling: false,
            }),
            ..Default::default()
        };
        world
            .insert(crate::net::chunk::Chunk {
                x: 0,
                z: 0,
                sections: vec![crate::net::chunk::Section {
                    non_air: 0,
                    fluids: 0,
                    blocks: vec![0; 4096],
                    biomes: vec![0; 64],
                }],
                block_entities: vec![],
            })
            .unwrap();
        world
    }
    fn state(facing: &str, half: &str) -> u32 {
        builtins::get()
            .unwrap()
            .block_states
            .iter()
            .position(|s| {
                s.block.as_ref() == "minecraft:chest"
                    && s.properties.get("facing").is_some_and(|v| v == facing)
                    && s.properties.get("type").is_some_and(|v| v == half)
                    && s.properties
                        .get("waterlogged")
                        .is_some_and(|v| v == "false")
            })
            .unwrap() as u32
    }
    fn block_entity(report: &mut SessionReport, p: [i32; 3], state: u32) {
        report.world.set_block(p[0], p[1], p[2], state);
        report.world.block_entities.insert(
            p,
            Arc::new(crate::net::block_entity::BlockEntity {
                position: p,
                kind: 0,
                data: crate::net::nbt::Tag::End,
            }),
        );
    }
    #[test]
    fn original_four_chest_transformations_match() {
        let r = crate::net::block_event::reference();
        for row in r["transforms"].as_array().unwrap() {
            let expected: [f32; 16] = serde_json::from_value(row["matrix"].clone()).unwrap();
            assert!(matrix(row["facing"].as_str().unwrap())
                .abs_diff_eq(Mat4::from_cols_array(&expected), 2e-6));
        }
    }
    #[test]
    #[ignore = "Requires original chest PNGs; CPU only"]
    fn original_33_chest_poses_match_all_facings_without_texture_churn() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        let r = crate::net::block_event::reference();
        let now = std::time::Instant::now();
        let mut report = SessionReport {
            world: world(),
            ..Default::default()
        };
        let p = [2, 1, 2];
        for row in r["poses"].as_array().unwrap() {
            let open = row["open"].as_f64().unwrap() as f32;
            let source: crate::assets::entity_models::Part =
                serde_json::from_value(row["root"].clone()).unwrap();
            for transform in r["transforms"].as_array().unwrap() {
                let facing = transform["facing"].as_str().unwrap();
                let id = state(facing, row["half"].as_str().unwrap());
                block_entity(&mut report, p, id);
                Arc::make_mut(&mut report.block_events).lids.insert(
                    p,
                    block_event::Lid {
                        block: 0,
                        open: true,
                        initial: open,
                        started: now,
                    },
                );
                let actual = mesher.chest_meshes_at(&report, now).unwrap().remove(0);
                let expected_matrix: [f32; 16] =
                    serde_json::from_value(transform["matrix"].clone()).unwrap();
                let mut expected = Mesh::new(Vec3::ZERO, Vec3::ONE, 0);
                crate::assets::entity_models::bake(
                    &source,
                    Mat4::from_cols_array(&expected_matrix),
                    false,
                    0,
                    [1., 1.],
                    &mut expected,
                    &mut None,
                )
                .unwrap();
                assert_eq!(actual.indices, expected.indices);
                for (a, b) in actual.vertices.iter().zip(&expected.vertices) {
                    assert!(Vec3::from_array(a.position).abs_diff_eq(b.position.into(), 3e-6));
                    assert_eq!(a.uv, b.uv);
                    assert_eq!(a.origin, p.map(|v| v as f32));
                }
                let size = mesher.texture_pixels().len();
                assert!(Arc::ptr_eq(
                    &actual,
                    &mesher.chest_meshes_at(&report, now).unwrap()[0]
                ));
                assert_eq!(size, mesher.texture_pixels().len());
            }
        }
        assert_eq!(mesher.chests.textures.len(), 3);
        // A server event on one half opens both rendered halves.
        block_entity(&mut report, p, state("north", "left"));
        block_entity(&mut report, [3, 1, 2], state("north", "right"));
        Arc::make_mut(&mut report.block_events).lids.insert(
            p,
            block_event::Lid {
                block: 0,
                open: true,
                initial: 1.,
                started: now,
            },
        );
        let halves = mesher.chest_meshes_at(&report, now).unwrap();
        assert_eq!(halves.len(), 2);
        assert!(halves.iter().all(|m| m.max.y > 2.3));
        report.world.block_entities.clear();
        assert!(mesher.chest_meshes_at(&report, now).unwrap().is_empty());
    }
    #[test]
    fn block_event_checks_block_identity_and_discards_replaced_columns() {
        use base64::Engine;
        let r = crate::net::block_event::reference();
        let row = &r["packets"][1];
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(row["body"].as_str().unwrap())
            .unwrap();
        let mut report = SessionReport {
            world: world(),
            ..Default::default()
        };
        // Rewrite just the BlockPos to loaded coordinates; payload stays original.
        let mut bytes = bytes;
        let packed = (2i64 << 38) | (2i64 << 12) | 1;
        bytes[..8].copy_from_slice(&packed.to_be_bytes());
        let mut events = block_event::Store::default();
        assert!(events.packet(&report.world, &bytes).unwrap());
        assert!(events.lids.is_empty());
        block_entity(&mut report, [2, 1, 2], state("north", "single"));
        assert!(events.packet(&report.world, &bytes).unwrap());
        assert!(events.lids[&[2, 1, 2]].open);
        let revision = events.revision;
        events.packet(&report.world, &bytes).unwrap();
        assert_eq!(events.revision, revision);
        report.world.set_block(2, 1, 2, 0);
        events.retain_world(&report.world);
        assert!(events.lids.is_empty());
        block_entity(&mut report, [2, 1, 2], state("north", "single"));
        events.packet(&report.world, &bytes).unwrap();
        events.forget_column(0, 0);
        assert!(events.lids.is_empty());
    }
}
