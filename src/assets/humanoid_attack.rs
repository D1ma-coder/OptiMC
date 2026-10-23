//! HumanoidModel.setupAttackAnimation's ordinary WHACK branch. Other animation
//! types and entity-specific setupAnim overrides must be supplied separately.
use glam::{Mat4, Vec3};
pub const NAMES: [&str; 4] = ["head", "body", "right_arm", "left_arm"];
fn table() -> &'static [f32] {
    static TABLE: std::sync::OnceLock<Box<[f32]>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        (0..65536)
            .map(|i| (i as f64 / 10430.378350470453).sin() as f32)
            .collect()
    })
}
fn sin(v: f32) -> f32 {
    table()[(((v as f64 * 10430.378350470453) as i64) & 65535) as usize]
}
fn cos(v: f32) -> f32 {
    table()[(((v as f64 * 10430.378350470453 + 16384.) as i64) & 65535) as usize]
}
pub fn pose(parts: &mut [[f32; 9]; 4], progress: f32, left: bool, head_pitch: f32, age_scale: f32) {
    if progress <= 0. {
        return;
    }
    let turn = sin(progress.sqrt() * std::f32::consts::TAU) * 0.2 * if left { -1. } else { 1. };
    parts[1][4] = turn;
    parts[2][2] = sin(turn) * 5. * age_scale;
    parts[2][0] = -cos(turn) * 5. * age_scale;
    parts[3][2] = -sin(turn) * 5. * age_scale;
    parts[3][0] = cos(turn) * 5. * age_scale;
    parts[2][4] += turn;
    parts[3][4] += turn;
    parts[3][3] += turn;
    let eased = 1. - (1. - progress).powi(4);
    let pull = sin(eased * std::f32::consts::PI);
    let aim = sin(progress * std::f32::consts::PI) * -(head_pitch - 0.7) * 0.75;
    let arm = if left { 3 } else { 2 };
    parts[arm][3] -= pull * 1.2 + aim;
    parts[arm][4] += turn * 2.;
    parts[arm][5] += sin(progress * std::f32::consts::PI) * -0.4;
}
pub fn matrix(t: [f32; 9]) -> Mat4 {
    Mat4::from_translation(Vec3::new(t[0], t[1], t[2]) / 16.)
        * Mat4::from_rotation_z(t[5])
        * Mat4::from_rotation_y(t[4])
        * Mat4::from_rotation_x(t[3])
        * Mat4::from_scale(Vec3::new(t[6], t[7], t[8]))
}
#[derive(Clone)]
pub struct Skeleton {
    pub root: Mat4,
    pub parts: [[f32; 9]; 4],
}
impl Skeleton {
    pub fn transforms(&self, progress: f32, left: bool, pitch: f32) -> [Mat4; 4] {
        let mut posed = self.parts;
        pose(&mut posed, progress, left, pitch, 1.);
        std::array::from_fn(|i| {
            self.root * matrix(posed[i]) * (self.root * matrix(self.parts[i])).inverse()
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    fn every_original_whack_pose_matches_both_arms_pitch_and_age_scale() {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../data/26.2/humanoid-attack.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
        use sha1::{Digest, Sha1};
        assert_eq!(
            format!("{:x}", Sha1::digest(&bytes)),
            reference["sha1"]["humanoid-attack.json"]
        );
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(rows.len(), 272);
        for row in rows {
            let mut parts: [[f32; 9]; 4] = serde_json::from_value(row["initial"].clone()).unwrap();
            pose(
                &mut parts,
                row["progress"].as_f64().unwrap() as f32,
                row["left"].as_bool().unwrap(),
                row["pitch"].as_f64().unwrap() as f32,
                row["scale"].as_f64().unwrap() as f32,
            );
            let expected: [[f32; 9]; 4] = serde_json::from_value(row["expected"].clone()).unwrap();
            for (actual, wanted) in parts.iter().flatten().zip(expected.iter().flatten()) {
                assert!(
                    (actual - wanted).abs() < 0.00001,
                    "{actual} != {wanted}; {row}"
                );
            }
        }
    }
}
