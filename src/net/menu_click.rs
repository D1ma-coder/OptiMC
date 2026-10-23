//! ContainerInput wire actions. Server rules remain authoritative for every slot.
use super::codec::varint;
use anyhow::{ensure, Result};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub slot: i16,
    pub button: u8,
    /// PICKUP, QUICK_MOVE, SWAP, CLONE, THROW, QUICK_CRAFT, PICKUP_ALL.
    pub kind: u8,
}
impl Click {
    pub fn new(slot: i16, button: u8, kind: u8) -> Self {
        Self { slot, button, kind }
    }
    pub fn validate(self) -> Result<()> {
        ensure!(self.slot >= 0 || self.slot == -999, "Invalid clicked slot");
        let valid = match self.kind {
            0 | 1 | 4 | 6 => self.button <= 1,
            2 => self.button < 9 || self.button == 40,
            3 => self.button == 2,
            5 => self.button <= 10 && self.button & 3 <= 2,
            _ => false,
        };
        ensure!(valid, "Invalid container input/button");
        Ok(())
    }
    pub fn encode(self, id: i32, state: i32) -> Result<Vec<u8>> {
        self.validate()?;
        ensure!(id >= 0 && state >= 0, "Invalid container state");
        let mut out = Vec::new();
        varint(id, &mut out);
        varint(state, &mut out);
        out.extend(self.slot.to_be_bytes());
        out.push(self.button);
        varint(self.kind as i32, &mut out);
        // No client slot prediction is reported. The original server's
        // broadcastChanges reconciles slots and carried after clicked().
        // These are the exact empty map and HashedStack.EMPTY wire values,
        // not fabricated hashes for a stack whose components we cannot hash.
        out.extend([0, 0]);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    #[test]
    fn every_original_container_input_and_button_matches_the_jar_codec() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/container-clicks.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        for row in r["packets"].as_array().unwrap() {
            let click = Click::new(
                row["slot"].as_i64().unwrap() as i16,
                row["button"].as_u64().unwrap() as u8,
                row["kind"].as_u64().unwrap() as u8,
            );
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            assert_eq!(click.encode(300, 8).unwrap(), body, "{row}");
        }
        for click in [
            Click::new(-1, 0, 0),
            Click::new(0, 3, 0),
            Click::new(0, 41, 2),
            Click::new(0, 3, 5),
            Click::new(0, 7, 5),
            Click::new(0, 0, 7),
        ] {
            assert!(click.validate().is_err());
        }
    }
}
