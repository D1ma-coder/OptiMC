pub mod abilities;
pub mod animation;
pub mod attributes;
mod batch;
pub mod block_entity;
pub mod block_event;
mod bundle;
pub mod chat;
pub mod chunk;
pub mod codec;
pub mod commands;
pub mod component;
pub mod container;
pub mod cookie;
pub mod effects;
pub mod entity;
pub mod environment;
pub mod headless;
pub mod hud;
pub mod item_defaults;
pub mod items;
mod java_case;
pub mod journal;
pub mod last_seen;
pub mod light;
pub mod live;
pub mod loading;
pub mod maps;
pub mod menu_click;
pub mod metadata;
pub mod movement;
pub mod nbt;
pub mod player_info;
pub mod protocol;
pub mod registry;
pub mod request;
pub mod scoreboard;
pub mod sign_edit;
pub mod spawn;
pub mod tags;
pub mod vector;
pub mod vitals;
use crate::assets::PROTOCOL;
use anyhow::{bail, ensure, Context, Result};
use codec::{read_string, read_varint, string, varint, Wire};
use md5::{Digest, Md5};
use serde_json::Value;
use std::{
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct Address {
    pub host: String,
    pub port: u16,
}
impl Address {
    pub fn parse(input: &str) -> Result<Self> {
        let (host, port) = if let Some(rest) = input.strip_prefix('[') {
            let (host, tail) = rest.split_once(']').context("Unclosed IPv6 bracket")?;
            (
                host.to_owned(),
                if tail.is_empty() {
                    25565
                } else {
                    tail.strip_prefix(':').context("Invalid address")?.parse()?
                },
            )
        } else if input.matches(':').count() == 1 {
            let (h, p) = input.rsplit_once(':').unwrap();
            (h.to_owned(), p.parse()?)
        } else {
            (input.to_owned(), 25565)
        };
        ensure!(
            !host.is_empty() && host.len() <= 255 && port != 0,
            "Invalid server address"
        );
        Ok(Self { host, port })
    }
    fn connect(&self) -> Result<Wire<TcpStream>> {
        let mut last = None;
        for address in (self.host.as_str(), self.port).to_socket_addrs()? {
            match TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
                Ok(stream) => {
                    stream.set_nodelay(true)?;
                    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
                    return Ok(Wire {
                        stream,
                        compression: None,
                        journal: None,
                    });
                }
                Err(e) => last = Some(e),
            }
        }
        bail!("Cannot connect to {}:{}: {:?}", self.host, self.port, last)
    }
    fn handshake(&self, wire: &mut Wire<TcpStream>, state: i32) -> Result<()> {
        let mut body = Vec::new();
        varint(PROTOCOL, &mut body);
        string(&self.host, &mut body);
        body.extend(self.port.to_be_bytes());
        varint(state, &mut body);
        wire.send(0, &body)
    }
}
pub fn status(address: &Address) -> Result<Value> {
    let mut wire = address.connect()?;
    wire.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    wire.stream
        .set_write_timeout(Some(Duration::from_secs(5)))?;
    address.handshake(&mut wire, 1)?;
    wire.send(0, &[])?;
    let (id, data) = wire.receive()?;
    ensure!(id == 0, "Expected status response");
    let mut value: Value = serde_json::from_str(&read_string(&mut data.as_slice(), 32767)?)?;
    ensure!(value.is_object(), "Invalid server status object");
    let start = Instant::now();
    let payload = 0x4f5054494d43i64.to_be_bytes();
    wire.send(1, &payload)?;
    let (id, pong) = wire.receive()?;
    ensure!(id == 1 && pong == payload, "Invalid status pong");
    value["optimc_latency_ms"] = serde_json::json!(start.elapsed().as_secs_f64() * 1000.);
    Ok(value)
}
pub fn offline_uuid(name: &str) -> [u8; 16] {
    let mut out: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}")).into();
    out[6] = (out[6] & 0x0f) | 0x30;
    out[8] = (out[8] & 0x3f) | 0x80;
    out
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Login,
    Configuration,
    Play,
}
fn client_information() -> Vec<u8> {
    let mut out = vec![];
    string("en_us", &mut out);
    out.push(12);
    varint(0, &mut out);
    out.extend([1, 127]);
    varint(1, &mut out);
    out.extend([0, 1]);
    varint(0, &mut out);
    out
}
/// Diagnostic offline-mode session. Handles negotiation and liveness, but does not
/// yet feed server chunks to the graphical world. Unknown packets are counted.
pub fn session(address: &Address, name: &str, seconds: u64) -> Result<()> {
    session_state(address, name, seconds).map(|_| ())
}
#[derive(Clone, Debug)]
pub struct ServerMessage {
    pub text: crate::text::RichText,
    pub overlay: bool,
    pub received: Instant,
    pub signature: Option<last_seen::Signature>,
    pub delete_at: Option<Instant>,
}
#[derive(Clone, Default)]
pub struct SessionReport {
    pub packets: u64,
    pub messages: std::sync::Arc<std::collections::VecDeque<ServerMessage>>,
    pub chunks: u64,
    pub lights: std::sync::Arc<light::Store>,
    pub environment: std::sync::Arc<environment::State>,
    pub server_brand: Option<std::sync::Arc<str>>,
    pub vitals: vitals::Vitals,
    pub attributes: std::sync::Arc<attributes::Store>,
    pub effects: std::sync::Arc<effects::Store>,
    pub tags: std::sync::Arc<tags::Store>,
    pub selected_slot: Option<(u8, u64)>,
    pub last_block_ack: Option<i32>,
    pub sign_editor: std::sync::Arc<sign_edit::Store>,
    pub inventory: std::sync::Arc<container::Store>,
    pub hud: std::sync::Arc<hud::State>,
    pub scoreboard: std::sync::Arc<scoreboard::Store>,
    pub maps: std::sync::Arc<maps::Store>,
    pub metadata: std::sync::Arc<metadata::Store>,
    pub player_info: std::sync::Arc<player_info::Store>,
    pub commands: std::sync::Arc<commands::Store>,
    pub entity_packets: u64,
    pub unhandled: std::collections::BTreeMap<String, u64>,
    pub entities: entity::Tracker,
    pub animations: std::sync::Arc<animation::Store>,
    pub block_events: std::sync::Arc<block_event::Store>,
    pub registries: registry::Registries,
    pub world: crate::world::remote::RemoteWorld,
}
pub fn session_state(address: &Address, name: &str, seconds: u64) -> Result<SessionReport> {
    session_state_connection(
        live::Connection::connect(address.clone(), name.to_owned())?,
        seconds,
    )
}
pub fn session_state_logged(
    address: &Address,
    name: &str,
    seconds: u64,
    cache: &std::path::Path,
) -> Result<SessionReport> {
    session_state_connection(
        live::Connection::connect_logged(address.clone(), name.to_owned(), cache)?,
        seconds,
    )
}
fn session_state_connection(connection: live::Connection, seconds: u64) -> Result<SessionReport> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut report = SessionReport::default();
    while Instant::now() < deadline {
        if let Some(snapshot) = connection.take_snapshot() {
            report = snapshot.report.clone();
        }
        match connection.status() {
            live::Status::Closed(reason) => bail!("{reason}"),
            live::Status::Rejected(reason) => return Err(component::Disconnect(reason).into()),
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    connection.cancel();
    // This CLI function may wait for a final snapshot; Connection::cancel and
    // Drop themselves remain non-blocking for a graphical caller.
    let finish = Instant::now() + Duration::from_secs(1);
    while !matches!(
        connection.status(),
        live::Status::Cancelled | live::Status::Closed(_) | live::Status::Rejected(_)
    ) && Instant::now() < finish
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    if let Some(snapshot) = connection.take_snapshot() {
        report = snapshot.report.clone();
    }
    println!(
        "Session finished: {} packets, {} chunk packets, {} entity packets, {} loaded entities",
        report.packets,
        report.chunks,
        report.entity_packets,
        report.entities.entities.len()
    );
    Ok(report)
}
fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.len() <= 16
            && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'),
        "Username must contain 1–16 ASCII letters, digits or underscores"
    );
    Ok(())
}
fn begin_login(address: &Address, name: &str, wire: &mut Wire<TcpStream>) -> Result<()> {
    address.handshake(wire, 2)?;
    wire.set_phase("login");
    let mut hello = Vec::new();
    string(name, &mut hello);
    hello.extend(offline_uuid(name));
    wire.send(0, &hello)?;
    Ok(())
}
pub(crate) struct SessionMachine {
    phase: State,
    pub report: SessionReport,
    pub position: [f64; 3],
    pub rotation: [f32; 2],
    pub velocity: [f64; 3],
    pub teleport_version: u64,
    pub world_epoch: u64,
    pub position_ready: bool,
    pub abilities: abilities::Abilities,
    pub abilities_version: u64,
    pub velocity_version: u64,
    pub player_state_version: u64,
    pub client_loaded: bool,
    pub loading: loading::Tracker,
    bundle: bundle::Buffer,
    cookies: cookie::Store,
    chunk_batches: batch::Calculator,
    last_seen: last_seen::Tracker,
    chat: chat::State,
}
impl SessionMachine {
    fn new() -> Self {
        Self {
            phase: State::Login,
            report: SessionReport::default(),
            position: [0.; 3],
            rotation: [0.; 2],
            velocity: [0.; 3],
            teleport_version: 0,
            world_epoch: 0,
            position_ready: false,
            abilities: abilities::Abilities::default(),
            abilities_version: 0,
            velocity_version: 0,
            player_state_version: 0,
            client_loaded: false,
            loading: Default::default(),
            bundle: bundle::Buffer::default(),
            cookies: cookie::Store::default(),
            chunk_batches: batch::Calculator::default(),
            last_seen: Default::default(),
            chat: Default::default(),
        }
    }
    fn local_motion(&mut self, mut data: &[u8]) -> Result<bool> {
        let entity = read_varint(&mut data)?;
        if Some(entity) != self.report.world.player_id {
            return Ok(false);
        }
        let velocity = vector::read(&mut data)?;
        ensure!(
            data.is_empty() && velocity.is_finite(),
            "Invalid local player velocity"
        );
        self.velocity = velocity.to_array();
        self.velocity_version = self.velocity_version.wrapping_add(1);
        Ok(true)
    }
    fn handle(&mut self, id: i32, data: &[u8], wire: &mut Wire<TcpStream>) -> Result<()> {
        if std::env::var_os("OPTIMC_NET_TRACE").is_some() {
            eprintln!("packet {:?} 0x{id:02x} ({} bytes)", self.phase, data.len());
            if (self.phase == State::Configuration && id == 1)
                || (self.phase == State::Play && id == 0x18)
            {
                if let Ok(channel) = read_string(&mut &data[..], 32767) {
                    eprintln!("custom payload channel: {channel}");
                }
            }
        }
        self.report.packets += 1;
        if self.phase == State::Play && (id == 0 || self.bundle.is_open()) {
            if let Some(packets) = self.bundle.receive(id, data)? {
                for (id, body) in packets {
                    self.apply(id, &body, wire)?;
                }
            }
            return Ok(());
        }
        self.apply(id, data, wire)
    }
    fn apply(&mut self, id: i32, mut data: &[u8], wire: &mut Wire<TcpStream>) -> Result<()> {
        if (self.phase == State::Configuration && id == 1)
            || (self.phase == State::Play && id == 0x18)
        {
            if let Some(brand) = environment::brand(data)? {
                self.report.server_brand = Some(brand.into());
                return Ok(());
            }
        }
        if self.phase == State::Play && id == 0x26 {
            if std::sync::Arc::make_mut(&mut self.report.environment).weather_event(data)? {
                return Ok(());
            }
            // The source ignores this action's float parameter (all bit patterns).
            if data.len() == 5 && data[0] == 13 {
                self.loading.loading_packets_received();
                return Ok(());
            }
            if let Some(mode) = environment::game_mode(data)? {
                if let Some(spawn) = &mut self.report.world.spawn {
                    if spawn.game_mode != mode {
                        spawn.previous_mode = Some(spawn.game_mode);
                    }
                    spawn.game_mode = mode;
                    self.abilities.apply_game_mode(mode);
                    self.abilities_version = self.abilities_version.wrapping_add(1);
                    self.player_state_version = self.player_state_version.wrapping_add(1);
                }
                return Ok(());
            }
        }
        if self.phase == State::Play
            && id == 0x07
            && std::sync::Arc::make_mut(&mut self.report.block_events)
                .packet(&self.report.world, data)?
        {
            return Ok(());
        }
        if self.phase == State::Play && id == 0x02 {
            let packet = animation::Packet::decode(data)?;
            if let Some(off_hand) = packet.off_hand() {
                if self.report.entities.entities.contains_key(&packet.entity)
                    || self.report.world.player_id == Some(packet.entity)
                {
                    // Equipment-specific swing_animation is not synchronized yet.
                    let haste = self
                        .report
                        .effects
                        .amplifier(packet.entity, "minecraft:haste")
                        .into_iter()
                        .chain(
                            self.report
                                .effects
                                .amplifier(packet.entity, "minecraft:conduit_power"),
                        )
                        .max();
                    let fatigue = self
                        .report
                        .effects
                        .amplifier(packet.entity, "minecraft:mining_fatigue");
                    let ticks = if let Some(level) = haste {
                        6i32.saturating_sub(level.saturating_add(1))
                    } else if let Some(level) = fatigue {
                        6i32.saturating_add(level.saturating_add(1).saturating_mul(2))
                    } else {
                        6
                    };
                    std::sync::Arc::make_mut(&mut self.report.animations).start(
                        packet.entity,
                        off_hand,
                        ticks,
                        Instant::now(),
                    )?;
                }
                return Ok(());
            }
        }
        if self.phase == State::Play && id == 0x33 {
            return std::sync::Arc::make_mut(&mut self.report.maps).packet(data);
        }
        if self.phase == State::Play && id == 0x63 {
            let mut header = data;
            let entity = read_varint(&mut header)?;
            if self.report.entities.entities.contains_key(&entity)
                || self.report.world.player_id == Some(entity)
            {
                if std::sync::Arc::make_mut(&mut self.report.metadata).packet(data)? {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }
        if self.phase == State::Play && scoreboard::Store::handles(id) {
            return std::sync::Arc::make_mut(&mut self.report.scoreboard).packet(id, data);
        }
        if self.phase == State::Play && id == 0x83 {
            let update = attributes::Update::decode(data)?;
            let local = Some(update.entity) == self.report.world.player_id;
            if local || self.report.entities.entities.contains_key(&update.entity) {
                if std::env::var_os("OPTIMC_NET_TRACE").is_some() {
                    for (id, value) in &update.values {
                        eprintln!(
                            "attribute entity={} {} = {}",
                            update.entity,
                            crate::assets::builtins::get()?
                                .name("minecraft:attribute", *id)
                                .unwrap_or("unknown"),
                            value.value
                        );
                    }
                }
                std::sync::Arc::make_mut(&mut self.report.attributes).apply(update)?;
                if local {
                    self.player_state_version = self.player_state_version.wrapping_add(1);
                }
            }
            return Ok(());
        }
        if self.phase == State::Play && hud::State::handles(id) {
            std::sync::Arc::make_mut(&mut self.report.hud).packet(id, data)?;
            return Ok(());
        }
        if self.phase == State::Play && id == 0x65 && self.local_motion(data)? {
            self.report.entity_packets += 1;
            return Ok(());
        }

        if self.phase == State::Play
            && self
                .report
                .entities
                .packet(id, data)
                .context("Decoding protocol 776 entity state")?
        {
            self.report.entity_packets += 1;
            if id == 0x4d {
                std::sync::Arc::make_mut(&mut self.report.animations)
                    .swings
                    .retain(|id, _| {
                        self.report.entities.entities.contains_key(id)
                            || Some(*id) == self.report.world.player_id
                    });
                let local = self.report.world.player_id;
                std::sync::Arc::make_mut(&mut self.report.attributes)
                    .entities
                    .retain(|id, _| {
                        Some(*id) == local || self.report.entities.entities.contains_key(id)
                    });
                std::sync::Arc::make_mut(&mut self.report.metadata)
                    .entities
                    .retain(|id, _| {
                        Some(*id) == local || self.report.entities.entities.contains_key(id)
                    });
            } else if id == 1 {
                let entity = read_varint(&mut data)?;
                std::sync::Arc::make_mut(&mut self.report.animations)
                    .swings
                    .remove(&entity);
                std::sync::Arc::make_mut(&mut self.report.metadata)
                    .entities
                    .remove(&entity);
            }
            return Ok(());
        }
        match (self.phase,id) {
            (State::Login,0)=>{let raw=read_string(&mut data,32767)?;return Err(component::Disconnect(component::json_rich(&raw).context("Invalid login disconnect component")?).into());},
            (State::Login,1)=>bail!("Server requires online authentication; offline-mode connection is unavailable"),
            (State::Login,2)=>{
                wire.send(3,&[])?;self.phase=State::Configuration;wire.set_phase("configuration");
                let mut brand=Vec::new();string("minecraft:brand",&mut brand);string("OptiMC",&mut brand);wire.send(2,&brand)?;
                wire.send(0,&client_information())?;
                println!("Login accepted; entering configuration (protocol {PROTOCOL})");
            }
            (State::Login,3)=>{
                let n=read_varint(&mut data)?;ensure!(data.is_empty(),"Trailing compression threshold bytes");
                // Connection.setupCompression removes compression handlers for a negative threshold.
                wire.compression=if n<0 {None} else {Some(n as usize)};
                if let Some(j)=&wire.journal {j.compression(wire.compression);}
            }
            (State::Login,4)=>{let tx=read_varint(&mut data)?;let mut out=vec![];varint(tx,&mut out);out.push(0);wire.send(2,&out)?;}
            (State::Login,5)=>cookie(&self.cookies,wire,4,data)?,
            (State::Configuration,0x0a)|(State::Play,0x78)=>self.cookies.store(data)?,
            (State::Configuration,0)=>cookie(&self.cookies,wire,1,data)?,
            (State::Configuration,2)|(State::Play,0x20)=>return Err(component::Disconnect(component::network_rich(data).context("Invalid server disconnect component")?).into()),
            (State::Configuration,3)=>{wire.send(3,&[])?;self.phase=State::Play;wire.set_phase("play");println!("Configuration complete; play session established");}
            (State::Configuration,4)=>{ensure!(data.len()==8,"Invalid keepalive");wire.send(4,data)?;}
            (State::Configuration,5)=>{ensure!(data.len()==4,"Invalid ping");wire.send(5,data)?;}
            (State::Configuration,7)=>self.report.registries.packet(data).context("Decoding configuration registry")?,
            (State::Configuration,9)=>decline_pack(wire,6,data)?,
            (State::Configuration,0x0e)=>{let mut out=vec![];varint(0,&mut out);wire.send(7,&out)?;}
            (State::Configuration,0x13)=>bail!("Server code of conduct requires user acceptance; interactive prompt not yet implemented"),
            (State::Play,0x31)=>{
                self.client_loaded=false;self.loading.start(Instant::now());
                self.abilities=abilities::Abilities::default();self.abilities_version=self.abilities_version.wrapping_add(1);
                self.world_epoch=self.world_epoch.wrapping_add(1);self.position_ready=false;self.report.last_block_ack=None;self.report.sign_editor=Default::default();self.report.inventory=Default::default();self.report.effects=Default::default();
                let header=spawn::login(data).context("Decoding play login")?;
                self.abilities.apply_game_mode(header.spawn.game_mode);
                self.report.environment=std::sync::Arc::new(environment::State {chunk_radius:Some(header.view_distance),simulation_distance:Some(header.simulation_distance as i32),..Default::default()});
                self.report.world=crate::world::remote::RemoteWorld::default();self.report.world.player_id=Some(header.player_id);
                self.report.world.enter(header.spawn,&self.report.registries)?;self.report.entities.entities.clear();self.report.animations=Default::default();self.report.block_events=Default::default();self.report.attributes=Default::default();self.report.selected_slot=None;self.report.scoreboard=Default::default();self.report.player_info=Default::default();self.report.commands=Default::default();self.chat=Default::default();self.last_seen=Default::default();self.report.maps=Default::default();self.report.lights=Default::default();self.report.metadata=Default::default();self.player_state_version=self.player_state_version.wrapping_add(1);
            }
            (State::Play,0x52)=>{
                self.client_loaded=false;self.loading.start(Instant::now());
                self.world_epoch=self.world_epoch.wrapping_add(1);self.position_ready=false;self.report.last_block_ack=None;self.report.sign_editor=Default::default();self.report.inventory=Default::default();self.report.effects=Default::default();
                let (spawn,keep)=spawn::respawn(data).context("Decoding respawn")?;
                self.abilities.apply_game_mode(spawn.game_mode);self.abilities_version=self.abilities_version.wrapping_add(1);
                if let Some(local)=self.report.world.player_id {std::sync::Arc::make_mut(&mut self.report.attributes).respawn(local,keep&1!=0)?;self.player_state_version=self.player_state_version.wrapping_add(1);}
                if self.report.world.enter(spawn,&self.report.registries)? {std::sync::Arc::make_mut(&mut self.report.environment).chunk_center=None;std::sync::Arc::make_mut(&mut self.report.environment).rain_level=0.;std::sync::Arc::make_mut(&mut self.report.environment).thunder_level=0.;self.report.lights=Default::default();self.report.entities.entities.clear();self.report.animations=Default::default();self.report.block_events=Default::default();self.report.metadata=Default::default();let local=self.report.world.player_id;std::sync::Arc::make_mut(&mut self.report.attributes).entities.retain(|id,_|Some(*id)==local);}
            }
            (State::Play,n) if container::Store::handles(n)=>{
                if let Err(error)=std::sync::Arc::make_mut(&mut self.report.inventory).packet(n,data){
                    if let Some(component)=error.downcast_ref::<items::UnsupportedComponent>() {
                        let first = self.report.unhandled.entry("play:inventory:unsupported_component".into()).or_default();
                        *first += 1;
                        if *first == 1 { eprintln!("Server inventory packet rejected: {component}"); }
                        // Component names originate in the bounded official registry.
                        *self.report.unhandled.entry(format!("play:inventory:unsupported:{}",component.0)).or_default()+=1;
                    }else{return Err(error.context("Decoding server inventory"));}
                }
            },
            (State::Configuration,0x0d)|(State::Play,0x86)=>std::sync::Arc::make_mut(&mut self.report.tags).packet(data)?,
            (State::Play,0x84)|(State::Play,0x4e)=>std::sync::Arc::make_mut(&mut self.report.effects).packet(id,data)?,
            (State::Play,0x04)=>{let sequence=read_varint(&mut data)?;ensure!(data.is_empty(),"Trailing block ack bytes");self.report.last_block_ack=Some(sequence);},
            (State::Play,0x69)=>{let slot=read_varint(&mut data)?;ensure!(data.is_empty(),"Trailing held slot bytes");if (0..9).contains(&slot){let serial=self.report.selected_slot.map_or(1,|(_,serial)|serial.wrapping_add(1));self.report.selected_slot=Some((slot as u8,serial));self.player_state_version=self.player_state_version.wrapping_add(1);}},
            (State::Play,0x45)|(State::Play,0x46)|(State::Play,0x7a)=>std::sync::Arc::make_mut(&mut self.report.player_info).packet(id,data)?,
            (State::Play,0x0f)|(State::Play,0x10)|(State::Play,0x17)=>std::sync::Arc::make_mut(&mut self.report.commands).packet(id,data)?,
            (State::Play,0x3c)=>sign_edit::open(&mut self.report,data)?,
            (State::Play,0x0a)=>std::sync::Arc::make_mut(&mut self.report.environment).difficulty(data)?,
            (State::Play,0x6f)=>std::sync::Arc::make_mut(&mut self.report.environment).simulation_distance(data)?,
            (State::Play,0x61)=>std::sync::Arc::make_mut(&mut self.report.environment).default_spawn=Some(environment::RespawnData::decode(data)?),
            (State::Play,0x71)=>std::sync::Arc::make_mut(&mut self.report.environment).time(data,&self.report.registries)?,
            (State::Play,0x5e)=>std::sync::Arc::make_mut(&mut self.report.environment).center(data)?,
            (State::Play,0x5f)=>std::sync::Arc::make_mut(&mut self.report.environment).radius(data)?,
            (State::Play,0x68)=>self.report.vitals=vitals::Vitals::decode(data)?,
            (State::Play,0x79)=>{
                ensure!(data.len()>=2 && data[data.len()-1]<=1,"Invalid system chat packet");
                let text=component::network_rich(&data[..data.len()-1])?;
                let messages=std::sync::Arc::make_mut(&mut self.report.messages);
                if messages.len()==100 {messages.pop_front();}
                messages.push_back(ServerMessage{text,overlay:data[data.len()-1]!=0,received:Instant::now(),signature:None,delete_at:None});
            }
            (State::Play,0x41)=>{
                let message=self.chat.player(data,&self.report.registries)?;
                let shown=message.text.is_some();
                if let Some(text)=message.text {let messages=std::sync::Arc::make_mut(&mut self.report.messages);if messages.len()==100 {messages.pop_front();}messages.push_back(ServerMessage{text,overlay:false,received:Instant::now(),signature:message.signature,delete_at:None});}
                if self.last_seen.add(message.signature,shown)&&self.last_seen.offset>64 {let mut out=Vec::new();varint(self.last_seen.take_offset(),&mut out);wire.send(0x06,&out)?;}
            }
            (State::Play,0x21)=>{
                let text=self.chat.disguised(data,&self.report.registries)?;let messages=std::sync::Arc::make_mut(&mut self.report.messages);if messages.len()==100{messages.pop_front();}messages.push_back(ServerMessage{text,overlay:false,received:Instant::now(),signature:None,delete_at:None});
            }
            (State::Play,0x1f)=>{
                let signature=self.chat.deleted(data)?;self.last_seen.ignore(&signature);
                for message in std::sync::Arc::make_mut(&mut self.report.messages) {if message.signature==Some(signature){message.delete_at=Some(Instant::now().max(message.received+Duration::from_secs(3)));}}
            }
            (State::Play,0x06)=>self.report.world.block_entity_update(data)?,
            (State::Play,0x08)=>{self.report.world.update(data)?;std::sync::Arc::make_mut(&mut self.report.block_events).retain_world(&self.report.world);},
            (State::Play,0x40)=>{self.abilities=abilities::Abilities::decode(data)?;self.abilities_version=self.abilities_version.wrapping_add(1);}
            (State::Play,0x25)=>{self.report.world.forget(data)?;let packed=i64::from_be_bytes(data.try_into()?);std::sync::Arc::make_mut(&mut self.report.lights).forget(packed as i32,(packed>>32) as i32);std::sync::Arc::make_mut(&mut self.report.block_events).retain_world(&self.report.world);},
            (State::Play,0x54)=>{self.report.world.section_update(data)?;std::sync::Arc::make_mut(&mut self.report.block_events).retain_world(&self.report.world);},
            (State::Play,0x15)=>cookie(&self.cookies,wire,0x15,data)?,
            (State::Play,0x2c)=>{ensure!(data.len()==8,"Invalid play keepalive");wire.send(0x1c,data)?;}
            (State::Play,0x3d)=>{ensure!(data.len()==4,"Invalid play ping");wire.send(0x2d,data)?;}
            (State::Play,0x0c)=>{ensure!(data.is_empty(),"Trailing chunk batch start");self.chunk_batches.start();}
            (State::Play,0x0b)=>{let count=read_varint(&mut data)?;ensure!(data.is_empty(),"Trailing chunk batch finish");let desired=self.chunk_batches.finish(count);wire.send(0x0b,&desired.to_be_bytes())?;}
            (State::Play,0x30)=>{
                let (column, update)=light::Data::packet(data)?;
                let d=self.report.world.dimension.as_ref().context("Light update before dimension")?;
                std::sync::Arc::make_mut(&mut self.report.lights).apply(column,d.min_y,d.height,update)?;
            }
            (State::Play,0x2d)=>{
                let count=self.report.world.dimension.as_ref().context("Chunk before dimension")?.height as usize/16;
                let (decoded,light_data) = chunk::decode_with_light(data,count).context("Decoding protocol 776 chunk")?;
                self.report.chunks += 1;
                if let Some(j)=&wire.journal {j.chunk(&decoded);}
                if self.report.chunks == 1 {println!("Decoded server chunk {},{}: {} sections", decoded.x, decoded.z, decoded.sections.len());}
                std::sync::Arc::make_mut(&mut self.report.block_events).forget_column(decoded.x,decoded.z);
                let column=[decoded.x,decoded.z];
                self.report.world.insert(decoded)?;
                let d=self.report.world.dimension.as_ref().unwrap();
                std::sync::Arc::make_mut(&mut self.report.lights).apply(column,d.min_y,d.height,light_data)?;
            }
            (State::Play,0x48)=>{
                let (teleport, pose) = entity::player_position(data, entity::Pose {
                    position: self.position.into(), velocity: self.velocity.into(),
                    yaw: self.rotation[0], pitch: self.rotation[1],
                })?;
                let mut ack=vec![];varint(teleport,&mut ack);wire.send(0,&ack)?;
                self.position = pose.position.to_array();
                self.velocity = pose.velocity.to_array();
                self.rotation = [pose.yaw, pose.pitch];
                self.teleport_version=self.teleport_version.wrapping_add(1);
                self.position_ready=true;
                let mut out = Vec::new();
                for value in self.position {out.extend(value.to_be_bytes());}
                for value in self.rotation {out.extend(value.to_be_bytes());}
                out.push(0);wire.send(0x1f,&out)?;
            }
            (State::Play,0x51)=>decline_pack(wire,0x31,data)?,
            (State::Play,0x76)=>{self.client_loaded=false;self.loading=Default::default();self.world_epoch=self.world_epoch.wrapping_add(1);self.position_ready=false;self.report.last_block_ack=None;self.report.sign_editor=Default::default();self.report.inventory=Default::default();self.report.effects=Default::default();self.report.entities.entities.clear();self.report.animations=Default::default();self.report.block_events=Default::default();self.report.metadata=Default::default();self.report.maps=Default::default();self.report.lights=Default::default();self.report.player_info=Default::default();self.report.commands=Default::default();self.chat=Default::default();self.last_seen=Default::default();self.report.tags=Default::default();self.report.environment=Default::default();self.report.registries=registry::Registries::default();self.report.world=crate::world::remote::RemoteWorld::default();self.report.attributes=Default::default();self.player_state_version=self.player_state_version.wrapping_add(1);wire.send(0x10,&[])?;self.phase=State::Configuration;wire.set_phase("configuration");wire.send(0,&client_information())?;}
            _=>{
                let phase=match self.phase {State::Login=>"login",State::Configuration=>"configuration",State::Play=>"play"};
                let name=protocol::name(phase,"clientbound",id).unwrap_or("unknown_packet_id");
                let key=format!("{phase}/{name}");
                let count=self.report.unhandled.entry(key).or_default();
                if *count==0 {
                    eprintln!("Unimplemented server packet: {phase} 0x{id:02x} {name} ({} bytes)",data.len());
                    if self.phase==State::Play && id==0x26 && data.len()==5 {
                        eprintln!("Unimplemented game event action={} parameter={}",data[0],f32::from_be_bytes(data[1..].try_into().unwrap()));
                    }
                }
                *count=count.saturating_add(1);
            }
        }

        Ok(())
    }
}
fn cookie(store: &cookie::Store, wire: &mut Wire<TcpStream>, id: i32, data: &[u8]) -> Result<()> {
    wire.send(id, &store.response(data)?)
}

fn decline_pack(wire: &mut Wire<TcpStream>, id: i32, data: &[u8]) -> Result<()> {
    ensure!(data.len() >= 16, "Invalid resource pack request");
    let mut out = data[..16].to_vec();
    varint(1, &mut out);
    wire.send(id, &out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn velocity_for_local_player_is_applied_atomically_without_an_entity_spawn() {
        let mut machine = SessionMachine::new();
        machine.report.world.player_id = Some(99);
        let mut bytes = Vec::new();
        varint(99, &mut bytes);
        vector::write(glam::DVec3::new(1.25, -0.5, 0.75), &mut bytes);
        assert!(machine.local_motion(&bytes).unwrap());
        assert_eq!(machine.velocity_version, 1);
        for (actual, wanted) in machine.velocity.into_iter().zip([1.25, -0.5, 0.75]) {
            assert!((actual - wanted).abs() < 0.001);
        }
        let before = machine.velocity;
        bytes.push(1);
        assert!(machine.local_motion(&bytes).is_err());
        assert_eq!(machine.velocity, before);
        assert_eq!(machine.velocity_version, 1);
        bytes[0] = 98;
        assert!(!machine.local_motion(&bytes).unwrap());
    }
    #[test]
    fn offline_identity_matches_java() {
        assert_eq!(
            offline_uuid("Notch"),
            [
                0xb5, 0x0a, 0xd3, 0x85, 0x82, 0x9d, 0x31, 0x41, 0xa2, 0x16, 0x7e, 0x7d, 0x75, 0x39,
                0xba, 0x7f
            ]
        );
    }
    #[test]
    fn ipv6_and_default_ports() {
        assert_eq!(Address::parse("[::1]:25566").unwrap().port, 25566);
        assert_eq!(Address::parse("::1").unwrap().host, "::1");
        assert_eq!(Address::parse("localhost").unwrap().port, 25565);
        assert!(Address::parse(":25565").is_err());
    }
}
