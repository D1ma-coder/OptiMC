//! Vanilla block-destruction timing, driven at client ticks, independent of FPS.
use super::picking::BlockHit;
use crate::net::{
    items::{
        tool::{Blocks, Tool},
        Stack,
    },
    request::DigAction,
    SessionReport,
};
use anyhow::{ensure, Result};
use std::{
    io::Read,
    sync::{Arc, OnceLock},
};
#[derive(serde::Deserialize)]
pub struct Block {
    pub state: u32,
    pub block: u32,
    pub hardness: f32,
    pub tool: bool,
    pub air: bool,
    pub water: f32,
}
pub fn blocks() -> Result<&'static [Block]> {
    static DATA: OnceLock<Result<Vec<Block>, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        (|| -> Result<Vec<Block>> {
            use sha1::{Digest, Sha1};
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(
                include_bytes!("../../data/26.2/mining-reference.json.gz").as_slice(),
            )
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)?;
            ensure!(raw.len() <= 8 * 1024 * 1024, "Mining reference too large");
            let manifest: serde_json::Value =
                serde_json::from_str(include_str!("../../data/26.2/reference.json"))?;
            ensure!(
                format!("{:x}", Sha1::digest(&raw)) == manifest["sha1"]["mining-reference.json"],
                "Mining reference hash mismatch"
            );
            let r: serde_json::Value = serde_json::from_slice(&raw)?;
            let values: Vec<Block> = serde_json::from_value(r["states"].clone())?;
            let ids = crate::assets::builtins::get()?;
            ensure!(
                values.len() == ids.block_states.len(),
                "Mining state count mismatch"
            );
            for (i, b) in values.iter().enumerate() {
                ensure!(
                    b.state as usize == i
                        && b.hardness.is_finite()
                        && ids.state(b.state).is_some_and(
                            |s| ids.name("minecraft:block", b.block) == Some(s.block.as_ref())
                        ),
                    "Mining state mismatch"
                );
            }
            Ok(values)
        })()
        .map_err(|e| format!("{e:#}"))
    })
    .as_ref()
    .map(Vec::as_slice)
    .map_err(|e| anyhow::anyhow!(e.clone()))
}
fn matches(set: &Blocks, block: u32, report: &SessionReport) -> bool {
    match set {
        Blocks::Ids(ids) => ids.contains(&block),
        Blocks::Tag(tag) => report.tags.contains("minecraft:block", tag, block),
    }
}
fn tool_values(tool: Option<&Tool>, block: &Block, report: &SessionReport) -> (f32, bool) {
    let speed = tool.map_or(1., |tool| {
        tool.rules
            .iter()
            .find_map(|r| r.speed.filter(|_| matches(&r.blocks, block.block, report)))
            .unwrap_or(tool.default_speed)
    });
    let correct = !block.tool
        || tool.is_some_and(|tool| {
            tool.rules
                .iter()
                .find_map(|r| r.drops.filter(|_| matches(&r.blocks, block.block, report)))
                .unwrap_or(false)
        });
    (speed, correct)
}
pub fn progress(
    report: &SessionReport,
    state: u32,
    slot: u8,
    grounded: bool,
    eye: glam::DVec3,
) -> f32 {
    let Some(block) = blocks().ok().and_then(|b| b.get(state as usize)) else {
        return 0.;
    };
    if block.air || block.hardness < 0. {
        return 0.;
    }
    if block.hardness == 0. {
        return 1.;
    }
    let tool = report.inventory.main_hand(slot).and_then(Stack::tool);
    let (mut speed, correct) = tool_values(tool.as_ref(), block, report);
    let entity = report.world.player_id.unwrap_or(-1);
    let attr = |name: &str, fallback: f32| {
        report
            .attributes
            .value(entity, name)
            .map_or(fallback, |v| v as f32)
    };
    if speed > 1. {
        speed += attr("minecraft:mining_efficiency", 0.);
    }
    let haste = report
        .effects
        .amplifier(entity, "minecraft:haste")
        .into_iter()
        .chain(report.effects.amplifier(entity, "minecraft:conduit_power"))
        .max();
    if let Some(a) = haste {
        speed *= 1. + (a as f32 + 1.) * 0.2;
    }
    if let Some(a) = report.effects.amplifier(entity, "minecraft:mining_fatigue") {
        speed *= match a {
            0 => 0.3,
            1 => 0.09,
            2 => 0.0027,
            _ => 0.00081,
        };
    }
    speed *= attr("minecraft:block_break_speed", 1.);
    let pos = eye.floor().as_ivec3();
    if let Some(fluid) = report
        .world
        .block(pos.x, pos.y, pos.z)
        .and_then(|s| blocks().ok()?.get(s as usize))
    {
        if eye.y - (pos.y as f64) < f64::from(fluid.water) && fluid.water > 0. {
            speed *= attr("minecraft:submerged_mining_speed", 0.2);
        }
    }
    if !grounded {
        speed /= 5.;
    }
    (speed / block.hardness / if correct { 30. } else { 100. }).max(0.)
}
struct Active {
    hit: BlockHit,
    state: u32,
    hand: Option<Arc<Stack>>,
    progress: f32,
}
#[derive(Default)]
pub struct Mining {
    active: Option<Active>,
    cooldown: u8,
    finished: Option<([i32; 3], u32)>,
    generation: Option<(u64, u64)>,
}
fn same_hand(a: Option<&Arc<Stack>>, b: Option<&Arc<Stack>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            Arc::ptr_eq(a, b)
                || (a.item == b.item && a.components == b.components && a.removed == b.removed)
        }
        _ => false,
    }
}
impl Mining {
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }
    pub fn generation(&mut self, world: u64, teleport: u64) {
        if self.generation != Some((world, teleport)) {
            *self = Self {
                generation: Some((world, teleport)),
                ..Default::default()
            };
        }
    }
    pub fn abort(&mut self) -> Option<(DigAction, BlockHit)> {
        self.finished = None;
        self.active.take().map(|a| (DigAction::Abort, a.hit))
    }
    pub fn update(
        &mut self,
        hit: BlockHit,
        state: u32,
        hand: Option<Arc<Stack>>,
        delta: f32,
        creative: bool,
        advance: bool,
    ) -> Vec<(DigAction, BlockHit)> {
        let mut out = Vec::new();
        if advance && self.cooldown > 0 {
            self.cooldown -= 1;
            return out;
        }
        if !advance {
            self.finished = None;
        }
        if self.finished == Some((hit.block, state)) {
            return out;
        }
        if self.active.as_ref().is_some_and(|a| {
            a.hit.block != hit.block
                || a.state != state
                || !same_hand(a.hand.as_ref(), hand.as_ref())
        }) {
            if let Some(abort) = self.abort() {
                out.push(abort);
            }
        }
        if self.active.is_none() {
            out.push((DigAction::Start, hit));
            if creative || delta >= 1. {
                self.finished = Some((hit.block, state));
                self.cooldown = 5;
            } else {
                self.active = Some(Active {
                    hit,
                    state,
                    hand,
                    progress: 0.,
                });
            }
        } else if advance {
            let active = self.active.as_mut().unwrap();
            active.progress += delta;
            if active.progress >= 1. {
                out.push((DigAction::Stop, hit));
                self.active = None;
                self.finished = Some((hit.block, state));
                self.cooldown = 5;
            }
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn hit(x: i32) -> BlockHit {
        BlockHit {
            block: [x, 1, 2],
            position: glam::DVec3::new(x as f64, 1., 2.),
            face: 1,
            inside: false,
            border: false,
        }
    }
    #[test]
    fn start_accumulate_finish_abort_and_tool_change_follow_client_ticks() {
        let mut mining = Mining::default();
        mining.generation(1, 1);
        assert_eq!(
            mining.update(hit(0), 1, None, 0.25, false, false)[0].0,
            DigAction::Start
        );
        for _ in 0..3 {
            assert!(mining.update(hit(0), 1, None, 0.25, false, true).is_empty());
        }
        assert_eq!(
            mining.update(hit(0), 1, None, 0.25, false, true)[0].0,
            DigAction::Stop
        );
        for _ in 0..20 {
            assert!(mining.update(hit(0), 1, None, 0.25, false, true).is_empty());
        }
        assert_eq!(
            mining.update(hit(1), 1, None, 0.25, false, true)[0].0,
            DigAction::Start
        );
        let switched = mining.update(hit(2), 1, None, 0.25, false, true);
        assert_eq!(
            switched.iter().map(|a| a.0).collect::<Vec<_>>(),
            [DigAction::Abort, DigAction::Start]
        );
        assert_eq!(mining.abort().unwrap().0, DigAction::Abort);
        assert!(mining.abort().is_none());
        assert_eq!(
            mining.update(hit(1), 1, None, 1., false, false)[0].0,
            DigAction::Start
        );
        assert!(mining.active.is_none());
        mining.generation(2, 2);
        assert_eq!(
            mining.update(hit(1), 1, None, 0., true, false)[0].0,
            DigAction::Start
        );
        assert!(mining.active.is_none());
    }
    #[test]
    fn official_hardness_and_tool_rules_distinguish_hand_pickaxe_and_server_overrides() {
        let ids = crate::assets::builtins::get().unwrap();
        let state = ids.default_state("minecraft:stone").unwrap();
        let block = &blocks().unwrap()[state as usize];
        assert_eq!(block.hardness, 1.5);
        assert!(block.tool);
        let report = SessionReport::default();
        assert_eq!(tool_values(None, block, &report), (1., false));
        let tool = Tool {
            rules: vec![crate::net::items::tool::Rule {
                blocks: Blocks::Ids(vec![block.block]),
                speed: Some(12.5),
                drops: Some(true),
            }],
            default_speed: 1.,
            damage_per_block: 1,
            creative: true,
        };
        assert_eq!(tool_values(Some(&tool), block, &report), (12.5, true));
        assert!((progress(&report, state, 0, true, glam::DVec3::ZERO) - 1. / 150.).abs() < 1e-6);
        assert!((progress(&report, state, 0, false, glam::DVec3::ZERO) - 1. / 750.).abs() < 1e-6);
    }
}
