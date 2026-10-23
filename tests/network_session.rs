use optimc::{
    assets::PROTOCOL,
    net::{
        self,
        codec::{read_string, read_varint, string, varint, Wire},
        Address,
    },
};
use std::{io::Read, net::TcpListener, thread, time::Duration};

/// Exercise real TCP framing, state transitions, compression, cookies, server
/// liveness and absolute/relative teleport acknowledgements against a fixture.
fn world_fixture(live: bool) -> (Address, thread::JoinHandle<()>) {
    world_fixture_gravity(live, 0.3)
}
fn world_fixture_gravity(live: bool, gravity: f64) -> (Address, thread::JoinHandle<()>) {
    world_fixture_options(live, gravity, false)
}
fn world_fixture_options(
    live: bool,
    gravity: f64,
    bundle: bool,
) -> (Address, thread::JoinHandle<()>) {
    world_fixture_transport(live, gravity, bundle, false)
}
fn world_fixture_transport(
    live: bool,
    gravity: f64,
    bundle: bool,
    disable_compression: bool,
) -> (Address, thread::JoinHandle<()>) {
    world_fixture_maps(live, gravity, bundle, disable_compression, false)
}
fn world_fixture_maps(
    live: bool,
    gravity: f64,
    bundle: bool,
    disable_compression: bool,
    maps: bool,
) -> (Address, thread::JoinHandle<()>) {
    world_fixture_actions(live, gravity, bundle, disable_compression, maps, false)
}
fn world_fixture_actions(
    live: bool,
    gravity: f64,
    bundle: bool,
    disable_compression: bool,
    maps: bool,
    actions: bool,
) -> (Address, thread::JoinHandle<()>) {
    world_fixture_picking(
        live,
        gravity,
        bundle,
        disable_compression,
        maps,
        actions,
        false,
    )
}
fn world_fixture_picking(
    live: bool,
    gravity: f64,
    bundle: bool,
    disable_compression: bool,
    maps: bool,
    actions: bool,
    entity_actions: bool,
) -> (Address, thread::JoinHandle<()>) {
    world_fixture_full(
        live,
        gravity,
        bundle,
        disable_compression,
        maps,
        actions,
        (entity_actions, false, false, false, false),
    )
}
fn inventory_reference() -> serde_json::Value {
    use std::io::Read;
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(
        include_bytes!("../data/26.2/inventory-reference.json.gz").as_slice(),
    )
    .read_to_end(&mut raw)
    .unwrap();
    serde_json::from_slice(&raw).unwrap()
}
fn inventory_packet(row: &serde_json::Value) -> (i32, Vec<u8>) {
    use base64::Engine;
    (
        row["id"].as_i64().unwrap() as i32,
        base64::engine::general_purpose::STANDARD
            .decode(row["body"].as_str().unwrap())
            .unwrap(),
    )
}
fn world_fixture_full(
    live: bool,
    gravity: f64,
    bundle: bool,
    disable_compression: bool,
    maps: bool,
    actions: bool,
    extra: (bool, bool, bool, bool, bool),
) -> (Address, thread::JoinHandle<()>) {
    let (entity_actions, inventory, input_actions, delayed_loading, sign_editing) = extra;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut wire = Wire {
            stream,
            compression: None,
            journal: None,
        };
        let (id, bytes) = wire.receive().unwrap();
        assert_eq!(id, 0);
        let mut input = bytes.as_slice();
        assert_eq!(read_varint(&mut input).unwrap(), PROTOCOL);
        assert_eq!(read_string(&mut input, 255).unwrap(), "127.0.0.1");
        let mut p = [0; 2];
        input.read_exact(&mut p).unwrap();
        assert_eq!(u16::from_be_bytes(p), port);
        assert_eq!(read_varint(&mut input).unwrap(), 2);
        let (id, bytes) = wire.receive().unwrap();
        assert_eq!(id, 0);
        let mut input = bytes.as_slice();
        assert_eq!(read_string(&mut input, 16).unwrap(), "Fixture");
        assert_eq!(input, net::offline_uuid("Fixture"));
        wire.send(3, &[32]).unwrap();
        wire.compression = Some(32);
        if disable_compression {
            let mut threshold = Vec::new();
            varint(-1, &mut threshold);
            wire.send(3, &threshold).unwrap();
            wire.compression = None;
        }
        let mut success = net::offline_uuid("Fixture").to_vec();
        string("Fixture", &mut success);
        varint(0, &mut success);
        wire.send(2, &success).unwrap();
        assert_eq!(wire.receive().unwrap(), (3, vec![]));
        let mut brand = Vec::new();
        string("minecraft:brand", &mut brand);
        string("OptiMC", &mut brand);
        assert_eq!(wire.receive().unwrap(), (2, brand));
        assert_eq!(wire.receive().unwrap().0, 0);
        wire.send(0x0e, &[0]).unwrap();
        assert_eq!(wire.receive().unwrap(), (7, vec![0]));
        wire.send(4, &77i64.to_be_bytes()).unwrap();
        assert_eq!(wire.receive().unwrap(), (4, 77i64.to_be_bytes().to_vec()));
        let mut key = vec![];
        string("optimc:test", &mut key);
        wire.send(0, &key).unwrap();
        key.push(0);
        assert_eq!(wire.receive().unwrap(), (1, key));
        let mut cookie_key = Vec::new();
        string("optimc:opaque", &mut cookie_key);
        let mut stored = cookie_key.clone();
        varint(4, &mut stored);
        stored.extend([0, 255, 1, 128]);
        wire.send(0x0a, &stored).unwrap();
        wire.send(0, &cookie_key).unwrap();
        let mut response = cookie_key.clone();
        response.extend([1, 4, 0, 255, 1, 128]);
        assert_eq!(wire.receive().unwrap(), (1, response.clone()));

        let dimension = net::nbt::Tag::Compound(
            [
                ("min_y", net::nbt::Tag::Int(-64)),
                ("height", net::nbt::Tag::Int(384)),
                ("logical_height", net::nbt::Tag::Int(384)),
                ("has_skylight", net::nbt::Tag::Byte(1)),
                ("has_ceiling", net::nbt::Tag::Byte(0)),
            ]
            .into_iter()
            .map(|(key, value)| (net::nbt::NbtString::from(key), value))
            .collect(),
        );
        let mut registry = Vec::new();
        string("minecraft:dimension_type", &mut registry);
        varint(1, &mut registry);
        string("minecraft:overworld", &mut registry);
        registry.push(1);
        net::nbt::write_network(&dimension, &mut registry).unwrap();
        wire.send(7, &registry).unwrap();
        if entity_actions {
            use base64::Engine;
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(
                include_bytes!("../data/26.2/entity-picking.json.gz").as_slice(),
            )
            .read_to_end(&mut raw)
            .unwrap();
            let reference: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            wire.send(
                0x0d,
                &base64::engine::general_purpose::STANDARD
                    .decode(reference["tags_packet"].as_str().unwrap())
                    .unwrap(),
            )
            .unwrap();
        }

        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../data/26.2/world-environment.json.gz").as_slice(),
        )
        .read_to_end(&mut raw)
        .unwrap();
        let environment: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let mut clock_registry = Vec::new();
        string("minecraft:world_clock", &mut clock_registry);
        varint(2, &mut clock_registry);
        for name in environment["registry"].as_array().unwrap() {
            string(name.as_str().unwrap(), &mut clock_registry);
            clock_registry.extend([1, 10, 0]);
        }
        wire.send(7, &clock_registry).unwrap();
        let brand = base64::engine::general_purpose::STANDARD
            .decode(environment["packets"][3]["body"].as_str().unwrap())
            .unwrap();
        wire.send(1, &brand).unwrap();
        wire.send(3, &[]).unwrap();
        assert_eq!(wire.receive().unwrap(), (3, vec![]));
        let mut login = 99i32.to_be_bytes().to_vec();
        login.push(0);
        varint(1, &mut login);
        string("minecraft:overworld", &mut login);
        for value in [20, 12, 12] {
            varint(value, &mut login);
        }
        login.extend([0, 1, 0]);
        varint(0, &mut login);
        string("minecraft:overworld", &mut login);
        login.extend(0i64.to_be_bytes());
        login.extend([1, 255, 0, 0, 0]);
        varint(0, &mut login);
        varint(63, &mut login);
        login.extend([0, 0]);
        wire.send(0x31, &login).unwrap();
        let loading: serde_json::Value =
            serde_json::from_str(include_str!("../data/26.2/level-loading.json")).unwrap();
        let loading_body = base64::engine::general_purpose::STANDARD
            .decode(loading["body"].as_str().unwrap())
            .unwrap();
        if !delayed_loading {
            wire.send(0x26, &loading_body).unwrap();
        }

        for row in environment["times"].as_array().unwrap().iter().chain(
            environment["packets"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["label"] != "brand_config"),
        ) {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            wire.send(row["id"].as_i64().unwrap() as i32, &body)
                .unwrap();
        }

        if maps && !live {
            // 0x05 is block destruction, not a sequence ACK: VarInt entity,
            // packed BlockPos, unsigned stage. It must never overwrite ACK 37.
            let mut destruction = Vec::new();
            varint(42, &mut destruction);
            destruction.extend(0i64.to_be_bytes());
            destruction.push(4);
            wire.send(0x05, &destruction).unwrap();
            // Real source codec game-mode packet: survival; updates permissions,
            // HUD mode and the previous mode without a separate abilities packet.
            let row = &environment["gamemodes"][3];
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            wire.send(0x26, &bytes).unwrap();
        }
        let mut attrs = include_bytes!("../data/26.2/attribute-packet.bin").to_vec();
        attrs[0] = 99;
        if gravity == 0. {
            attrs.clear();
            varint(99, &mut attrs);
            varint(1, &mut attrs);
            varint(18, &mut attrs);
            attrs.extend(0f64.to_be_bytes());
            varint(0, &mut attrs);
        }
        wire.send(0x83, &attrs).unwrap();
        let mut health = 17.5f32.to_be_bytes().to_vec();
        varint(7, &mut health);
        health.extend(2.5f32.to_be_bytes());
        wire.send(0x68, &health).unwrap();
        wire.send(0x69, &[4]).unwrap();

        wire.send(0x15, &cookie_key).unwrap();
        assert_eq!(wire.receive().unwrap(), (0x15, response));

        wire.send(0x2c, &123i64.to_be_bytes()).unwrap();
        assert_eq!(
            wire.receive().unwrap(),
            (0x1c, 123i64.to_be_bytes().to_vec())
        );
        for (flags, expected) in [
            (0, if live { 12f64 } else { 10. }),
            (1, if live { 24f64 } else { 20. }),
        ] {
            let mut teleport = vec![7];
            for v in if live {
                [12f64, -44., -24., 0., 0., 0.]
            } else {
                [10f64, 64., -20., 0., 0., 0.]
            } {
                teleport.extend(v.to_be_bytes());
            }
            teleport.extend((if live { 45f32 } else { 30f32 }).to_be_bytes());
            teleport.extend((if live { 35f32 } else { 15f32 }).to_be_bytes());
            teleport.extend((flags as u32).to_be_bytes());
            wire.send(0x48, &teleport).unwrap();
            assert_eq!(wire.receive().unwrap(), (0, vec![7]));
            let (id, movement) = wire.receive().unwrap();
            assert_eq!(id, 0x1f);
            assert_eq!(
                f64::from_be_bytes(movement[..8].try_into().unwrap()),
                expected
            );
        }
        // Real framed/compressed 26.2 entity traffic, including the new
        // one-byte zero LP velocity before the angle fields in add_entity.
        for entity_id in [42u8, 43] {
            let mut spawn = vec![entity_id];
            spawn.extend([1; 16]);
            spawn.push(32);
            for v in [0.25f64, 64., -0.5] {
                spawn.extend(v.to_be_bytes());
            }
            spawn.extend([0, 0, 0, 0, 0]);
            wire.send(1, &spawn).unwrap();
        }
        let mut movement = vec![42];
        for delta in [4096i16, 0, -2048] {
            movement.extend(delta.to_be_bytes());
        }
        movement.extend([64, 0, 1]);
        wire.send(0x36, &movement).unwrap();
        wire.send(0x53, &[42, 32]).unwrap();
        wire.send(0x65, &[42, 0]).unwrap();
        wire.send(0x4d, &[1, 43]).unwrap();
        let mut chunk = vec![];
        chunk.extend(1i32.to_be_bytes());
        chunk.extend((-2i32).to_be_bytes());
        varint(0, &mut chunk); // no heightmaps
        let mut sections = Vec::new();
        for section in 0..24 {
            sections.extend((if section == 0 { 4096u16 } else { 0 }).to_be_bytes());
            sections.extend(0u16.to_be_bytes());
            sections.push(0);
            varint(if section == 0 { 1 } else { 0 }, &mut sections);
            sections.extend([0, 0]);
        }
        // Like LevelChunk.replaceWithPacketData, read the dimension's 24
        // sections, rather than interpreting storage padding as extra height.
        if maps {
            sections.extend([0; 16]);
        }
        varint(sections.len() as i32, &mut chunk);
        chunk.extend(sections);
        chunk.push(0); // no block entities
        let mut light_raw = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../data/26.2/light-updates.json.gz").as_slice(),
        )
        .read_to_end(&mut light_raw)
        .unwrap();
        let light_reference: serde_json::Value = serde_json::from_slice(&light_raw).unwrap();
        let light_initial = base64::engine::general_purpose::STANDARD
            .decode(light_reference["packets"][0]["body"].as_str().unwrap())
            .unwrap();
        let mut light_tail = light_initial.as_slice();
        read_varint(&mut light_tail).unwrap();
        read_varint(&mut light_tail).unwrap();
        chunk.extend(light_tail); // official source codec light payload in full chunk

        wire.send(0x2d, &chunk).unwrap();
        let mut sign_raw = Vec::new();
        flate2::read::GzDecoder::new(include_bytes!("../data/26.2/sign-editor.json.gz").as_slice())
            .read_to_end(&mut sign_raw)
            .unwrap();
        let sign_reference: serde_json::Value = serde_json::from_slice(&sign_raw).unwrap();
        let sign_row = sign_reference["packets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| {
                r["position"] == serde_json::json!([24, -45, -24])
                    && r["front"] == true
                    && r["lines"][0] == "Привет"
            })
            .unwrap();
        if sign_editing {
            let open = base64::engine::general_purpose::STANDARD
                .decode(sign_row["open"].as_str().unwrap())
                .unwrap();
            let mut block = open[..8].to_vec();
            varint(
                optimc::assets::builtins::get()
                    .unwrap()
                    .default_state("minecraft:oak_sign")
                    .unwrap() as i32,
                &mut block,
            );
            wire.send(0x08, &block).unwrap();
            wire.send(0x3c, &open).unwrap();
        }

        for row in light_reference["packets"].as_array().unwrap() {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            wire.send(0x30, &body).unwrap();
        }
        for row in light_reference["weather"]
            .as_array()
            .unwrap()
            .iter()
            .take(4)
        {
            let body = base64::engine::general_purpose::STANDARD
                .decode(row["body"].as_str().unwrap())
                .unwrap();
            wire.send(0x26, &body).unwrap();
        }

        let mut extra = chunk.clone();
        extra[..4].copy_from_slice(&2i32.to_be_bytes());
        extra[4..8].copy_from_slice(&0i32.to_be_bytes());
        wire.send(0x2d, &extra).unwrap();
        wire.send(0x25, &2i64.to_be_bytes()).unwrap();
        let (cx, sy, cz) = (1i64, -3i64, -2i64);
        let section = (cx & 0x3fffff) << 42 | (cz & 0x3fffff) << 20 | (sy & 0xfffff);
        let mut batch = section.to_be_bytes().to_vec();
        varint(1, &mut batch);
        net::codec::varlong((2 << 12) | (1 << 8) | (2 << 4) | 3, &mut batch);
        wire.send(0x54, &batch).unwrap();
        let (x, y, z) = (18i64, -46i64, -29i64);
        let packed = (x & 0x3ffffff) << 38 | (z & 0x3ffffff) << 12 | (y & 4095);
        let mut change = packed.to_be_bytes().to_vec();
        varint(4, &mut change);
        wire.send(0x08, &change).unwrap();
        // Server-owned HUD fixture: real network-NBT components and packet field order.
        let mut xp = 0.5f32.to_be_bytes().to_vec();
        varint(12, &mut xp);
        varint(123, &mut xp);
        wire.send(0x67, &xp).unwrap();
        let component = |text: &str| {
            let mut b = Vec::new();
            net::nbt::write_network(&net::nbt::Tag::String(text.into()), &mut b).unwrap();
            b
        };
        wire.send(0x70, &component("Подзаголовок")).unwrap();
        wire.send(0x72, &component("§aЗаголовок")).unwrap();
        wire.send(0x57, &component("Action bar")).unwrap();
        let mut boss = vec![7; 16];
        boss.push(0);
        boss.extend(component("§cСерверный босс"));
        boss.extend(0.75f32.to_be_bytes());
        boss.extend([2, 1, 7]);
        wire.send(0x09, &boss).unwrap();
        // Bodies encoded by the original 26.2 STREAM_CODECs, through real TCP.
        let mut scoreboard_json = Vec::new();
        flate2::read::GzDecoder::new(
            &include_bytes!("../data/26.2/scoreboard-reference.json.gz")[..],
        )
        .read_to_end(&mut scoreboard_json)
        .unwrap();
        let reference: serde_json::Value = serde_json::from_slice(&scoreboard_json).unwrap();
        use base64::Engine;
        for packet in reference["packets"].as_array().unwrap().iter().take(9) {
            let body = base64::engine::general_purpose::STANDARD
                .decode(packet["body"].as_str().unwrap())
                .unwrap();
            wire.send(packet["id"].as_i64().unwrap() as i32, &body)
                .unwrap();
        }
        if maps {
            // Correct protocol IDs: block entity data is 0x06; 0x07 is a block event.
            let signs = optimc::assets::signs::reference().unwrap();
            wire.send(
                0x06,
                &base64::engine::general_purpose::STANDARD
                    .decode(signs["update"].as_str().unwrap())
                    .unwrap(),
            )
            .unwrap();
            let chest = optimc::assets::builtins::get()
                .unwrap()
                .default_state("minecraft:chest")
                .unwrap();
            let position = (18i64 << 38) | ((-29i64 & 0x3ffffff) << 12) | (-46i64 & 4095);
            let mut update = position.to_be_bytes().to_vec();
            varint(chest as i32, &mut update);
            wire.send(0x08, &update).unwrap();
            let kind = (0..128)
                .find(|&id| {
                    optimc::assets::builtins::get()
                        .unwrap()
                        .name("minecraft:block_entity_type", id)
                        == Some("minecraft:chest")
                })
                .unwrap();
            let mut entity = position.to_be_bytes().to_vec();
            varint(kind as i32, &mut entity);
            entity.extend([10, 0]);
            wire.send(0x06, &entity).unwrap();
            let mut reference = Vec::new();
            flate2::read::GzDecoder::new(
                include_bytes!("../data/26.2/chest-events.json.gz").as_slice(),
            )
            .read_to_end(&mut reference)
            .unwrap();
            let reference: serde_json::Value = serde_json::from_slice(&reference).unwrap();
            let mut event = base64::engine::general_purpose::STANDARD
                .decode(reference["packets"][1]["body"].as_str().unwrap())
                .unwrap();
            event[..8].copy_from_slice(&position.to_be_bytes());
            wire.send(0x07, &event).unwrap();
            let mut json = Vec::new();
            flate2::read::GzDecoder::new(
                &include_bytes!("../data/26.2/maps-reference.json.gz")[..],
            )
            .read_to_end(&mut json)
            .unwrap();
            let data: serde_json::Value = serde_json::from_slice(&json).unwrap();
            for packet in data["packets"].as_array().unwrap().iter().take(3) {
                let id = packet["id"].as_i64().unwrap() as i32;
                let mut body = base64::engine::general_purpose::STANDARD
                    .decode(packet["body"].as_str().unwrap())
                    .unwrap();
                if id == 1 || id == 0x63 {
                    let mut rest = body.as_slice();
                    assert_eq!(read_varint(&mut rest).unwrap(), 99);
                    let mut renamed = Vec::new();
                    varint(199, &mut renamed);
                    renamed.extend(rest);
                    body = renamed;
                }
                wire.send(id, &body).unwrap();
            }
        }
        if maps {
            for map_id in 1000..2024 {
                let mut body = Vec::new();
                varint(map_id, &mut body);
                body.extend([0, 0, 0, 1, 1, 0, 0, 1, 4]);
                wire.send(0x33, &body).unwrap();
            }
            wire.send(0x02, &[42, 3]).unwrap();
        }
        if inventory {
            let reference = inventory_reference();
            for row in reference["packets"].as_array().unwrap().iter().take(9) {
                let (id, body) = inventory_packet(row);
                wire.send(id, &body).unwrap();
            }
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(
                include_bytes!("../data/26.2/item-contents.json.gz").as_slice(),
            )
            .read_to_end(&mut raw)
            .unwrap();
            let contents: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            use base64::Engine;
            for row in contents["stacks"].as_array().unwrap() {
                let body = base64::engine::general_purpose::STANDARD
                    .decode(row["body"].as_str().unwrap())
                    .unwrap();
                wire.send(0x60, &body).unwrap();
            }
            wire.send(0x60, &[0]).unwrap();
            for row in reference["unsupported"].as_array().unwrap() {
                let (id, body) = inventory_packet(row);
                wire.send(id, &body).unwrap();
            }
        }
        wire.send(0x0c, &[]).unwrap();
        wire.send(0x0b, &[1]).unwrap();
        let (id, ack) = wire.receive().unwrap();
        assert_eq!(id, 0x0b);
        let desired = f32::from_be_bytes(ack.try_into().unwrap());
        assert!(desired.is_finite() && desired > 0. && desired <= 5.25);
        if live {
            thread::sleep(Duration::from_millis(300));
            if delayed_loading {
                let ignored = base64::engine::general_purpose::STANDARD
                    .decode(loading["ignoredParameterBody"].as_str().unwrap())
                    .unwrap();
                wire.send(0x26, &ignored).unwrap();
            }
            let mut update = packed.to_be_bytes().to_vec();
            varint(
                optimc::assets::builtins::get()
                    .unwrap()
                    .default_state("minecraft:oak_stairs")
                    .unwrap() as i32,
                &mut update,
            );
            wire.send(0x08, &update).unwrap();
            wire.send(0x2c, &999i64.to_be_bytes()).unwrap();
            wire.stream
                .set_read_timeout(Some(Duration::from_secs(15)))
                .unwrap();
            let mut input_step = 0;
            let mut inventory_step = 0;
            let mut menu_click_step = 0;
            let mut entity_step = 0;
            let mut action_count = 0;
            let mut action_slot = false;
            let mut action_input = false;
            let mut movements = 0;
            let mut sign_updates = 0;
            let mut loaded = 0;
            let mut pong = false;
            let mut bundled_ticks = 0;
            let mut bundled_pong = false;
            loop {
                let (id, body) = match wire.receive() {
                    Ok(packet) => packet,
                    Err(error) => {
                        assert_eq!(
                            error.downcast_ref::<std::io::Error>().unwrap().kind(),
                            std::io::ErrorKind::UnexpectedEof,
                            "{error:#}"
                        );
                        break;
                    }
                };
                match id {
                    0x3d if sign_editing => {
                        sign_updates += 1;
                        let expected = base64::engine::general_purpose::STANDARD
                            .decode(sign_row["update"].as_str().unwrap())
                            .unwrap();
                        assert_eq!(body, expected);
                        assert_eq!(
                            sign_updates, 1,
                            "Repeated or stale sign update reached socket"
                        );
                    }
                    0x1c => {
                        if body == 1234i64.to_be_bytes() {
                            assert_eq!(
                                bundled_ticks, 3,
                                "Bundled handler ran before closing delimiter"
                            );
                            bundled_pong = true;
                        } else {
                            assert_eq!(body, 999i64.to_be_bytes());
                            pong = true;
                        }
                    }
                    0x1e | 0x1f => {
                        let x = f64::from_be_bytes(body[..8].try_into().unwrap());
                        let y = f64::from_be_bytes(body[8..16].try_into().unwrap());
                        let z = f64::from_be_bytes(body[16..24].try_into().unwrap());
                        assert!(
                            (x - 24.).abs() < 1e-5 && (z + 24.).abs() < 1e-5,
                            "Stale/invalid motion: {x} {y} {z}"
                        );
                        assert!(
                            (-48.00001..=-43.99999).contains(&y),
                            "Player must land on the server stone: {y}"
                        );
                        if gravity == 0. {
                            assert_eq!(
                                y, -44.,
                                "Server requested zero gravity: the client must not fall"
                            );
                        }
                        movements += 1;
                    }
                    0x2c => {
                        assert!(body.is_empty());
                        loaded += 1;
                        assert_eq!(loaded, 1, "player_loaded must be sent once per world");
                        if bundle {
                            wire.send(0, &[]).unwrap();
                            let mut xp = 0.25f32.to_be_bytes().to_vec();
                            varint(21, &mut xp);
                            varint(321, &mut xp);
                            wire.send(0x67, &xp).unwrap();
                            wire.send(0x2c, &1234i64.to_be_bytes()).unwrap();
                        }
                    }
                    0x0d => {
                        assert!(body.is_empty(), "Client tick end has no payload");
                        if bundle && loaded == 1 && bundled_ticks < 3 {
                            bundled_ticks += 1;
                            if bundled_ticks == 3 {
                                wire.send(0, &[]).unwrap();
                            }
                        }
                    }
                    0x35 => {
                        assert_eq!(body, [0, if actions || entity_actions { 8 } else { 4 }]);
                        action_slot = true;
                    }
                    0x2b if actions || entity_actions => {
                        assert_eq!(body, [32]);
                        action_input = true;
                    }
                    0x1a if entity_actions => {
                        assert_eq!(entity_step, 0);
                        assert!(action_slot && action_input);
                        assert_eq!(
                            body,
                            net::entity::interact(
                                199,
                                net::entity::Hand::Main,
                                glam::DVec3::new(1.125, -0.125, 0.25),
                                true
                            )
                            .unwrap()
                            .1
                        );
                        entity_step += 1;
                    }
                    0x01 if entity_actions => {
                        assert_eq!(entity_step, 1);
                        assert_eq!(body, net::entity::attack(199).1);
                        entity_step += 1;
                    }
                    0x3f if entity_actions => {
                        assert_eq!(entity_step, 2);
                        assert_eq!(body, [0]);
                        entity_step += 1;
                        let mut empty_tags = vec![1];
                        string("minecraft:entity_type", &mut empty_tags);
                        empty_tags.push(0);
                        wire.send(0x86, &empty_tags).unwrap();
                        let mut health = 19.25f32.to_be_bytes().to_vec();
                        varint(7, &mut health);
                        health.extend(2.5f32.to_be_bytes());
                        wire.send(0x68, &health).unwrap();
                    }
                    0x42 if actions => {
                        assert!(
                            action_slot && action_input,
                            "Use arrived before slot/sneak synchronization"
                        );
                        action_count += 1;
                        let expected = net::request::Request::UseBlock {
                            hit: optimc::world::picking::BlockHit {
                                block: [-31, -22, 47],
                                position: glam::DVec3::new(-30.875, -21.25, 47.625),
                                face: 2,
                                inside: false,
                                border: false,
                            },
                            hand: net::entity::Hand::Main,
                            slot: 8,
                            input: 32,
                            teleport_version: 0,
                        };
                        let (_, expected) = expected
                            .encode_sequenced(
                                0,
                                0,
                                &mut net::last_seen::Tracker::default(),
                                action_count,
                            )
                            .unwrap();
                        assert_eq!(body, expected);
                        let mut ack = Vec::new();
                        varint(action_count, &mut ack);
                        wire.send(0x04, &ack).unwrap();
                    }
                    0x29 | 0x43 if input_actions => {
                        assert!(
                            action_slot,
                            "World click preceded selected-slot synchronization"
                        );
                        let requests = input_requests(0);
                        let sequence = [1, 0, 2, 3, 4][input_step];
                        let expected = requests[input_step]
                            .encode_sequenced(0, 0, &mut Default::default(), sequence)
                            .unwrap();
                        assert_eq!((id, body), expected, "World action {input_step}");
                        if input_step == 4 {
                            let mut ack = Vec::new();
                            varint(4, &mut ack);
                            wire.send(0x04, &ack).unwrap();
                        }
                        input_step += 1;
                    }
                    0x12 if inventory => {
                        let reference = container_click_reference();
                        let expected = inventory_packet(&reference["packets"][menu_click_step]);
                        assert_eq!((id, body), expected, "Menu click {menu_click_step}");
                        menu_click_step += 1;
                    }
                    0x29 | 0x13 if inventory => {
                        assert!(action_slot, "Inventory action preceded selected-slot sync");
                        let reference = inventory_reference();
                        let expected = inventory_packet(&reference["actions"][inventory_step]);
                        assert_eq!((id, body), expected, "Inventory action {inventory_step}");
                        inventory_step += 1;
                    }
                    0x20 => assert_eq!(body.len(), 9),
                    0x21 => assert_eq!(body.len(), 1),
                    _ => panic!("Unexpected live outgoing packet {id:x}: {body:?}"),
                }
            }
            if bundle {
                assert!(bundled_pong, "Open bundle stalled outgoing gameplay ticks");
            }
            if input_actions {
                assert_eq!(input_step, 5, "Lost or stale world mouse action");
            }
            if inventory {
                assert_eq!(
                    menu_click_step,
                    container_click_reference()["packets"]
                        .as_array()
                        .unwrap()
                        .len()
                );
                assert_eq!(
                    inventory_step, 4,
                    "Stale actions leaked or valid actions were dropped"
                );
            }
            if entity_actions {
                assert_eq!(entity_step, 3, "Missing or stale entity action/swing");
            }
            if actions {
                assert_eq!(action_count, 2, "Stale or duplicate block actions leaked");
            }
            assert!(pong, "Keepalive must survive live player ticks");
            if sign_editing {
                assert_eq!(sign_updates, 1);
            }
            if !delayed_loading && !sign_editing {
                assert!(movements > 0, "Expected player position packets");
            }
            assert_eq!(
                loaded, 1,
                "Expected explicit world readiness acknowledgement"
            );
        } else {
            let mut buf = [0];
            let _ = wire.stream.read(&mut buf);
        }
    });
    (
        Address {
            host: "127.0.0.1".into(),
            port,
        },
        server,
    )
}
#[test]
fn original_map_and_frame_metadata_reach_the_live_session_report() {
    let (address, server) = world_fixture_maps(false, 0., false, false, true);
    let report = net::session_state(&address, "Fixture", 1).unwrap();
    assert!(report.world.chunks.values().all(|c| c.sections.len() == 24));
    assert_eq!(
        report.entities.entities[&199].type_name().unwrap(),
        Some("minecraft:item_frame")
    );
    let (invisible, rotation, item) = report.metadata.frame(199);
    assert!(!invisible);
    assert_eq!(rotation, 1);
    assert_eq!(item.unwrap().map, Some(42));
    assert_eq!(report.maps.maps.len(), 1025);
    assert_eq!(report.server_brand.as_deref(), Some("Paper fixture 26.2"));
    assert_eq!(report.world.spawn.as_ref().unwrap().game_mode, 0);
    assert_eq!(report.world.spawn.as_ref().unwrap().previous_mode, Some(1));
    assert!(!report
        .unhandled
        .keys()
        .any(|key| key.ends_with("/game_event")));
    assert_eq!(report.environment.chunk_center, Some([-300, 512]));
    assert_eq!(report.environment.chunk_radius, Some(12));
    assert_eq!(report.environment.game_time, 91);
    assert_eq!(report.lights.sample([16, -64, -32]), [Some(15), Some(15)]);
    assert_eq!(report.lights.sample([-16, -64, -32]), [Some(0), Some(15)]);
    assert_eq!(report.lights.sample([-16, -48, -32]), [Some(0), Some(9)]);
    assert_eq!(report.environment.rain_level, 0.);
    assert!(!report
        .unhandled
        .keys()
        .any(|k| k.ends_with("/light_update")));

    assert_eq!(report.last_block_ack, Some(37));
    assert!(report
        .unhandled
        .keys()
        .any(|key| key.ends_with("/block_destruction")));
    assert_eq!(report.environment.difficulty, Some((0, true)));
    assert_eq!(report.environment.simulation_distance, Some(12));
    assert_eq!(
        report
            .environment
            .clock(&report.registries, "minecraft:overworld")
            .unwrap()
            .total_ticks,
        9007199254745999
    );
    assert_eq!(
        report.environment.default_spawn.as_ref().unwrap().position,
        [-31, -22, 47]
    );
    assert!(!report.unhandled.keys().any(|key| [
        "set_time",
        "set_default_spawn_position",
        "set_chunk_cache_center",
        "set_chunk_cache_radius",
        "change_difficulty",
        "block_changed_ack",
        "set_simulation_distance",
        "custom_payload"
    ]
    .iter()
    .any(|name| key.ends_with(name))));

    assert!(report.block_events.lids[&[18, -46, -29]].open);
    assert!(report.animations.swings[&42].off_hand);
    assert!(!report.unhandled.keys().any(|key| key.ends_with("/animate")));
    let map = &report.maps.maps[&42];
    assert_eq!(map.scale, 2);
    assert_eq!(map.colors.len(), 16384);
    assert_eq!(
        map.decorations[0].name.as_ref().unwrap().plain_text(),
        "Marker"
    );
    assert!(!report
        .unhandled
        .keys()
        .any(|key| key.ends_with("map_item_data")
            || key.ends_with("set_entity_data")
            || key.ends_with("block_entity_data")
            || key.ends_with("block_event")));
    server.join().unwrap();
}
#[test]
fn login_can_disable_compression_before_the_uncompressed_success_packet() {
    let (address, server) = world_fixture_transport(false, 0., false, true);
    let report = net::session_state(&address, "Fixture", 1).unwrap();
    assert_eq!(report.chunks, 2);
    assert_eq!(
        report
            .scoreboard
            .sidebar("Fixture")
            .unwrap()
            .title
            .plain_text(),
        "Points"
    );
    server.join().unwrap();
}
#[test]
fn offline_mode_negotiates_and_decodes_a_26_2_chunk() {
    let (address, server) = world_fixture(false);
    let report = net::session_state(&address, "Fixture", 1).unwrap();
    assert_eq!(report.chunks, 2);
    assert_eq!(report.attributes.movement(99).gravity, 0.30000000000000004);
    assert_eq!(report.selected_slot, Some((4, 1)));
    let sidebar = report.scoreboard.sidebar("Fixture").unwrap();
    assert_eq!(sidebar.title.plain_text(), "Points");
    assert_eq!(sidebar.rows.len(), 4);
    assert_eq!(sidebar.rows[0].0.plain_text(), "[R] Alpha!");
    assert_eq!(sidebar.rows[1].1.runs[0].style.color, Some(0xffaa00));
    assert_eq!(report.hud.experience.level, 12);
    assert_eq!(report.hud.experience.total, 123);
    assert_eq!(report.hud.titles.title.plain_text(), "Заголовок");
    assert_eq!(report.hud.titles.subtitle.plain_text(), "Подзаголовок");
    assert_eq!(
        report.hud.action.as_ref().unwrap().0.plain_text(),
        "Action bar"
    );
    assert_eq!(report.hud.bosses.len(), 1);
    assert_eq!(report.hud.bosses[0].name.plain_text(), "Серверный босс");
    assert_eq!(report.hud.bosses[0].target, 0.75);

    assert_eq!(
        report.vitals,
        net::vitals::Vitals {
            health: 17.5,
            food: 7,
            saturation: 2.5
        }
    );
    assert_eq!(report.entity_packets, 6);
    assert_eq!(report.entities.entities.len(), 1);
    let entity = &report.entities.entities[&42];
    assert_eq!(entity.pose.position.to_array(), [1.25, 64., -1.]);
    assert_eq!(entity.pose.yaw, 90.);
    assert_eq!(entity.head_yaw, 45.);
    assert!(entity.on_ground);
    assert_eq!(entity.type_name().unwrap(), Some("minecraft:creeper"));
    assert_eq!(report.registries.dimension(0).unwrap().min_y, -64);
    assert_eq!(report.world.player_id, Some(99));
    assert_eq!(report.world.block(16, -64, -32), Some(1));
    assert_eq!(report.world.block(16, -48, -32), Some(0));
    assert_eq!(report.world.chunks.len(), 1);
    assert_eq!(report.world.block(17, -45, -30), Some(2));
    assert_eq!(report.world.block(18, -46, -29), Some(4));
    assert_eq!(
        report.world.chunks[&(1, -2)].sections[0]
            .blocks
            .allocated_values(),
        1
    );
    server.join().unwrap();
}
#[test]
fn status_roundtrip_checks_protocol_and_ping() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut wire = Wire {
            stream,
            compression: None,
            journal: None,
        };
        wire.receive().unwrap();
        assert_eq!(wire.receive().unwrap(), (0, vec![]));
        let mut data = vec![];
        string(
            r#"{"version":{"name":"26.2","protocol":776},"players":{"max":20,"online":0},"description":{"text":"Fixture"}}"#,
            &mut data,
        );
        wire.send(0, &data).unwrap();
        let (id, ping) = wire.receive().unwrap();
        assert_eq!(id, 1);
        wire.send(1, &ping).unwrap();
    });
    let response = net::status(&Address {
        host: "127.0.0.1".into(),
        port,
    })
    .unwrap();
    assert_eq!(response["version"]["protocol"], 776);
    assert!(response["optimc_latency_ms"].as_f64().unwrap() >= 0.);
    server.join().unwrap();
}

