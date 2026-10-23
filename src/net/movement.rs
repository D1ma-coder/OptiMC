//! 26.2 LocalPlayer sendPosition/input/sprint/slot packet selection.
use super::{codec::varint, entity::Pose};
use anyhow::{ensure, Result};
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub world_epoch: u64,
    pub teleport_version: u64,
    pub abilities_version: u64,
    pub velocity_version: u64,
    pub player_state_version: u64,
    pub pose: Pose,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    /// forward/back/left/right/jump/shift/sprint, in that bit order.
    pub input: u8,
    pub sprinting: bool,
    pub slot: u8,
    pub flying: bool,
}
impl Motion {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.pose.position.is_finite()
                && self.pose.velocity.is_finite()
                && self.pose.yaw.is_finite()
                && self.pose.pitch.is_finite(),
            "Non-finite player movement"
        );
        ensure!(
            self.slot < 9 && self.input & 128 == 0,
            "Invalid player input/slot"
        );
        Ok(())
    }
}
pub struct Sender {
    position: glam::DVec3,
    rotation: [f32; 2],
    flags: u8,
    reminder: u8,
    input: u8,
    sprinting: bool,
    slot: u8,
}
impl Sender {
    pub fn new(pose: Pose) -> Self {
        Self {
            position: pose.position,
            rotation: [pose.yaw, pose.pitch],
            flags: 0,
            reminder: 0,
            input: 0,
            sprinting: false,
            slot: 0,
        }
    }
    /// Actions flush the current selected item and sneak/input before using it.
    pub fn sync_input_slot(&mut self, slot: u8, input: u8) -> Result<Vec<(i32, Vec<u8>)>> {
        ensure!(slot < 9 && input & 128 == 0, "Invalid player input/slot");
        let mut packets = Vec::new();
        if slot != self.slot {
            packets.push((0x35, (slot as i16).to_be_bytes().to_vec()));
            self.slot = slot;
        }
        if input != self.input {
            packets.push((0x2b, vec![input]));
            self.input = input;
        }
        Ok(packets)
    }
    pub fn tick(&mut self, motion: Motion, player_id: i32) -> Result<Vec<(i32, Vec<u8>)>> {
        motion.validate()?;
        let mut packets = Vec::with_capacity(4);
        packets.extend(self.sync_input_slot(motion.slot, motion.input)?);
        if motion.sprinting != self.sprinting {
            let mut body = Vec::new();
            varint(player_id, &mut body);
            varint(if motion.sprinting { 1 } else { 2 }, &mut body);
            varint(0, &mut body);
            packets.push((0x2a, body));
            self.sprinting = motion.sprinting;
        }
        self.reminder += 1;
        let position = motion.pose.position;
        let rotation = [motion.pose.yaw, motion.pose.pitch];
        let moved =
            (position - self.position).length_squared() > 2e-4f64.powi(2) || self.reminder >= 20;
        let rotated = rotation != self.rotation;
        let flags = motion.on_ground as u8 | ((motion.horizontal_collision as u8) << 1);
        let id = match (moved, rotated) {
            (true, true) => Some(0x1f),
            (true, false) => Some(0x1e),
            (false, true) => Some(0x20),
            (false, false) if flags != self.flags => Some(0x21),
            _ => None,
        };
        if let Some(id) = id {
            let mut body = Vec::with_capacity(33);
            if moved {
                for value in position.to_array() {
                    body.extend(value.to_be_bytes());
                }
            }
            if rotated {
                for value in rotation {
                    body.extend(value.to_be_bytes());
                }
            }
            body.push(flags);
            packets.push((id, body));
        }
        if moved {
            self.position = position;
            self.reminder = 0;
        }
        if rotated {
            self.rotation = rotation;
        }
        self.flags = flags;
        Ok(packets)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rendering_frame_rate_cannot_increase_player_packet_rate() {
        for fps in [30, 60, 120, 240, 1000] {
            let mut player = crate::player::Player::spawn(glam::DVec3::new(0., 100., 0.));
            let mut sender = Sender::new(Pose::default());
            let mut ticks = 0;
            let mut moves = 0;
            for _ in 0..fps {
                player.advance_with(
                    1. / fps as f64,
                    crate::player::Input::default(),
                    0.,
                    &crate::player::DemoCollision,
                    |p| {
                        ticks += 1;
                        let m = Motion {
                            world_epoch: 1,
                            teleport_version: 1,
                            abilities_version: 0,
                            velocity_version: 0,
                            player_state_version: 0,
                            pose: Pose {
                                position: p.feet,
                                velocity: p.velocity,
                                yaw: 0.,
                                pitch: 0.,
                            },
                            on_ground: p.grounded,
                            horizontal_collision: p.horizontal_collision,
                            input: 0,
                            sprinting: false,
                            slot: 0,
                            flying: false,
                        };
                        moves += sender.tick(m, 1).unwrap().len();
                    },
                );
            }
            assert_eq!(ticks, 20, "fps={fps}");
            assert!(moves <= 21, "fps={fps}: {moves} packets");
        }
    }
    #[test]
    fn packet_selection_threshold_heartbeat_and_flags_match_local_player() {
        let pose = Pose::default();
        let mut sender = Sender::new(pose);
        let mut motion = Motion {
            world_epoch: 1,
            teleport_version: 1,
            abilities_version: 0,
            velocity_version: 0,
            player_state_version: 0,
            pose,
            on_ground: false,
            horizontal_collision: false,
            input: 0,
            sprinting: false,
            slot: 0,
            flying: false,
        };
        for _ in 0..19 {
            assert!(sender.tick(motion, 99).unwrap().is_empty());
        }
        assert_eq!(sender.tick(motion, 99).unwrap()[0].0, 0x1e);
        motion.pose.position.x = 0.0001;
        assert!(sender.tick(motion, 99).unwrap().is_empty());
        motion.pose.position.x = 0.0003;
        assert_eq!(sender.tick(motion, 99).unwrap()[0].0, 0x1e);
        motion.pose.yaw = 30.;
        assert_eq!(
            sender.tick(motion, 99).unwrap()[0],
            (
                0x20,
                [
                    30f32.to_be_bytes().as_slice(),
                    0f32.to_be_bytes().as_slice(),
                    &[0]
                ]
                .concat()
            )
        );
        motion.on_ground = true;
        motion.horizontal_collision = true;
        assert_eq!(sender.tick(motion, 99).unwrap(), vec![(0x21, vec![3])]);
        motion.pose.position.z = 1.;
        motion.pose.pitch = 10.;
        let packets = sender.tick(motion, 99).unwrap();
        assert_eq!(packets[0].0, 0x1f);
        assert_eq!(packets[0].1.len(), 33);
        motion.input = 1 | 4 | 16 | 32 | 64;
        motion.sprinting = true;
        motion.slot = 8;
        assert_eq!(
            sender.tick(motion, 99).unwrap(),
            vec![
                (0x35, vec![0, 8]),
                (0x2b, vec![117]),
                (0x2a, vec![99, 1, 0])
            ]
        );
        motion.sprinting = false;
        assert_eq!(
            sender.tick(motion, 99).unwrap(),
            vec![(0x2a, vec![99, 2, 0])]
        );
        motion.pose.velocity.x = f64::NAN;
        assert!(sender.tick(motion, 99).is_err());
    }
}
