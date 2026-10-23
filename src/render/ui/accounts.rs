use super::*;
use crate::accounts::Profile;
fn rect(x: f32, y: f32, w: f32) -> Rect {
    Rect { x, y, w, h: 20. }
}
impl Ui {
    pub(super) fn account_form(&mut self, edit: Option<usize>) {
        self.account_edit = edit;
        self.account_submit = false;
        let profile = edit.and_then(|i| self.accounts.book.profiles.get(i));
        self.account_name = profile.map_or(String::new(), |p| p.username.clone());
        self.account_ely = profile.is_none_or(|p| p.ely_skins);
        self.focus = true;
        self.message.clear();
        self.screen = Screen::AccountEdit;
    }
    pub(super) fn accounts_screen(&mut self) {
        let c = (self.width * 0.5).floor();
        let h = self.height;
        let writable = self.accounts.load_error.is_none();
        match self.screen {
            Screen::Accounts => {
                self.centered("Аккаунты", 16., [1.; 4]);
                let profiles = self.accounts.book.profiles.clone();
                self.account_selected = self.account_selected.min(profiles.len() - 1);
                let rows = ((h - 123.) / 24.).floor().max(1.) as usize;
                self.account_scroll = self.account_scroll.min(profiles.len().saturating_sub(rows));
                for (row, (i, p)) in profiles
                    .iter()
                    .enumerate()
                    .skip(self.account_scroll)
                    .take(rows)
                    .enumerate()
                {
                    let label = format!(
                        "{}{}  [{}]",
                        if i == self.accounts.book.active {
                            "> "
                        } else {
                            ""
                        },
                        p.username,
                        if p.ely_skins { "Ely.by" } else { "Standard" }
                    );
                    if self.button(&label, rect(c - 150., 40. + row as f32 * 24., 300.), true) {
                        self.account_selected = i;
                    }
                    if i == self.account_selected {
                        self.text("*", c - 159., 46. + row as f32 * 24., [1., 1., 0.5, 1.]);
                    }
                }
                if self.button("Выбрать", rect(c - 150., h - 76., 98.), writable) {
                    let result = self.accounts.select(self.account_selected);
                    self.account_result(result);
                }
                if self.button(
                    "Создать",
                    rect(c - 49., h - 76., 98.),
                    writable && profiles.len() < 128,
                ) {
                    self.account_form(None);
                }
                if self.button("Изменить", rect(c + 52., h - 76., 98.), writable) {
                    self.account_form(Some(self.account_selected));
                }
                if self.button(
                    "Удалить",
                    rect(c - 150., h - 52., 98.),
                    writable && profiles.len() > 1,
                ) {
                    self.screen = Screen::AccountDelete;
                }
                if self.button(
                    "Обновить скин",
                    rect(c - 49., h - 52., 199.),
                    self.accounts.status != "Загрузка скина Ely.by...",
                ) {
                    self.accounts.reload();
                }
                if self.button("Готово", rect(c - 100., h - 28., 200.), true) {
                    self.screen = Screen::Main;
                    self.message.clear();
                }
                let status = self.accounts.load_error.clone().unwrap_or_else(|| {
                    if self.message.is_empty() {
                        self.accounts.status.clone()
                    } else {
                        self.message.clone()
                    }
                });
                let line = self
                    .wrapped(&status, self.width - 12.)
                    .first()
                    .cloned()
                    .unwrap_or_default();
                self.centered(&line, h - 91., [0.85, 0.85, 0.85, 1.]);
            }
            Screen::AccountEdit => {
                self.centered(
                    if self.account_edit.is_some() {
                        "Изменить аккаунт"
                    } else {
                        "Создать аккаунт"
                    },
                    20.,
                    [1.; 4],
                );
                self.text("Ник игрока", c - 100., 60., [0.7, 0.7, 0.7, 1.]);
                let name = self.account_name.clone();
                if self.input_field(&name, rect(c - 100., 76., 200.), self.focus) {
                    self.focus = true;
                }
                if self.button(
                    if self.account_ely {
                        "Скины Ely.by: Вкл"
                    } else {
                        "Скины Ely.by: Выкл"
                    },
                    rect(c - 100., 106., 200.),
                    true,
                ) {
                    self.account_ely = !self.account_ely;
                }
                self.centered(
                    "Скин загружается по нику, без входа",
                    136.,
                    [0.75, 0.75, 0.75, 1.],
                );
                let message = self.message.clone();
                let line = self
                    .wrapped(&message, self.width - 12.)
                    .first()
                    .cloned()
                    .unwrap_or_default();
                self.centered(&line, h - 60., [1., 0.6, 0.4, 1.]);
                if self.button(
                    "Сохранить",
                    rect(c - 100., h - 28., 98.),
                    crate::multiplayer::valid_username(&self.account_name),
                ) || std::mem::take(&mut self.account_submit)
                {
                    let result = self.accounts.edit(
                        self.account_edit,
                        Profile {
                            username: self.account_name.clone(),
                            ely_skins: self.account_ely,
                        },
                    );
                    if self.account_result(result) {
                        self.account_selected = self.accounts.book.active;
                        self.screen = Screen::Accounts;
                        self.focus = false;
                    }
                }
                if self.button("Отмена", rect(c + 2., h - 28., 98.), true) {
                    self.screen = Screen::Accounts;
                    self.focus = false;
                    self.message.clear();
                }
            }
            Screen::AccountDelete => {
                self.centered("Удалить локальный профиль?", h * 0.5 - 40., [1.; 4]);
                let name = self.accounts.book.profiles[self.account_selected]
                    .username
                    .clone();
                self.centered(&name, h * 0.5 - 20., [1.; 4]);
                let message = self.message.clone();
                let line = self
                    .wrapped(&message, self.width - 12.)
                    .first()
                    .cloned()
                    .unwrap_or_default();
                self.centered(&line, h * 0.5 + 44., [1., 0.6, 0.4, 1.]);
                if self.button("Удалить", rect(c - 100., h * 0.5 + 12., 98.), true) {
                    let result = self.accounts.delete(self.account_selected);
                    if self.account_result(result) {
                        self.screen = Screen::Accounts;
                        self.account_selected = self.accounts.book.active;
                    }
                }
                if self.button("Отмена", rect(c + 2., h * 0.5 + 12., 98.), true) {
                    self.screen = Screen::Accounts;
                }
            }
            _ => {}
        }
    }
    fn account_result(&mut self, result: Result<()>) -> bool {
        match result {
            Ok(()) => {
                self.username = self.accounts.active().username.clone();
                self.message.clear();
                true
            }
            Err(e) => {
                self.message = e.to_string();
                false
            }
        }
    }
}
