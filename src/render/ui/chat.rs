use super::*;
use crate::{
    net::commands::{Suggestion, Suggestions},
    render::chat_editor::Editor,
};
use std::time::{Duration, Instant};
pub(super) struct State {
    pub editor: Editor,
    control: bool,
    submit: bool,
    changed: Instant,
    last_input: String,
    request: i32,
    pending: Option<(i32, String)>,
    options: Option<Suggestions>,
    selected: usize,
    scroll: usize,
    pub log_scroll: usize,
    cycling: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            editor: Default::default(),
            control: false,
            submit: false,
            changed: Instant::now(),
            last_input: String::new(),
            request: 0,
            pending: None,
            options: None,
            selected: 0,
            scroll: 0,
            log_scroll: 0,
            cycling: false,
        }
    }
}
fn matches(prefix: &str, name: &str) -> bool {
    let prefix = prefix.to_lowercase();
    let name = name.to_lowercase();
    name.starts_with(&prefix) || name.split('_').any(|part| part.starts_with(&prefix))
}
impl Ui {
    pub fn open_chat(&mut self, slash: bool) {
        self.chat.editor.open(if slash { "/" } else { "" });
        self.chat.submit = false;
        self.chat.changed = Instant::now();
        self.chat.last_input = String::new();
        self.chat.pending = None;
        self.chat.options = None;
        self.chat.log_scroll = 0;
        self.chat.cycling = false;
        self.screen = Screen::Chat;
    }
    pub fn chat_error(&mut self, error: String) {
        let messages = std::sync::Arc::make_mut(&mut self.server_messages);
        if messages.len() == 100 {
            messages.pop_front();
        }
        messages.push_back(crate::net::ServerMessage {
            text: crate::text::RichText::plain(&error),
            overlay: false,
            received: Instant::now(),
            signature: None,
            delete_at: None,
        });
    }
    fn apply_completion(&mut self) {
        if let Some(options) = &mut self.chat.options {
            if let Some(value) = options.entries.get(self.chat.selected) {
                if self
                    .chat
                    .editor
                    .replace_utf16(options.start, options.length, &value.text)
                {
                    options.length = value.text.encode_utf16().count();
                    self.chat.last_input = self.chat.editor.text.clone();
                }
            }
        }
    }
    pub(super) fn chat_event(&mut self, event: &WindowEvent) {
        use winit::event::{Ime, MouseScrollDelta};
        let before = (self.chat.editor.text.clone(), self.chat.editor.cursor);
        let mut cycle = false;
        match event {
            WindowEvent::ModifiersChanged(m) => {
                self.chat.control = if cfg!(target_os = "macos") {
                    m.state().super_key()
                } else {
                    m.state().control_key()
                };
            }
            WindowEvent::Focused(false) => self.chat.control = false,
            WindowEvent::Ime(Ime::Commit(text)) => self.chat.editor.insert(text),
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y as i32,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 20.) as i32,
                };
                self.chat.log_scroll = self
                    .chat
                    .log_scroll
                    .saturating_add_signed(lines as isize * 3);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let shift = self.shift;
                let control = self.chat.control;
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                        self.chat.submit = true
                    }
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        self.chat.editor.delete(false, control)
                    }
                    PhysicalKey::Code(KeyCode::Delete) => self.chat.editor.delete(true, control),
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        self.chat.editor.horizontal(false, control, shift)
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        self.chat.editor.horizontal(true, control, shift)
                    }
                    PhysicalKey::Code(KeyCode::Home) => self.chat.editor.move_to(0, shift),
                    PhysicalKey::Code(KeyCode::End) => {
                        let end = self.chat.editor.text.len();
                        self.chat.editor.move_to(end, shift);
                    }
                    PhysicalKey::Code(KeyCode::KeyA) if control => {
                        self.chat.editor.anchor = 0;
                        self.chat.editor.cursor = self.chat.editor.text.len();
                    }
                    PhysicalKey::Code(KeyCode::Tab) => {
                        if let Some(options) = &self.chat.options {
                            let n = options.entries.len();
                            if n > 0 {
                                if self.chat.cycling {
                                    self.chat.selected = if shift {
                                        (self.chat.selected + n - 1) % n
                                    } else {
                                        (self.chat.selected + 1) % n
                                    };
                                }
                                self.apply_completion();
                                self.chat.cycling = true;
                                cycle = true;
                            }
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowUp | KeyCode::ArrowDown) => {
                        let up = event.physical_key == PhysicalKey::Code(KeyCode::ArrowUp);
                        if let Some(options) = &self.chat.options {
                            let n = options.entries.len();
                            if n > 0 {
                                self.chat.selected = if up {
                                    (self.chat.selected + n - 1) % n
                                } else {
                                    (self.chat.selected + 1) % n
                                };
                                cycle = true;
                            }
                        } else {
                            self.chat.editor.history(up);
                        }
                    }
                    PhysicalKey::Code(KeyCode::PageUp) => {
                        self.chat.log_scroll = self.chat.log_scroll.saturating_add(19)
                    }
                    PhysicalKey::Code(KeyCode::PageDown) => {
                        self.chat.log_scroll = self.chat.log_scroll.saturating_sub(19)
                    }
                    _ => {
                        if !control {
                            if let Some(text) = &event.text {
                                self.chat.editor.insert(text);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        if !cycle && before != (self.chat.editor.text.clone(), self.chat.editor.cursor) {
            self.chat.changed = Instant::now();
            self.chat.options = None;
            self.chat.pending = None;
            self.chat.selected = 0;
            self.chat.scroll = 0;
            self.chat.cycling = false;
        }
    }
    fn update_completions(&mut self, actions: &mut Vec<Action>) {
        let text = &self.chat.editor.text[..self.chat.editor.cursor];
        if let Some((id, input)) = &self.chat.pending {
            if input == text {
                if let Some(suggestions) = self
                    .server_commands
                    .suggestions
                    .as_ref()
                    .filter(|s| s.request == *id)
                {
                    self.chat.options = Some(suggestions.clone());
                    return;
                }
            }
        }
        if text == self.chat.last_input || self.chat.changed.elapsed() < Duration::from_millis(100)
        {
            return;
        }
        let text = text.to_owned();
        self.chat.last_input = text.clone();
        self.chat.selected = 0;
        if text.starts_with('/') && text.contains(' ') {
            self.chat.request = self.chat.request.wrapping_add(1);
            self.chat.pending = Some((self.chat.request, text.clone()));
            actions.push(Action::Suggest {
                id: self.chat.request,
                input: text,
            });
            return;
        }
        let byte_start = text.rfind(' ').map_or(0, |i| i + 1);
        let start = if text.starts_with('/') { 1 } else { byte_start };
        let prefix = &text[start..];
        let mut values = if text.starts_with('/') {
            self.server_commands.tree.root_literals(prefix)
        } else {
            self.server_players
                .players
                .values()
                .map(|p| p.profile.name.clone())
                .chain(self.server_commands.custom.iter().cloned())
                .filter(|name| matches(prefix, name))
                .collect::<Vec<_>>()
        };
        values.sort_by_key(|s| s.to_lowercase());
        values.dedup();
        self.chat.options = if values.is_empty() {
            None
        } else {
            Some(Suggestions {
                request: -1,
                start: text[..start].encode_utf16().count(),
                length: prefix.encode_utf16().count(),
                entries: values
                    .into_iter()
                    .map(|text| Suggestion {
                        text,
                        tooltip: None,
                    })
                    .collect(),
            })
        };
    }
    pub(super) fn chat_screen(&mut self, actions: &mut Vec<Action>) {
        if self.chat.submit {
            self.chat.submit = false;
            if let Some(text) = self.chat.editor.submit() {
                actions.push(Action::Chat(text));
            }
            self.screen = Screen::Playing;
            actions.push(Action::Resume);
            return;
        }
        self.update_completions(actions);
        self.fill(
            Rect {
                x: 2.,
                y: self.height - 14.,
                w: self.width - 4.,
                h: 12.,
            },
            [0., 0., 0., 0.5],
        );
        let text = self.chat.editor.text.clone();
        let cursor = self.chat.editor.cursor;
        let mut start = 0;
        while self.text_width(&text[start..cursor]) > self.width - 12. {
            start += text[start..].chars().next().unwrap().len_utf8();
        }
        let mut end = text.len();
        while self.text_width(&text[start..end]) > self.width - 8. {
            end -= text[..end].chars().next_back().unwrap().len_utf8();
        }
        let selected = self.chat.editor.selected();
        let a = selected.start.max(start).min(end);
        let b = selected.end.max(start).min(end);
        if a < b {
            self.fill(
                Rect {
                    x: 4. + self.text_width(&text[start..a]),
                    y: self.height - 13.,
                    w: self.text_width(&text[a..b]),
                    h: 10.,
                },
                [0., 0., 1., 0.5],
            );
        }
        self.text(&text[start..end], 4., self.height - 12., [1.; 4]);
        let caret = 4. + self.text_width(&text[start..cursor]);
        if self.chat.changed.elapsed().as_millis() % 1000 < 500 {
            self.fill(
                Rect {
                    x: caret,
                    y: self.height - 13.,
                    w: 1.,
                    h: 10.,
                },
                [1.; 4],
            );
        }
        if let Some(options) = self.chat.options.clone().filter(|o| !o.entries.is_empty()) {
            let count = options.entries.len().min(10);
            let first = self.chat.selected.saturating_sub(count - 1);
            let width = options
                .entries
                .iter()
                .map(|e| self.text_width(&e.text))
                .fold(0., f32::max)
                + 2.;
            let x = 4.;
            let y = self.height - 15. - count as f32 * 12.;
            self.fill(
                Rect {
                    x,
                    y,
                    w: width,
                    h: count as f32 * 12.,
                },
                [0., 0., 0., 0.9],
            );
            for (index, entry) in options.entries.iter().enumerate().skip(first).take(count) {
                let yy = y + (index - first) as f32 * 12.;
                let color = if index == self.chat.selected {
                    [1., 1., 0., 1.]
                } else {
                    [0.66, 0.66, 0.66, 1.]
                };
                self.text(&entry.text, x + 1., yy + 2., color);
            }
        }
    }
}