/// A timeout can fall inside the VarInt prefix or body. Cancellation must close
/// that connection without waiting for the rest of the packet or a UI join.
#[test]
fn live_worker_keeps_fragmented_frames_and_cancels_a_partial_packet() {
    use net::live::{Connection, Status};
    use std::{
        io::{Cursor, Write},
        sync::mpsc,
        time::Instant,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (partial_tx, partial_rx) = mpsc::channel();
    let (closed_tx, closed_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut wire = Wire {
            stream,
            compression: None,
            journal: None,
        };
        assert_eq!(wire.receive().unwrap().0, 0);
        assert_eq!(wire.receive().unwrap().0, 0);
        let mut encoded = Wire {
            stream: Cursor::new(Vec::new()),
            compression: None,
            journal: None,
        };
        encoded.send(3, &[0]).unwrap();
        encoded.compression = Some(0);
        let mut success = net::offline_uuid("Fixture").to_vec();
        string("Fixture", &mut success);
        varint(0, &mut success);
        encoded.send(2, &success).unwrap();
        let bytes = encoded.stream.into_inner();
        let mut start = 0;
        for end in [1, 2, 4, 7, bytes.len() - 2, bytes.len()] {
            wire.stream.write_all(&bytes[start..end]).unwrap();
            start = end;
            thread::sleep(Duration::from_millis(40));
        }
        wire.compression = Some(0);
        assert_eq!(wire.receive().unwrap(), (3, vec![]));
        let mut brand = Vec::new();
        string("minecraft:brand", &mut brand);
        string("OptiMC", &mut brand);
        assert_eq!(wire.receive().unwrap(), (2, brand));
        assert_eq!(wire.receive().unwrap().0, 0);
        wire.send(4, &555i64.to_be_bytes()).unwrap();
        assert_eq!(wire.receive().unwrap(), (4, 555i64.to_be_bytes().to_vec()));
        wire.send(3, &[]).unwrap();
        assert_eq!(wire.receive().unwrap(), (3, vec![]));
        wire.send(0x2c, &777i64.to_be_bytes()).unwrap();
        assert_eq!(
            wire.receive().unwrap(),
            (0x1c, 777i64.to_be_bytes().to_vec())
        );
        let mut teleport = vec![8];
        for value in [12f64, 65., -30., 0.25, -0.5, 0.75] {
            teleport.extend(value.to_be_bytes());
        }
        teleport.extend(45f32.to_be_bytes());
        teleport.extend(10f32.to_be_bytes());
        teleport.extend(0u32.to_be_bytes());
        wire.send(0x48, &teleport).unwrap();
        assert_eq!(wire.receive().unwrap(), (0, vec![8]));
        assert_eq!(wire.receive().unwrap().0, 0x1f);
        // Start a two-byte frame prefix and never finish it.
        wire.stream.write_all(&[0x80]).unwrap();
        partial_tx.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(wire.stream.read(&mut byte).unwrap(), 0);
        closed_tx.send(()).unwrap();
    });
    let connection = Connection::connect(
        Address {
            host: "127.0.0.1".into(),
            port,
        },
        "Fixture".into(),
    )
    .unwrap();
    partial_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        assert!(Instant::now() < deadline, "No published play snapshot");
        assert!(
            !matches!(connection.status(), Status::Closed(_)),
            "{:?}",
            connection.status()
        );
        if let Some(snapshot) = connection.take_snapshot() {
            if snapshot.report.packets == 6 {
                break snapshot;
            }
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(connection.status(), Status::Playing);
    assert_eq!(snapshot.report.packets, 6);
    assert_eq!(snapshot.position, [12., 65., -30.]);
    assert_eq!(snapshot.velocity, [0.25, -0.5, 0.75]);
    assert_eq!(snapshot.rotation, [45., 10.]);
    assert_eq!(snapshot.teleport_version, 1);
    assert!(connection.take_snapshot().is_none());
    let start = Instant::now();
    connection.cancel();
    assert!(start.elapsed() < Duration::from_millis(100));
    closed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    while connection.status() != Status::Cancelled {
        assert!(start.elapsed() < Duration::from_secs(2));
        thread::sleep(Duration::from_millis(5));
    }
    server.join().unwrap();
}

#[test]
#[ignore = "Requires Vulkan window, TCP and imported official 26.2 resources"]
fn received_server_world_reaches_vulkan_without_demo_terrain() {
    let (address, server) = world_fixture(false);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let screenshot = root.join(".cache/server-world.png");
    let _ = std::fs::remove_file(&screenshot);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_optimc"))
        .current_dir(root)
        .args([
            "--quality",
            "low",
            "connect",
            &format!("{}:{}", address.host, address.port),
            "--username",
            "Fixture",
            "--seconds",
            "1",
            "--screenshot",
        ])
        .arg(&screenshot)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Server snapshot:"));
    let image = image::open(&screenshot).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (1440, 900));
    server.join().unwrap();
}

