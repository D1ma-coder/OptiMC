//! Block entity snapshots from chunk payloads and standalone server updates.
use super::{
    codec::read_varint,
    nbt::{self, Tag},
    spawn,
};
use anyhow::{ensure, Result};
use std::io::Read;
#[derive(Clone, Debug, PartialEq)]
pub struct BlockEntity {
    pub position: [i32; 3],
    pub kind: u32,
    pub data: Tag,
}
fn payload(input: &mut &[u8], position: [i32; 3], budget: &mut nbt::Budget) -> Result<BlockEntity> {
    let kind = read_varint(input)?;
    ensure!(kind >= 0, "Negative block entity type");
    let data = budget.read_network(input)?;
    ensure!(
        matches!(data, Tag::End | Tag::Compound(_)),
        "Invalid block entity NBT"
    );
    Ok(BlockEntity {
        position,
        kind: kind as u32,
        data,
    })
}
pub fn chunk(input: &mut &[u8], x: i32, z: i32) -> Result<Vec<BlockEntity>> {
    let count = read_varint(input)?;
    ensure!(
        (0..=65536).contains(&count),
        "Invalid chunk block entity count"
    );
    let mut budget = nbt::Budget::new(nbt::Limits::default());
    let mut result = Vec::new();
    for _ in 0..count {
        let mut packed = [0; 3];
        input.read_exact(&mut packed)?;
        let position = [
            x.checked_mul(16)
                .and_then(|x| x.checked_add((packed[0] >> 4) as i32))
                .ok_or_else(|| anyhow::anyhow!("Chunk coordinate overflow"))?,
            i16::from_be_bytes([packed[1], packed[2]]) as i32,
            z.checked_mul(16)
                .and_then(|z| z.checked_add((packed[0] & 15) as i32))
                .ok_or_else(|| anyhow::anyhow!("Chunk coordinate overflow"))?,
        ];
        result.push(payload(input, position, &mut budget)?);
    }
    Ok(result)
}
pub fn update(bytes: &[u8]) -> Result<BlockEntity> {
    let mut input = bytes;
    let mut position = [0; 8];
    input.read_exact(&mut position)?;
    let value = payload(
        &mut input,
        spawn::block_position(i64::from_be_bytes(position)),
        &mut nbt::Budget::new(nbt::Limits::default()),
    )?;
    ensure!(input.is_empty(), "Trailing block entity update bytes");
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn original_sign_chunk_and_update_payloads_match_and_truncations_fail() {
        let r = crate::assets::signs::reference().unwrap();
        let decode = |key: &str| {
            base64::engine::general_purpose::STANDARD
                .decode(r[key].as_str().unwrap())
                .unwrap()
        };
        let bytes = decode("update");
        let entity = update(&bytes).unwrap();
        assert_eq!(entity.position, [-31, -22, 47]);
        assert_eq!(
            crate::assets::builtins::get()
                .unwrap()
                .name("minecraft:block_entity_type", entity.kind),
            Some("minecraft:sign")
        );
        let chunk_bytes = decode("chunk_entities");
        let mut input = chunk_bytes.as_slice();
        let entities = chunk(&mut input, -2, 2).unwrap();
        assert!(input.is_empty());
        assert_eq!(entities.as_slice(), std::slice::from_ref(&entity));
        let Tag::List { values, .. } = entity
            .data
            .get("front_text")
            .unwrap()
            .get("messages")
            .unwrap()
        else {
            panic!()
        };
        let text =
            crate::text::RichText::from_json(&crate::net::component::json_tag(&values[0])).unwrap();
        assert_eq!(text.runs[0].text, "Привет");
        assert!(text.runs[0].style.bold);
        for length in 0..bytes.len() {
            assert!(
                update(&bytes[..length]).is_err(),
                "accepted truncation {length}"
            );
        }
        for length in 0..chunk_bytes.len() {
            assert!(
                chunk(&mut &chunk_bytes[..length], -2, 2).is_err(),
                "accepted chunk truncation {length}"
            );
        }
    }
}
