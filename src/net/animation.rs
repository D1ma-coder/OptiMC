//! ClientboundAnimatePacket. Only hand swings are consumed here; other actions
//! retain explicit unhandled diagnostics until their own renderers are present.
use super::codec::read_varint;
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, time::Instant};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub entity: i32,
    pub action: u8,
}
impl Packet {
    pub fn decode(mut input: &[u8]) -> Result<Self> {
        let entity = read_varint(&mut input)?;
        ensure!(input.len() == 1, "Invalid animate packet length");
        Ok(Self {
            entity,
            action: input[0],
        })
    }
    pub fn off_hand(self) -> Option<bool> {
        match self.action {
            0 => Some(false),
            3 => Some(true),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Swing {
    pub off_hand: bool,
    pub serial: u64,
    pub started: Instant,
    pub ticks: i32,
}
impl Swing {
    pub fn progress(self, now: Instant) -> f32 {
        (now.saturating_duration_since(self.started).as_secs_f32() * 20. / self.ticks.max(1) as f32)
            .clamp(0., 1.)
    }
}
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub swings: BTreeMap<i32, Swing>,
    pub revision: u64,
}
impl Store {
    pub fn start(&mut self, entity: i32, off_hand: bool, ticks: i32, now: Instant) -> Result<()> {
        if self
            .swings
            .get(&entity)
            .is_some_and(|s| s.progress(now) < 0.5)
        {
            return Ok(());
        }
        ensure!(
            self.swings.contains_key(&entity) || self.swings.len() < 16384,
            "Animation entity budget exceeded"
        );
        self.revision = self.revision.wrapping_add(1);
        self.swings.insert(
            entity,
            Swing {
                off_hand,
                serial: self.revision,
                started: now,
                ticks,
            },
        );
        Ok(())
    }
    pub fn active(&self, now: Instant) -> bool {
        self.swings.values().any(|s| s.progress(now) < 1.)
    }
}
/// Metadata indices exported from Mob/Avatar, which share index 15 but use
/// different serializers. An off-hand swing uses the opposite physical arm.
pub fn left_arm(metadata: &super::metadata::Store, entity: i32, off_hand: bool) -> bool {
    static INDICES: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    let indices = INDICES.get_or_init(|| {
        serde_json::from_str(include_str!("../../data/26.2/animation-metadata.json"))
            .expect("original animation indices")
    });
    let values = metadata.entities.get(&entity);
    let avatar = indices["DATA_PLAYER_MAIN_HAND"].as_u64().unwrap() as u8;
    let mob = indices["DATA_MOB_FLAGS_ID"].as_u64().unwrap() as u8;
    let left = match values.and_then(|v| v.get(&avatar)) {
        Some(super::metadata::Value::Encoded { serializer, bytes })
            if *serializer == super::maps::reference().serializers["HUMANOID_ARM"] =>
        {
            bytes.as_slice() == [0]
        }
        _ => {
            matches!(values.and_then(|v|v.get(&mob)),Some(super::metadata::Value::Byte(flags)) if flags&2!=0)
        }
    };
    left ^ off_hand
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn original_animate_stream_codec_and_metadata_indices_match() {
        use base64::Engine;
        use sha1::{Digest, Sha1};
        use std::io::Read;
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/animate-packets.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
        assert_eq!(
            format!("{:x}", Sha1::digest(&bytes)),
            reference["sha1"]["animate-packets.json"]
        );
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 24);
        for row in rows {
            let data = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            let p = Packet::decode(&data).unwrap();
            assert_eq!(p.entity, row["entity"].as_i64().unwrap() as i32);
            assert_eq!(p.action, row["action"].as_u64().unwrap() as u8);
        }
        let raw = include_bytes!("../../data/26.2/animation-metadata.json");
        assert_eq!(
            format!("{:x}", Sha1::digest(raw)),
            reference["sha1"]["animation-metadata.json"]
        );
        let indices: serde_json::Value = serde_json::from_slice(raw).unwrap();
        let mut metadata = super::super::metadata::Store::default();
        let index = indices["DATA_MOB_FLAGS_ID"].as_u64().unwrap() as u8;
        metadata.entities.insert(
            1,
            std::sync::Arc::new(
                [(index, super::super::metadata::Value::Byte(2))]
                    .into_iter()
                    .collect(),
            ),
        );
        assert!(left_arm(&metadata, 1, false));
        assert!(!left_arm(&metadata, 1, true));
    }
    #[test]
    fn hand_actions_are_distinct_and_restart_only_after_half_duration() {
        let now = Instant::now();
        let mut store = Store::default();
        assert_eq!(
            Packet::decode(&[0xac, 2, 0]).unwrap().off_hand(),
            Some(false)
        );
        assert_eq!(
            Packet::decode(&[0xac, 2, 3]).unwrap().off_hand(),
            Some(true)
        );
        for action in [1, 2, 4, 5, 255] {
            assert_eq!(Packet::decode(&[1, action]).unwrap().off_hand(), None);
        }
        assert!(Packet::decode(&[1]).is_err());
        assert!(Packet::decode(&[1, 0, 0]).is_err());
        store.start(300, false, 6, now).unwrap();
        store
            .start(300, true, 6, now + Duration::from_millis(149))
            .unwrap();
        assert!(!store.swings[&300].off_hand);
        store
            .start(300, true, 6, now + Duration::from_millis(150))
            .unwrap();
        assert!(store.swings[&300].off_hand);
        assert!((store.swings[&300].progress(now + Duration::from_millis(300)) - 0.5).abs() < 1e-6);
        assert!(!store.active(now + Duration::from_millis(450)));
    }
}
