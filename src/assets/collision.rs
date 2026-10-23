//! Collision data exported once from official 26.2 block states. The baseline
//! context is empty; dynamic/entity-sensitive rules require runtime handling.
use anyhow::{ensure, Context, Result};
use flate2::read::GzDecoder;
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{
    io::Read,
    sync::{Arc, OnceLock},
};
#[derive(Debug)]
pub struct StatePhysics {
    pub boxes: Arc<[[f64; 6]]>,
    pub friction: f32,
    pub speed: f32,
    pub jump: f32,
    pub dynamic: bool,
}
#[derive(Deserialize)]
struct Report {
    shapes: Vec<Vec<[f64; 6]>>,
    states: Vec<(usize, f32, f32, f32, bool)>,
}
pub struct Physics {
    pub states: Vec<StatePhysics>,
}
impl Physics {
    fn load() -> Result<Self> {
        Self::load_report(
            include_bytes!("../../data/26.2/collision.json.gz"),
            "collision.json",
        )
    }
    fn load_report(encoded: &[u8], hash_key: &str) -> Result<Self> {
        let mut bytes = Vec::new();
        GzDecoder::new(encoded)
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 8 * 1024 * 1024, "Collision report too large");
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../data/26.2/reference.json"))?;
        ensure!(
            format!("{:x}", Sha1::digest(&bytes))
                == reference["sha1"][hash_key]
                    .as_str()
                    .context("Missing collision hash")?,
            "Collision report hash mismatch"
        );
        let report: Report = serde_json::from_slice(&bytes)?;
        ensure!(
            report.states.len() == super::builtins::get()?.block_states.len(),
            "Collision/state count mismatch"
        );
        for shape in &report.shapes {
            for b in shape {
                ensure!(
                    b.iter().all(|v| v.is_finite() && v.abs() <= 16.)
                        && (0..3).all(|i| b[i] <= b[i + 3]),
                    "Invalid collision box"
                );
            }
        }
        let shapes: Vec<Arc<[[f64; 6]]>> = report.shapes.into_iter().map(Arc::from).collect();
        let states = report
            .states
            .into_iter()
            .map(|(shape, friction, speed, jump, dynamic)| {
                ensure!(
                    friction > 0.
                        && friction <= 1.
                        && (0. ..=1.).contains(&speed)
                        && (0. ..=1.).contains(&jump),
                    "Invalid block motion factor"
                );
                Ok(StatePhysics {
                    boxes: shapes
                        .get(shape)
                        .context("Invalid collision shape index")?
                        .clone(),
                    friction,
                    speed,
                    jump,
                    dynamic,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { states })
    }
    pub fn state(&self, id: u32) -> Option<&StatePhysics> {
        self.states.get(id as usize)
    }
}
pub fn get() -> Result<&'static Physics> {
    static DATA: OnceLock<Result<Physics, String>> = OnceLock::new();
    DATA.get_or_init(|| Physics::load().map_err(|error| format!("{error:#}")))
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))
}
/// Selection/interaction shapes differ from camera visual and collision shapes.
pub fn outline() -> Result<&'static Physics> {
    static DATA: OnceLock<Result<Physics, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        Physics::load_report(
            include_bytes!("../../data/26.2/outline.json.gz"),
            "outline.json",
        )
        .map_err(|e| format!("{e:#}"))
    })
    .as_ref()
    .map_err(|e| anyhow::anyhow!(e.clone()))
}
pub fn visual() -> Result<&'static Physics> {
    static DATA: OnceLock<Result<Physics, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        Physics::load_report(
            include_bytes!("../../data/26.2/camera_shapes.json.gz"),
            "camera_shapes.json",
        )
        .map_err(|e| format!("{e:#}"))
    })
    .as_ref()
    .map_err(|e| anyhow::anyhow!(e.clone()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_visual_shapes_differ_from_physical_glass_and_fence_shapes() {
        let ids = crate::assets::builtins::get().unwrap();
        let visual = visual().unwrap();
        let physical = get().unwrap();
        let glass = ids.default_state("minecraft:glass").unwrap();
        assert!(visual.state(glass).unwrap().boxes.is_empty());
        assert!(!physical.state(glass).unwrap().boxes.is_empty());
        let fence = ids.default_state("minecraft:oak_fence").unwrap();
        assert_eq!(visual.state(fence).unwrap().boxes[0][4], 1.);
        assert_eq!(physical.state(fence).unwrap().boxes[0][4], 1.5);
    }
    #[test]
    fn official_shapes_share_storage_and_preserve_slab_fence_and_ice_properties() {
        let ids = crate::assets::builtins::get().unwrap();
        let physics = get().unwrap();
        let state = |name| physics.state(ids.default_state(name).unwrap()).unwrap();
        assert!(state("minecraft:air").boxes.is_empty());
        assert_eq!(
            &*state("minecraft:stone").boxes,
            &[[0., 0., 0., 1., 1., 1.]]
        );
        assert!(Arc::ptr_eq(
            &state("minecraft:stone").boxes,
            &state("minecraft:dirt").boxes
        ));
        assert_eq!(state("minecraft:oak_slab").boxes[0][4], 0.5);
        assert_eq!(state("minecraft:oak_fence").boxes[0][4], 1.5);
        assert!(state("minecraft:ice").friction > state("minecraft:stone").friction);
        assert_eq!(physics.states.iter().filter(|s| s.dynamic).count(), 199);
    }
}
