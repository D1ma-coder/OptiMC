//! Component wire shapes inspected against the original Tool codecs.
use crate::net::{
    codec::read_varint,
    maps::{byte, bytes},
    registry::identifier,
};
use anyhow::{ensure, Result};
#[derive(Clone, Debug, PartialEq)]
pub enum Blocks {
    Tag(String),
    Ids(Vec<u32>),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub blocks: Blocks,
    pub speed: Option<f32>,
    pub drops: Option<bool>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub rules: Vec<Rule>,
    pub default_speed: f32,
    pub damage_per_block: i32,
    pub creative: bool,
}
pub fn float(input: &mut &[u8]) -> Result<f32> {
    let value = f32::from_be_bytes(bytes(input, 4)?.try_into().unwrap());
    ensure!(value.is_finite(), "Non-finite item component float");
    Ok(value)
}
pub fn count(input: &mut &[u8], max: usize) -> Result<usize> {
    let n = read_varint(input)?;
    ensure!(
        n >= 0 && n as usize <= max && n as usize <= input.len(),
        "Invalid item component count"
    );
    Ok(n as usize)
}
pub fn blocks(input: &mut &[u8]) -> Result<Blocks> {
    let size = read_varint(input)?;
    ensure!((0..=100001).contains(&size), "Invalid block holder set");
    if size == 0 {
        return Ok(Blocks::Tag(identifier(input)?));
    }
    let mut ids = Vec::with_capacity((size - 1) as usize);
    ensure!(
        (size - 1) as usize <= input.len(),
        "Truncated block holder set"
    );
    for _ in 1..size {
        let id = read_varint(input)?;
        ensure!(id >= 0, "Negative block registry ID");
        ids.push(id as u32);
    }
    Ok(Blocks::Ids(ids))
}
impl Tool {
    pub fn read(input: &mut &[u8]) -> Result<Self> {
        let n = count(input, 4096)?;
        let mut rules = Vec::with_capacity(n);
        for _ in 0..n {
            let blocks = blocks(input)?;
            let speed = if byte(input)? != 0 {
                Some(float(input)?)
            } else {
                None
            };
            let drops = if byte(input)? != 0 {
                Some(byte(input)? != 0)
            } else {
                None
            };
            rules.push(Rule {
                blocks,
                speed,
                drops,
            });
        }
        let default_speed = float(input)?;
        let damage_per_block = read_varint(input)?;
        let creative = byte(input)? != 0;
        Ok(Self {
            rules,
            default_speed,
            damage_per_block,
            creative,
        })
    }
}
