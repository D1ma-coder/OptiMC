use super::*;
impl Ui {
    fn progress_sprite(&mut self, name: &str, rect: Rect, fraction: f32) {
        let sprite = self.sprites[name];
        let pixels = (fraction.clamp(0., 1.) * 183.).floor().min(rect.w);
        if pixels <= 0. {
            return;
        }
        self.quad(
            Rect { w: pixels, ..rect },
            [
                sprite.uv[0],
                sprite.uv[1],
                sprite.uv[0] + (sprite.uv[2] - sprite.uv[0]) * pixels / rect.w,
                sprite.uv[3],
            ],
            [1.; 4],
        );
    }
    fn scaled_hud_text(&mut self, text: &crate::text::RichText, y: f32, scale: f32, alpha: f32) {
        let lines = self.styled_lines(text, f32::MAX);
        for (i, line) in lines.iter().enumerate() {
            let y = y + i as f32 * 9. * scale;
            let begin = self.vertices.len();
            self.centered_styled_line(line, y, [1., 1., 1., alpha]);
            let center = self.width * 0.5;
            for vertex in &mut self.vertices[begin..] {
                let x = (vertex.position[0] + 1.) * self.width * 0.5;
                let py = (1. - vertex.position[1]) * self.height * 0.5;
                vertex.position = [
                    (center + (x - center) * scale) / self.width * 2. - 1.,
                    1. - (y + (py - y) * scale) / self.height * 2.,
                ];
            }
        }
    }
    pub(super) fn synchronized_hud(&mut self) {
        let hud = self.server_hud.clone();
        if self.survival_hud {
            let rect = Rect {
                x: (self.width * 0.5).floor() - 91.,
                y: self.height - 29.,
                w: 182.,
                h: 5.,
            };
            self.image("xp_background", rect);
            self.progress_sprite("xp_progress", rect, hud.experience.progress);
            if hud.experience.level > 0 {
                let level = hud.experience.level.to_string();
                let x = (self.width - self.text_width(&level)) * 0.5;
                let y = self.height - 35.;
                for (dx, dy) in [(-1., 0.), (1., 0.), (0., -1.), (0., 1.)] {
                    self.text(&level, x + dx, y + dy, [0., 0., 0., 1.]);
                }
                self.text(&level, x, y, [128. / 255., 1., 32. / 255., 1.]);
            }
        }
        let colors = ["pink", "blue", "red", "green", "yellow", "purple", "white"];
        let overlays = ["", "notched_6", "notched_10", "notched_12", "notched_20"];
        for (i, boss) in hud.bosses.iter().enumerate() {
            let y = 12. + i as f32 * 19.;
            if y > self.height / 3. {
                break;
            }
            let rect = Rect {
                x: (self.width * 0.5).floor() - 91.,
                y,
                w: 182.,
                h: 5.,
            };
            let color = colors[boss.color as usize];
            self.image(&format!("boss_{color}_background"), rect);
            let overlay = overlays[boss.overlay as usize];
            if !overlay.is_empty() {
                self.image(&format!("boss_{overlay}_background"), rect);
            }
            self.progress_sprite(&format!("boss_{color}_progress"), rect, boss.progress());
            if !overlay.is_empty() {
                self.progress_sprite(&format!("boss_{overlay}_progress"), rect, boss.progress());
            }
            self.scaled_hud_text(&boss.name, y - 9., 1., 1.);
        }
        let alpha = hud.titles.alpha();
        if alpha > 0. {
            self.scaled_hud_text(&hud.titles.title, self.height * 0.5 - 40., 4., alpha);
            self.scaled_hud_text(&hud.titles.subtitle, self.height * 0.5 + 10., 2., alpha);
        }
        if let Some((text, at)) = &hud.action {
            let newer = self
                .server_messages
                .iter()
                .rev()
                .find(|m| m.overlay)
                .is_none_or(|m| m.received < *at);
            let alpha = (3. - at.elapsed().as_secs_f32()).clamp(0., 1.);
            if newer && alpha > 0. {
                self.scaled_hud_text(text, self.height - 68., 1., alpha);
            }
        }
    }
}
