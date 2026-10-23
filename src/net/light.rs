//! Server-owned nibble light arrays. This stores network updates, not a local
//! light propagation engine; renderer lightmap consumers are separate.
use super::codec::read_varint;
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layer {
    Empty,
    Data(Arc<[u8; 2048]>),
}
impl Layer {
    pub fn get(&self, x: u8, y: u8, z: u8) -> u8 {
        let i = (usize::from(y & 15) << 8) | (usize::from(z & 15) << 4) | usize::from(x & 15);
        match self {
            Self::Empty => 0,
            Self::Data(bytes) => (bytes[i / 2] >> ((i & 1) * 4)) & 15,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Data {
    masks: [Vec<u64>; 4],
    updates: [Vec<Layer>; 2],
}
fn mask(input: &mut &[u8]) -> Result<Vec<u64>> {
    let count = read_varint(input)?;
    // FriendlyByteBuf packet bound; validate before allocating.
    ensure!(
        count >= 0 && count as usize <= input.len() / 8,
        "Invalid light mask length"
    );
    let mut words = Vec::with_capacity(count as usize);
    for _ in 0..count {
        words.push(u64::from_be_bytes(input[..8].try_into()?));
        *input = &input[8..];
    }
    Ok(words)
}
fn layers(input: &mut &[u8]) -> Result<Vec<Layer>> {
    let count = read_varint(input)?;
    ensure!(
        count >= 0 && count as usize <= input.len() / 2050,
        "Invalid light layer count"
    );
    let mut result = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let len = read_varint(input)?;
        // DataLayer(byte[]) requires exactly 2048, even though the wire byte-array
        // codec admits shorter arrays before the listener constructs DataLayer.
        ensure!(
            len == 2048 && input.len() >= 2048,
            "Invalid light nibble array length"
        );
        let bytes: [u8; 2048] = input[..2048].try_into()?;
        *input = &input[2048..];
        result.push(if bytes.iter().all(|&b| b == 0) {
            Layer::Empty
        } else {
            Layer::Data(Arc::new(bytes))
        });
    }
    Ok(result)
}
impl Data {
    pub fn decode(mut input: &[u8]) -> Result<Self> {
        let masks = [
            mask(&mut input)?,
            mask(&mut input)?,
            mask(&mut input)?,
            mask(&mut input)?,
        ];
        let updates = [layers(&mut input)?, layers(&mut input)?];
        ensure!(input.is_empty(), "Trailing light update bytes");
        Ok(Self { masks, updates })
    }
    pub fn packet(mut input: &[u8]) -> Result<([i32; 2], Self)> {
        let column = [read_varint(&mut input)?, read_varint(&mut input)?];
        Ok((column, Self::decode(input)?))
    }
    fn bit(&self, mask: usize, bit: usize) -> bool {
        self.masks[mask]
            .get(bit / 64)
            .is_some_and(|word| word & (1u64 << (bit & 63)) != 0)
    }
}
#[derive(Clone, Debug, Default)]
pub struct Section {
    pub sky: Option<Layer>,
    pub block: Option<Layer>,
}
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub columns: BTreeMap<[i32; 2], Arc<BTreeMap<i32, Arc<Section>>>>,
    pub section_count: usize,
    pub revision: u64,
}
impl Store {
    pub fn apply(&mut self, column: [i32; 2], min_y: i32, height: u32, data: Data) -> Result<()> {
        let min_section = min_y.div_euclid(16) - 1;
        let count = height as usize / 16 + 2;
        // Match readSectionList: ascending masks, data wins over an overlapping
        // empty bit, outside-height bits and surplus list entries are ignored.
        let mut changes = BTreeMap::<i32, Section>::new();
        for layer in 0..2 {
            let mut updates = data.updates[layer].iter();
            for bit in 0..count {
                let value = if data.bit(layer, bit) {
                    Some(
                        updates
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Missing light data for section mask"))?
                            .clone(),
                    )
                } else if data.bit(layer + 2, bit) {
                    Some(Layer::Empty)
                } else {
                    None
                };
                if let Some(value) = value {
                    let key = min_section + bit as i32;
                    let section = changes.entry(key).or_insert_with(|| {
                        self.columns
                            .get(&column)
                            .and_then(|c| c.get(&key))
                            .map_or_else(Section::default, |v| v.as_ref().clone())
                    });
                    if layer == 0 {
                        section.sky = Some(value);
                    } else {
                        section.block = Some(value);
                    }
                }
            }
        }
        if !changes.is_empty() {
            let current = Arc::make_mut(self.columns.entry(column).or_default());
            for (key, section) in changes {
                if current.insert(key, Arc::new(section)).is_none() {
                    self.section_count += 1;
                }
            }
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }
    pub fn sample(&self, position: [i32; 3]) -> [Option<u8>; 2] {
        let key = position.map(|v| v.div_euclid(16));
        self.columns
            .get(&[key[0], key[2]])
            .and_then(|column| column.get(&key[1]))
            .map_or([None, None], |s| {
                let [x, y, z] = position.map(|v| v.rem_euclid(16) as u8);
                [
                    s.sky.as_ref().map(|v| v.get(x, y, z)),
                    s.block.as_ref().map(|v| v.get(x, y, z)),
                ]
            })
    }
    pub fn forget(&mut self, x: i32, z: i32) {
        if let Some(column) = self.columns.remove(&[x, z]) {
            self.section_count -= column.len();
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

#[cfg(test)]
pub(crate) fn reference() -> serde_json::Value {
    use sha1::{Digest, Sha1};
    use std::io::Read;
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(
        include_bytes!("../../data/26.2/light-updates.json.gz").as_slice(),
    )
    .read_to_end(&mut raw)
    .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha1::digest(&raw)),
        manifest["sha1"]["light-updates.json"]
    );
    serde_json::from_slice(&raw).unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    fn packet(row: &serde_json::Value) -> ([i32; 2], Data) {
        Data::packet(
            &base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn original_nibbles_masks_padding_and_partial_updates_are_preserved() {
        let r = reference();
        let (column, data) = packet(&r["packets"][0]);
        assert_eq!(column, [-1, -2]);
        let mut store = Store::default();
        store.apply(column, -64, 384, data).unwrap();
        assert_eq!(store.section_count, 5); // bit 70 is outside the dimension
        for (i, expected) in r["nibbles"].as_array().unwrap().iter().enumerate() {
            let [x, y, z] = [(i & 15) as i32, (i >> 8) as i32, ((i >> 4) & 15) as i32];
            assert_eq!(
                store.sample([-16 + x, -80 + y, -32 + z]),
                [Some(expected.as_u64().unwrap() as u8), Some(0)]
            );
            assert_eq!(
                store.sample([-16 + x, -64 + y, -32 + z]),
                [Some(15), Some(15 - expected.as_u64().unwrap() as u8)]
            );
        }
        assert_eq!(store.sample([-16, 320, -32])[0], Some(7));
        assert_eq!(store.sample([-16, 0, -32]), [None, None]);
        let before = store.clone();
        let (column, data) = packet(&r["packets"][1]);
        store.apply(column, -64, 384, data).unwrap();
        assert_eq!(store.sample([-16, -64, -32]), [Some(0), Some(15)]);
        assert_eq!(store.sample([-16, -48, -32]), [Some(0), Some(9)]);
        assert_eq!(before.sample([-16, -48, -32]), [Some(0), Some(4)]);
        assert!(Arc::ptr_eq(
            &before.columns[&[-1, -2]][&-5],
            &store.columns[&[-1, -2]][&-5]
        ));
        store.forget(-1, -2);
        assert!(store.columns.is_empty());
        assert!(!before.columns.is_empty());
    }
    #[test]
    fn malformed_updates_never_publish_partial_section_data() {
        let r = reference();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(r["packets"][0]["body"].as_str().unwrap())
            .unwrap();
        for end in 0..raw.len() {
            assert!(Data::packet(&raw[..end]).is_err(), "Truncated at {end}");
        }
        let mut trailing = raw.clone();
        trailing.push(0);
        assert!(Data::packet(&trailing).is_err());
        let (column, mut data) = Data::packet(&raw).unwrap();
        data.updates[1].clear();
        let mut store = Store::default();
        assert!(store.apply(column, -64, 384, data).is_err());
        assert!(store.columns.is_empty());
        assert_eq!(store.revision, 0);
    }
    #[test]
    fn snapshots_share_unchanged_columns_and_release_only_unloaded_light() {
        let r = reference();
        let (column, data) = packet(&r["packets"][0]);
        let mut store = Store::default();
        store.apply(column, -64, 384, data.clone()).unwrap();
        store.apply([3, 4], -64, 384, data).unwrap();
        let before = store.clone();
        let (_, data) = packet(&r["packets"][1]);
        store.apply(column, -64, 384, data).unwrap();
        assert!(!Arc::ptr_eq(
            &store.columns[&column],
            &before.columns[&column]
        ));
        assert!(Arc::ptr_eq(
            &store.columns[&[3, 4]],
            &before.columns[&[3, 4]]
        ));
        assert_eq!(store.section_count, 10);
        store.forget(column[0], column[1]);
        assert_eq!(store.section_count, 5);
        store.forget(column[0], column[1]);
        assert_eq!(store.section_count, 5);
        assert_eq!(before.section_count, 10);
    }
    #[test]
    fn weather_setters_match_original_float_bits() {
        let r = reference();
        let mut state = super::super::environment::State::default();
        for row in r["weather"].as_array().unwrap() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            assert!(state.weather_event(&body).unwrap());
            assert_eq!(
                state.rain_level.to_bits(),
                row["rainBits"].as_i64().unwrap() as u32
            );
            assert_eq!(
                state.thunder_level.to_bits(),
                row["thunderBits"].as_i64().unwrap() as u32
            );
        }
    }
}
