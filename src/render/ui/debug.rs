use super::*;
impl Ui {
    pub(super) fn debug_screen(&mut self) {
        for (right, lines) in [
            (false, self.debug_left.clone()),
            (true, self.debug_right.clone()),
        ] {
            for (i, line) in lines.iter().enumerate() {
                let width = self.text_width(line);
                let x = if right { self.width - width - 2. } else { 2. };
                let y = 2. + i as f32 * 9.;
                self.fill(
                    Rect {
                        x: x - 1.,
                        y: y - 1.,
                        w: width + 2.,
                        h: 9.,
                    },
                    [0., 0., 0., 0.5],
                );
                self.text(line, x, y, [0.88, 0.88, 0.88, 1.]);
            }
        }
    }
}
