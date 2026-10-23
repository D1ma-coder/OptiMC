//! Server effects, including haste/conduit power and fatigue used by mining.
use super::{codec::read_varint, maps::byte};
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, time::Instant};
#[derive(Clone, Debug)]
pub struct Effect {
    pub amplifier: i32,
    pub duration: i32,
    pub flags: u8,
    pub received: Instant,
}
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub entities: BTreeMap<i32, BTreeMap<u32, Effect>>,
}
impl Store {
    pub fn amplifier(&self, entity: i32, name: &str) -> Option<i32> {
        let ids = crate::assets::builtins::get().ok()?;
        self.entities.get(&entity)?.iter().find_map(|(&id, e)| {
            (ids.name("minecraft:mob_effect", id) == Some(name)
                && (e.duration == -1
                    || e.received.elapsed().as_secs_f64() * 20. < e.duration as f64))
                .then_some(e.amplifier)
        })
    }
    pub fn packet(&mut self, id: i32, mut input: &[u8]) -> Result<()> {
        let entity = read_varint(&mut input)?;
        let effect = read_varint(&mut input)?;
        ensure!(
            effect >= 0
                && crate::assets::builtins::get()?
                    .name("minecraft:mob_effect", effect as u32)
                    .is_some(),
            "Invalid effect ID"
        );
        let value = if id == 0x84 {
            let amplifier = read_varint(&mut input)?;
            let duration = read_varint(&mut input)?;
            let flags = byte(&mut input)?;
            ensure!(
                amplifier >= 0 && duration >= -1,
                "Invalid effect duration/amplifier"
            );
            Some(Effect {
                amplifier,
                duration,
                flags,
                received: Instant::now(),
            })
        } else {
            ensure!(id == 0x4e, "Invalid effect packet");
            None
        };
        ensure!(input.is_empty(), "Trailing effect bytes");
        if let Some(value) = value {
            ensure!(
                self.entities.len() < 16384 || self.entities.contains_key(&entity),
                "Effect entity budget exceeded"
            );
            self.entities
                .entry(entity)
                .or_default()
                .insert(effect as u32, value);
        } else if let Some(effects) = self.entities.get_mut(&entity) {
            effects.remove(&(effect as u32));
            if effects.is_empty() {
                self.entities.remove(&entity);
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    #[test]
    fn original_effects_preserve_duration_flags_and_remove_atomically() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/mining-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let mut store = Store::default();
        for row in r["effects"].as_array().unwrap() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            let id = row["id"].as_i64().unwrap() as i32;
            for len in 0..body.len() {
                let mut bad = store.clone();
                assert!(bad.packet(id, &body[..len]).is_err());
                assert_eq!(bad.entities.len(), store.entities.len());
            }
            store.packet(id, &body).unwrap();
            match row["label"].as_str().unwrap() {
                "haste" => assert_eq!(store.amplifier(42, "minecraft:haste"), Some(2)),
                "fatigue" => assert_eq!(store.amplifier(42, "minecraft:mining_fatigue"), Some(1)),
                "remove" => assert_eq!(store.amplifier(42, "minecraft:haste"), None),
                _ => unreachable!(),
            }
        }
        assert_eq!(store.amplifier(42, "minecraft:mining_fatigue"), Some(1));
    }
}
