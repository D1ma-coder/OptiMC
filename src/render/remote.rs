//! Coalesced, cancellable server-world meshing outside the window thread.
use crate::{
    net::{
        live::{Connection, Snapshot, Status},
        Address,
    },
    world::{remote_mesh::RemoteMesher, Mesh},
};
use anyhow::{Context, Result};
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread,
};

pub struct Scene {
    pub snapshot: Arc<Snapshot>,
    pub meshes: Vec<Arc<Mesh>>,
    pub pixels: Arc<Vec<u8>>,
    pub texture_versions: Arc<Vec<u64>>,
    pub visibility: Arc<crate::mods::sodium::Graph>,
}
/// Mesh identity belongs to chunk data, not to the player's teleport serial.
/// Original LevelRenderer checks the column under the render camera.
/// A correction within an unchanged column must not wait for a needless remesh.
pub fn compiled_camera_column(
    geometry_epoch: u64,
    current_epoch: u64,
    geometry: &crate::world::remote::RemoteWorld,
    current: &crate::world::remote::RemoteWorld,
    position: [f64; 3],
) -> bool {
    let key = (
        (position[0].floor() as i32).div_euclid(16),
        (position[2].floor() as i32).div_euclid(16),
    );
    geometry_epoch == current_epoch
        && geometry.dimension == current.dimension
        && geometry
            .chunks
            .get(&key)
            .zip(current.chunks.get(&key))
            .is_some_and(|(a, b)| Arc::ptr_eq(a, b))
}
// The GPU also drops mesh/texture identities when the world epoch changes.
// Atlas allocations from a preceding level must follow the same lifetime.
fn prepare_epoch(
    epoch: &mut Option<u64>,
    incoming: u64,
    mesher: &mut RemoteMesher,
    cache: &std::path::Path,
) -> bool {
    if *epoch == Some(incoming) {
        return false;
    }
    *mesher = RemoteMesher::new(cache);
    *epoch = Some(incoming);
    true
}
struct State {
    stop: bool,
    request: Option<Arc<Snapshot>>,
    response: Option<Result<Scene, String>>,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}
