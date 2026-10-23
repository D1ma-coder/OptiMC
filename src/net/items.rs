//! ItemStack component patches shared by entity contents and server inventory.
pub mod tool;
use super::{
    codec::{read_string, read_varint},
    maps::{byte, bytes},
    nbt,
};
use anyhow::{ensure, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedComponent(pub String);
impl std::fmt::Display for UnsupportedComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unsupported item component {}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Read;
    #[test]
    fn books_and_nested_item_templates_match_original_jar_without_losing_bytes() {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/item-contents.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        for row in reference["stacks"].as_array().unwrap() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            let mut input = body.as_slice();
            let stack =
                Stack::read(&mut input).unwrap_or_else(|e| panic!("{}: {e:#}", row["label"]));
            assert!(input.is_empty(), "{}", row["label"]);
            assert_eq!(stack.item, row["item"].as_u64().unwrap() as u32);
            assert_eq!(stack.count, 1);
            assert_eq!(stack.components.len(), 1);
            // Recreate this fixture's exact ordinary stack header + patch.
            let mut roundtrip = Vec::new();
            super::super::codec::varint(stack.count, &mut roundtrip);
            super::super::codec::varint(stack.item as i32, &mut roundtrip);
            roundtrip.extend([1, 0]);
            for (id, bytes) in &stack.components {
                super::super::codec::varint(*id as i32, &mut roundtrip);
                roundtrip.extend(bytes);
            }
            assert_eq!(roundtrip, body, "{}", row["label"]);
            for len in 0..body.len() {
                assert!(
                    Stack::read(&mut &body[..len]).is_err(),
                    "{} truncation {len}",
                    row["label"]
                );
            }
        }
    }
    #[test]
    fn nested_items_cannot_exhaust_recursion_or_accept_negative_template_counts() {
        let bundle = component_id("minecraft:bundle_contents").unwrap() as i32;
        let item = super::super::item_defaults::get()
            .unwrap()
            .iter()
            .find(|item| item.name == "minecraft:bundle")
            .unwrap()
            .id as i32;
        let mut body = vec![1];
        super::super::codec::varint(item, &mut body);
        for _ in 0..66 {
            body.extend([1, 0]);
            super::super::codec::varint(bundle, &mut body);
            body.push(1);
            super::super::codec::varint(item, &mut body);
            body.push(1);
        }
        body.extend([0, 0]);
        assert!(Stack::read(&mut body.as_slice())
            .unwrap_err()
            .to_string()
            .contains("budget"));
        let mut nodes = 0;
        let mut template = Vec::new();
        super::super::codec::varint(item, &mut template);
        super::super::codec::varint(-1, &mut template);
        assert!(Stack::read_inner(&mut template.as_slice(), 0, true, &mut nodes).is_err());
    }
    #[test]
    fn supported_item_component_payloads_match_original_jar_codecs() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../data/26.2/maps-reference.json.gz")[..])
            .read_to_end(&mut bytes)
            .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for fixture in reference["components"].as_array().unwrap() {
            let mut packet = vec![1];
            crate::net::codec::varint(1133, &mut packet);
            packet.extend([1, 0]);
            let id = fixture["id"].as_i64().unwrap() as u32;
            crate::net::codec::varint(id as i32, &mut packet);
            let payload = base64::engine::general_purpose::STANDARD
                .decode(fixture["body"].as_str().unwrap())
                .unwrap();
            packet.extend(&payload);
            let mut input = packet.as_slice();
            let stack =
                Stack::read(&mut input).unwrap_or_else(|e| panic!("{}: {e}", fixture["name"]));
            assert!(input.is_empty(), "{}", fixture["name"]);
            assert_eq!(stack.components[&id], payload, "{}", fixture["name"]);
        }
    }
}
impl std::error::Error for UnsupportedComponent {}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stack {
    pub count: i32,
    pub item: u32,
    pub map: Option<i32>,
    /// Exact payloads for supported component codecs; no lossy item rewriting.
    pub components: BTreeMap<u32, Vec<u8>>,
    pub removed: Vec<u32>,
}
pub fn component_id(name: &str) -> Option<u32> {
    static IDS: std::sync::OnceLock<BTreeMap<String, u32>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        let ids = crate::assets::builtins::get().expect("official component registry");
        (0..1024)
            .filter_map(|id| {
                ids.name("minecraft:data_component_type", id)
                    .map(|name| (name.to_string(), id))
            })
            .collect()
    })
    .get(name)
    .copied()
}
impl Stack {
    pub fn component(&self, name: &str) -> Option<&[u8]> {
        if self.count <= 0 {
            return None;
        }
        let id = component_id(name)?;
        if self.removed.contains(&id) {
            return None;
        }
        self.components.get(&id).map(Vec::as_slice).or_else(|| {
            super::item_defaults::get()
                .ok()?
                .get(self.item as usize)?
                .raw(id)
        })
    }
    pub fn display_name(&self) -> crate::text::RichText {
        for name in ["minecraft:custom_name", "minecraft:item_name"] {
            if let Some(raw) = self.component(name) {
                if let Ok(text) = super::component::network_rich(raw) {
                    return text;
                }
            }
        }
        let key = super::item_defaults::get()
            .ok()
            .and_then(|items| items.get(self.item as usize))
            .map(|item| item.translation.as_str())
            .unwrap_or("item.unknown");
        crate::text::RichText::from_json(&serde_json::json!({"translate":key})).unwrap_or_default()
    }
    pub fn maximum(&self) -> i32 {
        self.component("minecraft:max_stack_size")
            .and_then(|mut bytes| read_varint(&mut bytes).ok())
            .unwrap_or(1)
    }
    pub fn tool(&self) -> Option<tool::Tool> {
        let mut bytes = self.component("minecraft:tool")?;
        let tool = tool::Tool::read(&mut bytes).ok()?;
        bytes.is_empty().then_some(tool)
    }
    pub fn read(input: &mut &[u8]) -> Result<Self> {
        Self::read_inner(input, 0, false, &mut 0)
    }
    fn read_inner(
        input: &mut &[u8],
        depth: usize,
        template: bool,
        nodes: &mut usize,
    ) -> Result<Self> {
        ensure!(depth <= 64 && *nodes < 65536, "Nested item budget exceeded");
        *nodes += 1;
        // ItemStackTemplate is item-first; ordinary network stacks are count-first.
        let (item, count) = if template {
            (read_varint(input)?, read_varint(input)?)
        } else {
            let count = read_varint(input)?;
            ensure!(count >= 0, "Negative item count");
            if count == 0 {
                return Ok(Self::default());
            }
            (read_varint(input)?, count)
        };
        ensure!(count > 0 && item >= 0, "Invalid item template/count/ID");
        let builtins = crate::assets::builtins::get()?;
        ensure!(
            builtins.name("minecraft:item", item as u32).is_some(),
            "Unknown item ID"
        );
        let added = read_varint(input)?;
        let removed = read_varint(input)?;
        ensure!(
            (0..=256).contains(&added) && (0..=256).contains(&removed),
            "Item component count exceeds bound"
        );
        let mut stack = Self {
            count,
            item: item as u32,
            ..Default::default()
        };
        for _ in 0..added {
            let id = read_varint(input)?;
            ensure!(id >= 0, "Invalid component ID");
            let name = builtins
                .name("minecraft:data_component_type", id as u32)
                .ok_or_else(|| UnsupportedComponent(format!("id {id}")))?;
            let start = *input;
            match name.strip_prefix("minecraft:").unwrap_or(name) {
                "writable_book_content" => {
                    for _ in 0..tool::count(input, 100)? {
                        read_string(input, 1024)?;
                        if byte(input)? != 0 {
                            read_string(input, 1024)?;
                        }
                    }
                }
                "written_book_content" => {
                    read_string(input, 32)?;
                    if byte(input)? != 0 {
                        read_string(input, 32)?;
                    }
                    read_string(input, 32767)?;
                    read_varint(input)?;
                    for _ in 0..tool::count(input, 65536)? {
                        nbt::read_network(input)?;
                        if byte(input)? != 0 {
                            nbt::read_network(input)?;
                        }
                    }
                    byte(input)?;
                }
                "container" | "bundle_contents" | "charged_projectiles" => {
                    let optional = name == "minecraft:container";
                    let limit = if optional {
                        256
                    } else if name == "minecraft:charged_projectiles" {
                        1024
                    } else {
                        65536
                    };
                    for _ in 0..tool::count(input, limit)? {
                        if !optional || byte(input)? != 0 {
                            Self::read_inner(input, depth + 1, true, nodes)?;
                        }
                    }
                }
                "custom_model_data" => {
                    for _ in 0..tool::count(input, 65536)? {
                        bytes(input, 4)?;
                    }
                    for _ in 0..tool::count(input, 65536)? {
                        byte(input)?;
                    }
                    for _ in 0..tool::count(input, 65536)? {
                        read_string(input, 32767)?;
                    }
                    for _ in 0..tool::count(input, 65536)? {
                        bytes(input, 4)?;
                    }
                }
                "profile" => {
                    profile(input)?;
                }
                "potion_contents" => {
                    if byte(input)? != 0 {
                        read_varint(input)?;
                    }
                    if byte(input)? != 0 {
                        bytes(input, 4)?;
                    }
                    for _ in 0..tool::count(input, 65536)? {
                        read_varint(input)?;
                        effect_details(input, 0)?;
                    }
                    if byte(input)? != 0 {
                        read_string(input, 32767)?;
                    }
                }
                "potion_duration_scale" => {
                    bytes(input, 4)?;
                }
                "tool" => {
                    tool::Tool::read(input)?;
                }
                "attack_range" => {
                    for _ in 0..6 {
                        tool::float(input)?;
                    }
                }
                "enchantments" | "stored_enchantments" => {
                    let n = tool::count(input, 4096)?;
                    for _ in 0..n {
                        read_varint(input)?;
                        read_varint(input)?;
                    }
                }
                "attribute_modifiers" => {
                    let n = tool::count(input, 4096)?;
                    for _ in 0..n {
                        read_varint(input)?;
                        super::registry::identifier(input)?;
                        bytes(input, 8)?;
                        read_varint(input)?;
                        read_varint(input)?;
                        match read_varint(input)? {
                            0 | 1 => {}
                            2 => {
                                nbt::read_network(input)?;
                            }
                            _ => {
                                return Err(UnsupportedComponent(
                                    "attribute modifier display".into(),
                                )
                                .into())
                            }
                        }
                    }
                }
                "tooltip_display" => {
                    byte(input)?;
                    let n = tool::count(input, 256)?;
                    for _ in 0..n {
                        read_varint(input)?;
                    }
                }
                "repairable" => {
                    let n = read_varint(input)?;
                    ensure!((0..=100001).contains(&n), "Invalid repairable holder set");
                    if n == 0 {
                        super::registry::identifier(input)?;
                    } else {
                        for _ in 1..n {
                            read_varint(input)?;
                        }
                    }
                }
                "food" => {
                    read_varint(input)?;
                    tool::float(input)?;
                    byte(input)?;
                }
                "map_id" => stack.map = Some(read_varint(input)?),
                "max_stack_size"
                | "max_damage"
                | "damage"
                | "repair_cost"
                | "enchantable"
                | "rarity"
                | "map_post_processing"
                | "ominous_bottle_amplifier" => {
                    read_varint(input)?;
                }
                "map_color" | "dyed_color" => {
                    bytes(input, 4)?;
                }
                "block_entity_data" | "entity_data" => {
                    read_varint(input)?;
                    nbt::read_network(input)?;
                }
                "bucket_entity_data"
                | "custom_data"
                | "custom_name"
                | "item_name"
                | "map_decorations"
                | "intangible_projectile" => {
                    nbt::read_network(input)?;
                }
                "lore" => {
                    let count = read_varint(input)?;
                    ensure!((0..=256).contains(&count), "Too many lore lines");
                    for _ in 0..count {
                        nbt::read_network(input)?;
                    }
                }
                "item_model" | "tooltip_style" | "note_block_sound" => {
                    read_string(input, 32767)?;
                }
                "enchantment_glint_override" => {
                    byte(input)?;
                }
                "unbreakable"
                | "creative_slot_lock"
                | "hide_additional_tooltip"
                | "hide_tooltip" => {}
                _ => return Err(UnsupportedComponent(name.into()).into()),
            }
            ensure!(
                !stack.components.contains_key(&(id as u32)),
                "Duplicate added item component"
            );
            stack
                .components
                .insert(id as u32, start[..start.len() - input.len()].to_vec());
        }
        for _ in 0..removed {
            let id = read_varint(input)?;
            ensure!(
                id >= 0
                    && builtins
                        .name("minecraft:data_component_type", id as u32)
                        .is_some(),
                "Invalid removed item component"
            );
            ensure!(
                !stack.components.contains_key(&(id as u32))
                    && !stack.removed.contains(&(id as u32)),
                "Conflicting item component patch"
            );
            stack.removed.push(id as u32);
        }
        Ok(stack)
    }
}

// ResolvableProfile uses Either<GameProfile, Partial> followed by PlayerSkin.Patch.
fn profile(input: &mut &[u8]) -> Result<()> {
    if byte(input)? != 0 {
        bytes(input, 16)?;
        read_string(input, 16)?;
    } else {
        if byte(input)? != 0 {
            read_string(input, 16)?;
        }
        if byte(input)? != 0 {
            bytes(input, 16)?;
        }
    }
    for _ in 0..tool::count(input, 1024)? {
        read_string(input, 32767)?;
        read_string(input, 32767)?;
        if byte(input)? != 0 {
            read_string(input, 32767)?;
        }
    }
    for _ in 0..3 {
        if byte(input)? != 0 {
            super::registry::identifier(input)?;
        }
    }
    if byte(input)? != 0 {
        read_varint(input)?;
    }
    Ok(())
}
fn effect_details(input: &mut &[u8], depth: usize) -> Result<()> {
    ensure!(depth < 64, "Nested potion effect budget exceeded");
    read_varint(input)?;
    read_varint(input)?;
    bytes(input, 3)?;
    if byte(input)? != 0 {
        effect_details(input, depth + 1)?;
    }
    Ok(())
}
