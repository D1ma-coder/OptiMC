//! Presentation-only, frame-rate-independent camera transitions.
use super::Camera;
use glam::Vec3;
#[derive(Default)]
pub(super) struct Motion {
    view: Option<View>,
}
struct View {
    offset: Vec3,
    yaw: f32,
    pitch: f32,
    fov: f32,
}
fn factor(dt: f32, half_life: f32) -> f32 {
    1. - (-std::f32::consts::LN_2 * dt.max(0.) / half_life).exp()
}
fn angle(from: f32, to: f32, t: f32) -> f32 {
    from + ((to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI)
        * t
}
impl Motion {
    pub fn reset(&mut self) {
        self.view = None;
    }
    pub fn step(&mut self, target: &Camera, anchor: Vec3, dt: f32, enabled: bool) -> Camera {
        let direction = target.direction().normalize_or_zero();
        let yaw = direction.x.atan2(-direction.z);
        let pitch = direction.y.clamp(-1., 1.).asin();
        let desired = View {
            offset: target.eye - anchor,
            yaw,
            pitch,
            fov: target.fov,
        };
        if !enabled || dt > 0.25 {
            self.view = None;
            return target.clone();
        }
        let view = self.view.get_or_insert(View { ..desired });
        view.offset = view.offset.lerp(desired.offset, factor(dt, 0.022));
        view.yaw = angle(view.yaw, desired.yaw, factor(dt, 0.010));
        view.pitch += (desired.pitch - view.pitch) * factor(dt, 0.010);
        view.fov += (desired.fov - view.fov) * factor(dt, 0.025);
        let mut camera = target.clone();
        camera.eye = anchor + view.offset;
        // Keep raw yaw/pitch for the avatar's pose. Only the rendered view direction is filtered.
        camera.direction_override = Some(Vec3::new(
            view.yaw.sin() * view.pitch.cos(),
            view.pitch.sin(),
            -view.yaw.cos() * view.pitch.cos(),
        ));
        camera.fov = view.fov;
        camera
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn transitions_are_fast_monotonic_and_frame_rate_independent() {
        let mut final_offset: Option<f32> = None;
        for fps in [30, 60, 120, 240, 1000] {
            let mut motion = Motion::default();
            let mut camera = Camera::panorama(Duration::ZERO);
            camera.eye = Vec3::Y * 1.62;
            motion.step(&camera, Vec3::ZERO, 0., true);
            camera.eye.y = 1.27;
            let mut previous = 1.62;
            for _ in 0..fps / 5 {
                let view = motion.step(&camera, Vec3::ZERO, 1. / fps as f32, true);
                assert!(view.eye.y <= previous + 1e-6 && view.eye.y >= 1.27);
                previous = view.eye.y;
            }
            assert!((previous - 1.27).abs() < 0.001);
            if let Some(expected) = final_offset {
                assert!((previous - expected).abs() < 0.00001);
            } else {
                final_offset = Some(previous);
            }
        }
    }
    #[test]
    fn walking_anchor_and_network_pose_are_not_delayed() {
        let mut motion = Motion::default();
        let mut camera = Camera::panorama(Duration::ZERO);
        camera.eye = Vec3::Y * 1.62;
        motion.step(&camera, Vec3::ZERO, 0., true);
        let anchor = Vec3::X * 100.;
        camera.eye += anchor;
        let view = motion.step(&camera, anchor, 1. / 120., true);
        assert_eq!(view.eye, camera.eye);
        camera.yaw = 1.;
        let raw = camera.yaw;
        let view = motion.step(&camera, anchor, 1. / 120., true);
        assert_eq!(camera.yaw, raw);
        assert_ne!(view.direction(), camera.direction());
        assert_eq!(
            motion.step(&camera, anchor, 1. / 120., false).direction(),
            camera.direction()
        );
    }
    #[test]
    fn rotation_uses_short_path_across_angle_seam() {
        let a = 179f32.to_radians();
        let b = (-179f32).to_radians();
        let mid = angle(a, b, 0.5);
        assert!((mid.abs() - std::f32::consts::PI).abs() < 1e-5);
    }
}
