//! Player inventory projection and standalone demonstration interactions.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Item {
    Registry(u32),
    Grass,
    Dirt,
    Stone,
    Log,
    Sand,
    DiamondSword,
    IronPickaxe,
    Apple,
    Bread,
}
impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Self::Registry(id) => crate::net::item_defaults::get()
                .ok()
                .and_then(|items| items.get(id as usize))
                .map(|item| crate::text::localized(&item.translation).unwrap_or(&item.name))
                .unwrap_or("?"),
            Self::Grass => "Дёрн",
            Self::Dirt => "Земля",
            Self::Stone => "Камень",
            Self::Log => "Дубовое бревно",
            Self::Sand => "Песок",
            Self::DiamondSword => "Алмазный меч",
            Self::IronPickaxe => "Железная кирка",
            Self::Apple => "Яблоко",
            Self::Bread => "Хлеб",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Registry(_) => "",
            Self::Grass => "block_grass",
            Self::Dirt => "block_dirt",
            Self::Stone => "block_stone",
            Self::Log => "block_log",
            Self::Sand => "block_sand",
            Self::DiamondSword => "diamond_sword.png",
            Self::IronPickaxe => "iron_pickaxe.png",
            Self::Apple => "apple.png",
            Self::Bread => "bread.png",
        }
    }
    pub fn maximum(self) -> i32 {
        if let Self::Registry(id) = self {
            return crate::net::item_defaults::get()
                .ok()
                .and_then(|items| items.get(id as usize))
                .map_or(1, |item| item.maximum);
        }
        if matches!(self, Self::DiamondSword | Self::IronPickaxe) {
            1
        } else {
            64
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stack {
    pub item: Item,
    pub count: i32,
}
#[derive(Clone, Debug)]
pub struct Inventory {
    pub slots: [Option<Stack>; 46],
    pub cursor: Option<Stack>,
    pub selected: usize,
}
impl Default for Inventory {
    fn default() -> Self {
        let mut slots = [None; 46];
        for (index, (item, count)) in [
            (Item::Grass, 64),
            (Item::Dirt, 64),
            (Item::Stone, 64),
            (Item::Log, 32),
            (Item::DiamondSword, 1),
            (Item::IronPickaxe, 1),
            (Item::Apple, 16),
            (Item::Bread, 16),
            (Item::Sand, 64),
        ]
        .into_iter()
        .enumerate()
        {
            slots[index] = Some(Stack { item, count });
        }
        Self {
            slots,
            cursor: None,
            selected: 0,
        }
    }
}
impl Inventory {
    /// Read-only HUD projection; exact components stay in the authoritative store.
    pub fn apply_server(&mut self, store: &crate::net::container::Store) {
        self.slots = [None; 46];
        for (menu, stack) in store.player.iter().enumerate() {
            if let Some(index) = crate::net::container::local_slot(menu) {
                if stack.count > 0 {
                    self.slots[index] = Some(Stack {
                        item: Item::Registry(stack.item),
                        count: stack.count,
                    });
                }
            }
        }
        self.cursor = (store.carried.count > 0).then(|| Stack {
            item: Item::Registry(store.carried.item),
            count: store.carried.count,
        });
    }
    pub fn selected_stack(&self) -> Option<Stack> {
        self.slots[self.selected]
    }
    pub fn scroll(&mut self, steps: i32) {
        self.selected = (self.selected as i32 - steps).rem_euclid(9) as usize;
    }
    pub fn swap_offhand(&mut self) {
        self.slots.swap(self.selected, 40);
    }
    pub fn drop_selected(&mut self, whole: bool) -> Option<Stack> {
        let stack = self.slots[self.selected].as_mut()?;
        let dropped = Stack {
            item: stack.item,
            count: if whole { stack.count } else { 1 },
        };
        stack.count -= dropped.count;
        if stack.count == 0 {
            self.slots[self.selected] = None;
        }
        Some(dropped)
    }
    pub fn click(&mut self, index: usize, right: bool, shift: bool) {
        if index >= 45 || (36..40).contains(&index) {
            return;
        } // Armor/equipment validation awaits item definitions.
        if shift && self.cursor.is_none() {
            self.quick_move(index);
            return;
        }
        let slot = &mut self.slots[index];
        match (self.cursor, *slot) {
            (None, Some(stack)) => {
                let take = if right {
                    stack.count / 2 + stack.count % 2
                } else {
                    stack.count
                };
                self.cursor = Some(Stack {
                    count: take,
                    ..stack
                });
                *slot = if take == stack.count {
                    None
                } else {
                    Some(Stack {
                        count: stack.count - take,
                        ..stack
                    })
                };
            }
            (Some(mut hand), None) => {
                let place = if right { 1 } else { hand.count };
                *slot = Some(Stack {
                    count: place,
                    ..hand
                });
                hand.count -= place;
                self.cursor = (hand.count > 0).then_some(hand);
            }
            (Some(mut hand), Some(mut stack)) if hand.item == stack.item => {
                let room = stack.item.maximum().saturating_sub(stack.count);
                let amount = room.min(if right { 1 } else { hand.count });
                stack.count += amount;
                hand.count -= amount;
                *slot = Some(stack);
                self.cursor = (hand.count > 0).then_some(hand);
            }
            (Some(hand), Some(stack)) => {
                *slot = Some(hand);
                self.cursor = Some(stack);
            }
            _ => {}
        }
    }
    pub fn quick_move(&mut self, index: usize) {
        let Some(mut stack) = self.slots[index] else {
            return;
        };
        let target = if index < 9 { 9..36 } else { 0..9 };
        for i in target.clone() {
            if let Some(other) = &mut self.slots[i] {
                if other.item == stack.item {
                    let amount = stack
                        .item
                        .maximum()
                        .saturating_sub(other.count)
                        .min(stack.count);
                    other.count += amount;
                    stack.count -= amount;
                }
            }
        }
        for i in target {
            if self.slots[i].is_none() && stack.count > 0 {
                self.slots[i] = Some(stack);
                stack.count = 0;
                break;
            }
        }
        self.slots[index] = (stack.count > 0).then_some(stack);
    }
    pub fn close(&mut self) {
        let Some(mut hand) = self.cursor.take() else {
            return;
        };
        for stack in self.slots[..36].iter_mut().flatten() {
            if stack.item == hand.item {
                let amount = stack
                    .item
                    .maximum()
                    .saturating_sub(stack.count)
                    .min(hand.count);
                stack.count += amount;
                hand.count -= amount;
            }
        }
        for slot in &mut self.slots[..36] {
            if slot.is_none() && hand.count > 0 {
                *slot = Some(hand);
                hand.count = 0;
                break;
            }
        }
        self.cursor = (hand.count > 0).then_some(hand); // Never silently destroy an unplaceable cursor stack.
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn total(inv: &Inventory) -> u32 {
        inv.slots
            .iter()
            .flatten()
            .map(|s| s.count as u32)
            .sum::<u32>()
            + inv.cursor.map_or(0, |s| s.count as u32)
    }
    #[test]
    fn splitting_placing_merging_and_quick_move_conserve_items() {
        let mut inv = Inventory::default();
        let before = total(&inv);
        inv.click(0, true, false);
        assert_eq!(inv.cursor.unwrap().count, 32);
        inv.click(9, true, false);
        assert_eq!(inv.slots[9].unwrap().count, 1);
        inv.click(0, false, false);
        assert!(inv.cursor.is_none());
        assert_eq!(inv.slots[0].unwrap().count, 63);
        inv.click(9, false, true);
        assert_eq!(inv.slots[0].unwrap().count, 64);
        assert!(inv.slots[9].is_none());
        assert_eq!(total(&inv), before);
    }
    #[test]
    fn tools_cannot_stack_and_cursor_is_not_lost_on_close() {
        let mut inv = Inventory::default();
        inv.click(4, false, false);
        inv.click(10, false, false);
        inv.click(10, false, false);
        inv.close();
        assert!(inv.cursor.is_none());
        assert_eq!(inv.slots[4].unwrap().count, 1);
    }
    #[test]
    fn hotbar_wrap_offhand_and_drop() {
        let mut inv = Inventory::default();
        inv.scroll(1);
        assert_eq!(inv.selected, 8);
        inv.swap_offhand();
        assert!(inv.selected_stack().is_none());
        assert_eq!(inv.slots[40].unwrap().item, Item::Sand);
        inv.swap_offhand();
        assert_eq!(inv.drop_selected(false).unwrap().count, 1);
        assert_eq!(inv.selected_stack().unwrap().count, 63);
    }
}
