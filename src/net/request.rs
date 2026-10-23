//! User-generated actions; only the socket worker encodes/sends them.
use super::codec::{string, varint};
use anyhow::{ensure, Result};
#[derive(Clone, Copy, Debug)]
pub enum InventoryAction {
    SwapHands,
    DropItem,
    DropAll,
}
impl InventoryAction {
    fn id(self) -> i32 {
        match self {
            Self::SwapHands => 6,
            Self::DropItem => 4,
            Self::DropAll => 3,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigAction {
    Start,
    Abort,
    Stop,
}
impl DigAction {
    pub fn id(self) -> i32 {
        match self {
            Self::Start => 0,
            Self::Abort => 1,
            Self::Stop => 2,
        }
    }
}
#[derive(Clone, Debug)]
pub enum Request {
    SignUpdate {
        serial: u64,
        position: [i32; 3],
        front: bool,
        lines: [String; 4],
    },
    MenuClick {
        id: i32,
        serial: u64,
        state: i32,
        click: super::menu_click::Click,
    },
    UseItem {
        hand: super::entity::Hand,
        slot: u8,
        input: u8,
        yaw: f32,
        pitch: f32,
        teleport_version: u64,
    },
    Dig {
        action: DigAction,
        block: [i32; 3],
        face: u8,
        slot: u8,
        input: u8,
        teleport_version: u64,
    },
    Chat(String),
    InventoryAction {
        action: InventoryAction,
        slot: u8,
        input: u8,
        teleport_version: u64,
    },
    CloseMenu {
        id: i32,
        serial: u64,
    },
    EntityAction {
        hit: crate::world::entity_pick::EntityHit,
        attack: bool,
        hand: super::entity::Hand,
        slot: u8,
        input: u8,
        teleport_version: u64,
    },
    Swing {
        slot: u8,
        input: u8,
        teleport_version: u64,
    },
    UseBlock {
        hit: crate::world::picking::BlockHit,
        hand: super::entity::Hand,
        slot: u8,
        input: u8,
        teleport_version: u64,
    },
    Suggestions {
        id: i32,
        input: String,
    },
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::SignUpdate {
                position,
                front,
                lines,
                ..
            } => {
                super::sign_edit::encode(*position, *front, lines)?;
            }
            Self::MenuClick {
                id, state, click, ..
            } => {
                ensure!(*id >= 0 && *state >= 0, "Invalid menu click state");
                click.validate()?;
            }
            Self::UseItem {
                slot,
                input,
                yaw,
                pitch,
                ..
            } => ensure!(
                *slot < 9
                    && input & 128 == 0
                    && yaw.is_finite()
                    && pitch.is_finite()
                    && (-90. ..=90.).contains(pitch),
                "Invalid item use"
            ),
            Self::Dig {
                block,
                face,
                slot,
                input,
                ..
            } => ensure!(
                *slot < 9
                    && input & 128 == 0
                    && *face < 6
                    && (-33554432..33554432).contains(&block[0])
                    && (-33554432..33554432).contains(&block[2])
                    && (-2048..2048).contains(&block[1]),
                "Invalid block action"
            ),
            Self::InventoryAction { slot, input, .. } => {
                ensure!(*slot < 9 && input & 128 == 0, "Invalid inventory action")
            }
            Self::CloseMenu { id, .. } => ensure!(*id >= 0, "Invalid menu close ID"),
            Self::EntityAction {
                hit, slot, input, ..
            } => ensure!(
                *slot < 9
                    && input & 128 == 0
                    && hit.position.is_finite()
                    && hit.relative.is_finite()
                    && hit.relative.abs().max_element() <= 1024.,
                "Invalid entity action"
            ),
            Self::Swing { slot, input, .. } => {
                ensure!(*slot < 9 && input & 128 == 0, "Invalid swing input/slot")
            }
            Self::UseBlock {
                hit, slot, input, ..
            } => ensure!(
                *slot < 9
                    && input & 128 == 0
                    && hit.face < 6
                    && hit.position.is_finite()
                    && (-33554432..33554432).contains(&hit.block[0])
                    && (-33554432..33554432).contains(&hit.block[2])
                    && (-2048..2048).contains(&hit.block[1])
                    && (hit.position - glam::DVec3::from_array(hit.block.map(f64::from)))
                        .abs()
                        .max_element()
                        <= 16.,
                "Invalid block use target/input"
            ),
            Self::Chat(text) => ensure!(
                !text.is_empty()
                    && text.encode_utf16().count() <= 256
                    && text.chars().all(|c| c >= ' ' && c != '\u{7f}' && c != '§'),
                "Invalid chat input"
            ),
            Self::Suggestions { input, .. } => ensure!(
                input.encode_utf16().count() <= 32767,
                "Suggestion input too long"
            ),
        }
        Ok(())
    }
    pub fn encode(
        &self,
        timestamp_ms: i64,
        salt: i64,
        seen: &mut super::last_seen::Tracker,
    ) -> Result<(i32, Vec<u8>)> {
        self.encode_sequenced(timestamp_ms, salt, seen, 0)
    }
    pub fn encode_sequenced(
        &self,
        timestamp_ms: i64,
        salt: i64,
        seen: &mut super::last_seen::Tracker,
        sequence: i32,
    ) -> Result<(i32, Vec<u8>)> {
        self.validate()?;
        ensure!(sequence >= 0, "Negative action sequence");
        let mut body = Vec::new();
        let id = match self {
            Self::SignUpdate {
                position,
                front,
                lines,
                ..
            } => {
                body = super::sign_edit::encode(*position, *front, lines)?;
                0x3d
            }
            Self::MenuClick {
                id, state, click, ..
            } => {
                body = click.encode(*id, *state)?;
                0x12
            }
            Self::UseItem {
                hand, yaw, pitch, ..
            } => {
                varint(
                    match hand {
                        super::entity::Hand::Main => 0,
                        super::entity::Hand::Off => 1,
                    },
                    &mut body,
                );
                varint(sequence, &mut body);
                body.extend(yaw.to_be_bytes());
                body.extend(pitch.to_be_bytes());
                0x43
            }
            Self::Dig {
                action,
                block,
                face,
                ..
            } => {
                varint(action.id(), &mut body);
                let [x, y, z] = *block;
                let packed = ((i64::from(x) & 0x3ffffff) << 38)
                    | ((i64::from(z) & 0x3ffffff) << 12)
                    | (i64::from(y) & 0xfff);
                body.extend(packed.to_be_bytes());
                body.push(*face);
                varint(
                    if *action == DigAction::Abort {
                        0
                    } else {
                        sequence
                    },
                    &mut body,
                );
                0x29
            }
            Self::InventoryAction { action, .. } => {
                varint(action.id(), &mut body);
                body.extend(0i64.to_be_bytes());
                body.push(0);
                varint(0, &mut body);
                0x29
            }
            Self::CloseMenu { id, .. } => {
                varint(*id, &mut body);
                0x13
            }
            Self::EntityAction {
                hit,
                attack,
                hand,
                input,
                ..
            } => {
                return if *attack {
                    Ok(super::entity::attack(hit.id))
                } else {
                    super::entity::interact(hit.id, *hand, hit.relative, input & 32 != 0)
                }
            }
            Self::Swing { .. } => {
                varint(0, &mut body);
                0x3f
            }
            Self::UseBlock { hit, hand, .. } => {
                varint(
                    match hand {
                        super::entity::Hand::Main => 0,
                        super::entity::Hand::Off => 1,
                    },
                    &mut body,
                );
                let [x, y, z] = hit.block;
                let packed = ((i64::from(x) & 0x3ffffff) << 38)
                    | ((i64::from(z) & 0x3ffffff) << 12)
                    | (i64::from(y) & 0xfff);
                body.extend(packed.to_be_bytes());
                varint(i32::from(hit.face), &mut body);
                for a in 0..3 {
                    body.extend(((hit.position[a] - f64::from(hit.block[a])) as f32).to_be_bytes());
                }
                body.extend([hit.inside as u8, hit.border as u8]);
                varint(sequence, &mut body);
                0x42
            }
            Self::Chat(text) => {
                if let Some(command) = text.strip_prefix('/') {
                    string(command, &mut body);
                    0x07
                } else {
                    string(text, &mut body);
                    body.extend(timestamp_ms.to_be_bytes());
                    body.extend(salt.to_be_bytes());
                    body.push(0); // unsigned offline profile
                    body.extend(seen.update());
                    0x09
                }
            }
            Self::Suggestions { id, input } => {
                varint(*id, &mut body);
                string(input, &mut body);
                0x0f
            }
        };
        Ok((id, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    #[test]
    fn air_use_and_every_dig_action_face_match_original_packets() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/mining-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        for (index, row) in reference["packets"].as_array().unwrap().iter().enumerate() {
            let request = if index < 2 {
                Request::UseItem {
                    hand: if index == 0 {
                        super::super::entity::Hand::Main
                    } else {
                        super::super::entity::Hand::Off
                    },
                    slot: 8,
                    input: 32,
                    yaw: -173.25,
                    pitch: 32.5,
                    teleport_version: 3,
                }
            } else {
                let action = match (index - 2) / 6 {
                    0 => DigAction::Start,
                    1 => DigAction::Abort,
                    _ => DigAction::Stop,
                };
                Request::Dig {
                    action,
                    block: [-31, -22, 47],
                    face: ((index - 2) % 6) as u8,
                    slot: 8,
                    input: 32,
                    teleport_version: 3,
                }
            };
            let actual = request
                .encode_sequenced(0, 0, &mut Default::default(), 300)
                .unwrap();
            assert_eq!(actual.0, row["id"].as_i64().unwrap() as i32);
            assert_eq!(
                actual.1,
                base64::engine::general_purpose::STANDARD
                    .decode(row["body"].as_str().unwrap())
                    .unwrap(),
                "{}",
                row["label"]
            );
        }
    }
    #[test]
    fn inventory_actions_and_varint_close_ids_match_original_jar() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/inventory-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let requests = [
            Request::InventoryAction {
                action: InventoryAction::SwapHands,
                slot: 8,
                input: 32,
                teleport_version: 1,
            },
            Request::InventoryAction {
                action: InventoryAction::DropItem,
                slot: 8,
                input: 32,
                teleport_version: 1,
            },
            Request::InventoryAction {
                action: InventoryAction::DropAll,
                slot: 8,
                input: 32,
                teleport_version: 1,
            },
            Request::CloseMenu { id: 300, serial: 9 },
            Request::CloseMenu { id: 0, serial: 10 },
        ];
        let rows = reference["actions"].as_array().unwrap();
        assert_eq!(requests.len(), rows.len());
        for (request, row) in requests.iter().zip(rows) {
            let encoded = request
                .encode_sequenced(0, 0, &mut Default::default(), 31)
                .unwrap();
            assert_eq!(encoded.0, row["id"].as_i64().unwrap() as i32);
            assert_eq!(
                encoded.1,
                base64::engine::general_purpose::STANDARD
                    .decode(row["body"].as_str().unwrap())
                    .unwrap(),
                "{}",
                row["label"]
            );
        }
    }
    #[test]
    fn entity_actions_match_original_nonzero_lp_vector_and_secondary_flag() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/entity-picking.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        for row in reference["interactions"].as_array().unwrap() {
            let hit = crate::world::entity_pick::EntityHit {
                id: 300,
                uuid: [1; 16],
                position: glam::DVec3::ZERO,
                relative: glam::DVec3::new(1.125, -0.125, 0.25),
            };
            let request = Request::EntityAction {
                hit,
                attack: false,
                hand: if row["hand"] == 0 {
                    super::super::entity::Hand::Main
                } else {
                    super::super::entity::Hand::Off
                },
                slot: 8,
                input: if row["secondary"] == true { 32 } else { 0 },
                teleport_version: 1,
            };
            let (id, body) = request
                .encode(0, 0, &mut super::super::last_seen::Tracker::default())
                .unwrap();
            assert_eq!(id, 0x1a);
            assert_eq!(
                body,
                base64::engine::general_purpose::STANDARD
                    .decode(row["body"].as_str().unwrap())
                    .unwrap()
            );
            let attack = Request::EntityAction {
                hit,
                attack: true,
                hand: super::super::entity::Hand::Main,
                slot: 8,
                input: 0,
                teleport_version: 1,
            };
            assert_eq!(
                attack
                    .encode(0, 0, &mut super::super::last_seen::Tracker::default())
                    .unwrap(),
                (1, vec![0xac, 2])
            );
        }
    }
    #[test]
    fn block_use_matches_all_original_hands_faces_and_inside_flags() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/use-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let data: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        for row in data["packets"].as_array().unwrap() {
            let request = Request::UseBlock {
                hit: crate::world::picking::BlockHit {
                    block: [-31, -22, 47],
                    position: glam::DVec3::new(-30.875, -21.25, 47.625),
                    face: row["face"].as_u64().unwrap() as u8,
                    inside: row["inside"].as_bool().unwrap(),
                    border: true,
                },
                hand: if row["hand"] == 0 {
                    super::super::entity::Hand::Main
                } else {
                    super::super::entity::Hand::Off
                },
                slot: 8,
                input: 32,
                teleport_version: 1,
            };
            let (id, body) = request
                .encode_sequenced(0, 0, &mut super::super::last_seen::Tracker::default(), 300)
                .unwrap();
            assert_eq!(id, 0x42);
            assert_eq!(
                body,
                base64::engine::general_purpose::STANDARD
                    .decode(row["body"].as_str().unwrap())
                    .unwrap()
            );
        }
    }
    #[test]
    fn offline_chat_command_and_suggestion_request_match_original_codec_bytes() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/chat-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for (index, request) in [
            (0, Request::Chat("Привет 😀".into())),
            (1, Request::Chat("/help".into())),
            (
                3,
                Request::Suggestions {
                    id: 42,
                    input: "/msg Al".into(),
                },
            ),
        ] {
            let expected = &r["packets"][index];
            let (id, body) = request
                .encode(
                    2000000000000,
                    1234,
                    &mut super::super::last_seen::Tracker::default(),
                )
                .unwrap();
            assert_eq!(id, expected["id"].as_i64().unwrap() as i32);
            assert_eq!(
                body,
                base64::engine::general_purpose::STANDARD
                    .decode(expected["body"].as_str().unwrap())
                    .unwrap()
            );
        }
    }
}
