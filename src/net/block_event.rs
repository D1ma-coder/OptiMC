//! Server block events. Chest viewer counts drive a lid, not an inventory open.
use super::{codec::read_varint, spawn};
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, time::Instant};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub position: [i32; 3],
    pub event: u8,
    pub parameter: u8,
    pub block: u32,
}
impl Packet {
    pub fn decode(mut input: &[u8]) -> Result<Self> {
        ensure!(input.len() >= 11, "Truncated block event");
        let position = spawn::block_position(i64::from_be_bytes(input[..8].try_into()?));
        let event = input[8];
        let parameter = input[9];
        input = &input[10..];
        let block = read_varint(&mut input)?;
        ensure!(
            block >= 0 && input.is_empty(),
            "Invalid block event block/length"
        );
        ensure!(
            crate::assets::builtins::get()?
                .name("minecraft:block", block as u32)
                .is_some(),
            "Unknown block event block"
        );
        Ok(Self {
            position,
            event,
            parameter,
            block: block as u32,
        })
    }
}
pub fn chest(name: &str) -> bool {
    let name = name
        .trim_start_matches("minecraft:")
        .trim_start_matches("waxed_");
    matches!(
        name,
        "chest"
            | "trapped_chest"
            | "ender_chest"
            | "copper_chest"
            | "exposed_copper_chest"
            | "weathered_copper_chest"
            | "oxidized_copper_chest"
    )
}
#[derive(Clone, Copy, Debug)]
pub struct Lid {
    pub block: u32,
    pub open: bool,
    pub initial: f32,
    pub started: Instant,
}
impl Lid {
    pub fn openness(&self, now: Instant) -> f32 {
        let time = now.saturating_duration_since(self.started).as_secs_f32() * 20.;
        let count = (time.floor() as usize).min(11);
        let mut previous = self.initial;
        for _ in 0..count {
            previous = step(previous, self.open);
        }
        let next = step(previous, self.open);
        previous + (next - previous) * (time.fract())
    }
    pub fn active(&self, now: Instant) -> bool {
        let value = self.openness(now);
        if self.open {
            value < 1.
        } else {
            value > 0.
        }
    }
}
pub fn step(value: f32, open: bool) -> f32 {
    if open {
        (value + 0.1).min(1.)
    } else {
        (value - 0.1).max(0.)
    }
}
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub lids: BTreeMap<[i32; 3], Lid>,
    pub revision: u64,
}
impl Store {
    pub fn packet(
        &mut self,
        world: &crate::world::remote::RemoteWorld,
        data: &[u8],
    ) -> Result<bool> {
        let packet = Packet::decode(data)?;
        let ids = crate::assets::builtins::get()?;
        let name = ids.name("minecraft:block", packet.block).unwrap();
        if packet.event != 1 || !chest(name) {
            return Ok(false);
        }
        let current = world.block(packet.position[0], packet.position[1], packet.position[2]);
        if !current
            .and_then(|s| ids.block_states.get(s as usize))
            .is_some_and(|state| state.block.as_ref() == name)
        {
            return Ok(true);
        }
        let now = Instant::now();
        let open = packet.parameter > 0;
        if self
            .lids
            .get(&packet.position)
            .is_some_and(|lid| lid.block == packet.block && lid.open == open)
        {
            return Ok(true);
        }
        let initial = self
            .lids
            .get(&packet.position)
            .filter(|lid| lid.block == packet.block)
            .map_or(0., |lid| lid.openness(now));
        self.lids.insert(
            packet.position,
            Lid {
                block: packet.block,
                open,
                initial,
                started: now,
            },
        );
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }
    pub fn active(&self, now: Instant) -> bool {
        self.lids.values().any(|lid| lid.active(now))
    }
    pub fn forget_column(&mut self, x: i32, z: i32) {
        self.lids
            .retain(|p, _| p[0].div_euclid(16) != x || p[2].div_euclid(16) != z);
    }
    pub fn retain_world(&mut self, world: &crate::world::remote::RemoteWorld) {
        let Ok(ids) = crate::assets::builtins::get() else {
            return;
        };
        self.lids.retain(|p, lid| {
            world
                .block(p[0], p[1], p[2])
                .and_then(|s| ids.block_states.get(s as usize))
                .is_some_and(|s| Some(s.block.as_ref()) == ids.name("minecraft:block", lid.block))
        });
    }
}

#[cfg(test)]
pub(crate) fn reference() -> serde_json::Value {
    use sha1::{Digest, Sha1};
    use std::io::Read;
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(include_bytes!("../../data/26.2/chest-events.json.gz").as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha1::digest(&bytes)),
        manifest["sha1"]["chest-events.json"]
    );
    serde_json::from_slice(&bytes).unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn original_block_event_bytes_and_lid_steps_match_and_truncations_fail() {
        let r = reference();
        assert_eq!(r["packets"].as_array().unwrap().len(), 12);
        for row in r["packets"].as_array().unwrap() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            let p = Packet::decode(&bytes).unwrap();
            assert_eq!(p.position, [-31, -22, 47]);
            assert_eq!(p.event, 1);
            assert_eq!(p.parameter, row["viewers"].as_u64().unwrap() as u8);
            assert_eq!(p.block, row["block"].as_u64().unwrap() as u32);
            for end in 0..bytes.len() {
                assert!(Packet::decode(&bytes[..end]).is_err());
            }
        }
        for row in r["ticks"].as_array().unwrap() {
            let initial = row["initial"].as_f64().unwrap() as f32;
            let next = step(initial, row["open"].as_bool().unwrap());
            for (i, sample) in row["samples"].as_array().unwrap().iter().enumerate() {
                let value = initial + (next - initial) * (i as f32 / 4.);
                assert!((value - sample.as_f64().unwrap() as f32).abs() < 1e-6);
            }
        }
    }
}
