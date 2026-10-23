//! Protocol 776 LpVec3: two low bytes + big-endian high word, optional scale.
use super::codec::{read_varint, varint};
use anyhow::Result;
use glam::DVec3;
use std::io::Read;
pub fn read(input: &mut impl Read) -> Result<DVec3> {
    let mut first = [0];
    input.read_exact(&mut first)?;
    if first[0] == 0 {
        return Ok(DVec3::ZERO);
    }
    let mut low = [0];
    let mut high = [0; 4];
    input.read_exact(&mut low)?;
    input.read_exact(&mut high)?;
    let packed = (u32::from_be_bytes(high) as u64) << 16 | (low[0] as u64) << 8 | first[0] as u64;
    let mut scale = (first[0] & 3) as u64;
    if first[0] & 4 != 0 {
        scale |= (read_varint(input)? as u32 as u64) << 2;
    }
    let unpack = |shift: u32| {
        (((packed >> shift) & 32767).min(32766) as f64 * 2. / 32766. - 1.) * scale as f64
    };
    Ok(DVec3::new(unpack(3), unpack(18), unpack(33)))
}
pub fn write(value: DVec3, out: &mut Vec<u8>) {
    let clean = value.to_array().map(|v| {
        if v.is_nan() {
            0.
        } else {
            v.clamp(-17179869183., 17179869183.)
        }
    });
    let largest = clean.into_iter().map(f64::abs).fold(0., f64::max);
    if largest < 1. / 32766. {
        out.push(0);
        return;
    }
    let scale = largest.ceil() as u64;
    let continuation = scale > 3;
    let pack = |v: f64| ((v / scale as f64 * 0.5 + 0.5) * 32766. + 0.5).floor() as u64;
    let packed = (scale & 3)
        | if continuation { 4 } else { 0 }
        | pack(clean[0]) << 3
        | pack(clean[1]) << 18
        | pack(clean[2]) << 33;
    out.push(packed as u8);
    out.push((packed >> 8) as u8);
    out.extend_from_slice(&((packed >> 16) as u32).to_be_bytes());
    if continuation {
        varint((scale >> 2) as i32, out);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vectors_preserve_range_and_protocol_error_bound() {
        for vector in [
            DVec3::ZERO,
            DVec3::new(1., -0.42, 0.),
            DVec3::new(100., -300., 0.125),
            DVec3::splat(17179869183.),
            DVec3::splat(-17179869183.),
        ] {
            let mut bytes = Vec::new();
            write(vector, &mut bytes);
            let mut input = bytes.as_slice();
            let decoded = read(&mut input).unwrap();
            assert!(input.is_empty());
            let error = vector.abs().max_element().ceil().max(1.) / 32766.;
            assert!((decoded - vector).abs().max_element() <= error + 1e-8);
        }
    }
    #[test]
    fn lp_vectors_match_bytes_produced_by_official_26_2_class() {
        // Obtained by executing LpVec3.class from the imported JAR in a
        // temporary development harness. Java is not used by the client.
        let cases = [
            (DVec3::ZERO, "00"),
            (DVec3::new(1., -0.42, 0.), "f1ff7ffe947b"),
            (DVec3::new(100., -300., 0.125), "a4aa800c00024b"),
            (DVec3::splat(17179869183.), "f7fffffdfffbffffffff0f"),
            (DVec3::splat(-17179869183.), "070000000000ffffffff0f"),
        ];
        for (value, hex) in cases {
            let expected: Vec<_> = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let mut actual = Vec::new();
            write(value, &mut actual);
            assert_eq!(actual, expected);
        }
        let decoded = read(&mut &[0xa4, 0xaa, 0x80, 0x0c, 0, 2, 0x4b][..]).unwrap();
        assert_eq!(decoded.z, 0.1281816517121115);
    }
    #[test]
    fn zero_is_one_byte_and_extended_scale_uses_unsigned_varint() {
        let mut bytes = Vec::new();
        write(DVec3::ZERO, &mut bytes);
        assert_eq!(bytes, [0]);
        bytes.clear();
        write(DVec3::splat(17179869183.), &mut bytes);
        assert_eq!(bytes.len(), 11);
        assert!(read(&mut bytes.as_slice()).unwrap().x > 1e10);
        assert!(read(&mut &[1, 0][..]).is_err());
    }
}
