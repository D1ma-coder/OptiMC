// SPDX-License-Identifier: LicenseRef-tr7zw-Protective
// Rust behavior port of 3d-Skin-Layers 1.11.4 by tr7zw.
// https://github.com/tr7zw/3d-skin-layers ; see LICENSES/SkinLayers.txt.
use serde::{Deserialize, Serialize};
const STEP: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];
fn axis(direction: usize) -> usize {
    [1, 1, 2, 2, 0, 0][direction]
}
fn size_uv([w, h, d]: [i32; 3], face: usize) -> [i32; 2] {
    match face {
        0 | 1 => [w, d],
        2 | 3 => [w, h],
        _ => [d, h],
    }
}
fn texture_uv([u, v]: [i32; 2], [bu, bv]: [i32; 2], [w, _, d]: [i32; 3], face: usize) -> [i32; 2] {
    let offset = match face {
        0 => [d, 0],
        1 => [d + w, 0],
        2 => [d, d],
        3 => [d + w + d, d],
        4 => [0, d],
        _ => [d + w, d],
    };
    [bu + offset[0] + u, bv + offset[1] + v]
}
fn xyz([u, v]: [i32; 2], [w, h, d]: [i32; 3], face: usize) -> [i32; 3] {
    match face {
        0 => [u, 0, d - 1 - v],
        1 => [u, h - 1, d - 1 - v],
        2 => [u, v, 0],
        3 => [w - 1 - u, v, d - 1],
        4 => [0, v, d - 1 - u],
        _ => [w - 1, v, u],
    }
}
fn uv([x, y, z]: [i32; 3], [w, _, d]: [i32; 3], face: usize) -> [i32; 2] {
    match face {
        0 | 1 => [x, d - 1 - z],
        2 => [x, y],
        3 => [w - 1 - x, y],
        4 => [d - 1 - z, y],
        _ => [z, y],
    }
}
fn on_face([u, v]: [i32; 2], [w, h]: [i32; 2]) -> bool {
    u >= 0 && v >= 0 && u < w && v < h
}
fn add(a: [i32; 3], b: [i32; 3], times: i32) -> [i32; 3] {
    std::array::from_fn(|i| a[i] + b[i] * times)
}
fn alpha(skin: &image::RgbaImage, [u, v]: [i32; 2]) -> u8 {
    if u < 0 || v < 0 || u >= skin.width() as i32 || v >= skin.height() as i32 {
        0
    } else {
        skin.get_pixel(u as u32, v as u32)[3]
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voxel {
    pub position: [i32; 3],
    pub uv: [i32; 2],
    pub hidden: u8,
    pub corners: Vec<[u8; 2]>,
}
/// The original wrapper's face-neighbor, transparency and corner rules. Positions
/// are relative integer cells; ModelPart placement/scaling is a separate step.
pub fn voxels(skin: &image::RgbaImage, size: [i32; 3], offset: [i32; 2], fast: bool) -> Vec<Voxel> {
    let mut output = Vec::new();
    for (face, face_step) in STEP.iter().enumerate() {
        let face_size = size_uv(size, face);
        for u in 0..face_size[0] {
            for v in 0..face_size[1] {
                let pixel = texture_uv([u, v], offset, size, face);
                let opacity = alpha(skin, pixel);
                if opacity == 0 {
                    continue;
                }
                let position = xyz([u, v], size, face);
                let solid = opacity == 255;
                let mut hidden = 0;
                let mut corners = Vec::new();
                let mut edge = false;
                let mut behind = false;
                for (direction, &step) in STEP.iter().enumerate() {
                    if axis(direction) == axis(face) {
                        continue;
                    }
                    let neighbor = add(position, step, 1);
                    let neighbor_uv = uv(neighbor, size, face);
                    if on_face(neighbor_uv, face_size) {
                        let opacity = alpha(skin, texture_uv(neighbor_uv, offset, size, face));
                        if opacity > 0 {
                            if !solid || opacity == 255 {
                                hidden |= 1 << direction;
                            }
                            continue;
                        }
                        let farther = add(neighbor, step, 1);
                        let farther_uv = uv(farther, size, face);
                        if !on_face(farther_uv, face_size) {
                            let opacity = alpha(
                                skin,
                                texture_uv(uv(farther, size, direction), offset, size, direction),
                            );
                            if opacity > 0 && (!solid || opacity == 255) {
                                hidden |= 1 << direction;
                            }
                        }
                    } else {
                        edge = true;
                        if alpha(
                            skin,
                            texture_uv(uv(position, size, direction), offset, size, direction),
                        ) > 0
                        {
                            behind = true;
                            hidden |= 1 << direction;
                            corners.push([(face ^ 1) as u8, direction as u8]);
                        } else if alpha(
                            skin,
                            texture_uv(
                                uv(add(position, *face_step, -1), size, direction),
                                offset,
                                size,
                                direction,
                            ),
                        ) > 0
                        {
                            behind = true;
                        }
                    }
                }
                if !edge || behind {
                    hidden |= 1 << (face ^ 1);
                }
                if fast {
                    hidden |= 1 << face;
                }
                corners.sort_unstable();
                output.push(Voxel {
                    position,
                    uv: pixel,
                    hidden,
                    corners,
                });
            }
        }
    }
    output
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub enabled: bool,
    pub enable_hat: bool,
    pub enable_jacket: bool,
    pub enable_left_sleeve: bool,
    pub enable_right_sleeve: bool,
    pub enable_left_pants: bool,
    pub enable_right_pants: bool,
    pub base_voxel_size: f32,
    pub body_voxel_width_size: f32,
    pub head_voxel_size: f32,
    pub render_distance_lod: f32,
    pub fast_render: bool,
    pub first_person_pixel_scaling: f32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            enable_hat: true,
            enable_jacket: true,
            enable_left_sleeve: true,
            enable_right_sleeve: true,
            enable_left_pants: true,
            enable_right_pants: true,
            base_voxel_size: 1.15,
            body_voxel_width_size: 1.05,
            head_voxel_size: 1.18,
            render_distance_lod: 14.,
            fast_render: true,
            first_person_pixel_scaling: 1.1,
        }
    }
}
impl Config {
    pub fn normalize(&mut self) {
        for value in [
            &mut self.base_voxel_size,
            &mut self.body_voxel_width_size,
            &mut self.head_voxel_size,
            &mut self.first_person_pixel_scaling,
        ] {
            *value = if value.is_finite() {
                value.clamp(1., 2.)
            } else {
                1.15
            };
        }
        self.render_distance_lod = if self.render_distance_lod.is_finite() {
            self.render_distance_lod.clamp(1., 64.)
        } else {
            14.
        };
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    fn opacity(u: u32, v: u32, pattern: u64) -> u8 {
        match pattern {
            0 => 0,
            1 => 255,
            2 => {
                if (u + v).is_multiple_of(2) {
                    255
                } else {
                    0
                }
            }
            3 => {
                if (u * 17 + v * 31) % 11 < 3 {
                    255
                } else {
                    0
                }
            }
            4 => {
                if (u * 7 + v * 13).is_multiple_of(3) {
                    128
                } else {
                    255
                }
            }
            _ => {
                if u == 12 && v == 9 {
                    255
                } else {
                    0
                }
            }
        }
    }
    #[test]
    fn original_wrapper_all_voxel_positions_uvs_hidden_faces_and_corner_cuts_match() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/mods/skinlayers-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let cases: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for case in cases.as_array().unwrap() {
            let pattern = case["pattern"].as_u64().unwrap();
            let skin = image::RgbaImage::from_fn(64, 64, |u, v| {
                image::Rgba([255, 255, 255, opacity(u, v, pattern)])
            });
            let size = std::array::from_fn(|i| case["size"][i].as_i64().unwrap() as i32);
            let actual = voxels(&skin, size, [8, 4], case["fast"].as_bool().unwrap());
            let expected = case["boxes"].as_array().unwrap();
            assert_eq!(
                actual.len(),
                expected.len(),
                "size {size:?}, pattern {pattern}"
            );
            for (voxel, row) in actual.iter().zip(expected) {
                let uv: Vec<i32> = row["uv"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap() as i32)
                    .collect();
                assert_eq!(
                    voxel.uv.as_slice(),
                    uv,
                    "pixel size {size:?}, pattern {pattern}"
                );
                let position: [i32; 3] = std::array::from_fn(|i| {
                    (row["position"][i].as_f64().unwrap()
                        + if i == 0 {
                            size[0] as f64 / 2.
                        } else if i == 2 {
                            size[2] as f64 / 2.
                        } else {
                            0.
                        }) as i32
                });
                assert_eq!(voxel.position, position);
                assert_eq!(
                    voxel.hidden,
                    row["hidden"].as_u64().unwrap() as u8,
                    "pixel {:?}, size {size:?}, pattern {pattern}",
                    voxel.uv
                );
                let mut corners: Vec<[u8; 2]> = row["corners"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| [r[0].as_u64().unwrap() as u8, r[1].as_u64().unwrap() as u8])
                    .collect();
                corners.sort_unstable();
                assert_eq!(voxel.corners, corners);
            }
        }
    }
}

