//! Conservative polygon LOD: merge only complete, equal-material tiled planes.
//! Arbitrary models, transparency, maps, signs and entities remain untouched.
use super::Mesh;
use glam::Vec3;
use std::collections::{BTreeMap, HashSet};

pub fn variants(mesh: &Mesh) -> Vec<Mesh> {
    if mesh.section.is_none() || mesh.translucent || !mesh.vertices.len().is_multiple_of(4) {
        return Vec::new();
    }
    if mesh.indices.len() != mesh.vertices.len() / 4 * 6
        || mesh
            .indices
            .as_chunks::<6>()
            .0
            .iter()
            .enumerate()
            .any(|(quad, indices)| {
                let b = quad as u32 * 4;
                *indices != [b, b + 1, b + 2, b, b + 2, b + 3]
            })
    {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut last = mesh.indices.len();
    for span in [2, 4, 8, 16] {
        let next = merge(mesh, span);
        if span == 2 && next.indices.len() == mesh.indices.len() {
            return Vec::new();
        }
        if next.indices.len() * 4 <= last * 3 {
            last = next.indices.len();
            result.push(next);
        } else {
            result.push(Mesh::new(mesh.min, mesh.max, mesh.level));
        }
    }
    if result.iter().all(|m| m.indices.is_empty()) {
        result.clear();
    }
    result
}
fn merge(mesh: &Mesh, span: i32) -> Mesh {
    let mut groups: BTreeMap<Vec<u32>, Vec<usize>> = BTreeMap::new();
    let mut output = Mesh::new(mesh.min, mesh.max, mesh.level);
    output.section = mesh.section;
    let copy = |i: usize, out: &mut Mesh| {
        let base = out.vertices.len() as u32;
        out.vertices.extend_from_slice(&mesh.vertices[i..i + 4]);
        out.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    for (quad, vertices) in mesh.vertices.as_chunks::<4>().0.iter().enumerate() {
        let i = quad * 4;
        let min = vertices.iter().fold(Vec3::splat(f32::INFINITY), |a, v| {
            a.min(Vec3::from_array(v.position))
        });
        let max = vertices
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |a, v| {
                a.max(Vec3::from_array(v.position))
            });
        // Quarter-turn model matrices introduce ~1e-7 errors. Quantize only
        // faces already within 1e-5 of a complete integer-aligned unit square.
        let integral = vertices
            .iter()
            .all(|v| v.position.iter().all(|p| (p - p.round()).abs() <= 1e-5));
        let min = min.round();
        let max = max.round();
        let extent = max - min;
        let axis = (0..3)
            .find(|&a| extent[a] == 0. && extent[(a + 1) % 3] == 1. && extent[(a + 2) % 3] == 1.);
        let valid = axis.is_some()
            && integral
            && vertices.iter().all(|v| {
                v.origin == [0.; 3]
                    && v.lod == 0
                    && v.layer == vertices[0].layer
                    && v.tint == vertices[0].tint
                    && v.normal == vertices[0].normal
                    && v.shade == vertices[0].shade
                    && v.uv.iter().all(|&u| {
                        (u - u.round()).abs() <= 1e-5 && (u.round() == 0. || u.round() == 1.)
                    })
            });
        if !valid {
            copy(i, &mut output);
            continue;
        }
        let axis = axis.unwrap();
        let mut key = vec![
            axis as u32,
            if min[axis] == 0. {
                0
            } else {
                min[axis].to_bits()
            },
            vertices[0].layer,
        ];
        key.extend(vertices[0].tint.map(f32::to_bits));
        key.push(vertices[0].shade.to_bits());
        key.extend(vertices[0].normal.map(|n| {
            if n.round() == 0. {
                0
            } else {
                n.round().to_bits()
            }
        }));
        for v in vertices {
            key.extend(
                (Vec3::from_array(v.position).round() - min)
                    .to_array()
                    .map(|p| if p == 0. { 0 } else { p.to_bits() }),
            );
            key.extend(v.uv.map(|u| {
                if u.round() == 0. {
                    0
                } else {
                    u.round().to_bits()
                }
            }));
        }
        key.push((min[(axis + 1) % 3] as i32).div_euclid(span) as u32);
        key.push((min[(axis + 2) % 3] as i32).div_euclid(span) as u32);
        groups.entry(key).or_default().push(i);
    }
    for (key, quads) in groups {
        let axis = key[0] as usize;
        let cells: HashSet<_> = quads
            .iter()
            .map(|&i| {
                let min = mesh.vertices[i..i + 4]
                    .iter()
                    .fold(Vec3::splat(f32::INFINITY), |a, v| {
                        a.min(Vec3::from_array(v.position))
                    });
                (
                    min[(axis + 1) % 3].round() as i32,
                    min[(axis + 2) % 3].round() as i32,
                )
            })
            .collect();
        if quads.len() != (span * span) as usize || cells.len() != quads.len() {
            for i in quads {
                copy(i, &mut output);
            }
            continue;
        }
        let min = quads
            .iter()
            .flat_map(|&i| &mesh.vertices[i..i + 4])
            .fold(Vec3::splat(f32::INFINITY), |a, v| {
                a.min(Vec3::from_array(v.position))
            });
        let min = min.round();
        let first = &mesh.vertices[quads[0]..quads[0] + 4];
        let local_min = first.iter().fold(Vec3::splat(f32::INFINITY), |a, v| {
            a.min(Vec3::from_array(v.position))
        });
        let local_min = local_min.round();
        let base = output.vertices.len() as u32;
        for v in first {
            let mut v = *v;
            v.position =
                (min + (Vec3::from_array(v.position).round() - local_min) * span as f32).to_array();
            v.uv = v.uv.map(|u| u.round() * span as f32);
            output.vertices.push(v);
        }
        output
            .indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    output
}

/// Distance from the actual bounds protects the near end of large meshes.
pub fn distance(eye: Vec3, min: Vec3, max: Vec3) -> f32 {
    (min - eye).max(Vec3::ZERO).max(eye - max).length()
}
pub fn select(distance: f32, base: f32, previous: usize) -> usize {
    if base <= 0. {
        return 0;
    }
    let thresholds = [base, base * 2., base * 4., base * 8.];
    let mut level = previous.min(4);
    while level < 4 && distance > thresholds[level] * 1.08 {
        level += 1;
    }
    while level > 0 && distance < thresholds[level - 1] * 0.92 {
        level -= 1;
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_planes_reduce_without_holes_or_texture_stretch() {
        let mut mesh = Mesh::new(Vec3::ZERO, Vec3::new(16., 0., 16.), 0);
        mesh.section = Some([0; 3]);
        for z in 0..16 {
            for x in 0..16 {
                let (x, z) = (x as f32, z as f32);
                mesh.quad(
                    [
                        [x, 0., z],
                        [x, 0., z + 1.],
                        [x + 1., 0., z + 1.],
                        [x + 1., 0., z],
                    ],
                    [1.; 2],
                    3,
                    [1.; 3],
                );
            }
        }
        let levels = variants(&mesh);
        assert_eq!(
            levels
                .iter()
                .map(|m| m.indices.len() / 6)
                .collect::<Vec<_>>(),
            vec![64, 16, 4, 1]
        );
        for (level, span) in levels.iter().zip([2., 4., 8., 16.]) {
            assert_eq!(level.min, mesh.min);
            assert_eq!(level.max, mesh.max);
            assert!(level
                .vertices
                .iter()
                .all(|v| v.normal == [0., 1., 0.] && v.uv.iter().all(|u| *u == 0. || *u == span)));
        }
        let mut noisy = mesh.clone();
        for v in &mut noisy.vertices {
            for p in &mut v.position {
                *p += 1e-7;
            }
        }
        assert_eq!(
            variants(&noisy)
                .iter()
                .map(|m| m.indices.len() / 6)
                .collect::<Vec<_>>(),
            vec![64, 16, 4, 1]
        );
        mesh.vertices.drain(0..4);
        mesh.indices.truncate(mesh.indices.len() - 6);
        assert!(variants(&mesh)
            .iter()
            .filter(|m| !m.indices.is_empty())
            .all(|m| m.indices.len() >= 6));
        mesh.translucent = true;
        assert!(variants(&mesh).is_empty());
    }
    #[test]
    fn holes_material_boundaries_and_non_tiled_uvs_are_preserved() {
        let make = |hole: bool, varied: bool, sub_uv: bool| {
            let mut mesh = Mesh::new(Vec3::splat(-16.), Vec3::ZERO, 0);
            mesh.section = Some([-1; 3]);
            for z in -16..0 {
                for x in -16..0 {
                    if hole && x == -8 && z == -8 {
                        continue;
                    }
                    let layer = if varied { ((x + z) & 1) as u32 } else { 0 };
                    let (x, z) = (x as f32, z as f32);
                    mesh.quad(
                        [
                            [x, 0., z],
                            [x, 0., z + 1.],
                            [x + 1., 0., z + 1.],
                            [x + 1., 0., z],
                        ],
                        [if sub_uv { 0.5 } else { 1. }; 2],
                        layer,
                        [1.; 3],
                    );
                }
            }
            mesh
        };
        assert!(variants(&make(false, true, false)).is_empty());
        assert!(variants(&make(false, false, true)).is_empty());
        let mesh = make(true, false, false);
        for level in variants(&mesh).iter().filter(|m| !m.indices.is_empty()) {
            let mut area = 0.;
            for q in level.vertices.as_chunks::<4>().0.iter() {
                let min = q.iter().fold(Vec3::splat(f32::INFINITY), |a, v| {
                    a.min(Vec3::from_array(v.position))
                });
                let max = q.iter().fold(Vec3::splat(f32::NEG_INFINITY), |a, v| {
                    a.max(Vec3::from_array(v.position))
                });
                area += (max.x - min.x) * (max.z - min.z);
                assert!(!(min.x < -7. && max.x > -8. && min.z < -7. && max.z > -8.));
            }
            assert_eq!(area, 255.);
        }
    }
    #[test]
    fn all_polygon_lods_preserve_unlit_tint_and_low_shading_boundaries() {
        let mut mesh = Mesh::new(Vec3::ZERO, Vec3::new(16., 0., 16.), 0);
        mesh.section = Some([0; 3]);
        for z in 0..16 {
            for x in 0..16 {
                let shade = if x < 8 { 0.65 } else { 0.78 };
                let (x, z) = (x as f32, z as f32);
                mesh.quad_uv_shaded(
                    [
                        [x, 0., z],
                        [x, 0., z + 1.],
                        [x + 1., 0., z + 1.],
                        [x + 1., 0., z],
                    ],
                    [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                    1,
                    [0.46, 0.77, 0.27],
                    shade,
                );
            }
        }
        for level in variants(&mesh) {
            for v in &level.vertices {
                assert_eq!(v.tint, [0.46, 0.77, 0.27]);
                assert!(v.shade == 0.65 || v.shade == 0.78);
            }
            for quad in level.vertices.as_chunks::<4>().0 {
                let expected = if quad.iter().all(|v| v.position[0] <= 8.) {
                    0.65
                } else {
                    0.78
                };
                assert!(quad.iter().all(|v| v.shade == expected));
            }
        }
    }
    #[test]
    fn hysteresis_bounds_and_off() {
        assert_eq!(
            distance(Vec3::ZERO, Vec3::splat(-16.), Vec3::splat(16.)),
            0.
        );
        assert_eq!(select(110., 100., 0), 1);
        assert_eq!(select(99., 100., 1), 1);
        assert_eq!(select(90., 100., 1), 0);
        assert_eq!(select(10000., 0., 4), 0);
        assert_eq!(select(10000., 100., 0), 4);
    }
}
