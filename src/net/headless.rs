//! Developer-only idle gameplay driver, using the same Rust physics as the window.
//! No inputs, automatic reconnects, auth commands or server challenges are generated.
use super::{
    entity::Pose,
    live::{Connection, Snapshot, Status},
    movement::Motion,
    Address, SessionReport,
};
use crate::{
    player::{Input, Player},
    world::collision::RemoteCollision,
};
use anyhow::{bail, Context, Result};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub fn session(address: &Address, name: &str, seconds: u64) -> Result<SessionReport> {
    run(
        Connection::connect(address.clone(), name.to_owned())?,
        seconds,
    )
}
pub fn session_logged(
    address: &Address,
    name: &str,
    seconds: u64,
    cache: &std::path::Path,
) -> Result<SessionReport> {
    run(
        Connection::connect_logged(address.clone(), name.to_owned(), cache)?,
        seconds,
    )
}
fn run(connection: Connection, seconds: u64) -> Result<SessionReport> {
    let physics = crate::assets::collision::get().context("Loading official collision data")?;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut snapshot: Option<Arc<Snapshot>> = None;
    let mut player: Option<Player> = None;
    let mut generations = (u64::MAX, u64::MAX, u64::MAX, u64::MAX);
    let mut last = Instant::now();
    let mut loaded = None;
    let mut ticks = 0u64;
    while Instant::now() < deadline {
        if let Some(next) = connection.take_snapshot() {
            snapshot = Some(next);
        }
        let now = Instant::now();
        let dt = (now - last).as_secs_f64();
        last = now;
        if let Some(s) = &snapshot {
            if s.world_epoch != generations.0 {
                player = None;
                loaded = None;
            }
            if s.position_ready {
                let p = player.get_or_insert_with(|| Player::spawn(s.position.into()));
                if (s.world_epoch, s.teleport_version) != (generations.0, generations.1) {
                    p.correct(Pose {
                        position: s.position.into(),
                        velocity: s.velocity.into(),
                        yaw: s.rotation[0],
                        pitch: s.rotation[1],
                    });
                }
                if s.abilities_version != generations.2 || s.world_epoch != generations.0 {
                    p.abilities = s.abilities;
                }
                if s.velocity_version != generations.3 {
                    p.velocity = s.velocity.into();
                }
                if let Some(id) = s.report.world.player_id {
                    p.attributes = s.report.attributes.movement(id);
                }
                p.food = s.report.vitals.food;
                p.no_clip = s
                    .report
                    .world
                    .spawn
                    .as_ref()
                    .is_some_and(|spawn| spawn.game_mode == 3);
                let world = RemoteCollision {
                    world: &s.report.world,
                    physics,
                };
                if world.loaded(p.feet) {
                    let tag = (s.world_epoch, s.teleport_version);
                    if loaded != Some(tag) || !s.client_loaded {
                        connection.mark_loaded(tag.0, tag.1);
                        loaded = Some(tag);
                    }
                    let mut failure = None;
                    let yaw = (s.rotation[0] + 180.).to_radians();
                    p.advance_with(dt, Input::default(), yaw, &world, |p| {
                        let motion = Motion {
                            world_epoch: s.world_epoch,
                            teleport_version: s.teleport_version,
                            abilities_version: s.abilities_version,
                            velocity_version: s.velocity_version,
                            player_state_version: s.player_state_version,
                            pose: Pose {
                                position: p.feet,
                                velocity: p.velocity,
                                yaw: s.rotation[0],
                                pitch: s.rotation[1],
                            },
                            on_ground: p.grounded,
                            horizontal_collision: p.horizontal_collision,
                            input: 0,
                            sprinting: false,
                            slot: s.report.selected_slot.map_or(0, |(slot, _)| slot),
                            flying: p.abilities.flying,
                        };
                        if let Err(e) = connection.send_motion(motion) {
                            failure = Some(e);
                        }
                        ticks += 1;
                    });
                    if let Some(e) = failure {
                        return Err(e);
                    }
                }
            }
            generations = (
                s.world_epoch,
                s.teleport_version,
                s.abilities_version,
                s.velocity_version,
            );
        }
        match connection.status() {
            Status::Closed(reason) => bail!(reason),
            Status::Rejected(text) => return Err(super::component::Disconnect(text).into()),
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    connection.cancel();
    // This developer-only CLI may wait briefly for the network journal to finish.
    // Window-thread cancellation remains non-blocking.
    let finish = Instant::now() + Duration::from_secs(1);
    while !matches!(
        connection.status(),
        Status::Cancelled | Status::Closed(_) | Status::Rejected(_)
    ) && Instant::now() < finish
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    if let Some(last) = connection.take_snapshot() {
        snapshot = Some(last);
    }
    let report = snapshot.map_or_else(SessionReport::default, |s| s.report.clone());
    println!(
        "Idle gameplay diagnostic finished: {ticks} physics ticks, {} received packets, {} chunks",
        report.packets, report.chunks
    );
    Ok(report)
}
