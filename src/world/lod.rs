use super::{
    height,
    mesh::{greedy, Volume},
    Mesh,
};
use glam::{Mat4, Vec3, Vec4};
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Tile {
    pub x: i32,
    pub z: i32,
    pub size: i32,
}
impl Tile {
    pub fn distance(self, eye: Vec3) -> f32 {
        let dx = (self.x as f32 - eye.x)
            .max(0.)
            .max(eye.x - (self.x + self.size) as f32);
        let dz = (self.z as f32 - eye.z)
            .max(0.)
            .max(eye.z - (self.z + self.size) as f32);
        dx.hypot(dz)
    }
    fn children(self) -> [Self; 4] {
        let s = self.size / 2;
        [(0, 0), (s, 0), (0, s), (s, s)].map(|(x, z)| Self {
            x: self.x + x,
            z: self.z + z,
            size: s,
        })
    }
}
/// Non-overlapping quadtree cover with hysteresis and ≤2:1 adjacent tile sizes.
/// Vertical skirts hide height sampling discontinuities across resolution changes.
pub struct Lod {
    previous_splits: HashSet<Tile>,
    pub radius: i32,
}
impl Lod {
    pub fn new(radius: i32) -> Self {
        Self {
            previous_splits: HashSet::new(),
            radius: (radius.clamp(256, 4096) as u32).next_power_of_two() as i32,
        }
    }
    pub fn select(&mut self, eye: Vec3) -> Vec<Tile> {
        let span = self.radius * 4;
        let center_x = (eye.x as i32).div_euclid(self.radius) * self.radius;
        let center_z = (eye.z as i32).div_euclid(self.radius) * self.radius;
        let root = Tile {
            x: center_x - self.radius * 2,
            z: center_z - self.radius * 2,
            size: span,
        };
        let mut splits = HashSet::new();
        let mut leaves = Vec::new();
        let mut pending = vec![root];
        while let Some(tile) = pending.pop() {
            let factor = if self.previous_splits.contains(&tile) {
                4.5
            } else {
                4.0
            };
            if tile.size > 32 && tile.distance(eye) < (tile.size as f32 * factor).max(384.) {
                splits.insert(tile);
                pending.extend(tile.children());
            } else {
                leaves.push(tile);
            }
        }
        // Enforce the geometric continuity constraint even after a camera jump.
        loop {
            let mut refine = HashSet::new();
            for (i, a) in leaves.iter().enumerate() {
                for b in &leaves[i + 1..] {
                    let touch_x = (a.x + a.size == b.x || b.x + b.size == a.x)
                        && a.z < b.z + b.size
                        && b.z < a.z + a.size;
                    let touch_z = (a.z + a.size == b.z || b.z + b.size == a.z)
                        && a.x < b.x + b.size
                        && b.x < a.x + a.size;
                    if (touch_x || touch_z) && a.size.max(b.size) > 2 * a.size.min(b.size) {
                        refine.insert(if a.size > b.size { *a } else { *b });
                    }
                }
            }
            if refine.is_empty() {
                break;
            }
            leaves = leaves
                .into_iter()
                .flat_map(|t| {
                    if refine.contains(&t) {
                        splits.insert(t);
                        t.children().to_vec()
                    } else {
                        vec![t]
                    }
                })
                .collect();
        }
        leaves.sort_by(|a, b| a.distance(eye).total_cmp(&b.distance(eye)));
        self.previous_splits = splits;
        leaves
    }
}
// Resolution is a property of the tile, so camera motion cannot thrash the cache.
// 32-block tiles keep complete block faces to at least 384 blocks. The next
// three rings sample every 1, 2 and 4 blocks before switching to coarse terrain.
pub fn sample_step(tile: Tile) -> i32 {
    match tile.size {
        0..=64 => 1,
        65..=128 => 2,
        129..=256 => 4,
        _ => (tile.size / 64).max(1),
    }
}
pub fn tile_mesh(tile: Tile, _eye: Vec3) -> Mesh {
    if tile.size == 32 {
        return greedy(&Volume::terrain(tile.x, tile.z, 32), true);
    }
    let step = sample_step(tile);
    let level = step.ilog2() as u8 + 1;
    let mut mesh = Mesh::new(
        Vec3::new(tile.x as f32, -64., tile.z as f32),
        Vec3::new(
            (tile.x + tile.size) as f32,
            96.,
            (tile.z + tile.size) as f32,
        ),
        level,
    );
    let h = |x, z| height(x, z) as f32;
    // Shared grid vertices preserve all samples while avoiding four copies of
    // every vertex. World-space UVs remain continuous across tile boundaries.
    let cells = tile.size / step;
    let row = (cells + 1) as u32;
    for iz in 0..=cells {
        for ix in 0..=cells {
            let x = tile.x + ix * step;
            let z = tile.z + iz * step;
            let normal = Vec3::new(
                h(x - step, z) - h(x + step, z),
                (2 * step) as f32,
                h(x, z - step) - h(x, z + step),
            )
            .normalize();
            mesh.vertices.push(super::Vertex {
                position: [x as f32, h(x, z), z as f32],
                uv: [x as f32, z as f32],
                tint: [0.46, 0.77, 0.27],
                layer: 0,
                lod: level as u32,
                normal: normal.to_array(),
                origin: [0.; 3],
                shade: 1.,
            });
        }
    }
    for iz in 0..cells as u32 {
        for ix in 0..cells as u32 {
            let a = iz * row + ix;
            let b = a + row;
            mesh.indices
                .extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        }
    }
    for i in (0..tile.size).step_by(step as usize) {
        let edges = [
            ((tile.x + i, tile.z), (tile.x + i + step, tile.z)),
            (
                (tile.x + tile.size, tile.z + i),
                (tile.x + tile.size, tile.z + i + step),
            ),
            (
                (tile.x + i + step, tile.z + tile.size),
                (tile.x + i, tile.z + tile.size),
            ),
            ((tile.x, tile.z + i + step), (tile.x, tile.z + i)),
        ];
        for ((x, z), (xx, zz)) in edges {
            let a = h(x, z);
            let b = h(xx, zz);
            let drop = (step * 4).max(8) as f32;
            mesh.quad_uv_shaded(
                [
                    [x as f32, a, z as f32],
                    [xx as f32, b, zz as f32],
                    [xx as f32, b - drop, zz as f32],
                    [x as f32, a - drop, z as f32],
                ],
                [[0., 0.], [step as f32, 0.], [step as f32, drop], [0., drop]],
                2,
                [1.; 3],
                0.6,
            );
        }
    }
    mesh.tighten_bounds();
    mesh
}
pub fn build(tiles: &[Tile], eye: Vec3) -> Vec<Mesh> {
    tiles.par_iter().map(|&t| tile_mesh(t, eye)).collect()
}

