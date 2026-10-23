//! Server-authorized sign editor and the four-line update packet.
use super::{codec::string, component, nbt::Tag, spawn, SessionReport};
use anyhow::{ensure, Result};
#[derive(Clone, Debug)]
pub struct Open {
    pub serial: u64,
    pub position: [i32; 3],
    pub front: bool,
    pub hanging: bool,
    pub wall: bool,
    pub wood: String,
    pub color: u32,
    pub lines: [String; 4],
}
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub serial: u64,
    pub open: Option<Open>,
}
pub fn is_sign(report: &SessionReport, position: [i32; 3]) -> Result<Option<bool>> {
    let Some(id) = report.world.block(position[0], position[1], position[2]) else {
        return Ok(None);
    };
    let ids = crate::assets::builtins::get()?;
    let Some(state) = ids.state(id) else {
        return Ok(None);
    };
    if !state.block.ends_with("_sign") {
        return Ok(None);
    }
    if let Some(entity) = report.world.block_entities.get(&position) {
        if !matches!(
            ids.name("minecraft:block_entity_type", entity.kind),
            Some("minecraft:sign" | "minecraft:hanging_sign")
        ) {
            return Ok(None);
        }
    }
    Ok(Some(state.block.contains("hanging_sign")))
}
pub fn open(report: &mut SessionReport, input: &[u8]) -> Result<()> {
    ensure!(input.len() == 9, "Invalid open sign editor length");
    let position = spawn::block_position(i64::from_be_bytes(input[..8].try_into()?));
    let front = input[8] != 0;
    let Some(hanging) = is_sign(report, position)? else {
        return Ok(());
    };
    let side = report
        .world
        .block_entities
        .get(&position)
        .and_then(|e| e.data.get(if front { "front_text" } else { "back_text" }));
    let messages = side.and_then(|side| side.get("messages"));
    let mut lines = std::array::from_fn(|_| String::new());
    if let Some(Tag::List { values, .. }) = messages {
        for (line, value) in lines.iter_mut().zip(values) {
            *line = crate::text::RichText::from_json(&component::json_tag(value))?.plain_text();
        }
    }
    let ids = crate::assets::builtins::get()?;
    let state = ids
        .state(
            report
                .world
                .block(position[0], position[1], position[2])
                .unwrap(),
        )
        .unwrap();
    let name = state
        .block
        .strip_prefix("minecraft:")
        .unwrap_or(&state.block);
    let wall = name.contains("wall_sign") || name.contains("wall_hanging_sign");
    let wood = ["_wall_hanging_sign", "_hanging_sign", "_wall_sign", "_sign"]
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .unwrap_or("oak")
        .to_owned();
    let dye = side
        .and_then(|v| v.get("color"))
        .and_then(Tag::string)
        .map(|v| v.to_string_lossy())
        .unwrap_or_else(|| "black".into());
    let glow = side
        .and_then(|v| v.get("has_glowing_text"))
        .and_then(Tag::integer)
        .unwrap_or(0)
        != 0;
    let mut color = crate::assets::signs::reference()?["dyes"][dye]
        .as_i64()
        .unwrap_or(-16777216) as u32
        & 0xffffff;
    if !glow {
        color = (((color >> 16 & 255) as f32 * 0.4) as u32) << 16
            | (((color >> 8 & 255) as f32 * 0.4) as u32) << 8
            | ((color & 255) as f32 * 0.4) as u32;
    }
    let store = std::sync::Arc::make_mut(&mut report.sign_editor);
    store.serial = store.serial.wrapping_add(1);
    store.open = Some(Open {
        serial: store.serial,
        position,
        front,
        hanging,
        wall,
        wood,
        color,
        lines,
    });
    Ok(())
}
pub fn encode(position: [i32; 3], front: bool, lines: &[String; 4]) -> Result<Vec<u8>> {
    let [x, y, z] = position;
    ensure!(
        (-33_554_432..33_554_432).contains(&x)
            && (-33_554_432..33_554_432).contains(&z)
            && (-2048..2048).contains(&y),
        "Sign position exceeds packed range"
    );
    let packed = ((i64::from(x) & 0x3ffffff) << 38)
        | ((i64::from(z) & 0x3ffffff) << 12)
        | (i64::from(y) & 0xfff);
    let mut body = packed.to_be_bytes().to_vec();
    body.push(front as u8);
    for line in lines {
        ensure!(
            line.encode_utf16().count() <= 384,
            "Sign line exceeds 384 UTF-16 units"
        );
        string(line, &mut body);
    }
    Ok(body)
}
#[cfg(test)]
pub(crate) fn reference() -> serde_json::Value {
    use sha1::{Digest, Sha1};
    use std::io::Read;
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(include_bytes!("../../data/26.2/sign-editor.json.gz").as_slice())
        .read_to_end(&mut raw)
        .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha1::digest(&raw)),
        manifest["sha1"]["sign-editor.json"]
    );
    serde_json::from_slice(&raw).unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    #[test]
    fn original_open_and_update_packets_preserve_both_sides_and_utf16_limits() {
        use crate::world::remote::{Column, StoredSection, Values};
        use std::sync::Arc;
        let r = reference();
        for row in r["packets"].as_array().unwrap() {
            let p = std::array::from_fn(|i| row["position"][i].as_i64().unwrap() as i32);
            let lines = std::array::from_fn(|i| row["lines"][i].as_str().unwrap().to_owned());
            let front = row["front"].as_bool().unwrap();
            let update = base64::engine::general_purpose::STANDARD
                .decode(row["update"].as_str().unwrap())
                .unwrap();
            assert_eq!(encode(p, front, &lines).unwrap(), update);
            let request = super::super::request::Request::SignUpdate {
                serial: 7,
                position: p,
                front,
                lines,
            };
            request.validate().unwrap();
            assert_eq!(
                request.encode(0, 0, &mut Default::default()).unwrap(),
                (0x3d, update)
            );
            let mut report = SessionReport::default();
            let state = crate::assets::builtins::get()
                .unwrap()
                .default_state("minecraft:oak_sign")
                .unwrap();
            report.world.chunks.insert(
                (p[0].div_euclid(16), p[2].div_euclid(16)),
                Arc::new(Column {
                    sections: vec![Arc::new(StoredSection {
                        blocks: Values::Uniform {
                            value: state,
                            len: 4096,
                        },
                        biomes: Values::Uniform { value: 0, len: 64 },
                    })],
                }),
            );
            report.world.dimension = Some(super::super::registry::Dimension {
                name: "minecraft:overworld".into(),
                min_y: p[1].div_euclid(16) * 16,
                height: 16,
                logical_height: 16,
                skylight: true,
                ceiling: false,
            });
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["open"].as_str().unwrap())
                .unwrap();
            open(&mut report, &body).unwrap();
            let opened = report.sign_editor.open.as_ref().unwrap();
            assert_eq!(opened.position, p);
            assert_eq!(opened.front, front);
            assert_eq!(opened.wood, "oak");
            assert_eq!(opened.lines, ["", "", "", ""]);
            if p == [-31, -22, 47] {
                let source = crate::assets::signs::reference().unwrap();
                let body_nbt = base64::engine::general_purpose::STANDARD
                    .decode(source["update"].as_str().unwrap())
                    .unwrap();
                let entity = super::super::block_entity::update(&body_nbt).unwrap();
                report.world.block_entities.insert(p, Arc::new(entity));
                open(&mut report, &body).unwrap();
                let opened = report.sign_editor.open.as_ref().unwrap();
                assert_eq!(
                    opened.lines,
                    if front {
                        ["Привет", "Front", "", ""]
                    } else {
                        ["Back", "", "", ""]
                    }
                );
                if front {
                    assert_eq!(
                        opened.color,
                        source["dyes"]["red"].as_i64().unwrap() as u32 & 0xffffff
                    );
                }
            }
            let before = report.sign_editor.serial;
            for n in 0..body.len() {
                assert!(open(&mut report, &body[..n]).is_err());
                assert_eq!(report.sign_editor.serial, before);
            }
            let mut empty = SessionReport::default();
            open(&mut empty, &body).unwrap();
            assert!(empty.sign_editor.open.is_none());
        }
        assert!(encode(
            [0, 0, 0],
            true,
            &[
                "😀".repeat(193),
                String::new(),
                String::new(),
                String::new()
            ]
        )
        .is_err());
    }
}
