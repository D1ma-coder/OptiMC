use super::*;
use crate::net::{items::Stack, SessionReport};
use glam::Mat4;

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    position: [u64; 3],
    direction: i32,
    invisible: bool,
    rotation: u8,
    item: Option<Stack>,
    glow: bool,
}
#[derive(Default)]
pub(super) struct Cache {
    meshes: HashMap<i32, (Stamp, Arc<Mesh>)>,
    maps: HashMap<i32, (Arc<[u8; 16384]>, [u32; 4])>,
}

fn matrix(direction: i32) -> Mat4 {
    let directions = [
        Vec3::NEG_Y,
        Vec3::Y,
        Vec3::NEG_Z,
        Vec3::Z,
        Vec3::NEG_X,
        Vec3::X,
    ];
    let d = directions[direction as usize];
    let (pitch, yaw) = match direction {
        0 => (90f32, 180f32),
        1 => (-90., 180.),
        2 => (0., 0.),
        3 => (0., 180.),
        4 => (0., 90.),
        _ => (0., -90.),
    };
    Mat4::from_translation(d * 0.46875)
        * Mat4::from_rotation_x(pitch.to_radians())
        * Mat4::from_rotation_y(yaw.to_radians())
}
fn map_matrix(root: Mat4, rotation: u8, invisible: bool) -> Mat4 {
    root * Mat4::from_translation(Vec3::new(0., 0., if invisible { 0.5 } else { 0.4375 }))
        * Mat4::from_rotation_z(((rotation % 4) as f32 * 90.).to_radians())
        * Mat4::from_rotation_z(std::f32::consts::PI)
        * Mat4::from_scale(Vec3::splat(1. / 128.))
        * Mat4::from_translation(Vec3::new(-64., -64., -1.))
}
impl RemoteMesher {
    fn map_layers(&mut self, id: i32, map: &crate::net::maps::Map) -> [u32; 4] {
        let existing = self.frames.maps.get(&id).cloned();
        if let Some((colors, layers)) = &existing {
            if Arc::ptr_eq(colors, &map.colors) {
                return *layers;
            }
        }
        let layers = existing.map(|(_, layers)| layers).unwrap_or_else(|| {
            let first = 7 + (self.pixels.len() / 16384) as u32;
            self.pixels.resize(self.pixels.len() + 4 * 16384, 0);
            self.texture_versions.extend([0; 4]);
            [first, first + 1, first + 2, first + 3]
        });
        for (tile, layer) in layers.into_iter().enumerate() {
            let (tx, ty) = (tile % 2 * 64, tile / 2 * 64);
            let start = (layer as usize - 7) * 16384;
            let mut changed = self.texture_versions[layer as usize - 7] == 0;
            for y in 0..64 {
                for x in 0..64 {
                    let color = crate::net::maps::rgba(map.colors[(ty + y) * 128 + tx + x]);
                    let dest =
                        &mut self.pixels[start + (y * 64 + x) * 4..start + (y * 64 + x + 1) * 4];
                    if dest != color {
                        dest.copy_from_slice(&color);
                        changed = true;
                    }
                }
            }
            if changed {
                self.texture_versions[layer as usize - 7] =
                    self.texture_versions[layer as usize - 7].wrapping_add(1)
                        | MAP_TILE_VERSION_FLAG;
            }
        }
        self.frames.maps.insert(id, (map.colors.clone(), layers));
        layers
    }
    /// Actual server entities; original model resources and full 128×128 map data.
    pub fn frame_meshes(&mut self, report: &SessionReport) -> Result<Vec<Arc<Mesh>>> {
        let mut result = Vec::new();
        let mut retained = HashMap::new();
        let mut entities: Vec<_> = report.entities.entities.values().collect();
        entities.sort_by_key(|e| e.id);
        for entity in entities {
            let name = entity.type_name()?;
            if !matches!(
                name,
                Some("minecraft:item_frame" | "minecraft:glow_item_frame")
            ) {
                continue;
            }
            ensure!(
                (0..6).contains(&entity.spawn_data),
                "Invalid item frame direction"
            );
            let (invisible, rotation, item) = report.metadata.frame(entity.id);
            let map_id = item.and_then(|item| item.map);
            let layers = map_id.and_then(|id| {
                report
                    .maps
                    .maps
                    .get(&id)
                    .map(|map| self.map_layers(id, map))
            });
            let stamp = Stamp {
                position: entity.pose.position.to_array().map(f64::to_bits),
                direction: entity.spawn_data,
                invisible,
                rotation,
                item: item.cloned(),
                glow: name == Some("minecraft:glow_item_frame"),
            };
            if let Some((old, mesh)) = self.frames.meshes.get(&entity.id) {
                // A map arriving after the frame must also add the map quad.
                let had_map = mesh.vertices.iter().any(|v| v.lod & 256 != 0);
                if old == &stamp && had_map == layers.is_some() {
                    retained.insert(entity.id, (stamp, mesh.clone()));
                    result.push(mesh.clone());
                    continue;
                }
            }
            let position = entity.pose.position.as_vec3();
            // Preserve the sub-centimetre map/backing separation even near the
            // world border, where absolute f32 coordinates lose whole metres.
            let low = (entity.pose.position - position.as_dvec3()).as_vec3();
            let root = Mat4::from_translation(low) * matrix(entity.spawn_data);
            let mut mesh = Mesh::new(position - Vec3::ONE, position + Vec3::ONE, 1);
            if !invisible {
                let model = format!(
                    "minecraft:block/{}{}",
                    if stamp.glow {
                        "glow_item_frame"
                    } else {
                        "item_frame"
                    },
                    if map_id.is_some() { "_map" } else { "" }
                );
                let part = self.part(&Variant {
                    model,
                    x: 0,
                    y: 0,
                    uvlock: false,
                    weight: 1,
                })?;
                let frame = root * Mat4::from_translation(Vec3::splat(-0.5));
                for quad in &part.quads {
                    mesh.quad_uv(
                        quad.baked
                            .points
                            .map(|p| frame.transform_point3(p).to_array()),
                        quad.baked.uv.map(|p| p.to_array()),
                        quad.layer,
                        [1.; 3],
                    );
                }
            }
            if let Some(layers) = layers {
                let transform = map_matrix(root, rotation, invisible);
                for (tile, layer) in layers.into_iter().enumerate() {
                    let (x, y) = ((tile % 2 * 64) as f32, (tile / 2 * 64) as f32);
                    let start = mesh.vertices.len();
                    mesh.quad_uv(
                        [
                            [x, y + 64., -0.01],
                            [x + 64., y + 64., -0.01],
                            [x + 64., y, -0.01],
                            [x, y, -0.01],
                        ]
                        .map(|p| transform.transform_point3(Vec3::from_array(p)).to_array()),
                        [[0., 1.], [1., 1.], [1., 0.], [0., 0.]],
                        layer,
                        [1.; 3],
                    );
                    for vertex in &mut mesh.vertices[start..] {
                        vertex.lod = 256 | u32::from(stamp.glow);
                    }
                }
            }
            for vertex in &mut mesh.vertices {
                vertex.origin = position.to_array();
                vertex.lod |= 1024;
            }
            mesh.tighten_bounds();
            let mesh = Arc::new(mesh);
            retained.insert(entity.id, (stamp, mesh.clone()));
            result.push(mesh);
        }
        self.frames.meshes = retained;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    #[ignore = "Requires imported official frame resources; CPU only"]
    fn map_backing_gap_survives_all_directions_at_world_border_coordinates() {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let packets = crate::net::maps::packets();
        let mut report = SessionReport::default();
        for (_, id, body) in packets.iter().take(3) {
            match id {
                1 => {
                    report.entities.packet(*id, body).unwrap();
                }
                0x63 => {
                    Arc::make_mut(&mut report.metadata).packet(body).unwrap();
                }
                0x33 => {
                    Arc::make_mut(&mut report.maps).packet(body).unwrap();
                }
                _ => unreachable!(),
            }
        }
        let mut mesher = RemoteMesher::new(&cache);
        for coordinate in [0., 1000000., -1000000., 29999984., -29999984.] {
            for direction in 0..6 {
                let e = report.entities.entities.get_mut(&99).unwrap();
                e.pose.position = glam::DVec3::new(coordinate + 0.5, 64.5, coordinate + 0.03125);
                e.spawn_data = direction;
                let meshes = mesher.frame_meshes(&report).unwrap();
                let mesh = &meshes[0];
                let map = mesh.vertices.iter().find(|v| v.lod & 256 != 0).unwrap();
                let normal = Vec3::from_array(map.normal);
                let backing = mesh
                    .vertices
                    .iter()
                    .find(|v| v.lod & 256 == 0 && Vec3::from_array(v.normal).dot(normal) > 0.99)
                    .unwrap();
                let gap = (Vec3::from_array(map.position) - Vec3::from_array(backing.position))
                    .dot(normal);
                assert!(
                    (0.007..0.009).contains(&gap),
                    "{coordinate}, direction {direction}, gap {gap}"
                );
                assert_eq!(map.origin, backing.origin);
                assert_ne!(map.lod & 1024, 0);
            }
        }
    }
    #[test]
    #[ignore = "Requires imported official frame models/textures; CPU only"]
    fn official_frame_map_reaches_mesh_and_full_resolution_tiles_and_updates_in_place() {
        let cache = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut report = crate::net::SessionReport::default();
        let packets = crate::net::maps::packets();
        for (_, id, body) in packets.iter().take(3) {
            match id {
                1 => {
                    assert!(report.entities.packet(*id, body).unwrap());
                }
                0x63 => {
                    assert!(Arc::make_mut(&mut report.metadata).packet(body).unwrap());
                }
                0x33 => {
                    Arc::make_mut(&mut report.maps).packet(body).unwrap();
                }
                _ => unreachable!(),
            }
        }
        let mut mesher = RemoteMesher::new(&cache);
        let first = mesher.frame_meshes(&report).unwrap();
        assert_eq!(first.len(), 1);
        let pixels: Vec<_> = first[0]
            .vertices
            .iter()
            .filter(|v| v.lod & 256 != 0)
            .collect();
        assert_eq!(pixels.len(), 16, "Four tiles retain all 128x128 pixels");
        assert!(
            first[0].vertices.len() > 16,
            "Original frame border must also be rendered"
        );
        let map = &report.maps.maps[&42];
        for y in 0..128 {
            for x in 0..128 {
                let tile = y / 64 * 2 + x / 64;
                let layer = pixels[tile * 4].layer as usize;
                let offset = (layer - 7) * 16384 + ((y % 64) * 64 + x % 64) * 4;
                assert_eq!(
                    &mesher.texture_pixels()[offset..offset + 4],
                    &crate::net::maps::rgba(map.colors[y * 128 + x])
                );
            }
        }
        let versions = mesher.texture_versions().to_vec();
        let repeated = mesher.frame_meshes(&report).unwrap();
        assert!(Arc::ptr_eq(&first[0], &repeated[0]));
        assert_eq!(versions, mesher.texture_versions());
        let patch = packets.iter().find(|p| p.0 == "patch").unwrap();
        Arc::make_mut(&mut report.maps).packet(&patch.2).unwrap();
        let changed = mesher.frame_meshes(&report).unwrap();
        assert!(
            Arc::ptr_eq(&first[0], &changed[0]),
            "Pixel changes reuse frame geometry"
        );
        assert_eq!(
            versions
                .iter()
                .zip(mesher.texture_versions())
                .filter(|(a, b)| a != b)
                .count(),
            1,
            "Only the changed 64x64 tile is uploaded"
        );
        let clear = packets.iter().find(|p| p.0 == "icons_clear").unwrap();
        let before = mesher.texture_versions().to_vec();
        Arc::make_mut(&mut report.maps).packet(&clear.2).unwrap();
        mesher.frame_meshes(&report).unwrap();
        assert_eq!(
            before,
            mesher.texture_versions(),
            "Icon-only updates must not upload pixels"
        );
        let empty = packets.iter().find(|p| p.0 == "empty").unwrap();
        Arc::make_mut(&mut report.metadata)
            .packet(&empty.2)
            .unwrap();
        let emptied = mesher.frame_meshes(&report).unwrap();
        assert!(emptied[0].vertices.iter().all(|v| v.lod & 256 == 0));
        report.entities.entities.clear();
        assert!(mesher.frame_meshes(&report).unwrap().is_empty());
    }
    #[test]
    fn six_directions_and_eight_rotations_match_original_pose_stack_matrices() {
        let mut json = Vec::new();
        flate2::read::GzDecoder::new(
            &include_bytes!("../../../data/26.2/maps-reference.json.gz")[..],
        )
        .read_to_end(&mut json)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&json).unwrap();
        for value in reference["transforms"].as_array().unwrap() {
            let root = matrix(value["direction"].as_i64().unwrap() as i32);
            let map = map_matrix(
                root,
                value["rotation"].as_u64().unwrap() as u8,
                value["invisible"].as_bool().unwrap(),
            );
            for (name, matrix) in [("frame", root), ("map", map)] {
                for (actual, wanted) in matrix
                    .to_cols_array()
                    .into_iter()
                    .zip(value[name].as_array().unwrap())
                {
                    assert!(
                        (actual - wanted.as_f64().unwrap() as f32).abs() < 1e-5,
                        "{value} {name}: {actual} != {wanted}"
                    );
                }
            }
        }
    }
}
