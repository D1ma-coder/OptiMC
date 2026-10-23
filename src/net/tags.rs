//! Server tag replacement per registry; shared by Configuration and Play.
use super::{codec::read_varint, registry::identifier};
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, sync::Arc};
type Tags = BTreeMap<String, Vec<i32>>;
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub registries: BTreeMap<String, Arc<Tags>>,
}
fn count(input: &mut &[u8], maximum: usize) -> Result<usize> {
    let n = read_varint(input)?;
    ensure!(n >= 0 && n as usize <= maximum, "Tag count exceeds budget");
    Ok(n as usize)
}
impl Store {
    pub fn contains(&self, registry: &str, tag: &str, id: u32) -> bool {
        i32::try_from(id).ok().is_some_and(|id| {
            self.registries
                .get(registry)
                .and_then(|tags| tags.get(tag))
                .is_some_and(|ids| ids.binary_search(&id).is_ok())
        })
    }
    pub fn packet(&mut self, mut input: &[u8]) -> Result<()> {
        let n = count(&mut input, 1024)?;
        let mut replacements = BTreeMap::new();
        let mut budget = 0usize;
        for _ in 0..n {
            let registry = identifier(&mut input)?;
            let n = count(&mut input, 65536)?;
            let mut tags = BTreeMap::new();
            for _ in 0..n {
                let tag = identifier(&mut input)?;
                let n = count(&mut input, 1_000_000)?;
                budget = budget
                    .checked_add(n)
                    .ok_or_else(|| anyhow::anyhow!("Tag budget overflow"))?;
                ensure!(
                    budget <= 1_000_000 && n <= input.len(),
                    "Tag member budget exceeded"
                );
                let mut ids = Vec::with_capacity(n);
                for _ in 0..n {
                    let id = read_varint(&mut input)?;
                    ensure!(id >= 0, "Negative tag registry ID");
                    ids.push(id);
                }
                ids.sort_unstable();
                ids.dedup();
                ensure!(tags.insert(tag, ids).is_none(), "Duplicate tag");
            }
            ensure!(
                replacements.insert(registry, Arc::new(tags)).is_none(),
                "Duplicate tag registry"
            );
        }
        ensure!(input.is_empty(), "Trailing tag bytes");
        let mut next = self.registries.clone();
        next.extend(replacements);
        ensure!(
            next.len() <= 1024
                && next
                    .values()
                    .map(|tags| tags.values().map(Vec::len).sum::<usize>())
                    .sum::<usize>()
                    <= 1_000_000,
            "Total tag budget exceeded"
        );
        self.registries = next;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    #[test]
    fn original_tag_packet_is_atomic_replaces_registry_and_preserves_absent_registries() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/entity-picking.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(r["tags_packet"].as_str().unwrap())
            .unwrap();
        let mut store = Store::default();
        store.packet(&bytes).unwrap();
        assert!(store.contains(
            "minecraft:entity_type",
            "minecraft:redirectable_projectile",
            127
        ));
        assert!(!store.contains(
            "minecraft:entity_type",
            "minecraft:redirectable_projectile",
            2
        ));
        for n in 0..bytes.len() {
            let mut failed = store.clone();
            assert!(failed.packet(&bytes[..n]).is_err(), "{n}");
            assert_eq!(failed.registries, store.registries);
        }
        store.packet(&[0]).unwrap();
        assert_eq!(store.registries.len(), 1);
        let mut empty = vec![1];
        super::super::codec::string("minecraft:entity_type", &mut empty);
        empty.push(0);
        store.packet(&empty).unwrap();
        assert!(!store.contains(
            "minecraft:entity_type",
            "minecraft:redirectable_projectile",
            1
        ));
    }
}
