//! PacketBundlePacker / BundlerInfo semantics from the official 26.2 client.
//! Incoming subpackets become visible only when the closing delimiter arrives.
use anyhow::{ensure, Result};

type Packet = (i32, Vec<u8>);
const MAX_PACKETS: usize = 4096;
// An additional allocation bound for hostile/incomplete bundles.
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Buffer {
    open: bool,
    packets: Vec<Packet>,
    bytes: usize,
}
impl Buffer {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Called for delimiters and while a bundle is open. None means incomplete.
    pub fn receive(&mut self, id: i32, data: &[u8]) -> Result<Option<Vec<Packet>>> {
        if id == 0 {
            ensure!(data.is_empty(), "Invalid bundle delimiter");
            self.open = !self.open;
            if self.open {
                return Ok(None);
            }
            self.bytes = 0;
            return Ok(Some(std::mem::take(&mut self.packets)));
        }
        ensure!(self.open, "Subpacket outside bundle");
        // StartConfiguration is the terminal packet in the Play protocol.
        ensure!(id != 0x76, "Terminal message received in bundle");
        ensure!(
            self.packets.len() < MAX_PACKETS,
            "Bundle exceeds 4096 packets"
        );
        ensure!(
            data.len() <= MAX_BYTES - self.bytes,
            "Bundle exceeds 64 MiB"
        );
        self.bytes += data.len();
        self.packets.push((id, data.to_vec()));
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_waits_for_close_and_preserves_order() {
        let mut buffer = Buffer::default();
        assert!(buffer.receive(0, &[]).unwrap().is_none());
        assert!(buffer.receive(0x69, &[4]).unwrap().is_none());
        assert!(buffer.receive(0x69, &[2]).unwrap().is_none());
        assert!(buffer.is_open());
        assert_eq!(
            buffer.receive(0, &[]).unwrap().unwrap(),
            [(0x69, vec![4]), (0x69, vec![2])]
        );
        assert!(!buffer.is_open());
        buffer.receive(0, &[]).unwrap();
        assert!(buffer.receive(0, &[]).unwrap().unwrap().is_empty());
    }

    #[test]
    fn original_count_boundary_and_terminal_rejection() {
        let mut buffer = Buffer::default();
        buffer.receive(0, &[]).unwrap();
        assert!(buffer.receive(0x76, &[]).is_err());
        assert!(buffer.receive(0, &[1]).is_err());
        for _ in 0..MAX_PACKETS {
            buffer.receive(0x69, &[1]).unwrap();
        }
        assert!(buffer.receive(0x69, &[1]).is_err());
        assert_eq!(buffer.receive(0, &[]).unwrap().unwrap().len(), MAX_PACKETS);
    }

    #[test]
    fn byte_limit_is_checked_before_allocation() {
        let mut buffer = Buffer {
            open: true,
            bytes: MAX_BYTES,
            ..Default::default()
        };
        assert!(buffer.receive(1, &[1]).is_err());
        assert!(buffer.packets.is_empty());
    }
}
