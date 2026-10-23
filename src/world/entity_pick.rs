//! Protocol entity picking: data-driven base dimensions and special attachment
//! boxes. Stateful species/vehicle/equipment rules are recorded in the audit.
use crate::net::{entity::Entity, metadata::Value, SessionReport};
use glam::DVec3;
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read, sync::OnceLock};
#[derive(Deserialize)]
struct Definition {
    name: String,
    living: bool,
    width: f32,
    height: f32,
    #[serde(rename = "isPickable")]
    pick_owner: String,
    #[serde(rename = "getPickRadius")]
    radius_owner: String,
    indices: BTreeMap<String, u8>,
    poses: Vec<Option<[f32; 2]>>,
    baby_poses: Vec<Option<[f32; 2]>>,
}
#[derive(Deserialize)]
struct Catalog {
    types: Vec<Definition>,
}
fn catalog() -> &'static Catalog {
    static DATA: OnceLock<Catalog> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/entity-picking.json.gz").as_slice(),
        )
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut raw)
        .expect("embedded entity picking data");
        use sha1::{Digest, Sha1};
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../data/26.2/reference.json"))
                .expect("reference manifest");
        assert!(raw.len() <= 8 * 1024 * 1024);
        assert_eq!(
            format!("{:x}", Sha1::digest(&raw)),
            reference["sha1"]["entity-picking.json"].as_str().unwrap()
        );
        let data: Catalog =
            serde_json::from_slice(&raw).expect("validated embedded entity picking catalog");
        assert_eq!(data.types.len(), 158);
        let ids = crate::assets::builtins::get().expect("official registries");
        for (kind, d) in data.types.iter().enumerate() {
            assert_eq!(
                ids.name("minecraft:entity_type", kind as u32),
                Some(d.name.as_str())
            );
        }
        data
    })
}
/// Load once before opening the window, avoiding cold-load stalls on clicks.
pub fn prepare() {
    let _ = catalog();
}
#[derive(Clone, Copy, Debug)]
pub struct EntityHit {
    pub id: i32,
    pub uuid: [u8; 16],
    pub position: DVec3,
    pub relative: DVec3,
}
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Block(super::picking::BlockHit),
    Entity(EntityHit),
}
fn value<'a>(
    report: &'a SessionReport,
    e: &Entity,
    d: &Definition,
    name: &str,
) -> Option<&'a Value> {
    report
        .metadata
        .entities
        .get(&e.id)?
        .get(d.indices.get(name)?)
}
fn number(report: &SessionReport, e: &Entity, d: &Definition, name: &str) -> Option<f32> {
    match value(report, e, d, name)? {
        Value::Byte(n) => Some(*n as f32),
        Value::Int(n) => Some(*n as f32),
        Value::Encoded { serializer, bytes } => {
            let ids = &crate::net::maps::reference().serializers;
            if ids.get("FLOAT") == Some(serializer) && bytes.len() == 4 {
                Some(f32::from_be_bytes(bytes.as_slice().try_into().ok()?))
            } else if ids.get("BOOLEAN") == Some(serializer) && bytes.len() == 1 {
                Some((bytes[0] != 0) as u8 as f32)
            } else if ids.get("POSE") == Some(serializer) {
                crate::net::codec::read_varint(&mut bytes.as_slice())
                    .ok()
                    .map(|n| n as f32)
            } else {
                None
            }
        }
        _ => None,
    }
}
pub fn frame_box(anchor: [i32; 3], face: i32, map: bool) -> Option<[f64; 6]> {
    let n = match face {
        0 => -DVec3::Y,
        1 => DVec3::Y,
        2 => -DVec3::Z,
        3 => DVec3::Z,
        4 => -DVec3::X,
        5 => DVec3::X,
        _ => return None,
    };
    let center = DVec3::from_array(anchor.map(f64::from)) + DVec3::splat(0.5) - n * 0.46875;
    let extent =
        DVec3::splat(if map { 0.5 } else { 0.375 }) * (DVec3::ONE - n.abs()) + n.abs() * 0.03125;
    let lo = center - extent;
    let hi = center + extent;
    Some([lo.x, lo.y, lo.z, hi.x, hi.y, hi.z])
}
fn bounds(report: &SessionReport, e: &Entity, d: &Definition) -> Option<[f64; 6]> {
    if d.name == "minecraft:item_frame" || d.name == "minecraft:glow_item_frame" {
        return frame_box(
            e.pose.position.floor().as_ivec3().to_array(),
            e.spawn_data,
            report
                .metadata
                .frame(e.id)
                .2
                .is_some_and(|s| s.map.is_some()),
        );
    }
    let pose = number(report, e, d, "DATA_POSE").unwrap_or(0.) as usize;
    let mut size = d
        .poses
        .get(pose)
        .copied()
        .flatten()
        .unwrap_or([d.width, d.height]);
    if d.name == "minecraft:interaction" {
        size = [
            number(report, e, d, "DATA_WIDTH_ID").unwrap_or(1.),
            number(report, e, d, "DATA_HEIGHT_ID").unwrap_or(1.),
        ];
    }
    if pose != 2 && d.living {
        let scale = report
            .attributes
            .value(e.id, "minecraft:scale")
            .unwrap_or(1.) as f32;
        let baby = if d.name == "minecraft:armor_stand" {
            number(report, e, d, "DATA_CLIENT_FLAGS").unwrap_or(0.) as u8 & 1 != 0
        } else {
            number(report, e, d, "DATA_BABY_ID").unwrap_or(0.) != 0.
        };
        if baby {
            size = d
                .baby_poses
                .get(pose)
                .copied()
                .flatten()
                .unwrap_or_else(|| size.map(|s| s * 0.5));
        }
        size = size.map(|s| s * scale);
    }
    if size.iter().any(|v| !v.is_finite() || *v < 0. || *v > 1024.) {
        return None;
    }
    let half = f64::from(size[0]) / 2.;
    let p = e.pose.position;
    Some([
        p.x - half,
        p.y,
        p.z - half,
        p.x + half,
        p.y + f64::from(size[1]),
        p.z + half,
    ])
}
fn pickable(report: &SessionReport, e: &Entity, d: &Definition) -> bool {
    if d.name == "minecraft:player"
        && report
            .player_info
            .players
            .get(&e.uuid)
            .is_some_and(|p| p.game_mode == 3)
    {
        return false;
    }
    if d.name == "minecraft:armor_stand"
        && number(report, e, d, "DATA_CLIENT_FLAGS").unwrap_or(0.) as u8 & 16 != 0
    {
        return false;
    }
    match d.pick_owner.rsplit('.').next().unwrap_or("") {
        "Entity" | "EnderDragon" => false,
        "Projectile" | "AbstractArrow" => {
            report.tags.contains(
                "minecraft:entity_type",
                "minecraft:redirectable_projectile",
                e.kind,
            ) && !(d.pick_owner.ends_with(".AbstractArrow")
                && number(report, e, d, "IN_GROUND").unwrap_or(0.) != 0.)
        }
        _ => true,
    }
}
/// AABB.clip does not use VoxelShape's 0.001 inside-probe shortcut.
pub fn clip_box(b: [f64; 6], start: DVec3, end: DVec3) -> Option<DVec3> {
    let delta = end - start;
    let mut best = 1.;
    let mut hit = false;
    for axis in 0..3 {
        if delta[axis].abs() <= 1e-7 {
            continue;
        }
        let t = (b[axis + if delta[axis] > 0. { 0 } else { 3 }] - start[axis]) / delta[axis];
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let pu = start[u] + t * delta[u];
        let pv = start[v] + t * delta[v];
        if t > 0.
            && t < best
            && pu > b[u] - 1e-7
            && pu < b[u + 3] + 1e-7
            && pv > b[v] - 1e-7
            && pv < b[v + 3] + 1e-7
        {
            best = t;
            hit = true;
        }
    }
    hit.then(|| start + delta * best)
}
pub fn target(
    report: &SessionReport,
    start: DVec3,
    direction: DVec3,
    block_reach: f64,
    entity_reach: f64,
) -> Option<Target> {
    if !start.is_finite()
        || !direction.is_finite()
        || !block_reach.is_finite()
        || !entity_reach.is_finite()
    {
        return None;
    }
    let reach = block_reach.max(entity_reach).clamp(0., 64.);
    let block = super::picking::block(&report.world, start, direction, reach);
    let mut distance = block.map_or(reach * reach, |h| start.distance_squared(h.position));
    let end = start + direction.normalize_or_zero() * distance.sqrt();
    let mut picked = None;
    // Stable ordering. Exact ties in vanilla depend on Level entity traversal order.
    let mut entities: Vec<_> = report.entities.entities.values().collect();
    entities.sort_by_key(|e| e.id);
    for e in entities {
        if Some(e.id) == report.world.player_id {
            continue;
        }
        let Some(d) = catalog().types.get(e.kind as usize) else {
            continue;
        };
        if !pickable(report, e, d) {
            continue;
        }
        let Some(mut b) = bounds(report, e, d) else {
            continue;
        };
        let radius = if d.radius_owner.ends_with(".Projectile") {
            1.
        } else {
            0.
        };
        for a in 0..3 {
            b[a] -= radius;
            b[a + 3] += radius;
        }
        let inside = (0..3).all(|a| start[a] >= b[a] && start[a] < b[a + 3]);
        let location = clip_box(b, start, end);
        if inside
            || location.is_some_and(|p| start.distance_squared(p) < distance || distance == 0.)
        {
            let position = location.unwrap_or(start);
            distance = if inside {
                0.
            } else {
                start.distance_squared(position)
            };
            picked = Some(EntityHit {
                id: e.id,
                uuid: e.uuid,
                position,
                relative: position - e.pose.position,
            });
        }
    }
    if let Some(hit) = picked {
        if start.distance_squared(hit.position)
            < block.map_or(reach * reach, |b| start.distance_squared(b.position))
        {
            return (start.distance_squared(hit.position) < entity_reach * entity_reach)
                .then_some(Target::Entity(hit));
        }
    }
    block
        .filter(|h| start.distance_squared(h.position) < block_reach * block_reach)
        .map(Target::Block)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> serde_json::Value {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/entity-picking.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        serde_json::from_slice(&raw).unwrap()
    }
    fn entity(name: &str, id: i32, position: DVec3) -> Entity {
        let ids = crate::assets::builtins::get().unwrap();
        let kind = (0..158)
            .find(|&kind| ids.name("minecraft:entity_type", kind) == Some(name))
            .unwrap();
        Entity {
            id,
            uuid: [id as u8; 16],
            kind,
            spawn_data: 2,
            pose: crate::net::entity::Pose {
                position,
                ..Default::default()
            },
            previous: Default::default(),
            head_yaw: 0.,
            on_ground: false,
        }
    }
    fn metadata(report: &mut SessionReport, id: i32, index: u8, value: Value) {
        std::sync::Arc::make_mut(
            std::sync::Arc::make_mut(&mut report.metadata)
                .entities
                .entry(id)
                .or_default(),
        )
        .insert(index, value);
    }
    #[test]
    fn projectile_selection_tracks_server_tags_and_arrow_in_ground_metadata() {
        let mut report = SessionReport::default();
        let e = entity("minecraft:arrow", 7, DVec3::ZERO);
        let d = catalog()
            .types
            .iter()
            .find(|d| d.name == "minecraft:arrow")
            .unwrap();
        assert!(!pickable(&report, &e, d));
        std::sync::Arc::make_mut(&mut report.tags)
            .registries
            .insert(
                "minecraft:entity_type".into(),
                std::sync::Arc::new(BTreeMap::from([(
                    "minecraft:redirectable_projectile".into(),
                    vec![e.kind as i32],
                )])),
            );
        assert!(pickable(&report, &e, d));
        metadata(
            &mut report,
            7,
            d.indices["IN_GROUND"],
            Value::Encoded {
                serializer: crate::net::maps::reference().serializers["BOOLEAN"],
                bytes: vec![1],
            },
        );
        assert!(!pickable(&report, &e, d));
    }
    #[test]
    fn exported_pose_dimensions_match_native_bounds_and_small_armor_stand() {
        let data = reference();
        let mut report = SessionReport::default();
        let mut checked = 0;
        for row in data["types"].as_array().unwrap() {
            let name = row["name"].as_str().unwrap();
            if name.ends_with("item_frame") {
                continue;
            }
            let e = entity(name, 1, DVec3::new(-31., -22., 47.));
            let d = catalog().types.iter().find(|d| d.name == name).unwrap();
            for (pose, expected) in row["pose_boxes"].as_array().unwrap().iter().enumerate() {
                if expected.is_null() {
                    continue;
                }
                let mut bytes = Vec::new();
                crate::net::codec::varint(pose as i32, &mut bytes);
                metadata(
                    &mut report,
                    1,
                    6,
                    Value::Encoded {
                        serializer: crate::net::maps::reference().serializers["POSE"],
                        bytes,
                    },
                );
                let b = bounds(&report, &e, d).unwrap();
                for axis in 0..6 {
                    assert_eq!(
                        b[axis].to_bits(),
                        u64::from_str_radix(row["pose_box_bits"][pose][axis].as_str().unwrap(), 16)
                            .unwrap(),
                        "{name} pose {pose} axis {axis}"
                    );
                }
                checked += 1;
            }
            std::sync::Arc::make_mut(&mut report.metadata)
                .entities
                .clear();
        }
        assert_eq!(checked, 1956);
        let e = entity("minecraft:armor_stand", 1, DVec3::ZERO);
        let d = catalog()
            .types
            .iter()
            .find(|d| d.name == "minecraft:armor_stand")
            .unwrap();
        metadata(
            &mut report,
            1,
            d.indices["DATA_CLIENT_FLAGS"],
            Value::Byte(1),
        );
        let b = bounds(&report, &e, d).unwrap();
        assert_eq!(b[3] - b[0], 0.25);
        assert_eq!(b[4], f64::from(0.9875f32));
        metadata(
            &mut report,
            1,
            d.indices["DATA_CLIENT_FLAGS"],
            Value::Byte(16),
        );
        assert!(!pickable(&report, &e, d));
    }
    #[test]
    fn entity_precedes_block_and_marker_or_range_do_not_click_through() {
        let mut report = SessionReport::default();
        report.world.dimension = Some(crate::net::registry::Dimension {
            name: "minecraft:overworld".into(),
            min_y: 0,
            height: 16,
            logical_height: 16,
            skylight: true,
            ceiling: false,
        });
        report
            .world
            .insert(crate::net::chunk::Chunk {
                x: 0,
                z: 0,
                block_entities: Vec::new(),
                sections: vec![crate::net::chunk::Section {
                    non_air: 0,
                    fluids: 0,
                    blocks: vec![0; 4096],
                    biomes: vec![0; 64],
                }],
            })
            .unwrap();
        let stone = crate::assets::builtins::get()
            .unwrap()
            .default_state("minecraft:stone")
            .unwrap();
        report.world.set_block(0, 0, 4, stone);
        let e = entity("minecraft:armor_stand", 2, DVec3::new(0.5, 0., 2.5));
        report.entities.entities.insert(2, e);
        let origin = DVec3::new(0.5, 0.5, 0.);
        let pick = |report: &SessionReport, range| target(report, origin, DVec3::Z, 4.5, range);
        assert!(matches!(pick(&report,3.),Some(Target::Entity(hit)) if hit.id==2));
        assert!(
            pick(&report, 2.).is_none(),
            "Out-of-reach entity must not click the block behind it"
        );
        metadata(&mut report, 2, 15, Value::Byte(16));
        assert!(matches!(pick(&report, 3.), Some(Target::Block(_))));
        metadata(&mut report, 2, 15, Value::Byte(0));
        report.world.set_block(0, 0, 1, stone);
        assert!(matches!(pick(&report, 3.), Some(Target::Block(_))));
        report.world.set_block(0, 0, 1, 0);
        report.entities.entities.insert(
            2,
            entity("minecraft:item_frame", 2, DVec3::new(0.5, 0.5, 2.96875)),
        );
        metadata(&mut report, 2, 0, Value::Byte(32));
        assert!(
            matches!(pick(&report, 3.), Some(Target::Entity(_))),
            "Invisible frames remain pickable"
        );
    }
    #[test]
    fn all_frame_directions_map_sizes_and_world_border_coordinates_match_jar() {
        for row in reference()["frames"].as_array().unwrap() {
            let b = frame_box(
                [row["x"].as_i64().unwrap() as i32, -22, 47],
                row["face"].as_i64().unwrap() as i32,
                row["map"].as_bool().unwrap(),
            )
            .unwrap();
            for a in 0..6 {
                assert_eq!(b[a], row["box"][a].as_f64().unwrap());
            }
        }
    }
    #[test]
    fn thousand_original_aabb_clips_match() {
        for (i, row) in reference()["clips"].as_array().unwrap().iter().enumerate() {
            let b = std::array::from_fn(|a| row["box"][a].as_f64().unwrap());
            let start = DVec3::from_array(std::array::from_fn(|a| row["ray"][a].as_f64().unwrap()));
            let end =
                DVec3::from_array(std::array::from_fn(|a| row["ray"][a + 3].as_f64().unwrap()));
            let hit = clip_box(b, start, end);
            if row.get("hit").is_none() {
                assert!(hit.is_none(), "{i}");
            } else {
                let expected =
                    DVec3::from_array(std::array::from_fn(|a| row["hit"][a].as_f64().unwrap()));
                assert!(hit.unwrap().distance(expected) < 1e-10, "{i}");
            }
        }
    }
}