#[test]
#[ignore = "Requires Vulkan window, TCP and imported official 26.2 resources"]
fn live_server_world_updates_while_window_is_rendering() {
    let (address, server) = world_fixture(true);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let screenshot = root.join(".cache/server-live.png");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_optimc"))
        .current_dir(root)
        .args([
            "--quality",
            "low",
            "play",
            &format!("{}:{}", address.host, address.port),
            "--username",
            "Fixture",
            "--frames",
            "120",
            "--screenshot",
        ])
        .arg(&screenshot)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.matches("Live scene:").count() >= 2,
        "Expected a later server update: {stdout}"
    );
    assert!(
        stdout.contains("4 textures"),
        "Late oak-stairs material must reach the render path: {stdout}"
    );
    assert!(
        !stdout.contains("Terrain:"),
        "Demo meshing must not run for a live server"
    );
    assert_eq!(
        image::open(&screenshot).unwrap().to_rgba8().dimensions(),
        (1440, 900)
    );
    server.join().unwrap();
}

#[test]
fn queued_motion_is_fenced_by_world_teleport_and_ability_versions() {
    use net::{
        entity::Pose,
        live::{Connection, Status},
        movement::Motion,
    };
    use std::time::Instant;
    let (address, server) = world_fixture(true);
    let connection = Connection::connect(address, "Fixture".into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        assert!(Instant::now() < deadline);
        assert!(
            !matches!(connection.status(), Status::Closed(_)),
            "{:?}",
            connection.status()
        );
        if let Some(snapshot) = connection.take_snapshot() {
            if snapshot.position_ready && snapshot.report.world.chunks.len() == 1 {
                break snapshot;
            }
        }
        thread::sleep(Duration::from_millis(5));
    };
    assert!(!snapshot.client_loaded);
    connection.mark_loaded(
        snapshot.world_epoch,
        snapshot.teleport_version.wrapping_sub(1),
    );
    thread::sleep(Duration::from_millis(50));
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    let ready_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if connection.take_snapshot().is_some_and(|s| s.client_loaded) {
            break;
        }
        assert!(
            Instant::now() < ready_deadline,
            "No loaded acknowledgement snapshot"
        );
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(50));
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    let motion = Motion {
        world_epoch: snapshot.world_epoch,
        teleport_version: snapshot.teleport_version,
        abilities_version: snapshot.abilities_version,
        velocity_version: snapshot.velocity_version,
        player_state_version: snapshot.player_state_version,
        pose: Pose {
            position: glam::DVec3::new(24., -45., -24.),
            ..Pose::default()
        },
        on_ground: false,
        horizontal_collision: false,
        input: 0,
        sprinting: false,
        slot: snapshot.report.selected_slot.map_or(0, |(slot, _)| slot),
        flying: false,
    };
    for field in 0..5 {
        let mut stale = motion;
        stale.pose.position = glam::DVec3::splat(999.);
        match field {
            0 => stale.world_epoch = stale.world_epoch.wrapping_sub(1),
            1 => stale.teleport_version = stale.teleport_version.wrapping_sub(1),
            2 => stale.abilities_version = stale.abilities_version.wrapping_sub(1),
            3 => stale.velocity_version = stale.velocity_version.wrapping_sub(1),
            _ => stale.player_state_version = stale.player_state_version.wrapping_sub(1),
        }
        connection.send_motion(stale).unwrap();
    }
    connection.send_motion(motion).unwrap();
    thread::sleep(Duration::from_millis(500));
    assert_eq!(connection.status(), Status::Playing);
    connection.cancel();
    server.join().unwrap();
}

