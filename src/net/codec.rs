use anyhow::{bail, ensure, Context, Result};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::io::{Read, Write};
pub const MAX_PACKET: usize = 8 * 1024 * 1024;
pub fn varint(mut value: i32, out: &mut Vec<u8>) {
    loop {
        let b = (value as u32 & 0x7f) as u8;
        value = ((value as u32) >> 7) as i32;
        out.push(b | if value != 0 { 128 } else { 0 });
        if value == 0 {
            break;
        }
    }
}
pub fn read_varint<R: Read>(r: &mut R) -> Result<i32> {
    let mut value = 0u32;
    for i in 0..5 {
        let mut b = [0];
        r.read_exact(&mut b)?;
        if i == 4 && b[0] & 0xf0 != 0 {
            bail!("VarInt overflow")
        }
        value |= ((b[0] & 127) as u32) << (i * 7);
        if b[0] & 128 == 0 {
            return Ok(value as i32);
        }
    }
    bail!("VarInt too long")
}
pub fn varlong(mut value: i64, out: &mut Vec<u8>) {
    loop {
        let byte = (value as u64 & 127) as u8;
        value = ((value as u64) >> 7) as i64;
        out.push(byte | if value != 0 { 128 } else { 0 });
        if value == 0 {
            return;
        }
    }
}
pub fn read_varlong(input: &mut impl Read) -> Result<i64> {
    let mut value = 0u64;
    for index in 0..10 {
        let mut byte = [0];
        input.read_exact(&mut byte)?;
        ensure!(index != 9 || byte[0] & 0xfe == 0, "VarLong overflow");
        value |= ((byte[0] & 127) as u64) << (index * 7);
        if byte[0] & 128 == 0 {
            return Ok(value as i64);
        }
    }
    bail!("VarLong too long")
}
pub fn string(s: &str, out: &mut Vec<u8>) {
    varint(s.len() as i32, out);
    out.extend_from_slice(s.as_bytes());
}
pub fn read_string(r: &mut impl Read, max: usize) -> Result<String> {
    let len = read_varint(r)?;
    ensure!(len >= 0 && len as usize <= max * 4, "Invalid string length");
    let mut bytes = vec![0; len as usize];
    r.read_exact(&mut bytes)?;
    let s = String::from_utf8(bytes)?;
    ensure!(
        s.encode_utf16().count() <= max,
        "String exceeds character limit"
    );
    Ok(s)
}
pub struct Wire<S> {
    pub stream: S,
    pub compression: Option<usize>,
    pub journal: Option<super::journal::Journal>,
}
impl<S: Read + Write> Wire<S> {
    pub fn set_phase(&self, phase: &'static str) {
        if let Some(j) = &self.journal {
            j.phase(phase);
        }
    }
    pub fn send(&mut self, id: i32, body: &[u8]) -> Result<()> {
        ensure!(body.len() < MAX_PACKET, "Outgoing packet too large");
        let mut data = Vec::with_capacity(body.len() + 5);
        varint(id, &mut data);
        data.extend_from_slice(body);
        let mut frame = Vec::new();
        if let Some(threshold) = self.compression {
            if data.len() >= threshold {
                varint(data.len() as i32, &mut frame);
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                encoder.write_all(&data)?;
                frame.extend(encoder.finish()?);
            } else {
                varint(0, &mut frame);
                frame.extend(data);
            }
        } else {
            frame = data;
        }
        let mut length = Vec::new();
        varint(frame.len() as i32, &mut length);
        self.stream.write_all(&length)?;
        self.stream.write_all(&frame)?;
        self.stream.flush()?;
        if let Some(j) = &self.journal {
            j.record("send", id, body.len());
        }
        if std::env::var_os("OPTIMC_NET_TRACE").is_some() {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            eprintln!("send {millis} 0x{id:02x} ({} bytes)", body.len());
        }
        Ok(())
    }
    pub fn receive(&mut self) -> Result<(i32, Vec<u8>)> {
        let length = read_varint(&mut self.stream).context("Reading packet frame")?;
        ensure!(
            length > 0 && length as usize <= MAX_PACKET,
            "Invalid frame length {length}"
        );
        let mut frame = vec![0; length as usize];
        self.stream.read_exact(&mut frame)?;
        decode_frame(frame, self.compression)
    }
}
fn decode_frame(frame: Vec<u8>, compression: Option<usize>) -> Result<(i32, Vec<u8>)> {
    let data = if let Some(threshold) = compression {
        let mut input = frame.as_slice();
        let expected = read_varint(&mut input)?;
        ensure!(
            expected >= 0 && expected as usize <= MAX_PACKET,
            "Invalid decompressed length"
        );
        if expected == 0 {
            ensure!(
                input.len() < threshold,
                "Uncompressed packet exceeds threshold"
            );
            input.to_vec()
        } else {
            ensure!(
                expected as usize >= threshold,
                "Compressed packet below threshold"
            );
            let mut decoder = ZlibDecoder::new(input);
            let mut output = Vec::new();
            (&mut decoder)
                .take(expected as u64 + 1)
                .read_to_end(&mut output)?;
            ensure!(
                output.len() == expected as usize,
                "Decompressed size mismatch"
            );
            ensure!(
                decoder.total_in() as usize == input.len(),
                "Trailing compressed data"
            );
            output
        }
    } else {
        frame
    };
    let mut input = data.as_slice();
    let id = read_varint(&mut input)?;
    ensure!(id >= 0, "Negative packet ID");
    Ok((id, input.to_vec()))
}

