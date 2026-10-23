use super::{height, Mesh};
use glam::Vec3;

#[derive(Clone)]
pub struct Volume {
    pub size: [usize; 3],
    pub origin: [i32; 3],
    blocks: Vec<u16>,
}
impl Volume {
    pub fn new(size: [usize; 3], origin: [i32; 3]) -> Self {
        assert!(size.iter().all(|&s| s > 0 && s <= 256));
        Self {
            size,
            origin,
            blocks: vec![0; size.iter().product()],
        }
    }
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: u16) {
        self.blocks[(y * self.size[2] + z) * self.size[0] + x] = block;
    }
    pub fn get(&self, p: [i32; 3]) -> u16 {
        if (0..3).any(|d| p[d] < 0 || p[d] >= self.size[d] as i32) {
            return 0;
        }
        self.blocks[(p[1] as usize * self.size[2] + p[2] as usize) * self.size[0] + p[0] as usize]
    }
    pub fn terrain(x: i32, z: i32, width: usize) -> Self {
        let mut v = Self::new([width, 96, width], [x, 0, z]);
        for a in 0..width {
            for b in 0..width {
                let h = height(x + a as i32, z + b as i32).clamp(1, 95) as usize;
                for y in 0..h {
                    v.set(
                        a,
                        y,
                        b,
                        if y + 1 == h {
                            1
                        } else if y + 4 >= h {
                            2
                        } else {
                            3
                        },
                    );
                }
            }
        }
        v
    }
}
/// Greedy rectangles merge coplanar faces of equal material and orientation.
/// Halo sampling removes faces shared by adjacent terrain tiles.
pub fn greedy(volume: &Volume, terrain_halo: bool) -> Mesh {
    let origin = Vec3::from_array(volume.origin.map(|x| x as f32));
    let mut mesh = Mesh::new(
        origin,
        origin + Vec3::from_array(volume.size.map(|x| x as f32)),
        0,
    );
    let sample = |p: [i32; 3]| -> u16 {
        if terrain_halo
            && p[1] >= 0
            && p[1] < 96
            && (p[0] < 0
                || p[2] < 0
                || p[0] >= volume.size[0] as i32
                || p[2] >= volume.size[2] as i32)
        {
            let h = height(volume.origin[0] + p[0], volume.origin[2] + p[2]);
            if p[1] >= h {
                0
            } else if p[1] + 1 == h {
                1
            } else if p[1] + 4 >= h {
                2
            } else {
                3
            }
        } else {
            volume.get(p)
        }
    };
    for d in 0..3 {
        let u = (d + 1) % 3;
        let v = (d + 2) % 3;
        let nu = volume.size[u];
        let nv = volume.size[v];
        let mut mask = vec![0i32; nu * nv];
        for slice in -1..volume.size[d] as i32 {
            for j in 0..nv {
                for i in 0..nu {
                    let mut p = [0; 3];
                    p[d] = slice;
                    p[u] = i as i32;
                    p[v] = j as i32;
                    let a = sample(p);
                    p[d] += 1;
                    let b = sample(p);
                    mask[j * nu + i] = if a != 0 && b == 0 && slice >= 0 {
                        a as i32
                    } else if a == 0 && b != 0 && slice + 1 < volume.size[d] as i32 {
                        -(b as i32)
                    } else {
                        0
                    };
                }
            }
            for j in 0..nv {
                let mut i = 0;
                while i < nu {
                    let face = mask[j * nu + i];
                    if face == 0 {
                        i += 1;
                        continue;
                    }
                    let mut w = 1;
                    while i + w < nu && mask[j * nu + i + w] == face {
                        w += 1;
                    }
                    let mut h = 1;
                    'grow: while j + h < nv {
                        for k in 0..w {
                            if mask[(j + h) * nu + i + k] != face {
                                break 'grow;
                            }
                        }
                        h += 1;
                    }
                    let mut p = volume.origin.map(|x| x as f32);
                    p[d] += (slice + 1) as f32;
                    p[u] += i as f32;
                    p[v] += j as f32;
                    let mut du = [0.; 3];
                    du[u] = w as f32;
                    let mut dv = [0.; 3];
                    dv[v] = h as f32;
                    let add = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
                        std::array::from_fn(|k| a[k] + b[k])
                    };
                    let points = if face > 0 {
                        [p, add(p, du), add(add(p, du), dv), add(p, dv)]
                    } else {
                        [p, add(p, dv), add(add(p, du), dv), add(p, du)]
                    };
                    let top = d == 1 && face > 0;
                    let layer = match face.unsigned_abs() {
                        1 if top => 0,
                        1 if d == 1 => 2,
                        1 => 1,
                        2 => 2,
                        _ => 3,
                    };
                    let shade = if d == 1 {
                        if top {
                            1.
                        } else {
                            0.5
                        }
                    } else if d == 0 {
                        0.78
                    } else {
                        0.65
                    };
                    let tint = if layer == 0 {
                        [0.46, 0.77, 0.27]
                    } else {
                        [1.; 3]
                    };
                    // Image V grows downward. All vertical faces must therefore
                    // use -worldY, independent of greedy sweep axis and winding.
                    let uvs = points.map(|p| match d {
                        0 => [p[2], -p[1]],
                        1 => [p[0], p[2]],
                        _ => [p[0], -p[1]],
                    });
                    mesh.quad_uv_shaded(points, uvs, layer, tint, shade);
                    for y in j..j + h {
                        for x in i..i + w {
                            mask[y * nu + x] = 0;
                        }
                    }
                    i += w;
                }
            }
        }
    }
    mesh.tighten_bounds();
    mesh
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grass_texture_top_stays_at_top_on_all_four_sides() {
        let mut volume = Volume::new([2, 3, 2], [-17, 12, 25]);
        for x in 0..2 {
            for y in 0..3 {
                for z in 0..2 {
                    volume.set(x, y, z, 1);
                }
            }
        }
        let mesh = greedy(&volume, false);
        let sides: Vec<_> = mesh
            .vertices
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|quad| quad[0].layer == 1)
            .collect();
        assert_eq!(sides.len(), 4);
        for side in sides {
            for vertex in side {
                assert_eq!(vertex.uv[1], -vertex.position[1]);
            }
            let normal = Vec3::from_array(side[0].normal);
            for a in side {
                for b in side {
                    if a.position[1] != b.position[1]
                        && (Vec3::from_array(a.position) - Vec3::from_array(b.position))
                            .cross(Vec3::Y)
                            .length()
                            < 0.001
                    {
                        assert_eq!(
                            a.uv[0], b.uv[0],
                            "Side texture must not rotate by 90 degrees"
                        );
                    }
                }
            }
            assert_eq!(normal.y, 0.);
        }
    }
    #[test]
    fn triangle_winding_points_away_from_solid_cube() {
        let mut v = Volume::new([1; 3], [0; 3]);
        v.set(0, 0, 0, 3);
        let mesh = greedy(&v, false);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = triangle.map(|i| Vec3::from_array(mesh.vertices[i as usize].position));
            let normal = (b - a).cross(c - a);
            assert!(normal.dot((a + b + c) / 3. - Vec3::splat(0.5)) > 0.);
        }
    }
    #[test]
    fn solid_cube_merges_to_six_faces() {
        let mut v = Volume::new([8; 3], [0; 3]);
        for x in 0..8 {
            for y in 0..8 {
                for z in 0..8 {
                    v.set(x, y, z, 3);
                }
            }
        }
        let m = greedy(&v, false);
        assert_eq!(m.indices.len(), 36);
        assert_eq!(m.vertices.len(), 24);
    }
    #[test]
    fn empty_volume_has_no_mesh() {
        assert!(greedy(&Volume::new([16; 3], [0; 3]), false)
            .indices
            .is_empty());
    }
    #[test]
    fn adjacent_blocks_remove_internal_face() {
        let mut v = Volume::new([2, 1, 1], [0; 3]);
        v.set(0, 0, 0, 1);
        v.set(1, 0, 0, 2);
        assert_eq!(greedy(&v, false).indices.len(), 60);
    }
}
