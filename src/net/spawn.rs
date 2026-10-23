//! Protocol 776 login/respawn headers. Dimension holder IDs are direct VarInts,
//! not the +1/inline format used by a different holder codec.
use super::{codec::read_varint, registry::identifier};
use anyhow::{ensure, Result};
use std::io::Read;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnInfo {
    pub dimension_type: u32,
    pub dimension: String,
    pub seed: i64,
    pub game_mode: u8,
    pub previous_mode: Option<u8>,
    pub debug: bool,
    pub flat: bool,
    pub death: Option<(String, [i32; 3])>,
    pub portal_cooldown: i32,
    pub sea_level: i32,
}
#[derive(Clone, Debug)]
pub struct Login {
    pub player_id: i32,
    pub hardcore: bool,
    pub levels: Vec<String>,
    pub max_players: u32,
    pub view_distance: u32,
    pub simulation_distance: u32,
    pub reduced_debug: bool,
    pub death_screen: bool,
    pub limited_crafting: bool,
    pub spawn: SpawnInfo,
    pub online_mode: bool,
    pub secure_chat: bool,
}
fn array<const N: usize>(input: &mut &[u8]) -> Result<[u8; N]> {
    let mut out = [0; N];
    input.read_exact(&mut out)?;
    Ok(out)
}
fn boolean(input: &mut &[u8]) -> Result<bool> {
    Ok(array::<1>(input)?[0] != 0)
}
fn unsigned(input: &mut &[u8]) -> Result<u32> {
    let value = read_varint(input)?;
    ensure!(value >= 0, "Negative spawn field");
    Ok(value as u32)
}
pub fn block_position(packed: i64) -> [i32; 3] {
    [
        (packed >> 38) as i32,
        (packed << 52 >> 52) as i32,
        (packed << 26 >> 38) as i32,
    ]
}
fn common(input: &mut &[u8]) -> Result<SpawnInfo> {
    let dimension_type = unsigned(input)?;
    let dimension = identifier(input)?;
    let seed = i64::from_be_bytes(array(input)?);
    let game_mode = array::<1>(input)?[0];
    let previous = array::<1>(input)?[0];
    ensure!(
        game_mode <= 3 && (previous <= 3 || previous == 255),
        "Invalid game mode"
    );
    let debug = boolean(input)?;
    let flat = boolean(input)?;
    let death = if boolean(input)? {
        Some((
            identifier(input)?,
            block_position(i64::from_be_bytes(array(input)?)),
        ))
    } else {
        None
    };
    let portal_cooldown = read_varint(input)?;
    let sea_level = read_varint(input)?;
    Ok(SpawnInfo {
        dimension_type,
        dimension,
        seed,
        game_mode,
        previous_mode: (previous != 255).then_some(previous),
        debug,
        flat,
        death,
        portal_cooldown,
        sea_level,
    })
}
pub fn login(bytes: &[u8]) -> Result<Login> {
    let mut input = bytes;
    let player_id = i32::from_be_bytes(array(&mut input)?);
    let hardcore = boolean(&mut input)?;
    let count = unsigned(&mut input)?;
    ensure!(
        count <= 1024 && count as usize <= input.len(),
        "Too many login dimensions"
    );
    let mut levels = Vec::with_capacity(count as usize);
    for _ in 0..count {
        levels.push(identifier(&mut input)?);
    }
    let max_players = unsigned(&mut input)?;
    let view_distance = unsigned(&mut input)?;
    let simulation_distance = unsigned(&mut input)?;
    let reduced_debug = boolean(&mut input)?;
    let death_screen = boolean(&mut input)?;
    let limited_crafting = boolean(&mut input)?;
    let spawn = common(&mut input)?;
    let online_mode = boolean(&mut input)?;
    let secure_chat = boolean(&mut input)?;
    ensure!(input.is_empty(), "Trailing login packet data");
    Ok(Login {
        player_id,
        hardcore,
        levels,
        max_players,
        view_distance,
        simulation_distance,
        reduced_debug,
        death_screen,
        limited_crafting,
        spawn,
        online_mode,
        secure_chat,
    })
}
pub fn respawn(bytes: &[u8]) -> Result<(SpawnInfo, u8)> {
    let mut input = bytes;
    let spawn = common(&mut input)?;
    let keep = array::<1>(&mut input)?[0];
    ensure!(input.is_empty(), "Trailing respawn packet data");
    Ok((spawn, keep))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::codec::{string, varint};
    #[test]
    fn common_spawn_uses_direct_registry_id_and_signed_previous_mode() {
        let mut bytes = vec![0];
        string("minecraft:overworld", &mut bytes);
        bytes.extend(123i64.to_be_bytes());
        bytes.extend([1, 255, 0, 0, 0]);
        varint(0, &mut bytes);
        varint(63, &mut bytes);
        bytes.push(3);
        let (spawn, keep) = respawn(&bytes).unwrap();
        assert_eq!(spawn.dimension_type, 0);
        assert_eq!(spawn.previous_mode, None);
        assert_eq!(spawn.sea_level, 63);
        assert_eq!(keep, 3);
    }
    #[test]
    fn negative_global_position_components_are_sign_extended() {
        let x = -100i64;
        let y = -64i64;
        let z = -200i64;
        let packed = (x & 0x3ff_ffff) << 38 | (z & 0x3ff_ffff) << 12 | (y & 4095);
        assert_eq!(block_position(packed), [-100, -64, -200]);
    }
}
