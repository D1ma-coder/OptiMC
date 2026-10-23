//! Official 26.2 attributes, ordered modifiers and server-owned entity snapshots.
use super::{codec::read_varint, registry::identifier};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    sync::OnceLock,
};
#[derive(Deserialize)]
pub struct Definition {
    pub id: u32,
    pub name: String,
    #[serde(rename = "default")]
    pub default_value: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub syncable: bool,
}
#[derive(Deserialize)]
struct Catalog {
    minecraft: String,
    protocol: i32,
    attributes: Vec<Definition>,
}
fn catalog() -> Result<&'static Catalog> {
    static DATA: OnceLock<Result<Catalog, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        (|| -> Result<Catalog> {
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(
                include_bytes!("../../data/26.2/attributes.json.gz").as_slice(),
            )
            .take(1024 * 1024)
            .read_to_end(&mut raw)?;
            let reference: serde_json::Value =
                serde_json::from_str(include_str!("../../data/26.2/reference.json"))?;
            ensure!(
                format!("{:x}", Sha1::digest(&raw)) == reference["sha1"]["attributes.json"],
                "Attribute reference hash mismatch"
            );
            let c: Catalog = serde_json::from_slice(&raw)?;
            ensure!(
                c.minecraft == "26.2" && c.protocol == 776,
                "Wrong attribute version"
            );
            let ids = crate::assets::builtins::get()?;
            for (i, a) in c.attributes.iter().enumerate() {
                ensure!(
                    a.id as usize == i
                        && ids.name("minecraft:attribute", a.id) == Some(a.name.as_str()),
                    "Attribute registry mismatch"
                );
            }
            Ok(c)
        })()
        .map_err(|e| format!("{e:#}"))
    })
    .as_ref()
    .map_err(|e| anyhow::anyhow!(e.clone()))
}
fn double(input: &mut &[u8]) -> Result<f64> {
    let mut b = [0; 8];
    input.read_exact(&mut b)?;
    let v = f64::from_be_bytes(b);
    ensure!(v.is_finite(), "Non-finite attribute");
    Ok(v)
}
#[derive(Clone, Debug)]
pub struct Modifier {
    pub id: String,
    pub amount: f64,
    pub operation: u8,
}
#[derive(Clone, Debug)]
pub struct Instance {
    pub base: f64,
    pub modifiers: Vec<Modifier>,
    pub value: f64,
}
fn evaluate(base: f64, modifiers: &[Modifier], definition: &Definition) -> f64 {
    evaluate_skipping(base, modifiers, definition, None)
}
fn evaluate_skipping(
    base: f64,
    modifiers: &[Modifier],
    definition: &Definition,
    skip: Option<&str>,
) -> f64 {
    let mut base = base;
    for m in modifiers
        .iter()
        .filter(|m| m.operation == 0 && Some(m.id.as_str()) != skip)
    {
        base += m.amount;
    }
    let mut value = base;
    for m in modifiers
        .iter()
        .filter(|m| m.operation == 1 && Some(m.id.as_str()) != skip)
    {
        value += base * m.amount;
    }
    for m in modifiers
        .iter()
        .filter(|m| m.operation == 2 && Some(m.id.as_str()) != skip)
    {
        value *= 1. + m.amount;
    }
    // RangedAttribute.sanitizeValue: NaN -> lower bound; overflow clamps to range.
    if value.is_nan() {
        return definition.min.unwrap_or(0.);
    }
    value.clamp(
        definition.min.unwrap_or(f64::MIN),
        definition.max.unwrap_or(f64::MAX),
    )
}
pub struct Update {
    pub entity: i32,
    pub values: BTreeMap<u32, Instance>,
}
impl Update {
    pub fn decode(mut data: &[u8]) -> Result<Self> {
        let entity = read_varint(&mut data)?;
        let count = read_varint(&mut data)?;
        let catalog = catalog()?;
        ensure!((0..=256).contains(&count), "Invalid attribute count");
        let mut values = BTreeMap::new();
        for _ in 0..count {
            // Attribute.STREAM_CODEC is holderRegistry: direct registry ID, no +1/inline value.
            let id = read_varint(&mut data)?;
            let definition = catalog
                .attributes
                .get(id as usize)
                .context("Unknown attribute ID")?;
            let base = double(&mut data)?;
            let count = read_varint(&mut data)?;
            ensure!((0..=1024).contains(&count), "Too many attribute modifiers");
            let mut modifiers = Vec::new();
            let mut ids = BTreeSet::new();
            for _ in 0..count {
                let id = identifier(&mut data)?;
                ensure!(ids.insert(id.clone()), "Duplicate modifier ID");
                let amount = double(&mut data)?;
                let operation = read_varint(&mut data)?;
                ensure!((0..=2).contains(&operation), "Unknown attribute operation");
                modifiers.push(Modifier {
                    id,
                    amount,
                    operation: operation as u8,
                });
            }
            let value = evaluate(base, &modifiers, definition);
            values.insert(
                id as u32,
                Instance {
                    base,
                    modifiers,
                    value,
                },
            );
        }
        ensure!(data.is_empty(), "Trailing attribute packet bytes");
        Ok(Self { entity, values })
    }
}
#[derive(Clone, Default)]
pub struct Store {
    pub entities: BTreeMap<i32, BTreeMap<u32, Instance>>,
}
impl Store {
    pub fn apply(&mut self, update: Update) -> Result<()> {
        ensure!(
            self.entities.len() < 16384 || self.entities.contains_key(&update.entity),
            "Too many entity attribute maps"
        );
        self.entities
            .entry(update.entity)
            .or_default()
            .extend(update.values);
        Ok(())
    }
    pub fn value(&self, entity: i32, name: &str) -> Option<f64> {
        let id = catalog()
            .ok()?
            .attributes
            .iter()
            .find(|d| d.name == name)?
            .id;
        Some(self.entities.get(&entity)?.get(&id)?.value)
    }
    pub fn respawn(&mut self, entity: i32, keep_modifiers: bool) -> Result<()> {
        let catalog = catalog()?;
        if !keep_modifiers {
            if let Some(values) = self.entities.get_mut(&entity) {
                for (id, value) in values {
                    value.modifiers.clear();
                    value.value = evaluate(
                        value.base,
                        &value.modifiers,
                        &catalog.attributes[*id as usize],
                    );
                }
            }
        }
        Ok(())
    }
    fn local_speed(&self, entity: i32) -> Option<f64> {
        let definition = catalog()
            .ok()?
            .attributes
            .iter()
            .find(|d| d.name == "minecraft:movement_speed")?;
        let instance = self.entities.get(&entity)?.get(&definition.id)?;
        // LivingEntity.setSprinting owns this modifier locally. Do not multiply
        // an echoed server sprint modifier a second time in Player::tick.
        Some(evaluate_skipping(
            instance.base,
            &instance.modifiers,
            definition,
            Some("minecraft:sprinting"),
        ))
    }
    pub fn movement(&self, entity: i32) -> Movement {
        let v = |name: &str, default: f64| self.value(entity, name).unwrap_or(default);
        Movement {
            gravity: v("minecraft:gravity", 0.08),
            air_drag: v("minecraft:air_drag_modifier", 1.),
            friction: v("minecraft:friction_modifier", 1.),
            speed: self.local_speed(entity).unwrap_or(0.1),
            jump: v("minecraft:jump_strength", 0.42),
            step: v("minecraft:step_height", 0.6),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Movement {
    pub gravity: f64,
    pub air_drag: f64,
    pub friction: f64,
    pub speed: f64,
    pub jump: f64,
    pub step: f64,
}
impl Default for Movement {
    fn default() -> Self {
        Self {
            gravity: 0.08,
            air_drag: 1.,
            friction: 1.,
            speed: 0.1,
            jump: 0.42,
            step: 0.6,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn echoed_sprint_modifier_does_not_double_local_sprint_speed() {
        let mut store = Store::default();
        let mut values = BTreeMap::new();
        values.insert(
            26,
            Instance {
                base: 0.1,
                modifiers: vec![Modifier {
                    id: "minecraft:sprinting".into(),
                    amount: 0.3,
                    operation: 2,
                }],
                value: 0.13,
            },
        );
        store.apply(Update { entity: 42, values }).unwrap();
        assert_eq!(store.value(42, "minecraft:movement_speed"), Some(0.13));
        assert_eq!(store.movement(42).speed, 0.1);
    }
    #[test]
    fn respawn_preserves_bases_and_respects_keep_modifiers_flag() {
        let mut store = Store::default();
        store
            .apply(Update::decode(include_bytes!("../../data/26.2/attribute-packet.bin")).unwrap())
            .unwrap();
        store.respawn(42, true).unwrap();
        assert_eq!(store.movement(42).gravity, 0.30000000000000004);
        store.respawn(42, false).unwrap();
        assert_eq!(store.movement(42).gravity, 0.08);
    }
    #[test]
    fn original_jar_packet_and_modifier_value_match() {
        let data = include_bytes!("../../data/26.2/attribute-packet.bin");
        let update = Update::decode(data).unwrap();
        assert_eq!(update.entity, 42);
        let mut store = Store::default();
        store.apply(update).unwrap();
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("../../data/26.2/physics-golden.json")).unwrap();
        assert_eq!(
            store.value(42, "minecraft:gravity"),
            golden["modified_gravity"].as_f64()
        );
        assert_eq!(store.movement(42).gravity, 0.30000000000000004);
        for end in 0..data.len() {
            assert!(Update::decode(&data[..end]).is_err());
        }
        assert_eq!(catalog().unwrap().attributes.len(), 40);
        assert!(catalog()
            .unwrap()
            .attributes
            .iter()
            .any(|a| a.syncable && a.default_value == 0.08));
    }
    #[test]
    fn snapshots_replace_only_the_sent_attributes() {
        let mut store = Store::default();
        store
            .apply(Update::decode(include_bytes!("../../data/26.2/attribute-packet.bin")).unwrap())
            .unwrap();
        let mut update = Update {
            entity: 42,
            values: BTreeMap::new(),
        };
        update.values.insert(
            26,
            Instance {
                base: 0.2,
                modifiers: vec![],
                value: 0.2,
            },
        );
        store.apply(update).unwrap();
        assert_eq!(store.movement(42).speed, 0.2);
        assert_eq!(store.movement(42).gravity, 0.30000000000000004);
    }
}
