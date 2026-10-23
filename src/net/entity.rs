//! Entity state packets for Java 26.2; independent of rendering and entity AI.
use super::{
    codec::{read_varint, varint},
    vector,
};
use anyhow::{ensure, Result};
use glam::DVec3;
use std::{collections::HashMap, io::Read};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub position: DVec3,
    pub velocity: DVec3,
    pub yaw: f32,
    pub pitch: f32,
}
#[derive(Clone, Debug)]
pub struct Entity {
    pub id: i32,
    pub uuid: [u8; 16],
    pub kind: u32,
    pub spawn_data: i32,
    pub pose: Pose,
    pub previous: Pose,
    pub head_yaw: f32,
    pub on_ground: bool,
}
impl Entity {
    pub fn type_name(&self) -> Result<Option<&'static str>> {
        Ok(crate::assets::builtins::get()?.name("minecraft:entity_type", self.kind))
    }
}
#[derive(Clone, Debug)]
pub enum Event {
    Spawn(Entity),
    Relative {
        id: i32,
        delta: Option<[i16; 3]>,
        rotation: Option<[f32; 2]>,
        on_ground: bool,
    },
    Head {
        id: i32,
        yaw: f32,
    },
    Motion {
        id: i32,
        velocity: DVec3,
    },
    Position {
        id: i32,
        pose: Pose,
        flags: u32,
        on_ground: bool,
    },
    Remove(Vec<i32>),
}
fn array<const N: usize>(input: &mut impl Read) -> Result<[u8; N]> {
    let mut value = [0; N];
    input.read_exact(&mut value)?;
    Ok(value)
}
fn vec3(input: &mut impl Read) -> Result<DVec3> {
    let values = [
        f64::from_be_bytes(array(input)?),
        f64::from_be_bytes(array(input)?),
        f64::from_be_bytes(array(input)?),
    ];
    ensure!(
        values.into_iter().all(f64::is_finite),
        "Non-finite entity vector"
    );
    Ok(DVec3::from_array(values))
}
fn angle(input: &mut impl Read) -> Result<f32> {
    Ok(i8::from_be_bytes(array(input)?) as f32 * 360. / 256.)
}
fn pose(input: &mut impl Read) -> Result<Pose> {
    let position = vec3(input)?;
    let velocity = vec3(input)?;
    let yaw = f32::from_be_bytes(array(input)?);
    let pitch = f32::from_be_bytes(array(input)?);
    ensure!(
        yaw.is_finite() && pitch.is_finite(),
        "Non-finite entity rotation"
    );
    Ok(Pose {
        position,
        velocity,
        yaw,
        pitch,
    })
}
/// Player position correction uses the same PositionMoveRotation as entity
/// teleports, preceded by the acknowledgement ID (and no on-ground boolean).
/// Decode and validate the entire packet before the caller mutates state or
/// acknowledges it on the wire.
pub fn player_position(bytes: &[u8], previous: Pose) -> Result<(i32, Pose)> {
    let mut input = bytes;
    let teleport = read_varint(&mut input)?;
    let change = pose(&mut input)?;
    let flags = u32::from_be_bytes(array(&mut input)?);
    ensure!(input.is_empty(), "Trailing player teleport bytes");
    let absolute = previous.absolute(change, flags);
    ensure!(
        absolute.position.is_finite()
            && absolute.velocity.is_finite()
            && absolute.yaw.is_finite()
            && absolute.pitch.is_finite(),
        "Non-finite absolute player teleport"
    );
    Ok((teleport, absolute))
}
pub fn decode(packet: i32, bytes: &[u8]) -> Result<Option<Event>> {
    let mut input = bytes;
    let event = match packet {
        0x01 => {
            let id = read_varint(&mut input)?;
            let uuid = array(&mut input)?;
            let kind = read_varint(&mut input)?;
            ensure!(kind >= 0, "Negative entity type");
            let position = vec3(&mut input)?;
            let velocity = vector::read(&mut input)?;
            let pitch = angle(&mut input)?;
            let yaw = angle(&mut input)?;
            let head_yaw = angle(&mut input)?;
            let spawn_data = read_varint(&mut input)?;
            let pose = Pose {
                position,
                velocity,
                yaw,
                pitch,
            };
            Event::Spawn(Entity {
                id,
                uuid,
                kind: kind as u32,
                spawn_data,
                pose,
                previous: pose,
                head_yaw,
                on_ground: false,
            })
        }
        0x35 | 0x36 | 0x38 => {
            let id = read_varint(&mut input)?;
            let delta = if packet != 0x38 {
                Some([
                    i16::from_be_bytes(array(&mut input)?),
                    i16::from_be_bytes(array(&mut input)?),
                    i16::from_be_bytes(array(&mut input)?),
                ])
            } else {
                None
            };
            let rotation = if packet != 0x35 {
                Some([angle(&mut input)?, angle(&mut input)?])
            } else {
                None
            };
            let on_ground = array::<1>(&mut input)?[0] != 0;
            Event::Relative {
                id,
                delta,
                rotation,
                on_ground,
            }
        }
        0x23 | 0x7d => {
            let id = read_varint(&mut input)?;
            let pose = pose(&mut input)?;
            let flags = if packet == 0x7d {
                u32::from_be_bytes(array(&mut input)?)
            } else {
                0
            };
            let on_ground = array::<1>(&mut input)?[0] != 0;
            Event::Position {
                id,
                pose,
                flags,
                on_ground,
            }
        }
        0x53 => Event::Head {
            id: read_varint(&mut input)?,
            yaw: angle(&mut input)?,
        },
        0x65 => Event::Motion {
            id: read_varint(&mut input)?,
            velocity: vector::read(&mut input)?,
        },
        0x4d => {
            let count = read_varint(&mut input)?;
            ensure!(
                count >= 0 && count as usize <= input.len(),
                "Invalid removed entity count"
            );
            let mut ids = Vec::with_capacity(count as usize);
            for _ in 0..count {
                ids.push(read_varint(&mut input)?);
            }
            Event::Remove(ids)
        }
        _ => return Ok(None),
    };
    ensure!(
        input.is_empty(),
        "Trailing bytes in protocol 776 entity packet {packet:#x}"
    );
    Ok(Some(event))
}
impl Pose {
    pub fn absolute(self, change: Self, flags: u32) -> Self {
        let mut result = change;
        for axis in 0..3 {
            if flags & (1 << axis) != 0 {
                result.position[axis] += self.position[axis];
            }
        }
        if flags & 8 != 0 {
            result.yaw += self.yaw;
        }
        if flags & 16 != 0 {
            result.pitch += self.pitch;
        }
        result.pitch = result.pitch.clamp(-90., 90.);
        let mut velocity = self.velocity;
        if flags & 256 != 0 {
            // Vec3.xRot / yRot use the opposite sign from glam's active rotations.
            let pitch = (self.pitch - result.pitch).to_radians() as f64;
            let yaw = (self.yaw - result.yaw).to_radians() as f64;
            let (s, c) = pitch.sin_cos();
            velocity = DVec3::new(
                velocity.x,
                velocity.y * c + velocity.z * s,
                velocity.z * c - velocity.y * s,
            );
            let (s, c) = yaw.sin_cos();
            velocity = DVec3::new(
                velocity.x * c + velocity.z * s,
                velocity.y,
                velocity.z * c - velocity.x * s,
            );
        }
        for axis in 0..3 {
            if flags & (1 << (axis + 5)) != 0 {
                result.velocity[axis] += velocity[axis];
            }
        }
        result
    }
}
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    pub entities: HashMap<i32, Entity>,
    pub revision: u64,
    pub frame_revision: u64,
}
impl Tracker {
    pub fn apply(&mut self, event: Event) {
        self.revision = self.revision.wrapping_add(1);
        let is_frame = |e: &Entity| {
            matches!(
                e.type_name().ok().flatten(),
                Some("minecraft:item_frame" | "minecraft:glow_item_frame")
            )
        };
        let changed = match &event {
            Event::Spawn(entity) => is_frame(entity),
            Event::Remove(ids) => ids
                .iter()
                .any(|id| self.entities.get(id).is_some_and(is_frame)),
            Event::Relative { id, .. }
            | Event::Head { id, .. }
            | Event::Motion { id, .. }
            | Event::Position { id, .. } => self.entities.get(id).is_some_and(is_frame),
        };
        if changed {
            self.frame_revision = self.frame_revision.wrapping_add(1);
        }
        match event {
            Event::Spawn(entity) => {
                self.entities.insert(entity.id, entity);
            }
            Event::Remove(ids) => {
                for id in ids {
                    self.entities.remove(&id);
                }
            }
            Event::Relative {
                id,
                delta,
                rotation,
                on_ground,
            } => {
                if let Some(entity) = self.entities.get_mut(&id) {
                    entity.previous = entity.pose;
                    if let Some(delta) = delta {
                        for (axis, delta) in delta.into_iter().enumerate() {
                            if delta != 0 {
                                entity.pose.position[axis] =
                                    ((entity.pose.position[axis] * 4096. + 0.5).floor()
                                        + delta as f64)
                                        / 4096.;
                            }
                        }
                    }
                    if let Some([yaw, pitch]) = rotation {
                        entity.pose.yaw = yaw;
                        entity.pose.pitch = pitch;
                    }
                    entity.on_ground = on_ground;
                }
            }
            Event::Head { id, yaw } => {
                if let Some(entity) = self.entities.get_mut(&id) {
                    entity.head_yaw = yaw;
                }
            }
            Event::Motion { id, velocity } => {
                if let Some(entity) = self.entities.get_mut(&id) {
                    entity.pose.velocity = velocity;
                }
            }
            Event::Position {
                id,
                pose,
                flags,
                on_ground,
            } => {
                if let Some(entity) = self.entities.get_mut(&id) {
                    entity.previous = entity.pose;
                    entity.pose = entity.pose.absolute(pose, flags);
                    entity.on_ground = on_ground;
                }
            }
        }
    }
    pub fn packet(&mut self, id: i32, data: &[u8]) -> Result<bool> {
        if let Some(event) = decode(id, data)? {
            self.apply(event);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Hand {
    Main,
    Off,
}
/// 26.2 split attack into its own packet. Interact always has hand + LpVec3.
/// Returns a packet body; no action is sent without the caller's user input.
pub fn attack(id: i32) -> (i32, Vec<u8>) {
    let mut bytes = Vec::new();
    varint(id, &mut bytes);
    (0x01, bytes)
}
pub fn interact(id: i32, hand: Hand, location: DVec3, secondary: bool) -> Result<(i32, Vec<u8>)> {
    ensure!(location.is_finite(), "Non-finite interaction hit point");
    let mut bytes = Vec::new();
    varint(id, &mut bytes);
    varint(
        match hand {
            Hand::Main => 0,
            Hand::Off => 1,
        },
        &mut bytes,
    );
    vector::write(location, &mut bytes);
    bytes.push(secondary as u8);
    Ok((0x1a, bytes))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_correction_validates_whole_packet_and_preserves_relative_velocity() {
        let previous = Pose {
            position: DVec3::new(10., 64., -20.),
            velocity: DVec3::new(1., 2., 3.),
            yaw: 30.,
            pitch: 15.,
        };
        let mut bytes = vec![7];
        for value in [2f64, 70., 3., 0.25, 0.5, 0.75] {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(5f32.to_be_bytes());
        bytes.extend(200f32.to_be_bytes());
        bytes.extend((1u32 | 4 | 8 | 32 | 128).to_be_bytes());
        let (id, pose) = player_position(&bytes, previous).unwrap();
        assert_eq!(id, 7);
        assert_eq!(pose.position, DVec3::new(12., 70., -17.));
        assert_eq!(pose.velocity, DVec3::new(1.25, 0.5, 3.75));
        assert_eq!([pose.yaw, pose.pitch], [35., 90.]);
        for end in 0..bytes.len() {
            assert!(player_position(&bytes[..end], previous).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(player_position(&trailing, previous).is_err());
        bytes[25..33].copy_from_slice(&f64::NAN.to_be_bytes());
        assert!(player_position(&bytes, previous).is_err());
    }
    #[test]
    fn relative_motion_preserves_zero_axis_and_rounds_negative_like_java() {
        let pose = Pose {
            position: DVec3::new(-0.5 / 4096., 3.123456, 1.),
            ..Pose::default()
        };
        let mut tracker = Tracker::default();
        tracker.apply(Event::Spawn(Entity {
            id: 42,
            uuid: [1; 16],
            kind: 7,
            spawn_data: 0,
            pose,
            previous: pose,
            head_yaw: 0.,
            on_ground: false,
        }));
        let mut bytes = vec![42];
        for delta in [1i16, 0, -4096] {
            bytes.extend(delta.to_be_bytes());
        }
        bytes.push(1);
        assert!(tracker.packet(0x35, &bytes).unwrap());
        let entity = &tracker.entities[&42];
        assert_eq!(entity.pose.position, DVec3::new(1. / 4096., 3.123456, 0.));
        assert!(entity.on_ground);
        tracker.packet(0x4d, &[1, 42]).unwrap();
        assert!(tracker.entities.is_empty());
    }
    #[test]
    fn spawn_consumes_lp_velocity_before_angles_and_data() {
        let mut bytes = vec![7];
        bytes.extend([3; 16]);
        bytes.push(12);
        for value in [1f64, 2., 3.] {
            bytes.extend(value.to_be_bytes());
        }
        vector::write(DVec3::new(0.5, -0.25, 1.), &mut bytes);
        bytes.extend([0, 64, 128, 5]);
        let Event::Spawn(entity) = decode(1, &bytes).unwrap().unwrap() else {
            panic!()
        };
        assert_eq!(entity.kind, 12);
        assert_eq!(entity.pose.yaw, 90.);
        assert_eq!(entity.head_yaw, -180.);
        assert_eq!(entity.spawn_data, 5);
        assert!(
            (entity.pose.velocity - DVec3::new(0.5, -0.25, 1.))
                .abs()
                .max_element()
                < 1. / 32766.
        );
        bytes.push(0);
        assert!(decode(1, &bytes).is_err());
    }
    #[test]
    fn teleport_relative_flags_include_velocity_and_pitch_clamp() {
        let old = Pose {
            position: DVec3::new(10., 20., 30.),
            velocity: DVec3::new(1., 2., 3.),
            yaw: 90.,
            pitch: 20.,
        };
        let change = Pose {
            position: DVec3::new(1., 2., 3.),
            velocity: DVec3::splat(0.5),
            yaw: 5.,
            pitch: 100.,
        };
        let absolute = old.absolute(change, 1 | 4 | 8 | 32 | 128);
        assert_eq!(absolute.position, DVec3::new(11., 2., 33.));
        assert_eq!(absolute.yaw, 95.);
        assert_eq!(absolute.pitch, 90.);
        assert_eq!(absolute.velocity, DVec3::new(1.5, 0.5, 3.5));
    }
    #[test]
    fn relative_teleport_rotates_velocity_before_axis_addition() {
        let old = Pose {
            velocity: DVec3::X,
            ..Pose::default()
        };
        let change = Pose {
            yaw: 90.,
            velocity: DVec3::new(0.25, 0.5, 0.75),
            ..Pose::default()
        };
        let absolute = old.absolute(change, 256 | 32 | 64 | 128);
        assert!((absolute.velocity - DVec3::new(0.25, 0.5, 1.75)).length() < 1e-6);
    }
    #[test]
    fn interaction_matches_new_split_attack_and_lp_hit_layout() {
        assert_eq!(attack(300), (1, vec![0xac, 2]));
        let (id, bytes) = interact(42, Hand::Off, DVec3::ZERO, true).unwrap();
        assert_eq!(id, 0x1a);
        assert_eq!(bytes, [42, 1, 0, 1]);
        assert!(interact(42, Hand::Main, DVec3::splat(f64::NAN), false).is_err());
    }
}
