# Аудит протокола 776 (Minecraft Java 26.2)

Источник: официальный client.jar SHA-1 `2dc72797acbc1b63fc16a11c4ac393605f453754`. Регистрации извлечены javap из всех пяти Protocols-классов. Для каждого clientbound пакета найден оригинальный listener handler (включая наследуемые). Кодеки всех зарегистрированных типов сохранены в `.cache/protocol-audit/packet-codecs.txt`; обработчики — в `client-handlers.txt`. Сервер, Minecraft и GPU для аудита не запускались.

**Полная синхронизация не достигнута. Ни один пакет в этом отчёте не объявляется полностью проверенным по всем полям и побочным эффектам.** Таблица охватывает весь реестр и выявляет пропуски; `partial_handler` означает наличие ветки/декодера, а не полное совпадение состояния или всех побочных эффектов с оригиналом. Ключи/ID Play и Configuration дополнительно сопоставлены с предыдущей проверенной таблицей без расхождений.

`missing_handler`: пакет пока только учитывается в bounded diagnostic counters и журнале по имени. `explicitly_unsupported`: явный отказ вместо симуляции успеха. `emitted_partial`: отправка подключена, но вся семантика ещё требует проверки. `encoder_only`: есть формат, нет игрового действия.

Приоритет оставшихся пробелов: inventory/container revision и подтверждения действий; entity metadata, attributes/effects и player info; game events/health/experience; tags/features/recipes/commands; player/disguised/signed chat ACK; custom payload/dialogs; resource-pack UI и transfer; biome/light/block-entity state; vehicle/entity interaction и prediction. Поведение rendering/presentation после WindowServer crash не проверялось на GPU.

Воспроизведение: `python3 tools/audit_protocol.py` (development JDK/javap), затем `python3 tools/protocol_coverage.py` и `cargo fmt`. JSON версии таблицы: `docs/protocol-776-inventory.json`, `docs/protocol-776-coverage.json`. Runtime lookup генерируется в Rust; Java/Python не нужны клиенту.

Сводка: emitted_partial=44, explicitly_unsupported=2, missing_handler=71, not_emitted=43, partial_handler=96.

## handshake / serverbound (1)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `client_intention` | `—` | emitted_partial |  |

## status / clientbound (2)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `status_response` | `handleStatusResponse` | partial_handler |  |
| `0x01` | `pong_response` | `handlePongResponse` | partial_handler |  |

## status / serverbound (2)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `status_request` | `—` | emitted_partial |  |
| `0x01` | `ping_request` | `—` | emitted_partial |  |

## login / clientbound (6)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `login_disconnect` | `handleDisconnect` | partial_handler |  |
| `0x01` | `hello` | `handleHello` | explicitly_unsupported | Online encryption/authentication is explicitly rejected; offline-only login remains supported. |
| `0x02` | `login_finished` | `handleLoginFinished` | partial_handler |  |
| `0x03` | `login_compression` | `handleCompression` | partial_handler |  |
| `0x04` | `custom_query` | `handleCustomQuery` | partial_handler |  |
| `0x05` | `cookie_request` | `handleRequestCookie` | partial_handler |  |

## login / serverbound (5)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `hello` | `—` | emitted_partial | Online encryption/authentication is explicitly rejected; offline-only login remains supported. |
| `0x01` | `key` | `—` | not_emitted |  |
| `0x02` | `custom_query_answer` | `—` | emitted_partial |  |
| `0x03` | `login_acknowledged` | `—` | emitted_partial |  |
| `0x04` | `cookie_response` | `—` | emitted_partial |  |

