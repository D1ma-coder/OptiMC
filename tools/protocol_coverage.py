#!/usr/bin/env python3
"""Conservative exhaustive inventory coverage, never a claim of complete synchronization."""
import collections,hashlib,json,pathlib,re
root=pathlib.Path(__file__).resolve().parents[1]
inventory=json.loads((root/'docs/protocol-776-inventory.json').read_text())
source=(root/'src/net/mod.rs').read_text();entity=(root/'src/net/entity.rs').read_text()
handled={(phase.lower(),int(value,0)) for phase,value in re.findall(r'\(State::(Login|Configuration|Play)\s*,\s*(0x[0-9a-fA-F]+|\d+)\)',source)}
handled|={('play',0),('play',0x65),('play',0x83),('play',0x33),('play',0x63)}
# Partial brand/game-event dispatch uses boolean guards, not match arms.
handled|={('configuration',1),('play',0x18),('play',0x26),('play',0x02),('play',0x07)}
# Store::handles uses an inclusive range; retain every guarded inventory dispatch.
handled|={('play',n) for n in [0x11,0x12,0x13,0x14,0x3b,0x60,0x6c]}
scoreboard=(root/'src/net/scoreboard.rs').read_text().split('pub fn handles',1)[1].split('pub fn packet',1)[0]
handled|={('play',int(n,16)) for n in re.findall(r'0x[0-9a-fA-F]+',scoreboard)}
hud=(root/'src/net/hud.rs').read_text().split('pub fn handles',1)[1].split('pub fn packet',1)[0]
handled|={('play',int(n,16)) for n in re.findall(r'0x[0-9a-fA-F]+',hud)}
entity=entity[entity.index('pub fn decode'):entity.index('pub fn decode')+8000] if 'pub fn decode' in entity else entity
handled|={('play',int(n,16)) for n in re.findall(r'0x[0-9a-fA-F]+',entity[:entity.index('pub fn apply')])} if 'pub fn apply' in entity else {('play',n) for n in [1,0x23,0x35,0x36,0x38,0x4d,0x53,0x65,0x7d]}
# Sender paths inspected in SessionMachine, begin_login, movement::Sender and live::run.
sent={'handshake':{0},'status':{0,1},'login':{0,2,3,4},'configuration':{0,1,2,3,4,5,6,7},'play':{0,0x06,0x07,0x09,0x0f,0x0b,0x0d,0x10,0x15,0x1c,0x1e,0x1f,0x20,0x21,0x28,0x2a,0x2b,0x2c,0x2d,0x31,0x35,0x42,0x01,0x1a,0x3f,0x13,0x29,0x43,0x3d}}
notes={
 ('play','open_sign_editor'):'Valid sign block opens a four-line editor using source GUI textures, front/back NBT and width limits. Core Unicode cursor/selection/line navigation is wired. Clipboard, IME preedit, exact validity auto-close, full fonts and local world-text prediction remain incomplete.',
 ('play','sign_update'):'Four raw lines + side + packed position, source wire codec verified. Worker validates current world and server editor serial/side/position, sends once and publishes closure. Full GUI parity remains incomplete.',
 ('play','light_update'):'Source masks/nibble arrays decode and persist with partial/empty-layer semantics; same data is read from chunk packets. Light propagation, mesh/lightmap shading and neighbor dirtying are not wired yet.',
 ('configuration','custom_payload'):'minecraft:brand is decoded; other channels retain diagnostics. Plugin messages are not fully implemented.',
 ('play','custom_payload'):'minecraft:brand is decoded; other channels retain diagnostics.',
 ('play','game_event'):'CHANGE_GAME_MODE rounding/abilities and LEVEL_CHUNKS_LOAD_START loading tracker, marker gate, compiled-column callback, timeout and exceptions. Weather 1/2/7/8 updates state using original setters; rain visuals/sound, respawn options and other actions remain incomplete.',
 ('play','change_difficulty'):'Original VarInt WRAP mapping and locked state retained and shown in diagnostics. All difficulty consumers remain incomplete.',
 ('play','set_simulation_distance'):'Initial login and update signed distance retained. Full client entity tick-range behavior remains incomplete.',
 ('play','set_time'):'Registry-indexed world clock states and ClientClockManager updates tested against original. Sky/day-night rendering is not wired yet.',
 ('play','set_default_spawn_position'):'Dimension/position/yaw/pitch retained. Compass consumer remains incomplete.',
 ('play','set_chunk_cache_center'):'Server center retained; native ring storage/inRange/pruning remains incomplete.',
 ('play','set_chunk_cache_radius'):'Server radius retained separately from render distance; native cache resizing remains incomplete.',

 ('play','container_close'):'Server close clears the active menu regardless of supplied ID, like vanilla. Client close uses ContainerID VarInt and current menu serial; publishes local closure without requiring a server reply. Cursor return/drop reconciliation remains incomplete.',
 ('play','container_set_content'):'Atomic content/state/cursor updates, player-slot sharing and source menu layouts. Unknown undelimited components reject the entire update and increment diagnostics. Click prediction, hash reconciliation and full component support remain incomplete.',
 ('play','container_set_slot'):'Atomic state/slot replacement for player or active menu; stale menu IDs ignored. Slot stacks preserve exact supported component patches. Client click transactions remain incomplete.',
 ('play','container_set_data'):'Stores signed-short server menu properties with a bounded map. Most specialized property-driven widgets remain incomplete.',
 ('play','open_screen'):'Opens server-titled menu with source slot layout; basic official backgrounds and read-only slots are wired. Six layouts and all click/recipe/button semantics remain incomplete.',
 ('play','set_cursor_item'):'Retains supported component patches, including filtered books and recursively nested container/bundle/projectile templates. Unknown components do not publish partial inventory. Client click prediction is incomplete.',
 ('play','set_player_inventory'):'Maps direct player inventory indices to shared InventoryMenu and active-window slots; original armor/offhand/hotbar index order is tested.',
 ('play','player_action'):'F/Q drops and LMB START/ABORT/STOP destruction are wired with current slot/input and world/teleport fences. Client-tick progress uses source hardness, server tool/tags/attributes/effects. Local block prediction, crack overlays and full item-specific mining semantics remain incomplete.',
 ('play','use_item'):'RMB in air emits source hand/sequence/yaw/pitch fields, including an offhand when the main hand is empty. Slot/input sync and stale-generation rejection are TCP-tested. Full use-duration, release and interaction-result fallback semantics remain incomplete.',
 ('play','update_mob_effect'):'Stores validated effect amplifier/duration/flags atomically; haste/conduit power/fatigue feed mining speed. Full HUD/particles and all effect consumers remain incomplete.',
 ('play','remove_mob_effect'):'Removes server effect snapshots atomically; mining immediately observes removal. All remaining vanilla side effects remain incomplete.',
 ('play','attack'):'User click sends split attack + main-hand swing after slot/input sync. Target UUID, world and teleport are fenced. Item ATTACK_RANGE, full cooldown and hand animation remain incomplete.',
 ('play','interact'):'User main-hand use is wired with LP relative hit and secondary flag; offhand fallback, item InteractionResult and containers remain incomplete.',
 ('play','swing'):'Main-hand swing follows attack and user click misses/blocks; full hand animation and mining remain incomplete.',
 ('play','update_tags'):'Replaces tags per registry transactionally with global budgets; projectile picking consumes redirectable_projectile. Other tag consumers remain incomplete.',
 ('configuration','update_tags'):'Retains packet registry/tag/member IDs with bounded atomic replacement; projectile picking consumes these after Play.',

 ('play','block_changed_ack'):'Sequence is decoded and retained for the current world. Full client prediction/reconciliation is not implemented.',
 ('play','use_item_on'):'Main-hand block clicks/hold are wired; current slot/input are flushed first and stale teleport/world targets dropped. Baseline entity-priority picking is wired; special stateful dimensions, interaction-shape override, prediction, item-specific logic and container state remain incomplete.',

 ('play','block_entity_data'):'Chunk/standalone block-entity NBT snapshots are stored and invalidated. Both sign sides feed live bitmap text meshes. Other block entity renderers, lightmap and sign editing remain incomplete.',
 ('play','player_info_update'):'All eight actions, profile properties and unverified chat-session keys decode transactionally. Offline Tab/player-name completion is wired; heads, LIST hearts and signed-chat key verification remain incomplete.',
 ('play','player_info_remove'):'Removes known player profiles and updates Tab/completion snapshots; entity/profile lifecycle parity remains incomplete.',
 ('play','tab_list'):'Rich header/footer and wrapped offline Tab layout are wired; full original font and online-mode skin behavior remain incomplete.',
 ('play','commands'):'All 57 original argument-property payloads and graph redirects decode. Root completion works; full Brigadier context/execution parity is not implemented.',
 ('play','command_suggestions'):'Request ID, UTF-16 replacement range, strings and optional rich tooltips decode. Full dropdown/tooltip/context behavior remains incomplete.',
 ('play','custom_chat_completions'):'ADD/REMOVE/SET update the bounded local completion set; full chat UI parity remains incomplete.',
 ('play','player_chat'):'Original packed body/filter/bound-chat-type and signature cache decode; LastSeen/ACK connected. Cryptographic chain validation and complete chat semantics are not implemented.',
 ('play','disguised_chat'):'Original component and registry-driven chat decoration feed chat; full font/events behavior remains incomplete.',
 ('play','delete_chat'):'Signature references mark messages for delayed replacement; wall-clock approximation differs from exact client tick semantics.',

 ('play','map_item_data'):'Original palette, full/partial 128x128 pixels and optional decorations decode transactionally. Maps and original frame models are wired to live geometry. Decoration sprites/names and map lighting parity remain incomplete.',
 ('play','set_entity_data'):'Original item-frame indices and supported serializer/component codecs are JAR-fixture tested. Unknown codecs are counted as unhandled; special entity metadata and all variant/animation effects remain incomplete.',
 ('play','add_entity'):'Tracked poses now feed live geometry for 90 official base models and item/glow frames. Special renderers, player skins, equipment, state variants and limb animations remain incomplete.',
 ('login','hello'):'Online encryption/authentication is explicitly rejected; offline-only login remains supported.',
 ('configuration','code_of_conduct'):'Disconnects with an explicit unsupported interactive acceptance message; no automatic acceptance.',
 ('configuration','select_known_packs'):'Replies with no selected known packs; server must transmit complete registry data.',
 ('configuration','resource_pack_push'):'Declines all server packs; no pack UI/download/apply support.',
 ('play','resource_pack_push'):'Declines all server packs; no pack UI/download/apply support.',
 ('play','chunk_batch_finished'):'Original timing-based ChunkBatchSizeCalculator smoothing; off-thread decoding does not yet measure all main-thread/GPU work.',
 ('play','player_position'):'Relative pose/velocity correction and teleport ACK. Does not reset the global physics phase or LocalPlayer sendPosition tracker; readiness is retried against current generation.',
 ('play','update_attributes'):'All 40 official attribute definitions and ordered modifier operations decode; local gravity, drag, friction, movement speed, jump and step affect physics. Entity scaling and all attribute-driven rendering/interaction remain incomplete.',
 ('play','set_held_slot'):'Valid server-selected slots update the hotbar; queued motion is fenced by player-state version. Invalid slots are ignored as in the original listener.',
 ('play','set_entity_motion'):'LP Vec3 applies to remote entities and local velocity; version fence rejects queued old physics ticks.',
 ('play','boss_event'):'All six operations, rich name, flags and 100 ms progress interpolation; music/fog/darken-world side effects remain unwired.',
 ('play','set_objective'):'Add/change/remove display component, render type and optional number format. Sidebar uses original integer layout; player-list hearts and below-name rendering remain unwired.',
 ('play','set_score'):'Authoritative signed value, optional display and blank/styled/fixed number format. Top 15 sidebar rows ordered by score and Java case-insensitive owner. Fonts, bidi and interactive text parity remain incomplete.',
 ('play','reset_score'):'Removes one objective score or all scores for the owner; original nullable UTF format tested against JAR-encoded bodies.',
 ('play','set_player_team'):'All five operations and 26.2 parameter fields; member migration, prefix/suffix and inherited optional color feed sidebar. Visibility/collision flags stored; entity name-tag, collision and friendly-invisible effects remain unwired.',
 ('play','set_display_objective'):'All 19 display slots stored, team-color sidebar override and general fallback. Integer LIST is wired to Tab; LIST hearts and BELOW_NAME remain unwired.',
 ('play','set_experience'):'Authoritative progress/level/total with original GUI sprites; mount/spectator HUD alternatives remain incomplete.',
 ('play','set_title_text'):'Title/subtitle/actionbar/clear/timing synchronization and fades; freeze/pause edge cases still need comparison.',
 ('play','system_chat'):'Rich NBT component + overlay bit, bounded message history and HUD. Signed/player/disguised chat remains missing.',
 ('play','bundle'):'Subpackets apply in order only after closing delimiter, as PacketBundlePacker. Outgoing gameplay continues against the committed world. Original 4096 packet limit; additional 64 MiB allocation bound and explicit incomplete-bundle EOF. Malformed handlers are not a rollback transaction.',
 ('play','level_chunk_with_light'):'Blocks decode/render; full light propagation, biomes, block-entity rendering and heightmap consumers remain incomplete.',
 ('play','login'):'Dimension/spawn/entity ID; several login policy/distance/HUD fields are not yet synchronized.',
 ('play','respawn'):'Dimension/pose fences and attribute bases with KEEP_ATTRIBUTE_MODIFIERS. Inventory, effects and all entity-data keep semantics remain incomplete.',
 ('play','interact'):'Encoder exists in entity.rs, but user interaction path is not wired.',
 ('play','attack'):'Encoder exists in entity.rs, but user attack path is not wired.',
 ('play','client_tick_end'):'Sent after accepted local physics ticks in GUI and connect --simulate-player. Plain diagnostic connect does not run player physics.',
 ('play','player_loaded'):'Sent once after a versioned GUI/world readiness request. Actual graphical path not re-run after WindowServer crash.',
}
rows=[]
for r in inventory['packets']:
 row=dict(r);phase,flow,number=r['phase'],r['flow'],r['id'];name=r['name']
 if flow=='clientbound':
  mode='partial_handler' if (phase,number) in handled or phase=='status' else 'missing_handler'
  if phase=='login' and name=='hello':mode='explicitly_unsupported'
  if phase=='configuration' and name=='code_of_conduct':mode='explicitly_unsupported'
  location='src/net/mod.rs; src/net/entity.rs; src/net/live.rs; src/net/hud.rs; src/net/attributes.rs; src/net/bundle.rs; src/net/scoreboard.rs' if mode=='partial_handler' else 'not implemented'
 else:
  mode='emitted_partial' if number in sent.get(phase,set()) else 'not_emitted'
  if phase=='play' and name in ['attack','interact'] and number not in sent.get(phase,set()):mode='encoder_only'
  location='src/net/mod.rs; src/net/movement.rs; src/net/live.rs' if mode=='emitted_partial' else 'not wired'
 row.update(coverage=mode,implementation_reference=location,limitations=notes.get((phase,name),''),full_semantic_parity_verified=False)
 rows.append(row)
