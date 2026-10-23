//! Visual prediction for ordinary storage menus; incoming server state wins.
use super::*;
use crate::net::menu_click::Click;
fn same(a: &Stack, b: &Stack) -> bool {
    a.item == b.item && a.components == b.components && a.removed == b.removed
}
fn counted(mut stack: Stack, count: i32) -> Stack {
    if count <= 0 {
        Stack::default()
    } else {
        stack.count = count;
        stack
    }
}
impl Store {
    fn current(&self) -> &[Arc<Stack>] {
        self.open.as_ref().map_or(&self.player, |m| &m.slots)
    }
    fn put(&mut self, index: usize, item: Stack) {
        let player = self.open.as_ref().and_then(|m| m.player_index(index));
        let item = Arc::new(item);
        if let Some(menu) = &mut self.open {
            menu.slots[index] = item.clone();
            if let Some(i) = player {
                self.player[i] = item;
            }
        } else {
            self.player[index] = item;
        }
    }
    fn cursor(&mut self, stack: Stack) {
        self.carried = Arc::new(stack);
        if self.open.is_none() {
            self.player_carried = self.carried.clone();
        }
    }
    fn limit(&self, index: usize, item: &Stack) -> i32 {
        if self.open.is_none() && (5..9).contains(&index) {
            1
        } else {
            item.maximum().clamp(1, 64)
        }
    }
    fn placeable(&self, index: usize, item: &Stack) -> bool {
        if self.open.is_some() || index >= 9 {
            return true;
        }
        if index == 0 {
            return false;
        }
        if index <= 4 {
            return true;
        }
        // Do not predict equipment restrictions/curse bindings without slot rules.
        let _ = item;
        false
    }
    fn move_to(&mut self, mut stack: Stack, targets: &[usize]) -> Stack {
        for empty in [false, true] {
            for &i in targets {
                if stack.count == 0 {
                    return stack;
                }
                let target = (**self.current().get(i).unwrap()).clone();
                if !self.placeable(i, &stack) || (target.count == 0) != empty {
                    continue;
                }
                if target.count > 0 && !same(&target, &stack) {
                    continue;
                }
                let amount = (self.limit(i, &stack) - target.count)
                    .max(0)
                    .min(stack.count);
                if amount > 0 {
                    let count = target.count + amount;
                    self.put(i, counted(stack.clone(), count));
                    stack = counted(stack.clone(), stack.count - amount);
                }
            }
        }
        stack
    }
    /// Unsupported specialized slot logic is left to the authoritative server.
    pub fn predict(&self, clicks: &[Click], creative: bool) -> Self {
        let mut out = self.clone();
        if let Some(menu) = &out.open {
            let supported = layout(menu.kind).is_some_and(|l| {
                let name = l.name.trim_start_matches("minecraft:");
                name.starts_with("generic_9x")
                    || matches!(name, "generic_3x3" | "hopper" | "shulker_box")
            });
            if !supported {
                return out;
            }
        }
        let mut drag = Vec::new();
        let mut mode = 0;
        for &click in clicks {
            let i = click.slot as usize;
            if click.kind == 5 {
                match click.button & 3 {
                    0 => {
                        drag.clear();
                        mode = click.button >> 2;
                    }
                    1 => {
                        if i < out.current().len() && !drag.contains(&i) {
                            drag.push(i);
                        }
                    }
                    2 => {
                        let carried = (*out.carried).clone();
                        let mut left = carried.count;
                        drag.retain(|&j| {
                            let s = &out.current()[j];
                            out.placeable(j, &carried) && (s.count == 0 || same(s, &carried))
                        });
                        if !drag.is_empty() && left > 0 {
                            let amount = if mode == 0 {
                                left / drag.len() as i32
                            } else {
                                1
                            };
                            for &j in &drag {
                                let old = out.current()[j].count;
                                let limit = out.limit(j, &carried);
                                let add = if mode == 2 && creative {
                                    limit - old
                                } else {
                                    amount.min(left).min((limit - old).max(0))
                                };
                                if add > 0 {
                                    out.put(j, counted(carried.clone(), old + add));
                                    if mode != 2 {
                                        left -= add;
                                    }
                                }
                            }
                            out.cursor(counted(carried, left));
                        }
                        drag.clear();
                    }
                    _ => {}
                }
                continue;
            }
            if click.slot == -999 {
                if click.kind == 0 {
                    let c = (*out.carried).clone();
                    let count = if click.button == 0 { 0 } else { c.count - 1 };
                    out.cursor(counted(c, count));
                }
                continue;
            }
            let Some(slot) = out.current().get(i).map(|s| (**s).clone()) else {
                continue;
            };
            // Crafting output/input and equipment effects need their own predictor.
            if out.open.is_none() && i < 9 {
                continue;
            }
            let cursor = (*out.carried).clone();
            match click.kind {
                0 => {
                    if cursor.count == 0 {
                        let amount = if click.button == 0 {
                            slot.count
                        } else {
                            (slot.count + 1) / 2
                        };
                        out.cursor(counted(slot.clone(), amount));
                        out.put(i, counted(slot.clone(), slot.count - amount));
                    } else if out.placeable(i, &cursor) {
                        if slot.count == 0 || same(&slot, &cursor) {
                            let amount = (out.limit(i, &cursor) - slot.count)
                                .max(0)
                                .min(if click.button == 0 { cursor.count } else { 1 });
                            if amount > 0 {
                                out.put(i, counted(cursor.clone(), slot.count + amount));
                                out.cursor(counted(cursor.clone(), cursor.count - amount));
                            }
                        } else if cursor.count <= out.limit(i, &cursor) {
                            out.put(i, cursor);
                            out.cursor(slot);
                        }
                    }
                }
                1 => {
                    let targets: Vec<_> = if let Some(menu) = &out.open {
                        if menu.player_index(i).is_none() {
                            (0..menu.slots.len())
                                .rev()
                                .filter(|&j| menu.player_index(j).is_some())
                                .collect()
                        } else {
                            (0..menu.slots.len())
                                .filter(|&j| menu.player_index(j).is_none())
                                .collect()
                        }
                    } else if i >= 36 {
                        (9..36).collect()
                    } else {
                        (36..45).collect()
                    };
                    let remaining = out.move_to(slot, &targets);
                    out.put(i, remaining);
                }
                2 => {
                    let player = if click.button == 40 {
                        45
                    } else {
                        36 + click.button as usize
                    };
                    if player >= out.player.len() {
                        continue;
                    }
                    let other = (*out.player[player]).clone();
                    if other.count > out.limit(i, &other) {
                        continue;
                    }
                    if out.open.is_none() && i == player {
                        continue;
                    }
                    out.put(i, other);
                    out.player[player] = Arc::new(slot);
                    out.share_player(player);
                }
                3 if creative && cursor.count == 0 && slot.count > 0 => {
                    let max = slot.maximum();
                    out.cursor(counted(slot, max));
                }
                4 if cursor.count == 0 => {
                    let remaining = if click.button == 0 { slot.count - 1 } else { 0 };
                    out.put(i, counted(slot, remaining));
                }
                6 if cursor.count > 0 => {
                    let mut cursor = cursor;
                    for full in [false, true] {
                        let mut order: Vec<_> = (0..out.current().len()).collect();
                        if click.button == 1 {
                            order.reverse();
                        }
                        for j in order {
                            if out.open.is_none() && j < 9 {
                                continue;
                            }
                            let target = (*out.current()[j]).clone();
                            if target.count == 0
                                || !same(&target, &cursor)
                                || (!full && target.count >= target.maximum())
                            {
                                continue;
                            }
                            let take = (cursor.maximum() - cursor.count).max(0).min(target.count);
                            if take > 0 {
                                cursor.count += take;
                                out.put(j, counted(target.clone(), target.count - take));
                            }
                        }
                    }
                    out.cursor(cursor);
                }
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    #[test]
    fn storage_prediction_matches_original_chest_menu_after_every_source_click() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../../data/26.2/container-prediction.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let decode = |s: &serde_json::Value| {
            base64::engine::general_purpose::STANDARD
                .decode(s.as_str().unwrap())
                .unwrap()
        };
        let mut initial = Store::default();
        for row in r["initial"].as_array().unwrap() {
            initial
                .packet(row["id"].as_i64().unwrap() as i32, &decode(&row["body"]))
                .unwrap();
        }
        let mut clicks = Vec::new();
        for (step, row) in r["steps"].as_array().unwrap().iter().enumerate() {
            clicks.push(Click::new(
                row["slot"].as_i64().unwrap() as i16,
                row["button"].as_u64().unwrap() as u8,
                row["kind"].as_u64().unwrap() as u8,
            ));
            let predicted = initial.predict(&clicks, false);
            let mut expected = initial.clone();
            expected.packet(0x12, &decode(&row["body"])).unwrap();
            assert_eq!(
                predicted.open.as_ref().unwrap().slots,
                expected.open.as_ref().unwrap().slots,
                "step {step} {row}"
            );
            assert_eq!(predicted.player, expected.player, "step {step}");
            assert_eq!(predicted.carried, expected.carried, "step {step}");
        }
        assert_eq!(
            initial.open.as_ref().unwrap().slots[0].count,
            23,
            "Prediction mutated authority"
        );
    }
}