## configuration / clientbound (20)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `cookie_request` | `handleRequestCookie` | partial_handler |  |
| `0x01` | `custom_payload` | `handleDisconnect` | partial_handler | minecraft:brand is decoded; other channels retain diagnostics. Plugin messages are not fully implemented. |
| `0x02` | `disconnect` | `handleDisconnect` | partial_handler |  |
| `0x03` | `finish_configuration` | `handleConfigurationFinished` | partial_handler |  |
| `0x04` | `keep_alive` | `handleKeepAlive` | partial_handler |  |
| `0x05` | `ping` | `handlePing` | partial_handler |  |
| `0x06` | `reset_chat` | `handleResetChat` | missing_handler |  |
| `0x07` | `registry_data` | `handleRegistryData` | partial_handler |  |
| `0x08` | `resource_pack_pop` | `handleResourcePackPop` | missing_handler |  |
| `0x09` | `resource_pack_push` | `handleResourcePackPush` | partial_handler | Declines all server packs; no pack UI/download/apply support. |
| `0x0a` | `store_cookie` | `handleStoreCookie` | partial_handler |  |
| `0x0b` | `transfer` | `handleTransfer` | missing_handler |  |
| `0x0c` | `update_enabled_features` | `handleEnabledFeatures` | missing_handler |  |
| `0x0d` | `update_tags` | `handleUpdateTags` | partial_handler | Retains packet registry/tag/member IDs with bounded atomic replacement; projectile picking consumes these after Play. |
| `0x0e` | `select_known_packs` | `handleSelectKnownPacks` | partial_handler | Replies with no selected known packs; server must transmit complete registry data. |
| `0x0f` | `custom_report_details` | `handleCustomReportDetails` | missing_handler |  |
| `0x10` | `server_links` | `handleServerLinks` | missing_handler |  |
| `0x11` | `clear_dialog` | `handleClearDialog` | missing_handler |  |
| `0x12` | `show_dialog` | `handleCodeOfConduct` | missing_handler |  |
| `0x13` | `code_of_conduct` | `handleCodeOfConduct` | explicitly_unsupported | Disconnects with an explicit unsupported interactive acceptance message; no automatic acceptance. |

## configuration / serverbound (10)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `client_information` | `—` | emitted_partial |  |
| `0x01` | `cookie_response` | `—` | emitted_partial |  |
| `0x02` | `custom_payload` | `—` | emitted_partial | minecraft:brand is decoded; other channels retain diagnostics. Plugin messages are not fully implemented. |
| `0x03` | `finish_configuration` | `—` | emitted_partial |  |
| `0x04` | `keep_alive` | `—` | emitted_partial |  |
| `0x05` | `pong` | `—` | emitted_partial |  |
| `0x06` | `resource_pack` | `—` | emitted_partial |  |
| `0x07` | `select_known_packs` | `—` | emitted_partial | Replies with no selected known packs; server must transmit complete registry data. |
| `0x08` | `custom_click_action` | `—` | not_emitted |  |
| `0x09` | `accept_code_of_conduct` | `—` | not_emitted |  |

