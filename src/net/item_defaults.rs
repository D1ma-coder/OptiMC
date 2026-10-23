//! Vanilla component defaults exported from the exact client after initializer
//! binding. Server patches override these; server registry-default rebinding is
//! a separate unfinished path, so dynamic raw IDs are not synthesized here.
use anyhow::{ensure, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Read, sync::OnceLock};
#[derive(Deserialize)]
pub struct Item {
    pub id: u32,
    pub name: String,
    pub translation: String,
    pub maximum: i32,
    pub values: BTreeMap<String, serde_json::Value>,
    components: BTreeMap<u32, String>,
    #[serde(skip)]
    raw: BTreeMap<u32, Vec<u8>>,
}
pub fn get() -> Result<&'static [Item]> {
    static DATA: OnceLock<Result<Vec<Item>, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        (|| -> Result<Vec<Item>> {
            use base64::Engine;
            use sha1::{Digest, Sha1};
            let mut bytes = Vec::new();
            flate2::read::GzDecoder::new(
                include_bytes!("../../data/26.2/item-defaults.json.gz").as_slice(),
            )
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 16 * 1024 * 1024,
                "Item defaults report too large"
            );
            let manifest: serde_json::Value =
                serde_json::from_str(include_str!("../../data/26.2/reference.json"))?;
            ensure!(
                format!("{:x}", Sha1::digest(&bytes))
                    == manifest["sha1"]["item-defaults.json"]
                        .as_str()
                        .unwrap_or(""),
                "Item defaults hash mismatch"
            );
            let mut items: Vec<Item> = serde_json::from_slice(&bytes)?;
            ensure!(items.len() == 1537, "Item registry/default count mismatch");
            let ids = super::super::assets::builtins::get()?;
            for (index, item) in items.iter_mut().enumerate() {
                ensure!(
                    index == item.id as usize
                        && ids.name("minecraft:item", item.id) == Some(item.name.as_str()),
                    "Item ID/default mismatch"
                );
                for (id, raw) in &item.components {
                    item.raw
                        .insert(*id, base64::engine::general_purpose::STANDARD.decode(raw)?);
                }
            }
            Ok(items)
        })()
        .map_err(|e| format!("{e:#}"))
    })
    .as_ref()
    .map(|v| v.as_slice())
    .map_err(|e| anyhow::anyhow!(e.clone()))
}
impl Item {
    pub fn raw(&self, id: u32) -> Option<&[u8]> {
        self.raw.get(&id).map(Vec::as_slice)
    }
}