#[test]
fn block_use_orders_current_slot_input_and_sequence_and_drops_stale_targets() {
    use net::{
        entity::Pose,
        live::{Connection, Status},
        movement::Motion,
    };
    use std::time::Instant;
    let (address, server) = world_fixture_actions(true, 0.3, false, false, false, true);
    let connection = Connection::connect(address, "Fixture".into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        assert!(Instant::now() < deadline);
        assert!(
            !matches!(connection.status(), Status::Closed(_)),
            "{:?}",
            connection.status()
        );
        if let Some(snapshot) = connection.take_snapshot() {
            if snapshot.position_ready && snapshot.report.world.chunks.len() == 1 {
                break snapshot;
            }
        }
        thread::sleep(Duration::from_millis(5));
    };
    assert!(!snapshot.client_loaded);
    connection.mark_loaded(
        snapshot.world_epoch,
        snapshot.teleport_version.wrapping_sub(1),
    );
    thread::sleep(Duration::from_millis(50));
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    let ready_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if connection.take_snapshot().is_some_and(|s| s.client_loaded) {
            break;
        }
        assert!(
            Instant::now() < ready_deadline,
            "No loaded acknowledgement snapshot"
        );
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(50));
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    let motion = Motion {
        world_epoch: snapshot.world_epoch,
        teleport_version: snapshot.teleport_version,
        abilities_version: snapshot.abilities_version,
        velocity_version: snapshot.velocity_version,
        player_state_version: snapshot.player_state_version,
        pose: Pose {
            position: glam::DVec3::new(24., -45., -24.),
            ..Pose::default()
        },
        on_ground: false,
        horizontal_collision: false,
        input: 32,
        sprinting: false,
        slot: 8,
        flying: false,
    };
    for field in 0..5 {
        let mut stale = motion;
        stale.pose.position = glam::DVec3::splat(999.);
        match field {
            0 => stale.world_epoch = stale.world_epoch.wrapping_sub(1),
            1 => stale.teleport_version = stale.teleport_version.wrapping_sub(1),
            2 => stale.abilities_version = stale.abilities_version.wrapping_sub(1),
            3 => stale.velocity_version = stale.velocity_version.wrapping_sub(1),
            _ => stale.player_state_version = stale.player_state_version.wrapping_sub(1),
        }
        connection.send_motion(stale).unwrap();
    }
    connection.send_motion(motion).unwrap();
    let hit = optimc::world::picking::BlockHit {
        block: [-31, -22, 47],
        position: glam::DVec3::new(-30.875, -21.25, 47.625),
        face: 2,
        inside: false,
        border: false,
    };
    let request = |teleport_version| net::request::Request::UseBlock {
        hit,
        hand: net::entity::Hand::Main,
        slot: 8,
        input: 32,
        teleport_version,
    };
    connection
        .request(
            snapshot.world_epoch.wrapping_sub(1),
            request(snapshot.teleport_version),
        )
        .unwrap();
    connection
        .request(
            snapshot.world_epoch,
            request(snapshot.teleport_version.wrapping_sub(1)),
        )
        .unwrap();
    for _ in 0..2 {
        connection
            .request(snapshot.world_epoch, request(snapshot.teleport_version))
            .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if connection
            .take_snapshot()
            .is_some_and(|s| s.report.last_block_ack == Some(2))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "No block acknowledgement: {:?}",
            connection.status()
        );
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(50));
    assert_eq!(connection.status(), Status::Playing);
    connection.cancel();
    server.join().unwrap();
}

