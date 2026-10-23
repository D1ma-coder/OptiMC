//! Offline player-list overlay: original 80-player cap, column-major 20-row
//! layout, rich header/footer/names, integer objective formats and ping sprites.
use super::*;
use crate::{
    net::{player_info, scoreboard},
    text::RichText,
};
use std::sync::Arc;
#[derive(Clone)]
struct Row {
    name: RichText,
    score: RichText,
    latency: i32,
    spectator: bool,
}
pub(super) struct Cache {
    players: Arc<player_info::Store>,
    scores: Arc<scoreboard::Store>,
    rows: Arc<Vec<Row>>,
}
#[derive(Debug)]
struct Layout {
    columns: usize,
    rows: usize,
    cell: f32,
    x: f32,
    total: f32,
}
fn layout(count: usize, width: f32, name: f32, score: f32) -> Layout {
    let count = count.min(80);
    let mut columns = 1;
    let mut rows = count;
    while rows > 20 {
        columns += 1;
        rows = count.div_ceil(columns);
    }
    let cell = (((name + score + 13.) * columns as f32).min(width - 50.) / columns as f32).floor();
    let total = cell * columns as f32 + 5. * (columns - 1) as f32;
    Layout {
        columns,
        rows,
        cell,
        x: (width * 0.5).floor() - (total * 0.5).floor(),
        total,
    }
}
impl Ui {
    fn player_rows(&mut self) -> Arc<Vec<Row>> {
        let changed = self.tab_cache.as_ref().is_none_or(|c| {
            !Arc::ptr_eq(&c.players, &self.server_players)
                || !Arc::ptr_eq(&c.scores, &self.server_scoreboard)
        });
        if changed {
            let rows = self
                .server_players
                .listed(&self.server_scoreboard)
                .into_iter()
                .map(|p| Row {
                    name: player_info::Store::display_name(p, &self.server_scoreboard),
                    score: if p.game_mode == 3 {
                        RichText::default()
                    } else {
                        self.server_scoreboard
                            .list_score(&p.profile.name)
                            .unwrap_or_default()
                    },
                    latency: p.latency,
                    spectator: p.game_mode == 3,
                })
                .collect();
            self.tab_cache = Some(Cache {
                players: self.server_players.clone(),
                scores: self.server_scoreboard.clone(),
                rows: Arc::new(rows),
            });
        }
        self.tab_cache.as_ref().unwrap().rows.clone()
    }
    pub(super) fn player_list(&mut self) {
        let rows = self.player_rows();
        let lines: Vec<_> = rows
            .iter()
            .map(|r| {
                (
                    self.styled_lines(&r.name, f32::MAX)
                        .into_iter()
                        .next()
                        .unwrap_or_default(),
                    self.styled_lines(&r.score, f32::MAX)
                        .into_iter()
                        .next()
                        .unwrap_or_default(),
                )
            })
            .collect();
        let max_name = lines
            .iter()
            .map(|(n, _)| self.styled_width(n))
            .fold(0., f32::max);
        let score = lines
            .iter()
            .filter(|(_, s)| !s.is_empty())
            .map(|(_, s)| self.styled_width(s))
            .fold(0., f32::max);
        let gap = if score > 0. { 1. + score } else { 0. };
        let layout = layout(rows.len(), self.width, max_name, gap);
        let header = self.server_players.header.clone();
        let footer = self.server_players.footer.clone();
        let header = if header.plain_text().is_empty() {
            Vec::new()
        } else {
            self.styled_lines(&header, self.width - 50.)
        };
        let footer = if footer.plain_text().is_empty() {
            Vec::new()
        } else {
            self.styled_lines(&footer, self.width - 50.)
        };
        let width = header
            .iter()
            .chain(&footer)
            .map(|l| self.styled_width(l))
            .fold(layout.total, f32::max);
        let mut y = 10.;
        let panel_x = (self.width * 0.5).floor() - (width * 0.5).floor() - 1.;
        if !header.is_empty() {
            self.fill(
                Rect {
                    x: panel_x,
                    y: y - 1.,
                    w: width + 2.,
                    h: header.len() as f32 * 9. + 1.,
                },
                [0., 0., 0., 0.5],
            );
            for line in header {
                self.centered_styled_line(&line, y, [1.; 4]);
                y += 9.;
            }
            y += 1.;
        }
        self.fill(
            Rect {
                x: panel_x,
                y: y - 1.,
                w: width + 2.,
                h: layout.rows as f32 * 9. + 1.,
            },
            [0., 0., 0., 0.5],
        );
        for (index, (row, (name, score))) in rows.iter().zip(&lines).enumerate() {
            let column = index / layout.rows.max(1);
            let line = index % layout.rows.max(1);
            debug_assert!(column < layout.columns);
            let x = layout.x + column as f32 * (layout.cell + 5.);
            let yy = y + line as f32 * 9.;
            self.fill(
                Rect {
                    x,
                    y: yy,
                    w: layout.cell,
                    h: 8.,
                },
                [1., 1., 1., 0.125],
            );
            let alpha = if row.spectator { 0.565 } else { 1. };
            self.styled_line(name, x, yy, [1., 1., 1., alpha]);
            if !score.is_empty() {
                let score_x = x + max_name + gap - self.styled_width(score);
                self.styled_line(score, score_x, yy, [1.; 4]);
            }
            self.image(
                player_info::ping_sprite(row.latency),
                Rect {
                    x: x + layout.cell - 11.,
                    y: yy,
                    w: 10.,
                    h: 8.,
                },
            );
        }
        if !footer.is_empty() {
            y += layout.rows as f32 * 9. + 1.;
            self.fill(
                Rect {
                    x: panel_x,
                    y: y - 1.,
                    w: width + 2.,
                    h: footer.len() as f32 * 9. + 1.,
                },
                [0., 0., 0., 0.5],
            );
            for line in footer {
                self.centered_styled_line(&line, y, [1.; 4]);
                y += 9.;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn list_layout_matches_original_column_major_twenty_row_limit() {
        for (count, columns, rows) in [
            (0, 1, 0),
            (1, 1, 1),
            (20, 1, 20),
            (21, 2, 11),
            (40, 2, 20),
            (41, 3, 14),
            (60, 3, 20),
            (61, 4, 16),
            (80, 4, 20),
            (900, 4, 20),
        ] {
            let l = layout(count, 640., 80., 30.);
            assert_eq!((l.columns, l.rows), (columns, rows));
            assert!(l.total <= 640. - 35.);
            assert_eq!(l.cell, l.cell.floor());
        }
    }
}
