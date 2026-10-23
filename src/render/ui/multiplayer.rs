use super::*;
use crate::multiplayer::{self, Ping, Server};

fn rect(x: f32, y: f32, w: f32) -> Rect {
    Rect { x, y, w, h: 20. }
}
impl Ui {
    fn join_server(&mut self, actions: &mut Vec<Action>) {
        if !multiplayer::valid_username(&self.username)
            || crate::net::Address::parse(&self.server).is_err()
        {
            return;
        }
        let mut profile = self.accounts.active().clone();
        profile.username = self.username.clone();
        let result = if profile != *self.accounts.active() {
            self.accounts.edit(Some(self.accounts.book.active), profile)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            self.message = format!("Не удалось сохранить профиль: {e}");
            return;
        }
        actions.push(Action::Connect(self.server.clone(), self.username.clone()));
        self.screen = Screen::Connecting;
        self.focus = false;
        self.message = "Подключение...".into();
    }
    fn server_form(&mut self, edit: Option<usize>) {
        self.browser.editing = edit;
        if let Some(server) = edit.and_then(|i| self.browser.book.servers.get(i)) {
            self.browser.name = server.name.clone();
            self.server = server.address.clone();
        } else {
            self.browser.name = "Сервер Minecraft".into();
            self.server.clear();
        }
        self.name_focus = true;
        self.focus = true;
        self.message.clear();
        self.screen = Screen::ServerEdit;
    }
    pub(super) fn input_field(&mut self, value: &str, r: Rect, focused: bool) -> bool {
        self.fill(
            Rect {
                x: r.x - 1.,
                y: r.y - 1.,
                w: r.w + 2.,
                h: r.h + 2.,
            },
            if focused {
                [1.; 4]
            } else {
                [0.4, 0.4, 0.4, 1.]
            },
        );
        self.fill(r, [0., 0., 0., 0.9]);
        let suffix = if focused { "_" } else { "" };
        let mut visible = value.to_owned();
        while !visible.is_empty() && self.text_width(&visible) > r.w - 12. {
            visible.remove(0);
        }
        self.text(&format!("{visible}{suffix}"), r.x + 4., r.y + 6., [1.; 4]);
        self.click == Some(MouseButton::Left) && r.contains(self.mouse)
    }
    fn server_favicon(&mut self, address: &str, rect: Rect) {
        let Some(pixels) = self.browser.icons.get(address).cloned() else {
            self.image("unknown_server", rect);
            return;
        };
        self.dynamic_image(address, pixels, rect, [0., 0., 1., 1.]);
    }
    pub(super) fn dynamic_image(
        &mut self,
        address: &str,
        pixels: std::sync::Arc<image::RgbaImage>,
        rect: Rect,
        crop: [f32; 4],
    ) -> Sprite {
        let cached = self
            .favicon_slots
            .get(address)
            .filter(|(_, old)| std::sync::Arc::ptr_eq(old, &pixels))
            .map(|(slot, _)| *slot);
        let slot = if let Some(slot) = cached {
            slot
        } else {
            let slot = self
                .favicon_slots
                .get(address)
                .map(|(slot, _)| *slot)
                .unwrap_or_else(|| {
                    let slot = self.next_favicon_slot % 1024;
                    self.next_favicon_slot += 1;
                    self.favicon_slots.retain(|_, (i, _)| *i != slot);
                    slot
                });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.favicon_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: (slot % 32) as u32 * 64,
                        y: (slot / 32) as u32 * 64,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                pixels.as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
                wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
            );
            self.favicon_slots
                .insert(address.to_owned(), (slot, pixels));
            slot
        };
        let x = (slot % 32) as f32 * 64. / 2048.;
        let y = (slot / 32) as f32 * 64. / 2048.;
        let start = self.vertices.len();
        self.quad(
            rect,
            [
                x + crop[0] * 64. / 2048.,
                y + crop[1] * 64. / 2048.,
                x + crop[2] * 64. / 2048.,
                y + crop[3] * 64. / 2048.,
            ],
            [1.; 4],
        );
        for vertex in &mut self.vertices[start..] {
            vertex.atlas = 1;
        }
        Sprite {
            uv: [
                x + crop[0] * 64. / 2048.,
                y + crop[1] * 64. / 2048.,
                x + crop[2] * 64. / 2048.,
                y + crop[3] * 64. / 2048.,
            ],
            size: [64. * (crop[2] - crop[0]), 64. * (crop[3] - crop[1])],
        }
    }
    pub(super) fn wrapped(&self, text: &str, width: f32) -> Vec<String> {
        wrap(text, width, |s| self.text_width(s))
    }
    pub(super) fn multiplayer_screen(&mut self, actions: &mut Vec<Action>) {
        let c = (self.width * 0.5).floor();
        let h = self.height;
        self.browser.poll();
        let selected = self
            .browser
            .selected
            .filter(|&i| i < self.browser.book.servers.len());
        match self.screen {
            Screen::Multiplayer => {
                self.centered("Сетевая игра", 16., [1.; 4]);
                let top = 33.;
                let bottom = h - 60.;
                self.fill(
                    Rect {
                        x: 0.,
                        y: top,
                        w: self.width,
                        h: (bottom - top).max(0.),
                    },
                    [0., 0., 0., 0.45],
                );
                let visible: Vec<_> = self
                    .browser
                    .book
                    .servers
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| {
                        s.extra
                            .get(&"hidden".into())
                            .and_then(crate::net::nbt::Tag::integer)
                            != Some(1)
                    })
                    .map(|(i, s)| (i, s.clone()))
                    .collect();
                let rows = ((bottom - top) / 36.).floor().max(1.) as usize;
                self.browser.scroll = self.browser.scroll.min(visible.len().saturating_sub(rows));
                let mut double_join = false;
                for (row, (index, server)) in visible
                    .iter()
                    .skip(self.browser.scroll)
                    .take(rows)
                    .enumerate()
                {
                    let r = Rect {
                        x: c - 152.,
                        y: top + row as f32 * 36.,
                        w: 305.,
                        h: 36.,
                    };
                    if selected == Some(*index) {
                        self.fill(r, [0.55, 0.55, 0.55, 1.]);
                        self.fill(
                            Rect {
                                x: r.x + 1.,
                                y: r.y + 1.,
                                w: r.w - 2.,
                                h: r.h - 2.,
                            },
                            [0., 0., 0., 1.],
                        );
                    }
                    if self.click == Some(MouseButton::Left) && r.contains(self.mouse) {
                        let now = std::time::Instant::now();
                        double_join = self.last_server_click.is_some_and(|(i, t)| {
                            i == *index && now.duration_since(t).as_millis() < 250
                        });
                        self.last_server_click = Some((*index, now));
                        self.browser.selected = Some(*index);
                    }
                    self.server_favicon(
                        &server.address,
                        Rect {
                            x: r.x + 2.,
                            y: r.y + 2.,
                            w: 32.,
                            h: 32.,
                        },
                    );
                    let (motd, players, ping, tooltip, color) =
                        match self.browser.statuses.get(&server.address) {
                            Some(Ping::Online {
                                motd,
                                players,
                                latency,
                                ..
                            }) => (
                                motd.clone(),
                                players.clone(),
                                multiplayer::ping_sprite(*latency),
                                format!("{latency:.0} мс"),
                                [0.65, 0.65, 0.65, 1.],
                            ),
                            Some(Ping::Failed(reason)) => (
                                crate::text::RichText::plain(&format!(
                                    "Не удаётся подключиться: {reason}"
                                )),
                                String::new(),
                                "ping_unreachable",
                                reason.clone(),
                                [1., 0.3, 0.3, 1.],
                            ),
                            _ => (
                                "Проверка соединения...".into(),
                                String::new(),
                                "ping_waiting",
                                "Проверка соединения...".into(),
                                [0.65, 0.65, 0.65, 1.],
                            ),
                        };
                    let (players_x, ping_x) =
                        header_positions(r.x + r.w, self.text_width(&players));
                    let available = (players_x - r.x - 41.).max(0.);
                    let mut name = crate::text::RichText::plain(&server.name).chars();
                    while !name.is_empty() && self.styled_width(&name) > available {
                        name.pop();
                    }
                    self.styled_line(&name, r.x + 36., r.y + 2., [1.; 4]);
                    self.text(&players, players_x, r.y + 2., [0.65, 0.65, 0.65, 1.]);
                    let ping_rect = Rect {
                        x: ping_x,
                        y: r.y + 1.,
                        w: 10.,
                        h: 8.,
                    };
                    self.image(ping, ping_rect);
                    if ping_rect.contains(self.mouse) {
                        let width = self.text_width(&tooltip).min(self.width - 20.);
                        let x = (self.mouse[0] - width - 4.).max(4.);
                        self.fill(
                            Rect {
                                x: x - 3.,
                                y: r.y - 16.,
                                w: width + 6.,
                                h: 14.,
                            },
                            [0.08, 0.02, 0.12, 0.96],
                        );
                        self.text(&tooltip, x, r.y - 13., [1.; 4]);
                    }
                    for (i, line) in self
                        .styled_lines(&motd, r.w - 40.)
                        .iter()
                        .take(2)
                        .enumerate()
                    {
                        self.styled_line(line, r.x + 36., r.y + 13. + i as f32 * 9., color);
                    }
                }
                if visible.len() > rows {
                    let track = (bottom - top).max(1.);
                    let thumb = (track * rows as f32 / visible.len() as f32)
                        .max(12.)
                        .min(track);
                    let y = top
                        + (track - thumb) * self.browser.scroll as f32
                            / (visible.len() - rows) as f32;
                    self.fill(
                        Rect {
                            x: c + 155.,
                            y: top,
                            w: 4.,
                            h: track,
                        },
                        [0., 0., 0., 0.7],
                    );
                    self.fill(
                        Rect {
                            x: c + 155.,
                            y,
                            w: 4.,
                            h: thumb,
                        },
                        [0.6, 0.6, 0.6, 1.],
                    );
                }
                let selected = self
                    .browser
                    .selected
                    .filter(|&i| i < self.browser.book.servers.len());
                let join = self.button(
                    "Подключиться",
                    rect(c - 154., h - 52., 100.),
                    selected.is_some(),
                );
                if join || double_join || self.connect_requested {
                    if let Some(i) = selected {
                        self.server = self.browser.book.servers[i].address.clone();
                        self.join_server(actions);
                    }
                }
                if self.button("По адресу", rect(c - 50., h - 52., 100.), true) {
                    self.screen = Screen::DirectConnect;
                    self.focus = true;
                    self.username_focus = false;
                    self.message.clear();
                }
                if self.button(
                    "Добавить",
                    rect(c + 54., h - 52., 100.),
                    self.browser.load_error.is_none(),
                ) {
                    self.server_form(None);
                }
                if self.button(
                    "Настроить",
                    rect(c - 154., h - 28., 74.),
                    selected.is_some(),
                ) {
                    self.server_form(selected);
                }
                let delete =
                    self.button("Удалить", rect(c - 76., h - 28., 74.), selected.is_some());
                if (delete || self.delete_requested) && selected.is_some() {
                    self.screen = Screen::ConfirmDelete;
                }
                if self.button("Обновить", rect(c + 2., h - 28., 74.), true) {
                    self.browser.refresh();
                }
                if self.button("Отмена", rect(c + 80., h - 28., 74.), true) {
                    self.screen = Screen::Main;
                }
                let message = self
                    .browser
                    .load_error
                    .clone()
                    .unwrap_or_else(|| self.message.clone());
                if !message.is_empty() {
                    self.centered(
                        &self
                            .wrapped(&message, self.width - 16.)
                            .first()
                            .cloned()
                            .unwrap_or_default(),
                        h - 65.,
                        [1., 0.6, 0.4, 1.],
                    );
                }
            }
            Screen::DirectConnect | Screen::ServerEdit => {
                let edit = self.screen == Screen::ServerEdit;
                let form_top = (h - 220.).clamp(40., 100.);
                self.centered(
                    if edit {
                        "Настройка сервера"
                    } else {
                        "Прямое подключение"
                    },
                    20.,
                    [1.; 4],
                );
                self.text(
                    if edit {
                        "Название сервера"
                    } else {
                        "Адрес сервера"
                    },
                    c - 100.,
                    form_top,
                    [0.65, 0.65, 0.65, 1.],
                );
                let first = if edit {
                    self.browser.name.clone()
                } else {
                    self.server.clone()
                };
                if self.input_field(
                    &first,
                    rect(c - 100., form_top + 16., 200.),
                    if edit {
                        self.name_focus
                    } else {
                        !self.username_focus
                    },
                ) {
                    self.name_focus = true;
                    self.username_focus = false;
                    self.focus = true;
                }
                self.text(
                    if edit {
                        "Адрес сервера"
                    } else {
                        "Имя игрока"
                    },
                    c - 100.,
                    form_top + 46.,
                    [0.65, 0.65, 0.65, 1.],
                );
                let second = if edit {
                    self.server.clone()
                } else {
                    self.username.clone()
                };
                if self.input_field(
                    &second,
                    rect(c - 100., form_top + 62., 200.),
                    if edit {
                        !self.name_focus
                    } else {
                        self.username_focus
                    },
                ) {
                    self.name_focus = false;
                    self.username_focus = true;
                    self.focus = true;
                }
                let valid = crate::net::Address::parse(&self.server).is_ok()
                    && if edit {
                        !self.browser.name.trim().is_empty()
                    } else {
                        multiplayer::valid_username(&self.username)
                    };
                let submit = self.button(
                    if edit {
                        "Готово"
                    } else {
                        "Подключиться"
                    },
                    rect(c - 100., h - 52., 200.),
                    valid,
                );
                if (submit || self.connect_requested) && valid {
                    if edit {
                        let result = Server::new(self.browser.name.clone(), self.server.clone())
                            .and_then(|mut server| {
                                let old = self.browser.book.clone();
                                if let Some(i) = self.browser.editing {
                                    server.extra = self.browser.book.servers[i].extra.clone();
                                    self.browser.book.servers[i] = server;
                                } else {
                                    self.browser.book.servers.push(server);
                                }
                                if let Err(e) = self.browser.save() {
                                    self.browser.book = old;
                                    return Err(e);
                                }
                                self.browser.selected = Some(
                                    self.browser
                                        .editing
                                        .unwrap_or(self.browser.book.servers.len() - 1),
                                );
                                Ok(())
                            });
                        match result {
                            Ok(()) => {
                                self.screen = Screen::Multiplayer;
                                self.browser.refresh();
                                self.message.clear();
                            }
                            Err(e) => self.message = e.to_string(),
                        }
                    } else {
                        self.join_server(actions);
                    }
                }
                if self.button("Отмена", rect(c - 100., h - 28., 200.), true) {
                    self.screen = Screen::Multiplayer;
                    self.focus = false;
                    self.message.clear();
                }
                let message = self.message.clone();
                for (i, line) in self
                    .wrapped(&message, self.width - 20.)
                    .iter()
                    .take(3)
                    .enumerate()
                {
                    self.centered(line, form_top + 90. + i as f32 * 10., [1., 0.5, 0.4, 1.]);
                }
            }
            Screen::ConfirmDelete => {
                self.centered(
                    "Вы уверены, что хотите удалить сервер?",
                    h * 0.5 - 40.,
                    [1.; 4],
                );
                let name = selected
                    .map(|i| self.browser.book.servers[i].name.clone())
                    .unwrap_or_default();
                self.centered(&name, h * 0.5 - 22., [1., 0.8, 0.4, 1.]);
                if self.button(
                    "Удалить",
                    rect(c - 102., h * 0.5 + 10., 100.),
                    selected.is_some(),
                ) {
                    if let Some(i) = selected {
                        let old = self.browser.book.clone();
                        self.browser.book.servers.remove(i);
                        if let Err(e) = self.browser.save() {
                            self.browser.book = old;
                            self.message = e.to_string();
                        } else {
                            self.browser.selected = None;
                        }
                    }
                    self.screen = Screen::Multiplayer;
                }
                if self.button("Отмена", rect(c + 2., h * 0.5 + 10., 100.), true) {
                    self.screen = Screen::Multiplayer;
                }
            }
            Screen::Disconnected => {
                self.centered(
                    "Не удалось подключиться к серверу",
                    h * 0.5 - 70.,
                    [0.65, 0.65, 0.65, 1.],
                );
                let reason = self
                    .rich_reason
                    .clone()
                    .unwrap_or_else(|| crate::text::RichText::plain(&self.message));
                let lines = self.styled_lines(&reason, self.width - 60.);
                let rows = ((h - 110.) / 10.).floor().max(1.) as usize;
                self.reason_scroll = self.reason_scroll.min(lines.len().saturating_sub(rows));
                let shown = lines.len().min(rows);
                let start = (h * 0.5 - 40.).min((h - 55. - shown as f32 * 10.).max(35.));
                for (i, line) in lines.iter().skip(self.reason_scroll).take(rows).enumerate() {
                    self.centered_styled_line(line, start + i as f32 * 10., [1.; 4]);
                }
                if self.button(
                    "Назад к списку серверов",
                    rect(c - 100., h - 35., 200.),
                    true,
                ) {
                    self.screen = Screen::Multiplayer;
                    self.reason_scroll = 0;
                    self.message.clear();
                }
            }
            _ => {}
        }
        self.connect_requested = false;
        self.delete_requested = false;
    }
}