#[test]
fn entity_use_attack_and_swing_are_ordered_and_fenced_by_uuid_and_teleport() {
    use net::{
        entity::Pose,
        live::{Connection, Status},
        movement::Motion,
    };
    use std::time::Instant;
    let (address, server) = world_fixture_picking(true, 0.3, false, false, true, false, true);
    let connection = Connection::connect(address, "Fixture".into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        assert!(Instant::now() < deadline);
        assert!(
            !matches!(connection.status(), Status::Closed(_)),
            "{:?}",
            connection.status()
        );
        if let Some(snapshot) = connection.take_snapshot() {
            if snapshot.position_ready && snapshot.report.world.chunks.len() == 1 {
                break snapshot;
            }
        }
        thread::sleep(Duration::from_millis(5));
    };
    assert!(!snapshot.client_loaded);
    connection.mark_loaded(
        snapshot.world_epoch,
        snapshot.teleport_version.wrapping_sub(1),
    );
    thread::sleep(Duration::from_millis(50));
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    let ready_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if connection.take_snapshot().is_some_and(|s| s.client_loaded) {
            break;
        }
        assert!(
            Instant::now() < ready_deadline,
            "No loaded acknowledgement snapshot"
        );
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(50));
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    let motion = Motion {
        world_epoch: snapshot.world_epoch,
        teleport_version: snapshot.teleport_version,
        abilities_version: snapshot.abilities_version,
        velocity_version: snapshot.velocity_version,
        player_state_version: snapshot.player_state_version,
        pose: Pose {
            position: glam::DVec3::new(24., -45., -24.),
            ..Pose::default()
        },
        on_ground: false,
        horizontal_collision: false,
        input: 32,
        sprinting: false,
        slot: 8,
        flying: false,
    };
    for field in 0..5 {
        let mut stale = motion;
        stale.pose.position = glam::DVec3::splat(999.);
        match field {
            0 => stale.world_epoch = stale.world_epoch.wrapping_sub(1),
            1 => stale.teleport_version = stale.teleport_version.wrapping_sub(1),
            2 => stale.abilities_version = stale.abilities_version.wrapping_sub(1),
            3 => stale.velocity_version = stale.velocity_version.wrapping_sub(1),
            _ => stale.player_state_version = stale.player_state_version.wrapping_sub(1),
        }
        connection.send_motion(stale).unwrap();
    }
    connection.send_motion(motion).unwrap();
    let entity = &snapshot.report.entities.entities[&199];
    assert!(snapshot.report.tags.contains(
        "minecraft:entity_type",
        "minecraft:redirectable_projectile",
        127
    ));
    let hit = optimc::world::entity_pick::EntityHit {
        id: 199,
        uuid: entity.uuid,
        position: entity.pose.position + glam::DVec3::new(1.125, -0.125, 0.25),
        relative: glam::DVec3::new(1.125, -0.125, 0.25),
    };
    let request = |hit, attack, teleport_version| net::request::Request::EntityAction {
        hit,
        attack,
        hand: net::entity::Hand::Main,
        slot: 8,
        input: 32,
        teleport_version,
    };
    let mut reused = hit;
    reused.uuid = [255; 16];
    connection
        .request(
            snapshot.world_epoch,
            request(reused, false, snapshot.teleport_version),
        )
        .unwrap();
    let mut removed = hit;
    removed.id = 12345;
    connection
        .request(
            snapshot.world_epoch,
            request(removed, false, snapshot.teleport_version),
        )
        .unwrap();
    connection
        .request(
            snapshot.world_epoch,
            request(hit, false, snapshot.teleport_version.wrapping_sub(1)),
        )
        .unwrap();
    connection
        .request(
            snapshot.world_epoch.wrapping_sub(1),
            request(hit, false, snapshot.teleport_version),
        )
        .unwrap();
    connection
        .request(
            snapshot.world_epoch,
            request(hit, false, snapshot.teleport_version),
        )
        .unwrap();
    connection
        .request(
            snapshot.world_epoch,
            request(hit, true, snapshot.teleport_version),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if connection.take_snapshot().is_some_and(|s| {
            if s.report.vitals.health == 19.25 {
                assert!(!s.report.tags.contains(
                    "minecraft:entity_type",
                    "minecraft:redirectable_projectile",
                    127
                ));
                true
            } else {
                false
            }
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "No entity action completion: {:?}",
            connection.status()
        );
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(50));
    assert_eq!(connection.status(), Status::Playing);
    connection.cancel();
    server.join().unwrap();
}

