//! Minecraft 26.2 / protocol 776 chunk sections. Unlike older versions, each
//! section has both block and fluid counts and packed arrays have no length prefix.
use super::codec::{read_varint, MAX_PACKET};
use anyhow::{ensure, Context, Result};
use std::io::Read;

#[derive(Debug)]
pub struct Section {
    pub non_air: u16,
    pub fluids: u16,
    pub blocks: Vec<u32>,
    pub biomes: Vec<u32>,
}
#[derive(Debug)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub sections: Vec<Section>,
    pub block_entities: Vec<super::block_entity::BlockEntity>,
}
fn byte(input: &mut impl Read) -> Result<u8> {
    let mut b = [0];
    input.read_exact(&mut b)?;
    Ok(b[0])
}
fn short(input: &mut impl Read) -> Result<u16> {
    let mut b = [0; 2];
    input.read_exact(&mut b)?;
    Ok(u16::from_be_bytes(b))
}
fn int(input: &mut impl Read) -> Result<i32> {
    let mut b = [0; 4];
    input.read_exact(&mut b)?;
    Ok(i32::from_be_bytes(b))
}
/// Zero, indirect, and global palettes; values never straddle a 64-bit word.
pub fn palette(
    input: &mut impl Read,
    entries: usize,
    local_max: u8,
    min_bits: u8,
) -> Result<Vec<u32>> {
    let bits = byte(input)?;
    ensure!(bits <= 32, "Invalid palette width {bits}");
    if bits == 0 {
        let value = read_varint(input)?;
        ensure!(value >= 0, "Negative palette state");
        return Ok(vec![value as u32; entries]);
    }
    ensure!(bits >= min_bits, "Palette width below minimum");
    let mut values = vec![];
    if bits <= local_max {
        let count = read_varint(input)?;
        ensure!(
            count > 0 && count as usize <= (1usize << bits),
            "Invalid palette length"
        );
        for _ in 0..count {
            let id = read_varint(input)?;
            ensure!(id >= 0, "Negative palette state");
            values.push(id as u32);
        }
    }
    let per_word = 64 / bits as usize;
    let words = entries.div_ceil(per_word);
    let mask = (1u64 << bits) - 1;
    let mut result = Vec::with_capacity(entries);
    for _ in 0..words {
        let mut bytes = [0; 8];
        input.read_exact(&mut bytes)?;
        let word = u64::from_be_bytes(bytes);
        for i in 0..per_word {
            if result.len() == entries {
                break;
            }
            let value = ((word >> (i * bits as usize)) & mask) as u32;
            result.push(if values.is_empty() {
                value
            } else {
                *values
                    .get(value as usize)
                    .ok_or_else(|| anyhow::anyhow!("Palette index out of range"))?
            });
        }
    }
    Ok(result)
}
pub fn decode(data: &[u8]) -> Result<Chunk> {
    decode_sections(data, None).map(|(chunk, _)| chunk)
}
/// LevelChunk.replaceWithPacketData reads exactly the dimension's section array.
/// The byte-array length bounds storage, rather than defining vertical height.
pub fn decode_for_dimension(data: &[u8], section_count: usize) -> Result<Chunk> {
    ensure!(
        (1..=256).contains(&section_count),
        "Invalid dimension section count"
    );
    decode_sections(data, Some(section_count)).map(|(chunk, _)| chunk)
}
pub fn decode_with_light(data: &[u8], section_count: usize) -> Result<(Chunk, super::light::Data)> {
    ensure!(
        (1..=256).contains(&section_count),
        "Invalid dimension section count"
    );
    let (chunk, light) = decode_sections(data, Some(section_count))?;
    Ok((chunk, light.context("Chunk missing light payload")?))
}
fn decode_sections(
    data: &[u8],
    section_count: Option<usize>,
) -> Result<(Chunk, Option<super::light::Data>)> {
    ensure!(data.len() <= MAX_PACKET, "Chunk too large");
    let mut input = data;
    let x = int(&mut input)?;
    let z = int(&mut input)?;
    let heightmaps = read_varint(&mut input)?;
    ensure!((0..=6).contains(&heightmaps), "Invalid heightmap count");
    for _ in 0..heightmaps {
        let kind = read_varint(&mut input)?;
        ensure!((0..=5).contains(&kind), "Invalid heightmap type");
        let count = read_varint(&mut input)?;
        ensure!((0..=1024).contains(&count), "Invalid heightmap length");
        let skip = count as usize * 8;
        ensure!(input.len() >= skip, "Truncated heightmap");
        input = &input[skip..];
    }
    let len = read_varint(&mut input)?;
    ensure!(
        (0..=2 * 1024 * 1024).contains(&len) && input.len() >= len as usize,
        "Invalid section byte length"
    );
    let mut sections_input = &input[..len as usize];
    input = &input[len as usize..];
    let mut sections = Vec::new();
    while section_count.map_or(!sections_input.is_empty(), |count| sections.len() < count) {
        ensure!(sections.len() < 256, "Too many vertical sections");
        let non_air = short(&mut sections_input)?;
        let fluids = short(&mut sections_input)?;
        // LevelChunkSection.read stores both short counters verbatim, without
        // range validation. They are server hints, not lengths: palette decoding
        // remains bounded independently. Proxy-generated counts can be stale.

        let index = sections.len();
        let blocks = palette(&mut sections_input, 4096, 8, 4).with_context(|| {
            format!("Chunk {x},{z}, section {index} block palette (counts {non_air}/{fluids})")
        })?;
        let biomes = palette(&mut sections_input, 64, 3, 1)
            .with_context(|| format!("Chunk {x},{z}, section {index} biome palette"))?;
        sections.push(Section {
            non_air,
            fluids,
            blocks,
            biomes,
        });
    }
    let block_entities = super::block_entity::chunk(&mut input, x, z)?;
    let light = if input.is_empty() {
        None
    } else {
        Some(super::light::Data::decode(input)?)
    };
    Ok((
        Chunk {
            x,
            z,
            sections,
            block_entities,
        },
        light,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::codec::varint;
    #[test]
    fn occupancy_hints_do_not_reject_valid_palettes_or_relax_bounds() {
        for count in [4097u16, 32767, 32768, 65535] {
            let mut section = count.to_be_bytes().to_vec();
            section.extend(count.to_be_bytes());
            section.extend([0, 1, 0, 0]);
            let mut data = vec![0; 9]; // x, z, empty heightmap map
            varint(section.len() as i32, &mut data);
            data.extend(section);
            data.push(0); // block entity count
            let chunk = decode_for_dimension(&data, 1).unwrap();
            assert_eq!(chunk.sections[0].non_air, count);
            assert_eq!(chunk.sections[0].fluids, count);
            assert_eq!(chunk.sections[0].blocks, vec![1; 4096]);
            assert!(decode_for_dimension(&data, 2).is_err());
            data[14] = 33; // invalid palette width remains rejected
            assert!(decode_for_dimension(&data, 1).is_err());
        }
    }
    #[test]
    fn constant_palette_consumes_no_array_length() {
        let data = [0, 42, 99];
        let mut input = data.as_slice();
        assert_eq!(palette(&mut input, 64, 3, 1).unwrap(), vec![42; 64]);
        assert_eq!(input, [99]);
    }
    #[test]
    fn packed_palette_respects_word_padding() {
        let mut bytes = vec![5];
        varint(2, &mut bytes);
        varint(4, &mut bytes);
        varint(9, &mut bytes);
        // 12 values per word at 5 bits each, last 4 bits are unused.
        for _ in 0..2 {
            bytes.extend(0x0080_2008_0200_8020u64.to_be_bytes());
        }
        let out = palette(&mut bytes.as_slice(), 24, 8, 4).unwrap();
        assert_eq!(out.len(), 24);
        assert_eq!(out[0], 4);
        assert_eq!(out[1], 9);
        assert_eq!(out[12], 4);
    }
    #[test]
    fn rejects_palette_index_overflow() {
        let mut bytes = vec![4, 1, 7];
        bytes.extend(u64::MAX.to_be_bytes());
        assert!(palette(&mut bytes.as_slice(), 16, 8, 4).is_err());
    }
    #[test]
    fn decodes_26_2_fluid_count_and_single_palettes() {
        let section = [0x10, 0x00, 0, 7, 0, 1, 0, 0];
        let mut data = vec![];
        data.extend((-12i32).to_be_bytes());
        data.extend(8i32.to_be_bytes());
        varint(0, &mut data);
        varint(section.len() as i32, &mut data);
        data.extend(section);
        data.push(0);
        let chunk = decode(&data).unwrap();
        assert_eq!((chunk.x, chunk.z), (-12, 8));
        assert_eq!(chunk.sections.len(), 1);
        assert_eq!(chunk.sections[0].fluids, 7);
        assert_eq!(chunk.sections[0].blocks, vec![1; 4096]);
    }
    #[test]
    fn dimension_controls_section_count_and_trailing_buffer_is_not_extra_sections() {
        let mut data = vec![0; 8];
        varint(0, &mut data);
        let mut sections = Vec::new();
        for _ in 0..24 {
            sections.extend([0x10, 0, 0, 0, 0, 1, 0, 0]);
        }
        // Extra complete empty sections and arbitrary storage padding must not
        // change the dimension's height (LevelChunk.replaceWithPacketData).
        sections.extend([0; 16]);
        sections.extend([0xfe, 0xff]);
        varint(sections.len() as i32, &mut data);
        data.extend(sections);
        data.push(0);
        let chunk = decode_for_dimension(&data, 24).unwrap();
        assert_eq!(chunk.sections.len(), 24);
        assert!(chunk.sections.iter().all(|s| s.blocks == vec![1; 4096]));
        assert!(decode_for_dimension(&data, 27).is_err());
        assert!(decode_for_dimension(&data, 0).is_err());
    }
}
