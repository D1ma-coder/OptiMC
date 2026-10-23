//! Finite-segment clipping against visual voxel boxes (camera, no fluids).
use crate::player::CollisionWorld;
use glam::DVec3;
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub position: DVec3,
    pub block: [i32; 3],
}
fn intersection(start: DVec3, delta: DVec3, min: DVec3, max: DVec3) -> Option<f64> {
    let probe = start + delta * 0.001;
    if (0..3).all(|i| probe[i] >= min[i] && probe[i] < max[i]) {
        return Some(0.001);
    }
    let mut near = 0f64;
    let mut far = 1f64;
    for axis in 0..3 {
        if delta[axis].abs() < 1e-12 {
            if start[axis] < min[axis] || start[axis] > max[axis] {
                return None;
            }
        } else {
            let a = (min[axis] - start[axis]) / delta[axis];
            let b = (max[axis] - start[axis]) / delta[axis];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    if (0. ..=1.).contains(&near) && far >= 0. {
        Some(near)
    } else {
        None
    }
}
pub fn visual(world: &(impl CollisionWorld + ?Sized), start: DVec3, end: DVec3) -> Option<Hit> {
    let delta = end - start;
    if !start.is_finite()
        || !end.is_finite()
        || delta.length_squared() < 1e-7
        || delta.length_squared() > 64. * 64.
    {
        return None;
    }
    // Original static visual shapes span [-.25,1.5]. Include neighboring cells.
    let lower = start.min(end).floor().as_ivec3() - glam::IVec3::ONE;
    let upper = start.max(end).floor().as_ivec3() + glam::IVec3::ONE;
    let mut best = 1f64;
    let mut hit = None;
    for y in lower.y..=upper.y {
        for z in lower.z..=upper.z {
            for x in lower.x..=upper.x {
                world.visual_boxes(x, y, z, &mut |min, max| {
                    if let Some(t) = intersection(start, delta, min, max) {
                        if t <= best {
                            best = t;
                            hit = Some(Hit {
                                position: start + delta * t,
                                block: [x, y, z],
                            });
                        }
                    }
                });
            }
        }
    }
    hit
}
/// Revalidate a smoothed camera segment with the same eight-corner safety volume.
pub fn camera_segment(
    world: &(impl CollisionWorld + ?Sized),
    origin: DVec3,
    desired: DVec3,
) -> DVec3 {
    let delta = desired - origin;
    let distance = delta.length();
    if distance < 1e-6 {
        return desired;
    }
    let direction = delta / distance;
    let mut permitted = distance;
    for corner in 0..8 {
        let offset = DVec3::new(
            if corner & 1 == 0 { -0.1 } else { 0.1 },
            if corner & 2 == 0 { -0.1 } else { 0.1 },
            if corner & 4 == 0 { -0.1 } else { 0.1 },
        );
        if let Some(hit) = visual(world, origin + offset, desired + offset) {
            permitted =
                permitted.min(((hit.position - origin - offset).dot(direction) - 0.05).max(0.));
        }
    }
    origin + direction * permitted
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Thin;
    impl CollisionWorld for Thin {
        fn solid(&self, x: i32, y: i32, z: i32) -> bool {
            x == 0 && y == 1 && z == 1
        }
        fn visual_boxes(&self, x: i32, y: i32, z: i32, visit: &mut dyn FnMut(DVec3, DVec3)) {
            if self.solid(x, y, z) {
                visit(DVec3::new(0., 1., 1.234), DVec3::new(1., 2., 1.244));
            }
        }
    }
    #[test]
    fn smoothed_camera_retracts_before_a_new_wall_and_never_crosses_it() {
        struct Wall;
        impl CollisionWorld for Wall {
            fn solid(&self, x: i32, _y: i32, _z: i32) -> bool {
                x == 2
            }
        }
        let origin = DVec3::new(0., 1.6, 0.);
        let desired = DVec3::new(4., 1.6, 0.);
        let clipped = camera_segment(&Wall, origin, desired);
        assert!(clipped.x > 1.7 && clipped.x < 1.9);
        assert_eq!(camera_segment(&Wall, origin, origin), origin);
    }
    #[test]
    fn thin_visual_face_is_hit_exactly_without_fixed_distance_steps() {
        let hit = visual(
            &Thin,
            DVec3::new(0.5, 1.62, 0.5),
            DVec3::new(0.5, 1.62, 4.5),
        )
        .unwrap();
        assert!((hit.position.z - 1.234).abs() < 1e-9);
        assert_eq!(hit.block, [0, 1, 1]);
    }
    #[test]
    fn rays_outside_the_partial_box_are_not_blocked_by_a_solid_cell() {
        assert!(visual(&Thin, DVec3::new(0.5, 2.2, 0.5), DVec3::new(0.5, 2.2, 4.5)).is_none());
    }
}