/// Incremental TCP framing. A read timeout never discards a partial length or
/// body. The compression threshold is supplied at decode time, after handling
/// the preceding packet (which may negotiate compression).
#[derive(Default)]
pub struct FrameBuffer {
    bytes: Vec<u8>,
    consumed: usize,
}
impl FrameBuffer {
    pub fn append(&mut self, bytes: &[u8]) -> Result<()> {
        ensure!(
            self.bytes.len() - self.consumed + bytes.len() <= MAX_PACKET + 5 + 65536,
            "Incoming frame buffer exceeds limit"
        );
        if self.consumed > 0 {
            self.bytes.drain(..self.consumed);
            self.consumed = 0;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub fn pending_bytes(&self) -> usize {
        self.bytes.len() - self.consumed
    }
    pub fn next_packet(&mut self, compression: Option<usize>) -> Result<Option<(i32, Vec<u8>)>> {
        let remaining = &self.bytes[self.consumed..];
        let mut length = 0u32;
        for (index, &byte) in remaining.iter().take(5).enumerate() {
            ensure!(index != 4 || byte & 0xf0 == 0, "Frame VarInt overflow");
            length |= ((byte & 127) as u32) << (index * 7);
            if byte & 128 != 0 {
                continue;
            }
            ensure!(
                length > 0 && length as usize <= MAX_PACKET,
                "Invalid frame length {length}"
            );
            let prefix = index + 1;
            let end = prefix + length as usize;
            if remaining.len() < end {
                return Ok(None);
            }
            let packet = decode_frame(remaining[prefix..end].to_vec(), compression)?;
            self.consumed += end;
            if self.consumed == self.bytes.len() {
                self.bytes.clear();
                self.consumed = 0;
            }
            return Ok(Some(packet));
        }
        ensure!(remaining.len() < 5, "Frame VarInt too long");
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn signed_varlongs_and_overflow() {
        for value in [i64::MIN, i64::MAX, -1, 0, 300] {
            let mut bytes = Vec::new();
            super::varlong(value, &mut bytes);
            let mut input = bytes.as_slice();
            assert_eq!(super::read_varlong(&mut input).unwrap(), value);
            assert!(input.is_empty());
        }
        assert!(
            super::read_varlong(&mut &[255, 255, 255, 255, 255, 255, 255, 255, 255, 2][..])
                .is_err()
        );
    }

    use super::*;
    use std::io::Cursor;
    #[test]
    fn incremental_frames_preserve_every_split_and_compression_transition() {
        let mut wire = Wire {
            stream: Cursor::new(Vec::new()),
            compression: None,
            journal: None,
        };
        wire.send(3, &[32]).unwrap();
        wire.compression = Some(32);
        wire.send(2, &[19; 512]).unwrap();
        wire.send(4, &[9; 8]).unwrap();
        let bytes = wire.stream.into_inner();
        for split in 0..=bytes.len() {
            let mut buffer = FrameBuffer::default();
            buffer.append(&bytes[..split]).unwrap();
            let mut compression = None;
            let mut packets = Vec::new();
            for part in [&[][..], &bytes[split..]] {
                buffer.append(part).unwrap();
                while let Some(packet) = buffer.next_packet(compression).unwrap() {
                    if packet.0 == 3 {
                        compression = Some(32);
                    }
                    packets.push(packet);
                }
            }
            assert_eq!(
                packets,
                vec![(3, vec![32]), (2, vec![19; 512]), (4, vec![9; 8])]
            );
        }
        // A multi-byte length and payload arriving one byte at a time.
        let mut wire = Wire {
            stream: Cursor::new(Vec::new()),
            compression: None,
            journal: None,
        };
        wire.send(123, &[42; 512]).unwrap();
        let bytes = wire.stream.into_inner();
        let mut buffer = FrameBuffer::default();
        for (index, byte) in bytes.iter().enumerate() {
            buffer.append(&[*byte]).unwrap();
            let packet = buffer.next_packet(None).unwrap();
            if index + 1 == bytes.len() {
                assert_eq!(packet, Some((123, vec![42; 512])));
            } else {
                assert!(packet.is_none());
            }
        }
    }
    #[test]
    fn incremental_frames_reject_invalid_lengths_and_bound_buffer() {
        for bytes in [vec![0], vec![255; 5], vec![255, 255, 255, 255, 16]] {
            let mut buffer = FrameBuffer::default();
            buffer.append(&bytes).unwrap();
            assert!(buffer.next_packet(None).is_err());
        }
        let mut buffer = FrameBuffer::default();
        assert!(buffer.append(&vec![0; MAX_PACKET + 65542]).is_err());
    }
    #[test]
    fn signed_varints_roundtrip() {
        for n in [0, 1, 127, 128, 255, 2147483647, -1, i32::MIN] {
            let mut v = vec![];
            varint(n, &mut v);
            assert_eq!(read_varint(&mut v.as_slice()).unwrap(), n);
        }
    }
    #[test]
    fn rejects_overflow_and_truncation() {
        assert!(read_varint(&mut [0xff, 0xff, 0xff, 0xff, 0x10].as_slice()).is_err());
        assert!(read_varint(&mut [0x80].as_slice()).is_err());
    }
    #[test]
    fn frames_roundtrip_with_compression() {
        for compression in [None, Some(0), Some(16), Some(1024)] {
            let mut wire = Wire {
                stream: Cursor::new(Vec::new()),
                compression,
                journal: None,
            };
            wire.send(0x123, &[42; 512]).unwrap();
            wire.stream.set_position(0);
            assert_eq!(wire.receive().unwrap(), (0x123, vec![42; 512]));
        }
    }
    #[test]
    fn oversized_frame_rejected_before_allocation() {
        let mut bytes = vec![];
        varint(MAX_PACKET as i32 + 1, &mut bytes);
        assert!(Wire {
            stream: Cursor::new(bytes),
            compression: None,
            journal: None,
        }
        .receive()
        .is_err());
    }
}