use crate::world::Mesh;
use glam::Vec3;
#[derive(Clone, Copy, Debug)]
pub enum Part {
    Hat,
    Jacket,
    LeftPants,
    RightPants,
    LeftSleeve,
    RightSleeve,
}
pub struct Layers {
    pub parts: [Option<Mesh>; 6],
    pub hand: Option<Mesh>,
}
impl Layers {
    pub fn build(skin: &image::RgbaImage, slim: bool, c: &Config) -> Self {
        let arm = if slim { 3 } else { 4 };
        let specs = [
            ([8, 8, 8], [32, 0], c.enable_hat),
            ([8, 12, 4], [16, 32], c.enable_jacket),
            ([4, 12, 4], [0, 48], c.enable_left_pants),
            ([4, 12, 4], [0, 32], c.enable_right_pants),
            ([arm, 12, 4], [48, 48], c.enable_left_sleeve),
            ([arm, 12, 4], [40, 32], c.enable_right_sleeve),
        ];
        let parts = std::array::from_fn(|i| {
            specs[i]
                .2
                .then(|| layer_mesh(skin, specs[i].0, specs[i].1, c, i, false))
        });
        let hand = c
            .enable_right_sleeve
            .then(|| layer_mesh(skin, [arm, 12, 4], [40, 32], c, 5, true));
        Self { parts, hand }
    }
    pub fn get(&self, part: Part) -> Option<&Mesh> {
        self.parts[part as usize].as_ref()
    }
}
fn face_points(min: Vec3, max: Vec3, face: usize) -> [Vec3; 4] {
    let [x, y, z] = min.to_array();
    let [a, b, c] = max.to_array();
    match face {
        0 => [[a, y, z], [x, y, z], [x, y, c], [a, y, c]],
        1 => [[a, b, c], [x, b, c], [x, b, z], [a, b, z]],
        2 => [[a, b, z], [x, b, z], [x, y, z], [a, y, z]],
        3 => [[x, b, c], [a, b, c], [a, y, c], [x, y, c]],
        4 => [[x, b, z], [x, b, c], [x, y, c], [x, y, z]],
        _ => [[a, b, c], [a, b, z], [a, y, z], [a, y, c]],
    }
    .map(Vec3::from_array)
}
fn layer_mesh(
    skin: &image::RgbaImage,
    size: [i32; 3],
    offset: [i32; 2],
    c: &Config,
    part: usize,
    first: bool,
) -> Mesh {
    let dims = Vec3::new(size[0] as f32, size[1] as f32, size[2] as f32);
    let scale = if part == 0 {
        Vec3::splat(c.head_voxel_size)
    } else {
        Vec3::new(
            if part == 1 {
                c.body_voxel_width_size
            } else if first {
                c.first_person_pixel_scaling
            } else {
                c.base_voxel_size
            },
            1.035,
            if first {
                c.first_person_pixel_scaling
            } else {
                c.base_voxel_size
            },
        )
    };
    let center = Vec3::new(0., dims.y / 2., 0.);
    let convert = |p: Vec3| {
        let point = Vec3::new(dims.x / 2. - p.x, dims.y - p.y, p.z - dims.z / 2.);
        (point - center) * scale + center
    };
    let mut mesh = Mesh::new(Vec3::ZERO, Vec3::ZERO, 0);
    for voxel in voxels(skin, size, offset, c.fast_render) {
        let min = Vec3::new(
            voxel.position[0] as f32,
            voxel.position[1] as f32,
            voxel.position[2] as f32,
        );
        for face in 0..6 {
            if voxel.hidden & (1 << face) != 0 {
                continue;
            }
            let mut points = face_points(min, min + Vec3::ONE, face);
            points.swap(1, 3);
            if let Some(cut) = voxel
                .corners
                .iter()
                .rev()
                .find(|cut| cut.iter().all(|d| axis(*d as usize) != axis(face)))
            {
                let mut removed = 0;
                for i in 1..4 {
                    for &direction in cut {
                        let direction = direction as usize;
                        let a = axis(direction);
                        let delta = (points[removed][a] - points[i][a]) * STEP[direction][a] as f32;
                        if delta > 0. {
                            break;
                        }
                        if delta < 0. {
                            removed = i;
                            break;
                        }
                    }
                }
                let kept: Vec<_> = points
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, p)| (i != removed).then_some(p))
                    .collect();
                points = [kept[0], kept[1], kept[2], kept[2]];
            }
            let [u, v] = voxel.uv.map(|v| v as f32 / 64.);
            let end = [u + 1. / 64., v + 1. / 64.];
            mesh.quad_uv(
                points.map(|p| convert(p).to_array()),
                [[end[0], v], [u, v], [u, end[1]], end],
                6,
                [1.; 3],
            );
        }
    }
    if c.fast_render {
        for face in 0..6 {
            let mut points = face_points(Vec3::ZERO, dims, face);
            points.swap(1, 3);
            let base = texture_uv([0, 0], offset, size, face);
            let tex = points.map(|p| {
                let [u, v] = match face {
                    0 | 1 => [p.x, dims.z - p.z],
                    2 => [p.x, p.y],
                    3 => [dims.x - p.x, p.y],
                    4 => [dims.z - p.z, p.y],
                    _ => [p.z, p.y],
                };
                [(base[0] as f32 + u) / 64., (base[1] as f32 + v) / 64.]
            });
            mesh.quad_uv(points.map(|p| convert(p).to_array()), tex, 6, [1.; 3]);
        }
    }
    mesh.tighten_bounds();
    mesh
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    #[test]
    fn full_skin_has_outward_shell_and_fast_render_removes_hidden_voxel_work() {
        let skin = image::RgbaImage::from_pixel(64, 64, image::Rgba([255; 4]));
        let config = Config {
            fast_render: false,
            ..Default::default()
        };
        let mesh = layer_mesh(&skin, [8, 8, 8], [32, 0], &config, 0, false);
        assert_eq!(mesh.indices.len(), 384 * 6);
        for quad in mesh.vertices.as_chunks::<4>().0 {
            let center = quad
                .iter()
                .map(|v| Vec3::from_array(v.position))
                .sum::<Vec3>()
                / 4.;
            assert!(Vec3::from_array(quad[0].normal).dot(center - Vec3::new(0., 4., 0.)) > 0.);
        }
        let fast = layer_mesh(&skin, [8, 8, 8], [32, 0], &Config::default(), 0, false);
        assert_eq!(fast.indices.len(), 36);
        let empty = image::RgbaImage::new(64, 64);
        assert!(layer_mesh(&empty, [8, 8, 8], [32, 0], &config, 0, false)
            .indices
            .is_empty());
    }
}
