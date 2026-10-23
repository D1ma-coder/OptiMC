//! Classic 64×64 player skin layout, independently generated cuboid geometry.
use super::Mesh;
use crate::player::Player;
use glam::{Mat4, Vec3};
const UNIT: f32 = 1.8 / 32.;
fn cuboid(mesh: &mut Mesh, transform: Mat4, min: Vec3, size: Vec3, uv: [f32; 2]) {
    cube(mesh, transform, min, size, uv, 0.);
}
fn cube(mesh: &mut Mesh, transform: Mat4, min: Vec3, size: Vec3, uv: [f32; 2], inflate: f32) {
    let max = min + size + Vec3::splat(inflate);
    let min = min - Vec3::splat(inflate);
    let [x, y, z] = min.to_array();
    let [xx, yy, zz] = max.to_array();
    let [w, h, d] = size.to_array();
    let [u, v] = uv;
    let faces = [
        (
            [[x, yy, z], [x, yy, zz], [xx, yy, zz], [xx, yy, z]],
            [u + d, v, u + d + w, v + d],
        ),
        (
            [[x, y, zz], [x, y, z], [xx, y, z], [xx, y, zz]],
            [u + d + w, v, u + d + w + w, v + d],
        ),
        (
            [[xx, y, z], [x, y, z], [x, yy, z], [xx, yy, z]],
            [u + d, v + d + h, u + d + w, v + d],
        ),
        (
            [[x, y, zz], [xx, y, zz], [xx, yy, zz], [x, yy, zz]],
            [u + 2. * d + w, v + d + h, u + 2. * d + 2. * w, v + d],
        ),
        (
            [[x, y, z], [x, y, zz], [x, yy, zz], [x, yy, z]],
            [u, v + d + h, u + d, v + d],
        ),
        (
            [[xx, y, zz], [xx, y, z], [xx, yy, z], [xx, yy, zz]],
            [u + d + w, v + d + h, u + 2. * d + w, v + d],
        ),
    ];
    for (points, uv) in faces {
        let points = points.map(|p| {
            transform
                .transform_point3(Vec3::from_array(p) * UNIT)
                .to_array()
        });
        let [u, v, uu, vv] = uv.map(|p| p / 64.);
        mesh.quad_uv(points, [[u, v], [uu, v], [uu, vv], [u, vv]], 6, [1.; 3]);
    }
}
fn overlay(
    mesh: &mut Mesh,
    transform: Mat4,
    min: Vec3,
    size: Vec3,
    uv: [f32; 2],
    inflate: f32,
    layer: Option<&Mesh>,
) {
    if let Some(layer) = layer {
        let base = mesh.vertices.len() as u32;
        let offset = min + Vec3::new(size.x * 0.5, 0., size.z * 0.5);
        for vertex in &layer.vertices {
            let mut v = *vertex;
            v.position = transform
                .transform_point3((Vec3::from_array(v.position) + offset) * UNIT)
                .to_array();
            v.normal = transform
                .transform_vector3(v.normal.into())
                .normalize_or_zero()
                .to_array();
            mesh.vertices.push(v);
        }
        mesh.indices.extend(layer.indices.iter().map(|i| base + *i));
    } else {
        cube(mesh, transform, min, size, uv, inflate);
    }
}
pub fn body(player: &Player, yaw: f32, pitch: f32) -> Mesh {
    body_model(player, yaw, pitch, false)
}
pub fn body_model(player: &Player, yaw: f32, pitch: f32, slim: bool) -> Mesh {
    body_model_layers(player, yaw, pitch, slim, None)
}
pub fn body_model_layers(
    player: &Player,
    yaw: f32,
    pitch: f32,
    slim: bool,
    layers: Option<&crate::mods::skin_layers::Layers>,
) -> Mesh {
    use crate::mods::skin_layers::Part;

    let feet = player.interpolated_feet().as_vec3();
    let root = Mat4::from_translation(feet) * Mat4::from_rotation_y(-yaw);
    let mut mesh = Mesh::new(feet - Vec3::ONE, feet + Vec3::new(1., 2., 1.), 0);
    let mut overlays = Mesh::new(mesh.min, mesh.max, 0);
    let swing = (player.walk_distance * std::f32::consts::PI * 2.).sin() * player.bob * 8.;
    let torso = if player.crouching {
        Mat4::from_translation(Vec3::new(0., -0.2, 0.)) * Mat4::from_rotation_x(0.25)
    } else {
        Mat4::IDENTITY
    };
    cuboid(
        &mut mesh,
        root * torso,
        Vec3::new(-4., 12., -2.),
        Vec3::new(8., 12., 4.),
        [16., 16.],
    );
    overlay(
        &mut overlays,
        root * torso,
        Vec3::new(-4., 12., -2.),
        Vec3::new(8., 12., 4.),
        [16., 32.],
        0.25,
        layers.and_then(|l| l.get(Part::Jacket)),
    );
    let head =
        root * torso * Mat4::from_translation(Vec3::Y * 24. * UNIT) * Mat4::from_rotation_x(pitch);
    cuboid(
        &mut mesh,
        head,
        Vec3::new(-4., 0., -4.),
        Vec3::splat(8.),
        [0., 0.],
    );
    overlay(
        &mut overlays,
        head,
        Vec3::new(-4., 0., -4.),
        Vec3::splat(8.),
        [32., 0.],
        0.5,
        layers.and_then(|l| l.get(Part::Hat)),
    );
    for (x, angle, uv, outer) in [
        (-4., swing, [0., 16.], [0., 32.]),
        (0., -swing, [16., 48.], [0., 48.]),
    ] {
        let t = root
            * Mat4::from_translation(Vec3::new(x * UNIT, 12. * UNIT, 0.))
            * Mat4::from_rotation_x(angle);
        cuboid(
            &mut mesh,
            t,
            Vec3::new(0., -12., -2.),
            Vec3::new(4., 12., 4.),
            uv,
        );
        overlay(
            &mut overlays,
            t,
            Vec3::new(0., -12., -2.),
            Vec3::new(4., 12., 4.),
            outer,
            0.25,
            layers.and_then(|l| {
                l.get(if x < 0. {
                    Part::RightPants
                } else {
                    Part::LeftPants
                })
            }),
        );
    }
    let arm = if slim { 3. } else { 4. };
    let shoulder = 4. + arm * 0.5;
    for (x, angle, uv, outer) in [
        (-shoulder, -swing, [40., 16.], [40., 32.]),
        (shoulder, swing, [32., 48.], [48., 48.]),
    ] {
        let t = root
            * torso
            * Mat4::from_translation(Vec3::new(x * UNIT, 22. * UNIT, 0.))
            * Mat4::from_rotation_x(angle);
        cuboid(
            &mut mesh,
            t,
            Vec3::new(-arm * 0.5, -10., -2.),
            Vec3::new(arm, 12., 4.),
            uv,
        );
        overlay(
            &mut overlays,
            t,
            Vec3::new(-arm * 0.5, -10., -2.),
            Vec3::new(arm, 12., 4.),
            outer,
            0.25,
            layers.and_then(|l| {
                l.get(if x < 0. {
                    Part::RightSleeve
                } else {
                    Part::LeftSleeve
                })
            }),
        );
    }
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(overlays.vertices);
    mesh.indices
        .extend(overlays.indices.into_iter().map(|i| i + base));
    mesh
}
pub fn hand(player: &Player, yaw: f32, pitch: f32) -> Mesh {
    hand_model(player, yaw, pitch, false)
}
pub fn hand_model(player: &Player, yaw: f32, pitch: f32, slim: bool) -> Mesh {
    hand_at(player.eye(), yaw, pitch, slim)
}
pub fn hand_at(eye: Vec3, yaw: f32, pitch: f32, slim: bool) -> Mesh {
    hand_at_layers(eye, yaw, pitch, slim, None)
}
pub fn hand_at_layers(eye: Vec3, yaw: f32, pitch: f32, slim: bool, layer: Option<&Mesh>) -> Mesh {
    hand_at_layers_animated(eye, yaw, pitch, slim, layer, Mat4::IDENTITY)
}
pub fn hand_at_layers_animated(
    eye: Vec3,
    yaw: f32,
    pitch: f32,
    slim: bool,
    layer: Option<&Mesh>,
    motion: Mat4,
) -> Mesh {
    let root = Mat4::from_translation(eye)
        * Mat4::from_rotation_y(-yaw)
        * Mat4::from_rotation_x(pitch)
        * motion
        * Mat4::from_translation(Vec3::new(0.35, -0.4, -0.55))
        * Mat4::from_rotation_z(-0.18)
        * Mat4::from_rotation_x(-0.55);
    let mut mesh = Mesh::new(eye - Vec3::ONE, eye + Vec3::ONE, 0);
    let arm = if slim { 3. } else { 4. };
    let min = Vec3::new(-arm * 0.5, -6., -2.);
    let size = Vec3::new(arm, 12., 4.);
    cuboid(&mut mesh, root, min, size, [40., 16.]);
    overlay(&mut mesh, root, min, size, [40., 32.], 0.25, layer);
    mesh
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slim_model_has_narrower_arms_with_the_same_skin_layout() {
        let player = Player::spawn(glam::DVec3::ZERO);
        let wide = body_model(&player, 0., 0., false);
        let slim = body_model(&player, 0., 0., true);
        let extent = |mesh: &Mesh| {
            mesh.vertices
                .iter()
                .map(|v| v.position[0].abs())
                .fold(0., f32::max)
        };
        assert!(extent(&slim) < extent(&wide));
        assert_eq!(slim.indices.len(), wide.indices.len());
        for v in slim.vertices {
            assert!(v.uv.iter().all(|v| (0.0..=1.0).contains(v)));
        }
        assert_eq!(hand_model(&player, 0., 0., true).indices.len(), 72);
    }
    #[test]
    fn skin_coordinates_and_faces_are_valid() {
        let mesh = body(&Player::spawn(glam::DVec3::ZERO), 0., 0.);
        assert_eq!(mesh.indices.len(), 12 * 6 * 6);
        for v in mesh.vertices {
            assert!(v.uv.iter().all(|v| (0.0..=1.0).contains(v)));
            assert!(Vec3::from_array(v.normal).is_normalized());
        }
    }
}
