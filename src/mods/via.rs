// SPDX-License-Identifier: GPL-3.0-only
// Rust port of ViaVersion's version-2 MappingDataLoader codecs, taken from the
// ViaFabricPlus 5.0.3 embedded ViaVersion JAR. https://github.com/ViaVersion/ViaVersion
// See LICENSES/GPL-3.0.txt and THIRD_PARTY_NOTICES.md.
use crate::net::{
    codec::read_varint,
    nbt::{self, Tag},
};
use anyhow::{ensure, Context, Result};
use base64::Engine;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    io::Read,
    sync::OnceLock,
};
const MAX_IDS: usize = 1_048_576;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub values: Vec<i32>,
    pub mapped_size: i32,
}
impl Mapping {
    pub fn get(&self, id: i32) -> i32 {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.values.get(i))
            .copied()
            .unwrap_or(-1)
    }
    pub fn inverse(&self) -> Result<Self> {
        ensure!(
            self.mapped_size >= 0 && self.mapped_size as usize <= MAX_IDS,
            "Invalid inverse mapping size"
        );
        let mut values = vec![-1; self.mapped_size as usize];
        for (old, &new) in self.values.iter().enumerate() {
            if new == -1 {
                continue;
            }
            ensure!(
                new >= 0 && (new as usize) < values.len(),
                "Mapping target outside inverse registry"
            );
            if values[new as usize] == -1 {
                values[new as usize] = old as i32;
            }
        }
        Ok(Self {
            values,
            mapped_size: self.values.len() as i32,
        })
    }
}
fn integer(tag: &Tag, key: &str) -> Result<i32> {
    let value = tag
        .get(key)
        .and_then(Tag::integer)
        .context(format!("Missing mapping integer {key}"))?;
    Ok(i32::try_from(value)?)
}
fn length(tag: &Tag, key: &str) -> Result<usize> {
    let value = integer(tag, key)?;
    ensure!(
        value >= 0 && value as usize <= MAX_IDS,
        "Mapping length out of bounds"
    );
    Ok(value as usize)
}
fn zigzag(input: &mut &[u8]) -> Result<i32> {
    let v = read_varint(input)? as u32;
    Ok(((v >> 1) as i32) ^ -((v & 1) as i32))
}
pub fn decode_table(root: &Tag, name: &str) -> Result<Option<Mapping>> {
    let Some(table) = root.get(name) else {
        return Ok(None);
    };
    ensure!(
        root.get("version").and_then(Tag::integer) == Some(2),
        "Unsupported Via mapping format version"
    );
    let size = length(table, "size")?;
    let mapped_size = table.get("mappedSize").and_then(Tag::integer).unwrap_or(-1) as i32;
    let strategy = integer(table, "id")?;
    if strategy == 3 {
        return Ok(Some(Mapping {
            values: (0..size as i32).collect(),
            mapped_size,
        }));
    }
    let Some(Tag::ByteArray(bytes)) = table.get("val") else {
        anyhow::bail!("Missing compressed mapping values");
    };
    let mut input = bytes.as_slice();
    let mut values = vec![-1; size];
    if strategy == 0 {
        let mut current = 0i32;
        for value in &mut values {
            current = current.wrapping_add(zigzag(&mut input)?);
            *value = current;
        }
        ensure!(input.is_empty(), "Trailing direct mapping bytes");
    } else {
        ensure!(
            strategy == 1 || strategy == 2,
            "Unknown Via mapping strategy"
        );
        let mut pairs = Vec::new();
        let mut at = -1i32;
        let mut current = 0i32;
        while !input.is_empty() {
            let skip = read_varint(&mut input)?;
            ensure!(skip >= 0, "Negative mapping delta");
            at = at
                .checked_add(1)
                .and_then(|v| v.checked_add(skip))
                .context("Mapping source overflow")?;
            ensure!(
                (at as usize) < size,
                "Mapping source outside array registry"
            );
            current = current.wrapping_add(zigzag(&mut input)?);
            pairs.push((at as usize, current));
        }
        if strategy == 1 {
            ensure!(!pairs.is_empty(), "Empty shift mapping");
            for (i, value) in values.iter_mut().enumerate().take(pairs[0].0) {
                *value = i as i32;
            }
            for (i, &(start, mapped)) in pairs.iter().enumerate() {
                let end = pairs.get(i + 1).map_or(size, |p| p.0);
                for (delta, value) in values[start..end].iter_mut().enumerate() {
                    *value = mapped.wrapping_add(delta as i32);
                }
            }
        } else {
            if table.get("nofill").is_none() {
                for (i, value) in values.iter_mut().enumerate() {
                    *value = i as i32;
                }
            }
            for (at, value) in pairs {
                values[at] = value;
            }
        }
    }
    Ok(Some(Mapping {
        values,
        mapped_size,
    }))
}
#[derive(Clone, Debug, Deserialize)]
pub struct Version {
    pub id: i32,
    pub name: String,
    pub snapshot: bool,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Deserialize)]
