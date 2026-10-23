use super::*;
use crate::net::scoreboard::{Sidebar, Store};
use std::sync::Arc;

pub(super) struct SidebarCache {
    board: Arc<Store>,
    player: String,
    sidebar: Option<Arc<Sidebar>>,
}

#[derive(Debug, PartialEq)]
struct Layout {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}
fn layout(width: i32, height: i32, content: i32, rows: usize) -> Layout {
    let bottom = height / 2 + rows as i32 * 9 / 3;
    Layout {
        left: width - content - 3,
        right: width - 1,
        top: bottom - rows as i32 * 9,
        bottom,
    }
}

impl Ui {
    fn scoreboard_line(&mut self, line: &[crate::text::StyledChar], x: f32, y: f32) {
        let mut line = line.to_vec();
        for ch in &mut line {
            ch.style.shadow = Some(0);
        }
        self.styled_line(&line, x, y, [1.; 4]);
    }
    pub(super) fn scoreboard_sidebar(&mut self) {
        if self.sidebar_cache.as_ref().is_none_or(|cache| {
            !Arc::ptr_eq(&cache.board, &self.server_scoreboard) || cache.player != self.username
        }) {
            self.sidebar_cache = Some(SidebarCache {
                board: self.server_scoreboard.clone(),
                player: self.username.clone(),
                sidebar: self.server_scoreboard.sidebar(&self.username).map(Arc::new),
            });
        }
        let Some(sidebar) = self.sidebar_cache.as_ref().and_then(|c| c.sidebar.clone()) else {
            return;
        };
        let chars = |text: &crate::text::RichText| {
            self.styled_lines(text, f32::MAX)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        let title = chars(&sidebar.title);
        let rows: Vec<_> = sidebar
            .rows
            .iter()
            .map(|(name, score)| (chars(name), chars(score)))
            .collect();
        let title_width = self.styled_width(&title).ceil() as i32;
        let separator = self.text_width(": ").ceil() as i32;
        let width = rows.iter().fold(title_width, |width, (name, score)| {
            let score_width = self.styled_width(score).ceil() as i32;
            width.max(
                self.styled_width(name).ceil() as i32
                    + if score_width > 0 {
                        separator + score_width
                    } else {
                        0
                    },
            )
        });
        let l = layout(self.width as i32, self.height as i32, width, rows.len());
        let body = Rect {
            x: (l.left - 2) as f32,
            y: (l.top - 1) as f32,
            w: (l.right - l.left + 2) as f32,
            h: (l.bottom - l.top + 1) as f32,
        };
        // Original backgroundForChatOnly=true defaults: ARGB float alpha truncation.
        self.fill(
            Rect {
                y: (l.top - 10) as f32,
                h: 9.,
                ..body
            },
            [0., 0., 0., 102. / 255.],
        );
        self.fill(body, [0., 0., 0., 76. / 255.]);
        self.scoreboard_line(
            &title,
            (l.left + width / 2 - title_width / 2) as f32,
            (l.top - 9) as f32,
        );
        for (i, (name, score)) in rows.iter().enumerate() {
            let y = (l.top + i as i32 * 9) as f32;
            self.scoreboard_line(name, l.left as f32, y);
            self.scoreboard_line(score, l.right as f32 - self.styled_width(score).ceil(), y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidebar_uses_original_integer_anchor_for_empty_and_fifteen_rows() {
        assert_eq!(
            layout(320, 240, 100, 15),
            Layout {
                left: 217,
                right: 319,
                top: 30,
                bottom: 165
            }
        );
        assert_eq!(
            layout(427, 241, 99, 0),
            Layout {
                left: 325,
                right: 426,
                top: 120,
                bottom: 120
            }
        );
    }
}