#[test]
fn configuration_and_play_disconnect_preserve_server_text_instead_of_hex() {
    use net::nbt::{self, Tag};
    for play in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut wire = Wire {
                stream,
                compression: None,
                journal: None,
            };
            assert_eq!(wire.receive().unwrap().0, 0); // Handshake.
            assert_eq!(wire.receive().unwrap().0, 0); // Hello.
            let mut accepted = net::offline_uuid("Fixture").to_vec();
            string("Fixture", &mut accepted);
            varint(0, &mut accepted);
            wire.send(2, &accepted).unwrap();
            assert_eq!(wire.receive().unwrap(), (3, vec![]));
            let mut brand = Vec::new();
            string("minecraft:brand", &mut brand);
            string("OptiMC", &mut brand);
            assert_eq!(wire.receive().unwrap(), (2, brand));
            assert_eq!(wire.receive().unwrap().0, 0); // Client information follows brand.
            if play {
                wire.send(3, &[]).unwrap();
                assert_eq!(wire.receive().unwrap(), (3, vec![]));
            }
            let reason = Tag::Compound(
                [
                    ("text".into(), Tag::String("LuckyWorld ⟹\n".into())),
                    (
                        "extra".into(),
                        Tag::List {
                            element_type: 10,
                            values: vec![Tag::Compound(
                                [
                                    (
                                        "text".into(),
                                        Tag::String(
                                            "Вы провалили проверку. Возможно вы бот.".into(),
                                        ),
                                    ),
                                    ("bold".into(), Tag::Byte(1)),
                                ]
                                .into(),
                            )],
                        },
                    ),
                ]
                .into(),
            );
            let mut bytes = Vec::new();
            nbt::write_network(&reason, &mut bytes).unwrap();
            wire.send(if play { 0x20 } else { 2 }, &bytes).unwrap();
        });
        let error = net::session_state(
            &Address {
                host: "127.0.0.1".into(),
                port,
            },
            "Fixture",
            2,
        )
        .err()
        .expect("Server rejected session");
        assert_eq!(
            error.to_string(),
            "LuckyWorld ⟹\nВы провалили проверку. Возможно вы бот."
        );
        server.join().unwrap();
    }
}

