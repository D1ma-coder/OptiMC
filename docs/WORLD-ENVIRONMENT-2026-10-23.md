# Состояние мира и серверные настройки, 23 октября 2026

Последующее дополнение: LEVEL_CHUNKS_LOAD_START и tracker загрузки перенесены
в [следующем цикле](LEVEL-LOADING-2026-10-23.md); ниже описан первоначальный checkpoint.

Источник: локальный официальный client.jar 26.2, protocol 776,
SHA-1 `2dc72797acbc1b63fc16a11c4ac393605f453754`.
CPU-only helper `tools/reference/ExportWorldEnvironment.java` вызывает исходные
StreamCodec, ClientClockManager и GameType.updatePlayerAbilities. Результат
`data/26.2/world-environment.json.gz`, SHA-1 распакованных данных проверяется тестами.

## Реализовано в этом цикле

- Play `set_time` 0x71: gameTime и карта clock states (VarLong totalTicks,
  partialTick/rate). Holder IDs разрешаются по серверному world_clock registry;
  порядок deliberately reversed в fixture. ClockManager advance, отрицательные
  скорости, остановка, неполные обновления, long wrapping и точность больших
  значений воспроизведены. Изменения атомарны при ошибке декодирования.
- `set_default_spawn_position` 0x61: dimension, packed position, yaw и pitch.
  Wire constructor сохраняет углы, в отличие от фабрики RespawnData.of.
- `set_chunk_cache_center` 0x5e и `set_chunk_cache_radius` 0x5f: серверное состояние
  хранится отдельно от пользовательской дальности отрисовки.
- `change_difficulty` 0x0a: VarInt Difficulty с исходным WRAP mapping и locked.
  `set_simulation_distance` 0x6f: signed VarInt без выдуманного ограничения.
  Начальная дистанция также берётся из Play login.
- Configuration 0x01 / Play 0x18: minecraft:brand. Неизвестные каналы сохраняют
  unhandled diagnostic. Brand хранится в Arc, без глубокого clone каждого snapshot.
- `game_event` 0x26, только CHANGE_GAME_MODE (action 3): исходный
  floor(parameter + .5), GameType.byId ZERO fallback, previous mode и способности
  creative/spectator. Политика flying соответствует GameType, скорости сохраняются;
  последующий abilities packet по-прежнему авторитетен. Начальные login/respawn
  режимы тоже применяют способности.
- Исправлена маршрутизация block_changed_ack: 0x04 вместо ошибочного 0x05.
  Source codec ACK 37 проходит TCP, а настоящий по формату block_destruction 0x05
  не попадает в ACK handler и не перезаписывает sequence. Разрушение/трещины
  других игроков и полная block prediction reconciliation ещё не перенесены.
- F3 показывает полученные brand, clocks, spawn, difficulty, simulation distance,
  chunk center/radius. Это дополнительные диагностические строки, не заявление
  о полном совпадении layout F3 с оригиналом.

## Проверка

235 library tests прошли (22 отдельно помеченных теста не запускались).
Четыре environment tests проверяют 5 последовательных ClockManager updates,
224 GameType/abilities cases, 24 difficulty packets, 7 simulation packets,
spawn/cache/brand, повреждённые и укороченные тела.
Локальный TCP fixture передаёт исходные bytes через настоящий SessionMachine;
проверяет состояние, mode transition и отсутствие unhandled для этих обработчиков.
16 локальных TCP-тестов, Clippy all-targets -D warnings, fmt check и release
прошли. Binary cmp и plist lint подтвердили упаковку `.cache/OptiMC.app`.
Executable SHA-256: `cc73447d8e1f17170f550ea2794d603bce676ffda8455f856fc83bb5e7ad1219`.
Графические тесты, запуск приложения и подключение к публичным серверам не выполняются
согласно `GPU-INCIDENT-2026-10-20.md`.

## Что ещё не перенесено

Солнце/небо пока не используют clock timeline: световое направление всё ещё
фиксировано. World spawn ещё не управляет компасом. Chunk cache пока не имеет
полной native ring/inRange/pruning логики. Simulation distance хранится, но полное
ограничение client entity ticking по ней ещё не воспроизведено. Для difficulty
не заявляется завершённая клиентская логика hunger/локальных эффектов.

Остальные game_event действия остаются явно unhandled; в новом executable
диагностика дополнена action и parameter. LEVEL_CHUNKS_LOAD_START в оригинале
переходит из WaitingForServer в WaitingForPlayerChunk: нужен перенос tracker
с compiled-section callback, spectator/dead/outside-height условиями и timeout;
просто принять пакет без этого поведения было бы неверно. Журнал соединения
сохраняет метаданные, а не тела пакетов, поэтому старый лог `0x26` не позволяет
доказать, какое действие прислал сервер.

Из последнего пользовательского лога отдельно остаются enabled features,
recipes/book, world border, ticking state/step, waypoint, server data,
equipment rendering, particles/sounds, sign editor, passengers, advancements,
damage/level/entity events и недостающие metadata/animate actions. Часть packet
families уже имеет частичные обработчики; отсутствие одного diagnostic не
является доказательством полного переноса. Ошибка packet ID для block ack исправлена в этом цикле. Полная цель остаётся незавершённой.
