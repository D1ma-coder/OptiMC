# Двухэтапная загрузка внешнего мира, 23 октября 2026

Источник — официальный client.jar 26.2, protocol 776, SHA-1
`2dc72797acbc1b63fc16a11c4ac393605f453754`.
Сверены ClientPacketListener.handleGameEvent/tick/notifyPlayerLoaded,
LevelRenderer.isSectionCompiledAndVisible (camera blockPos, section visibility .3),
LevelLoadTracker и его WaitingForServer/WaitingForPlayerChunk/ClientState.
CPU helper `tools/reference/ExportLevelLoading.java` вызывает настоящий codec
и переходы tracker с null player/level; timeout short-circuit позволяет проверить
готовность без Minecraft instance, сервера и GPU. Fixtures сохранены в
`data/26.2/level-loading.json`, hash в reference.json проверяется unit test.

## Изменения

- `game_event` 0x26, action 13 LEVEL_CHUNKS_LOAD_START переводит tracker из
  WaitingForServer в WaitingForPlayerChunk. Float parameter оригиналом не
  используется, включая NaN; source packet с таким значением тоже TCP-tested.
- Login/respawn начинают новую загрузку, reconfiguration сбрасывает tracker.
  Повторный marker не сбрасывает уже готовую секцию и не начинает новый timeout.
- Ранняя готовность геометрии не заменяет server marker. Callback разрешается
  только после получения соответствующего snapshot; его token хранится отдельно
  от одноразовой очереди snapshots. Старые world/teleport tokens отвергаются.
- После marker: compiled section, spectator, dead player, player/camera outside
  build height или истечение исходных 30 000 ms позволяют завершить загрузку.
  Timeout строго `>`, а не `>=`, в целых миллисекундах. До marker timeout не
  действует, как в исходном WaitingForServer.
- Worker отправляет ServerboundPlayerLoaded 0x2c ровно один раз. GUI переводится
  в Playing по подтверждённому worker состоянию, включая исключительные пути.
- Готовность GPU-сцены сверяет epoch, dimension и Arc текущей колонки под камерой.
  Телепорт внутри неизменившейся колонки не требует лишнего remesh; заменённые
  chunk data и геометрия предыдущего мира не считаются готовыми.
- Headless diagnostic повторяет notification до готовности, если первый вызов
  пришёл до marker. Geometry/camera reports coalesce; compiled=true остаётся
  установленным для одной generation, подобно исходному AtomicBoolean.

## Проверки и ограничения

239 library tests (22 ignored), 17 localhost TCP tests (2 GPU ignored), Clippy
all-targets -D warnings и fmt проходят. Новый delayed-marker TCP сценарий
проверяет ранний callback, неготовность до marker, устаревший epoch и единственный
player_loaded. Существующие movement, actions, inventory и bundles тесты также
проходят. CPU-тест проверяет column identity при коррекциях позиции.

Original helper проверяет сам codec, отсутствие callback до marker, появление
callback после marker, duplicate identity, reset и timeout. Ветви spectator/dead/
outside-height сверены по байткоду и проверены native truth-table тестами;
выполнение этих ветвей внутри оригинального LocalPlayer с живым миром не запускалось.

Графический путь ещё консервативнее исходного: ожидает завершения pending mesh и
texture upload всей подготовленной сцены, а не только секции под камерой. Исходный LevelRenderer также требует
  section visibility >= .3: нативная анимация появления секций ещё не перенесена. Отдельный
per-section GPU readiness callback ещё нужен. Camera height берётся из текущей
камеры предыдущего frame/poll; отдельный финальный smoothing sample не передаётся.
Независимая worker проверка готовности пока не привязана к исходной фазе client tick.
Death определяется по синхронизированному health; полная death/removed lifecycle
ещё требует переноса. Эти ограничения не скрываются заявлением о полной parity.

Окно, GPU и публичные серверы не запускались после WindowServer инцидента.
Другие game_event действия (погода, звуки, credits, respawn/crafting options)
сохраняют явную диагностику. Общая цель остаётся активной.

Для следующего этапа сохранены `.cache/protocol-audit/level-renderer-readiness-26.2.txt`
и `render-section-readiness-26.2.txt`: callback проверяется после uploadTerrainBuffersToGpu,
а не просто после CPU meshing. Нужно разделить готовность/загрузку отдельных секций.

Release `.cache/OptiMC.app`; executable SHA-256 `daeba0c042089142e66e732800c1fc62488fb5b83719607657549b86b7d292b3`.
Backup `.cache/OptiMC-before-level-loading`; release/cmp/plist подтверждены.
