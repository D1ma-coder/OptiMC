//! Shared entity metadata storage. Unsupported codecs are explicit, never guessed.
use super::{
    codec::{read_string, read_varint, read_varlong},
    items::{Stack, UnsupportedComponent},
    maps::{byte, bytes},
    nbt,
};
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Byte(u8),
    Int(i32),
    Item(Stack),
    Encoded { serializer: i32, bytes: Vec<u8> },
}
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub entities: BTreeMap<i32, Arc<BTreeMap<u8, Value>>>,
    pub revision: u64,
}

fn decode(mut input: &[u8]) -> Result<(i32, BTreeMap<u8, Value>)> {
    let entity = read_varint(&mut input)?;
    let mut values = BTreeMap::new();
    let reference = super::maps::reference();
    loop {
        let index = byte(&mut input)?;
        if index == 255 {
            break;
        }
        ensure!(
            !values.contains_key(&index),
            "Duplicate entity metadata index"
        );
        let serializer = read_varint(&mut input)?;
        let name = reference
            .serializers
            .iter()
            .find(|(_, id)| **id == serializer)
            .map(|(name, _)| name.as_str())
            .ok_or_else(|| UnsupportedComponent(format!("entity serializer {serializer}")))?;
        let start = input;
        let value = match name {
            "BYTE" => Value::Byte(byte(&mut input)?),
            "INT" => Value::Int(read_varint(&mut input)?),
            "ITEM_STACK" => Value::Item(Stack::read(&mut input)?),
            _ => {
                match name {
                    "LONG" => {
                        read_varlong(&mut input)?;
                    }
                    "FLOAT" => {
                        bytes(&mut input, 4)?;
                    }
                    "STRING" => {
                        read_string(&mut input, 32767)?;
                    }
                    "COMPONENT" => {
                        nbt::read_network(&mut input)?;
                    }
                    "OPTIONAL_COMPONENT" => {
                        if byte(&mut input)? != 0 {
                            nbt::read_network(&mut input)?;
                        }
                    }
                    "BOOLEAN" => {
                        byte(&mut input)?;
                    }
                    "ROTATIONS" | "VECTOR3" => {
                        bytes(&mut input, 12)?;
                    }
                    "QUATERNION" => {
                        bytes(&mut input, 16)?;
                    }
                    "BLOCK_POS" => {
                        bytes(&mut input, 8)?;
                    }
                    "OPTIONAL_BLOCK_POS" => {
                        if byte(&mut input)? != 0 {
                            bytes(&mut input, 8)?;
                        }
                    }
                    "DIRECTION"
                    | "BLOCK_STATE"
                    | "OPTIONAL_BLOCK_STATE"
                    | "OPTIONAL_UNSIGNED_INT"
                    | "POSE"
                    | "HUMANOID_ARM"
                    | "ARMADILLO_STATE"
                    | "SNIFFER_STATE"
                    | "WEATHERING_COPPER_STATE"
                    | "COPPER_GOLEM_STATE" => {
                        read_varint(&mut input)?;
                    }
                    _ => {
                        return Err(UnsupportedComponent(format!("entity serializer {name}")).into())
                    }
                }
                Value::Encoded {
                    serializer,
                    bytes: start[..start.len() - input.len()].to_vec(),
                }
            }
        };
        values.insert(index, value);
    }
    ensure!(input.is_empty(), "Trailing entity metadata bytes");
    Ok((entity, values))
}
impl Store {
    pub fn packet(&mut self, data: &[u8]) -> Result<bool> {
        let (id, values) = match decode(data) {
            Ok(value) => value,
            Err(error) if error.downcast_ref::<UnsupportedComponent>().is_some() => {
                return Ok(false)
            }
            Err(error) => return Err(error),
        };
        ensure!(
            self.entities.contains_key(&id) || self.entities.len() < 16384,
            "Entity metadata limit exceeded"
        );
        if values.is_empty() {
            return Ok(true);
        }
        let current = Arc::make_mut(self.entities.entry(id).or_default());
        let changed = values
            .iter()
            .any(|(index, value)| current.get(index) != Some(value));
        if changed {
            current.extend(values);
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(true)
    }
    pub fn frame(&self, id: i32) -> (bool, u8, Option<&Stack>) {
        let Some(values) = self.entities.get(&id) else {
            return (false, 0, None);
        };
        let r = super::maps::reference();
        let invisible =
            matches!(values.get(&r.flags_index),Some(Value::Byte(flags)) if flags&32!=0);
        let rotation = match values.get(&r.rotation_index) {
            Some(Value::Int(rotation)) => (*rotation as u8) & 7,
            _ => 0,
        };
        let item = match values.get(&r.item_index) {
            Some(Value::Item(item)) if item.count > 0 => Some(item),
            _ => None,
        };
        (invisible, rotation, item)
    }
}

/// The six independent server rotations and flags used by ArmorStandModel.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmorStand {
    pub flags: u8,
    pub poses: [[f32; 3]; 6],
    pub yaw: f32,
}
impl Store {
    pub fn armor_stand(&self, id: i32, yaw: f32) -> Result<ArmorStand> {
        let reference = crate::assets::signs::reference()?;
        let values = self.entities.get(&id);
        let fields = [
            "HEAD",
            "BODY",
            "LEFT_ARM",
            "RIGHT_ARM",
            "LEFT_LEG",
            "RIGHT_LEG",
        ];
        let mut poses = [[0.; 3]; 6];
        for (i, field) in fields.iter().enumerate() {
            for axis in 0..3 {
                poses[i][axis] = reference["armor_defaults"][format!("DEFAULT_{field}_POSE")][axis]
                    .as_f64()
                    .unwrap() as f32;
            }
            let index = reference["armor_indices"][format!("DATA_{field}_POSE")]
                .as_u64()
                .unwrap() as u8;
            if let Some(Value::Encoded { serializer, bytes }) = values.and_then(|v| v.get(&index)) {
                if *serializer == super::maps::reference().serializers["ROTATIONS"]
                    && bytes.len() == 12
                {
                    for (axis, raw) in bytes.as_chunks::<4>().0.iter().enumerate() {
                        let value = f32::from_be_bytes(*raw);
                        poses[i][axis] = if value.is_finite() { value % 360. } else { 0. };
                    }
                }
            }
        }
        let flags_index = reference["armor_indices"]["DATA_CLIENT_FLAGS"]
            .as_u64()
            .unwrap() as u8;
        let flags = match values.and_then(|v| v.get(&flags_index)) {
            Some(Value::Byte(v)) => *v,
            _ => 0,
        };
        Ok(ArmorStand { flags, poses, yaw })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_frame_contents_emptying_and_rotation_use_exported_indices() {
        let mut store = Store::default();
        for (label, id, body) in super::super::maps::packets() {
            if id != 0x63 {
                continue;
            }
            assert!(store.packet(&body).unwrap());
            let (invisible, rotation, item) = store.frame(99);
            assert!(!invisible);
            if label == "contents" {
                assert_eq!(rotation, 1);
                assert_eq!(item.unwrap().map, Some(42));
            } else if label == "empty" {
                assert!(item.is_none());
            }
        }
    }
    #[test]
    fn truncated_metadata_never_publishes_a_partial_stack() {
        let body = super::super::maps::packets()
            .into_iter()
            .find(|p| p.0 == "contents")
            .unwrap()
            .2;
        for end in 0..body.len() {
            let mut store = Store::default();
            assert!(store.packet(&body[..end]).is_err());
            assert!(store.entities.is_empty());
        }
    }
}
