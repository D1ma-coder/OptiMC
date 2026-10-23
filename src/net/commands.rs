//! Server command graph and Brigadier suggestion ranges (UTF-16 offsets).
use super::{
    codec::{read_string, read_varint},
    component,
    maps::{byte, bytes},
};
use crate::text::RichText;
use anyhow::{ensure, Result};
use std::collections::BTreeSet;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub flags: u8,
    pub children: Vec<usize>,
    pub redirect: Option<usize>,
    pub name: String,
    pub parser: Option<u32>,
    pub properties: Vec<u8>,
    pub suggestions: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub root: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub text: String,
    pub tooltip: Option<RichText>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Suggestions {
    pub request: i32,
    pub start: usize,
    pub length: usize,
    pub entries: Vec<Suggestion>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    pub tree: Tree,
    pub suggestions: Option<Suggestions>,
    pub custom: BTreeSet<String>,
    pub revision: u64,
}
fn number(input: &mut &[u8], max: usize) -> Result<usize> {
    let n = read_varint(input)?;
    ensure!(
        n >= 0 && n as usize <= max,
        "Invalid command collection/index"
    );
    Ok(n as usize)
}
impl Tree {
    fn read(mut input: &[u8]) -> Result<Self> {
        let n = number(&mut input, 65536)?;
        ensure!(n > 0 && n <= input.len(), "Invalid command node count");
        let mut nodes = Vec::with_capacity(n);
        for _ in 0..n {
            let flags = byte(&mut input)?;
            ensure!(
                flags & 3 < 3 && flags & !63 == 0,
                "Invalid command node flags"
            );
            let count = number(&mut input, n)?;
            let mut children = Vec::with_capacity(count);
            for _ in 0..count {
                children.push(number(&mut input, n - 1)?);
            }
            let redirect = if flags & 8 != 0 {
                Some(number(&mut input, n - 1)?)
            } else {
                None
            };
            let name = if flags & 3 != 0 {
                read_string(&mut input, 32767)?
            } else {
                String::new()
            };
            let mut parser = None;
            let mut properties = Vec::new();
            if flags & 3 == 2 {
                let kind = number(&mut input, 56)? as u32;
                parser = Some(kind);
                let start = input;
                match kind {
                    1..=4 => {
                        let bits = byte(&mut input)?;
                        ensure!(bits & !3 == 0, "Invalid numeric argument bounds");
                        let size = if kind == 2 || kind == 4 { 8 } else { 4 };
                        if bits & 1 != 0 {
                            bytes(&mut input, size)?;
                        }
                        if bits & 2 != 0 {
                            bytes(&mut input, size)?;
                        }
                    }
                    5 => {
                        ensure!(
                            (0..=2).contains(&read_varint(&mut input)?),
                            "Invalid string argument kind"
                        );
                    }
                    6 | 31 => {
                        byte(&mut input)?;
                    }
                    43 => {
                        bytes(&mut input, 4)?;
                    }
                    44..=48 => {
                        super::registry::identifier(&mut input)?;
                    }
                    _ => {}
                }
                properties = start[..start.len() - input.len()].to_vec();
            }
            let suggestions = if flags & 16 != 0 {
                Some(super::registry::identifier(&mut input)?)
            } else {
                None
            };
            nodes.push(Node {
                flags,
                children,
                redirect,
                name,
                parser,
                properties,
                suggestions,
            });
        }
        let root = number(&mut input, n - 1)?;
        ensure!(
            nodes[root].flags & 3 == 0 && input.is_empty(),
            "Invalid command graph root/trailing data"
        );
        Ok(Self { nodes, root })
    }
    pub fn root_literals(&self, prefix: &str) -> Vec<String> {
        let Some(root) = self.nodes.get(self.root) else {
            return Vec::new();
        };
        let mut names: Vec<_> = root
            .children
            .iter()
            .filter_map(|&i| self.nodes.get(i))
            .filter(|n| {
                n.flags & 3 == 1 && n.name.to_lowercase().starts_with(&prefix.to_lowercase())
            })
            .map(|n| n.name.clone())
            .collect();
        names.sort();
        names
    }
}
impl Store {
    pub fn packet(&mut self, id: i32, mut input: &[u8]) -> Result<()> {
        match id {
            0x10 => {
                self.tree = Tree::read(input)?;
            }
            0x0f => {
                let request = read_varint(&mut input)?;
                let start = number(&mut input, 32767)?;
                let length = number(&mut input, 32767)?;
                ensure!(
                    start.checked_add(length).is_some_and(|n| n <= 32767),
                    "Invalid suggestion range"
                );
                let n = number(&mut input, 16384)?;
                ensure!(n <= input.len(), "Invalid suggestion count");
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    entries.push(Suggestion {
                        text: read_string(&mut input, 32767)?,
                        tooltip: if byte(&mut input)? != 0 {
                            Some(component::read_rich(&mut input)?)
                        } else {
                            None
                        },
                    });
                }
                ensure!(input.is_empty(), "Trailing command suggestions");
                self.suggestions = Some(Suggestions {
                    request,
                    start,
                    length,
                    entries,
                });
            }
            0x17 => {
                let action = number(&mut input, 2)?;
                let n = number(&mut input, 16384)?;
                ensure!(n <= input.len(), "Invalid chat completion count");
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    entries.push(read_string(&mut input, 32767)?);
                }
                ensure!(input.is_empty(), "Trailing custom chat completions");
                let mut custom = if action == 2 {
                    BTreeSet::new()
                } else {
                    self.custom.clone()
                };
                for entry in entries {
                    if action == 1 {
                        custom.remove(&entry);
                    } else {
                        custom.insert(entry);
                    }
                }
                ensure!(custom.len() <= 16384, "Custom chat completion limit");
                self.custom = custom;
            }
            _ => anyhow::bail!("Not a command packet"),
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    #[test]
    fn all_original_argument_properties_and_redirect_graph_decode() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/commands-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let decode = |s: &serde_json::Value| {
            base64::engine::general_purpose::STANDARD
                .decode(s.as_str().unwrap())
                .unwrap()
        };
        for argument in reference["arguments"].as_array().unwrap() {
            let mut graph = vec![2, 0, 1, 1, 2, 0];
            super::super::codec::string("value", &mut graph);
            let id = argument["id"].as_i64().unwrap() as i32;
            super::super::codec::varint(id, &mut graph);
            let properties = decode(&argument["properties"]);
            graph.extend(&properties);
            graph.push(0);
            let tree = Tree::read(&graph).unwrap_or_else(|e| panic!("{}: {e}", argument["name"]));
            assert_eq!(tree.nodes[1].parser, Some(id as u32));
            assert_eq!(tree.nodes[1].properties, properties);
        }
        let graph = decode(&reference["graph"]);
        let tree = Tree::read(&graph).unwrap();
        assert!(tree.nodes.iter().any(|n| n.redirect.is_some()));
        assert!(tree.root_literals("M").contains(&"msg".into()));
        for length in 0..graph.len() {
            assert!(Tree::read(&graph[..length]).is_err());
        }
    }
    #[test]
    fn original_suggestions_preserve_tooltips_and_custom_completion_actions() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/chat-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let r: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let mut store = Store::default();
        for index in 4..8 {
            let row = &r["packets"][index];
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            store
                .packet(row["id"].as_i64().unwrap() as i32, &bytes)
                .unwrap();
        }
        let suggestions = store.suggestions.unwrap();
        assert_eq!(
            (suggestions.request, suggestions.start, suggestions.length),
            (42, 5, 2)
        );
        assert_eq!(suggestions.entries[0].text, "Alice");
        assert_eq!(
            suggestions.entries[0].tooltip.as_ref().unwrap().runs[0].text,
            "Tooltip"
        );
        assert_eq!(store.custom, BTreeSet::from(["Bob".into()]));
    }
}
