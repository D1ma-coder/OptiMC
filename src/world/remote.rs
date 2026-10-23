//! Client-owned snapshot of chunks received from an external server. This is
//! storage and block update logic, not an integrated server or terrain generator.
use crate::net::{
    chunk::{Chunk, Section},
    codec::{read_varint, read_varlong},
    registry::{Dimension, Registries},
    spawn::{self, SpawnInfo},
};
use anyhow::{ensure, Context, Result};
use std::{collections::HashMap, io::Read, sync::Arc};
#[derive(Clone, Debug)]
pub enum Values {
    Uniform { value: u32, len: usize },
    Dense(Arc<[u32]>),
}
impl Values {
    fn new(values: Vec<u32>) -> Self {
        if let Some(&value) = values.first().filter(|&&v| values.iter().all(|&x| x == v)) {
            Self::Uniform {
                value,
                len: values.len(),
            }
        } else {
            Self::Dense(values.into())
        }
    }
    pub fn get(&self, index: usize) -> Option<u32> {
        match self {
            Self::Uniform { value, len } => (*len > index).then_some(*value),
            Self::Dense(values) => values.get(index).copied(),
        }
    }
    fn set(&mut self, index: usize, value: u32) {
        match self {
            Self::Uniform { value: old, len } => {
                if index < *len && *old != value {
                    let mut values = vec![*old; *len];
                    values[index] = value;
                    *self = Self::Dense(values.into());
                }
            }
            Self::Dense(values) => {
                if let Some(slot) = Arc::make_mut(values).get_mut(index) {
                    *slot = value;
                }
            }
        }
    }
    pub fn allocated_values(&self) -> usize {
        match self {
            Self::Uniform { .. } => 1,
            Self::Dense(values) => values.len(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct StoredSection {
    pub blocks: Values,
    pub biomes: Values,
}
impl From<Section> for StoredSection {
    fn from(section: Section) -> Self {
        Self {
            blocks: Values::new(section.blocks),
            biomes: Values::new(section.biomes),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Column {
    pub sections: Vec<Arc<StoredSection>>,
}
#[derive(Clone, Debug, Default)]
pub struct RemoteWorld {
    pub spawn: Option<SpawnInfo>,
    pub dimension: Option<Dimension>,
    pub player_id: Option<i32>,
    pub block_entities: HashMap<[i32; 3], Arc<crate::net::block_entity::BlockEntity>>,
    pub chunks: HashMap<(i32, i32), Arc<Column>>,
}
impl RemoteWorld {
    /// Return whether a new level was created. Same-dimension respawn keeps the
    /// world's other entities/chunks; dimension or height changes discard them.
    pub fn enter(&mut self, spawn: SpawnInfo, registries: &Registries) -> Result<bool> {
        let dimension = registries.dimension(spawn.dimension_type)?;
        let changed = self
            .spawn
            .as_ref()
            .is_none_or(|old| old.dimension != spawn.dimension)
            || self.dimension.as_ref() != Some(&dimension);
        if changed {
            self.chunks.clear();
            self.block_entities.clear();
        }
        self.spawn = Some(spawn);
        self.dimension = Some(dimension);
        Ok(changed)
    }
    pub fn insert(&mut self, chunk: Chunk) -> Result<()> {
        let dimension = self
            .dimension
            .as_ref()
            .context("Chunk arrived before world login/dimension registry")?;
        ensure!(
            chunk.sections.len() == dimension.height as usize / 16,
            "Chunk has {} sections; dimension {} requires {} (min_y={}, height={})",
            chunk.sections.len(),
            dimension.name,
            dimension.height / 16,
            dimension.min_y,
            dimension.height
        );
        self.block_entities
            .retain(|p, _| (p[0].div_euclid(16), p[2].div_euclid(16)) != (chunk.x, chunk.z));
        for entity in chunk.block_entities {
            self.block_entities
                .insert(entity.position, Arc::new(entity));
        }
        self.chunks.insert(
            (chunk.x, chunk.z),
            Arc::new(Column {
                sections: chunk
                    .sections
                    .into_iter()
                    .map(|section| Arc::new(StoredSection::from(section)))
                    .collect(),
            }),
        );
        Ok(())
    }
    fn location(&self, x: i32, y: i32, z: i32) -> Option<((i32, i32), usize, usize)> {
        let dimension = self.dimension.as_ref()?;
        let relative = y.checked_sub(dimension.min_y)?;
        if relative < 0 || relative >= dimension.height as i32 {
            return None;
        }
        let section = relative as usize / 16;
        let index = (y.rem_euclid(16) as usize * 16 + z.rem_euclid(16) as usize) * 16
            + x.rem_euclid(16) as usize;
        Some(((x.div_euclid(16), z.div_euclid(16)), section, index))
    }
    pub fn block(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        let (column, section, index) = self.location(x, y, z)?;
        self.chunks
            .get(&column)?
            .sections
            .get(section)?
            .blocks
            .get(index)
    }
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: u32) {
        if self.block_entities.contains_key(&[x, y, z]) {
            if let Ok(builtin) = crate::assets::builtins::get() {
                let old = self
                    .block(x, y, z)
                    .and_then(|id| builtin.state(id))
                    .map(|s| &s.block);
                let new = builtin.state(state).map(|s| &s.block);
                if old != new {
                    self.block_entities.remove(&[x, y, z]);
                }
            }
        }
        if let Some((column, section, index)) = self.location(x, y, z) {
            if self
                .chunks
                .get(&column)
                .and_then(|c| c.sections.get(section))
                .and_then(|s| s.blocks.get(index))
                == Some(state)
            {
                return;
            }
            if let Some(section) = self
                .chunks
                .get_mut(&column)
                .and_then(|c| Arc::make_mut(c).sections.get_mut(section))
            {
                Arc::make_mut(section).blocks.set(index, state);
            }
        }
    }
    pub fn update(&mut self, bytes: &[u8]) -> Result<()> {
        let mut input = bytes;
        let mut packed = [0; 8];
        input.read_exact(&mut packed)?;
        let [x, y, z] = spawn::block_position(i64::from_be_bytes(packed));
        let state = read_varint(&mut input)?;
        ensure!(
            state >= 0 && input.is_empty(),
            "Invalid block update packet"
        );
        self.set_block(x, y, z, state as u32);
        Ok(())
    }
    pub fn section_update(&mut self, bytes: &[u8]) -> Result<()> {
        let mut input = bytes;
        let mut raw = [0; 8];
        input.read_exact(&mut raw)?;
        let packed = i64::from_be_bytes(raw);
        let x = (packed >> 42) as i32;
        let z = (packed << 22 >> 42) as i32;
        let y = (packed << 44 >> 44) as i32;
        let count = read_varint(&mut input)?;
        ensure!(
            (0..=4096).contains(&count) && count as usize <= input.len(),
            "Invalid section block update count"
        );
        let mut updates = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let record = read_varlong(&mut input)?;
            ensure!(
                record >= 0 && (record >> 12) <= u32::MAX as i64,
                "Invalid block state in section update"
            );
            updates.push((
                x * 16 + ((record >> 8) & 15) as i32,
                y * 16 + (record & 15) as i32,
                z * 16 + ((record >> 4) & 15) as i32,
                (record >> 12) as u32,
            ));
        }
        ensure!(input.is_empty(), "Trailing section block update bytes");
        for (x, y, z, state) in updates {
            self.set_block(x, y, z, state);
        }
        Ok(())
    }
    pub fn block_entity_update(&mut self, bytes: &[u8]) -> Result<()> {
        let entity = crate::net::block_entity::update(bytes)?;
        let [x, y, z] = entity.position;
        // Like ClientPacketListener, updates outside loaded chunks have no target.
        if self.block(x, y, z).is_some() {
            self.block_entities
                .insert(entity.position, Arc::new(entity));
        }
        Ok(())
    }
    pub fn forget(&mut self, bytes: &[u8]) -> Result<()> {
        ensure!(bytes.len() == 8, "Invalid chunk unload packet");
        let packed = i64::from_be_bytes(bytes.try_into()?);
        let x = packed as i32;
        let z = (packed >> 32) as i32;
        self.chunks.remove(&(x, z));
        self.block_entities
            .retain(|p, _| (p[0].div_euclid(16), p[2].div_euclid(16)) != (x, z));
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> RemoteWorld {
        RemoteWorld {
            dimension: Some(Dimension {
                name: "minecraft:overworld".into(),
                min_y: -64,
                height: 384,
                logical_height: 384,
                skylight: true,
                ceiling: false,
            }),
            ..RemoteWorld::default()
        }
    }
    #[test]
    fn uniform_sections_are_compact_and_negative_coordinates_survive_updates() {
        let mut world = world();
        let sections = (0..24)
            .map(|_| Section {
                non_air: 0,
                fluids: 0,
                blocks: vec![0; 4096],
                biomes: vec![1; 64],
            })
            .collect();
        world
            .insert(Chunk {
                block_entities: vec![],
                x: -1,
                z: -1,
                sections,
            })
            .unwrap();
        assert_eq!(world.block(-1, -64, -1), Some(0));
        assert_eq!(world.block(-1, -65, -1), None);
        assert_eq!(
            world.chunks[&(-1, -1)].sections[0]
                .blocks
                .allocated_values(),
            1
        );
        world.set_block(-1, -64, -1, 42);
        assert_eq!(world.block(-1, -64, -1), Some(42));
        assert_eq!(world.block(-2, -64, -1), Some(0));
        assert_eq!(
            world.chunks[&(-1, -1)].sections[0]
                .blocks
                .allocated_values(),
            4096
        );
        let snapshot = world.clone();
        assert!(Arc::ptr_eq(
            &snapshot.chunks[&(-1, -1)],
            &world.chunks[&(-1, -1)]
        ));
        world.set_block(-1, -64, -1, 42);
        assert!(Arc::ptr_eq(
            &snapshot.chunks[&(-1, -1)],
            &world.chunks[&(-1, -1)]
        ));
        world.set_block(-1, -64, -1, 43);
        assert_eq!(snapshot.block(-1, -64, -1), Some(42));
        assert_eq!(world.block(-1, -64, -1), Some(43));
        let old = &snapshot.chunks[&(-1, -1)];
        let current = &world.chunks[&(-1, -1)];
        assert!(!Arc::ptr_eq(old, current));
        assert!(!Arc::ptr_eq(&old.sections[0], &current.sections[0]));
        assert!(Arc::ptr_eq(&old.sections[1], &current.sections[1]));
        world.forget(&(-1i64).to_be_bytes()).unwrap();
        assert_eq!(world.block(-1, -64, -1), None);
        assert_eq!(snapshot.block(-1, -64, -1), Some(42));
    }
    #[test]
    fn section_update_uses_signed_section_position_and_x_z_y_local_order() {
        let mut world = world();
        let sections = (0..24)
            .map(|_| Section {
                non_air: 0,
                fluids: 0,
                blocks: vec![0; 4096],
                biomes: vec![0; 64],
            })
            .collect();
        world
            .insert(Chunk {
                block_entities: vec![],
                x: -1,
                z: 2,
                sections,
            })
            .unwrap();
        let (section_x, section_y, section_z) = (-1i64, -4i64, 2i64);
        let section =
            (section_x & 0x3fffff) << 42 | (section_z & 0x3fffff) << 20 | (section_y & 0xfffff);
        let mut bytes = section.to_be_bytes().to_vec();
        crate::net::codec::varint(1, &mut bytes);
        crate::net::codec::varlong((42 << 12) | (15 << 8) | (3 << 4) | 2, &mut bytes);
        world.section_update(&bytes).unwrap();
        assert_eq!(world.block(-1, -62, 35), Some(42));
        assert_eq!(world.block(-2, -62, 35), Some(0));
    }
    #[test]
    fn wrong_height_is_rejected_before_replacing_a_column() {
        let mut world = world();
        assert!(world
            .insert(Chunk {
                block_entities: vec![],
                x: 0,
                z: 0,
                sections: Vec::new()
            })
            .is_err());
        assert!(world.chunks.is_empty());
    }
}
