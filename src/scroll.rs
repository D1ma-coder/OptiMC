//! Minecraft 26.2 ScrollWheelHandler accumulation and GLFW-compatible units.
use winit::event::MouseScrollDelta;
#[derive(Default)]
pub struct Wheel {
    accumulated: [f64; 2],
}
impl Wheel {
    pub fn push(&mut self, delta: [f64; 2]) -> [i32; 2] {
        if !delta.into_iter().all(f64::is_finite) {
            return [0; 2];
        }
        let mut result = [0; 2];
        for axis in 0..2 {
            let value = delta[axis];
            if self.accumulated[axis] != 0. && java_sign(value) != java_sign(self.accumulated[axis])
            {
                self.accumulated[axis] = 0.;
            }
            self.accumulated[axis] += value;
            result[axis] = self.accumulated[axis] as i32;
        }
        if result != [0; 2] {
            for (sum, steps) in self.accumulated.iter_mut().zip(result) {
                *sum -= steps as f64;
            }
        }
        result
    }
    pub fn hotbar(&mut self, delta: MouseScrollDelta, scale: f64) -> i32 {
        self.hotbar_settings(delta, scale, 1., false)
    }
    pub fn steps(
        &mut self,
        delta: MouseScrollDelta,
        scale: f64,
        sensitivity: f64,
        discrete: bool,
    ) -> [i32; 2] {
        let delta = glfw_units(delta, scale)
            .map(|v| (if discrete { java_sign(v) } else { v }) * sensitivity);
        self.push(delta)
    }
    pub fn hotbar_settings(
        &mut self,
        delta: MouseScrollDelta,
        scale: f64,
        sensitivity: f64,
        discrete: bool,
    ) -> i32 {
        let steps = self.steps(delta, scale, sensitivity, discrete);
        // Original getNextScrollWheelSelection uses only the sign of the accumulated integer.
        if steps[1] != 0 {
            steps[1].signum()
        } else {
            -steps[0].signum()
        }
    }
}
fn java_sign(value: f64) -> f64 {
    if value == 0. {
        0.
    } else {
        value.signum()
    }
}
pub fn glfw_units(delta: MouseScrollDelta, scale: f64) -> [f64; 2] {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => [x as f64, y as f64],
        // winit Cocoa supplies physical pixels; GLFW Cocoa supplies logical delta × 0.1.
        MouseScrollDelta::PixelDelta(p) => {
            let factor = 0.1 / scale.max(f64::MIN_POSITIVE);
            [p.x * factor, p.y * factor]
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_events_accumulate_and_reversal_drops_old_remainder() {
        let mut wheel = Wheel::default();
        for _ in 0..3 {
            assert_eq!(wheel.push([0., 0.25]), [0, 0]);
        }
        assert_eq!(wheel.push([0., 0.25]), [0, 1]);
        assert_eq!(wheel.push([0., 0.75]), [0, 0]);
        assert_eq!(wheel.push([0., -0.5]), [0, 0]);
        assert_eq!(wheel.push([0., -0.5]), [0, -1]);
    }
    #[test]
    fn retina_trackpad_is_dpi_independent_and_does_not_change_slot_per_pixel() {
        let mut a = Wheel::default();
        let mut b = Wheel::default();
        for i in 0..20 {
            let step = a.hotbar(
                MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0., 2.)),
                2.,
            );
            let other = b.hotbar(
                MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0., 1.)),
                1.,
            );
            assert_eq!(step, other);
            if i < 10 {
                assert_eq!(step, 0);
            }
        }
    }
    #[test]
    fn wheel_notch_and_large_event_move_one_slot_and_horizontal_is_fallback() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.hotbar(MouseScrollDelta::LineDelta(0., 1.), 1.), 1);
        assert_eq!(wheel.hotbar(MouseScrollDelta::LineDelta(0., -4.), 1.), -1);
        assert_eq!(wheel.hotbar(MouseScrollDelta::LineDelta(2., 0.), 1.), -1);
        assert_eq!(wheel.push([0., f64::NAN]), [0, 0]);
    }
}