counts=collections.Counter(r['coverage'] for r in rows)
report={'minecraft':'26.2','protocol':776,'source_client_sha1':inventory['source_client_sha1'],'scope':'All 256 registered packet types inspected against dispatch/sender coverage. partial does not mean complete field semantics, full synchronization or successful anti-bot verification.','counts':dict(counts),'full_semantic_parity_verified':False,'packets':rows}
(root/'docs/protocol-776-coverage.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
lines=['# Аудит протокола 776 (Minecraft Java 26.2)','',f'Источник: официальный client.jar SHA-1 `{inventory["source_client_sha1"]}`. Регистрации извлечены javap из всех пяти Protocols-классов. Для каждого clientbound пакета найден оригинальный listener handler (включая наследуемые). Кодеки всех зарегистрированных типов сохранены в `.cache/protocol-audit/packet-codecs.txt`; обработчики — в `client-handlers.txt`. Сервер, Minecraft и GPU для аудита не запускались.','',
'**Полная синхронизация не достигнута. Ни один пакет в этом отчёте не объявляется полностью проверенным по всем полям и побочным эффектам.** Таблица охватывает весь реестр и выявляет пропуски; `partial_handler` означает наличие ветки/декодера, а не полное совпадение состояния или всех побочных эффектов с оригиналом. Ключи/ID Play и Configuration дополнительно сопоставлены с предыдущей проверенной таблицей без расхождений.','',
'`missing_handler`: пакет пока только учитывается в bounded diagnostic counters и журнале по имени. `explicitly_unsupported`: явный отказ вместо симуляции успеха. `emitted_partial`: отправка подключена, но вся семантика ещё требует проверки. `encoder_only`: есть формат, нет игрового действия.','',
'Приоритет оставшихся пробелов: inventory/container revision и подтверждения действий; entity metadata, attributes/effects и player info; game events/health/experience; tags/features/recipes/commands; player/disguised/signed chat ACK; custom payload/dialogs; resource-pack UI и transfer; biome/light/block-entity state; vehicle/entity interaction и prediction. Поведение rendering/presentation после WindowServer crash не проверялось на GPU.','',
'Воспроизведение: `python3 tools/audit_protocol.py` (development JDK/javap), затем `python3 tools/protocol_coverage.py` и `cargo fmt`. JSON версии таблицы: `docs/protocol-776-inventory.json`, `docs/protocol-776-coverage.json`. Runtime lookup генерируется в Rust; Java/Python не нужны клиенту.','',
'Сводка: '+', '.join(f'{k}={v}' for k,v in sorted(counts.items()))+'.','']
for phase in ['handshake','status','login','configuration','play']:
 for flow in ['clientbound','serverbound']:
  selected=[r for r in rows if r['phase']==phase and r['flow']==flow]
  if not selected:continue
  lines += [f'## {phase} / {flow} ({len(selected)})','', '| ID | Пакет | Original handler | Состояние | Ограничение |','|---|---|---|---|---|']
  for r in selected:
   handler=(r.get('official_handler') or {}).get('method','—')
   lines.append(f'| `0x{r["id"]:02x}` | `{r["name"]}` | `{handler}` | {r["coverage"]} | {r["limitations"].replace("|","/")} |')
  lines.append('')
(root/'docs/PROTOCOL-AUDIT.md').write_text('\n'.join(lines)+'\n')
print(dict(counts))
