use super::*;
use std::sync::Arc;
impl Ui {
    pub fn sync_inventory(&mut self, store: Arc<crate::net::container::Store>) {
        if self.menu_prediction.is_some_and(|(revision, at)| {
            revision == store.revision && at.elapsed() < std::time::Duration::from_secs(1)
        }) && self
            .server_inventory
            .as_ref()
            .is_some_and(|old| old.menu_serial == store.menu_serial)
        {
            return;
        }
        self.menu_prediction = None;
        if self
            .server_inventory
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &store))
        {
            return;
        }
        let old_serial = self.server_inventory.as_ref().map(|old| old.menu_serial);
        if old_serial != Some(store.menu_serial) {
            self.menu_input.clear();
            if store.open.is_some() {
                self.screen = Screen::Container;
            } else if self.screen == Screen::Container {
                self.screen = Screen::Playing;
            }
        }
        self.inventory.apply_server(&store);
        self.server_inventory = Some(store);
    }
    pub(super) fn request_menu_close(&mut self) {
        if let Some(store) = &self.server_inventory {
            self.close_menu = Some((
                store.open.as_ref().map_or(0, |menu| menu.id),
                store.menu_serial,
            ));
        }
    }
    pub(super) fn server_stack_name(&self, local: usize) -> Option<crate::text::RichText> {
        let store = self.server_inventory.as_ref()?;
        let menu = (0..46).find(|&i| crate::net::container::local_slot(i) == Some(local))?;
        let stack = store.player.get(menu)?;
        Some(stack.display_name())
    }
    fn image_region(&mut self, name: &str, rect: Rect, source: [f32; 4]) {
        let sprite = self.sprites[name];
        let [x, y, w, h] = source;
        self.quad(
            rect,
            [
                sprite.uv[0] + x / sprite.size[0] * (sprite.uv[2] - sprite.uv[0]),
                sprite.uv[1] + y / sprite.size[1] * (sprite.uv[3] - sprite.uv[1]),
                sprite.uv[0] + (x + w) / sprite.size[0] * (sprite.uv[2] - sprite.uv[0]),
                sprite.uv[1] + (y + h) / sprite.size[1] * (sprite.uv[3] - sprite.uv[1]),
            ],
            [1.; 4],
        );
    }
    pub(super) fn item_tooltip(&mut self, name: &crate::text::RichText, x: f32, y: f32) {
        let lines = self.styled_lines(name, self.width.min(240.) - 20.);
        let width = lines
            .iter()
            .map(|l| self.styled_width(l))
            .fold(0., f32::max);
        let x = x.min(self.width - width - 6.).max(4.);
        let y = y.min(self.height - lines.len() as f32 * 10. - 6.).max(4.);
        self.fill(
            Rect {
                x: x - 3.,
                y: y - 3.,
                w: width + 6.,
                h: lines.len() as f32 * 10. + 4.,
            },
            [0.08, 0.02, 0.12, 0.96],
        );
        for (i, line) in lines.iter().enumerate() {
            self.styled_line(line, x, y + i as f32 * 10., [1.; 4]);
        }
    }
    pub(super) fn container_screen(&mut self) {
        let Some(store) = self
            .menu_preview
            .clone()
            .or_else(|| self.server_inventory.clone())
        else {
            return;
        };
        let Some(menu) = &store.open else {
            return;
        };
        let name = crate::assets::builtins::get()
            .ok()
            .and_then(|ids| ids.name("minecraft:menu", menu.kind))
            .unwrap_or("");
        let short = name.strip_prefix("minecraft:").unwrap_or(name);
        let rows = short
            .strip_prefix("generic_9x")
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|r| (1..=6).contains(r));
        let background = if rows.is_some() {
            Some("container_generic")
        } else {
            match short {
                "hopper" => Some("container_hopper"),
                "generic_3x3" => Some("container_generic_3x3"),
                "shulker_box" => Some("container_shulker"),
                "crafting" => Some("container_crafting"),
                "anvil" => Some("container_anvil"),
                "grindstone" => Some("container_grindstone"),
                "cartography_table" => Some("container_cartography_table"),
                "stonecutter" => Some("container_stonecutter"),
                "crafter_3x3" => Some("container_crafter_3x3"),
                _ => None,
            }
        };
        let height = rows.map_or(if short == "hopper" { 133. } else { 166. }, |r| {
            114. + r as f32 * 18.
        });
        let x = ((self.width - 176.) * 0.5).floor();
        let y = ((self.height - height) * 0.5).floor();
        if let Some(background) = background {
            if let Some(rows) = rows {
                let top = rows as f32 * 18. + 17.;
                self.image_region(
                    background,
                    Rect {
                        x,
                        y,
                        w: 176.,
                        h: top,
                    },
                    [0., 0., 176., top],
                );
                self.image_region(
                    background,
                    Rect {
                        x,
                        y: y + top,
                        w: 176.,
                        h: 96.,
                    },
                    [0., 126., 176., 96.],
                );
            } else {
                self.image(
                    background,
                    Rect {
                        x,
                        y,
                        w: 176.,
                        h: height,
                    },
                );
            }
            if let Some(line) = self.styled_lines(&menu.title, 160.).first() {
                self.styled_line(line, x + 8., y + 6., [0.25, 0.25, 0.25, 1.]);
            }
            let Some(layout) = crate::net::container::layout(menu.kind) else {
                return;
            };
            let mut hover = None;
            for (index, slot) in layout.slots.iter().enumerate() {
                let rect = Rect {
                    x: x + slot[0] as f32,
                    y: y + slot[1] as f32,
                    w: 16.,
                    h: 16.,
                };
                if let Some(stack) = menu.slots.get(index).filter(|s| s.count > 0) {
                    self.server_item(stack, rect.x, rect.y);
                }
                if rect.contains(self.mouse) {
                    self.fill(rect, [1., 1., 1., 0.25]);
                    hover = Some(index);
                }
            }
            if store.carried.count > 0 {
                self.server_item(&store.carried, self.mouse[0] - 8., self.mouse[1] - 8.);
            } else if let Some(stack) = hover
                .and_then(|i| menu.slots.get(i))
                .filter(|s| s.count > 0)
            {
                self.item_tooltip(
                    &stack.display_name(),
                    self.mouse[0] + 10.,
                    self.mouse[1] - 16.,
                );
            }
        } else {
            self.centered(
                "Это меню пока не поддерживается",
                self.height * 0.5 - 10.,
                [1.; 4],
            );
            if self.button(
                "Закрыть",
                Rect {
                    x: self.width * 0.5 - 100.,
                    y: self.height * 0.5 + 14.,
                    w: 200.,
                    h: 20.,
                },
                true,
            ) {
                self.request_menu_close();
                self.screen = Screen::Playing;
            }
        }
    }
}
