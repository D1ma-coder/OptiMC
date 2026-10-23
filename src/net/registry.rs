//! Configuration registry entries retain their exact packet-order protocol IDs.
use super::{
    codec::{read_string, read_varint},
    nbt::{Budget, Limits, Tag},
};
use crate::assets::models::ResourceId;
use anyhow::{ensure, Context, Result};
use std::{collections::HashMap, io::Read, sync::Arc};
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub data: Option<Arc<Tag>>,
}
#[derive(Clone, Debug)]
pub struct Registry {
    pub entries: Vec<Entry>,
    by_name: HashMap<String, u32>,
}
#[derive(Clone, Debug, Default)]
pub struct Registries {
    pub values: HashMap<String, Registry>,
    entries: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimension {
    pub name: String,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub skylight: bool,
    pub ceiling: bool,
}
impl Registries {
    pub fn packet(&mut self, bytes: &[u8]) -> Result<()> {
        let mut input = bytes;
        let kind = identifier(&mut input)?;
        let count = read_varint(&mut input)?;
        ensure!(
            count >= 0 && count as usize <= 100_000 && count as usize <= input.len(),
            "Invalid registry entry count"
        );
        let count = count as usize;
        let old = self.values.get(&kind).map_or(0, |v| v.entries.len());
        ensure!(
            self.entries - old + count <= 1_000_000,
            "Registry entry budget exceeded"
        );
        let mut entries = Vec::with_capacity(count);
        let mut by_name = HashMap::with_capacity(count);
        let mut budget = Budget::new(Limits {
            memory: 32 * 1024 * 1024,
            ..Limits::default()
        });
        for id in 0..count {
            let name = identifier(&mut input)?;
            ensure!(
                by_name.insert(name.clone(), id as u32).is_none(),
                "Duplicate registry name {name}"
            );
            let mut present = [0];
            input.read_exact(&mut present)?;
            let data = if present[0] != 0 {
                Some(Arc::new(budget.read_network(&mut input)?))
            } else {
                None
            };
            entries.push(Entry { name, data });
        }
        ensure!(input.is_empty(), "Trailing registry packet bytes");
        self.entries = self.entries - old + count;
        self.values.insert(kind, Registry { entries, by_name });
        Ok(())
    }
    pub fn entry(&self, registry: &str, id: u32) -> Option<&Entry> {
        self.values.get(registry)?.entries.get(id as usize)
    }
    pub fn id(&self, registry: &str, name: &str) -> Option<u32> {
        self.values.get(registry)?.by_name.get(name).copied()
    }
    pub fn dimension(&self, id: u32) -> Result<Dimension> {
        let entry = self
            .entry("minecraft:dimension_type", id)
            .context("Missing dimension type registry entry")?;
        let data = entry
            .data
            .as_ref()
            .context("Dimension data omitted despite declining known packs")?;
        let number = |key: &str| -> Result<i32> {
            let value = data
                .get(key)
                .and_then(Tag::integer)
                .with_context(|| format!("Missing dimension {key}"))?;
            Ok(value.try_into()?)
        };
        let min_y = number("min_y")?;
        let height = number("height")?;
        let logical_height = number("logical_height")?;
        ensure!(
            (-2032..=2032).contains(&min_y) && min_y.rem_euclid(16) == 0,
            "Invalid dimension min_y"
        );
        ensure!(
            (16..=4064).contains(&height) && height % 16 == 0 && min_y + height <= 2032,
            "Invalid dimension height"
        );
        ensure!(
            (0..=height).contains(&logical_height),
            "Invalid dimension logical height"
        );
        let boolean = |key: &str| -> Result<bool> {
            Ok(data
                .get(key)
                .and_then(Tag::integer)
                .with_context(|| format!("Missing dimension {key}"))?
                != 0)
        };
        Ok(Dimension {
            name: entry.name.clone(),
            min_y,
            height: height as u32,
            logical_height: logical_height as u32,
            skylight: boolean("has_skylight")?,
            ceiling: boolean("has_ceiling")?,
        })
    }
    pub fn len(&self) -> usize {
        self.entries
    }
    pub fn is_empty(&self) -> bool {
        self.entries == 0
    }
}
pub fn identifier(input: &mut impl Read) -> Result<String> {
    Ok(ResourceId::parse(&read_string(input, 32767)?)?.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{
        codec::{string, varint},
        nbt::{self, NbtString},
    };
    fn dimension_tag() -> Tag {
        Tag::Compound(
            [
                ("min_y", Tag::Int(-64)),
                ("height", Tag::Int(384)),
                ("logical_height", Tag::Int(384)),
                ("has_skylight", Tag::Byte(1)),
                ("has_ceiling", Tag::Byte(0)),
            ]
            .into_iter()
            .map(|(k, v)| (NbtString::from(k), v))
            .collect(),
        )
    }
    pub(super) fn packet() -> Vec<u8> {
        let mut bytes = Vec::new();
        string("minecraft:dimension_type", &mut bytes);
        varint(2, &mut bytes);
        string("minecraft:overworld", &mut bytes);
        bytes.push(1);
        nbt::write_network(&dimension_tag(), &mut bytes).unwrap();
        string("minecraft:omitted", &mut bytes);
        bytes.push(0);
        bytes
    }
    #[test]
    fn registry_ids_follow_packet_order_and_optional_data() {
        let mut registries = Registries::default();
        registries.packet(&packet()).unwrap();
        assert_eq!(
            registries.id("minecraft:dimension_type", "minecraft:omitted"),
            Some(1)
        );
        let dimension = registries.dimension(0).unwrap();
        assert_eq!(dimension.min_y, -64);
        assert_eq!(dimension.height, 384);
        assert!(dimension.skylight);
        assert!(registries.dimension(1).is_err());
    }
    #[test]
    fn malformed_registry_does_not_replace_the_previous_registry() {
        let mut registries = Registries::default();
        registries.packet(&packet()).unwrap();
        let mut broken = packet();
        broken.push(0);
        assert!(registries.packet(&broken).is_err());
        assert_eq!(registries.len(), 2);
    }
}
