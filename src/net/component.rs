//! Plain-text projection of server components. Styling/events are retained by
//! callers in their original packet; disconnect reasons must never become hex.
use super::nbt::{self, Tag};
use anyhow::{ensure, Result};
const MAX_TEXT: usize = 32767;
fn append(tag: &Tag, out: &mut String, depth: usize) -> Result<()> {
    ensure!(depth <= 128, "Text component too deep");
    match tag {
        Tag::String(text) => out.push_str(&text.to_string_lossy()),
        Tag::List { values, .. } => {
            for value in values {
                append(value, out, depth + 1)?;
            }
        }
        Tag::Compound(_) => {
            if let Some(text) = tag.get("text") {
                append(text, out, depth + 1)?;
            } else if let Some(key) = tag.get("translate").and_then(Tag::string) {
                let key = key.to_string_lossy();
                let template = tag
                    .get("fallback")
                    .and_then(Tag::string)
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_else(|| key.clone());
                out.push_str(&template);
                if let Some(Tag::List { values, .. }) = tag.get("with") {
                    for value in values {
                        out.push(' ');
                        append(value, out, depth + 1)?;
                    }
                }
            } else if let Some(text) = tag.get("keybind").or_else(|| tag.get("selector")) {
                append(text, out, depth + 1)?;
            }
            if let Some(extra) = tag.get("extra") {
                append(extra, out, depth + 1)?;
            }
        }
        _ => {}
    }
    ensure!(out.chars().count() <= MAX_TEXT, "Text component too long");
    Ok(())
}
pub fn network(bytes: &[u8]) -> Result<String> {
    let mut input = bytes;
    let tag = nbt::read_network(&mut input)?;
    ensure!(input.is_empty(), "Trailing text component bytes");
    let mut text = String::new();
    append(&tag, &mut text, 0)?;
    ensure!(!text.is_empty(), "Empty server disconnect reason");
    Ok(text)
}
#[cfg(test)]
mod tests {
    use super::super::nbt::NbtString;
    use super::*;
    #[test]
    fn styled_extra_lists_and_unicode_survive_disconnect_decoding() {
        let component = Tag::Compound(
            [
                (NbtString::from("text"), Tag::String("Сервер: ".into())),
                (
                    NbtString::from("extra"),
                    Tag::List {
                        element_type: 10,
                        values: vec![Tag::Compound(
                            [
                                (
                                    NbtString::from("text"),
                                    Tag::String("Проверка входа\nПовторите позже".into()),
                                ),
                                (NbtString::from("bold"), Tag::Byte(1)),
                            ]
                            .into(),
                        )],
                    },
                ),
            ]
            .into(),
        );
        let mut bytes = Vec::new();
        nbt::write_network(&component, &mut bytes).unwrap();
        assert_eq!(
            network(&bytes).unwrap(),
            "Сервер: Проверка входа\nПовторите позже"
        );
        for end in 0..bytes.len() {
            assert!(network(&bytes[..end]).is_err());
        }
    }
}

#[derive(Clone, Debug)]
pub struct Disconnect(pub crate::text::RichText);
impl std::fmt::Display for Disconnect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.plain_text())
    }
}
impl std::error::Error for Disconnect {}
pub fn json_rich(input: &str) -> Result<crate::text::RichText> {
    crate::text::RichText::from_json(&serde_json::from_str::<serde_json::Value>(input)?)
}
pub fn network_rich(bytes: &[u8]) -> Result<crate::text::RichText> {
    let mut input = bytes;
    let text = read_rich(&mut input)?;
    ensure!(input.is_empty(), "Trailing text component bytes");
    Ok(text)
}
pub fn read_rich(input: &mut &[u8]) -> Result<crate::text::RichText> {
    let tag = nbt::read_network(input)?;
    crate::text::RichText::from_json(&json_tag(&tag))
}
pub(crate) fn read_style(input: &mut &[u8]) -> Result<crate::text::Style> {
    let tag = nbt::read_network(input)?;
    Ok(crate::text::apply_style(
        &json_tag(&tag),
        Default::default(),
    ))
}
pub(crate) fn json_tag(tag: &Tag) -> serde_json::Value {
    use serde_json::{json, Value};
    match tag {
        Tag::String(s) => Value::String(s.to_string_lossy()),
        Tag::Byte(n) => json!(*n),
        Tag::Short(n) => json!(*n),
        Tag::Int(n) => json!(*n),
        Tag::Long(n) => json!(*n),
        Tag::Float(n) => json!(*n),
        Tag::Double(n) => json!(*n),
        Tag::Compound(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.to_string_lossy(), json_tag(v)))
                .collect(),
        ),
        Tag::List { values, .. } => Value::Array(values.iter().map(json_tag).collect()),
        _ => Value::Null,
    }
}
