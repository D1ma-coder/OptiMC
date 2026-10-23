use super::*;
use crate::render::chat_editor::Editor;
use std::time::Instant;
#[derive(Default)]
pub(super) struct State {
    seen: Option<(u64, u64)>,
    active: Option<Dialog>,
    pub(super) pending: Option<(u64, crate::net::request::Request)>,
}
struct Dialog {
    epoch: u64,
    open: crate::net::sign_edit::Open,
    lines: [Editor; 4],
    line: usize,
    control: bool,
    shift: bool,
    started: Instant,
}
fn insert_validated(editor: &mut Editor, input: &str, valid: impl Fn(&str) -> bool) {
    let start = editor.selected().start;
    let mut candidate = editor.clone();
    candidate.insert_limited(input, 384);
    if valid(&candidate.text) {
        *editor = candidate;
    } else {
        editor.move_to(start, false);
    }
}
impl Ui {
    pub fn reset_sign_editor(&mut self) {
        self.sign_edit = State::default();
    }
    pub fn sync_sign_editor(&mut self, epoch: u64, store: &crate::net::sign_edit::Store) {
        if self.sign_edit.seen.is_some_and(|(old, _)| old != epoch) {
            self.sign_edit = State::default();
            if self.screen == Screen::SignEditor {
                self.screen = Screen::Connecting;
            }
        }
        let Some(open) = &store.open else {
            return;
        };
        if self.sign_edit.seen == Some((epoch, open.serial)) {
            return;
        }
        self.sign_edit.seen = Some((epoch, open.serial));
        let lines = std::array::from_fn(|i| {
            let mut e = Editor::default();
            e.open(&open.lines[i]);
            e
        });
        self.sign_edit.active = Some(Dialog {
            epoch,
            open: open.clone(),
            lines,
            line: 0,
            control: false,
            shift: false,
            started: Instant::now(),
        });
        self.in_world = true;
        self.screen = Screen::SignEditor;
    }
    pub(super) fn finish_sign_edit(&mut self) {
        if let Some(dialog) = self.sign_edit.active.take() {
            self.sign_edit.pending = Some((
                dialog.epoch,
                crate::net::request::Request::SignUpdate {
                    serial: dialog.open.serial,
                    position: dialog.open.position,
                    front: dialog.open.front,
                    lines: dialog.lines.map(|e| e.text),
                },
            ));
        }
        self.screen = Screen::Playing;
    }
    pub(super) fn sign_event(&mut self, event: &WindowEvent) {
        use winit::event::Ime;
        let Some(mut d) = self.sign_edit.active.take() else {
            return;
        };
        let mut input = None;
        match event {
            WindowEvent::ModifiersChanged(m) => {
                d.control = if cfg!(target_os = "macos") {
                    m.state().super_key()
                } else {
                    m.state().control_key()
                };
                d.shift = m.state().shift_key();
            }
            WindowEvent::Focused(false) => {
                d.control = false;
                d.shift = false;
            }
            WindowEvent::Ime(Ime::Commit(value)) => input = Some(value.as_str()),
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let editor = &mut d.lines[d.line];
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowUp) => {
                        d.line = (d.line + 3) & 3;
                        let end = d.lines[d.line].text.len();
                        d.lines[d.line].move_to(end, false);
                    }
                    PhysicalKey::Code(
                        KeyCode::ArrowDown | KeyCode::Enter | KeyCode::NumpadEnter,
                    ) => {
                        d.line = (d.line + 1) & 3;
                        let end = d.lines[d.line].text.len();
                        d.lines[d.line].move_to(end, false);
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        editor.horizontal(false, d.control, d.shift)
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        editor.horizontal(true, d.control, d.shift)
                    }
                    PhysicalKey::Code(KeyCode::Home) => editor.move_to(0, d.shift),
                    PhysicalKey::Code(KeyCode::End) => {
                        let end = editor.text.len();
                        editor.move_to(end, d.shift);
                    }
                    PhysicalKey::Code(KeyCode::Backspace) => editor.delete(false, d.control),
                    PhysicalKey::Code(KeyCode::Delete) => editor.delete(true, d.control),
                    PhysicalKey::Code(KeyCode::KeyA) if d.control => {
                        editor.anchor = 0;
                        editor.cursor = editor.text.len();
                    }
                    _ if !d.control => input = event.text.as_deref(),
                    _ => {}
                }
            }
            _ => {}
        }
        if let Some(input) =
            input.filter(|value| value.chars().any(|c| c >= ' ' && c != '\u{7f}' && c != '§'))
        {
            let width = if d.open.hanging { 60. } else { 90. };
            insert_validated(&mut d.lines[d.line], input, |text| {
                self.text_width(text) <= width
            });
        }
        self.sign_edit.active = Some(d);
    }
    pub(super) fn sign_screen(&mut self, actions: &mut Vec<Action>) {
        let Some(d) = self.sign_edit.active.take() else {
            return;
        };
        let center = (self.width * 0.5).floor();
        self.centered(
            if d.open.hanging {
                "Редактирование подвесной таблички"
            } else {
                "Редактирование таблички"
            },
            40.,
            [1.; 4],
        );
        let y = if d.open.hanging { 125. } else { 90. };
        let (scale, w, h, offset) = if d.open.hanging {
            (4.5, 16., 16., -13.)
        } else {
            (3.9, 24., if d.open.wall { 12. } else { 26. }, 27.)
        };
        let key = format!(
            "sign_editor_{}_{}",
            if d.open.hanging { "hanging" } else { "plain" },
            d.open.wood
        );
        if let Some(sprite) = self.sprites.get(&key).copied() {
            let mut uv = sprite.uv;
            uv[3] = uv[1] + (uv[3] - uv[1]) * h / sprite.size[1];
            self.quad(
                Rect {
                    x: center - w * 0.5 * scale,
                    y: y + offset - if d.open.hanging { 8. } else { 13. } * scale,
                    w: w * scale,
                    h: h * scale,
                },
                uv,
                [1.; 4],
            );
        }
        let text_scale = if d.open.hanging { 1. } else { 0.976_562_8 };
        let line_height = if d.open.hanging { 9. } else { 10. };
        let color = [
            ((d.open.color >> 16) & 255) as f32 / 255.,
            ((d.open.color >> 8) & 255) as f32 / 255.,
            (d.open.color & 255) as f32 / 255.,
            1.,
        ];
        for (row, e) in d.lines.iter().enumerate() {
            let width = self.text_width(&e.text);
            let x = center - width * 0.5 * text_scale;
            let line_y = y + (row as f32 * line_height - 2. * line_height) * text_scale;
            let start = self.vertices.len();
            self.text(
                &e.text,
                -width * 0.5,
                row as f32 * line_height - 2. * line_height,
                color,
            );
            for v in &mut self.vertices[start..] {
                let px = (v.position[0] + 1.) * 0.5 * self.width;
                let py = (1. - v.position[1]) * 0.5 * self.height;
                v.position = [
                    (center + px * text_scale) / self.width * 2. - 1.,
                    1. - (y + py * text_scale) / self.height * 2.,
                ];
            }
            if row == d.line {
                let selection = e.selected();
                if !selection.is_empty() {
                    let a = self.text_width(&e.text[..selection.start]) * text_scale;
                    let b = self.text_width(&e.text[..selection.end]) * text_scale;
                    self.fill(
                        Rect {
                            x: x + a,
                            y: line_y,
                            w: b - a,
                            h: 9. * text_scale,
                        },
                        [0.1, 0.3, 1., 0.35],
                    );
                }
                if (d.started.elapsed().as_millis() / 300) & 1 == 0 {
                    let at = x + self.text_width(&e.text[..e.cursor]) * text_scale;
                    self.fill(
                        Rect {
                            x: at,
                            y: line_y,
                            w: 1.,
                            h: 9. * text_scale,
                        },
                        color,
                    );
                }
            }
        }
        self.sign_edit.active = Some(d);
        if self.button(
            "Готово",
            Rect {
                x: center - 100.,
                y: (self.height / 4.).floor() + 144.,
                w: 200.,
                h: 20.,
            },
            true,
        ) {
            self.finish_sign_edit();
            actions.push(Action::Resume);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_transitions_match_original_text_field_helper() {
        let r = crate::net::sign_edit::reference();
        let mut e = Editor::default();
        e.open("Hello 🌲 мир");
        for row in r["edits"].as_array().unwrap() {
            match row["op"].as_str().unwrap() {
                "open" => {}
                "left" => e.horizontal(false, false, false),
                "word_select" => e.horizontal(false, true, true),
                "home" => e.move_to(0, false),
                "delete" => e.delete(true, false),
                "all" => {
                    e.anchor = 0;
                    e.cursor = e.text.len();
                }
                "end" => {
                    let n = e.text.len();
                    e.move_to(n, false);
                }
                "left_select" => e.horizontal(false, false, true),
                "backspace" => e.delete(false, false),
                "insert" => {
                    insert_validated(&mut e, row["input"].as_str().unwrap(), |text| {
                        text.chars().count() <= 12
                    });
                }
                other => panic!("Unexpected {other}"),
            }
            assert_eq!(e.text, row["text"].as_str().unwrap(), "{:?}", row["op"]);
            assert_eq!(
                e.text[..e.cursor].encode_utf16().count(),
                row["cursor"].as_u64().unwrap() as usize
            );
            assert_eq!(
                e.text[..e.anchor].encode_utf16().count(),
                row["anchor"].as_u64().unwrap() as usize
            );
        }
    }
}
