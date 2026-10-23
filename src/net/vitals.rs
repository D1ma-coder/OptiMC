//! Server-authoritative ClientboundSetHealthPacket (protocol 776).
use super::codec::read_varint;
use anyhow::{ensure, Result};
use std::io::Read;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vitals {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
}
impl Default for Vitals {
    fn default() -> Self {
        Self {
            health: 20.,
            food: 20,
            saturation: 5.,
        }
    }
}
impl Vitals {
    pub fn decode(mut input: &[u8]) -> Result<Self> {
        let mut bytes = [0; 4];
        input.read_exact(&mut bytes)?;
        let health = f32::from_be_bytes(bytes);
        let food = read_varint(&mut input)?;
        input.read_exact(&mut bytes)?;
        let saturation = f32::from_be_bytes(bytes);
        ensure!(
            input.is_empty() && health.is_finite() && saturation.is_finite(),
            "Invalid health packet"
        );
        Ok(Self {
            health,
            food,
            saturation,
        })
    }
    pub fn heart(&self, index: usize) -> u8 {
        ((self.health.ceil().clamp(0., 20.) as i32 - index as i32 * 2).clamp(0, 2)) as u8
    }
    pub fn hunger(&self, index: usize) -> u8 {
        (self.food.clamp(0, 20) - index as i32 * 2).clamp(0, 2) as u8
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_health_food_and_saturation_decode_and_select_half_icons() {
        let mut bytes = 17.5f32.to_be_bytes().to_vec();
        super::super::codec::varint(7, &mut bytes);
        bytes.extend(2.5f32.to_be_bytes());
        let v = Vitals::decode(&bytes).unwrap();
        assert_eq!(
            v,
            Vitals {
                health: 17.5,
                food: 7,
                saturation: 2.5
            }
        );
        assert_eq!(v.heart(8), 2);
        assert_eq!(v.heart(9), 0);
        assert_eq!(v.hunger(3), 1);
        assert_eq!(v.hunger(4), 0);
        for end in 0..bytes.len() {
            assert!(Vitals::decode(&bytes[..end]).is_err());
        }
        bytes.extend([0]);
        assert!(Vitals::decode(&bytes).is_err());
    }
}