pub struct RemoteSession {
    connection: Connection,
    shared: Arc<Shared>,
    pub snapshot: Option<Arc<Snapshot>>,
    animation_frame: std::time::Instant,
    animating: bool,
}
impl RemoteSession {
    pub fn new(cache: PathBuf, address: Address, username: String) -> Result<Self> {
        let connection = Connection::connect_logged(address, username, &cache)?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                stop: false,
                request: None,
                response: None,
            }),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        thread::Builder::new()
            .name("server-mesh".into())
            .spawn(move || {
                let mut mesher = RemoteMesher::new(&cache);
                let mut pixels = Arc::new(Vec::new());
                let mut texture_versions = Arc::new(Vec::new());
                let mut world_epoch = None;
                loop {
                    let snapshot = {
                        let mut state = worker.state.lock().unwrap();
                        while !state.stop && state.request.is_none() {
                            state = worker.wake.wait(state).unwrap();
                        }
                        if state.stop {
                            break;
                        }
                        state.request.take().unwrap()
                    };
                    if prepare_epoch(&mut world_epoch, snapshot.world_epoch, &mut mesher, &cache) {
                        pixels = Arc::new(Vec::new());
                        texture_versions = Arc::new(Vec::new());
                    }
                    let result = mesher
                        .update_while(&snapshot.report.world, || {
                            !worker.state.lock().unwrap().stop
                        })
                        .and_then(|(mut meshes, _)| {
                            meshes.extend(mesher.frame_meshes(&snapshot.report)?);
                            meshes.extend(mesher.sign_meshes(&snapshot.report.world)?);
                            meshes.extend(mesher.chest_meshes(&snapshot.report)?);
                            meshes.extend(mesher.entity_meshes(&snapshot.report)?);
                            if texture_versions.as_slice() != mesher.texture_versions() {
                                pixels = Arc::new(mesher.texture_pixels().to_vec());
                                texture_versions = Arc::new(mesher.texture_versions().to_vec());
                            }
                            Ok(Scene {
                                snapshot,
                                visibility: mesher.visibility(),
                                meshes,
                                pixels: pixels.clone(),
                                texture_versions: texture_versions.clone(),
                            })
                        })
                        .map_err(|error| format!("{error:#}"));
                    let mut state = worker.state.lock().unwrap();
                    if state.stop {
                        break;
                    }
                    state.response = Some(result);
                }
            })
            .context("Starting server mesh worker")?;
        Ok(Self {
            connection,
            shared,
            snapshot: None,
            animation_frame: std::time::Instant::now(),
            animating: false,
        })
    }
    pub fn load_progress(
        &self,
        world_epoch: u64,
        teleport_version: u64,
        compiled: bool,
        camera_y: Option<f64>,
    ) {
        self.connection
            .load_progress(world_epoch, teleport_version, compiled, camera_y);
    }
    pub fn status(&self) -> Status {
        self.connection.status()
    }
    pub fn send_motion(&self, motion: crate::net::movement::Motion) -> Result<()> {
        self.connection.send_motion(motion)
    }
    pub fn request_at(&self, epoch: u64, request: crate::net::request::Request) -> Result<()> {
        self.connection.request(epoch, request)
    }
    pub fn request(&self, request: crate::net::request::Request) -> Result<()> {
        let epoch = self
            .snapshot
            .as_ref()
            .context("No active server snapshot")?
            .world_epoch;
        self.connection.request(epoch, request)
    }
    pub fn poll(&mut self) -> Option<Result<Scene, String>> {
        if let Some(snapshot) = self.connection.take_snapshot() {
            // Keep pose/entity changes available immediately. Geometry only needs
            // a worker update when a column/dimension dependency actually changed.
            let changed = self.snapshot.as_ref().is_none_or(|old| {
                old.world_epoch != snapshot.world_epoch
                    || !same_world(&old.report.world, &snapshot.report.world)
                    || !Arc::ptr_eq(&old.report.animations, &snapshot.report.animations)
                    || !Arc::ptr_eq(&old.report.block_events, &snapshot.report.block_events)
                    || old.report.entities.frame_revision != snapshot.report.entities.frame_revision
                    || old.report.entities.revision != snapshot.report.entities.revision
                    || !Arc::ptr_eq(&old.report.metadata, &snapshot.report.metadata)
                    || !Arc::ptr_eq(&old.report.maps, &snapshot.report.maps)
                    || !same_frame_metadata(&old.report, &snapshot.report)
            });
            self.snapshot = Some(snapshot.clone());
            if changed && snapshot.report.world.dimension.is_some() {
                self.shared.state.lock().unwrap().request = Some(snapshot);
                self.shared.wake.notify_one();
            }
        }
        let now = std::time::Instant::now();
        if let Some(snapshot) = &self.snapshot {
            let active =
                snapshot.report.animations.active(now) || snapshot.report.block_events.active(now);
            if (active || self.animating)
                && now.duration_since(self.animation_frame) >= std::time::Duration::from_millis(16)
            {
                if snapshot.report.world.dimension.is_some() {
                    self.shared.state.lock().unwrap().request = Some(snapshot.clone());
                    self.shared.wake.notify_one();
                }
                self.animation_frame = now;
                self.animating = active;
            }
        }
        self.shared.state.lock().unwrap().response.take()
    }
}
fn same_frame_metadata(a: &crate::net::SessionReport, b: &crate::net::SessionReport) -> bool {
    if Arc::ptr_eq(&a.metadata, &b.metadata) {
        return true;
    }
    b.entities
        .entities
        .values()
        .filter(|entity| {
            matches!(
                entity.type_name().ok().flatten(),
                Some("minecraft:item_frame" | "minecraft:glow_item_frame")
            )
        })
        .all(|entity| {
            match (
                a.metadata.entities.get(&entity.id),
                b.metadata.entities.get(&entity.id),
            ) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
        })
}
fn same_world(
    a: &crate::world::remote::RemoteWorld,
    b: &crate::world::remote::RemoteWorld,
) -> bool {
    a.block_entities.len() == b.block_entities.len()
        && a.block_entities.iter().all(|(key, value)| {
            b.block_entities
                .get(key)
                .is_some_and(|other| Arc::ptr_eq(value, other))
        })
        && a.dimension == b.dimension
        && a.spawn.as_ref().map(|s| &s.dimension) == b.spawn.as_ref().map(|s| &s.dimension)
        && a.chunks.len() == b.chunks.len()
        && a.chunks.iter().all(|(key, column)| {
            b.chunks
                .get(key)
                .is_some_and(|other| Arc::ptr_eq(column, other))
        })
}
impl Drop for RemoteSession {
    fn drop(&mut self) {
        self.connection.cancel();
        let mut state = self.shared.state.lock().unwrap();
        state.stop = true;
        state.request = None;
        state.response = None;
        self.shared.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compilation_tracks_current_column_identity_across_position_corrections() {
        let mut world = crate::world::remote::RemoteWorld::default();
        world.chunks.insert(
            (-1, 2),
            Arc::new(crate::world::remote::Column { sections: vec![] }),
        );
        let geometry = world.clone();
        assert!(compiled_camera_column(
            7,
            7,
            &geometry,
            &world,
            [-1., 0., 32.]
        ));
        assert!(compiled_camera_column(
            7,
            7,
            &geometry,
            &world,
            [-15., 0., 47.]
        ));
        assert!(!compiled_camera_column(
            7,
            8,
            &geometry,
            &world,
            [-1., 0., 32.]
        ));
        assert!(!compiled_camera_column(
            7,
            7,
            &geometry,
            &world,
            [0., 0., 32.]
        ));
        world.chunks.insert(
            (-1, 2),
            Arc::new(crate::world::remote::Column { sections: vec![] }),
        );
        assert!(!compiled_camera_column(
            7,
            7,
            &geometry,
            &world,
            [-1., 0., 32.]
        ));
    }
    #[test]
    #[ignore = "Requires original stone PNG; CPU only"]
    fn world_epoch_releases_the_previous_texture_atlas_only_once() {
        let cache = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".cache");
        let mut mesher = RemoteMesher::new(&cache);
        let mut epoch = None;
        assert!(prepare_epoch(&mut epoch, 7, &mut mesher, &cache));
        let mut world = crate::world::remote::RemoteWorld {
            dimension: Some(crate::net::registry::Dimension {
                name: "minecraft:overworld".into(),
                min_y: 0,
                height: 16,
                logical_height: 16,
                skylight: true,
                ceiling: false,
            }),
            ..Default::default()
        };
        let stone = crate::assets::builtins::get()
            .unwrap()
            .default_state("minecraft:stone")
            .unwrap();
        let mut blocks = vec![0; 4096];
        blocks[0] = stone;
        world
            .insert(crate::net::chunk::Chunk {
                x: 0,
                z: 0,
                sections: vec![crate::net::chunk::Section {
                    non_air: 1,
                    fluids: 0,
                    blocks,
                    biomes: vec![0; 64],
                }],
                block_entities: vec![],
            })
            .unwrap();
        mesher.update(&world).unwrap();
        let pixels = mesher.texture_pixels().len();
        assert!(pixels > 0);
        assert!(!prepare_epoch(&mut epoch, 7, &mut mesher, &cache));
        assert_eq!(mesher.texture_pixels().len(), pixels);
        assert!(prepare_epoch(&mut epoch, 8, &mut mesher, &cache));
        assert!(mesher.texture_pixels().is_empty());
        assert!(mesher.texture_versions().is_empty());
        assert!(mesher.visibility().sections.is_empty());
        mesher.update(&world).unwrap();
        assert_eq!(mesher.texture_pixels().len(), pixels);
    }
}
