use super::*;
use crate::text::{RichText, StyledChar};
impl Ui {
    pub(super) fn server_chat(&mut self) {
        let messages = self.server_messages.clone();
        let open = self.screen == Screen::Chat;
        let limit = if open { 20 } else { 10 };
        let mut lines = Vec::new();
        for message in messages
            .iter()
            .rev()
            .filter(|m| !m.overlay && (open || m.received.elapsed().as_secs_f32() < 10.))
        {
            let deleted=message.delete_at.filter(|at|*at<=std::time::Instant::now()).map(|_|RichText::from_json(&serde_json::json!({"translate":"chat.deleted_marker","color":"gray","italic":true})).unwrap_or_default());
            let text = deleted.as_ref().unwrap_or(&message.text);
            let alpha = if open {
                1.
            } else {
                (10. - message.received.elapsed().as_secs_f32()).clamp(0., 1.)
            };
            for line in self
                .styled_lines(text, (self.width - 4.).min(320.))
                .iter()
                .rev()
            {
                lines.push((line.clone(), alpha));
            }
        }
        let scroll = if open {
            self.chat.log_scroll.min(lines.len().saturating_sub(limit))
        } else {
            0
        };
        self.chat.log_scroll = scroll;
        for (row, (line, alpha)) in lines.iter().skip(scroll).take(limit).enumerate() {
            let y = self.height - 49. - row as f32 * 9.;
            self.fill(
                Rect {
                    x: 1.,
                    y: y - 1.,
                    w: self.styled_width(line) + 4.,
                    h: 9.,
                },
                [0., 0., 0., 0.5 * alpha],
            );
            self.styled_line(line, 2., y, [1., 1., 1., *alpha]);
        }
        if let Some(overlay) = messages.iter().rev().find(|m| m.overlay).filter(|m| {
            self.server_hud
                .action
                .as_ref()
                .is_none_or(|(_, at)| m.received >= *at)
        }) {
            let alpha = (3. - overlay.received.elapsed().as_secs_f32()).clamp(0., 1.);
            if alpha > 0. {
                for (i, line) in self
                    .styled_lines(&overlay.text, self.width - 20.)
                    .iter()
                    .take(2)
                    .enumerate()
                {
                    self.centered_styled_line(
                        line,
                        self.height - 68. + i as f32 * 9.,
                        [1., 1., 1., alpha],
                    );
                }
            }
        }
    }

    pub(super) fn styled_width(&self, line: &[StyledChar]) -> f32 {
        line.iter().copied().map(|c| self.styled_advance(c)).sum()
    }
    fn styled_advance(&self, c: StyledChar) -> f32 {
        self.text_width(&c.ch.to_string()) + if c.style.bold { 1. } else { 0. }
    }
    pub(super) fn styled_lines(&self, text: &RichText, width: f32) -> Vec<Vec<StyledChar>> {
        text.lines(width, |c| self.styled_advance(c))
    }
    pub(super) fn styled_line(&mut self, line: &[StyledChar], mut x: f32, y: f32, base: [f32; 4]) {
        for (index, c) in line.iter().copied().enumerate() {
            let advance = self.styled_advance(c);
            let mut ch = c.ch;
            if c.style.obfuscated && ch != ' ' {
                let tick = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    / 50;
                let candidates: Vec<_> =
                    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
                        .chars()
                        .filter(|&other| {
                            self.text_width(&other.to_string())
                                == advance - if c.style.bold { 1. } else { 0. }
                        })
                        .collect();
                if !candidates.is_empty() {
                    ch = candidates[(tick as usize + index) % candidates.len()];
                }
            }
            let color = if let Some(rgb) = c.style.color {
                [
                    (rgb >> 16 & 255) as f32 / 255.,
                    (rgb >> 8 & 255) as f32 / 255.,
                    (rgb & 255) as f32 / 255.,
                    base[3],
                ]
            } else {
                base
            };
            let shadow = c
                .style
                .shadow
                .map(|argb| {
                    [
                        (argb >> 16 & 255) as f32 / 255.,
                        (argb >> 8 & 255) as f32 / 255.,
                        (argb & 255) as f32 / 255.,
                        (argb >> 24) as f32 / 255. * base[3],
                    ]
                })
                .unwrap_or([color[0] * 0.25, color[1] * 0.25, color[2] * 0.25, color[3]]);
            if ch != ' ' {
                if let Some(glyph) = self
                    .glyphs
                    .get(&ch)
                    .or_else(|| self.glyphs.get(&'?'))
                    .copied()
                {
                    let rect = Rect {
                        x,
                        y: y + glyph.offset,
                        w: glyph.sprite.size[0],
                        h: glyph.sprite.size[1],
                    };
                    for (offset, tint) in [(1., shadow), (0., color)] {
                        if tint[3] == 0. {
                            continue;
                        }
                        for bold in 0..=u8::from(c.style.bold) {
                            let rect = Rect {
                                x: rect.x + offset + bold as f32,
                                y: rect.y + offset,
                                ..rect
                            };
                            let start = self.vertices.len();
                            self.quad(rect, glyph.sprite.uv, tint);
                            if c.style.italic {
                                for vertex in &mut self.vertices[start..] {
                                    let vy = (1. - vertex.position[1]) * self.height * 0.5;
                                    let skew = 1. - 2. * ((vy - rect.y) / rect.h);
                                    vertex.position[0] += skew * 2. / self.width;
                                }
                            }
                        }
                    }
                }
            }
            for (enabled, dy) in [(c.style.strike, 4.), (c.style.underline, 8.)] {
                if enabled {
                    self.fill(
                        Rect {
                            x: x + 1.,
                            y: y + dy + 1.,
                            w: advance,
                            h: 1.,
                        },
                        shadow,
                    );
                    self.fill(
                        Rect {
                            x,
                            y: y + dy,
                            w: advance,
                            h: 1.,
                        },
                        color,
                    );
                }
            }
            x += advance;
        }
    }
    pub(super) fn centered_styled_line(&mut self, line: &[StyledChar], y: f32, color: [f32; 4]) {
        let x = ((self.width - self.styled_width(line)) * 0.5).round();
        self.styled_line(line, x, y, color);
    }
}
