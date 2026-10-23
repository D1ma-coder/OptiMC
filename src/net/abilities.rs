use anyhow::{ensure, Result};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Abilities {
    pub invulnerable: bool,
    pub flying: bool,
    pub may_fly: bool,
    pub instant_build: bool,
    pub flying_speed: f32,
    pub walking_speed: f32,
}
impl Default for Abilities {
    fn default() -> Self {
        Self {
            invulnerable: false,
            flying: false,
            may_fly: false,
            instant_build: false,
            flying_speed: 0.05,
            walking_speed: 0.1,
        }
    }
}
impl Abilities {
    /// GameType.updatePlayerAbilities. Creative preserves the current flying
    /// flag; spectator forces it, survival/adventure clear it. Speeds persist.
    pub fn apply_game_mode(&mut self, mode: u8) {
        self.may_fly = mode == 1 || mode == 3;
        self.instant_build = mode == 1;
        self.invulnerable = mode == 1 || mode == 3;
        if mode == 3 {
            self.flying = true;
        } else if mode != 1 {
            self.flying = false;
        }
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() == 9, "Invalid abilities packet");
        let flying_speed = f32::from_be_bytes(bytes[1..5].try_into()?);
        let walking_speed = f32::from_be_bytes(bytes[5..9].try_into()?);
        ensure!(
            flying_speed.is_finite()
                && flying_speed >= 0.
                && walking_speed.is_finite()
                && walking_speed >= 0.,
            "Invalid abilities speed"
        );
        let flags = bytes[0];
        Ok(Self {
            invulnerable: flags & 1 != 0,
            flying: flags & 2 != 0,
            may_fly: flags & 4 != 0,
            instant_build: flags & 8 != 0,
            flying_speed,
            walking_speed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permissions_and_speeds_decode_and_invalid_packets_are_rejected() {
        let mut bytes = vec![15];
        bytes.extend(0.05f32.to_be_bytes());
        bytes.extend(0.1f32.to_be_bytes());
        let abilities = Abilities::decode(&bytes).unwrap();
        assert!(
            abilities.flying
                && abilities.may_fly
                && abilities.invulnerable
                && abilities.instant_build
        );
        assert_eq!(abilities.flying_speed, 0.05);
        for end in 0..bytes.len() {
            assert!(Abilities::decode(&bytes[..end]).is_err());
        }
        bytes[1..5].copy_from_slice(&f32::NAN.to_be_bytes());
        assert!(Abilities::decode(&bytes).is_err());
    }
}
