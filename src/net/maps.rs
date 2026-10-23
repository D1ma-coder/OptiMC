//! Authoritative 128×128 map pixels and decorations, using the 26.2 codecs.
use super::{codec::read_varint, component};
use crate::text::RichText;
use anyhow::{ensure, Result};
use std::{
    collections::BTreeMap,
    io::Read,
    sync::{Arc, OnceLock},
};

#[derive(serde::Deserialize)]
pub(crate) struct Reference {
    pub palette: Vec<i64>,
    pub serializers: BTreeMap<String, i32>,
    pub item_index: u8,
    pub rotation_index: u8,
    pub flags_index: u8,
}
pub(crate) fn reference() -> &'static Reference {
    static DATA: OnceLock<Reference> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../data/26.2/maps-reference.json.gz")[..])
            .read_to_end(&mut bytes)
            .expect("embedded maps reference");
        serde_json::from_slice(&bytes).expect("embedded maps reference JSON")
    })
}
pub fn rgba(color: u8) -> [u8; 4] {
    let color = reference().palette[color as usize] as u32;
    [
        (color >> 16) as u8,
        (color >> 8) as u8,
        color as u8,
        (color >> 24) as u8,
    ]
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoration {
    pub kind: u32,
    pub x: i8,
    pub y: i8,
    pub rotation: u8,
    pub name: Option<RichText>,
}
#[derive(Clone, Debug)]
pub struct Map {
    pub scale: u8,
    pub locked: bool,
    pub colors: Arc<[u8; 16384]>,
    pub decorations: Vec<Decoration>,
    pub revision: u64,
}
impl Default for Map {
    fn default() -> Self {
        Self {
            scale: 0,
            locked: false,
            colors: {
                static EMPTY: OnceLock<Arc<[u8; 16384]>> = OnceLock::new();
                EMPTY.get_or_init(|| Arc::new([0; 16384])).clone()
            },
            decorations: Vec::new(),
            revision: 0,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub maps: BTreeMap<i32, Arc<Map>>,
    pub revision: u64,
}
pub(crate) fn byte(input: &mut &[u8]) -> Result<u8> {
    let (&b, tail) = input
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("Truncated map/entity byte"))?;
    *input = tail;
    Ok(b)
}
pub(crate) fn bytes<'a>(input: &mut &'a [u8], len: usize) -> Result<&'a [u8]> {
    ensure!(input.len() >= len, "Truncated map/entity bytes");
    let (value, tail) = input.split_at(len);
    *input = tail;
    Ok(value)
}
struct Patch {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    colors: Vec<u8>,
}
struct Update {
    id: i32,
    scale: u8,
    locked: bool,
    decorations: Option<Vec<Decoration>>,
    patch: Option<Patch>,
}
impl Update {
    fn decode(mut data: &[u8]) -> Result<Self> {
        let id = read_varint(&mut data)?;
        let scale = byte(&mut data)?;
        let locked = byte(&mut data)? != 0;
        let decorations = if byte(&mut data)? != 0 {
            let count = read_varint(&mut data)?;
            ensure!((0..=1024).contains(&count), "Too many map decorations");
            let mut icons = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let kind = read_varint(&mut data)?;
                ensure!(kind >= 0, "Invalid map decoration type");
                let x = byte(&mut data)? as i8;
                let y = byte(&mut data)? as i8;
                let rotation = byte(&mut data)?;
                let name = if byte(&mut data)? != 0 {
                    Some(component::read_rich(&mut data)?)
                } else {
                    None
                };
                icons.push(Decoration {
                    kind: kind as u32,
                    x,
                    y,
                    rotation,
                    name,
                });
            }
            Some(icons)
        } else {
            None
        };
        let width = byte(&mut data)? as usize;
        let patch = if width > 0 {
            let height = byte(&mut data)? as usize;
            let x = byte(&mut data)? as usize;
            let y = byte(&mut data)? as usize;
            ensure!(
                height > 0 && x + width <= 128 && y + height <= 128,
                "Map patch outside 128x128"
            );
            let len = read_varint(&mut data)?;
            ensure!(
                len >= 0 && len as usize == width * height,
                "Map patch color count mismatch"
            );
            Some(Patch {
                x,
                y,
                width,
                height,
                colors: bytes(&mut data, len as usize)?.to_vec(),
            })
        } else {
            None
        };
        ensure!(data.is_empty(), "Trailing map packet bytes");
        Ok(Self {
            id,
            scale,
            locked,
            decorations,
            patch,
        })
    }
}
impl Store {
    pub fn packet(&mut self, data: &[u8]) -> Result<()> {
        let update = Update::decode(data)?;
        // ClientLevel.overrideMapData has no 256-map cutoff. Maps are world
        // state: dropping one would corrupt subsequent partial patches. Retain
        // every ID for this level and release the store at login/reconfiguration.

        let map = Arc::make_mut(self.maps.entry(update.id).or_default());
        map.scale = update.scale;
        map.locked = update.locked;
        if let Some(decorations) = update.decorations {
            map.decorations = decorations;
        }
        if let Some(patch) = update.patch {
            let colors = Arc::make_mut(&mut map.colors);
            for row in 0..patch.height {
                colors[(patch.y + row) * 128 + patch.x
                    ..(patch.y + row) * 128 + patch.x + patch.width]
                    .copy_from_slice(&patch.colors[row * patch.width..(row + 1) * patch.width]);
            }
        }
        self.revision = self.revision.wrapping_add(1);
        map.revision = self.revision;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn packets() -> Vec<(String, i32, Vec<u8>)> {
    use base64::Engine;
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(&include_bytes!("../../data/26.2/maps-reference.json.gz")[..])
        .read_to_end(&mut json)
        .unwrap();
    use sha1::{Digest, Sha1};
    let reference: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../data/26.2/reference.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha1::digest(&json)),
        reference["sha1"]["maps-reference.json"].as_str().unwrap()
    );
    let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
    value["packets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["label"].as_str().unwrap().into(),
                p["id"].as_i64().unwrap() as i32,
                base64::engine::general_purpose::STANDARD
                    .decode(p["body"].as_str().unwrap())
                    .unwrap(),
            )
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn more_than_256_maps_keep_partial_pixels_and_snapshot_isolation() {
        let mut store = Store::default();
        for id in 0..1024 {
            let mut packet = Vec::new();
            super::super::codec::varint(id, &mut packet);
            packet.extend([0, 0, 0, 1, 1, 0, 0, 1, 4]); // one pixel at (0,0)
            store.packet(&packet).unwrap();
        }
        assert_eq!(store.maps.len(), 1024);
        let previous = store.clone();
        for id in [0, 255, 256, 1023] {
            let mut packet = Vec::new();
            super::super::codec::varint(id, &mut packet);
            packet.extend([0, 0, 0, 1, 1, 1, 0, 1, 5]); // preserve old pixel; patch (1,0)
            store.packet(&packet).unwrap();
            assert_eq!(&store.maps[&id].colors[..2], &[4, 5]);
            assert_eq!(&previous.maps[&id].colors[..2], &[4, 0]);
        }
        let empty = Store::default();
        assert!(empty.maps.is_empty());
        let a = Map::default();
        let b = Map::default();
        assert!(Arc::ptr_eq(&a.colors, &b.colors));
    }
    #[test]
    fn jar_map_codecs_apply_partial_updates_and_distinguish_unchanged_from_empty_icons() {
        let mut store = Store::default();
        let packets = packets();
        let full = packets.iter().find(|p| p.0 == "map").unwrap();
        store.packet(&full.2).unwrap();
        let old = store.maps[&42].clone();
        assert_eq!(old.scale, 2);
        assert!(!old.locked);
        assert_eq!(old.decorations.len(), 1);
        assert_eq!(
            old.decorations[0].name.as_ref().unwrap().plain_text(),
            "Marker"
        );
        let partial = packets.iter().find(|p| p.0 == "patch").unwrap();
        store.packet(&partial.2).unwrap();
        let new = &store.maps[&42];
        assert!(new.locked);
        assert_eq!(new.colors[7 * 128 + 3..7 * 128 + 5], [4, 5]);
        assert_eq!(new.colors[8 * 128 + 3..8 * 128 + 5], [6, 7]);
        assert_eq!(new.colors[0], old.colors[0]);
        assert_eq!(new.decorations, old.decorations);
        assert!(!Arc::ptr_eq(&old.colors, &new.colors));
        let clear = packets.iter().find(|p| p.0 == "icons_clear").unwrap();
        store.packet(&clear.2).unwrap();
        assert!(store.maps[&42].decorations.is_empty());
    }
    #[test]
    fn every_truncation_is_rejected_before_mutating_a_map() {
        let mut store = Store::default();
        for (_, id, body) in packets().into_iter().filter(|p| p.1 == 0x33) {
            assert_eq!(id, 0x33);
            for end in 0..body.len() {
                let revision = store.revision;
                assert!(store.packet(&body[..end]).is_err());
                assert_eq!(store.revision, revision);
            }
            store.packet(&body).unwrap();
        }
    }
    #[test]
    fn official_palette_is_argb_converted_to_rgba_without_channel_swaps() {
        assert_eq!(rgba(0), [0, 0, 0, 0]);
        assert_eq!(rgba(4), [89, 125, 39, 255]);
        assert_eq!(reference().palette.len(), 256);
    }
}