## play / clientbound (141)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `bundle` | `createEntityFromPacket` | partial_handler | Subpackets apply in order only after closing delimiter, as PacketBundlePacker. Outgoing gameplay continues against the committed world. Original 4096 packet limit; additional 64 MiB allocation bound and explicit incomplete-bundle EOF. Malformed handlers are not a rollback transaction. |
| `0x01` | `add_entity` | `createEntityFromPacket` | partial_handler | Tracked poses now feed live geometry for 90 official base models and item/glow frames. Special renderers, player skins, equipment, state variants and limb animations remain incomplete. |
| `0x02` | `animate` | `handleAnimate` | partial_handler |  |
| `0x03` | `award_stats` | `handleAwardStats` | missing_handler |  |
| `0x04` | `block_changed_ack` | `handleBlockChangedAck` | partial_handler | Sequence is decoded and retained for the current world. Full client prediction/reconciliation is not implemented. |
| `0x05` | `block_destruction` | `handleBlockDestruction` | missing_handler |  |
| `0x06` | `block_entity_data` | `handleBlockEntityData` | partial_handler | Chunk/standalone block-entity NBT snapshots are stored and invalidated. Both sign sides feed live bitmap text meshes. Other block entity renderers, lightmap and sign editing remain incomplete. |
| `0x07` | `block_event` | `handleBlockEvent` | partial_handler |  |
| `0x08` | `block_update` | `handleBlockUpdate` | partial_handler |  |
| `0x09` | `boss_event` | `handleBossUpdate` | partial_handler | All six operations, rich name, flags and 100 ms progress interpolation; music/fog/darken-world side effects remain unwired. |
| `0x0a` | `change_difficulty` | `handleChangeDifficulty` | partial_handler | Original VarInt WRAP mapping and locked state retained and shown in diagnostics. All difficulty consumers remain incomplete. |
| `0x0b` | `chunk_batch_finished` | `handleChunkBatchFinished` | partial_handler | Original timing-based ChunkBatchSizeCalculator smoothing; off-thread decoding does not yet measure all main-thread/GPU work. |
| `0x0c` | `chunk_batch_start` | `handleChunkBatchStart` | partial_handler |  |
| `0x0d` | `chunks_biomes` | `handleChunksBiomes` | missing_handler |  |
| `0x0e` | `clear_titles` | `handleTitlesClear` | partial_handler |  |
| `0x0f` | `command_suggestions` | `handleCommandSuggestions` | partial_handler | Request ID, UTF-16 replacement range, strings and optional rich tooltips decode. Full dropdown/tooltip/context behavior remains incomplete. |
| `0x10` | `commands` | `handleCommands` | partial_handler | All 57 original argument-property payloads and graph redirects decode. Root completion works; full Brigadier context/execution parity is not implemented. |
| `0x11` | `container_close` | `handleContainerClose` | partial_handler | Server close clears the active menu regardless of supplied ID, like vanilla. Client close uses ContainerID VarInt and current menu serial; publishes local closure without requiring a server reply. Cursor return/drop reconciliation remains incomplete. |
| `0x12` | `container_set_content` | `handleContainerContent` | partial_handler | Atomic content/state/cursor updates, player-slot sharing and source menu layouts. Unknown undelimited components reject the entire update and increment diagnostics. Click prediction, hash reconciliation and full component support remain incomplete. |
| `0x13` | `container_set_data` | `handleContainerSetData` | partial_handler | Stores signed-short server menu properties with a bounded map. Most specialized property-driven widgets remain incomplete. |
| `0x14` | `container_set_slot` | `handleContainerSetSlot` | partial_handler | Atomic state/slot replacement for player or active menu; stale menu IDs ignored. Slot stacks preserve exact supported component patches. Client click transactions remain incomplete. |
| `0x15` | `cookie_request` | `handleRequestCookie` | partial_handler |  |
| `0x16` | `cooldown` | `handleItemCooldown` | missing_handler |  |
| `0x17` | `custom_chat_completions` | `handleCustomChatCompletions` | partial_handler | ADD/REMOVE/SET update the bounded local completion set; full chat UI parity remains incomplete. |
| `0x18` | `custom_payload` | `handleDamageEvent` | partial_handler | minecraft:brand is decoded; other channels retain diagnostics. |
| `0x19` | `damage_event` | `handleDamageEvent` | missing_handler |  |
| `0x1a` | `debug_block_value` | `handleDebugBlockValue` | missing_handler |  |
| `0x1b` | `debug_chunk_value` | `handleDebugChunkValue` | missing_handler |  |
| `0x1c` | `debug_entity_value` | `handleDebugEntityValue` | missing_handler |  |
| `0x1d` | `debug_event` | `handleDebugEvent` | missing_handler |  |
| `0x1e` | `debug_sample` | `handleDebugSample` | missing_handler |  |
| `0x1f` | `delete_chat` | `handleDeleteChat` | partial_handler | Signature references mark messages for delayed replacement; wall-clock approximation differs from exact client tick semantics. |
| `0x20` | `disconnect` | `handleDisconnect` | partial_handler |  |
| `0x21` | `disguised_chat` | `handleDisguisedChat` | partial_handler | Original component and registry-driven chat decoration feed chat; full font/events behavior remains incomplete. |
| `0x22` | `entity_event` | `handleEntityEvent` | missing_handler |  |
| `0x23` | `entity_position_sync` | `handleEntityPositionSync` | partial_handler |  |
| `0x24` | `explode` | `handleExplosion` | missing_handler |  |
| `0x25` | `forget_level_chunk` | `queueLightRemoval` | partial_handler |  |
| `0x26` | `game_event` | `handleGameEvent` | partial_handler | CHANGE_GAME_MODE rounding/abilities and LEVEL_CHUNKS_LOAD_START loading tracker, marker gate, compiled-column callback, timeout and exceptions. Weather 1/2/7/8 updates state using original setters; rain visuals/sound, respawn options and other actions remain incomplete. |
| `0x27` | `game_rule_values` | `handleGameRuleValues` | missing_handler |  |
| `0x28` | `game_test_highlight_pos` | `handleGameTestHighlightPos` | missing_handler |  |
| `0x29` | `mount_screen_open` | `handleMountScreenOpen` | missing_handler |  |
| `0x2a` | `hurt_animation` | `handleHurtAnimation` | missing_handler |  |
| `0x2b` | `initialize_border` | `handleInitializeBorder` | missing_handler |  |
| `0x2c` | `keep_alive` | `handleKeepAlive` | partial_handler |  |
| `0x2d` | `level_chunk_with_light` | `handleLevelChunkWithLight` | partial_handler | Blocks decode/render; full light propagation, biomes, block-entity rendering and heightmap consumers remain incomplete. |
| `0x2e` | `level_event` | `handleLevelEvent` | missing_handler |  |
| `0x2f` | `level_particles` | `handleParticleEvent` | missing_handler |  |
| `0x30` | `light_update` | `handleLightUpdatePacket` | partial_handler | Source masks/nibble arrays decode and persist with partial/empty-layer semantics; same data is read from chunk packets. Light propagation, mesh/lightmap shading and neighbor dirtying are not wired yet. |
| `0x31` | `login` | `handleLogin` | partial_handler | Dimension/spawn/entity ID; several login policy/distance/HUD fields are not yet synchronized. |
| `0x32` | `low_disk_space_warning` | `handleLowDiskSpaceWarning` | missing_handler |  |
| `0x33` | `map_item_data` | `handleMapItemData` | partial_handler | Original palette, full/partial 128x128 pixels and optional decorations decode transactionally. Maps and original frame models are wired to live geometry. Decoration sprites/names and map lighting parity remain incomplete. |
| `0x34` | `merchant_offers` | `handleMerchantOffers` | missing_handler |  |
| `0x35` | `move_entity_pos` | `handleMoveEntity` | partial_handler |  |
| `0x36` | `move_entity_pos_rot` | `handleMoveEntity` | partial_handler |  |
| `0x37` | `move_minecart_along_track` | `handleMinecartAlongTrack` | missing_handler |  |
| `0x38` | `move_entity_rot` | `handleMoveEntity` | partial_handler |  |
| `0x39` | `move_vehicle` | `handleMoveVehicle` | missing_handler |  |
| `0x3a` | `open_book` | `handleOpenBook` | missing_handler |  |
| `0x3b` | `open_screen` | `handleOpenScreen` | partial_handler | Opens server-titled menu with source slot layout; basic official backgrounds and read-only slots are wired. Six layouts and all click/recipe/button semantics remain incomplete. |
| `0x3c` | `open_sign_editor` | `handleOpenSignEditor` | partial_handler | Valid sign block opens a four-line editor using source GUI textures, front/back NBT and width limits. Core Unicode cursor/selection/line navigation is wired. Clipboard, IME preedit, exact validity auto-close, full fonts and local world-text prediction remain incomplete. |
| `0x3d` | `ping` | `handlePing` | partial_handler |  |
| `0x3e` | `pong_response` | `handlePongResponse` | missing_handler |  |
| `0x3f` | `place_ghost_recipe` | `handlePlaceRecipe` | missing_handler |  |
| `0x40` | `player_abilities` | `handlePlayerAbilities` | partial_handler |  |
| `0x41` | `player_chat` | `handlePlayerChat` | partial_handler | Original packed body/filter/bound-chat-type and signature cache decode; LastSeen/ACK connected. Cryptographic chain validation and complete chat semantics are not implemented. |
| `0x42` | `player_combat_end` | `handlePlayerCombatEnd` | missing_handler |  |
| `0x43` | `player_combat_enter` | `handlePlayerCombatEnter` | missing_handler |  |
| `0x44` | `player_combat_kill` | `handlePlayerCombatKill` | missing_handler |  |
| `0x45` | `player_info_remove` | `handlePlayerInfoRemove` | partial_handler | Removes known player profiles and updates Tab/completion snapshots; entity/profile lifecycle parity remains incomplete. |
| `0x46` | `player_info_update` | `handlePlayerInfoUpdate` | partial_handler | All eight actions, profile properties and unverified chat-session keys decode transactionally. Offline Tab/player-name completion is wired; heads, LIST hearts and signed-chat key verification remain incomplete. |
| `0x47` | `player_look_at` | `handleLookAt` | missing_handler |  |
| `0x48` | `player_position` | `handleMovePlayer` | partial_handler | Relative pose/velocity correction and teleport ACK. Does not reset the global physics phase or LocalPlayer sendPosition tracker; readiness is retried against current generation. |
| `0x49` | `player_rotation` | `handleRotatePlayer` | missing_handler |  |
| `0x4a` | `recipe_book_add` | `handleRecipeBookAdd` | missing_handler |  |
| `0x4b` | `recipe_book_remove` | `handleRecipeBookRemove` | missing_handler |  |
| `0x4c` | `recipe_book_settings` | `handleRecipeBookSettings` | missing_handler |  |
| `0x4d` | `remove_entities` | `handleRemoveEntities` | partial_handler |  |
| `0x4e` | `remove_mob_effect` | `handleRemoveMobEffect` | partial_handler | Removes server effect snapshots atomically; mining immediately observes removal. All remaining vanilla side effects remain incomplete. |
| `0x4f` | `reset_score` | `handleResetScore` | partial_handler | Removes one objective score or all scores for the owner; original nullable UTF format tested against JAR-encoded bodies. |
| `0x50` | `resource_pack_pop` | `handleResourcePackPop` | missing_handler |  |
| `0x51` | `resource_pack_push` | `handleResourcePackPush` | partial_handler | Declines all server packs; no pack UI/download/apply support. |
| `0x52` | `respawn` | `handleRespawn` | partial_handler | Dimension/pose fences and attribute bases with KEEP_ATTRIBUTE_MODIFIERS. Inventory, effects and all entity-data keep semantics remain incomplete. |
| `0x53` | `rotate_head` | `handleRotateMob` | partial_handler |  |
| `0x54` | `section_blocks_update` | `handleChunkBlocksUpdate` | partial_handler |  |
| `0x55` | `select_advancements_tab` | `handleSelectAdvancementsTab` | missing_handler |  |
| `0x56` | `server_data` | `handleServerData` | missing_handler |  |
| `0x57` | `set_action_bar_text` | `setActionBarText` | partial_handler |  |
| `0x58` | `set_border_center` | `handleSetBorderCenter` | missing_handler |  |
| `0x59` | `set_border_lerp_size` | `handleSetBorderLerpSize` | missing_handler |  |
| `0x5a` | `set_border_size` | `handleSetBorderSize` | missing_handler |  |
| `0x5b` | `set_border_warning_delay` | `handleSetBorderWarningDelay` | missing_handler |  |
| `0x5c` | `set_border_warning_distance` | `handleSetBorderWarningDistance` | missing_handler |  |
| `0x5d` | `set_camera` | `handleSetCamera` | missing_handler |  |
| `0x5e` | `set_chunk_cache_center` | `handleSetChunkCacheCenter` | partial_handler | Server center retained; native ring storage/inRange/pruning remains incomplete. |
| `0x5f` | `set_chunk_cache_radius` | `handleSetChunkCacheRadius` | partial_handler | Server radius retained separately from render distance; native cache resizing remains incomplete. |
| `0x60` | `set_cursor_item` | `handleSetCursorItem` | partial_handler | Retains supported component patches, including filtered books and recursively nested container/bundle/projectile templates. Unknown components do not publish partial inventory. Client click prediction is incomplete. |
| `0x61` | `set_default_spawn_position` | `handleSetSpawn` | partial_handler | Dimension/position/yaw/pitch retained. Compass consumer remains incomplete. |
| `0x62` | `set_display_objective` | `handleSetDisplayObjective` | partial_handler | All 19 display slots stored, team-color sidebar override and general fallback. Integer LIST is wired to Tab; LIST hearts and BELOW_NAME remain unwired. |
| `0x63` | `set_entity_data` | `handleSetEntityData` | partial_handler | Original item-frame indices and supported serializer/component codecs are JAR-fixture tested. Unknown codecs are counted as unhandled; special entity metadata and all variant/animation effects remain incomplete. |
| `0x64` | `set_entity_link` | `handleEntityLinkPacket` | missing_handler |  |
| `0x65` | `set_entity_motion` | `handleSetEntityMotion` | partial_handler | LP Vec3 applies to remote entities and local velocity; version fence rejects queued old physics ticks. |
| `0x66` | `set_equipment` | `handleSetEquipment` | missing_handler |  |
| `0x67` | `set_experience` | `handleSetExperience` | partial_handler | Authoritative progress/level/total with original GUI sprites; mount/spectator HUD alternatives remain incomplete. |
| `0x68` | `set_health` | `handleSetHealth` | partial_handler |  |
| `0x69` | `set_held_slot` | `handleSetHeldSlot` | partial_handler | Valid server-selected slots update the hotbar; queued motion is fenced by player-state version. Invalid slots are ignored as in the original listener. |
| `0x6a` | `set_objective` | `handleAddObjective` | partial_handler | Add/change/remove display component, render type and optional number format. Sidebar uses original integer layout; player-list hearts and below-name rendering remain unwired. |
| `0x6b` | `set_passengers` | `handleSetEntityPassengersPacket` | missing_handler |  |
| `0x6c` | `set_player_inventory` | `handleSetPlayerInventory` | partial_handler | Maps direct player inventory indices to shared InventoryMenu and active-window slots; original armor/offhand/hotbar index order is tested. |
| `0x6d` | `set_player_team` | `handleSetPlayerTeamPacket` | partial_handler | All five operations and 26.2 parameter fields; member migration, prefix/suffix and inherited optional color feed sidebar. Visibility/collision flags stored; entity name-tag, collision and friendly-invisible effects remain unwired. |
| `0x6e` | `set_score` | `handleSetScore` | partial_handler | Authoritative signed value, optional display and blank/styled/fixed number format. Top 15 sidebar rows ordered by score and Java case-insensitive owner. Fonts, bidi and interactive text parity remain incomplete. |
| `0x6f` | `set_simulation_distance` | `handleSetSimulationDistance` | partial_handler | Initial login and update signed distance retained. Full client entity tick-range behavior remains incomplete. |
| `0x70` | `set_subtitle_text` | `setSubtitleText` | partial_handler |  |
| `0x71` | `set_time` | `handleSetTime` | partial_handler | Registry-indexed world clock states and ClientClockManager updates tested against original. Sky/day-night rendering is not wired yet. |
| `0x72` | `set_title_text` | `setTitleText` | partial_handler | Title/subtitle/actionbar/clear/timing synchronization and fades; freeze/pause edge cases still need comparison. |
| `0x73` | `set_titles_animation` | `setTitlesAnimation` | partial_handler |  |
| `0x74` | `sound_entity` | `handleSoundEntityEvent` | missing_handler |  |
| `0x75` | `sound` | `handleSoundEvent` | missing_handler |  |
| `0x76` | `start_configuration` | `handleConfigurationStart` | partial_handler |  |
| `0x77` | `stop_sound` | `handleStopSoundEvent` | missing_handler |  |
| `0x78` | `store_cookie` | `handleStoreCookie` | partial_handler |  |
| `0x79` | `system_chat` | `handleSystemChat` | partial_handler | Rich NBT component + overlay bit, bounded message history and HUD. Signed/player/disguised chat remains missing. |
| `0x7a` | `tab_list` | `handleTabListCustomisation` | partial_handler | Rich header/footer and wrapped offline Tab layout are wired; full original font and online-mode skin behavior remain incomplete. |
| `0x7b` | `tag_query` | `handleTagQueryPacket` | missing_handler |  |
| `0x7c` | `take_item_entity` | `handleTakeItemEntity` | missing_handler |  |
| `0x7d` | `teleport_entity` | `handleTeleportEntity` | partial_handler |  |
| `0x7e` | `test_instance_block_status` | `handleTestInstanceBlockStatus` | missing_handler |  |
| `0x7f` | `ticking_state` | `handleTickingState` | missing_handler |  |
| `0x80` | `ticking_step` | `handleTickingStep` | missing_handler |  |
| `0x81` | `transfer` | `handleTransfer` | missing_handler |  |
| `0x82` | `update_advancements` | `handleUpdateAdvancementsPacket` | missing_handler |  |
| `0x83` | `update_attributes` | `handleUpdateAttributes` | partial_handler | All 40 official attribute definitions and ordered modifier operations decode; local gravity, drag, friction, movement speed, jump and step affect physics. Entity scaling and all attribute-driven rendering/interaction remain incomplete. |
| `0x84` | `update_mob_effect` | `handleUpdateMobEffect` | partial_handler | Stores validated effect amplifier/duration/flags atomically; haste/conduit power/fatigue feed mining speed. Full HUD/particles and all effect consumers remain incomplete. |
| `0x85` | `update_recipes` | `handleUpdateRecipes` | missing_handler |  |
| `0x86` | `update_tags` | `handleUpdateTags` | partial_handler | Replaces tags per registry transactionally with global budgets; projectile picking consumes redirectable_projectile. Other tag consumers remain incomplete. |
| `0x87` | `projectile_power` | `handleProjectilePowerPacket` | missing_handler |  |
| `0x88` | `custom_report_details` | `handleCustomReportDetails` | missing_handler |  |
| `0x89` | `server_links` | `handleServerLinks` | missing_handler |  |
| `0x8a` | `waypoint` | `handleWaypoint` | missing_handler |  |
| `0x8b` | `clear_dialog` | `handleClearDialog` | missing_handler |  |
| `0x8c` | `show_dialog` | `handleShowDialog` | missing_handler |  |

