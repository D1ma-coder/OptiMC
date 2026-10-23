//! Inventory gestures are collected before drawing, preserving press/release order.
use super::*;
use crate::net::{items::Stack as ServerStack, menu_click::Click};
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
#[derive(Clone)]
enum Event {
    Down(u8, [f32; 2], bool, Instant),
    Up(u8, [f32; 2]),
    Move([f32; 2]),
    Key(KeyCode, [f32; 2], bool),
}
#[derive(Default)]
pub(super) struct Input {
    events: VecDeque<Event>,
    gesture: Option<Gesture>,
    last_click: Option<(i16, u8, Instant)>,
    last_moved: Option<(Arc<ServerStack>, u8)>,
}
struct Gesture {
    button: u8,
    slot: i16,
    drag: bool,
    double: bool,
    shift: bool,
    visited: Vec<i16>,
}
struct Slot {
    id: i16,
    rect: Rect,
    stack: Arc<ServerStack>,
    group: u8,
}
struct View {
    body: Rect,
    slots: Vec<Slot>,
    carried: Arc<ServerStack>,
    creative: bool,
}
impl View {
    fn hit(&self, p: [f32; 2]) -> i16 {
        self.slots
            .iter()
            .find(|s| s.rect.contains(p))
            .map_or(if self.body.contains(p) { -1 } else { -999 }, |s| s.id)
    }
    fn slot(&self, id: i16) -> Option<&Slot> {
        self.slots.iter().find(|s| s.id == id)
    }
}
fn same(a: &ServerStack, b: &ServerStack) -> bool {
    a.item == b.item && a.components == b.components && a.removed == b.removed
}
impl Input {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    fn push(&mut self, event: Event) {
        if self.events.len() >= 512 {
            self.clear();
            return;
        }
        self.events.push_back(event);
    }
    fn process(&mut self, event: Event, view: &View, out: &mut Vec<Click>) {
        match event {
            Event::Down(button, p, shift, now) => {
                self.gesture = None;
                let slot = view.hit(p);
                if slot == -1 {
                    return;
                }
                let double = button == 0
                    && self.last_click.is_some_and(|(s, b, at)| {
                        s == slot
                            && b == button
                            && now.saturating_duration_since(at) < Duration::from_millis(250)
                    });
                self.last_click = Some((slot, button, now));
                if shift && slot >= 0 {
                    if double {
                        if let Some((item, group)) = &self.last_moved {
                            for s in &view.slots {
                                if s.group == *group && s.stack.count > 0 && same(&s.stack, item) {
                                    out.push(Click::new(s.id, button.min(1), 1));
                                }
                            }
                        }
                    } else {
                        if let Some(s) = view.slot(slot).filter(|s| s.stack.count > 0) {
                            self.last_moved = Some((s.stack.clone(), s.group));
                        }
                        out.push(Click::new(slot, button.min(1), 1));
                    }
                    return;
                }
                if button == 2 && !view.creative {
                    return;
                }
                if slot == -999 {
                    if view.carried.count > 0 && button <= 1 {
                        out.push(Click::new(slot, button, 0));
                    }
                    return;
                }
                if double {
                    self.gesture = Some(Gesture {
                        button,
                        slot,
                        drag: false,
                        double: true,
                        shift,
                        visited: Vec::new(),
                    });
                } else if view.carried.count > 0 {
                    self.gesture = Some(Gesture {
                        button,
                        slot,
                        drag: true,
                        double: false,
                        shift,
                        visited: Vec::new(),
                    });
                } else {
                    out.push(Click::new(slot, button, if button == 2 { 3 } else { 0 }));
                }
            }
            Event::Move(p) => {
                let slot = view.hit(p);
                if let Some(g) = self.gesture.as_mut().filter(|g| g.drag) {
                    if let Some(s) = view.slot(slot) {
                        let available = s.stack.count == 0
                            || (same(&s.stack, &view.carried) && s.stack.count < s.stack.maximum());
                        if available
                            && g.visited.len() < 128
                            && !g.visited.contains(&slot)
                            && (g.button == 2
                                || g.visited.len() < view.carried.count.max(0) as usize)
                        {
                            g.visited.push(slot);
                        }
                    }
                }
            }
            Event::Up(button, p) => {
                let Some(g) = self.gesture.take() else {
                    return;
                };
                if g.button != button {
                    return;
                }
                if g.double && !g.shift {
                    out.push(Click::new(g.slot, 0, 6));
                } else if g.drag && !g.visited.is_empty() {
                    let kind = match button {
                        0 => 0,
                        1 => 1,
                        _ => 2,
                    };
                    out.push(Click::new(-999, kind << 2, 5));
                    for slot in g.visited {
                        out.push(Click::new(slot, 1 | (kind << 2), 5));
                    }
                    out.push(Click::new(-999, 2 | (kind << 2), 5));
                } else {
                    let slot = view.hit(p);
                    if slot >= 0 || slot == -999 {
                        out.push(Click::new(slot, button, if button == 2 { 3 } else { 0 }));
                    }
                }
            }
            Event::Key(key, p, control) => {
                let slot = view.hit(p);
                if slot < 0 {
                    return;
                }
                let swap = match key {
                    KeyCode::Digit1 => Some(0),
                    KeyCode::Digit2 => Some(1),
                    KeyCode::Digit3 => Some(2),
                    KeyCode::Digit4 => Some(3),
                    KeyCode::Digit5 => Some(4),
                    KeyCode::Digit6 => Some(5),
                    KeyCode::Digit7 => Some(6),
                    KeyCode::Digit8 => Some(7),
                    KeyCode::Digit9 => Some(8),
                    KeyCode::KeyF => Some(40),
                    _ => None,
                };
                if let Some(button) = swap {
                    out.push(Click::new(slot, button, 2));
                } else if key == KeyCode::KeyQ {
                    out.push(Click::new(slot, u8::from(control), 4));
                }
            }
        }
    }
}
impl Ui {
    pub(super) fn menu_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::KeyboardInput { event, .. } = event {
            if matches!(
                event.physical_key,
                PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight)
            ) {
                self.control = event.state.is_pressed();
            }
        }
        if matches!(event, WindowEvent::Focused(false)) {
            self.control = false;
            self.menu_input.clear();
            return;
        }
        if self.server_inventory.is_none()
            || !matches!(self.screen, Screen::Inventory | Screen::Container)
        {
            return;
        }
        let input = match event {
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    _ => return,
                };
                if state.is_pressed() {
                    Event::Down(button, self.pointer, self.shift, Instant::now())
                } else {
                    Event::Up(button, self.pointer)
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                Event::Move([position.x as f32, position.y as f32])
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed() && !event.repeat =>
            {
                let PhysicalKey::Code(key) = event.physical_key else {
                    return;
                };
                Event::Key(key, self.pointer, self.control)
            }
            _ => return,
        };
        self.menu_input.push(input);
    }
    pub(super) fn process_menu_input(
        &mut self,
        gui: super::super::gui_scale::GuiScale,
        size: [u32; 2],
        actions: &mut Vec<Action>,
    ) {
        self.menu_preview = None;
        if !matches!(self.screen, Screen::Inventory | Screen::Container) {
            self.menu_input.clear();
            return;
        }
        let Some(store) = self.server_inventory.clone() else {
            self.menu_input.clear();
            return;
        };
        let mut slots = Vec::new();
        let (id, state, body) = if self.screen == Screen::Inventory {
            let x = ((self.width - 176.) * 0.5).floor();
            let y = ((self.height - 166.) * 0.5).floor();
            for raw in 0..46 {
                let local = crate::net::container::local_slot(raw).unwrap();
                let (sx, sy) = match local {
                    0..=8 => (8. + local as f32 * 18., 142.),
                    9..=35 => (
                        8. + ((local - 9) % 9) as f32 * 18.,
                        84. + ((local - 9) / 9) as f32 * 18.,
                    ),
                    36..=39 => (8., 8. + (local - 36) as f32 * 18.),
                    40 => (77., 62.),
                    41..=44 => (
                        98. + ((local - 41) % 2) as f32 * 18.,
                        18. + ((local - 41) / 2) as f32 * 18.,
                    ),
                    _ => (154., 28.),
                };
                slots.push(Slot {
                    id: raw as i16,
                    rect: Rect {
                        x: x + sx - 1.,
                        y: y + sy - 1.,
                        w: 18.,
                        h: 18.,
                    },
                    stack: store.player[raw].clone(),
                    group: if (9..45).contains(&raw) { 1 } else { 0 },
                });
            }
            (
                0,
                store.player_state,
                Rect {
                    x,
                    y,
                    w: 176.,
                    h: 166.,
                },
            )
        } else {
            let Some(menu) = &store.open else {
                return;
            };
            let Some(layout) = crate::net::container::layout(menu.kind) else {
                return;
            };
            let short = layout.name.trim_start_matches("minecraft:");
            let rows = short
                .strip_prefix("generic_9x")
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|r| (1..=6).contains(r));
            if rows.is_none()
                && !matches!(
                    short,
                    "hopper"
                        | "generic_3x3"
                        | "shulker_box"
                        | "crafting"
                        | "anvil"
                        | "grindstone"
                        | "cartography_table"
                        | "stonecutter"
                        | "crafter_3x3"
                )
            {
                self.menu_input.clear();
                return;
            }
            let height = rows.map_or(if short == "hopper" { 133. } else { 166. }, |r| {
                114. + r as f32 * 18.
            });
            let x = ((self.width - 176.) * 0.5).floor();
            let y = ((self.height - height) * 0.5).floor();
            for (i, s) in layout.slots.iter().enumerate() {
                if let Some(stack) = menu.slots.get(i) {
                    slots.push(Slot {
                        id: i as i16,
                        rect: Rect {
                            x: x + s[0] as f32 - 1.,
                            y: y + s[1] as f32 - 1.,
                            w: 18.,
                            h: 18.,
                        },
                        stack: stack.clone(),
                        group: u8::from(s[2] >= 0),
                    });
                }
            }
            (
                menu.id,
                menu.state,
                Rect {
                    x,
                    y,
                    w: 176.,
                    h: height,
                },
            )
        };
        let mut view = View {
            body,
            slots,
            carried: store.carried.clone(),
            creative: self.creative_inventory,
        };
        let mut clicks = Vec::new();
        while let Some(mut event) = self.menu_input.events.pop_front() {
            let p = match &mut event {
                Event::Down(_, p, ..) | Event::Up(_, p) | Event::Move(p) | Event::Key(_, p, _) => p,
            };
            *p = gui.pointer(*p, size);
            let before = clicks.len();
            self.menu_input.process(event, &view, &mut clicks);
            if clicks.len() != before {
                let predicted = store.predict(&clicks, self.creative_inventory);
                view.carried = predicted.carried.clone();
                let source = predicted
                    .open
                    .as_ref()
                    .map_or(&predicted.player, |m| &m.slots);
                for slot in &mut view.slots {
                    if let Some(stack) = source.get(slot.id as usize) {
                        slot.stack = stack.clone();
                    }
                }
            }
        }
        if !clicks.is_empty() {
            let predicted = Arc::new(store.predict(&clicks, self.creative_inventory));
            self.inventory.apply_server(&predicted);
            self.server_inventory = Some(predicted);
            self.menu_prediction = Some((store.revision, Instant::now()));
        }
        if let Some(g) = self
            .menu_input
            .gesture
            .as_ref()
            .filter(|g| g.drag && !g.visited.is_empty())
        {
            let mode = match g.button {
                0 => 0,
                1 => 1,
                _ => 2,
            };
            let mut preview = vec![Click::new(-999, mode << 2, 5)];
            preview.extend(g.visited.iter().map(|&s| Click::new(s, 1 | (mode << 2), 5)));
            preview.push(Click::new(-999, 2 | (mode << 2), 5));
            let source = self.server_inventory.as_ref().unwrap();
            self.menu_preview = Some(Arc::new(source.predict(&preview, self.creative_inventory)));
        }
        for click in clicks {
            actions.push(Action::MenuClick {
                id,
                state,
                serial: store.menu_serial,
                click,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(count: i32) -> View {
        let carried = Arc::new(ServerStack {
            item: 1,
            count,
            ..Default::default()
        });
        View {
            body: Rect {
                x: 0.,
                y: 0.,
                w: 100.,
                h: 100.,
            },
            slots: (0..3)
                .map(|i| Slot {
                    id: i,
                    rect: Rect {
                        x: i as f32 * 20.,
                        y: 0.,
                        w: 18.,
                        h: 18.,
                    },
                    stack: Default::default(),
                    group: 0,
                })
                .collect(),
            carried,
            creative: false,
        }
    }
    #[test]
    fn left_and_right_drag_send_original_quickcraft_stages_once_per_slot() {
        for button in [0, 1] {
            let view = view(12);
            let mut input = Input::default();
            let mut out = Vec::new();
            input.process(
                Event::Down(button, [5., 5.], false, Instant::now()),
                &view,
                &mut out,
            );
            for x in [5., 5., 25., 45., 25.] {
                input.process(Event::Move([x, 5.]), &view, &mut out);
            }
            assert!(out.is_empty());
            input.process(Event::Up(button, [45., 5.]), &view, &mut out);
            let mode = button << 2;
            assert_eq!(
                out,
                vec![
                    Click::new(-999, mode, 5),
                    Click::new(0, mode | 1, 5),
                    Click::new(1, mode | 1, 5),
                    Click::new(2, mode | 1, 5),
                    Click::new(-999, mode | 2, 5)
                ]
            );
            for c in &out {
                c.validate().unwrap();
            }
            input.process(Event::Up(button, [45., 5.]), &view, &mut out);
            assert_eq!(out.len(), 5);
        }
    }
    #[test]
    fn pickup_right_split_shift_swap_and_throw_are_distinct_inputs() {
        let view = view(0);
        let mut input = Input::default();
        let mut out = Vec::new();
        input.process(
            Event::Down(1, [5., 5.], false, Instant::now()),
            &view,
            &mut out,
        );
        input.process(Event::Up(1, [5., 5.]), &view, &mut out);
        input.process(
            Event::Down(0, [25., 5.], true, Instant::now()),
            &view,
            &mut out,
        );
        input.process(
            Event::Key(KeyCode::Digit9, [45., 5.], false),
            &view,
            &mut out,
        );
        input.process(Event::Key(KeyCode::KeyF, [45., 5.], false), &view, &mut out);
        input.process(Event::Key(KeyCode::KeyQ, [45., 5.], true), &view, &mut out);
        assert_eq!(
            out,
            vec![
                Click::new(0, 1, 0),
                Click::new(1, 0, 1),
                Click::new(2, 8, 2),
                Click::new(2, 40, 2),
                Click::new(2, 1, 4)
            ]
        );
    }
    #[test]
    fn double_click_collects_and_focus_cancel_does_not_finish_an_old_drag() {
        let view = view(12);
        let mut input = Input::default();
        let mut out = Vec::new();
        let now = Instant::now();
        input.process(Event::Down(0, [5., 5.], false, now), &view, &mut out);
        input.process(Event::Up(0, [5., 5.]), &view, &mut out);
        input.process(
            Event::Down(0, [5., 5.], false, now + Duration::from_millis(100)),
            &view,
            &mut out,
        );
        input.process(Event::Up(0, [5., 5.]), &view, &mut out);
        assert_eq!(out, vec![Click::new(0, 0, 0), Click::new(0, 0, 6)]);
        input.process(Event::Down(1, [5., 5.], false, now), &view, &mut out);
        input.process(Event::Move([25., 5.]), &view, &mut out);
        input.clear();
        input.process(Event::Up(1, [25., 5.]), &view, &mut out);
        assert_eq!(out.len(), 2);
    }
}
