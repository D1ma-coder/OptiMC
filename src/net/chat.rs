//! Chat payloads, signed-message cache and registry-driven decorations.
//! Cryptographic message-chain verification is not yet implemented; signatures
//! here are used for protocol references/ACKs, never labelled as authenticated.
use super::{
    codec::{read_string, read_varint},
    component,
    last_seen::Signature,
    maps::{byte, bytes},
    registry::Registries,
};
use crate::text::{RichText, Style};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
#[derive(Clone)]
pub struct State {
    cache: Vec<Option<Signature>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            cache: vec![None; 128],
        }
    }
}
pub struct Message {
    pub text: Option<RichText>,
    pub signature: Option<Signature>,
    pub content: RichText,
}
fn boolean(input: &mut &[u8]) -> Result<bool> {
    Ok(byte(input)? != 0)
}
fn signature(input: &mut &[u8]) -> Result<Signature> {
    Ok(bytes(input, 256)?.try_into()?)
}
fn style_value(style: Style) -> Value {
    let mut v = json!({"bold":style.bold,"italic":style.italic,"underlined":style.underline,"strikethrough":style.strike,"obfuscated":style.obfuscated});
    if let Some(c) = style.color {
        v["color"] = json!(format!("#{c:06x}"));
    }
    if let Some(c) = style.shadow {
        v["shadow_color"] = json!(c);
    }
    v
}
fn rich_value(text: &RichText) -> Value {
    Value::Array(
        text.runs
            .iter()
            .map(|run| {
                let mut v = style_value(run.style);
                v["text"] = json!(run.text);
                v
            })
            .collect(),
    )
}
fn decoration(input: &mut &[u8]) -> Result<Value> {
    let key = read_string(input, 32767)?;
    let n = read_varint(input)?;
    ensure!(
        (0..=1024).contains(&n),
        "Invalid chat decoration parameters"
    );
    let mut parameters = Vec::new();
    for _ in 0..n {
        parameters.push(match read_varint(input)? {
            1 => "target",
            2 => "content",
            _ => "sender",
        });
    }
    let style = component::read_style(input)?;
    Ok(json!({"translation_key":key,"parameters":parameters,"style":style_value(style)}))
}
fn bound(input: &mut &[u8], registries: &Registries, content: &RichText) -> Result<RichText> {
    let kind = read_varint(input)?;
    ensure!(kind >= 0, "Invalid chat type holder");
    let definition = if kind == 0 {
        let chat = decoration(input)?;
        decoration(input)?;
        chat
    } else {
        let entry = registries
            .entry("minecraft:chat_type", kind as u32 - 1)
            .context("Unknown chat type registry ID")?;
        let tag = entry
            .data
            .as_ref()
            .and_then(|d| d.get("chat"))
            .context("Missing chat type decoration")?;
        component::json_tag(tag)
    };
    let name = component::read_rich(input)?;
    let target = if boolean(input)? {
        component::read_rich(input)?
    } else {
        RichText::default()
    };
    let parameters = definition["parameters"]
        .as_array()
        .context("Missing chat decoration parameter list")?;
    let args: Vec<_> = parameters
        .iter()
        .map(|p| match p.as_str() {
            Some("target") => rich_value(&target),
            Some("content") => rich_value(content),
            _ => rich_value(&name),
        })
        .collect();
    let mut value = definition
        .get("style")
        .cloned()
        .unwrap_or_else(|| json!({}));
    ensure!(value.is_object(), "Invalid chat style");
    value["translate"] = definition["translation_key"].clone();
    value["with"] = json!(args);
    RichText::from_json(&value)
}
impl State {
    fn packed(&self, input: &mut &[u8]) -> Result<Signature> {
        let n = read_varint(input)?;
        ensure!((0..=128).contains(&n), "Invalid packed chat signature");
        if n == 0 {
            signature(input)
        } else {
            self.cache[n as usize - 1].context("Unknown cached chat signature")
        }
    }
    fn push(&mut self, seen: Vec<Signature>, signature: Option<Signature>) {
        let mut queue: VecDeque<_> = seen.into();
        if let Some(sig) = signature {
            queue.push_back(sig);
        }
        let incoming: HashSet<_> = queue.iter().copied().collect();
        for entry in &mut self.cache {
            let Some(next) = queue.pop_back() else {
                break;
            };
            let old = entry.replace(next);
            if let Some(old) = old.filter(|sig| !incoming.contains(sig)) {
                queue.push_front(old);
            }
        }
    }
    pub fn player(&mut self, mut input: &[u8], registries: &Registries) -> Result<Message> {
        read_varint(&mut input)?;
        bytes(&mut input, 16)?;
        read_varint(&mut input)?;
        let signature = if boolean(&mut input)? {
            Some(signature(&mut input)?)
        } else {
            None
        };
        let raw = read_string(&mut input, 256)?;
        bytes(&mut input, 16)?; // timestamp and salt
        let count = read_varint(&mut input)?;
        ensure!((0..=20).contains(&count), "Too many last-seen references");
        let mut seen = Vec::new();
        for _ in 0..count {
            seen.push(self.packed(&mut input)?);
        }
        let unsigned = if boolean(&mut input)? {
            Some(component::read_rich(&mut input)?)
        } else {
            None
        };
        let filter = read_varint(&mut input)?;
        ensure!((0..=2).contains(&filter), "Invalid chat filter mask");
        let mut mask = Vec::new();
        if filter == 2 {
            let n = read_varint(&mut input)?;
            ensure!((0..=1024).contains(&n), "Chat filter mask too large");
            for _ in 0..n {
                mask.push(u64::from_be_bytes(bytes(&mut input, 8)?.try_into()?));
            }
        }
        let content = if filter == 2 {
            let units: Vec<_> = raw
                .encode_utf16()
                .enumerate()
                .map(|(i, c)| {
                    if mask.get(i / 64).is_some_and(|m| m & (1 << (i % 64)) != 0) {
                        b'#' as u16
                    } else {
                        c
                    }
                })
                .collect();
            RichText::plain(&String::from_utf16_lossy(&units))
        } else {
            unsigned.unwrap_or_else(|| RichText::plain(&raw))
        };
        let text = bound(&mut input, registries, &content)?;
        ensure!(input.is_empty(), "Trailing player chat bytes");
        self.push(seen, signature);
        Ok(Message {
            text: if filter == 1 { None } else { Some(text) },
            signature,
            content,
        })
    }
    pub fn disguised(&self, mut input: &[u8], registries: &Registries) -> Result<RichText> {
        let content = component::read_rich(&mut input)?;
        let text = bound(&mut input, registries, &content)?;
        ensure!(input.is_empty(), "Trailing disguised chat");
        Ok(text)
    }
    pub fn deleted(&self, mut input: &[u8]) -> Result<Signature> {
        let sig = self.packed(&mut input)?;
        ensure!(input.is_empty(), "Trailing delete chat");
        Ok(sig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn original_player_disguised_filtered_and_deleted_chat_payloads_decode() {
        let fixture = super::super::last_seen::tests::reference();
        let packets = fixture["packets"].as_array().unwrap();
        let body = |index: usize| {
            base64::engine::general_purpose::STANDARD
                .decode(packets[index]["body"].as_str().unwrap())
                .unwrap()
        };
        let mut state = State::default();
        let registries = Registries::default();
        let unsigned = state.player(&body(8), &registries).unwrap();
        assert_eq!(unsigned.content.plain_text(), "Hello");
        assert!(unsigned.signature.is_none());
        assert!(unsigned.text.is_some());
        let signed = state.player(&body(9), &registries).unwrap();
        assert_eq!(signed.content.plain_text(), "Unsigned override");
        assert_eq!(signed.signature, Some([7; 256]));
        state.disguised(&body(10), &registries).unwrap();
        assert!(state.player(&body(11), &registries).unwrap().text.is_none());
        assert_eq!(state.deleted(&body(12)).unwrap(), [7; 256]);
        assert_eq!(state.deleted(&[1]).unwrap(), [7; 256]);
        for index in [8, 9, 11] {
            let data = body(index);
            for end in 0..data.len() {
                assert!(
                    State::default().player(&data[..end], &registries).is_err(),
                    "packet {index}, byte {end}"
                );
            }
        }
    }
}