fn wrap(text: &str, width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for c in paragraph.chars() {
            if c == '\r' {
                continue;
            }
            if line.is_empty() && c == ' ' {
                continue;
            }
            line.push(c);
            if measure(&line) > width && line.chars().count() > 1 {
                line.pop();
                if c == ' ' {
                    lines.push(std::mem::take(&mut line));
                    continue;
                }
                if let Some(space) = line.rfind(' ') {
                    let tail = line[space + 1..].to_owned();
                    lines.push(line[..space].to_owned());
                    line = tail;
                } else {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(c);
            }
        }
        lines.push(line);
    }
    lines
}
// Reserve the original 10-pixel ping sprite plus a gap independently of latency text.
fn header_positions(right: f32, players_width: f32) -> (f32, f32) {
    let ping_x = right - 15.;
    (ping_x - 5. - players_width, ping_x)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_count_never_overlaps_ping_even_for_long_counts() {
        for width in [0., 36., 60., 120., 180.] {
            let (players, ping) = header_positions(305., width);
            assert!(players + width + 5. <= ping);
            assert!(ping + 10. <= 305.);
        }
    }
    #[test]
    fn server_reason_wrap_keeps_unicode_paragraphs_and_long_words() {
        let lines = wrap(
            "Ошибка входа\n\nОченьдлинноеслово",
            6.,
            |s| s.chars().count() as f32,
        );
        assert_eq!(
            lines,
            vec!["Ошибка", "входа", "", "Оченьд", "линное", "слово"]
        );
        assert!(lines.iter().all(|s| s.chars().count() <= 6));
    }
}