#[test]
fn login_rejection_json_remains_styled_across_network_worker() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let cache =
        std::env::temp_dir().join(format!("optimc-rejected-{}-{stamp}", std::process::id()));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut wire = Wire {
            stream,
            compression: None,
            journal: None,
        };
        wire.receive().unwrap();
        wire.receive().unwrap();
        let json=serde_json::json!({"text":"", "extra":[{"text":"Lucky","color":"dark_green","bold":true},{"text":"World","color":"red","bold":true},{"text":"\n\nПожалуйста, попробуйте позже.","color":"gold"}]}).to_string();
        let mut bytes = Vec::new();
        string(&json, &mut bytes);
        wire.send(0, &bytes).unwrap();
    });
    let error = net::session_state_logged(
        &Address {
            host: "127.0.0.1".into(),
            port,
        },
        "Fixture",
        2,
        &cache,
    )
    .err()
    .unwrap();
    let reason = &error
        .downcast_ref::<net::component::Disconnect>()
        .unwrap()
        .0;
    assert_eq!(
        reason.plain_text(),
        "LuckyWorld\n\nПожалуйста, попробуйте позже."
    );
    assert_eq!(reason.runs[0].style.color, Some(0x00aa00));
    assert!(reason.runs[0].style.bold);
    assert_eq!(reason.runs[1].style.color, Some(0xff5555));
    let path = std::fs::read_dir(cache.join("network"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let log: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(log["outcome"], "server_disconnect");
    assert_eq!(log["server_disconnect_reason"], reason.plain_text());
    std::fs::remove_dir_all(cache).unwrap();
    server.join().unwrap();
}

#[test]
fn headless_idle_gameplay_uses_server_physics_and_acknowledges_world_once() {
    let (address, server) = world_fixture(true);
    let report = net::headless::session(&address, "Fixture", 1).unwrap();
    assert_eq!(report.chunks, 2);
    server.join().unwrap();
}

#[test]
fn server_zero_gravity_is_applied_during_play_without_a_false_fall() {
    let (address, server) = world_fixture_gravity(true, 0.);
    let report = net::headless::session(&address, "Fixture", 1).unwrap();
    assert_eq!(report.attributes.movement(99).gravity, 0.);
    assert_eq!(report.selected_slot, Some((4, 1)));
    server.join().unwrap();
}

#[test]
fn inbound_bundle_waits_for_close_without_stalling_gameplay_ticks() {
    let (address, server) = world_fixture_options(true, 0., true);
    let report = net::headless::session(&address, "Fixture", 2).unwrap();
    assert_eq!(report.hud.experience.level, 21);
    assert_eq!(report.hud.experience.total, 321);
    server.join().unwrap();
}

#[test]
fn play_eof_distinguishes_an_incomplete_frame_and_finishes_metadata_journal() {
    use std::io::Write;
    for (partial, bundle) in [(false, false), (true, false), (false, true)] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut wire = Wire {
                stream,
                compression: None,
                journal: None,
            };
            wire.receive().unwrap();
            wire.receive().unwrap();
            wire.send(2, &[]).unwrap();
            assert_eq!(wire.receive().unwrap().0, 3);
            assert_eq!(wire.receive().unwrap().0, 2);
            assert_eq!(wire.receive().unwrap().0, 0);
            wire.send(3, &[]).unwrap();
            assert_eq!(wire.receive().unwrap(), (3, vec![]));
            if partial {
                wire.stream.write_all(&[0x80]).unwrap();
            }
            if bundle {
                wire.send(0, &[]).unwrap();
                let mut xp = 0.25f32.to_be_bytes().to_vec();
                varint(21, &mut xp);
                varint(321, &mut xp);
                wire.send(0x67, &xp).unwrap();
            }
        });
        let cache = std::env::temp_dir().join(format!(
            "optimc-play-eof-{}-{partial}-{bundle}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&cache);
        let error = net::session_state_logged(
            &Address {
                host: "127.0.0.1".into(),
                port,
            },
            "Fixture",
            2,
            &cache,
        )
        .err()
        .unwrap();
        let reason = error.to_string();
        assert!(error.downcast_ref::<net::component::Disconnect>().is_none());
        assert!(
            reason.contains(if partial {
                "incomplete packet"
            } else if bundle {
                "incomplete bundle"
            } else {
                "without a disconnect message during Play"
            }),
            "{reason}"
        );
        server.join().unwrap();
        let path = std::fs::read_dir(cache.join("network"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let log: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(log["phase"], "play");
        assert_ne!(log["outcome"], "running");
        assert_eq!(log["counts"]["configuration/send/finish_configuration"], 1);
        std::fs::remove_dir_all(cache).unwrap();
    }
}

#[test]
fn server_inventory_actions_and_local_close_publish_without_a_server_reply() {
    use net::{
        entity::Pose,
        live::{Connection, Status},
        movement::Motion,
        request::{InventoryAction, Request},
    };
    use std::time::Instant;
    let (address, server) = world_fixture_full(
        true,
        0.3,
        false,
        false,
        false,
        false,
        (false, true, false, false, false),
    );
    let connection = Connection::connect(address, "Fixture".into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        assert!(Instant::now() < deadline, "{:?}", connection.status());
        if let Some(s) = connection.take_snapshot() {
            if s.position_ready
                && s.report
                    .inventory
                    .open
                    .as_ref()
                    .is_some_and(|m| m.data.get(&2) == Some(&42))
                && s.report
                    .unhandled
                    .contains_key("play:inventory:unsupported_component")
            {
                break s;
            }
        }
        thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(
        snapshot.report.unhandled["play:inventory:unsupported_component"],
        1
    );
    let inventory = &snapshot.report.inventory;
    let menu = inventory.open.as_ref().unwrap();
    assert_eq!((menu.id, menu.state, menu.slots[0].count), (300, 8, 6));
    assert!(std::sync::Arc::ptr_eq(
        &menu.slots[54],
        &inventory.player[36]
    ));
    assert_eq!(
        inventory.main_hand(0).unwrap().tool().unwrap().rules[0].speed,
        Some(12.5)
    );
    assert_eq!(inventory.carried.count, 0);
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    let ready = Instant::now() + Duration::from_secs(2);
    loop {
        if connection.take_snapshot().is_some_and(|s| s.client_loaded) {
            break;
        }
        assert!(Instant::now() < ready);
        thread::sleep(Duration::from_millis(5));
    }
    let click = net::menu_click::Click::new(54, 0, 0);
    for (id, serial) in [
        (301, inventory.menu_serial),
        (300, inventory.menu_serial.wrapping_sub(1)),
    ] {
        connection
            .request(
                snapshot.world_epoch,
                Request::MenuClick {
                    id,
                    serial,
                    state: 0,
                    click,
                },
            )
            .unwrap();
    }
    for row in container_click_reference()["packets"].as_array().unwrap() {
        connection
            .request(
                snapshot.world_epoch,
                Request::MenuClick {
                    id: 300,
                    serial: inventory.menu_serial,
                    state: 12345,
                    click: net::menu_click::Click::new(
                        row["slot"].as_i64().unwrap() as i16,
                        row["button"].as_u64().unwrap() as u8,
                        row["kind"].as_u64().unwrap() as u8,
                    ),
                },
            )
            .unwrap();
    }
    let motion = Motion {
        world_epoch: snapshot.world_epoch,
        teleport_version: snapshot.teleport_version,
        abilities_version: snapshot.abilities_version,
        velocity_version: snapshot.velocity_version,
        player_state_version: snapshot.player_state_version,
        pose: Pose {
            position: glam::DVec3::new(24., -45., -24.),
            ..Default::default()
        },
        on_ground: false,
        horizontal_collision: false,
        input: 0,
        sprinting: false,
        slot: 4,
        flying: false,
    };
    connection.send_motion(motion).unwrap();
    let action = |action, teleport_version| Request::InventoryAction {
        action,
        slot: 4,
        input: 0,
        teleport_version,
    };
    connection
        .request(
            snapshot.world_epoch.wrapping_sub(1),
            action(InventoryAction::DropAll, snapshot.teleport_version),
        )
        .unwrap();
    connection
        .request(
            snapshot.world_epoch,
            action(
                InventoryAction::DropAll,
                snapshot.teleport_version.wrapping_sub(1),
            ),
        )
        .unwrap();
    for a in [
        InventoryAction::SwapHands,
        InventoryAction::DropItem,
        InventoryAction::DropAll,
    ] {
        connection
            .request(snapshot.world_epoch, action(a, snapshot.teleport_version))
            .unwrap();
    }
    connection
        .request(
            snapshot.world_epoch,
            Request::CloseMenu {
                id: 301,
                serial: inventory.menu_serial,
            },
        )
        .unwrap();
    connection
        .request(
            snapshot.world_epoch,
            Request::CloseMenu {
                id: 300,
                serial: inventory.menu_serial.wrapping_sub(1),
            },
        )
        .unwrap();
    connection
        .request(
            snapshot.world_epoch,
            Request::CloseMenu {
                id: 300,
                serial: inventory.menu_serial,
            },
        )
        .unwrap();
    let close_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(s) = connection.take_snapshot() {
            if s.report.inventory.open.is_none() {
                assert_eq!(s.report.inventory.menu_serial, inventory.menu_serial + 1);
                assert_eq!(s.report.inventory.player, inventory.player);
                assert_eq!(s.report.inventory.revision, inventory.revision + 1);
                break;
            }
        }
        assert!(
            Instant::now() < close_deadline,
            "No locally published close: {:?}",
            connection.status()
        );
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(150));
    assert_eq!(connection.status(), Status::Playing);
    connection.cancel();
    server.join().unwrap();
}

fn input_requests(teleport_version: u64) -> Vec<net::request::Request> {
    use net::request::{DigAction, Request};
    let mut requests = Vec::new();
    for action in [
        DigAction::Start,
        DigAction::Abort,
        DigAction::Start,
        DigAction::Stop,
    ] {
        requests.push(Request::Dig {
            action,
            block: [-31, -22, 47],
            face: 2,
            slot: 4,
            input: 0,
            teleport_version,
        });
    }
    requests.push(Request::UseItem {
        hand: net::entity::Hand::Main,
        slot: 4,
        input: 0,
        yaw: -173.25,
        pitch: 32.5,
        teleport_version,
    });
    requests
}
#[test]
fn dig_and_air_clicks_keep_shared_sequences_and_drop_stale_mouse_input() {
    use net::{
        entity::Pose,
        live::{Connection, Status},
        movement::Motion,
    };
    use std::time::Instant;
    let (address, server) = world_fixture_full(
        true,
        0.3,
        false,
        false,
        false,
        false,
        (false, false, true, false, false),
    );
    let connection = Connection::connect(address, "Fixture".into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        assert!(Instant::now() < deadline, "{:?}", connection.status());
        if let Some(s) = connection.take_snapshot() {
            if s.position_ready && s.report.world.chunks.len() == 1 {
                break s;
            }
        }
        thread::sleep(Duration::from_millis(5));
    };
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    let ready = Instant::now() + Duration::from_secs(2);
    loop {
        if connection.take_snapshot().is_some_and(|s| s.client_loaded) {
            break;
        }
        assert!(Instant::now() < ready);
        thread::sleep(Duration::from_millis(5));
    }
    connection
        .send_motion(Motion {
            world_epoch: snapshot.world_epoch,
            teleport_version: snapshot.teleport_version,
            abilities_version: snapshot.abilities_version,
            velocity_version: snapshot.velocity_version,
            player_state_version: snapshot.player_state_version,
            pose: Pose {
                position: glam::DVec3::new(24., -45., -24.),
                ..Default::default()
            },
            on_ground: false,
            horizontal_collision: false,
            input: 0,
            sprinting: false,
            slot: 4,
            flying: false,
        })
        .unwrap();
    for request in input_requests(snapshot.teleport_version.wrapping_sub(1)) {
        connection.request(snapshot.world_epoch, request).unwrap();
    }
    for request in input_requests(snapshot.teleport_version) {
        connection
            .request(snapshot.world_epoch.wrapping_sub(1), request)
            .unwrap();
    }
    for request in input_requests(snapshot.teleport_version) {
        connection.request(snapshot.world_epoch, request).unwrap();
    }
    let done = Instant::now() + Duration::from_secs(2);
    loop {
        if connection
            .take_snapshot()
            .is_some_and(|s| s.report.last_block_ack == Some(4))
        {
            break;
        }
        assert!(Instant::now() < done, "{:?}", connection.status());
        thread::sleep(Duration::from_millis(5));
    }
    thread::sleep(Duration::from_millis(100));
    assert_eq!(connection.status(), Status::Playing);
    connection.cancel();
    server.join().unwrap();
}

fn container_click_reference() -> serde_json::Value {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(
        include_bytes!("../data/26.2/container-clicks.json.gz").as_slice(),
    )
    .read_to_end(&mut raw)
    .unwrap();
    serde_json::from_slice(&raw).unwrap()
}

#[test]
fn loading_marker_precedes_compilation_ack_and_stale_callbacks_are_rejected() {
    use net::live::Connection;
    use std::time::Instant;
    let (address, server) = world_fixture_full(
        true,
        0.3,
        false,
        false,
        false,
        false,
        (false, false, false, true, false),
    );
    let connection = Connection::connect(address, "Fixture".into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let first = loop {
        assert!(Instant::now() < deadline, "{:?}", connection.status());
        if let Some(s) = connection.take_snapshot() {
            if s.position_ready && !s.report.world.chunks.is_empty() {
                break s;
            }
        }
        thread::sleep(Duration::from_millis(2));
    };
    assert!(!first.loading_packets_received);
    assert!(!first
        .report
        .unhandled
        .keys()
        .any(|k| k.ends_with("/game_event")));
    connection.mark_loaded(first.world_epoch, first.teleport_version);
    let marked = loop {
        assert!(Instant::now() < deadline, "{:?}", connection.status());
        if let Some(s) = connection.take_snapshot() {
            assert!(
                !s.client_loaded,
                "Compiled notification before marker completed loading"
            );
            if s.loading_packets_received {
                assert!(!s
                    .report
                    .unhandled
                    .keys()
                    .any(|k| k.ends_with("/game_event")));
                break s;
            }
        }
        thread::sleep(Duration::from_millis(2));
    };
    connection.mark_loaded(marked.world_epoch.wrapping_sub(1), marked.teleport_version);
    thread::sleep(Duration::from_millis(60));
    assert!(!connection.take_snapshot().is_some_and(|s| s.client_loaded));
    connection.mark_loaded(marked.world_epoch, marked.teleport_version);
    loop {
        assert!(Instant::now() < deadline, "{:?}", connection.status());
        if connection.take_snapshot().is_some_and(|s| s.client_loaded) {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    connection.mark_loaded(marked.world_epoch, marked.teleport_version);
    thread::sleep(Duration::from_millis(60));
    if let Some(s) = connection.take_snapshot() {
        assert!(!s
            .report
            .unhandled
            .keys()
            .any(|k| k.ends_with("/game_event")));
    }
    connection.cancel();
    server.join().unwrap(); // fixture verifies exactly one player_loaded
}

#[test]
fn server_sign_editor_updates_once_and_rejects_stale_authorization() {
    use net::{live::Connection, request::Request};
    use std::time::Instant;
    let (address, server) = world_fixture_full(
        true,
        0.3,
        false,
        false,
        false,
        false,
        (false, false, false, false, true),
    );
    let connection = Connection::connect(address, "Fixture".into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        assert!(Instant::now() < deadline, "{:?}", connection.status());
        if let Some(s) = connection.take_snapshot() {
            if s.position_ready && s.report.sign_editor.open.is_some() {
                break s;
            }
        }
        thread::sleep(Duration::from_millis(2));
    };
    let open = snapshot.report.sign_editor.open.as_ref().unwrap();
    assert_eq!(open.position, [24, -45, -24]);
    assert!(!snapshot
        .report
        .unhandled
        .keys()
        .any(|k| k.ends_with("/open_sign_editor")));
    let request = Request::SignUpdate {
        serial: open.serial,
        position: open.position,
        front: open.front,
        lines: [
            "Привет".into(),
            "  ".into(),
            "😀🌲".into(),
            "Четвёртая".into(),
        ],
    };
    connection
        .request(snapshot.world_epoch.wrapping_sub(1), request.clone())
        .unwrap();
    let mut stale = request.clone();
    if let Request::SignUpdate { serial, .. } = &mut stale {
        *serial = serial.wrapping_sub(1);
    }
    connection.request(snapshot.world_epoch, stale).unwrap();
    connection
        .request(snapshot.world_epoch, request.clone())
        .unwrap();
    connection.request(snapshot.world_epoch, request).unwrap();
    connection.mark_loaded(snapshot.world_epoch, snapshot.teleport_version);
    loop {
        assert!(Instant::now() < deadline, "{:?}", connection.status());
        if connection
            .take_snapshot()
            .is_some_and(|s| s.client_loaded && s.report.sign_editor.open.is_none())
        {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    thread::sleep(Duration::from_millis(350));
    connection.cancel();
    server.join().unwrap();
}
