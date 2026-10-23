//! Official outline-box clipping for block interactions, independent of camera
//! smoothing, camera collision shapes and render LOD. Empty context baseline;
//! entity-sensitive and world-border shapes remain separate work.
use glam::DVec3;
#[derive(Clone, Copy, Debug)]
pub struct BlockHit {
    pub block: [i32; 3],
    pub position: DVec3,
    /// Down, Up, North, South, West, East (protocol Direction order).
    pub face: u8,
    pub inside: bool,
    pub border: bool,
}
fn nearest_opposite(delta: DVec3) -> u8 {
    let mut face = 2;
    let mut best = f32::MIN;
    let d = delta.as_vec3();
    for (i, n) in [
        [0., -1., 0.],
        [0., 1., 0.],
        [0., 0., -1.],
        [0., 0., 1.],
        [-1., 0., 0.],
        [1., 0., 0.],
    ]
    .into_iter()
    .enumerate()
    {
        let dot = d.dot(glam::Vec3::from_array(n));
        if dot > best {
            best = dot;
            face = i as u8;
        }
    }
    face ^ 1
}
pub fn clip(boxes: &[[f64; 6]], block: [i32; 3], start: DVec3, end: DVec3) -> Option<BlockHit> {
    let delta = end - start;
    if !start.is_finite() || !end.is_finite() || delta.length_squared() < 1e-7 {
        return None;
    }
    let origin = DVec3::from_array(block.map(f64::from));
    let probe = start + delta * 0.001 - origin;
    if boxes
        .iter()
        .any(|b| (0..3).all(|a| probe[a] >= b[a] && probe[a] < b[a + 3]))
    {
        return Some(BlockHit {
            block,
            position: start + delta * 0.001,
            face: nearest_opposite(delta),
            inside: true,
            border: false,
        });
    }
    let local = start - origin;
    let mut best = 1.;
    let mut direction = None;
    for b in boxes {
        // AABB.getDirection tests X, then Y, then Z, retaining the first tie.
        for (axis, negative, positive) in [(0, 4, 5), (1, 0, 1), (2, 2, 3)] {
            if delta[axis].abs() <= 1e-7 {
                continue;
            }
            let face = if delta[axis] > 0. { negative } else { positive };
            let plane = b[axis + if delta[axis] > 0. { 0 } else { 3 }];
            let t = (plane - local[axis]) / delta[axis];
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            let pu = local[u] + t * delta[u];
            let pv = local[v] + t * delta[v];
            if t > 0.
                && t < best
                && pu > b[u] - 1e-7
                && pu < b[u + 3] + 1e-7
                && pv > b[v] - 1e-7
                && pv < b[v + 3] + 1e-7
            {
                best = t;
                direction = Some(face);
            }
        }
    }
    direction.map(|face| BlockHit {
        block,
        position: start + delta * best,
        face,
        inside: false,
        border: false,
    })
}
pub fn block(
    world: &super::remote::RemoteWorld,
    start: DVec3,
    direction: DVec3,
    reach: f64,
) -> Option<BlockHit> {
    if !start.is_finite() || !direction.is_finite() || !reach.is_finite() || reach <= 0. {
        return None;
    }
    let end = start + direction.normalize_or_zero() * reach.min(64.);
    let delta = end - start;
    if delta.length_squared() < 1e-7 {
        return None;
    }
    let physics = crate::assets::collision::outline().ok()?;
    // Traverse touched cells, including neighbors for protruding outline boxes.
    let mut cell = (start - delta * 1e-7).floor().as_ivec3();
    let finish = (end + delta * 1e-7).floor().as_ivec3();
    let step = delta.signum().as_ivec3();
    let mut next = DVec3::splat(f64::INFINITY);
    let mut interval = DVec3::splat(f64::INFINITY);
    for a in 0..3 {
        if delta[a].abs() > 1e-12 {
            interval[a] = 1. / delta[a].abs();
            next[a] = (cell[a] as f64 + if step[a] > 0 { 1. } else { 0. } - start[a]) / delta[a];
        }
    }
    let mut visited = std::collections::HashSet::new();
    let mut best: Option<BlockHit> = None;
    for _ in 0..256 {
        for y in -1..=1 {
            for z in -1..=1 {
                for x in -1..=1 {
                    let p = (cell + glam::IVec3::new(x, y, z)).to_array();
                    if !visited.insert(p) {
                        continue;
                    }
                    let Some(state) = world
                        .block(p[0], p[1], p[2])
                        .and_then(|id| physics.state(id))
                    else {
                        continue;
                    };
                    if let Some(hit) = clip(&state.boxes, p, start, end) {
                        if best.is_none_or(|old| {
                            start.distance_squared(hit.position)
                                < start.distance_squared(old.position)
                        }) {
                            best = Some(hit);
                        }
                    }
                }
            }
        }
        if cell == finish {
            break;
        }
        let axis = if next.x < next.y && next.x < next.z {
            0
        } else if next.y < next.z {
            1
        } else {
            2
        };
        if next[axis] > 1. + 1e-7 {
            break;
        }
        next[axis] += interval[axis];
        cell[axis] += step[axis];
    }
    best
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    fn outline_hits_glass_and_rejects_non_finite_or_out_of_range_rays() {
        let cube = [[0., 0., 0., 1., 1., 1.]];
        assert!(clip(&cube, [0; 3], DVec3::splat(f64::NAN), DVec3::ONE).is_none());
        let ids = crate::assets::builtins::get().unwrap();
        let glass = ids.default_state("minecraft:glass").unwrap();
        assert!(crate::assets::collision::visual()
            .unwrap()
            .state(glass)
            .unwrap()
            .boxes
            .is_empty());
        let outline = crate::assets::collision::outline().unwrap();
        let hit = clip(
            &outline.state(glass).unwrap().boxes,
            [0, 0, 1],
            DVec3::new(0.5, 0.5, 0.),
            DVec3::new(0.5, 0.5, 2.),
        )
        .unwrap();
        assert_eq!(hit.face, 2);
        assert_eq!(hit.position.z, 1.);
        assert!(clip(
            &cube,
            [0, 0, 1],
            DVec3::new(0.5, 0.5, 0.),
            DVec3::new(0.5, 0.5, 0.9)
        )
        .is_none());
    }
    #[test]
    fn original_shape_clip_matches_1800_partial_block_rays() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/use-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let data: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let physics = crate::assets::collision::outline().unwrap();
        let mut world = super::super::remote::RemoteWorld {
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
                block_entities: Vec::new(),
                sections: vec![crate::net::chunk::Section {
                    non_air: 0,
                    fluids: 0,
                    blocks: vec![0; 4096],
                    biomes: vec![0; 64],
                }],
            })
            .unwrap();
        for (i, row) in data["rays"].as_array().unwrap().iter().enumerate() {
            let vec = |key: &str| {
                DVec3::from_array(std::array::from_fn(|a| row[key][a].as_f64().unwrap()))
            };
            let hit = clip(
                &physics
                    .state(row["state"].as_u64().unwrap() as u32)
                    .unwrap()
                    .boxes,
                [-31, -22, 47],
                vec("start"),
                vec("end"),
            );
            world.set_block(-31, -22, 47, row["state"].as_u64().unwrap() as u32);
            let delta = vec("end") - vec("start");
            let traversed = block(&world, vec("start"), delta, delta.length());
            assert_eq!(traversed.is_some(), hit.is_some(), "World traversal {i}");
            if let (Some(a), Some(b)) = (traversed, hit) {
                assert_eq!(a.block, b.block, "{i}");
                assert_eq!(a.face, b.face, "{i}");
                assert!(a.position.distance(b.position) < 1e-9, "{i}");
            }
            if row.get("hit").is_none() {
                assert!(hit.is_none(), "{i}");
                continue;
            }
            let hit = hit.unwrap_or_else(|| panic!("Missing hit {i}"));
            assert!(hit.position.distance(vec("hit")) < 1e-9, "{i}: {hit:?}");
            assert_eq!(hit.face, row["face"].as_u64().unwrap() as u8, "{i}");
            assert_eq!(hit.inside, row["inside"].as_bool().unwrap(), "{i}");
        }
    }
}
