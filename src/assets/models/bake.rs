use super::*;
use glam::{Mat4, Vec2, Vec3};
#[derive(Clone, Debug)]
pub struct BakedQuad {
    pub points: [Vec3; 4],
    pub uv: [Vec2; 4],
    pub normal: Vec3,
    pub material: Material,
    pub cullface: Option<Direction>,
    pub tint_index: i32,
    pub shade: bool,
    pub light_emission: u8,
}
fn corners(direction: Direction, from: Vec3, to: Vec3) -> [Vec3; 4] {
    let [x, y, z] = from.to_array();
    let [xx, yy, zz] = to.to_array();
    (match direction {
        Direction::Down => [[x, y, zz], [x, y, z], [xx, y, z], [xx, y, zz]],
        Direction::Up => [[x, yy, z], [x, yy, zz], [xx, yy, zz], [xx, yy, z]],
        Direction::North => [[xx, yy, z], [xx, y, z], [x, y, z], [x, yy, z]],
        Direction::South => [[x, yy, zz], [x, y, zz], [xx, y, zz], [xx, yy, zz]],
        Direction::West => [[x, yy, z], [x, y, z], [x, y, zz], [x, yy, zz]],
        Direction::East => [[xx, yy, zz], [xx, y, zz], [xx, y, z], [xx, yy, z]],
    })
    .map(Vec3::from_array)
}
fn default_uv(direction: Direction, from: Vec3, to: Vec3) -> [f32; 4] {
    match direction {
        Direction::Down => [from.x, 16. - to.z, to.x, 16. - from.z],
        Direction::Up => [from.x, from.z, to.x, to.z],
        Direction::North => [16. - to.x, 16. - to.y, 16. - from.x, 16. - from.y],
        Direction::South => [from.x, 16. - to.y, to.x, 16. - from.y],
        Direction::West => [from.z, 16. - to.y, to.z, 16. - from.y],
        Direction::East => [16. - to.z, 16. - to.y, 16. - from.z, 16. - from.y],
    }
}
fn rotation(r: &Rotation) -> Mat4 {
    let mut transform = match r.axis.as_deref() {
        Some("x") => Mat4::from_rotation_x(r.angle.unwrap().to_radians()),
        Some("y") => Mat4::from_rotation_y(r.angle.unwrap().to_radians()),
        Some("z") => Mat4::from_rotation_z(r.angle.unwrap().to_radians()),
        _ => {
            Mat4::from_rotation_z(r.z.to_radians())
                * Mat4::from_rotation_y(r.y.to_radians())
                * Mat4::from_rotation_x(r.x.to_radians())
        }
    };
    if r.rescale {
        // 26.2 normalizes each transformed basis axis by its largest absolute
        // component, then post-multiplies the scale. This also handles Euler
        // rotations and angles beyond 45 degrees without a cosine singularity.
        let factors = [Vec3::X, Vec3::Y, Vec3::Z]
            .map(|axis| 1. / transform.transform_vector3(axis).abs().max_element());
        transform *= Mat4::from_scale(Vec3::from_array(factors));
    }
    let origin = Vec3::from_array(r.origin) / 16.;
    Mat4::from_translation(origin) * transform * Mat4::from_translation(-origin)
}
fn basis(direction: Direction) -> (Vec3, Vec3) {
    let p = corners(direction, Vec3::ZERO, Vec3::ONE);
    (p[3] - p[0], p[1] - p[0])
}
pub(super) fn bake(model: &Model, variant: &Variant) -> Result<Vec<BakedQuad>> {
    let rotate = Mat4::from_rotation_y(-(variant.y as f32).to_radians())
        * Mat4::from_rotation_x(-(variant.x as f32).to_radians());
    let transform = Mat4::from_translation(Vec3::splat(0.5))
        * rotate
        * Mat4::from_translation(Vec3::splat(-0.5));
    let mut result = Vec::new();
    for element in &model.elements {
        let from = Vec3::from_array(element.from);
        let to = Vec3::from_array(element.to);
        let local = element
            .rotation
            .as_ref()
            .map(rotation)
            .unwrap_or(Mat4::IDENTITY);
        for (&direction, face) in &element.faces {
            let points = corners(direction, from / 16., to / 16.)
                .map(|p| transform.transform_point3(local.transform_point3(p)));
            let normal = (points[1] - points[0])
                .cross(points[2] - points[0])
                .normalize_or_zero();
            // Zero-thickness planes have valid faces; only degenerate triangles
            // with zero area are omitted.
            if normal == Vec3::ZERO {
                continue;
            }
            let [u, v, uu, vv] = face
                .uv
                .unwrap_or_else(|| default_uv(direction, from, to))
                .map(|v| v / 16.);
            let base = [
                Vec2::new(u, v),
                Vec2::new(u, vv),
                Vec2::new(uu, vv),
                Vec2::new(uu, v),
            ];
            let mut uv = std::array::from_fn(|i| base[(i + face.rotation as usize / 90) % 4]);
            if variant.uvlock {
                let target = Direction::closest(rotate.transform_vector3(direction.normal()));
                let (su, sv) = basis(direction);
                let (tu, tv) = basis(target);
                let ru = rotate.transform_vector3(su);
                let rv = rotate.transform_vector3(sv);
                uv = uv.map(|uv| {
                    let p = uv - Vec2::splat(0.5);
                    Vec2::new(
                        ru.dot(tu) * p.x + rv.dot(tu) * p.y,
                        ru.dot(tv) * p.x + rv.dot(tv) * p.y,
                    ) + Vec2::splat(0.5)
                });
            }
            result.push(BakedQuad {
                points,
                uv,
                normal,
                material: model.material(&format!("#{}", face.texture.trim_start_matches('#')))?,
                cullface: face
                    .cullface
                    .map(|d| Direction::closest(rotate.transform_vector3(d.normal()))),
                tint_index: face.tintindex,
                shade: element.shade,
                light_emission: element.light_emission,
            });
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cube() -> Model {
        let faces = [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ]
        .map(|d| {
            (
                d,
                Face {
                    texture: "#all".into(),
                    uv: None,
                    cullface: Some(d),
                    rotation: 0,
                    tintindex: -1,
                },
            )
        })
        .into();
        Model {
            textures: BTreeMap::from([("all".into(), Texture::Reference("block/stone".into()))]),
            elements: vec![Element {
                from: [0.; 3],
                to: [16.; 3],
                rotation: None,
                faces,
                shade: true,
                light_emission: 0,
            }],
            ..Model::default()
        }
    }
    fn variant(x: i32, y: i32, uvlock: bool) -> Variant {
        Variant {
            model: "block/test".into(),
            x,
            y,
            uvlock,
            weight: 1,
        }
    }
    #[test]
    fn cube_faces_point_outward_and_texture_v_is_down() {
        let faces = bake(&cube(), &variant(0, 0, false)).unwrap();
        assert_eq!(faces.len(), 6);
        for face in faces {
            assert!(face.normal.dot(face.points[0] - Vec3::splat(0.5)) > 0.);
            if face.normal.y.abs() < 0.5 {
                for (p, uv) in face.points.into_iter().zip(face.uv) {
                    assert_eq!(uv.y, 1. - p.y);
                }
            }
        }
    }
    #[test]
    fn uvlock_preserves_world_orientation_after_every_quadrant_rotation() {
        for x in [0, 90, 180, 270] {
            for y in [0, 90, 180, 270] {
                for face in bake(&cube(), &variant(x, y, true)).unwrap() {
                    let target = Direction::closest(face.normal);
                    let (u, v) = basis(target);
                    let canonical = corners(target, Vec3::ZERO, Vec3::ONE)[0];
                    for (p, uv) in face.points.into_iter().zip(face.uv) {
                        let relative = p - canonical;
                        assert!(
                            (Vec2::new(relative.dot(u), relative.dot(v)) - uv).length() < 1e-5,
                            "{x} {y} {target:?}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn uv_rotation_does_not_change_normals_or_classic_diffuse_on_rotated_models() {
        for x in [0, 90, 180, 270] {
            for y in [0, 90, 180, 270] {
                for uvlock in [false, true] {
                    let original = cube();
                    let reference = bake(&original, &variant(x, y, uvlock)).unwrap();
                    for rotation in [90, 180, 270] {
                        let mut rotated = original.clone();
                        for face in rotated.elements[0].faces.values_mut() {
                            face.rotation = rotation;
                        }
                        let result = bake(&rotated, &variant(x, y, uvlock)).unwrap();
                        for (a, b) in reference.iter().zip(&result) {
                            assert_eq!(a.normal, b.normal);
                            assert_eq!(a.points, b.points);
                            assert_ne!(a.uv, b.uv);
                            assert_eq!(
                                crate::world::classic_shade(a.normal, a.shade),
                                crate::world::classic_shade(b.normal, b.shade)
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn rescale_handles_quarter_turns_and_euler_rotations() {
        for source in [
            serde_json::json!({"origin":[0,0,0],"axis":"y","angle":90,"rescale":true}),
            serde_json::json!({"origin":[0,0,0],"x":20,"y":67.5,"z":35,"rescale":true}),
        ] {
            let r: Rotation = serde_json::from_value(source).unwrap();
            let transform = rotation(&r);
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                let rotated = transform.transform_vector3(axis);
                assert!(rotated.is_finite());
                assert!((rotated.abs().max_element() - 1.).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn euler_xyz_uses_zyx_matrix_order_about_model_origin() {
        let r: Rotation = serde_json::from_value(
            serde_json::json!({"origin":[8,0,8],"x":180,"y":-67.5,"z":-180}),
        )
        .unwrap();
        assert!(
            (rotation(&r).transform_point3(Vec3::new(0.5, 0., 0.5)) - Vec3::new(0.5, 0., 0.5))
                .length()
                < 1e-6
        );
    }
}