pub struct Resources {
    pub versions: Vec<Version>,
    files: BTreeMap<String, String>,
}
pub fn resources() -> Result<&'static Resources> {
    static RESOURCE: OnceLock<Result<Resources, String>> = OnceLock::new();
    RESOURCE
        .get_or_init(|| {
            (|| -> Result<Resources> {
                let mut data = Vec::new();
                flate2::read::GzDecoder::new(
                    include_bytes!("../../data/mods/via-mappings.json.gz").as_slice(),
                )
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut data)?;
                ensure!(data.len() <= 16 * 1024 * 1024, "Via resources exceed limit");
                Ok(serde_json::from_slice(&data)?)
            })()
            .map_err(|e| format!("{e:#}"))
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))
}
pub fn load_file(name: &str) -> Result<Tag> {
    let resource = resources()?
        .files
        .get(name)
        .context("Unknown Via mapping resource")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(resource)?;
    let mut input = bytes.as_slice();
    let (_, tag) = nbt::read_named(&mut input)?;
    ensure!(input.is_empty(), "Trailing mapping NBT bytes");
    Ok(tag)
}
/// A mapping-data route. It deliberately does not claim to translate the login,
/// configuration or play packet schemas; those require individual rewriters.
pub fn mapping_route(source: &str, target: &str) -> Result<Vec<String>> {
    if source == target {
        return Ok(Vec::new());
    }
    let mut pending = VecDeque::from([(source.to_owned(), Vec::<String>::new())]);
    let mut seen = HashSet::from([source.to_owned()]);
    while let Some((version, path)) = pending.pop_front() {
        for file in resources()?.files.keys() {
            let Some((from, to)) = file
                .strip_prefix("mappings-")
                .and_then(|s| s.strip_suffix(".nbt"))
                .and_then(|s| s.split_once("to"))
            else {
                continue;
            };
            if from != version || !seen.insert(to.to_owned()) {
                continue;
            }
            let mut next = path.clone();
            next.push(file.clone());
            if to == target {
                return Ok(next);
            }
            pending.push_back((to.to_owned(), next));
        }
    }
    anyhow::bail!("No mapping-data route from {source} to {target}")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_downloaded_via_tables_match_original_loader_and_legacy_array_rejection() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/mods/via-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let mut tested = 0;
        for row in reference["mappings"].as_array().unwrap() {
            let root = load_file(row["file"].as_str().unwrap()).unwrap();
            for (name, expected) in row["tables"].as_object().unwrap() {
                let result = decode_table(&root, name);
                if expected.get("error").is_some() {
                    assert!(result.is_err());
                    continue;
                }
                let mapping = result.unwrap().unwrap();
                assert_eq!(
                    mapping.mapped_size,
                    expected["mapped_size"].as_i64().unwrap() as i32
                );
                let values: Vec<i32> = expected["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap() as i32)
                    .collect();
                assert_eq!(mapping.values, values, "{} / {name}", row["file"]);
                tested += 1;
            }
        }
        assert!(tested > 100);
    }
    #[test]
    fn mapping_routes_follow_real_resources_and_do_not_claim_missing_legacy_pipeline() {
        let route = mapping_route("26.1", "26.2").unwrap();
        assert_eq!(route, ["mappings-26.1to26.2.nbt"]);
        assert!(mapping_route("1.12", "26.2").unwrap().len() > 15);
        assert!(mapping_route("1.8", "26.2").is_err());
        assert!(resources()
            .unwrap()
            .versions
            .iter()
            .any(|v| v.id == 776 && v.name.contains("26.2")));
        let mapping = Mapping {
            values: vec![2, -1, 2, 0],
            mapped_size: 3,
        };
        assert_eq!(mapping.inverse().unwrap().values, [3, -1, 0]);
    }
}