## play / serverbound (69)

| ID | Пакет | Original handler | Состояние | Ограничение |
|---|---|---|---|---|
| `0x00` | `accept_teleportation` | `—` | emitted_partial |  |
| `0x01` | `attack` | `—` | emitted_partial | Encoder exists in entity.rs, but user attack path is not wired. |
| `0x02` | `block_entity_tag_query` | `—` | not_emitted |  |
| `0x03` | `bundle_item_selected` | `—` | not_emitted |  |
| `0x04` | `change_difficulty` | `—` | not_emitted | Original VarInt WRAP mapping and locked state retained and shown in diagnostics. All difficulty consumers remain incomplete. |
| `0x05` | `change_game_mode` | `—` | not_emitted |  |
| `0x06` | `chat_ack` | `—` | emitted_partial |  |
| `0x07` | `chat_command` | `—` | emitted_partial |  |
| `0x08` | `chat_command_signed` | `—` | not_emitted |  |
| `0x09` | `chat` | `—` | emitted_partial |  |
| `0x0a` | `chat_session_update` | `—` | not_emitted |  |
| `0x0b` | `chunk_batch_received` | `—` | emitted_partial |  |
| `0x0c` | `client_command` | `—` | not_emitted |  |
| `0x0d` | `client_tick_end` | `—` | emitted_partial | Sent after accepted local physics ticks in GUI and connect --simulate-player. Plain diagnostic connect does not run player physics. |
| `0x0e` | `client_information` | `—` | not_emitted |  |
| `0x0f` | `command_suggestion` | `—` | emitted_partial |  |
| `0x10` | `configuration_acknowledged` | `—` | emitted_partial |  |
| `0x11` | `container_button_click` | `—` | not_emitted |  |
| `0x12` | `container_click` | `—` | not_emitted |  |
| `0x13` | `container_close` | `—` | emitted_partial | Server close clears the active menu regardless of supplied ID, like vanilla. Client close uses ContainerID VarInt and current menu serial; publishes local closure without requiring a server reply. Cursor return/drop reconciliation remains incomplete. |
| `0x14` | `container_slot_state_changed` | `—` | not_emitted |  |
| `0x15` | `cookie_response` | `—` | emitted_partial |  |
| `0x16` | `custom_payload` | `—` | not_emitted | minecraft:brand is decoded; other channels retain diagnostics. |
| `0x17` | `debug_subscription_request` | `—` | not_emitted |  |
| `0x18` | `edit_book` | `—` | not_emitted |  |
| `0x19` | `entity_tag_query` | `—` | not_emitted |  |
| `0x1a` | `interact` | `—` | emitted_partial | Encoder exists in entity.rs, but user interaction path is not wired. |
| `0x1b` | `jigsaw_generate` | `—` | not_emitted |  |
| `0x1c` | `keep_alive` | `—` | emitted_partial |  |
| `0x1d` | `lock_difficulty` | `—` | not_emitted |  |
| `0x1e` | `move_player_pos` | `—` | emitted_partial |  |
| `0x1f` | `move_player_pos_rot` | `—` | emitted_partial |  |
| `0x20` | `move_player_rot` | `—` | emitted_partial |  |
| `0x21` | `move_player_status_only` | `—` | emitted_partial |  |
| `0x22` | `move_vehicle` | `—` | not_emitted |  |
| `0x23` | `paddle_boat` | `—` | not_emitted |  |
| `0x24` | `pick_item_from_block` | `—` | not_emitted |  |
| `0x25` | `pick_item_from_entity` | `—` | not_emitted |  |
| `0x26` | `ping_request` | `—` | not_emitted |  |
| `0x27` | `place_recipe` | `—` | not_emitted |  |
| `0x28` | `player_abilities` | `—` | emitted_partial |  |
| `0x29` | `player_action` | `—` | emitted_partial | F/Q drops and LMB START/ABORT/STOP destruction are wired with current slot/input and world/teleport fences. Client-tick progress uses source hardness, server tool/tags/attributes/effects. Local block prediction, crack overlays and full item-specific mining semantics remain incomplete. |
| `0x2a` | `player_command` | `—` | emitted_partial |  |
| `0x2b` | `player_input` | `—` | emitted_partial |  |
| `0x2c` | `player_loaded` | `—` | emitted_partial | Sent once after a versioned GUI/world readiness request. Actual graphical path not re-run after WindowServer crash. |
| `0x2d` | `pong` | `—` | emitted_partial |  |
| `0x2e` | `recipe_book_change_settings` | `—` | not_emitted |  |
| `0x2f` | `recipe_book_seen_recipe` | `—` | not_emitted |  |
| `0x30` | `rename_item` | `—` | not_emitted |  |
| `0x31` | `resource_pack` | `—` | emitted_partial |  |
| `0x32` | `seen_advancements` | `—` | not_emitted |  |
| `0x33` | `select_trade` | `—` | not_emitted |  |
| `0x34` | `set_beacon` | `—` | not_emitted |  |
| `0x35` | `set_carried_item` | `—` | emitted_partial |  |
| `0x36` | `set_command_block` | `—` | not_emitted |  |
| `0x37` | `set_command_minecart` | `—` | not_emitted |  |
| `0x38` | `set_creative_mode_slot` | `—` | not_emitted |  |
| `0x39` | `set_game_rule` | `—` | not_emitted |  |
| `0x3a` | `set_jigsaw_block` | `—` | not_emitted |  |
| `0x3b` | `set_structure_block` | `—` | not_emitted |  |
| `0x3c` | `set_test_block` | `—` | not_emitted |  |
| `0x3d` | `sign_update` | `—` | emitted_partial | Four raw lines + side + packed position, source wire codec verified. Worker validates current world and server editor serial/side/position, sends once and publishes closure. Full GUI parity remains incomplete. |
| `0x3e` | `spectator_action` | `—` | not_emitted |  |
| `0x3f` | `swing` | `—` | emitted_partial | Main-hand swing follows attack and user click misses/blocks; full hand animation and mining remain incomplete. |
| `0x40` | `teleport_to_entity` | `—` | not_emitted |  |
| `0x41` | `test_instance_block_action` | `—` | not_emitted |  |
| `0x42` | `use_item_on` | `—` | emitted_partial | Main-hand block clicks/hold are wired; current slot/input are flushed first and stale teleport/world targets dropped. Baseline entity-priority picking is wired; special stateful dimensions, interaction-shape override, prediction, item-specific logic and container state remain incomplete. |
| `0x43` | `use_item` | `—` | emitted_partial | RMB in air emits source hand/sequence/yaw/pitch fields, including an offhand when the main hand is empty. Slot/input sync and stale-generation rejection are TCP-tested. Full use-duration, release and interaction-result fallback semantics remain incomplete. |
| `0x44` | `custom_click_action` | `—` | not_emitted |  |
