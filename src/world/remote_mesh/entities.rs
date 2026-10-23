//! Live base entity models. Atlas tiles preserve the original texels; special
//! renderers, state-dependent layers and limb animations have separate paths.
use super::*;
use crate::{
    assets::entity_models,
    net::{entity::Pose, metadata::Value, SessionReport},
    world::Vertex,
};
use glam::{Mat4, Vec2};

struct Template {
    mesh: Mesh,
    head: Option<Vec3>,
    humanoid: Option<crate::assets::humanoid_attack::Skeleton>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::entity::{Entity, Event};
    fn area(mesh: &Mesh) -> f32 {
        mesh.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                let a = Vec3::from_array(mesh.vertices[t[0] as usize].position);
                let b = Vec3::from_array(mesh.vertices[t[1] as usize].position);
                let c = Vec3::from_array(mesh.vertices[t[2] as usize].position);
                (b - a).cross(c - a).length() * 0.5
            })
            .sum()
    }
    #[test]
    #[ignore = "Requires original humanoid PNG; CPU only"]
    fn server_swing_moves_humanoid_and_restores_cached_neutral_geometry() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        let ids = crate::assets::builtins::get().unwrap();
        let kind = (0..158)
            .find(|&id| ids.name("minecraft:entity_type", id) == Some("minecraft:zombie"))
            .unwrap();
        let mut report = SessionReport::default();
        report.entities.apply(Event::Spawn(Entity {
            id: 77,
            uuid: [1; 16],
            kind,
            spawn_data: 0,
            pose: Pose::default(),
            previous: Pose::default(),
            head_yaw: 0.,
            on_ground: true,
        }));
        let neutral = mesher.entity_meshes(&report).unwrap()[0].clone();
        let now = std::time::Instant::now();
        std::sync::Arc::make_mut(&mut report.animations)
            .start(77, false, 6, now - std::time::Duration::from_millis(150))
            .unwrap();
        let swinging = mesher.entity_meshes(&report).unwrap()[0].clone();
        assert!(!Arc::ptr_eq(&neutral, &swinging));
        assert_eq!(neutral.indices, swinging.indices);
        assert!(neutral
            .vertices
            .iter()
            .zip(&swinging.vertices)
            .any(|(a, b)| a.position != b.position));
        assert!((area(&neutral) - area(&swinging)).abs() < 1e-5);
        std::sync::Arc::make_mut(&mut report.animations)
            .swings
            .get_mut(&77)
            .unwrap()
            .started = now - std::time::Duration::from_secs(1);
        let rested = mesher.entity_meshes(&report).unwrap()[0].clone();
        assert!(neutral
            .vertices
            .iter()
            .zip(&rested.vertices)
            .all(|(a, b)| a.position == b.position && a.uv == b.uv));
        assert!(Arc::ptr_eq(
            &rested,
            &mesher.entity_meshes(&report).unwrap()[0]
        ));
    }
    #[test]
    fn uv_clipping_interpolates_positions_without_changing_the_surface() {
        let points = vec![
            Point {
                position: Vec3::ZERO,
                uv: Vec2::ZERO,
            },
            Point {
                position: Vec3::X,
                uv: Vec2::X,
            },
            Point {
                position: Vec3::ONE,
                uv: Vec2::ONE,
            },
            Point {
                position: Vec3::Y + Vec3::Z,
                uv: Vec2::Y,
            },
        ];
        let result = clip(clip(points, 0, 0.25, true), 0, 0.5, false);
        assert_eq!(result.len(), 4);
        for p in result {
            assert!((p.position.x - p.uv.x).abs() < 1e-6);
            assert!((0.25..=0.5).contains(&p.uv.x));
        }
    }
    #[test]
    #[ignore = "Requires imported official entity textures; CPU only"]
    fn live_entities_preserve_all_ninety_base_models_texels_and_server_updates() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        let mut report = SessionReport::default();
        let reference = entity_models::get().unwrap().compile(&cache).unwrap();
        for model in &reference.models {
            let e = Entity {
                id: model.kind as i32,
                uuid: [0; 16],
                kind: model.kind,
                spawn_data: 0,
                pose: Pose {
                    yaw: 180.,
                    ..Default::default()
                },
                previous: Pose::default(),
                head_yaw: 180.,
                on_ground: true,
            };
            report.entities.apply(Event::Spawn(e));
        }
        let meshes = mesher.entity_meshes(&report).unwrap();
        assert_eq!(meshes.len(), 90);
        use sha1::{Digest, Sha1};
        let mut original_tiles = std::collections::HashSet::new();
        for definition in entity_models::get().unwrap().entities.values() {
            let Some(texture) = &definition.texture else {
                continue;
            };
            let id = ResourceId::parse(texture).unwrap();
            let image = image::open(
                cache
                    .join("26.2/resources/assets")
                    .join(id.namespace)
                    .join(id.path),
            )
            .unwrap()
            .to_rgba8();
            for ty in 0..8i32 {
                for tx in 0..8i32 {
                    let mut tile = Vec::new();
                    for y in ty * 64..(ty + 1) * 64 {
                        for x in tx * 64..(tx + 1) * 64 {
                            tile.extend_from_slice(
                                &image
                                    .get_pixel(
                                        x.rem_euclid(image.width() as i32) as u32,
                                        y.rem_euclid(image.height() as i32) as u32,
                                    )
                                    .0,
                            );
                        }
                    }
                    original_tiles.insert(Sha1::digest(&tile).to_vec());
                }
            }
        }
        for tile in mesher.pixels.as_chunks::<16384>().0 {
            assert!(original_tiles.contains(Sha1::digest(tile).as_slice()));
        }
        for model in &reference.models {
            let template = mesher.entities.templates[&model.kind].as_ref().unwrap();
            assert!(
                (area(&model.mesh) - area(&template.mesh)).abs() < 0.003,
                "surface changed for {}: {} -> {}; UV {:?}",
                model.kind,
                area(&model.mesh),
                area(&template.mesh),
                model.mesh.vertices.iter().map(|v| v.uv).fold(
                    ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]),
                    |(mut a, mut b), uv| {
                        for i in 0..2 {
                            a[i] = a[i].min(uv[i]);
                            b[i] = b[i].max(uv[i]);
                        }
                        (a, b)
                    }
                )
            );
            for vertex in &template.mesh.vertices {
                assert!(vertex.uv.iter().all(|v| (-0.00001..=1.00001).contains(v)));
            }
        }
        let count = mesher.pixels.len();
        let again = mesher.entity_meshes(&report).unwrap();
        assert_eq!(count, mesher.pixels.len());
        for a in &again {
            assert!(meshes.iter().any(|b| Arc::ptr_eq(a, b)));
        }
        let id = reference.models[0].kind as i32;
        let old = mesher.entities.meshes[&id].3.clone();
        report.entities.apply(Event::Relative {
            id,
            delta: Some([4096, 0, 0]),
            rotation: None,
            on_ground: true,
        });
        mesher.entity_meshes(&report).unwrap();
        let moved = &mesher.entities.meshes[&id].3;
        assert!((moved.min.x - old.min.x - 1.).abs() < 1e-5);
        report.entities.apply(Event::Remove(vec![id]));
        assert_eq!(mesher.entity_meshes(&report).unwrap().len(), 89);
        assert!(!mesher.entities.meshes.contains_key(&id));
    }
}
#[derive(Default)]
pub(super) struct Cache {
    templates: HashMap<u32, Option<Template>>,
    meshes: HashMap<i32, (u32, Pose, f32, Arc<Mesh>)>,
    armor: HashMap<i32, crate::net::metadata::ArmorStand>,
    attack: HashMap<i32, Option<(u64, u32)>>,
    tiles: HashMap<(u32, usize, i32, i32), u32>,
}
#[derive(Clone, Copy)]
struct Point {
    position: Vec3,
    uv: Vec2,
}
// Clip in UV space, interpolating positions along the very same model edge.
// This splits an original quad across 64px atlas tiles without scaling pixels.
fn clip(input: Vec<Point>, axis: usize, edge: f32, above: bool) -> Vec<Point> {
    let mut output = Vec::new();
    if input.is_empty() {
        return output;
    }
    let mut a = *input.last().unwrap();
    for b in input {
        let inside = |p: Point| {
            if above {
                p.uv[axis] >= edge
            } else {
                p.uv[axis] <= edge
            }
        };
        if inside(a) != inside(b) {
            let t = (edge - a.uv[axis]) / (b.uv[axis] - a.uv[axis]);
            output.push(Point {
                position: a.position.lerp(b.position, t),
                uv: a.uv.lerp(b.uv, t),
            });
        }
        if inside(b) {
            output.push(b);
        }
        a = b;
    }
    output
}
impl RemoteMesher {
    fn entity_template(
        &mut self,
        kind: u32,
        armor: Option<&crate::net::metadata::ArmorStand>,
    ) -> Result<()> {
        if armor.is_none() && self.entities.templates.contains_key(&kind) {
            return Ok(());
        }
        let compiled = if let Some(pose) = armor {
            entity_models::get()?.compile_armor_stand(&self.cache_path, kind, pose)?
        } else {
            entity_models::get()?.compile_kind(&self.cache_path, kind)?
        };
        let Some(geometry) = compiled.models.into_iter().next() else {
            self.entities.templates.insert(kind, None);
            return Ok(());
        };
        let name = crate::assets::builtins::get()?
            .name("minecraft:entity_type", kind)
            .context("Entity name")?;
        let id = ResourceId::parse(
            entity_models::get()?.entities[name]
                .texture
                .as_ref()
                .context("Entity texture")?,
        )?;
        let image = image::open(
            self.cache_path
                .join("26.2/resources/assets")
                .join(id.namespace)
                .join(id.path),
        )?
        .to_rgba8();

        let mut mesh = Mesh::new(Vec3::ZERO, Vec3::ZERO, 0);
        mesh.translucent = geometry.mesh.translucent;
        for quad in geometry.mesh.vertices.as_chunks::<4>().0 {
            let source_layer = (quad[0].layer & 0x7fff_ffff) as usize;
            let source: Vec<_> = quad
                .iter()
                .map(|v| Point {
                    position: Vec3::from_array(v.position),
                    uv: Vec2::from_array(v.uv),
                })
                .collect();
            let min = source
                .iter()
                .fold(Vec2::splat(f32::INFINITY), |v, p| v.min(p.uv));
            let max = source
                .iter()
                .fold(Vec2::splat(f32::NEG_INFINITY), |v, p| v.max(p.uv));
            for ty in (min.y * 4.).floor() as i32..((max.y * 4.).floor() as i32 + 1) {
                for tx in (min.x * 4.).floor() as i32..((max.x * 4.).floor() as i32 + 1) {
                    let mut polygon = source.clone();
                    for (axis, start) in [(0, tx), (1, ty)] {
                        polygon = clip(polygon, axis, start as f32 / 4., true);
                        polygon = clip(polygon, axis, (start + 1) as f32 / 4., false);
                    }
                    if polygon.len() < 3 {
                        continue;
                    }
                    let area: f32 = (1..polygon.len() - 1)
                        .map(|i| {
                            (polygon[i].position - polygon[0].position)
                                .cross(polygon[i + 1].position - polygon[0].position)
                                .length()
                        })
                        .sum();
                    if area < 1e-9 {
                        continue;
                    }
                    let layer = *self
                        .entities
                        .tiles
                        .entry((kind, source_layer, tx, ty))
                        .or_insert_with(|| {
                            let layer = 7 + (self.pixels.len() / 16384) as u32;
                            for y in ty * 64..(ty + 1) * 64 {
                                for x in tx * 64..(tx + 1) * 64 {
                                    self.pixels.extend_from_slice(
                                        &image
                                            .get_pixel(
                                                x.rem_euclid(image.width() as i32) as u32,
                                                y.rem_euclid(image.height() as i32) as u32,
                                            )
                                            .0,
                                    );
                                }
                            }
                            self.texture_versions.push(MAP_TILE_VERSION_FLAG | 1);
                            layer
                        });
                    let base = mesh.vertices.len() as u32;
                    for p in &polygon {
                        mesh.vertices.push(Vertex {
                            position: p.position.to_array(),
                            uv: [p.uv.x * 4. - tx as f32, p.uv.y * 4. - ty as f32],
                            layer: layer | (quad[0].layer & 0x8000_0000),
                            lod: quad[0].lod,
                            tint: [1.; 3],
                            normal: quad[0].normal,
                            origin: [0.; 3],
                            shade: quad[0].shade,
                        });
                    }
                    for i in 1..polygon.len() - 1 {
                        mesh.indices
                            .extend([base, base + i as u32, base + i as u32 + 1]);
                    }
                }
            }
        }
        mesh.tighten_bounds();
        self.entities.templates.insert(
            kind,
            Some(Template {
                mesh,
                head: geometry.head_pivot,
                humanoid: geometry.humanoid,
            }),
        );
        Ok(())
    }
    pub fn entity_meshes(&mut self, report: &SessionReport) -> Result<Vec<Arc<Mesh>>> {
        let mut result = Vec::new();
        let old = std::mem::take(&mut self.entities.meshes);
        let old_armor = std::mem::take(&mut self.entities.armor);
        let old_attack = std::mem::take(&mut self.entities.attack);
        let now = std::time::Instant::now();
        for e in report.entities.entities.values() {
            if matches!(
                e.type_name()?,
                Some("minecraft:item_frame" | "minecraft:glow_item_frame")
            ) {
                continue;
            }
            let invisible = report
                .metadata
                .entities
                .get(&e.id)
                .and_then(|m| m.get(&0))
                .is_some_and(|v| matches!(v,Value::Byte(flags) if flags&32!=0));
            if invisible {
                continue;
            }
            let swing = report
                .animations
                .swings
                .get(&e.id)
                .filter(|s| s.progress(now) < 1.);
            let progress = swing.map_or(0., |s| s.progress(now));
            let attack = swing.map(|s| (s.serial, progress.to_bits()));
            self.entities.attack.insert(e.id, attack);
            let armor = if e.type_name()? == Some("minecraft:armor_stand") {
                Some(report.metadata.armor_stand(e.id, e.pose.yaw)?)
            } else {
                None
            };
            if let Some(pose) = &armor {
                self.entities.armor.insert(e.id, pose.clone());
            }
            if let Some((_, _, _, mesh)) = old.get(&e.id).filter(|(kind, pose, head, _)| {
                *kind == e.kind
                    && *pose == e.pose
                    && *head == e.head_yaw
                    && old_armor.get(&e.id) == armor.as_ref()
                    && old_attack.get(&e.id).copied().flatten() == attack
            }) {
                result.push(mesh.clone());
                self.entities
                    .meshes
                    .insert(e.id, (e.kind, e.pose, e.head_yaw, mesh.clone()));
                continue;
            }
            self.entity_template(e.kind, armor.as_ref())?;
            let Some(template) = &self.entities.templates[&e.kind] else {
                continue;
            };
            let mesh = if let Some((_, _, _, mesh)) =
                old.get(&e.id).filter(|(kind, pose, head, _)| {
                    *kind == e.kind
                        && *pose == e.pose
                        && *head == e.head_yaw
                        && old_armor.get(&e.id) == armor.as_ref()
                        && old_attack.get(&e.id).copied().flatten() == attack
                }) {
                mesh.clone()
            } else {
                let origin = e.pose.position.as_vec3();
                let root = Mat4::from_translation((e.pose.position - origin.as_dvec3()).as_vec3())
                    * Mat4::from_rotation_y((180. - e.pose.yaw).to_radians());
                let head = template
                    .head
                    .map(|pivot| {
                        Mat4::from_translation(pivot)
                            * Mat4::from_rotation_y((e.pose.yaw - e.head_yaw).to_radians())
                            * Mat4::from_rotation_x(-e.pose.pitch.to_radians())
                            * Mat4::from_translation(-pivot)
                    })
                    .unwrap_or(Mat4::IDENTITY);
                let pose = template
                    .humanoid
                    .as_ref()
                    .filter(|_| swing.is_some())
                    .map(|h| {
                        h.transforms(
                            progress,
                            crate::net::animation::left_arm(
                                &report.metadata,
                                e.id,
                                swing.is_some_and(|s| s.off_hand),
                            ),
                            e.pose.pitch.to_radians(),
                        )
                    });
                let mut mesh = template.mesh.clone();
                for v in &mut mesh.vertices {
                    let part = if v.lod == 1 {
                        head
                    } else if let Some(pose) = &pose {
                        v.lod
                            .checked_sub(1)
                            .and_then(|index| pose.get(index as usize))
                            .copied()
                            .unwrap_or(Mat4::IDENTITY)
                    } else {
                        Mat4::IDENTITY
                    };
                    let transform = root * part;
                    v.position = transform.transform_point3(v.position.into()).to_array();
                    let normal = transform
                        .transform_vector3(v.normal.into())
                        .normalize_or_zero();
                    v.normal = normal.to_array();
                    v.lod = 512 | 1024;
                    v.origin = origin.to_array();
                    let a = Vec3::new(0.2, 1., -0.7).normalize();
                    let b = Vec3::new(-0.2, 1., 0.7).normalize();
                    v.tint =
                        [((normal.dot(a).max(0.) + normal.dot(b).max(0.)) * 0.6 + 0.4).min(1.); 3];
                }
                mesh.tighten_bounds();
                Arc::new(mesh)
            };
            result.push(mesh.clone());
            self.entities
                .meshes
                .insert(e.id, (e.kind, e.pose, e.head_yaw, mesh));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod armor_live_tests {
    use super::*;
    use crate::net::{
        codec::varint,
        entity::{Entity, Event},
    };
    #[test]
    #[ignore = "Requires imported official armor stand texture; CPU only"]
    fn armor_metadata_updates_rebuild_pose_without_reallocating_existing_texture_tiles() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let builtin = builtins::get().unwrap();
        let kind = (0..158)
            .find(|&id| builtin.name("minecraft:entity_type", id) == Some("minecraft:armor_stand"))
            .unwrap();
        let mut report = SessionReport::default();
        report.entities.apply(Event::Spawn(Entity {
            id: 91,
            uuid: [0; 16],
            kind,
            spawn_data: 0,
            pose: Pose::default(),
            previous: Pose::default(),
            head_yaw: 0.,
            on_ground: true,
        }));
        let mut mesher = RemoteMesher::new(&cache);
        let normal = mesher.entity_meshes(&report).unwrap()[0].clone();
        let r = crate::assets::signs::reference().unwrap();
        let flag_index = r["armor_indices"]["DATA_CLIENT_FLAGS"].as_u64().unwrap() as u8;
        let mut packet = Vec::new();
        varint(91, &mut packet);
        packet.extend([flag_index, 0, 4, 255]);
        Arc::make_mut(&mut report.metadata).packet(&packet).unwrap();
        let arms = mesher.entity_meshes(&report).unwrap()[0].clone();
        assert!(!Arc::ptr_eq(&normal, &arms));
        assert!(arms.vertices.len() > normal.vertices.len());
        let pixels = mesher.pixels.len();
        packet.clear();
        varint(91, &mut packet);
        packet.push(r["armor_indices"]["DATA_HEAD_POSE"].as_u64().unwrap() as u8);
        varint(
            crate::net::maps::reference().serializers["ROTATIONS"],
            &mut packet,
        );
        for n in [60f32, 30., -10.] {
            packet.extend(n.to_be_bytes());
        }
        packet.push(255);
        Arc::make_mut(&mut report.metadata).packet(&packet).unwrap();
        let posed = mesher.entity_meshes(&report).unwrap()[0].clone();
        assert!(!Arc::ptr_eq(&arms, &posed));
        assert_eq!(mesher.pixels.len(), pixels);
        assert!(arms
            .vertices
            .iter()
            .zip(&posed.vertices)
            .any(|(a, b)| a.position != b.position));
        let again = mesher.entity_meshes(&report).unwrap();
        assert!(Arc::ptr_eq(&again[0], &posed));
    }
}
