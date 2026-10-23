//! Socket worker shared by diagnostic and (eventually) graphical connections.
//! One worker owns protocol ordering; the consumer only receives the latest
//! immutable snapshot. No queue of obsolete world copies accumulates.
use super::{codec::FrameBuffer, Address, SessionMachine, SessionReport, State};
use anyhow::{bail, Context, Result};
use std::{
    collections::VecDeque,
    io::{ErrorKind, Read},
    net::{Shutdown, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Login,
    Configuration,
    Playing,
    Cancelled,
    Closed(String),
    Rejected(crate::text::RichText),
}
#[derive(Clone)]
pub struct Snapshot {
    pub report: SessionReport,
    pub position: [f64; 3],
    /// Server yaw/pitch in degrees; position is the player's feet.
    pub rotation: [f32; 2],
    pub velocity: [f64; 3],
    pub teleport_version: u64,
    pub world_epoch: u64,
    pub position_ready: bool,
    pub client_loaded: bool,
    pub loading_packets_received: bool,
    pub abilities: super::abilities::Abilities,
    pub abilities_version: u64,
    pub velocity_version: u64,
    pub player_state_version: u64,
}
struct LoadProgress {
    generation: (u64, u64),
    compiled: bool,
    camera_y: Option<f64>,
}
struct Shared {
    stop: AtomicBool,
    socket: Mutex<Option<TcpStream>>,
    status: Mutex<Status>,
    snapshot: Mutex<Option<Arc<Snapshot>>>,
    motion: Mutex<VecDeque<super::movement::Motion>>,
    loaded: Mutex<Option<LoadProgress>>,
    load_callback_generation: Mutex<Option<(u64, u64)>>,
    requests: Mutex<VecDeque<(u64, super::request::Request)>>,
}
pub struct Connection {
    shared: Arc<Shared>,
}
impl Connection {
    /// DNS, TCP connection and protocol work run off the caller's thread.
    pub fn connect(address: Address, name: String) -> Result<Self> {
        Self::connect_with_journal(address, name, None)
    }
    pub fn connect_logged(address: Address, name: String, cache: &std::path::Path) -> Result<Self> {
        Self::connect_with_journal(address, name, Some(cache.join("network")))
    }
    fn connect_with_journal(
        address: Address,
        name: String,
        journal_dir: Option<std::path::PathBuf>,
    ) -> Result<Self> {
        super::validate_name(&name)?;
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            socket: Mutex::new(None),
            status: Mutex::new(Status::Connecting),
            snapshot: Mutex::new(None),
            motion: Mutex::new(VecDeque::new()),
            loaded: Mutex::new(None),
            load_callback_generation: Mutex::new(None),
            requests: Mutex::new(VecDeque::new()),
        });
        let worker = shared.clone();
        thread::Builder::new()
            .name("optimc-network".into())
            .spawn(move || {
                let result = run(&address, &name, &worker, journal_dir.as_deref());
                let mut status = worker.status.lock().unwrap();
                *status = if worker.stop.load(Ordering::Acquire) {
                    Status::Cancelled
                } else {
                    match result {
                        Err(error)
                            if error
                                .downcast_ref::<super::component::Disconnect>()
                                .is_some() =>
                        {
                            Status::Rejected(
                                error
                                    .downcast_ref::<super::component::Disconnect>()
                                    .unwrap()
                                    .0
                                    .clone(),
                            )
                        }
                        Err(error) => Status::Closed(format!("{error:#}")),
                        Ok(()) => Status::Closed("Server closed the connection".into()),
                    }
                };
                worker.socket.lock().unwrap().take();
            })
            .context("Starting network worker")?;
        Ok(Self { shared })
    }
    pub fn mark_loaded(&self, world_epoch: u64, teleport_version: u64) {
        self.load_progress(world_epoch, teleport_version, true, None);
    }
    /// Report the current camera even while waiting for geometry. The worker
    /// owns the marker/timeout state and emits player_loaded exactly once.
    pub fn load_progress(
        &self,
        world_epoch: u64,
        teleport_version: u64,
        compiled: bool,
        camera_y: Option<f64>,
    ) {
        // Like getPlayerCompiledSectionCallback(): no callback can be obtained
        // from a snapshot which still waits for the server marker.
        let compiled = compiled
            && *self.shared.load_callback_generation.lock().unwrap()
                == Some((world_epoch, teleport_version));
        let mut pending = self.shared.loaded.lock().unwrap();
        let compiled = compiled
            || pending.as_ref().is_some_and(|old| {
                old.generation == (world_epoch, teleport_version) && old.compiled
            });
        *pending = Some(LoadProgress {
            generation: (world_epoch, teleport_version),
            compiled,
            camera_y: camera_y.filter(|y| y.is_finite()),
        });
    }
    pub fn status(&self) -> Status {
        self.shared.status.lock().unwrap().clone()
    }
    pub fn request(&self, world_epoch: u64, request: super::request::Request) -> Result<()> {
        request.validate()?;
        let mut queue = self.shared.requests.lock().unwrap();
        // A superseded completion request has no side effect and is coalesced.
        if matches!(request, super::request::Request::Suggestions { .. }) {
            queue.retain(|(_, r)| !matches!(r, super::request::Request::Suggestions { .. }));
        }
        anyhow::ensure!(queue.len() < 256, "User-action queue full");
        queue.push_back((world_epoch, request));
        Ok(())
    }
    /// Called once per simulated tick; tags fence corrections and world changes.
    pub fn send_motion(&self, motion: super::movement::Motion) -> Result<()> {
        motion.validate()?;
        let mut queue = self.shared.motion.lock().unwrap();
        anyhow::ensure!(queue.len() < 64, "Outgoing player movement queue full");
        queue.push_back(motion);
        Ok(())
    }
    /// Consume the latest snapshot. Repeated polling does not clone the world.
    pub fn take_snapshot(&self) -> Option<Arc<Snapshot>> {
        let snapshot = self.shared.snapshot.lock().unwrap().take();
        if let Some(s) = &snapshot {
            *self.shared.load_callback_generation.lock().unwrap() = s
                .loading_packets_received
                .then_some((s.world_epoch, s.teleport_version));
        }
        snapshot
    }
    /// Interrupt socket reads/writes immediately without joining on the UI
    /// thread. An OS DNS lookup already in flight may finish in the background.
    pub fn cancel(&self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(socket) = self.shared.socket.lock().unwrap().as_ref() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.cancel();
    }
}
fn publish(machine: &SessionMachine, shared: &Shared) {
    *shared.snapshot.lock().unwrap() = Some(Arc::new(Snapshot {
        report: machine.report.clone(),
        position: machine.position,
        rotation: machine.rotation,
        velocity: machine.velocity,
        teleport_version: machine.teleport_version,
        world_epoch: machine.world_epoch,
        position_ready: machine.position_ready,
        client_loaded: machine.client_loaded,
        loading_packets_received: machine.loading.has_loading_packets(),
        abilities: machine.abilities,
        abilities_version: machine.abilities_version,
        velocity_version: machine.velocity_version,
        player_state_version: machine.player_state_version,
    }));
}
fn run(
    address: &Address,
    name: &str,
    shared: &Shared,
    journal_dir: Option<&std::path::Path>,
) -> Result<()> {
    let mut wire = address.connect()?;
    let journal = journal_dir.and_then(|dir| match super::journal::Journal::open(dir) {
        Ok(j) => Some(j),
        Err(e) => {
            eprintln!("Cannot create network journal: {e}");
            None
        }
    });
    wire.journal = journal.clone();
    {
        let mut socket = shared.socket.lock().unwrap();
        if shared.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        *socket = Some(wire.stream.try_clone()?);
    }
    super::begin_login(address, name, &mut wire)?;
    wire.stream
        .set_read_timeout(Some(Duration::from_millis(10)))?;
    *shared.status.lock().unwrap() = Status::Login;
    let mut machine = SessionMachine::new();
    let mut sender = super::movement::Sender::new(super::entity::Pose::default());
    let mut generation = (0, 0);
    let mut frames = FrameBuffer::default();
    let mut bytes = [0; 65536];
    let mut action_sequence = 0i32;
    let mut published = 0;
    let mut published_loaded = false;
    let mut published_sign = None;
    let mut load_camera = None;
    let mut published_inventory = u64::MAX;
    let mut publish_at = Instant::now();
    let result = (|| -> Result<()> {
        while !shared.stop.load(Ordering::Acquire) {
            // Bound each batch so cancellation and publication stay responsive
            // even when a server streams chunks continuously.
            let started = Instant::now();
            let mut processed = 0;
            while processed < 256 && started.elapsed() < Duration::from_millis(5) {
                let Some((id, data)) = frames.next_packet(wire.compression)? else {
                    break;
                };
                if let Some(j) = &journal {
                    j.record("receive", id, data.len());
                }
                machine.handle(id, &data, &mut wire)?;
                processed += 1;
                if shared.stop.load(Ordering::Acquire) {
                    return Ok(());
                }
            }
            let phase = match machine.phase {
                State::Login => Status::Login,
                State::Configuration => Status::Configuration,
                State::Play => Status::Playing,
            };
            *shared.status.lock().unwrap() = phase;
            let current = (machine.world_epoch, machine.teleport_version);
            if current.0 != generation.0 {
                action_sequence = 0;
                // LocalPlayer's xLast/yLast/zLast fields start at zero and are
                // changed only by sendPosition, not by the teleport ACK handler.
                sender = super::movement::Sender::new(super::entity::Pose::default());
            }
            generation = current;
            // An incomplete inbound bundle leaves the committed world intact.
            // Keep ticking against that world instead of building a motion burst.
            if let Some(progress) = shared.loaded.lock().unwrap().take() {
                if machine.phase == State::Play
                    && progress.generation == current
                    && !machine.client_loaded
                {
                    if progress.compiled {
                        machine.loading.compiled_section();
                    }
                    load_camera = progress.camera_y.map(|y| (current, y));
                }
            }
            if machine.phase == State::Play && machine.position_ready && !machine.client_loaded {
                let outside = |y: f64| {
                    machine.report.world.dimension.as_ref().is_some_and(|d| {
                        y.floor() < f64::from(d.min_y)
                            || y.floor() >= f64::from(d.min_y) + f64::from(d.height)
                    })
                };
                let conditions = super::loading::Conditions {
                    player_outside_height: outside(machine.position[1]),
                    camera_outside_height: load_camera
                        .is_some_and(|(tag, y)| tag == current && outside(y)),
                    spectator: machine
                        .report
                        .world
                        .spawn
                        .as_ref()
                        .is_some_and(|s| s.game_mode == 3),
                    alive: machine.report.vitals.health > 0.,
                };
                if machine.loading.tick(Instant::now(), conditions) {
                    wire.send(0x2c, &[])?;
                    machine.client_loaded = true;
                }
            }
            let motions = std::mem::take(&mut *shared.motion.lock().unwrap());
            let requests = std::mem::take(&mut *shared.requests.lock().unwrap());
            for motion in motions {
                if machine.phase != State::Play
                    || !machine.position_ready
                    || !machine.client_loaded
                    || (motion.world_epoch, motion.teleport_version) != current
                    || motion.abilities_version != machine.abilities_version
                    || motion.velocity_version != machine.velocity_version
                    || motion.player_state_version != machine.player_state_version
                {
                    continue;
                }
                let Some(id) = machine.report.world.player_id else {
                    continue;
                };
                if motion.flying && !machine.abilities.may_fly && !machine.abilities.flying {
                    continue;
                }
                if motion.flying != machine.abilities.flying {
                    wire.send(0x28, &[if motion.flying { 2 } else { 0 }])?;
                    machine.abilities.flying = motion.flying;
                }
                for (id, body) in sender.tick(motion, id)? {
                    wire.send(id, &body)?;
                }
                // Minecraft.tick sends this after the player's packets, including stationary ticks.
                wire.send(0x0d, &[])?;
                machine.position = motion.pose.position.to_array();
                machine.velocity = motion.pose.velocity.to_array();
                machine.rotation = [motion.pose.yaw, motion.pose.pitch];
            }
            for (epoch, mut request) in requests {
                if machine.phase != State::Play
                    || epoch != machine.world_epoch
                    || (!machine.client_loaded
                        && !matches!(request, super::request::Request::SignUpdate { .. }))
                {
                    continue;
                }
                use super::request::Request;
                if let Request::SignUpdate {
                    serial,
                    position,
                    front,
                    ..
                } = &request
                {
                    if !machine
                        .report
                        .sign_editor
                        .open
                        .as_ref()
                        .is_some_and(|open| {
                            open.serial == *serial
                                && open.position == *position
                                && open.front == *front
                        })
                    {
                        continue;
                    }
                }
                if let Request::MenuClick {
                    id,
                    serial,
                    state,
                    click,
                } = &mut request
                {
                    let inventory = &machine.report.inventory;
                    let current = inventory.open.as_ref();
                    if *serial != inventory.menu_serial || current.map_or(0, |m| m.id) != *id {
                        continue;
                    }
                    let count = current.map_or(inventory.player.len(), |m| m.slots.len());
                    if click.slot != -999 && (click.slot < 0 || click.slot as usize >= count) {
                        continue;
                    }
                    *state = current.map_or(inventory.player_state, |m| m.state);
                }
                match &request {
                    Request::CloseMenu { id, serial }
                        if *serial != machine.report.inventory.menu_serial
                            || if *id == 0 {
                                machine.report.inventory.open.is_some()
                            } else {
                                !machine
                                    .report
                                    .inventory
                                    .open
                                    .as_ref()
                                    .is_some_and(|menu| menu.id == *id)
                            } =>
                    {
                        continue
                    }
                    Request::Swing {
                        slot,
                        input,
                        teleport_version,
                        ..
                    }
                    | Request::UseItem {
                        slot,
                        input,
                        teleport_version,
                        ..
                    }
                    | Request::Dig {
                        slot,
                        input,
                        teleport_version,
                        ..
                    }
                    | Request::InventoryAction {
                        slot,
                        input,
                        teleport_version,
                        ..
                    }
                    | Request::UseBlock {
                        slot,
                        input,
                        teleport_version,
                        ..
                    }
                    | Request::EntityAction {
                        slot,
                        input,
                        teleport_version,
                        ..
                    } => {
                        if *teleport_version != machine.teleport_version || !machine.position_ready
                        {
                            continue;
                        }
                        if let Request::EntityAction { hit, .. } = &request {
                            if !machine
                                .report
                                .entities
                                .entities
                                .get(&hit.id)
                                .is_some_and(|e| e.uuid == hit.uuid)
                            {
                                continue;
                            }
                        }
                        for (id, body) in sender.sync_input_slot(*slot, *input)? {
                            wire.send(id, &body)?;
                        }
                        if matches!(
                            request,
                            Request::UseBlock { .. }
                                | Request::UseItem { .. }
                                | Request::Dig {
                                    action: super::request::DigAction::Start
                                        | super::request::DigAction::Stop,
                                    ..
                                }
                        ) {
                            action_sequence = action_sequence
                                .checked_add(1)
                                .context("Action sequence exhausted")?;
                        }
                    }
                    _ => {}
                }
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis()
                    .min(i64::MAX as u128) as i64;
                let mut salt = [0; 8];
                getrandom::getrandom(&mut salt).map_err(|e| anyhow::anyhow!("Chat salt: {e}"))?;
                let (id, body) = request.encode_sequenced(
                    timestamp,
                    i64::from_ne_bytes(salt),
                    &mut machine.last_seen,
                    action_sequence,
                )?;
                wire.send(id, &body)?;
                if matches!(request, Request::SignUpdate { .. }) {
                    std::sync::Arc::make_mut(&mut machine.report.sign_editor).open = None;
                }
                if matches!(request, Request::CloseMenu { .. }) {
                    std::sync::Arc::make_mut(&mut machine.report.inventory).close();
                }
                if matches!(request, Request::EntityAction { attack: true, .. }) {
                    wire.send(0x3f, &[0])?;
                }
            }

            if !machine.bundle.is_open()
                && Instant::now() >= publish_at
                && (published != machine.report.packets
                    || published_loaded != machine.client_loaded
                    || published_sign != machine.report.sign_editor.open.as_ref().map(|o| o.serial)
                    || published_inventory != machine.report.inventory.revision)
            {
                publish(&machine, shared);
                published = machine.report.packets;
                published_loaded = machine.client_loaded;
                published_sign = machine.report.sign_editor.open.as_ref().map(|o| o.serial);
                published_inventory = machine.report.inventory.revision;
                publish_at = Instant::now() + Duration::from_millis(50);
            }
            // Drain buffered traffic before reading more; memory stays bounded.
            if processed == 256 || started.elapsed() >= Duration::from_millis(5) {
                continue;
            }
            if let Some(j) = &journal {
                j.flush_if_due();
            }
            match wire.stream.read(&mut bytes) {
                Ok(0) => {
                    if frames.pending_bytes() > 0 {
                        bail!("Server closed the connection with an incomplete packet ({} buffered bytes)",frames.pending_bytes());
                    }
                    if machine.bundle.is_open() {
                        bail!("Server closed the connection with an incomplete bundle during Play");
                    }
                    bail!(
                        "Server closed the connection without a disconnect message during {:?}",
                        machine.phase
                    );
                }
                Ok(count) => frames.append(&bytes[..count])?,
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    })();
    if let Some(j) = &journal {
        if let Err(error) = &result {
            if let Some(disconnect) = error.downcast_ref::<super::component::Disconnect>() {
                j.server_disconnect(&disconnect.0);
            }
        }
        let reason = if shared.stop.load(Ordering::Acquire) {
            "cancelled".into()
        } else {
            match &result {
                Ok(()) => "closed".into(),
                Err(e) if e.downcast_ref::<super::component::Disconnect>().is_some() => {
                    "server_disconnect".into()
                }
                Err(e) => format!("{e:#}"),
            }
        };
        j.finish(&reason);
    }
    if !machine.bundle.is_open()
        && (published != machine.report.packets
            || published_loaded != machine.client_loaded
            || published_inventory != machine.report.inventory.revision)
    {
        publish(&machine, shared);
    }
    result
}
