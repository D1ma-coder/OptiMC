//! Rust transcription of 26.2 Window.calculateScale / setGuiScale.
//! Reference: the locally imported official client JAR, not OS logical DPI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiScale {
    pub factor: u32,
    pub width: u32,
    pub height: u32,
}
impl GuiScale {
    pub fn calculate(framebuffer: [u32; 2], requested: u32, enforce_unicode: bool) -> Self {
        let [width, height] = framebuffer;
        let mut scale = 1;
        while scale != requested
            && scale < width
            && scale < height
            && width / (scale + 1) >= 320
            && height / (scale + 1) >= 240
        {
            scale += 1;
        }
        if enforce_unicode && !scale.is_multiple_of(2) {
            scale += 1;
        }
        Self {
            factor: scale,
            width: width.div_ceil(scale),
            height: height.div_ceil(scale),
        }
    }
    pub fn pointer(self, physical: [f32; 2], framebuffer: [u32; 2]) -> [f32; 2] {
        [
            physical[0] * self.width as f32 / framebuffer[0].max(1) as f32,
            physical[1] * self.height as f32 / framebuffer[1].max(1) as f32,
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_uses_original_320_by_240_minimum() {
        for (size, scale, gui) in [
            ([640, 480], 2, [320, 240]),
            ([1440, 900], 3, [480, 300]),
            ([1920, 1080], 4, [480, 270]),
            ([2880, 1800], 7, [412, 258]),
            ([3840, 2160], 9, [427, 240]),
            ([319, 239], 1, [319, 239]),
        ] {
            let actual = GuiScale::calculate(size, 0, false);
            assert_eq!(actual.factor, scale);
            assert_eq!([actual.width, actual.height], gui);
        }
    }
    #[test]
    fn manual_scale_unicode_rounding_and_mouse_use_framebuffer() {
        assert_eq!(GuiScale::calculate([1440, 900], 2, false).factor, 2);
        assert_eq!(GuiScale::calculate([1440, 900], 100, false).factor, 3);
        assert_eq!(GuiScale::calculate([1440, 900], 0, true).factor, 4);
        let odd = GuiScale::calculate([1441, 901], 3, false);
        assert_eq!((odd.width, odd.height), (481, 301));
        assert_eq!(odd.pointer([720.5, 450.5], [1441, 901]), [240.5, 150.5]);
    }
}
