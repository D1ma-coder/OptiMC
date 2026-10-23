//! Big-endian named/file and unnamed/network NBT, with Java modified UTF-8.
//! UTF-16 units are retained so even isolated Java surrogates round-trip safely.
use anyhow::{bail, ensure, Context, Result};
use std::{collections::BTreeMap, mem::size_of};
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NbtString(pub Vec<u16>);
impl From<&str> for NbtString {
    fn from(value: &str) -> Self {
        Self(value.encode_utf16().collect())
    }
}
impl NbtString {
    pub fn to_utf8(&self) -> Result<String> {
        Ok(String::from_utf16(&self.0)?)
    }
    pub fn to_string_lossy(&self) -> String {
        String::from_utf16_lossy(&self.0)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(NbtString),
    List { element_type: u8, values: Vec<Tag> },
    Compound(BTreeMap<NbtString, Tag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}
impl Tag {
    pub fn id(&self) -> u8 {
        match self {
            Self::End => 0,
            Self::Byte(_) => 1,
            Self::Short(_) => 2,
            Self::Int(_) => 3,
            Self::Long(_) => 4,
            Self::Float(_) => 5,
            Self::Double(_) => 6,
            Self::ByteArray(_) => 7,
            Self::String(_) => 8,
            Self::List { .. } => 9,
            Self::Compound(_) => 10,
            Self::IntArray(_) => 11,
            Self::LongArray(_) => 12,
        }
    }
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Compound(values) => values.get(&NbtString::from(key)),
            _ => None,
        }
    }
    pub fn integer(&self) -> Option<i64> {
        match self {
            Self::Byte(v) => Some(*v as i64),
            Self::Short(v) => Some(*v as i64),
            Self::Int(v) => Some(*v as i64),
            Self::Long(v) => Some(*v),
            _ => None,
        }
    }
    pub fn string(&self) -> Option<&NbtString> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub memory: usize,
    pub nodes: usize,
    pub depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            memory: 8 * 1024 * 1024,
            nodes: 1_000_000,
            depth: 512,
        }
    }
}
struct Reader<'a> {
    input: &'a [u8],
    limits: Limits,
    memory: usize,
    nodes: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        ensure!(count <= self.input.len(), "Truncated NBT payload");
        let (value, rest) = self.input.split_at(count);
        self.input = rest;
        Ok(value)
    }
    fn number<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into()?)
    }
    fn charge(&mut self, size: usize) -> Result<()> {
        self.memory = self
            .memory
            .checked_add(size)
            .context("NBT memory overflow")?;
        ensure!(
            self.memory <= self.limits.memory,
            "NBT memory limit exceeded"
        );
        Ok(())
    }
    fn length(&mut self, unit: usize) -> Result<usize> {
        let length = i32::from_be_bytes(self.number()?);
        ensure!(length >= 0, "Negative NBT array/list length");
        let length = length as usize;
        ensure!(
            length <= self.input.len() / unit.max(1),
            "Truncated/oversized NBT array/list"
        );
        Ok(length)
    }
    fn text(&mut self) -> Result<NbtString> {
        let length = u16::from_be_bytes(self.number()?) as usize;
        self.charge(length * 2)?;
        let bytes = self.take(length)?;
        let mut result = Vec::with_capacity(length);
        let mut i = 0;
        while i < bytes.len() {
            let a = bytes[i];
            i += 1;
            let value = match a {
                0..=127 => a as u16,
                0xc0..=0xdf => {
                    ensure!(
                        i < bytes.len() && bytes[i] & 0xc0 == 0x80,
                        "Malformed modified UTF-8"
                    );
                    let value = ((a & 31) as u16) << 6 | (bytes[i] & 63) as u16;
                    i += 1;
                    value
                }
                0xe0..=0xef => {
                    ensure!(
                        i + 1 < bytes.len()
                            && bytes[i] & 0xc0 == 0x80
                            && bytes[i + 1] & 0xc0 == 0x80,
                        "Malformed modified UTF-8"
                    );
                    let value = ((a & 15) as u16) << 12
                        | ((bytes[i] & 63) as u16) << 6
                        | (bytes[i + 1] & 63) as u16;
                    i += 2;
                    value
                }
                _ => bail!("Malformed modified UTF-8 leading byte"),
            };
            result.push(value);
        }
        Ok(NbtString(result))
    }
    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag> {
        let child_depth = depth + usize::from(id == 9 || id == 10);
        ensure!(
            child_depth <= self.limits.depth,
            "NBT nesting limit exceeded"
        );
        self.nodes += 1;
        ensure!(self.nodes <= self.limits.nodes, "NBT node limit exceeded");
        self.charge(size_of::<Tag>())?;
        Ok(match id {
            0 => Tag::End,
            1 => Tag::Byte(self.number::<1>()?[0] as i8),
            2 => Tag::Short(i16::from_be_bytes(self.number()?)),
            3 => Tag::Int(i32::from_be_bytes(self.number()?)),
            4 => Tag::Long(i64::from_be_bytes(self.number()?)),
            5 => Tag::Float(f32::from_be_bytes(self.number()?)),
            6 => Tag::Double(f64::from_be_bytes(self.number()?)),
            7 => {
                let len = self.length(1)?;
                self.charge(len)?;
                Tag::ByteArray(self.take(len)?.to_vec())
            }
            8 => Tag::String(self.text()?),
            9 => {
                let element_type = self.number::<1>()?[0];
                ensure!(element_type <= 12, "Invalid NBT list type");
                let len = self.length(1)?;
                ensure!(element_type != 0 || len == 0, "Nonempty END list");
                self.charge(
                    len.checked_mul(size_of::<Tag>())
                        .context("NBT list overflow")?,
                )?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    let mut value = self.payload(element_type, child_depth)?;
                    // 26.2 ListTag.addAndUnwrap: heterogeneous lists use a
                    // compound with the single empty-name entry as a wrapper.
                    if let Tag::Compound(fields) = &mut value {
                        if fields.len() == 1 && fields.contains_key(&NbtString::default()) {
                            value = fields.remove(&NbtString::default()).unwrap();
                        }
                    }
                    values.push(value);
                }
                let element_type = list_type(&values);
                Tag::List {
                    element_type,
                    values,
                }
            }
            10 => {
                let mut values = BTreeMap::new();
                loop {
                    let id = self.number::<1>()?[0];
                    if id == 0 {
                        break;
                    }
                    self.charge(size_of::<(NbtString, Tag)>() + 32)?;
                    let key = self.text()?;
                    let value = self.payload(id, child_depth)?;
                    values.insert(key, value);
                }
                Tag::Compound(values)
            }
            11 => {
                let len = self.length(4)?;
                self.charge(len * 4)?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(i32::from_be_bytes(self.number()?));
                }
                Tag::IntArray(values)
            }
            12 => {
                let len = self.length(8)?;
                self.charge(len * 8)?;
                let mut values = Vec::with_capacity(len);
                for _ in 0..len {
                    values.push(i64::from_be_bytes(self.number()?));
                }
                Tag::LongArray(values)
            }
            _ => bail!("Unknown NBT tag type {id}"),
        })
    }
}
pub fn read_network(input: &mut &[u8]) -> Result<Tag> {
    read_network_limited(input, Limits::default())
}
pub struct Budget {
    limits: Limits,
    memory: usize,
    nodes: usize,
}
impl Budget {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            memory: 0,
            nodes: 0,
        }
    }
    pub fn read_network(&mut self, input: &mut &[u8]) -> Result<Tag> {
        let mut reader = Reader {
            input,
            limits: self.limits,
            memory: self.memory,
            nodes: self.nodes,
        };
        let id = reader.number::<1>()?[0];
        let tag = reader.payload(id, 0)?;
        self.memory = reader.memory;
        self.nodes = reader.nodes;
        *input = reader.input;
        Ok(tag)
    }
}
pub fn read_network_limited(input: &mut &[u8], limits: Limits) -> Result<Tag> {
    Budget::new(limits).read_network(input)
}
pub fn read_named(input: &mut &[u8]) -> Result<(NbtString, Tag)> {
    let mut reader = Reader {
        input,
        limits: Limits::default(),
        memory: 0,
        nodes: 0,
    };
    let id = reader.number::<1>()?[0];
    let name = if id == 0 {
        NbtString::default()
    } else {
        reader.text()?
    };
    let tag = reader.payload(id, 0)?;
    *input = reader.input;
    Ok((name, tag))
}
fn output_room(out: &[u8], additional: usize) -> Result<()> {
    ensure!(
        out.len()
            .checked_add(additional)
            .is_some_and(|n| n <= 8 * 1024 * 1024),
        "NBT output exceeds limit"
    );
    Ok(())
}
fn write_text(value: &NbtString, out: &mut Vec<u8>) -> Result<()> {
    let length = value
        .0
        .iter()
        .map(|v| {
            if *v != 0 && *v <= 127 {
                1
            } else if *v <= 2047 {
                2
            } else {
                3
            }
        })
        .sum::<usize>();
    ensure!(
        length <= u16::MAX as usize,
        "NBT string exceeds modified UTF-8 limit"
    );
    output_room(out, length + 2)?;
    out.extend((length as u16).to_be_bytes());
    for &c in &value.0 {
        match c {
            1..=127 => out.push(c as u8),
            0..=2047 => out.extend([0xc0 | (c >> 6) as u8, 0x80 | (c & 63) as u8]),
            _ => out.extend([
                0xe0 | (c >> 12) as u8,
                0x80 | ((c >> 6) & 63) as u8,
                0x80 | (c & 63) as u8,
            ]),
        }
    }
    Ok(())
}
fn list_type(values: &[Tag]) -> u8 {
    let Some(first) = values.first() else {
        return 0;
    };
    if values.iter().all(|value| value.id() == first.id()) {
        first.id()
    } else {
        10
    }
}
fn write_payload(tag: &Tag, out: &mut Vec<u8>, depth: usize) -> Result<()> {
    let child_depth = depth + usize::from(matches!(tag, Tag::List { .. } | Tag::Compound(_)));
    ensure!(child_depth <= 512, "NBT nesting limit exceeded");
    match tag {
        Tag::End => {}
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend(v.to_be_bytes()),
        Tag::Int(v) => out.extend(v.to_be_bytes()),
        Tag::Long(v) => out.extend(v.to_be_bytes()),
        Tag::Float(v) => out.extend(if v.is_nan() { f32::NAN } else { *v }.to_be_bytes()),
        Tag::Double(v) => out.extend(if v.is_nan() { f64::NAN } else { *v }.to_be_bytes()),
        Tag::ByteArray(v) => {
            output_room(out, v.len().checked_add(4).context("NBT output overflow")?)?;
            write_length(v.len(), out)?;
            out.extend(v);
        }
        Tag::String(v) => write_text(v, out)?,
        Tag::List {
            element_type,
            values,
        } => {
            ensure!(
                *element_type <= 12 && (*element_type != 0 || values.is_empty()),
                "Invalid NBT list type"
            );
            let raw_type = list_type(values);
            out.push(raw_type);
            write_length(values.len(), out)?;
            for value in values {
                ensure!(value.id() != 0, "END element in NBT list");
                let wrapper = matches!(value,Tag::Compound(fields) if fields.len()==1 && fields.contains_key(&NbtString::default()));
                if raw_type == 10 && (value.id() != 10 || wrapper) {
                    ensure!(child_depth < 512, "NBT nesting limit exceeded");
                    out.push(value.id());
                    write_text(&NbtString::default(), out)?;
                    write_payload(value, out, child_depth + 1)?;
                    out.push(0);
                } else {
                    write_payload(value, out, child_depth)?;
                }
            }
        }
        Tag::Compound(values) => {
            for (key, value) in values {
                ensure!(value.id() != 0, "Named END inside compound");
                out.push(value.id());
                write_text(key, out)?;
                write_payload(value, out, child_depth)?;
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            output_room(
                out,
                v.len()
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(4))
                    .context("NBT output overflow")?,
            )?;
            write_length(v.len(), out)?;
            for value in v {
                out.extend(value.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            output_room(
                out,
                v.len()
                    .checked_mul(8)
                    .and_then(|n| n.checked_add(4))
                    .context("NBT output overflow")?,
            )?;
            write_length(v.len(), out)?;
            for value in v {
                out.extend(value.to_be_bytes());
            }
        }
    }
    ensure!(out.len() <= 8 * 1024 * 1024, "NBT output exceeds limit");
    Ok(())
}
fn write_length(length: usize, out: &mut Vec<u8>) -> Result<()> {
    ensure!(length <= i32::MAX as usize, "NBT length overflow");
    out.extend((length as i32).to_be_bytes());
    Ok(())
}
pub fn write_network(tag: &Tag, out: &mut Vec<u8>) -> Result<()> {
    out.push(tag.id());
    write_payload(tag, out, 0)
}

pub fn write_named(name: &NbtString, tag: &Tag, out: &mut Vec<u8>) -> Result<()> {
    ensure!(tag.id() != 0, "Named NBT root cannot be End");
    out.push(tag.id());
    write_text(name, out)?;
    write_payload(tag, out, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_heterogeneous_lists_unwrap_once_and_escape_literal_empty_keys() {
        use base64::Engine;
        let reference = super::super::sign_edit::reference();
        for (index, row) in reference["nbt"].as_array().unwrap().iter().enumerate() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row.as_str().unwrap())
                .unwrap();
            let mut input = bytes.as_slice();
            let tag = read_network(&mut input).unwrap();
            assert!(input.is_empty());
            let mut encoded = Vec::new();
            write_network(&tag, &mut encoded).unwrap();
            assert_eq!(encoded, bytes, "Source NBT case {index}");
            if index == 0 {
                let Tag::List { values, .. } = tag else {
                    panic!("Expected ListTag");
                };
                assert_eq!(values[0], Tag::String("Front".into()));
                assert_eq!(values[1], Tag::Int(7));
                assert_eq!(values[2].get("text"), Some(&Tag::String("Rich".into())));
                assert_eq!(values[3].get(""), Some(&Tag::String("literal key".into())));
            }
        }
    }
    #[test]
    fn all_payload_types_round_trip_network_root_without_a_name() {
        let tags = vec![
            Tag::Byte(-1),
            Tag::Short(-32000),
            Tag::Int(123),
            Tag::Long(i64::MIN),
            Tag::Float(1.25),
            Tag::Double(-0.5),
            Tag::ByteArray(vec![0, 128, 255]),
            Tag::String(NbtString::from("A\0😀")),
            Tag::List {
                element_type: 3,
                values: vec![Tag::Int(1), Tag::Int(2)],
            },
            Tag::Compound(BTreeMap::new()),
            Tag::IntArray(vec![-1, 2]),
            Tag::LongArray(vec![i64::MAX, -1]),
        ];
        let root = Tag::Compound(
            tags.into_iter()
                .enumerate()
                .map(|(i, v)| (NbtString::from(i.to_string().as_str()), v))
                .collect(),
        );
        let mut bytes = Vec::new();
        write_network(&root, &mut bytes).unwrap();
        assert_eq!(bytes[0], 10);
        let mut input = bytes.as_slice();
        assert_eq!(read_network(&mut input).unwrap(), root);
        assert!(input.is_empty());
    }
    #[test]
    fn java_modified_utf8_preserves_nul_and_unpaired_surrogates() {
        let bytes = [8, 0, 6, 0xc0, 0x80, 0xed, 0xa0, 0x80, 0x41];
        let tag = read_network(&mut bytes.as_slice()).unwrap();
        assert_eq!(tag, Tag::String(NbtString(vec![0, 0xd800, 65])));
        let mut encoded = Vec::new();
        write_network(&tag, &mut encoded).unwrap();
        assert_eq!(encoded, bytes);
        assert!(read_network(&mut &[8, 0, 4, 0xf0, 0x9f, 0x98, 0x80][..]).is_err());
    }
    #[test]
    fn named_root_consumes_name_but_network_root_does_not() {
        let bytes = [10, 0, 1, b'x', 3, 0, 1, b'y', 0, 0, 0, 7, 0, 99];
        let mut input = bytes.as_slice();
        let (name, tag) = read_named(&mut input).unwrap();
        assert_eq!(name.to_utf8().unwrap(), "x");
        assert_eq!(tag.get("y").unwrap().integer(), Some(7));
        assert_eq!(input, [99]);
    }
    #[test]
    fn one_packet_budget_is_shared_between_multiple_tags() {
        let mut budget = Budget::new(Limits {
            memory: size_of::<Tag>(),
            ..Limits::default()
        });
        let mut input = &[3, 0, 0, 0, 1, 3, 0, 0, 0, 2][..];
        assert_eq!(budget.read_network(&mut input).unwrap(), Tag::Int(1));
        assert!(budget.read_network(&mut input).is_err());
    }
    #[test]
    fn maximum_java_nbt_nesting_is_bounded_and_preserved() {
        let nested = |depth: usize| {
            let mut bytes = vec![10];
            for _ in 0..depth {
                bytes.extend([10, 0, 0]);
            }
            bytes.extend(std::iter::repeat_n(0, depth + 1));
            bytes
        };
        let allowed = nested(511);
        assert!(read_network(&mut allowed.as_slice()).is_ok());
        let too_deep = nested(512);
        assert!(read_network(&mut too_deep.as_slice()).is_err());
    }
    #[test]
    fn malformed_counts_types_depth_and_memory_are_rejected_before_large_allocation() {
        for bytes in [
            vec![7, 0x7f, 0xff, 0xff, 0xff],
            vec![9, 0, 0, 0, 0, 1],
            vec![13],
            vec![11, 0xff, 0xff, 0xff, 0xff],
        ] {
            assert!(read_network(&mut bytes.as_slice()).is_err());
        }
        let bytes = [10, 10, 0, 0, 0, 0];
        assert!(read_network_limited(
            &mut bytes.as_slice(),
            Limits {
                depth: 0,
                ..Limits::default()
            }
        )
        .is_err());
        assert!(read_network_limited(
            &mut &[3, 0, 0, 0, 0][..],
            Limits {
                memory: 0,
                ..Limits::default()
            }
        )
        .is_err());
    }
}
