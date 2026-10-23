//! 26.2 world clocks and level metadata. Clock updates use server registry IDs,
//! not the pre-26.2 dayTime packet or a hard-coded overworld clock index.
use super::{
    codec::{read_string, read_varint, read_varlong},
    registry::{self, Registries},
    spawn,
};
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, time::Instant};
fn long(input: &mut &[u8]) -> Result<i64> {
    ensure!(input.len() >= 8, "Truncated environment long");
    let value = i64::from_be_bytes(input[..8].try_into()?);
    *input = &input[8..];
    Ok(value)
}
fn float(input: &mut &[u8]) -> Result<f32> {
    ensure!(input.len() >= 4, "Truncated environment float");
    let value = f32::from_be_bytes(input[..4].try_into()?);
    *input = &input[4..];
    ensure!(value.is_finite(), "Non-finite environment value");
    Ok(value)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub total_ticks: i64,
    pub partial_tick: f32,
    pub rate: f32,
}
impl Clock {
    pub fn advance(&mut self, ticks: i64) {
        let value = self.partial_tick as f64 + ticks as f64 * self.rate as f64;
        // Mth.floor(double) returns int; Java long addition wraps.
        let whole = value.floor() as i32;
        self.partial_tick = (value - whole as f64) as f32;
        self.total_ticks = self.total_ticks.wrapping_add(whole as i64);
    }
    pub fn cycle(&self, period: i64, elapsed_ticks: f64) -> f64 {
        (self.total_ticks.rem_euclid(period) as f64
            + self.partial_tick as f64
            + elapsed_ticks * self.rate as f64)
            .rem_euclid(period as f64)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct RespawnData {
    pub dimension: String,
    pub position: [i32; 3],
    pub yaw: f32,
    pub pitch: f32,
}
impl RespawnData {
    pub fn decode(mut input: &[u8]) -> Result<Self> {
        let dimension = registry::identifier(&mut input)?;
        let position = spawn::block_position(long(&mut input)?);
        let yaw = float(&mut input)?;
        let pitch = float(&mut input)?;
        ensure!(input.is_empty(), "Trailing default spawn position bytes");
        // The stream constructor preserves values; only RespawnData.of clamps.
        Ok(Self {
            dimension,
            position,
            yaw,
            pitch,
        })
    }
}
#[derive(Clone, Debug, Default)]
pub struct State {
    pub game_time: i64,
    pub clocks: BTreeMap<u32, Clock>,
    pub time_received: Option<Instant>,
    pub default_spawn: Option<RespawnData>,
    pub chunk_center: Option<[i32; 2]>,
    pub chunk_radius: Option<u32>,
    pub simulation_distance: Option<i32>,
    pub difficulty: Option<(u8, bool)>,
    pub rain_level: f32,
    pub thunder_level: f32,
}
impl State {
    /// Level.setRainLevel/setThunderLevel; both previous/current values are set
    /// together by the source setters, so the packet itself adds no interpolation.
    pub fn weather_event(&mut self, input: &[u8]) -> Result<bool> {
        ensure!(input.len() == 5, "Invalid game event length");
        let parameter = f32::from_be_bytes(input[1..].try_into()?);
        match input[0] {
            1 => self.rain_level = 0.,
            2 => self.rain_level = 1.,
            7 => self.rain_level = parameter.clamp(0., 1.),
            8 => self.thunder_level = parameter.clamp(0., 1.),
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub fn difficulty(&mut self, mut input: &[u8]) -> Result<()> {
        let id = read_varint(&mut input)?;
        ensure!(input.len() == 1, "Invalid change difficulty length");
        // Difficulty.BY_ID uses WRAP; the boolean codec accepts any nonzero byte.
        self.difficulty = Some((id.rem_euclid(4) as u8, input[0] != 0));
        Ok(())
    }
    pub fn simulation_distance(&mut self, mut input: &[u8]) -> Result<()> {
        let distance = read_varint(&mut input)?;
        ensure!(input.is_empty(), "Trailing simulation distance bytes");
        // The original packet and ClientLevel preserve the signed int.
        self.simulation_distance = Some(distance);
        Ok(())
    }
    pub fn time(&mut self, mut input: &[u8], registries: &Registries) -> Result<()> {
        let game_time = long(&mut input)?;
        let count = read_varint(&mut input)?;
        ensure!(
            count >= 0 && count as usize <= input.len() / 10,
            "Invalid world clock count"
        );
        let mut updates = BTreeMap::new();
        for _ in 0..count {
            let id = read_varint(&mut input)?;
            ensure!(
                id >= 0
                    && registries
                        .entry("minecraft:world_clock", id as u32)
                        .is_some(),
                "Unknown world clock registry ID {id}"
            );
            let total_ticks = read_varlong(&mut input)?;
            let partial_tick = float(&mut input)?;
            let rate = float(&mut input)?;
            updates.insert(
                id as u32,
                Clock {
                    total_ticks,
                    partial_tick,
                    rate,
                },
            );
        }
        ensure!(input.is_empty(), "Trailing set_time bytes");
        let delta = game_time.wrapping_sub(self.game_time);
        for clock in self.clocks.values_mut() {
            clock.advance(delta);
        }
        self.clocks.extend(updates);
        self.game_time = game_time;
        self.time_received = Some(Instant::now());
        Ok(())
    }
    pub fn center(&mut self, mut input: &[u8]) -> Result<()> {
        let center = [read_varint(&mut input)?, read_varint(&mut input)?];
        ensure!(input.is_empty(), "Trailing chunk cache center bytes");
        self.chunk_center = Some(center);
        Ok(())
    }
    pub fn radius(&mut self, mut input: &[u8]) -> Result<()> {
        let radius = read_varint(&mut input)?;
        ensure!(
            radius >= 0 && input.is_empty(),
            "Invalid chunk cache radius"
        );
        self.chunk_radius = Some(radius as u32);
        Ok(())
    }
    pub fn clock<'a>(&'a self, registries: &Registries, name: &str) -> Option<&'a Clock> {
        self.clocks
            .get(&registries.id("minecraft:world_clock", name)?)
    }
    pub fn elapsed_ticks(&self, now: Instant) -> f64 {
        self.time_received.map_or(0., |at| {
            now.saturating_duration_since(at).as_secs_f64() * 20.
        })
    }
}
/// Common custom payload used in configuration and play. Unknown channels keep
/// their own explicit diagnostics; brand requires no protocol response.
pub fn brand(mut input: &[u8]) -> Result<Option<String>> {
    let channel = registry::identifier(&mut input)?;
    if channel != "minecraft:brand" {
        return Ok(None);
    }
    let brand = read_string(&mut input, 32767)?;
    ensure!(input.is_empty(), "Trailing server brand bytes");
    Ok(Some(brand))
}
/// CHANGE_GAME_MODE uses floor(param + .5), then GameType.byId's ZERO fallback.
/// Other game-event actions retain their own unhandled diagnostics.
pub fn game_mode(mut input: &[u8]) -> Result<Option<u8>> {
    ensure!(input.len() == 5, "Invalid game event length");
    let event = input[0];
    input = &input[1..];
    let value = float(&mut input)?;
    if event != 3 {
        return Ok(None);
    }
    let value = (value + 0.5).floor() as i32;
    Ok(Some(if (0..=3).contains(&value) {
        value as u8
    } else {
        0
    }))
}
#[cfg(test)]
pub(crate) fn reference() -> serde_json::Value {
    use sha1::{Digest, Sha1};
    use std::io::Read;
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(
        include_bytes!("../../data/26.2/world-environment.json.gz").as_slice(),
    )
    .read_to_end(&mut bytes)
    .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../data/26.2/reference.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha1::digest(&bytes)),
        manifest["sha1"]["world-environment.json"]
    );
    serde_json::from_slice(&bytes).unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    fn registries(reference: &serde_json::Value) -> Registries {
        let mut registry = Vec::new();
        super::super::codec::string("minecraft:world_clock", &mut registry);
        super::super::codec::varint(
            reference["registry"].as_array().unwrap().len() as i32,
            &mut registry,
        );
        for name in reference["registry"].as_array().unwrap() {
            super::super::codec::string(name.as_str().unwrap(), &mut registry);
            registry.extend([1, 10, 0]);
        }
        let mut r = Registries::default();
        r.packet(&registry).unwrap();
        r
    }
    #[test]
    fn difficulty_and_simulation_packets_match_original_and_commit_atomically() {
        let r = reference();
        let mut state = State::default();
        for row in r["difficulties"].as_array().unwrap() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            state.difficulty(&bytes).unwrap();
            assert_eq!(
                state.difficulty,
                Some((
                    row["difficulty"].as_u64().unwrap() as u8,
                    row["locked"].as_bool().unwrap()
                ))
            );
            for n in 0..bytes.len() {
                let previous = state.difficulty;
                assert!(state.difficulty(&bytes[..n]).is_err());
                assert_eq!(state.difficulty, previous);
            }
        }
        for row in r["distances"].as_array().unwrap() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            state.simulation_distance(&bytes).unwrap();
            assert_eq!(
                state.simulation_distance,
                Some(row["distance"].as_i64().unwrap() as i32)
            );
            for n in 0..bytes.len() {
                let previous = state.simulation_distance;
                assert!(state.simulation_distance(&bytes[..n]).is_err());
                assert_eq!(state.simulation_distance, previous);
            }
            let mut trailing = bytes;
            trailing.push(0);
            let previous = state.simulation_distance;
            assert!(state.simulation_distance(&trailing).is_err());
            assert_eq!(state.simulation_distance, previous);
        }
        let mut wrapped = Vec::new();
        super::super::codec::varint(-1, &mut wrapped);
        wrapped.push(2);
        state.difficulty(&wrapped).unwrap();
        assert_eq!(state.difficulty, Some((3, true)));
    }
    #[test]
    fn game_mode_rounding_and_all_initial_permissions_match_original() {
        let r = reference();
        let rows = r["gamemodes"].as_array().unwrap();
        assert_eq!(rows.len(), 224);
        for row in rows {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            let mode = game_mode(&bytes).unwrap().unwrap();
            assert_eq!(mode, row["mode"].as_u64().unwrap() as u8);
            let flags = row["initialFlags"].as_u64().unwrap() as u8;
            let mut a = super::super::abilities::Abilities {
                invulnerable: flags & 1 != 0,
                flying: flags & 2 != 0,
                may_fly: flags & 4 != 0,
                instant_build: flags & 8 != 0,
                flying_speed: 0.125,
                walking_speed: 0.25,
            };
            a.apply_game_mode(mode);
            let result = u8::from(a.invulnerable)
                | (u8::from(a.flying) << 1)
                | (u8::from(a.may_fly) << 2)
                | (u8::from(a.instant_build) << 3);
            assert_eq!(result, row["flags"].as_u64().unwrap() as u8);
            assert_eq!((a.flying_speed, a.walking_speed), (0.125, 0.25));
        }
        assert!(game_mode(&[3, 0, 0, 0]).is_err());
        assert!(game_mode(&[3, 0x7f, 0xc0, 0, 0]).is_err());
        assert!(game_mode(&[7, 0, 0, 0, 0]).unwrap().is_none());
    }
    #[test]
    fn clock_packets_match_original_manager_including_reverse_registry_and_negative_rates() {
        let reference = reference();
        let registry = registries(&reference);
        let mut state = State::default();
        for row in reference["times"].as_array().unwrap() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            let before = state.clone();
            for end in 0..bytes.len() {
                let mut broken = before.clone();
                assert!(broken.time(&bytes[..end], &registry).is_err());
                assert_eq!(broken.clocks, before.clocks);
                assert_eq!(broken.game_time, before.game_time);
            }
            state.time(&bytes, &registry).unwrap();
            assert_eq!(state.game_time, row["gameTime"].as_i64().unwrap());
            for clock in row["clocks"].as_array().unwrap() {
                let id = clock["id"].as_u64().unwrap() as u32;
                let actual = state.clocks[&id];
                assert_eq!(actual.total_ticks, clock["totalTicks"].as_i64().unwrap());
                assert_eq!(
                    actual.partial_tick,
                    clock["partialTick"].as_f64().unwrap() as f32
                );
                assert_eq!(actual.rate, clock["rate"].as_f64().unwrap() as f32);
            }
        }
        assert_eq!(
            state.clock(&registry, "minecraft:overworld").unwrap().rate,
            2.
        );
        // Do the modulo before converting an i64 clock to floating point.
        let clock = Clock {
            total_ticks: i64::MAX,
            partial_tick: 0.75,
            rate: 0.,
        };
        assert_eq!(
            clock.cycle(24000, 100.),
            i64::MAX.rem_euclid(24000) as f64 + 0.75
        );
    }
    #[test]
    fn original_spawn_center_radius_and_brand_reach_state_without_extra_fields() {
        let reference = reference();
        let mut state = State::default();
        for row in reference["packets"].as_array().unwrap() {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            match row["label"].as_str().unwrap() {
                "spawn" => {
                    state.default_spawn = Some(RespawnData::decode(&bytes).unwrap());
                    let spawn = state.default_spawn.as_ref().unwrap();
                    assert_eq!(spawn.dimension, "minecraft:overworld");
                    assert_eq!(spawn.position, [-31, -22, 47]);
                    assert_eq!((spawn.yaw, spawn.pitch), (37.5, -12.25));
                    for end in 0..bytes.len() {
                        assert!(RespawnData::decode(&bytes[..end]).is_err());
                    }
                }
                "center" => {
                    state.center(&bytes).unwrap();
                    assert_eq!(state.chunk_center, Some([-300, 512]));
                    let mut bad = bytes.clone();
                    bad.push(0);
                    assert!(state.center(&bad).is_err());
                    assert_eq!(state.chunk_center, Some([-300, 512]));
                }
                "radius" => {
                    state.radius(&bytes).unwrap();
                    assert_eq!(state.chunk_radius, Some(12));
                }
                "difficulty" => {
                    state.difficulty(&bytes).unwrap();
                    assert_eq!(state.difficulty, Some((0, true)));
                }
                "simulation" => {
                    state.simulation_distance(&bytes).unwrap();
                    assert_eq!(state.simulation_distance, Some(12));
                }
                "brand_config" | "brand_play" => assert_eq!(
                    brand(&bytes).unwrap().as_deref(),
                    Some("Paper fixture 26.2")
                ),
                "block_ack" => {
                    let mut input = bytes.as_slice();
                    assert_eq!(super::super::codec::read_varint(&mut input).unwrap(), 37);
                    assert!(input.is_empty());
                }
                label => panic!("Unexpected {label}"),
            }
        }
        let mut unknown = Vec::new();
        super::super::codec::string("fixture:opaque", &mut unknown);
        unknown.extend([255; 10]);
        assert!(brand(&unknown).unwrap().is_none());
    }
}
