pub mod avatar;
pub mod collision;
pub mod detail;
pub mod entity_pick;
pub mod lod;
pub mod mesh;
pub mod mining;
pub mod picking;
pub mod remote;
pub mod remote_mesh;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub tint: [f32; 3],
    pub layer: u32,
    pub lod: u32,
    pub normal: [f32; 3],
    /// High world origin. Small local offsets remain in `position` for entities
    /// requiring camera-relative precision (maps/sign text).
    pub origin: [f32; 3],
    /// Legacy diffuse for Low; kept separate from unlit material/biome tint.
    pub shade: f32,
}
#[derive(Clone, Debug)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub min: Vec3,
    pub max: Vec3,
    pub level: u8,
    pub translucent: bool,
    pub section: Option<[i32; 3]>,
    pub polygon_lods: Vec<Mesh>,
}
impl Mesh {
    pub fn new(min: Vec3, max: Vec3, level: u8) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            min,
            max,
            level,
            translucent: false,
            section: None,
            polygon_lods: Vec::new(),
        }
    }
    pub fn tighten_bounds(&mut self) {
        if self.vertices.is_empty() {
            return;
        }
        self.min = Vec3::splat(f32::INFINITY);
        self.max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in &self.vertices {
            let p = Vec3::from_array(vertex.position) + Vec3::from_array(vertex.origin);
            self.min = self.min.min(p);
            self.max = self.max.max(p);
        }
    }
    pub fn quad(&mut self, points: [[f32; 3]; 4], uv: [f32; 2], layer: u32, tint: [f32; 3]) {
        self.quad_uv(
            points,
            [[0., 0.], [uv[0], 0.], uv, [0., uv[1]]],
            layer,
            tint,
        );
    }
    pub fn quad_uv(
        &mut self,
        points: [[f32; 3]; 4],
        uvs: [[f32; 2]; 4],
        layer: u32,
        tint: [f32; 3],
    ) {
        self.quad_uv_shaded(points, uvs, layer, tint, 1.);
    }
    pub fn quad_uv_shaded(
        &mut self,
        points: [[f32; 3]; 4],
        uvs: [[f32; 2]; 4],
        layer: u32,
        tint: [f32; 3],
        shade: f32,
    ) {
        let base = self.vertices.len() as u32;
        let normal = (Vec3::from_array(points[1]) - Vec3::from_array(points[0]))
            .cross(Vec3::from_array(points[2]) - Vec3::from_array(points[0]))
            .normalize_or_zero()
            .to_array();
        for (position, uv) in points.into_iter().zip(uvs) {
            self.vertices.push(Vertex {
                position,
                uv,
                layer,
                lod: self.level as u32,
                tint,
                normal,
                origin: [0.; 3],
                shade,
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}
/// Deterministic demo terrain, separate from server-owned Minecraft terrain.
pub fn height(x: i32, z: i32) -> i32 {
    let x = x as f64;
    let z = z as f64;
    (36. + (x * 0.013).sin() * 13.
        + (z * 0.018).cos() * 10.
        + ((x + z) * 0.041).sin() * 4.
        + ((x - z) * 0.007).cos() * 15.) as i32
}

pub mod raycast;

/// Existing Low directional weights, independent of texture coordinates.
/// Enhanced lighting consumes the unlit tint directly, never their inverse.
pub fn classic_shade(normal: Vec3, enabled: bool) -> f32 {
    if !enabled || normal.y > 0.5 {
        1.
    } else if normal.y < -0.5 {
        0.5
    } else if normal.x.abs() > 0.5 {
        0.65
    } else {
        0.78
    }
}

#[cfg(test)]
mod lighting_tests {
    use super::*;
    #[test]
    fn mesh_keeps_material_color_separate_from_low_diffuse() {
        let tint = [0.46, 0.77, 0.27];
        let mut mesh = Mesh::new(Vec3::ZERO, Vec3::ONE, 0);
        let points = [[0., 0., 0.], [0., 1., 0.], [0., 1., 1.], [0., 0., 1.]];
        for rotation in 0..4 {
            let uv = [[0., 0.], [0., 1.], [1., 1.], [1., 0.]];
            mesh.quad_uv_shaded(
                points,
                std::array::from_fn(|i| uv[(i + rotation) % 4]),
                1,
                tint,
                0.65,
            );
        }
        for v in &mesh.vertices {
            assert_eq!(v.tint, tint);
            assert_eq!(v.shade, 0.65);
            assert_eq!(v.normal, [1., 0., 0.]);
            assert_eq!(v.tint.map(|c| c * v.shade), tint.map(|c| c * 0.65));
        }
        assert_eq!(classic_shade(Vec3::X, true), 0.65);
        assert_eq!(classic_shade(Vec3::Z, true), 0.78);
        assert_eq!(classic_shade(Vec3::NEG_Y, true), 0.5);
        assert_eq!(classic_shade(Vec3::NEG_Y, false), 1.);
    }
}