/// Reuses unchanged CPU meshes; cache lifetime is bounded by current coverage.
pub struct Streamer {
    lod: Lod,
    cache: HashMap<Tile, Arc<Mesh>>,
}
impl Streamer {
    pub fn new(radius: i32) -> Self {
        Self {
            lod: Lod::new(radius),
            cache: HashMap::new(),
        }
    }
    pub fn update(&mut self, eye: Vec3) -> Vec<Arc<Mesh>> {
        let radius = self.lod.radius as f32;
        let keys: Vec<_> = self
            .lod
            .select(eye)
            .into_iter()
            .filter(|tile| tile.distance(eye) <= radius)
            .collect();
        let missing: Vec<_> = keys
            .par_iter()
            .filter(|key| !self.cache.contains_key(key))
            .map(|&key| (key, Arc::new(tile_mesh(key, eye))))
            .collect();
        self.cache.extend(missing);
        let live: HashSet<_> = keys.iter().copied().collect();
        self.cache.retain(|key, _| live.contains(key));
        keys.into_iter()
            .map(|key| self.cache[&key].clone())
            .collect()
    }
}

pub struct Frustum {
    planes: [Vec4; 6],
}
impl Frustum {
    pub fn new(matrix: Mat4) -> Self {
        let r = matrix.transpose();
        let r0 = r.x_axis;
        let r1 = r.y_axis;
        let r2 = r.z_axis;
        let r3 = r.w_axis;
        let planes: [Vec4; 6] = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
        Self { planes }
    }
    pub fn visible(&self, min: Vec3, max: Vec3) -> bool {
        self.planes.iter().all(|p| {
            let corner = Vec3::new(
                if p.x >= 0. { max.x } else { min.x },
                if p.y >= 0. { max.y } else { min.y },
                if p.z >= 0. { max.z } else { min.z },
            );
            p.truncate().dot(corner) + p.w >= 0.
        })
    }
}
pub fn visible(matrix: Mat4, min: Vec3, max: Vec3) -> bool {
    Frustum::new(matrix).visible(min, max)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coverage_and_balance_hold_near_root_boundaries() {
        let mut lod = Lod::new(256);
        for eye in [
            Vec3::new(255., 70., 255.),
            Vec3::new(-257., 70., -1.),
            Vec3::new(8191., 70., -8193.),
        ] {
            let tiles = lod.select(eye);
            let min_x = tiles.iter().map(|t| t.x).min().unwrap();
            let max_x = tiles.iter().map(|t| t.x + t.size).max().unwrap();
            assert!(min_x as f32 <= eye.x - 256. && max_x as f32 >= eye.x + 256.);
            for (i, a) in tiles.iter().enumerate() {
                for b in &tiles[i + 1..] {
                    let adjacent = ((a.x + a.size == b.x || b.x + b.size == a.x)
                        && a.z < b.z + b.size
                        && b.z < a.z + a.size)
                        || ((a.z + a.size == b.z || b.z + b.size == a.z)
                            && a.x < b.x + b.size
                            && b.x < a.x + a.size);
                    if adjacent {
                        assert!(a.size.max(b.size) <= 2 * a.size.min(b.size));
                    }
                }
            }
        }
    }
    #[test]
    fn detail_extends_384_blocks_with_gradual_resolution() {
        let eye = Vec3::new(13., 70., -19.);
        let tiles = Lod::new(1024).select(eye);
        assert!(tiles
            .iter()
            .filter(|t| t.distance(eye) < 384.)
            .all(|t| t.size == 32));
        assert_eq!(
            [64, 128, 256, 512, 1024].map(|size| sample_step(Tile { x: 0, z: 0, size })),
            [1, 2, 4, 8, 16]
        );
    }
    #[test]
    fn sky_view_culls_bounds_of_actual_surface() {
        let eye = Vec3::new(0., 85., 0.);
        let matrix = Mat4::perspective_rh(70f32.to_radians(), 1.6, 0.05, 16384.)
            * Mat4::look_at_rh(eye, eye + Vec3::new(0., 1., -0.01), Vec3::Z);
        let mesh = tile_mesh(
            Tile {
                x: 0,
                z: 0,
                size: 32,
            },
            eye,
        );
        assert!(mesh.max.y < 85.);
        assert!(!visible(matrix, mesh.min, mesh.max));
    }
    #[test]
    fn stationary_camera_reuses_every_mesh() {
        let mut streamer = Streamer::new(256);
        let eye = Vec3::new(0., 70., 0.);
        let first = streamer.update(eye);
        let second = streamer.update(eye);
        assert_eq!(first.len(), second.len());
        assert!(first
            .iter()
            .zip(second.iter())
            .all(|(a, b)| Arc::ptr_eq(a, b)));
    }
    #[test]
    fn quadtree_covers_root_without_overlap() {
        let tiles = Lod::new(512).select(Vec3::new(-7., 70., 12.));
        assert_eq!(
            tiles.iter().map(|t| i64::from(t.size).pow(2)).sum::<i64>(),
            2048 * 2048
        );
        for (i, a) in tiles.iter().enumerate() {
            for b in &tiles[i + 1..] {
                assert!(
                    !(a.x < b.x + b.size
                        && b.x < a.x + a.size
                        && a.z < b.z + b.size
                        && b.z < a.z + a.size)
                );
            }
        }
    }
    #[test]
    fn camera_facing_culls_behind_eye() {
        let m = Mat4::perspective_rh(1., 1., 0.1, 100.)
            * Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        assert!(visible(
            m,
            Vec3::new(-1., -1., -10.),
            Vec3::new(1., 1., -8.)
        ));
        assert!(!visible(m, Vec3::new(-1., -1., 8.), Vec3::new(1., 1., 10.)));
    }
}
