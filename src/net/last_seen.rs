//! Original 20-entry chat acknowledgement window, including checksum and offset.
use super::codec::varint;
pub type Signature = [u8; 256];
#[derive(Clone)]
struct Entry {
    signature: Signature,
    pending: bool,
}
#[derive(Clone, Default)]
pub struct Tracker {
    entries: [Option<Entry>; 20],
    tail: usize,
    pub offset: i32,
    last: Option<Signature>,
}
impl Tracker {
    pub fn add(&mut self, signature: Option<Signature>, shown: bool) -> bool {
        if self.last == signature {
            return false;
        }
        self.last = signature;
        self.entries[self.tail] = if shown {
            signature.map(|signature| Entry {
                signature,
                pending: true,
            })
        } else {
            None
        };
        self.tail = (self.tail + 1) % 20;
        self.offset = self.offset.wrapping_add(1);
        true
    }
    pub fn ignore(&mut self, signature: &Signature) {
        for entry in &mut self.entries {
            if entry
                .as_ref()
                .is_some_and(|e| e.pending && &e.signature == signature)
            {
                *entry = None;
                break;
            }
        }
    }
    pub fn take_offset(&mut self) -> i32 {
        std::mem::take(&mut self.offset)
    }
    pub fn update(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        varint(self.take_offset(), &mut out);
        let mut bits = [0; 3];
        let mut checksum = 1i32;
        for i in 0..20 {
            if let Some(entry) = &mut self.entries[(self.tail + i) % 20] {
                bits[i / 8] |= 1 << (i % 8);
                entry.pending = false;
                let hash = entry.signature.iter().fold(1i32, |hash, b| {
                    hash.wrapping_mul(31).wrapping_add(*b as i8 as i32)
                });
                checksum = checksum.wrapping_mul(31).wrapping_add(hash);
            }
        }
        out.extend(bits);
        let checksum = checksum as u8;
        out.push(if checksum == 0 { 1 } else { checksum });
        out
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    pub(crate) fn reference() -> serde_json::Value {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../data/26.2/chat-reference.json.gz")[..])
            .read_to_end(&mut bytes)
            .unwrap();
        use sha1::{Digest, Sha1};
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
        assert_eq!(
            format!("{:x}", Sha1::digest(&bytes)),
            manifest["sha1"]["chat-reference.json"].as_str().unwrap()
        );
        serde_json::from_slice(&bytes).unwrap()
    }
    #[test]
    fn acknowledgement_ring_matches_original_tracker_byte_for_byte() {
        let reference = reference();
        let mut tracker = Tracker::default();
        let mut result = 0;
        assert_eq!(tracker.update(), vec![0, 0, 0, 0, 1]);
        for i in 0..30u8 {
            let signature = [i.wrapping_mul(11); 256];
            tracker.add(Some(signature), i % 3 != 0);
            if i % 7 == 0 {
                tracker.ignore(&signature);
            }
            if i % 5 == 4 {
                let expected = base64::engine::general_purpose::STANDARD
                    .decode(reference["last_seen_updates"][result].as_str().unwrap())
                    .unwrap();
                assert_eq!(tracker.update(), expected);
                result += 1;
            }
        }
    }
}
