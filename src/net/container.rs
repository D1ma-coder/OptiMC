//! Server inventory/menu state. Decode completes before a mutation is committed.
mod prediction;
use super::{codec::read_varint, component, items::Stack};
use crate::text::RichText;
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, io::Read, sync::Arc};
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Layout {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub slots: Vec<[i32; 3]>,
}
pub fn layout(kind: u32) -> Option<&'static Layout> {
    static DATA: std::sync::OnceLock<Vec<Layout>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/inventory-reference.json.gz").as_slice(),
        )
        .take(4 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .expect("menu layout report");
        let r: serde_json::Value = serde_json::from_slice(&bytes).expect("menu layout JSON");
        serde_json::from_value(r["layouts"].clone()).expect("menu layouts")
    })
    .iter()
    .find(|layout| layout.id == kind)
}
const MAX_SLOTS: usize = 4096;
#[derive(Clone, Debug)]
pub struct Menu {
    pub id: i32,
    pub kind: u32,
    pub title: RichText,
    pub state: i32,
    pub slots: Vec<Arc<Stack>>,
    pub data: BTreeMap<i16, i16>,
}
#[derive(Clone, Debug)]
pub struct Store {
    pub player: Vec<Arc<Stack>>,
    pub player_state: i32,
    pub carried: Arc<Stack>,
    pub open: Option<Menu>,
    pub revision: u64,
    pub menu_serial: u64,
    player_carried: Arc<Stack>,
}
fn empty_slots(count: usize) -> Vec<Arc<Stack>> {
    let empty = Arc::new(Stack::default());
    vec![empty; count]
}
impl Default for Store {
    fn default() -> Self {
        Self {
            player: empty_slots(46),
            player_state: 0,
            carried: Default::default(),
            player_carried: Default::default(),
            open: None,
            revision: 0,
            menu_serial: 0,
        }
    }
}
fn short(input: &mut &[u8]) -> Result<i16> {
    let mut bytes = [0; 2];
    input.read_exact(&mut bytes)?;
    Ok(i16::from_be_bytes(bytes))
}
fn natural(input: &mut &[u8], max: usize) -> Result<usize> {
    let n = read_varint(input)?;
    ensure!(n >= 0 && n as usize <= max, "Invalid inventory list size");
    Ok(n as usize)
}
pub fn player_slot(index: i32) -> Option<usize> {
    match index {
        0..=8 => Some(index as usize + 36),
        9..=35 => Some(index as usize),
        36..=39 => Some((44 - index) as usize),
        40 => Some(45),
        _ => None,
    }
}
pub fn local_slot(menu: usize) -> Option<usize> {
    match menu {
        0 => Some(45),
        1..=4 => Some(menu + 40),
        5..=8 => Some(menu + 31),
        9..=35 => Some(menu),
        36..=44 => Some(menu - 36),
        45 => Some(40),
        _ => None,
    }
}
fn replace(old: &mut Arc<Stack>, item: Stack) {
    if **old != item {
        *old = Arc::new(item);
    }
}
impl Menu {
    fn player_index(&self, slot: usize) -> Option<usize> {
        if let Some(layout) = layout(self.kind).filter(|l| !l.slots.is_empty()) {
            return player_slot(layout.slots.get(slot)?[2]);
        }
        let offset = self.slots.len().checked_sub(36)?;
        let local = slot.checked_sub(offset)?;
        match local {
            0..=26 => Some(local + 9),
            27..=35 => Some(local - 27 + 36),
            _ => None,
        }
    }
}
impl Store {
    pub fn handles(id: i32) -> bool {
        matches!(id, 0x11..=0x14 | 0x3b | 0x60 | 0x6c)
    }
    pub fn close(&mut self) {
        self.open = None;
        self.carried = Default::default();
        self.player_carried = Default::default();
        self.menu_serial = self.menu_serial.wrapping_add(1);
        self.revision = self.revision.wrapping_add(1);
    }
    fn share_player(&mut self, index: usize) {
        if let Some(menu) = &mut self.open {
            for slot in 0..menu.slots.len() {
                if menu.player_index(slot) == Some(index) {
                    menu.slots[slot] = self.player[index].clone();
                }
            }
        }
    }
    pub fn packet(&mut self, id: i32, mut input: &[u8]) -> Result<bool> {
        enum Update {
            Close,
            Open(Menu),
            Content(i32, i32, Vec<Stack>, Stack),
            Slot(i32, i32, i16, Stack),
            Data(i32, i16, i16),
            Cursor(Stack),
            Direct(i32, Stack),
        }
        let update = match id {
            0x11 => {
                read_varint(&mut input)?;
                Update::Close
            }
            0x12 => {
                let menu = read_varint(&mut input)?;
                let state = read_varint(&mut input)?;
                let count = natural(&mut input, MAX_SLOTS)?;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(Stack::read(&mut input)?);
                }
                Update::Content(menu, state, items, Stack::read(&mut input)?)
            }
            0x14 => Update::Slot(
                read_varint(&mut input)?,
                read_varint(&mut input)?,
                short(&mut input)?,
                Stack::read(&mut input)?,
            ),
            0x13 => Update::Data(
                read_varint(&mut input)?,
                short(&mut input)?,
                short(&mut input)?,
            ),
            0x60 => Update::Cursor(Stack::read(&mut input)?),
            0x6c => Update::Direct(read_varint(&mut input)?, Stack::read(&mut input)?),
            0x3b => {
                let menu = read_varint(&mut input)?;
                let kind = read_varint(&mut input)?;
                ensure!(
                    menu > 0
                        && kind >= 0
                        && crate::assets::builtins::get()?
                            .name("minecraft:menu", kind as u32)
                            .is_some(),
                    "Invalid menu type/id"
                );
                let title = component::network_rich(input)?;
                input = &[];
                Update::Open(Menu {
                    id: menu,
                    kind: kind as u32,
                    title,
                    state: 0,
                    slots: empty_slots(layout(kind as u32).map_or(0, |l| l.slots.len())),
                    data: BTreeMap::new(),
                })
            }
            _ => return Ok(false),
        };
        ensure!(input.is_empty(), "Trailing inventory packet bytes");
        match update {
            Update::Close => {
                self.close();
                return Ok(true);
            }
            Update::Open(menu) => {
                self.open = Some(menu);
                self.carried = Default::default();
                self.menu_serial = self.menu_serial.wrapping_add(1);
            }
            Update::Content(0, state, items, carried) => {
                ensure!(items.len() <= 46, "Player inventory slot count overflow");
                let count = items.len();
                for (slot, item) in self.player.iter_mut().zip(items) {
                    replace(slot, item);
                }
                for index in 0..count {
                    self.share_player(index);
                }
                self.player_state = state;
                replace(&mut self.player_carried, carried);
                if self.open.is_none() {
                    self.carried = self.player_carried.clone();
                }
            }
            Update::Content(id, state, items, carried) => {
                if let Some(menu) = self.open.as_mut().filter(|m| m.id == id) {
                    if let Some(layout) = layout(menu.kind).filter(|l| !l.slots.is_empty()) {
                        ensure!(
                            items.len() <= layout.slots.len(),
                            "Menu slot count overflow"
                        );
                    }
                    menu.state = state;
                    menu.slots
                        .resize_with(items.len().max(menu.slots.len()), Default::default);
                    for (slot, item) in menu.slots.iter_mut().zip(items) {
                        replace(slot, item);
                    }
                    for slot in 0..menu.slots.len() {
                        if let Some(index) = menu.player_index(slot) {
                            self.player[index] = menu.slots[slot].clone();
                        }
                    }
                    replace(&mut self.carried, carried);
                } else {
                    return Ok(true);
                }
            }
            Update::Slot(id, state, slot, item) => {
                if slot < 0 {
                    return Ok(true);
                }
                if id == 0 {
                    if let Some(old) = self.player.get_mut(slot as usize) {
                        replace(old, item);
                        self.player_state = state;
                        self.share_player(slot as usize);
                    } else {
                        return Ok(true);
                    }
                } else if let Some(menu) = self.open.as_mut().filter(|m| m.id == id) {
                    if let Some(old) = menu.slots.get_mut(slot as usize) {
                        replace(old, item);
                        menu.state = state;
                        if let Some(index) = menu.player_index(slot as usize) {
                            self.player[index] = menu.slots[slot as usize].clone();
                        }
                    } else {
                        return Ok(true);
                    }
                } else {
                    return Ok(true);
                }
            }
            Update::Data(id, index, value) => {
                if let Some(menu) = self.open.as_mut().filter(|m| m.id == id) {
                    ensure!(
                        menu.data.len() < 1024 || menu.data.contains_key(&index),
                        "Container property budget exceeded"
                    );
                    menu.data.insert(index, value);
                } else {
                    return Ok(true);
                }
            }
            Update::Cursor(item) => {
                replace(&mut self.carried, item);
                if self.open.is_none() {
                    self.player_carried = self.carried.clone();
                }
            }
            Update::Direct(index, item) => {
                if let Some(index) = player_slot(index) {
                    replace(&mut self.player[index], item);
                    self.share_player(index);
                } else {
                    return Ok(true);
                }
            }
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }
    pub fn main_hand(&self, slot: u8) -> Option<&Stack> {
        self.player
            .get(36 + usize::from(slot))
            .map(Arc::as_ref)
            .filter(|s| s.count > 0)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    fn fixtures() -> Vec<(String, i32, Vec<u8>)> {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/inventory-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        r["packets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["label"].as_str().unwrap().into(),
                    r["id"].as_i64().unwrap() as i32,
                    base64::engine::general_purpose::STANDARD
                        .decode(r["body"].as_str().unwrap())
                        .unwrap(),
                )
            })
            .collect()
    }
    #[test]
    fn hud_projection_uses_server_counts_slots_cursor_and_retains_selection() {
        let mut store = Store::default();
        for (_, id, body) in fixtures().into_iter().take(4) {
            store.packet(id, &body).unwrap();
        }
        let mut hud = crate::inventory::Inventory {
            selected: 8,
            ..Default::default()
        };
        hud.apply_server(&store);
        assert_eq!(hud.selected, 8);
        for (menu, raw) in store.player.iter().enumerate() {
            let local = local_slot(menu).unwrap();
            assert_eq!(hud.slots[local].map(|s| s.count).unwrap_or(0), raw.count);
            if let Some(s) = hud.slots[local] {
                assert_eq!(s.item, crate::inventory::Item::Registry(raw.item));
            }
        }
        assert_eq!(
            hud.slots[0].unwrap().item,
            crate::inventory::Item::Registry(store.player[36].item)
        );
        assert_eq!(hud.slots[1].unwrap().count, 9);
        assert_eq!(hud.slots[8].unwrap().count, 3);
        assert_eq!(hud.slots[40].unwrap().count, 32);
        assert_eq!(hud.cursor.unwrap().count, 2);
        hud.apply_server(&Store::default());
        assert!(hud.slots.iter().all(Option::is_none));
        assert!(hud.cursor.is_none());
        assert_eq!(hud.selected, 8);
    }
    #[test]
    fn plugin_menu_components_do_not_discard_content_or_player_slots() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/menu-items.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let mut store = Store::default();
        for row in reference["packets"].as_array().unwrap() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            store
                .packet(row["id"].as_i64().unwrap() as i32, &body)
                .unwrap();
        }
        let menu = store.open.as_ref().unwrap();
        assert_eq!(menu.slots.iter().filter(|s| s.count > 0).count(), 9);
        assert_eq!(menu.state, 7);
        assert_eq!(store.player[36].count, 1);
        for (index, row) in reference["stacks"].as_array().unwrap().iter().enumerate() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            let mut input = body.as_slice();
            let stack = super::super::items::Stack::read(&mut input).unwrap();
            assert!(input.is_empty(), "{}", row["label"]);
            assert_eq!(*menu.slots[index], stack);
            for length in 0..body.len() {
                assert!(super::super::items::Stack::read(&mut &body[..length]).is_err());
            }
        }
    }
    #[test]
    fn unknown_component_never_publishes_partial_inventory() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/inventory-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let mut store = Store::default();
        for (_, id, body) in fixtures().into_iter().take(9) {
            store.packet(id, &body).unwrap();
        }
        let before = store.clone();
        for row in r["unsupported"].as_array().unwrap() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            let err = store
                .packet(row["id"].as_i64().unwrap() as i32, &body)
                .unwrap_err();
            assert!(err
                .downcast_ref::<super::super::items::UnsupportedComponent>()
                .is_some());
            assert_eq!(store.revision, before.revision);
            assert_eq!(store.carried, before.carried);
            assert_eq!(store.player, before.player);
            assert_eq!(
                store.open.as_ref().unwrap().slots,
                before.open.as_ref().unwrap().slots
            );
        }
    }
    #[test]
    fn original_inventory_packets_share_player_slots_and_preserve_tool_components() {
        let mut store = Store::default();
        for (label, id, bytes) in fixtures() {
            store
                .packet(id, &bytes)
                .unwrap_or_else(|e| panic!("{label}: {e:#}"));
            match label.as_str() {
                "player_content" => {
                    let tool = &store.player[36];
                    assert_eq!(tool.count, 1);
                    assert_eq!(tool.display_name().plain_text(), "Серверная кирка");
                    assert_eq!(tool.tool().unwrap().rules[0].speed, Some(12.5));
                    assert_eq!(tool.maximum(), 1);
                    assert_eq!(store.player[37].count, 12);
                    assert_eq!(store.player[45].count, 32);
                }
                "player_slot" => {
                    assert_eq!(store.player[37].count, 9);
                    assert_eq!(store.player_state, 6);
                }
                "direct_player_slot" => assert_eq!(store.player[44].count, 3),
                "cursor" => assert_eq!(store.carried.count, 2),
                "open_chest" => {
                    assert_eq!(store.open.as_ref().unwrap().id, 300);
                    assert_eq!(store.open.as_ref().unwrap().slots.len(), 63);
                    assert_eq!(store.carried.count, 0);
                }
                "chest_content" => {
                    let menu = store.open.as_ref().unwrap();
                    assert_eq!(menu.slots[0].count, 5);
                    assert!(Arc::ptr_eq(&menu.slots[54], &store.player[36]));
                    assert_eq!(store.player[37].count, 0);
                }
                "chest_slot" | "stale_slot" => {
                    assert_eq!(store.open.as_ref().unwrap().slots[0].count, 6)
                }
                "data" => assert_eq!(store.open.as_ref().unwrap().data[&2], 42),
                "close" => assert!(store.open.is_none()),
                _ => panic!("Unknown fixture"),
            }
        }
    }
    #[test]
    fn truncated_packets_never_publish_partial_inventory_and_slot_orders_match() {
        let mut state = Store::default();
        for (label, id, body) in fixtures() {
            for n in 0..body.len() {
                let mut broken = state.clone();
                let old = broken.player.clone();
                let revision = broken.revision;
                assert!(broken.packet(id, &body[..n]).is_err(), "{label} prefix {n}");
                assert_eq!(broken.revision, revision);
                assert!(old
                    .iter()
                    .zip(&broken.player)
                    .all(|(a, b)| Arc::ptr_eq(a, b)));
            }
            state.packet(id, &body).unwrap();
        }
        assert_eq!(player_slot(0), Some(36));
        assert_eq!(player_slot(39), Some(5));
        assert_eq!(player_slot(36), Some(8));
        assert_eq!(player_slot(40), Some(45));
        assert_eq!(player_slot(41), None);
        let mut slots = std::collections::BTreeSet::new();
        for i in 0..46 {
            assert!(slots.insert(local_slot(i).unwrap()));
        }
        assert_eq!(slots.len(), 46);
    }
    #[test]
    fn vanilla_defaults_and_server_component_removals_are_resolved_in_order() {
        let defaults = super::super::item_defaults::get().unwrap();
        assert_eq!(defaults.len(), 1537);
        let item = defaults
            .iter()
            .find(|i| i.name == "minecraft:iron_pickaxe")
            .unwrap();
        let mut stack = Stack {
            count: 1,
            item: item.id,
            ..Default::default()
        };
        assert_eq!(stack.maximum(), 1);
        assert!(stack
            .tool()
            .unwrap()
            .rules
            .iter()
            .any(|r| r.speed == Some(6.)));
        stack
            .removed
            .push(super::super::items::component_id("minecraft:tool").unwrap());
        assert!(stack.tool().is_none());
    }
}
