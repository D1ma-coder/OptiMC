# Состояние проекта

23 октября, open_sign_editor: добавлена рабочая цепочка серверного редактора —
четыре строки, front/back NBT, 24 официальные GUI textures для normal/hanging
табличек, исходные ограничения ширины и core Unicode editing. Done/Escape
кодируют SignUpdate 0x3d без нормализации пробелов. Worker проверяет разрешение,
side/position/serial/epoch, отправляет один раз и публикует закрытие без server echo.
[Источники, проверки и ограничения](SIGN-EDITOR-2026-10-23.md).

Также исправлен общий NBT ListTag 26.2: scalar wrappers внутри heterogeneous
списков теперь снимаются, а literal empty-key compounds корректно экранируются.
Это устранило потерю plain «Front» на исходной табличке рядом с bold «Привет».
Source NBT cases, parser и writer сверены по байтам JAR, а не по предположению.

266 проверок проходят: 246 library (22 ignored), 18 localhost TCP (2 GPU ignored),
1 CPU atlas и 1 CPU sign mesh. Clippy all-targets -D warnings, fmt, release,
cmp и plist OK. GPU/окно/публичные серверы не запускались.
Release `.cache/OptiMC.app`, executable SHA-256:
`1068925c4b4095cbc8a84e0a299a6e231ed263eb0e621f7843d2aeb2453c2610`.
Backup `.cache/OptiMC-before-sign-editor`.

Clipboard/IME preedit, полные font/bidi semantics, auto-close distance и local
world sign prediction ещё требуют переноса. Визуальное освещение/погода из
предыдущего этапа пока не подключены; остальные пакеты нового лога остаются
в работе. Полная цель не завершена, server/singleplayer не интегрируются.

23 октября, свет/погода: light_update 0x30 и световой хвост chunk 0x2d теперь
декодируются и сохраняют sky/block arrays, empty masks, partial patches и padding
sections. Снапшоты разделяют Arc-колонки/секции; unload/new world освобождают
соответствующие данные. Weather game events 1/2/7/8 сохраняют rain/thunder level
по исходному listener/setters. Полученные значения видны в F3.
[Источники, проверки и ограничения](LIGHT-WEATHER-2026-10-23.md).

243 library (22 ignored), 17 localhost TCP (2 GPU ignored), Clippy all-targets
-D warnings, fmt, release, cmp и plist — OK. Проверены 4096 исходных nibble
samples, source packet codecs, partial/empty masks, повреждённые тела,
snapshot isolation и 28 weather cases. GPU/окно/публичные серверы не запускались.
Release `.cache/OptiMC.app`, executable SHA-256:
`406a871b815dcc8fc876a129df696809feba69fe34436af009cd6ec391051cef`.
Backup `.cache/OptiMC-before-light-state`.

Это перенос сетевого состояния: lightmap/shaders/AO, локальная propagation,
neighbor dirtying и визуальная погода ещё не подключены. Decoder пока строже
оригинала для коротких surplus arrays; это отмечено в документе этапа.
Начатая проверка respawn выявила дополнительные ошибки reset/keep semantics,
но их исправление отложено после нового пользовательского лога о свете.
Passengers/advancements/неподдержанные metadata и полная цель остаются в работе.

23 октября, game_event / загрузка: action 13 LEVEL_CHUNKS_LOAD_START управляет
двухэтапным tracker. До server marker callback геометрии не завершает загрузку;
после него действуют compiled column, spectator/dead/outside-height и исходный
30-секундный timeout. PlayerLoaded отправляется один раз, tokens старого мира/
телепорта отвергаются. Camera column сравнивается по identity данных, поэтому
коррекция позиции в неизменившемся чанке не требует remesh.
[Исходники JAR, тесты и ограничения](LEVEL-LOADING-2026-10-23.md).

239 library (22 ignored), 17 localhost TCP (2 GPU ignored), Clippy all-targets
-D warnings, fmt, release, cmp и plist — OK. Включая delayed-marker сценарий,
ранний/старый/повторный callback и source NaN parameter, который action 13
игнорирует. Окно/GPU и публичные серверы не запускались.
Release `.cache/OptiMC.app`, executable SHA-256:
`daeba0c042089142e66e732800c1fc62488fb5b83719607657549b86b7d292b3`.
Backup `.cache/OptiMC-before-level-loading`. Полная цель остаётся активной.

Ещё требуется per-section GPU readiness вместо ожидания всей сцены и исходная
анимация появления/visibility >= .3, финальный smoothing sample камеры,
привязка tracker tick к фазе client tick и полная death lifecycle. Другие
действия game_event, рецепты, пассажиры, equipment и остальные протокольные
пропуски из предыдущего checkpoint не объявляются завершёнными.

23 октября, серверное состояние мира: добавлены 26.2 world-clock updates с
серверными registry IDs, default spawn, chunk cache center/radius, difficulty
и locked, simulation distance, minecraft:brand. CHANGE_GAME_MODE применяет
исходное округление и GameType abilities, включая начальный login/respawn режим.
F3 показывает полученные параметры. Исправлен неверный ID block_changed_ack:
0x04 вместо 0x05; TCP regression отличает ACK от block_destruction.
[Источники, проверки и оставшиеся части](WORLD-ENVIRONMENT-2026-10-23.md).

235 library (22 ignored), 16 localhost TCP (2 GPU ignored) проходят; Clippy
all-targets -D warnings, fmt check, release, binary cmp и plist OK. Пять Naga
проверок прошли до последних изменений только Rust networking/diagnostics;
шейдеры с тех пор не менялись. Окно/GPU и публичные серверы не запускались.
Release `.cache/OptiMC.app`, executable SHA-256:
`cc73447d8e1f17170f550ea2794d603bce676ffda8455f856fc83bb5e7ad1219`.
Backup `.cache/OptiMC-before-world-environment`.

Полная цель не завершена: clocks ещё не управляют sky/day-night, spawn —
компасом; native chunk-cache ring и simulation tick range ещё не перенесены.
Остальные game_event действия остаются unhandled с action/parameter diagnostic.
Новый лог также подтверждает незавершённые recipes, equipment, passengers,
particles/sound, entity events, signs editor и другие системы. Аудит всего
реестра обновлён в `protocol-776-coverage.json`; partial handler не означает
полного совпадения с оригиналом. Старый журнал без packet bodies не позволяет
восстановить конкретный action у сообщения game_event.

22 октября, сундуки: добавлен block-entity renderer с исходными ChestModel
слоями/PNG для single/double, trapped, ender и copper/waxed chests. Block event
0x07/event=1 синхронизирует крышку по viewer count с проверкой block identity;
max открытия двух половин, исходная геометрия/UV, ориентация и cubic easing.
Движение продолжается между пакетами; неподвижные меши/слои переиспользуются.
Состояние очищается вместе с block entity при замене/выгрузке блока/колонки.
[Исходники JAR, проверки, ограничения](CHESTS-2026-10-22.md).

264 проверки проходят: 231 library, 11 world CPU, 1 epoch CPU, 16 localhost TCP,
5 Naga. В том числе 132 source geometry/orientation cases, 12 source packet
bytes и live TCP viewer event. Clippy/fmt/release/cmp/plist OK; GPU/окно и
публичные серверы не запускались. Шалкеры, звуки, Christmas override и полный
lightmap ещё не завершены. Из последнего лога время/точка появления/cache center/
game events/custom payload также остаются в работе; обработчики не подменены NOP.
Release `.cache/OptiMC.app`, executable SHA-256:
`4b97a4ee0b271ae56b5f4d5285f6ad37f56ffbbec861040745cbda99c766f589`.
Backup `.cache/OptiMC-before-chest-renderer`. Полная цель не завершена.

22 октября, Map cache limit exceeded: удалён наш hard limit 256 карт,
отсутствующий в ClientLevel оригинала. Все ID и частичные патчи сохраняются;
пустые карты делят immutable массив. На смене world_epoch пересоздаётся мешер
и освобождается прежний текстурный атлас. Localhost TCP принял 1025 карт без
разрыва; CPU-тест проверил очистку атласа только при смене эпохи. Публичный
сервер/GPU не запускались. Предел GPU atlas отдельно ещё не решён paging-ом.

Animate 0x02: кодек, состояние основных/вторых взмахов, стандартная WHACK-поза
частей HumanoidModel, зеркальная рука по metadata, базовые эффекты длительности.
272 source poses и 24 source packet bytes сверены с JAR; понадобилась sine table
Mth. Полные equipment-dependent timings, player/специальные entity animations,
wake-up/crit particles и остальные пакеты последнего лога ещё не завершены.
[Источники, проверки и ограничения](MAPS-ANIMATION-2026-10-22.md).

263 проверки проходят: 228 library, 10 world CPU, 1 epoch CPU, 16 localhost TCP,
5 Naga, 3 icon CPU. Clippy/fmt/release/cmp/plist OK. Графические тесты не запускались.
Release `.cache/OptiMC.app`, executable SHA-256:
`3fd7bb6933c96eda76a5a6880e9899bccd91e468e640245d25ca6c5c2061127a`.
Backup `.cache/OptiMC-before-map-lifetime-fix`. Полная цель не завершена.

22 октября, оптимизация подготовки сцены/кадра: повторные entity updates больше
не пересобирают terrain stamps/portal graph при тех же Arc-колонках. GPU mesh reuse
индексирован вместо квадратичного поиска; диапазоны LOD разделяют Arc. Draw lists
непрозрачных/прозрачных мешей собираются одним проходом, сохраняют capacity и
сортируют прозрачность по заранее вычисленному расстоянию с прежним tie order.
CPU benchmark: 421 колонка/842 пустые секции, 200 обновлений — 600,73 → 20,06 мс
суммарно (~3,00 → 0,10 мс на обновление участка). Это не измерение игрового FPS.

В эту сборку включены накопленные клики/drag/горячие клавиши инвентаря с
серверными state/serial fences, визуальное предсказание storage menus, иконки
голов с profile textures и gamma/alpha-correct mipmaps. Исправлена дополнительная
проверка occupancy чанков, отсутствующая в LevelChunkSection.read оригинального
JAR. Публичный сервер и визуальный результат не проверялись. Animate/passengers/
advancements, все metadata codecs и полный HashOps prediction ещё не завершены.
[Изменения, источники, измерения и ограничения](OPTIMIZATION-2026-10-22.md).

258 проверок проходят: 224 library, 9 world CPU (включая benchmark), 3 icon CPU,
5 Naga, 16 localhost TCP, 1 held geometry CPU. Clippy all-targets -D warnings,
fmt, release compile, binary cmp и plist — OK. Sandbox linker cache предупреждение
не помешало сборке. GPU/окно не запускались; ограничение после WindowServer сбоя
сохранено, пользовательские настройки/процессы не менялись.
Release `.cache/OptiMC.app`, executable SHA-256:
`ed48ae33e757ff32ac3c38b05980e22907ba0402253ca2ff80b6d313f28cf4ad`.
Backup `.cache/OptiMC-before-render-cpu-optimization`. Полная цель не завершена.

22 октября, повёрнутые текстуры/шейдеры: отделены Vertex.tint и shade. Убрана
ошибочная компенсация направленного затемнения (перепутанные X/Z и зависимость
от lod==0) в Medium/High; Low сохраняет прежние коэффициенты. UV-поворот не
меняет нормаль/освещение. LOD merge сохраняет shade boundaries; формат вершин
синхронизирован между Rust/WGSL и instancing. Сверены локальный BSL10.1.8 и
upstream Bliss Stable; полный перенос паков этим не заявляется.
229 проверок проходят: 214 library, 5 Naga, 8 world CPU, held geometry/projection.
Clippy/fmt/release/plist OK. [Подробности](SHADING-2026-10-22.md).
Окно/GPU и внешние серверы не запускались; визуальный результат не подтверждён.
Изменения tear-free presentation и ровного FPS limiter сохранены; на MoltenVK
без Mailbox остаётся FIFO fallback с пределом частоты экрана.
Release `.cache/OptiMC.app`, SHA-256 executable:
`f6253b3621834468ad86dc6c75394bd616d7a0b9318d4ac7e33c467a2021e1f4`.
Backup `.cache/OptiMC-before-shade-separation`. Общая цель активна.

22 октября, вывод кадров: убран fallback на Immediate при VSync Off. Выбирается
Mailbox, а при его отсутствии — FIFO, с явным указанием ограничения частотой
экрана в Graphics и заголовке. Unlimited FPS без разрывов на данном MoltenVK
этим не реализован. Исправлено накопление дрейфа дедлайнов FPS; 210 library tests,
Clippy all-targets -D warnings, fmt, release/plist проходят. Визуальный результат
и плавность GPU не измерялись; графические тесты после системного сбоя не запускались.
[Изменения, источники и ограничения](PRESENTATION-2026-10-22.md).
Обновлён `.cache/OptiMC.app`, backup `.cache/OptiMC-before-presentation`.
Executable SHA-256:
`96a69c9eaac98802f20bf5f22d7fc61e52570904dda3231cd0be2eb187f7151f`.
Пользовательский процесс и настройки не менялись. Общая цель активна.

22 октября, серверные меню и ввод: разблокирована E в сетевой игре; добавлены
custom_model_data, профили голов, potion contents/scale и entity/block-entity
components. Эталонное меню из JAR больше не отклоняется и сохраняет предметы
в меню/хотбаре. Пустая рука анимируется при ударе в воздух; ПКМ запускает базовый
взмах взаимодействия. 229 целевых проверок проходят, Clippy/fmt/release/plist — OK.
[Область изменений, проверка и ограничения](MENU-ITEMS-2026-10-22.md).
Все component codecs и отдельные use animations ещё не завершены; проверка
данного снимка на живом сервере не проводилась. Общая цель остаётся активной.
Release `.cache/OptiMC.app`, SHA-256 executable:
`0c1b35549a95817c0b307e0429d2f3228f87b8e89590147f37caebe9a8e5bfdc`.
Backup `.cache/OptiMC-before-menu-items`; окно/GPU не запускались, пользовательский
процесс не останавливался, settings/accounts/server list не менялись.

22 октября, предметы от первого лица: обычные модели основного выбранного
стека и offhand подключены из серверного инвентаря. Официальные display transforms,
alpha-contour extrusion, переключение модели и базовый взмах; отдельные worker,
кеш, материал и first-person depth pass. Исправлены расхождения ItemTransform/Mth.sin
по эталонам JAR. 237 проверок: 203 library + 16 TCP + 5 Naga + 8 world CPU geometry
+ 2 icons + 1 atlas + 1 held contours/models + 1 CPU projection. Clippy/fmt/release/
plist проходят. CPU-превью компаса просмотрено; окно/GPU не запускались.
[Полная область изменений и оставшиеся ветки](HELD-ITEMS-2026-10-22.md).
Карты/специальные модели, динамические свойства, use animations, точная подсветка
и экипировка третьего лица пока не завершены. Общая цель активна.
Release `.cache/OptiMC.app`, executable SHA-256:
`017a4cef4c93a619dd54a87097e8b031d3a971c9ec5d90173341516ebb31ef05`.
Backup `.cache/OptiMC-before-held-items`; пользовательское приложение не
останавливалось/не перезапускалось, settings/profiles/server list не менялись.


22 октября, ЛКМ/ПКМ и хотбар: ПКМ в воздухе теперь отправляет UseItem;
ЛКМ по блокам — START/ABORT/STOP с тиковым расчётом добычи. Источник параметров —
32366 официальных block states плюс серверные tool/tags/attributes/effects.
Ввод и слот синхронизируются до действий, world/teleport fences проверены.
Исправлены range_dispatch GUI-иконки компаса/часов и положение/затухание названия
предмета над HUD. 232 проверки проходят: 200 library + 16 TCP loopback +
5 Naga + 8 CPU geometry + 2 CPU icons + 1 CPU atlas; Clippy/fmt/release/plist зелёные.
[Подробности и ограничения](MOUSE-INPUT-2026-10-22.md).
Также завершена текущая проверка чтения серверного инвентаря и nested components:
[INVENTORY-2026-10-22.md](INVENTORY-2026-10-22.md).
Полное block prediction/cracks, item-use/release/fallback, held-item render,
container clicks и все остальные требования общей цели ещё не завершены.
Окно/GPU не запускались; внешние серверы в этом цикле не использовались.
Release `.cache/OptiMC.app`, SHA-256 executable:
`9f2ca456f62a3bff0b520f75317f3d06a859a213c7ead6a4d79b5e666c078352`.
Backup `.cache/OptiMC-before-mouse-actions`; текущий пользовательский процесс
не останавливался и не перезапускался. Settings/profiles/server list не менялись.
Общая цель активна.


22 октября, уточнение общего LOD: сохраняемая крутилка в настройках графики
(Авто → ручное качество → Выкл), исходная геометрия плюс четыре дальних уровня,
плавные shader tiers. Расчёт при зуме теперь использует перспективное фокусное
расстояние; Off остаётся отключённым при любом зуме. 219 проверок проходят:
190 library, 14 TCP loopback, 5 Naga, 8 CPU geometry, 1 CPU item icons,
1 CPU GUI atlas. Clippy/fmt/release/plist проходят. GPU/окно не запускались,
визуальная проверка и замеры FPS не выполнены.
Release `.cache/OptiMC.app`, SHA-256 executable:
`13c515f43f3ff4fda816f7bcf38e0455f0ff60dbfb5cd923d6b256fad4f0e5a8`.
Backup `.cache/OptiMC-before-lod-refinement`; настройки/профили не менялись.
Подробности [LOD-2026-10-22.md](LOD-2026-10-22.md). Общая цель активна.
Сборка также включает текущую незавершённую работу над серверным инвентарём:
содержимое, слоты и базовые окна читаются, F/Q отправляются штатными пакетами;
перенос предметов в серверных окнах и добыча блоков ещё не реализованы полностью.
Полная синхронизация всех компонентов предметов не достигнута.


22 октября, picking сущностей: общий Entity/Block raycast, рамки map/non-map,
small/marker-стойки, poses/scale; ЛКМ attack+swing и ПКМ interact подключены к
пользовательскому вводу. Добавлены серверные tags в Configuration/Play,
UUID/teleport/world fencing действий. 213 проверок проходят, Clippy/fmt зелёные.
Полная entity/item/mining parity не достигнута; ограничения и следующий объём:
[ENTITY-PICKING-2026-10-22.md](ENTITY-PICKING-2026-10-22.md). Общая цель активна.
Release упакован без запуска в `.cache/OptiMC.app`; executable совпадает с
target/release/optimc, SHA-256
`11a1ebd12e3cccbb61c60a79029283283da77da119f1316925f81676b777caaa`.
Backup `.cache/OptiMC-before-entity-picking`; пользовательские settings/profiles/
servers не перезаписывались. Следующие существенные пробелы: динамические
размеры/attachments, inventory/tool/item ATTACK_RANGE, добыча и контейнеры.

22 октября, взаимодействия: подключены выбор блоков по отдельным официальным
outline shapes, ПКМ главной рукой/удержание, selected-slot/input ordering,
sequence и приём block ACK. 205 CPU/TCP/Naga/geometry проверок проходят.
Это первая интеграция использования; entity precedence, добыча, предметы,
prediction и контейнеры ещё не завершены. Подробности и ограничения:
[BLOCK-USE-2026-10-22.md](BLOCK-USE-2026-10-22.md). Общая цель активна.
Release собран и упакован без запуска в `.cache/OptiMC.app`; SHA-256 совпадает
с target/release/optimc:
`bc766522a932827fd8eadd2bb55b62748e9b770ff7da37de77575bef4d074532`.
Backup executable `.cache/OptiMC-before-block-use`; настройки, профили и список
серверов не менялись. Следующий приоритет — entity precedence/picking перед
действием, затем attack/mining и авторитетные inventory/tool/effect параметры.

22 октября: добавлен общий LOD slider в настройках графики (Авто → ручное
качество → Выкл), четыре дальних уровня серверной геометрии и шейдерных
эффектов, hysteresis и сохранение детализации при зуме. Проверки: 189 CPU/Naga/
resource geometry passed; Clippy/fmt проходят. Область применения и ограничения:
[LOD-2026-10-22.md](LOD-2026-10-22.md). GPU/FPS пока не проверены.
Release обновлён в `.cache/OptiMC.app` без запуска; SHA-256 executable:
`e2329f9fb94314739417a78c6aa9fc561bb53c44e5764dc9c4187facc193ae71`.
Backup: `.cache/OptiMC-before-multilevel-lod`. Пользовательские настройки,
профили и список серверов не перезаписывались.

Новая контрольная точка 22 октября: частичный нативный перенос Zoomify,
3d-Skin-Layers, Sodium и mapping-ядра ViaVersion по скачанным JAR.
Подробности: [MOD-PORTS-2026-10-22.md](MOD-PORTS-2026-10-22.md).
Release упакован в `.cache/OptiMC.app` без запуска; SHA-256 executable
`ed236d77a238385c55b5a77cf1b19bd5837c4348d5f30562a524c9519b54cb88`.
Предыдущий executable: `.cache/OptiMC-before-mod-ports`.

Последняя контрольная точка: исправлены локальная точность рамок/карт, серверные позы и флаги стоек; добавлены данные и текст табличек, базовый интерактивный чат и Tab. Надпись Java Edition убрана. Доказательства и явные ограничения — [ENTITY-TEXT-AUDIT-2026-10-21.md](ENTITY-TEXT-AUDIT-2026-10-21.md). Полный перенос, экипировка и действия добычи/атаки/использования ещё не завершены; цель остаётся активной. GPU не запускался.

Последняя контрольная точка, 21 октября: исправлен runtime-разбор числа секций по высоте измерения; подключены живые рамки/карты и 90 базовых моделей сущностей. Подробные доказательства и ограничения — [LIVE-WORLD-2026-10-21.md](LIVE-WORLD-2026-10-21.md). Проверены 149 library + 12 TCP + 5 WGSL = 166 тестов, отдельно 4 теста официальной геометрии. Графическая проверка после WindowServer incident не выполнялась. Отказ антибота и полный перенос сущностей/протокола не завершены. Разделы ниже сохраняют историю предыдущих контрольных точек.

Контрольная точка: 19 октября 2026. Общая цель — полноценный независимый клиент Minecraft Java 26.2 на Rust/Vulkan — **ещё не достигнута**.

## Реализовано и проверено

- Cargo-проект Rust + WGSL, lockfile, MIT для собственного кода, CI-конфигурация Windows/Linux/macOS.
- Vulkan-only runtime через wgpu 24; на этом Mac используется MoltenVK 1.4.2. Устройство: Apple M2 Pro, backend `Vulkan`.
- Создание окна AppKit, swapchain, depth buffer, текстурный массив с mipmaps, opaque pipeline, свободная камера, ввод и debug LOD.
- Официальный манифест Mojang, проверка SHA-1 метаданных и JAR, проверка `version.json` (`26.2`, `776`), извлечение шести текстур. Импорт и online sync проверены.
- Процедурный демонстрационный мир, greedy meshing, halo face culling.
- Quadtree LOD с hysteresis, балансировкой соседей 2:1 и skirts; покрытие включает требуемый радиус при пересечении границ корня и при отрицательных координатах.
- Асинхронное построение, Rayon, ограниченные очереди, переиспользование CPU/GPU-мешей, порционная загрузка и атомарное переключение набора; frustum/backface culling.
- Собственные VarInt/string/framing/zlib codec с ограничениями размеров; offline UUID; TCP status и diagnostic offline session.
- Обмен login → configuration → play, cookies, plugin refusal, known packs, keepalive, ping, pack decline, teleports (absolute/relative), chunk batch acknowledgements, configuration restart.
- Декодер секций чанков именно 26.2: heightmap map, block+fluid counts, single/indirect/global palettes, фиксированное количество packed longs без старого префикса длины.
- 21 тест прошёл: 18 модульных, 2 интеграционных с TCP-fixture, 1 Naga validation. `cargo clippy --all-targets -- -D warnings` прошёл. Интеграционным тестам в Codex sandbox потребовался разрешённый запуск с loopback-доступом.
- Реальный запуск Vulkan и сохранение кадра: `.cache/vulkan-demo.png`. Изображение проверено визуально. Команда воспроизведения: `cargo run -- demo --radius 512 --screenshot .cache/vulkan-demo.png`.

Первичная CPU-проверка debug-сборки, radius=512: 304 тайла, 194 494 треугольника, 17.06 МиБ данных мешей, около 690 мс на холодное построение. Это не FPS и не показатель release-производительности. Повторные неизменившиеся тайлы кэшируются.

## Следующие необходимые этапы

1. Registry data / bounded network NBT; dimension type, min_y/height, game login и respawn. Сопоставление global block-state IDs 776 с официальными моделями/материалами. Не использовать таблицы 26.1 как якобы совместимые с 26.2.
2. Единое состояние мира: хранение/удаление чанков, block/section/biome updates, смена измерений, освещение. Сетевые события должны поступать в графический клиент вместо отдельной diagnostic session. Не генерировать вымышленные чанки для серверного режима.
3. Полный загрузчик официальных blockstates/models/textures: model inheritance, variants/multipart, rotation, UV, transparency, biome tint, animated textures и texture cache. Шесть текущих текстур недостаточны для всего Minecraft.
4. Игрок: server-authoritative movement, physics, collision, abilities, game modes, tick/input packets, inventory/hotbar, breaking/placing, basic menus и disconnect/reconnect. Телепортная диагностика пока не является игровым управлением.
5. Сущности, block entities, эффекты, GUI/чат, рецепты, звуки и прочие игровые системы. Нужен набор реальных тестов offline-mode сервера 26.2; текущий TCP-fixture не доказывает полной совместимости.
6. Серверный LOD: persistent disk cache реально полученных чанков, версия формата, инвалидирование при изменениях, multi-level reduction для геометрии, ограничение RAM/VRAM/disk и scheduler. Текущий surface LOD подходит для демо, не сохраняет пещеры/строения вдали. За пределами server view distance доступна только ранее сохранённая информация.
7. Оптимизация на измеренных сценариях: occlusion, indirect draws/batching, allocator/buffer pools, GPU frame timing, stress tests; дистрибутивы и реальные GPU smoke tests на Windows/Linux/macOS.

Не помечать общую цель выполненной после работающего демо или успешного login. Не называть `connect` полноценной поддержкой игры на сервере. Код Minecraft не копировать в проект; JDK не добавлять в runtime. Временные исследования официального JAR через javap использовались только при разработке.

## Release-проверка этой контрольной точки

`cargo build --release --offline --locked` завершился успешно. Последовательные одиночные замеры CPU на M2 Pro (холодное построение в каждом новом процессе):

| Radius | Тайлы | Треугольники | Данные мешей | Время |
|---|---:|---:|---:|---:|
| 1024 | 388 | 248 254 | 21.78 МиБ | 28.84 мс |
| 4096 | 574 | 367 294 | 32.23 МиБ | 33.41 мс |

Это локальные единичные измерения, не гарантии производительности и не FPS. Неизменившиеся тайлы при стриминге переиспользуются. Собранные нативные бинарные файлы: `target/debug/optimc`, `target/release/optimc`.

Окно тестового screenshot-запуска закрылось штатно после записи PNG. `cargo fmt --all -- --check` прошёл. В транзитивной зависимости `block 0.1.6` остаётся предупреждение Rust о будущей несовместимости; оно не препятствует текущей сборке/проверкам. Предупреждения xcrun в ограниченном sandbox о системном каталоге кэша не помешали линковке.

## Исправление UV, главное меню и настройки графики

По следующему запросу исправлена ориентация текстур: боковые грани использовали одинаковую схему UV для разных осей greedy sweep. Теперь V на всех четырёх вертикальных сторонах равен `-worldY`, U идёт по горизонтальной оси. Нормали и winding сохраняются; тест проверяет траву сверху на всех четырёх сторонах и исключает поворот текстуры на 90°. Проверен близкий Vulkan-кадр `.cache/uv-fixed-low.png`.

Добавлены:

- Главное меню с официальной панорамой, логотипом, кнопками, bitmap-шрифтом и кириллицей; меню паузы и путь «Настройки → Настройки графики». GUI реализована непосредственно на Rust/WGSL. Одинаковая геометрия интерфейса повторно не загружается на GPU. В главном меню с доступной панорамой мир не генерируется и не мешится до входа в игру.
- «Одиночная игра» открывает существующий демонстрационный мир. «Сетевая игра» открывает ввод адреса и настоящий server-status query. Игровое отображение сервера по-прежнему не готово, Realms выключен.
- VSync меняет actual swapchain present mode (Fifo / Immediate / Mailbox, с сообщением о fallback), FPS-ползунок управляет реальным frame pacing. Настройки сохраняются в `.cache/settings.json` и применяются сразу. Пользовательские настройки не сбрасываются при пересборке.
- Low: классический вид. Medium: shadow map 2048, PCF, направленный свет/sky ambient, атмосфера/облака, HDR/ACES, небольшой bloom. High: shadow map 4096, больше PCF-сэмплов и октав облаков, шесть шагов объёмного света через shadow map, усиленный bloom и виньетка. Это авторские эффекты с ориентиром на BSL/Bliss, не порт этих шейдерпаков. Воды/отражений и полноценных объёмных облаков пока нет.
- Дополнительные CLI: обычный запуск открывает меню; `menu --graphics`, `--quality low|medium|high`, `demo --closeup`, `demo --frames N`. Последний измеряет FPS после готовности мешей. `OPTIMC_CACHE` позволяет изолировать тестовую конфигурацию.

Проверки: исправленные UV, Low/Medium/High и главное меню реально отрисованы на Apple M2 Pro через Vulkan/MoltenVK. PNG: `.cache/uv-fixed-low.png`, `.cache/medium.png`, `.cache/high.png`, `.cache/main-menu.png`, `.cache/graphics-settings.png`. WGSL-модули terrain/shadow, sky, post и GUI проверяются Naga. Исправлена несовместимость динамически индексированного локального массива fullscreen-triangle с текущим SPIR-V → MSL конвертером: координаты теперь вычисляются из vertex index.

В отдельной конфигурации с лимитом 30 FPS и выключенным VSync измерено **29.20 FPS за 60 готовых кадров**, фактический mode **Immediate**. Это одиночная проверка ограничителя, не максимум производительности. GPU-тест реальных обработчиков GUI переключил VSync, установил 60 FPS через ползунок, выбрал High, сохранил/загрузил настройки и вернулся на Low после смены размера/DPI. Пользовательское окно для этих проверок не закрывалось; отдельный процесс OptiMC QA завершён.

Общая исходная цель остаётся активной. Следующее основное направление — registry/NBT и отображение настоящего серверного мира. Загрузка зависимостей официального генератора справочных отчётов в `/tmp` была начата до этого пользовательского запроса и закончилась сетевой ошибкой на commons-lang3; её можно продолжить по `/tmp/optimc-fetch-reference.py`. В проекте и runtime Java-зависимостей не появилось.

Итог этой итерации: **28 тестов прошли**, включая отдельно требующий GPU тест (запуск `cargo test --all-targets -- --include-ignored` с доступом к MoltenVK). `cargo fmt --check`, Clippy с `-D warnings` и итоговая release-сборка прошли. `.cache/OptiMC.app` обновлён атомарно бинарным файлом `target/release/optimc`; уже открытый пользовательский процесс не прерывался. Для загрузки последней сборки приложение нужно перезапустить.

## Игрок, полный ресурсный каталог, LOD и масштабирование (следующая итерация)

Общая цель остаётся незавершённой. Официальный ресурсный каталог скачан полностью: 5057 объектов asset index (около 459 МиБ) с SHA-1, плюс 19 494 ресурсных/data-файла из JAR (14.2 МиБ). Данные лежат в `26.2/resources`, объекты — в общем cache/objects. Наличие всех файлов **не означает**, что движок уже умеет отрисовывать все модели, воспроизводить звуки или реализует все механики.

Реализованы локальные демонстрационные:

- Инвентарь с официальным inventory.png, hotbar/selection/crosshair, официальными предметами и Steve. Block icons строятся из официальных текстур граней. 36 основных слотов, offhand, перенос/объединение, ПКМ половина/один предмет, Shift+клик, сохранение предмета на курсоре при невозможности вернуть его в слот. Armor/crafting output пока неактивны. Только девять демонстрационных типов предметов; рецептов и серверной синхронизации пока нет.
- WASD, Space прыжок, Shift красться, Ctrl бег, 1–9/колесо hotbar, E инвентарь, F offhand, F5 первый/задний/передний вид, F1 HUD, F11 borderless fullscreen. Физика сухих полных блоков: фиксированные 20 Hz, f64, swept AABB, ступень 0.6, защита края при приседании, прыжок/drag/gravity, camera collision. Отдельные тесты проверяют высоту прыжка, скорость, независимость от частоты кадров, стены/потолок и камеру.
- Кубическая модель игрока по UV официального Steve, анимация ходьбы, рука первого лица и модель в инвентаре. Пока нет внешнего слоя скина, предмета в руке, полного набора анимаций, плавной смены поз, жидкостей, лестниц, транспорта, эффектов и всех вариантов физики. Формулы bobView сверены с 26.2 GameRenderer, настройка покачивания доступна; полная идентичность поведения пока не заявляется.
- GUI Scale воспроизводит Window.calculateScale и setGuiScale из локального официального 26.2 JAR: Auto, целые масштабы, минимум 320×240, округление размеров вверх; координаты мыши учитывают framebuffer/GUI, включая Retina. Проверены Auto для 640×480, 1440×900, 1920×1080, 2880×1800, 3840×2160 и некратных размеров. Unicode-even ветка алгоритма также проверена; полной загрузки всех Unicode-провайдеров шрифта пока нет.
- Рендер: одна непрерывная крутилка **HD → разрешение экрана**, значение 0…1000, по умолчанию максимум. Пропорции сохраняются: 16:9 даёт минимум 1280×720, 16:10 — 1280×800. Верхняя граница вписывает пропорции окна в физические размеры текущего монитора и лимит GPU. При экране меньше HD используется доступное разрешение. Мир/depth/HDR масштабируются отдельно от нативного GUI. Переезд между мониторами и resize обновляют размеры, F11 включает полный экран.
- LOD: полноценные блоковые грани минимум до 384 вместо 192 блоков, далее шаги выборки 1/2/4/8/16…; 2:1 баланс и hysteresis сохранены. Mesh cache зависит от Tile, а не переходящей границы расстояния. Общие вершины heightfield снижают память без удаления выборок; фактические bounds поверхности улучшают frustum culling при взгляде вверх. Тайлы вне радиуса не мешатся/не загружаются. Это по-прежнему LOD демонстрационной поверхности; серверные пещеры, здания и persistent chunk cache ещё впереди.
- При захвате мыши используется прозрачный CustomCursor вместе с cursor hidden/locked; отпускание и потеря фокуса восстанавливают обычный курсор. Это закрывает путь появления стрелки при сбросе cursor rectangles AppKit. Длительная визуальная проверка отсутствия каждого вспыхивания пока не выполнена.
- Low имеет отдельные простые fragment entry points; при совпадении размеров сцены и окна пишет сразу в swapchain, при другом размере проходит через масштабируемую offscreen-сцену. Frustum planes извлекаются один раз на проход. GPU/CPU профилирование включается OPTIMC_PROFILE=1; OPTIMC_BENCH_SKY=1 позволяет воспроизвести взгляд вверх, OPTIMC_FORCE_HDR=1 — сравнить offscreen-путь.

Производительность: в отдельных замерах sky/Low без ограничения FPS основной CPU интервал оказался внутри queue.submit (порядка 3–8 мс), подготовка команд — около 0.02–0.08 мс. Пробы давали заметно разные FPS (примерно 133–288), поэтому стабильное исправление жалобы «~230 FPS» **ещё не доказано**. Удаление HDR само по себе не дало воспроизводимого улучшения. Эти результаты нельзя сравнивать с первоначальной геометрией или обещать как конечную производительность. Эксперимент MVK_CONFIG_SYNCHRONOUS_QUEUE_SUBMITS=0 не включён в runtime.

Проверено 42 теста (36 library, 2 network, 4 WGSL), включая настоящий Vulkan-тест GUI: VSync, FPS, quality, GUI resize, края ползунка разрешения и перенос половины стопки в инвентаре. Native PNG: `.cache/inventory.png`, `.cache/third-person.png`, `.cache/graphics-scaled.png`; после добавления render slider сделаны отдельные кадры HD/native. Нативная release-сборка и Clippy проверяются после последнего изменения; актуальное состояние — в результатах команд, старые таблицы выше относятся к предыдущей геометрии.

Финальная проверка этой итерации: release и Clippy `-D warnings` прошли. После изменения захвата slider повторно прошёл Vulkan GUI test; после отсечения тайлов за радиусом повторно прошли все 6 LOD tests. Полный набор до этих локальных правок прошёл 42/42. Нижняя граница настоящего scene attachment: **1280×800**, верхняя: **3456×2160**, output в обоих случаях **1440×900**; различие формата окна и монитора обрабатывается пропорционально. Native кадры `.cache/render-hd.png`, `.cache/render-native.png`, обновлённый `.cache/graphics-scaled.png` осмотрены. Приложение `.cache/OptiMC.app` обновлено атомарно; пользовательская `.cache/settings.json` сохранена без изменений.

## 20 октября: клиент без одиночной игры, официальные модели и сущности

Новая формулировка цели пользователя: полный клиент с оптимизацией, всеми сущностями и взаимодействиями; без встроенного сервера и без одиночной игры. Удалены кнопка одиночной игры и её Action из product UI. CLI demo/model остаются разработческими инструментами. Текущее соответствие и незакрытые требования записаны в `docs/COVERAGE.md`.

Добавлен `assets::models`: безопасные ResourceId, model inheritance, texture aliases/material objects (`sprite`, `force_translucent`), variants/weight selection, multipart AND/OR/negated alternatives, default UV, UV rotation/uvlock, face/cull direction, element rotation single-axis и Euler XYZ. Rescale сверён с CuboidRotation 26.2: normalize по max-abs компоненте каждого повёрнутого basis vector, post-multiply; простой 1/cos недостаточен для больших углов и Euler. Готовые грани хранятся в общем Arc cache, повторный запрос модели/квадранта/uvlock не пересобирает геометрию.

`assets audit-models` проходит **1198 blockstates, 2657 моделей, 8661 вариант, 86486 граней, 1230 texture refs**, включая 580 force-translucent quads и 2040 element rotations в разрешённых моделях. Проверка нашла реальный edge case `heavy_core`: face slot `all` допустим без `#`; обработка исправлена по TextureSlots/MaterialBaker клиента.

Разработческий `model` рендерит официальную геометрию в Vulkan. PNG `.cache/model-stairs.png`, `.cache/model-fence.png`, `.cache/model-glass.png` осмотрены: stairs, multipart fence и alpha-blended stained glass. Дополнительные изображения получают отдельные texture-array layers; translucent quads рисуются после opaque, с сортировкой и без depth write. Preview использует первый animation frame, демонстрационный tint; cullface/light emission/AO ещё не интегрированы в полный серверный мир. Наличие всех JSON не означает готовность всех специальных block/entity renderers.

Добавлены `net::vector` и `net::entity`: LpVec3, появление/тип/UUID/поза/скорость/head angle/data, short-delta movement с Java-round semantics, motion/head/position sync/relative teleport/remove. Диагностическая сессия теперь сохраняет Tracker и возвращает SessionReport. TCP/compression fixture присылает 2 spawn, относительное движение, голову/скорость и удаление; тест проверяет оставшийся entity pose. На смене мира/configuration tracker очищается. Тип сущности пока числовой registry ID — полноценного dispatch по моделям ещё нет.

LP wire format и goldens проверены выполнением оригинального LpVec3.class в отдельном `/tmp/optimc-lp-reference` harness (Java **не добавлена** в client/runtime). Эталонные байты включают zero, обычные скорости и ±17179869183 с unsigned continuation VarInt. В 26.2 `ServerboundAttackPacket` отдельный (0x01), `ServerboundInteractPacket` (0x1a) содержит hand + LpVec3 hit + secondary flag; сериализаторы реализованы и протестированы. Они пока не вызываются из игрового окна.

Итог проверки 20 октября: **59 тестов прошли** (53 library, 2 TCP, 4 WGSL), включая полный официальный каталог и реальный Vulkan GUI. `cargo fmt --check`, Clippy `-D warnings` и release-сборка прошли. После обновления rescale повторно прошли все тесты. Главное меню без одиночной игры отрисовано и осмотрено в `.cache/main-menu.png`. `.cache/OptiMC.app` обновлено атомарно; пользовательские настройки не переписаны. Общая цель остаётся активной: главное незавершённое звено — registry/NBT + живой world state/renderer, затем визуальные модели и metadata всех сущностей.

## 20 октября: NBT, реестры, измерения и клиентское хранилище чанков

Получены эталонные отчёты разовым запуском `net.minecraft.data.Main --reports` из проверенного официального JAR с 34 библиотеками, прошедшими SHA-1. Это только инструмент разработки, без `--server`, без встроенного сервера и без Java в runtime. В `data/26.2` встроены gzip-отчёты (~291 КиБ): **95 реестров, 158 entity types, 1537 item IDs, 1196 блоков / 32366 block states**. SHA-1 несжатых данных проверяются загрузчиком; OnceLock разбирает таблицы один раз, имена блоков переиспользуются через Arc. Все state ID/properties дополнительно проверены по official blockstate-dispatch. Учтены законные пустые multipart-комбинации (стена без столба и всех сторон), а не ошибочно требуются видимые грани у каждого state.

`net::nbt`: все стандартные payload types 0…12, named/file и unnamed/network roots, Java modified UTF-8, NUL и UTF-16 surrogate units без потери данных. Сверено поведение глубины с NbtAccounter: считаются только контейнеры, максимум 512. Memory/node budget может быть общим для всех entries одного registry packet; размеры массивов проверяются до выделения, writer ограничен по выходу. Проверены отрицательные/огромные размеры, неправильные типы, глубина и общий бюджет.

`net::registry` сохраняет configuration registry_data в packet order, optional NBT и lookup имени/ID. Dimension core получает min_y, height/logical_height, skylight/ceiling; остальные NBT-поля остаются сохранены в registry. `net::spawn` читает login/respawn/CommonPlayerSpawnInfo точно по 26.2: direct VarInt dimension holder ID (не +1), dimension key, seed, game mode/previous mode, flags, death location, portal cooldown, sea level, online/secure-chat flags.

`world::remote::RemoteWorld` — клиентское хранилище полученных чанков, не серверная симуляция. Проверяется число секций текущего измерения, поддержаны отрицательные координаты, одиночный block update, batch section update (VarLong + X/Z/Y layout) и chunk unload. Uniform sections хранят одно значение вместо 4096; Dense создаётся при первом отличающемся обновлении. Same-dimension respawn сохраняет world chunks, смена измерения очищает их. SessionReport возвращает registries + RemoteWorld; entity type names сопоставляются с официальными static IDs.

TCP fixture прошёл цепочку registry → finish configuration → play login → 2 chunks/24 sections → unload + single/batch block updates + entity movement. Проверяются min_y=-64, тип creeper по ID 32, содержимое блоков после updates, один оставшийся chunk и compact uniform storage. Это доказательство network/state path, **не готового graphical multiplayer**: UI по-прежнему показывает server status, live render/input linkage, block entities/light, metadata/equipment и визуальные модели всех сущностей ещё не подключены.

Итог: **75 тестов прошли** (69 library, 2 TCP, 4 WGSL), включая Vulkan GUI и полный resource/state audit. Clippy с `-D warnings` и release-сборка прошли. После переименования клиентского хранилища в RemoteWorld все targets проверены Clippy и пересобран release; переименование не меняло поведение. Изменений 3D/GUI в этом этапе не было, поэтому прежние Vulkan screenshots повторно не снимались. Общая цель остаётся активной.

## 20 октября: фоновая сессия, потоковый framing и снимки мира

Общий SessionMachine используется диагностическим клиентом через `net::live::Connection`. DNS/connect/handshake и обработка пакетов вынесены в именованный network worker. Потребитель получает статус и только последний Arc<Snapshot>, без очереди устаревших миров; частота публикации ограничена 20 Гц. Отмена закрывает сокет через shutdown, без join в вызывающем потоке; уже выполняющийся DNS lookup ОС остаётся фоновым до завершения. Диагностическая CLI-команда отдельно дожидается конечного снимка до одной секунды.

FrameBuffer хранит неполные VarInt/body между чтениями с timeout 10 мс. Compression threshold применяется к каждому следующему пакету после обработки предыдущего, в том числе когда оба уже в одном TCP read. Проверяется размер до выделения памяти, ограничен общий буфер. Пакеты разбираются порциями до 256 / ориентир 5 мс между проверками отмены (один decode может занять больше). Нет повторного чтения через read_exact с коротким тайм-аутом, которое теряло бы уже прочитанные байты.

RemoteWorld/columns/sections используют copy-on-write: снимок копирует карту Arc-ссылок, а не массивы блоков всех чанков. Изменение Dense копирует только изменяемую секцию; остальные секции остаются общими. Повторное присваивание того же state ID не копирует даже column. Проверены неизменность старого снимка после обновления/выгрузки и идентичность Arc нетронутых секций.

Исправлена обработка player teleport: весь пакет валидируется до подтверждения и изменения позы; применяются те же PositionMoveRotation-relative flags, включая скорость и ROTATE_DELTA, что и у entity teleport. Snapshot включает feet position, server yaw/pitch, velocity и счётчик принятых коррекций. Проверяются truncation на каждой границе, лишние байты и NaN velocity.

Полная проверка: **79 тестов прошли** (72 library, 3 TCP, 4 WGSL), включая официальный каталог и Vulkan GUI. Новый TCP fixture посылает фрагменты с паузами 40 мс, меняет compression, проверяет keepalive/config/play/teleport snapshot и отмену посреди незавершённого VarInt с EOF на другой стороне. Clippy `-D warnings` прошёл. Это готовый фоновый сетевой путь для диагностического клиента и API для окна; graphical multiplayer/meshing server chunks, отправка управления и визуализация всех сущностей всё ещё требуют интеграции. Общая цель не завершена.

Release-сборка этапа фоновой сессии завершена; `.cache/OptiMC.app/Contents/MacOS/OptiMC` заменён атомарно, SHA-256 совпадает с `target/release/optimc`. Пользовательские настройки не менялись, запущенные окна не закрывались. Изменений визуального рендера в этом этапе не было; GUI проверен существующим Vulkan-тестом.

## 20 октября: геометрия серверных секций и сквозной Vulkan-снимок

`world::remote_mesh::RemoteMesher` сопоставляет серверные state IDs с официальным blockstate dispatch, запекает варианты/multipart и использует официальные PNG/material flags. Части и текстуры переиспользуются; слои texture array стабильны, начинаются с 7 после базовых текстур/скина. Проверена подготовка всех **32366 состояний: 6812 различных частей, 1228 sprites**. В эту цифру не входят неиспользуемые статическими state IDs ресурсы и особые renderer paths. Анимационные PNG пока используют первый квадратный кадр; tint/освещение пока демонстрационные.

Для скрытия граней вычисляется консервативная маска полных непрозрачных unit-square faces. Она учитывает все возможные взвешенные варианты и не считает cutout/blended/force-translucent геометрию непрозрачной. Culling работает через границы чанков и вертикальных секций; отсутствующие соседи не создают геометрию. Пустые uniform air sections обходятся без 4096 итераций, полностью скрытые части с cullface не добавляют вершины. Точные voxel occlusion shapes, особые block rules и greedy merging server-моделей ещё не реализованы.

Кэш секции зависит от Arc её данных и шести соседей. Проверено повторное использование тех же Arc<Mesh>, перестроение только затронутых зависимостей, появление/выгрузка соседей и очистка при смене измерения. Это пока вызов обновления по требованию, не живой meshing worker оконного клиента. Opaque/cutout и blended геометрия разделены; blended имеет одну mesh на секцию, внутренняя сортировка отдельных граней пока отсутствует.

Добавлены position seed с Java wrapping int/long, 48-bit SingleThreadedRandomSource, unbiased bounded nextInt и nextLong с signed lower half. Сверены оригинальные `Mth`, `ModelBlockRenderer`, `WeightedVariants`, `WeightedList`, `MultiPartModel`, `BitRandomSource`. Multipart получает nextLong и сбрасывает seed перед каждой выбранной частью. Goldens получены запуском оригинальных Mth/SingleThreadedRandomSource через временный Java harness в `/tmp`, в том числе на ±30 млн и i32 limits; Java в runtime не добавлена. Специфичные block getSeed overrides/offsets ещё требуют отдельного переноса.

Разработческий `connect --screenshot PATH` после диагностической сессии отключается и передаёт полученный RemoteWorld в оконный Vulkan renderer. Это просмотр снимка, **не полноценная живая игра**. Для крупных наборов материалов device limit texture-array запрашивается под фактическое число слоёв; превышение adapter limit выдаёт ошибку, paging ещё требуется. Камера автоматически охватывает видимую геометрию.

Сквозной тест получает настоящий framed/compressed chunk fixture, unload/single/section updates, затем вызывает отдельный клиентский процесс и сохраняет `.cache/server-world.png` (1440×900). Кадр осмотрен: серверный каменный объём и два обновлённых блока над ним; demo terrain отсутствует. Другой тест проверяет 14 внешних граней у трёх соприкасающихся блоков через x/y boundaries и 11 граней stone+glass (stone не исчезает за стеклом).

Итог: **83 теста прошли** (75 library, 4 TCP/сквозных, 4 WGSL), включая все ignored asset/GPU tests. Clippy `-D warnings` и fmt прошли. Главное незавершённое звено — постоянная связь live Connection → mesh worker → игровое окно и ввод в обратную сторону; затем полное освещение/жидкости/сущности/инвентарь и остальные пункты COVERAGE. Общая цель активна.

Release-сборка server meshing прошла; `.cache/OptiMC.app` обновлено атомарно, SHA-256 бинарников совпадает. Сохранённые пользовательские настройки не менялись, пользовательские окна не закрывались.

## 20 октября: живое сетевое окно и вход из меню

Добавлен `render::remote::RemoteSession`: Connection сохраняет network ownership; отдельный `server-mesh` worker получает только последний снимок, строит секции и отдаёт последний результат. Геометрия не пересобирается на keepalive/повороты сущностей, если Arc-чанки и измерение не изменились. Pixel buffer переиспользуется через Arc до появления новых материалов. При отмене worker будится через Condvar, проверяет остановку между секциями, UI не выполняет join.

Vulkan texture array резервирует до min(adapter limit, 2048) слоёв (плюс проверка достаточности стартовой сцены); GPU-память массива используется для последующего streaming. Новые слои с mipmaps загружаются порциями с ориентиром 3 мс/кадр перед загрузкой зависимых meshes. Старые meshes остаются видимы до завершения нового набора. При новом соединении layer numbering начинается с 7, прежние meshes/очереди убираются. На GPU с меньшим лимитом сохраняется явная ошибка вместо invalid texture indexing; paging ещё не реализован.

SessionMachine/Snapshot получили world_epoch и position_ready. Login/respawn/reconfiguration инвалидируют готовность позиции и поколение мира; окно отбрасывает старую геометрию по epoch. До показа мира требуется подтверждённая серверная позиция, загруженный в отображаемом наборе чанк под игроком и завершённые GPU uploads. Проверка наличия колонки не зависит от высоты Y, чтобы допустимая позиция над build height не застревала в загрузке. Camera feet→eye добавляет 1.62, yaw/pitch переводятся в систему renderer; demo camera movement и demo terrain worker requests отключены при активном remote connection.

В Multiplayer добавлены имя ASCII 1…16, переключение поля Tab, вход кнопкой/Enter, сохранена проверка status. Экран загрузки показывает этап соединения и отмену; Esc также отменяет. Выход в главное меню закрывает Connection и mesher. Хотбар очищается от demo items. Серверные позиции отображаются, мышью можно осматриваться; отправка ввода/rotation и физика полученного мира ещё не подключены. Сетевой E пока не открывает локальный demo inventory, чтобы не показывать неподтверждённые сервером предметы. Metadata/entity models, реальный inventory protocol, interaction и остальной COVERAGE остаются незавершёнными.

Новый CLI `play SERVER --username NAME [--frames N --screenshot PATH]` использует тот же оконный путь. Сквозной fixture после появления мира через 300 мс меняет блок на oak stairs с новым материалом, проверяет keepalive 999, затем EOF при выходе клиента. Тест требует несколько live scene updates и появление четвёртой текстуры (одного начального и пустого снимка недостаточно), проверяет отсутствие demo mesh requests и PNG 1440×900 `.cache/server-live.png`. Кадр осмотрен: серверный каменный пол, crosshair и пустой хотбар. Vulkan GUI test дополнен проверкой Connect(address,username), Connecting, Cancel и Esc.

Полный прогон: **84 теста прошли** (75 library, 5 TCP/сквозных, 4 WGSL), включая ignored GPU/assets. После уточнения готовности чанка по X/Z повторяется сетевой прогон. Clippy с `-D warnings` прошёл. Общая цель не достигнута: ближайшее обязательное звено — отправка управления, синхронизация физики/abilities и полное отображение сущностей.

Повторный сетевой прогон после последнего изменения прошёл (5/5); fmt и Clippy прошли. Release собран, `.cache/OptiMC.app` обновлено атомарно, SHA-256 совпадает с `target/release/optimc`. Пользовательские настройки не менялись и уже запущенные окна не закрывались.

## 20 октября: исходящее управление и физика серверных форм

Из проверенного official JAR разовым development harness экспортированы collision AABB, friction/speed/jump factors и dynamic flag всех **32366 состояний**. Встроенный `collision.json.gz` занимает **10098 байт**, SHA-1 несжатых данных проверяется, 326 shape-наборов разделены через Arc. Контекст `EmptyBlockGetter / ZERO / CollisionContext.empty()` сохранён в отчёте; 199 dynamic rows отмечены явно. Harness только регистрирует данные, не запускает сервер, не добавляет Java в runtime. Способ воспроизведения описан в `data/26.2/README.md`.

CollisionWorld расширен box visitor; AABB clipping/перекрытие используют формы вместо единственного full-block solid. Поиск включает соседние ячейки для форм, выходящих за пределы блока (например, забор 1.5). RemoteCollision читает state ID из RemoteWorld; при ожидании незагруженного чанка выставляет временную границу движения. Применены block friction/speed/jump factors, включая ограничение friction-influenced acceleration по 26.2. Добавлена проверка диагональной опоры при Shift по Player.maybeBackOffFromEdge. Полная supporting-block/entity context/fluid/ladder/effect/moving-block/attribute physics остаётся незавершённой; базовая форма сама по себе не реализует 199 dynamic cases.

`net::movement::Sender` повторяет выбор LocalPlayer.sendPosition: displacement² > (2e-4)² либо reminder 20 ticks, отдельные Pos/PosRot/Rot/StatusOnly, on-ground/horizontal-collision bits. Только изменившиеся input bits, sprint command (26.2 actions 1/2), selected slot (short) отправляются дополнительно. Форматы сверены с LocalPlayer, Input$1, ServerboundMovePlayerPacket, PlayerCommandPacket и protocol 776 IDs. Incoming abilities (flags/fly/walk speed) валидируются и версионируются. Базовый разрешённый flight: double Space (7-tick toggle), vertical flySpeed×3 и Y drag 0.6; запрет включения при отсутствии разрешения, server-driven flying сохраняется, spectator no-clip использует текущий spawn mode. Полный mode-change/attributes/vehicles/flight parity ещё требует продолжения.

Окно вызывает callback после каждого physics tick и ставит Motion в ограниченную очередь 64 тика. Network worker единолично пишет сокет и проверяет world_epoch / teleport_version / abilities_version. Устаревшие команды отбрасываются. Принятые локальные position/velocity/rotation обновляют базу для следующих relative server corrections. Телепорт сбрасывает локальные feet/previous/velocity/interpolation, а также sender position/rotation reminder. Пауза в multiplayer продолжает физику с отпущенным вводом; demo по-прежнему имеет свою паузу. WASD, jump/sneak/sprint, mouse look, hotbar selection и F5 теперь связаны с серверным игроком. Inventory contents/actions/chat и визуальные модели остальных сущностей пока не реализованы.

Проверки: шаг на oak slab ровно 0.5 и остановка у стены на x=13.7, стояние на fence y=2.5, граница незагруженного чанка, диагональный Shift-edge, разрешённый flight/vertical speed и запрет survival flight. TCP fixture получает реальные position packets из работающего Vulkan окна, проверяет x/z и диапазон падения до stone top y=-48, keepalive во время тиков и EOF при выходе. Отдельный тест отправляет три заведомо неверные позиции с устаревшими world/teleport/abilities tags — сервер их не получает — и проверяет проход валидного Motion. Packet-selection unit test покрывает threshold/heartbeat, flags, rotation, sprint/slot/input и NaN rejection.

Итог полного прогона: **92 теста прошли** (82 library, 6 TCP/сквозных, 4 WGSL), включая ignored GPU/assets. Clippy `-D warnings` прошёл. `.cache/server-live.png` после приземления осмотрен: floor, crosshair, пустой хотбар и локальная рука. Общая цель не достигнута; обязательные следующие пробелы — entity metadata/models/interactions, inventory protocol, местная velocity/attributes, жидкости и динамические collision rules.

Release-сборка movement/collision прошла; `.cache/OptiMC.app` обновлено атомарно, SHA-256 совпадает с release binary. Настройки пользователя не переписывались; уже открытые пользовательские окна не закрывались. Для нового кода приложение нужно перезапустить.

## 20 октября, после сбоя WindowServer: сетевое меню и CPU-проверки

После пользовательского сообщения о падении рабочего стола GPU-тесты остановлены. Зафиксированы два WindowServer WATCHDOG timeout, включая тестовый optimc в графическом стеке; точная причина не установлена. Экспериментальные MVK env не сохранены в коде/настройках. Ограничение около 120 FPS при Immediate/cap=0 остаётся нерешённым. Условия дальнейшего разбора записаны в `docs/GPU-INCIDENT-2026-10-20.md`; автоматически повторять эксперименты нельзя.

Вместо прежней формы multiplayer добавлен список с оригинальными размерами header/footer (33/60), строками 36 GUI units, двумя рядами кнопок 100/74 units. Реализованы сохранение/редактирование/удаление с подтверждением, прямое подключение, двойной щелчок/Enter, стрелки, колесо и F5. Список хранится в named uncompressed NBT `servers.dat`, backup `servers.dat_old`; сохраняются дополнительные поля, включая icon/acceptTextures/hidden. Повреждённый существующий файл не перезаписывается пустым. Профиль offline-игрока сохраняется отдельно в profile.json. Статусы серверов обновляются одним последовательным worker, без login и автоматических повторных подключений; показываются MOTD, игроки и задержка. Default server icon взят из официальных ресурсов. Пользовательские favicon, LAN discovery, полная клавиатурная навигация кнопок/редактор текста и точное визуальное сравнение ещё не завершены.

Причины Configuration/Play disconnect теперь разбираются из network NBT в текст, отдельный экран поддерживает переносы и прокрутку. Перед client_information в первой Configuration отправляется штатный minecraft:brand с именем OptiMC, согласно последовательности original ClientHandshakePacketListenerImpl. Локальный TCP fixture проверяет порядок и русскоязычный текст отказа в обеих фазах.

Проведённый до остановки GPU-тестов диагностический вход на mclucky.me дошёл до Play и 9 chunk packets, затем сервер отклонил проверку антибота. Это не доказательство успешной игры: diagnostic connect не исполняет physics ticks. Проверка на живом сервере с полной штатной физикой остаётся впереди; обход защиты не добавлялся.

Дополнительно подготовлены данные 358 entity layers / 90 base living models / 87 текстур и CPU-компиляция геометрии с original transforms. Обнаруженные в boat mask неопределённые UV нормализованы с явной отметкой. Entity WGSL проверяется Naga на CPU, но live entity rendering пока не подключён. Полная цель остаётся активной и не достигнутой.

## 20 октября: форматирование, протокол, камера и локальные аккаунты

Список серверов получил favicon PNG 64×64 и оригинальные ping sprites. Число игроков имеет отдельную область; задержка в миллисекундах показывается подсказкой. RichText сохраняет JSON/network-NBT styles, inherited extra, translate/fallback и legacy §-коды от network worker до UI, включая Login/Configuration/Play disconnect. Цвет, bold, italic, underline, strikethrough и obfuscation отрисовываются GUI; полная font/bidi/interactive-component совместимость ещё не достигнута.

Аудит official JAR охватывает 256 регистраций всех фаз и 169 clientbound listener handlers. `docs/PROTOCOL-AUDIT.md` и JSON coverage различают наличие частичного обработчика от полного исполнения: текущие 44 partial_handler и 123 missing_handler **не являются полной синхронизацией**. Добавлены opaque cookies через Configuration/Play, локальная entity velocity, tick_end, отложенный player_loaded после готовности мира и ограничение bundle. SystemChat/overlay сохраняются в bounded history. Player chat, signed ACK, bossbars, scoreboard, Tab/player-info, container contents/revisions/click acknowledgement, block-entity state, chest/shulker animations и полная entity metadata/rendering остаются обязательными следующими этапами. Отказ антибота не обходится; повторные live login после ограничения IP не выполнялись.

Hotbar использует накопление дробной прокрутки и знак целого шага согласно MouseHandler/ScrollWheelHandler; Retina PixelDelta приводится к logical units. Панорама получила независимую камеру и скорость 2°/с с f64 phase. Third-person camera применяет восемь ray clips к экспортированным official VISUAL shapes; CPU golden для стены 2.0049937 сверён с Java reference. Eye height интерполируется. Остались entity-dependent collision contexts, scale/riding/sleeping и прочие camera states. SetHealth 0x68 обновляет health/food/saturation, HUD использует official half/full heart/food sprites; armor/absorption/status variants и вся анимация ещё не перенесены. F3 теперь отдельный overlay с реальными метриками вместо смены цвета LOD; это пока базовый набор, не полная копия F3 26.2.

Главное меню получило «Аккаунт: <ник>». Создание, редактирование, выбор и подтверждаемое удаление **локальных offline-профилей** сохраняются атомарно в accounts.json. Старый profile.json мигрирует как начальный профиль без изменения старого файла. Некорректный accounts.json защищён от перезаписи. Последний профиль удалить нельзя; ник проверяется как ASCII 1–16, дубликаты без учёта регистра запрещены. Выбор задаёт имя следующего соединения; изменение имени в Direct Connect обновляет активный профиль после успешного сохранения.

Для каждого профиля отдельно включаются публичные скины Ely.by **по нику, без входа, регистрации, паролей или токенов**. Источник: https://docs.ely.by/ru/skins-system.html. Фоновый worker ограничен одним запросом, устаревший результат другого ника отбрасывается; PNG/metadata имеют лимиты, есть disk cache и стандартный Steve fallback. Штатный redirect Ely.by на HTTP storage URL преобразуется в HTTPS с ограничением официальных хостов. Скин используется в голове главного меню, first-person arm и third-person body; поддерживаются slim arms, наружные слои и legacy 64×32 mirroring/alpha rules, сверенные по SkinTextureDownloader из 26.2 JAR. Плащи и скины остальных игроков пока не подключены.

Политика GPU incident остаётся в силе: после WindowServer timeout графические тесты не запускались. Mailbox-first selection улучшает платформы с Mailbox; MoltenVK на этом Mac его не предоставляет. Устранение macOS tearing/120 FPS ceiling при VSync Off **не подтверждено и остаётся открытым**. Экспериментальные асинхронные настройки MoltenVK не добавлялись в runtime. CPU GUI packing: 39 images, 1158/2048 rows. Публичная HTTPS загрузка и декодирование Ely.by скина успешно проверены отдельным CPU-тестом.

### Дополнение: серверный HUD, настройка ввода и плавная камера

Добавлен `net::hud::State` с server-authoritative experience (progress/level/total), всеми шестью операциями boss_event, rich title/subtitle/actionbar, clear/reset и server title timings. Прогресс bossbar интерполируется за 100 мс; цвет и notched overlay используют оригинальные sprites. Flags darken/music/fog сохраняются, но соответствующие эффекты ещё не подключены. Title/subtitle имеют исходные масштабы 4×/2× и серверные fade/stay timings. Подключён TCP-fixture, который проверяет доставку и применение этих пакетов через настоящий network worker. Armor/air/status effects/mount/spectator/potions, полный inventory HUD, scoreboard, player list/Tab, subtitles звуков, crosshair combat state и остальные HUD states остаются впереди. Нельзя называть весь HUD перенесённым.

Сопоставлены оригинальные LocalPlayer.tick/sendPosition, Minecraft.tick и ChunkBatchSizeCalculator. Вместо неизменного desired=16 используется оригинальная адаптивная формула 7 ms / smoothed nanos-per-chunk с clamp ×/÷3 и весом до 49. Batch finish разбирается как VarInt; исправлен также неправильный big-endian int в старом fixture. Physics movement до client_loaded больше не отправляется. CPU-тест при 30/60/120/240/1000 rendering FPS проверяет ровно 20 simulation ticks/сек. Исходящий OPTIMC_NET_TRACE теперь содержит timestamp, ID и размер без payload. **Причина конкретного отказа LuckyWorld “too many packets” без трассы живого соединения не установлена**; исправления не являются подтверждением прохождения антибота. Повторные live login не выполнялись.

Обычные VSync/FPS/GUI scale/view bobbing отделены от «Настройки OptiMC», куда перенесены proportional render resolution и Low/Medium/High. Добавлены действующие FOV 30…110, original cubic mouse sensitivity, invert X/Y, wheel sensitivity и discrete scroll. Полный набор Options 26.2 пока не реализован: инвентарь всех original OptionInstance fields и translation keys сохранён в `docs/settings-26.2-inventory.json`, а bytecode Options/OptionsScreen/VideoSettingsScreen — в `.cache/protocol-audit`. Разделы одиночной игры не добавляются.

По отдельному запросу добавлена быстрая плавная камера: presentation-only offset/orientation/FOV filter с half-life 22/10/25 ms, независимый от rendering FPS. Anchor следует за уже интерполированным игроком, поэтому walking и server-authoritative pose не получают дополнительной задержки. Тяжёлые frame stalls сбрасывают filter. Перемещение после smoothing повторно ограничивается восемью camera ray clips; устойчивое состояние не требует лишней трассировки. First-person hand использует итоговую видовую камеру. Переключатель «Плавная камера» находится в OptiMC, по умолчанию включён. Миграция старых settings добавляет defaults без перезаписи файла при старте. Регрессионные CPU-тесты покрывают crouch-like step, frame-rate invariance, angle seam, отсутствие задержки walking anchor и стену. Это намеренное дополнение к оригинальному поведению по последнему запросу пользователя.

Скин выбранного локального профиля теперь также отображается в inventory preview. Публичный Ely.by CPU-test прошёл отдельно. Последний полный CPU/TCP/Naga прогон: **130 passed**, 8 ignored (включая 2 GPU network tests, GUI GPU test и опциональные resource/network tests). Clippy `--all-targets -- -D warnings` прошёл. Графический запуск по-прежнему не выполнялся после WindowServer incident; визуальная проверка новых анимаций и GUI не подтверждена. Общая цель остаётся активной.

Release собран, `.cache/OptiMC.app/Contents/MacOS/OptiMC` обновлён атомарно без запуска; SHA-256 совпадает с target/release/optimc. Предыдущий app executable сохранён в `.cache/OptiMC.previous`. Пользовательские settings.json, servers.dat и profile.json не изменялись. Финальный CPU GUI atlas check: 63 изображения, 1172/2048 строк.

## 21 октября: отключение уже в Play — серверная гравитация и синхронизация игрока

Проверялась именно игровая фаза после получения чанков и серверных телепортов. Один прежний plain diagnostic connect прожил 12 секунд, но без физики это не воспроизводит оконную игру. Добавлен developer-only `connect --simulate-player`: те же Rust Player/RemoteCollision и исходящие Motion, 20 Hz, без окна/GPU, ввода, автоматических команд или reconnect. До исправления такой прогон был отклонён антиботом примерно через несколько секунд Play; исходящий поток составлял около 40–42 пакетов/сек при падении.

В контрольном прогоне после исправления видно конкретное значение `minecraft:gravity = 0`, присланное сервером в update_attributes 0x83. Раньше этот пакет игнорировался и игрок продолжал падать с gravity 0.08. Теперь все 40 official attributes имеют проверенные IDs/ranges/defaults; сохраняются base + упорядоченные modifier IDs/operations, диапазоны применяются после вычисления как в AttributeInstance. Локальные gravity, air_drag_modifier, friction_modifier, movement_speed, jump_strength и step_height подключены к физике; спринтовый modifier не умножается второй раз. Базы/модификаторы при respawn обрабатываются согласно KEEP_ATTRIBUTE_MODIFIERS, а старые queued motions отбрасываются по player_state_version. Применение остальных атрибутов к размерам/entity rendering/interaction остаётся дальнейшей задачей.

Исправлены точность float drag 0.98f/0.91f, раннее обнуление малой вертикальной скорости и общий horizontal-length threshold игрока. Падение 120 тиков совпадает с Java reference по IEEE bits. Teleport correction больше не обнуляет physics accumulator и не сбрасывает отдельный LocalPlayer sendPosition tracker; подтверждение телепорта остаётся штатным. Серверный set_held_slot 0x69 (VarInt) теперь меняет выбранный хотбар и версию player-state; невалидный индекс игнорируется как в оригинале.

Устранена гонка readiness: если pose изменилась между готовностью GUI и mark_loaded, отображение мира уже не препятствует повторной проверке актуального поколения. Snapshot публикует client_loaded даже без нового входящего пакета. Подтверждение по-прежнему отправляется один раз за мир; открытые пауза/настройки не сбрасываются повторной проверкой.

Контрольный public-server прогон `.cache/mclucky-idle-fixed-2026-10-21.log` остался в Play до штатного 20-секундного завершения, с 329 physics ticks и 9 chunk packets; прежнего кика в этом интервале не было. Это **не доказательство прохождения всех проверок антибота, полной авторизации, длительной игры или совместимости всех пакетов**. Никакого обхода проверок/смены brand/автоматического решения challenge не добавлено. GUI/GPU после WindowServer incident не запускались.

Оконный клиент и CLI diagnostic сохраняют bounded metadata journal в `.cache/network/session-*.json`: 256 последних packet IDs/names/sizes, этап, totals, точный максимум исходящих в sliding 1-second window с millisecond buckets и конечный результат. Тела пакетов и credentials не сохраняются; серверный disconnect в journal помечается как server_disconnect, rich reason остаётся в UI. Хранятся до 16 сессий. EOF без disconnect различается с EOF на незавершённом кадре; закрытие сервера само по себе не объявляется исправленным во всех возможных случаях. Headless CLI ждёт завершения журнала, отмена из GUI остаётся неблокирующей.

**140 CPU/TCP/Naga checks passed**, 8 ignored. TCP tests проверяют gravity=0 без ложного падения, server slot, generation fences, readiness, EOF в Play и завершённый journal. Clippy --all-targets -D warnings прошёл. Исправлен конфликт имён temporary directories параллельных account tests. Полная цель остаётся активной; scoreboard/Tab были исследованы по original JAR, но не реализованы в этом цикле, поскольку пользователь указал более срочное отключение в игре.

Release обновлён атомарно в `.cache/OptiMC.app/Contents/MacOS/OptiMC`, hash совпадает с target/release/optimc. Предыдущий executable сохранён как `.cache/OptiMC-before-network-fix`. Уже запущенное приложение не закрывалось и не перезапускалось; для загрузки нового кода нужен перезапуск пользователем. Пользовательские настройки, аккаунты и список серверов не менялись.

## 21 октября: повторная проверка реестра и исправление bundle

Повторно выполнен tools/audit_protocol.py по SHA-1 закреплённому official client.jar 26.2: 256 регистраций, 169 clientbound handlers, без расхождений Play/Configuration IDs с проверенной картой. Аудит теперь завершается ошибкой при дублирующихся phase/flow/ID или именах, неожиданном числе регистраций либо неразрешённом оригинальном listener. Это проверка полноты инвентаря, а не доказательство правильности всех codec fields/side effects. docs/protocol-776-coverage.json явно содержит full_semantic_parity_verified=false как для отчёта, так и для каждой строки. Текущий результат: 54 partial_handler, 113 missing_handler, 2 explicitly_unsupported; 32 emitted_partial, 53 not_emitted, 2 encoder_only. Полный перенос всех пакетов и обработчиков остаётся незавершённым.

Сверены оригинальные PacketBundlePacker, BundlerInfo и ClientPacketListener.handleBundlePacket. Раньше подпакеты применялись сразу, а исходящие Motion удерживались до закрытия bundle, создавая задержку и последующий пакетный всплеск. Теперь подпакеты сохраняются и применяются по порядку только после завершающего delimiter. До этого физические тики и readiness обрабатываются на последнем committed world; отдельная очередь движения из-за bundle не копится. Сохранён оригинальный предел 4096 подпакетов; добавлен предел выделения памяти 64 MiB. Terminal StartConfiguration внутри bundle отвергается как в оригинале. EOF на незавершённом bundle отличается от обычного закрытия. Malformed subpacket handlers всё ещё не являются rollback-транзакцией.

TCP regression test удерживает bundle открытым до трёх исходящих игровых тиков, затем проверяет отложенный keepalive reply и конечное серверное XP. Unit tests проверяют порядок, пустой bundle, границу 4096, terminal/invalid delimiter и memory bound до выделения памяти. EOF fixture проверяет обычный конец, незавершённый кадр и незавершённый bundle. Все **144 CPU/TCP/Naga tests passed**, 8 ignored; Clippy all-targets -D warnings прошёл. Логи: .cache/bundle-all-tests.log, .cache/bundle-clippy.log. Release собран и атомарно перенесён в app без запуска; backup .cache/OptiMC-before-bundle-fix. Пользовательские settings/accounts/servers не менялись.

Два новых оконных metadata journal показывают Play примерно 79 и 43 секунды с максимумом 41 и 42 исходящих пакета за секунду. В первом есть server disconnect. Эти сессии не содержат входящих bundle, поэтому данное исправление не объявляется причиной или устранением их отказов. Прежний 20-секундный headless прогон также не доказывает длительную совместимость. Новых подключений к LuckyWorld и графических запусков в этом цикле не было. Причина конкретного кика и полный перенос протокола остаются открытыми задачами; общая цель активна.

## 21 октября: серверный scoreboard и боковая таблица

Подключены пять ранее отсутствовавших Play handlers: reset_score 0x4f, set_display_objective 0x62, set_objective 0x6a, set_player_team 0x6d и set_score 0x6e. Отдельное Arc-состояние сохраняет objectives, render type, signed scores, optional display components и number formats, все 19 display slots, команды с parameters/membership. Поддержаны создание/изменение/удаление objectives, все пять операций команд, перенос участника между командами, сброс одного objective или всех очков владельца. Неизвестные objectives/teams не создаются обновлениями. Удаление objective очищает связанные scores и display slots. Начало нового Play login очищает scoreboard; обычный respawn его сохраняет. При disconnect/new connection GUI получает пустое состояние. Активный ник GUI синхронизируется с запрошенным именем соединения, включая запуск через CLI, чтобы выбор team-colored sidebar не использовал другой локальный профиль.

Формат Parameters взят из 26.2 record codec: displayName/prefix/suffix, VarInt visibility/collision, optional TeamColor и byte flags. NumberFormat registry dispatch поддерживает blank, styled network-NBT Style и fixed Component; значение строки переопределяет objective format, default sidebar — красный. Исходные render type и team flags сохраняются; применение collision/visibility/friendly-invisible к сущностям ещё не подключено. Tab с сердцами и подписи BELOW_NAME ещё не реализованы. Поэтому эти handlers по-прежнему отмечены partial, а не полностью перенесёнными.

Боковая панель использует owner для команды и сортировки, optional display для текста, prefix/suffix и наследуемый цвет. Скрытые владельцы с # исключаются; выбираются первые 15 строк по убыванию очков и Java CASE_INSENSITIVE_ORDER. Таблица simple case folds и 169 сравнений экспортированы development JDK 27; Unicode case folding не разворачивает ß в SS и проверен также на supplementary characters. Результат проекции кешируется по Arc-state/нику; выбор top 15 использует partial selection и сортировку только 15 элементов. Сохранены оригинальные integer anchors, высота строки 9, ширина с резервом ': ', отдельные header/body alpha по original defaults и отсутствие text shadow. Пользовательские опции backgroundForChatOnly/textBackgroundOpacity, полное font/bidi/interactive style поведение ещё не перенесены. Визуальная проверка GPU не выполнялась.

tools/reference/ExportScoreboard.java кодирует 17 тел пакетов настоящими STREAM_CODEC официального JAR, не запуская игру, сервер или GPU. Golden fixtures data/26.2/scoreboard-reference.json.gz и java-case-fold.json.gz имеют SHA-1 в reference.json. Тесты проверяют весь replay с цветами/стилями/format precedence, каждое усечение и trailing byte без изменения состояния, скрытые строки/top 15/порядок, цветной sidebar и fallback, перенос membership, duplicate objective, unknown objective и reset semantics. Дополнительные resource bounds: 256 objectives, 1024 teams, 16384 score entries и участников; строгая валидация malformed operations/enums может отличаться от отдельных original fallback paths и не объявляется полной семантической идентичностью.

TCP fixture теперь передаёт первые девять golden пакетов через реальную compression/framing/session dispatch и проверяет итоговый sidebar и styled score. **153 CPU/TCP/Naga checks passed**, 8 ignored; Clippy all-targets -D warnings прошёл. Логи .cache/scoreboard-all-tests.log и .cache/scoreboard-clippy.log. Общая coverage теперь 59 partial_handler, 108 missing_handler, 2 explicitly_unsupported; serverbound coverage пока прежняя. Весь HUD/протокол и причина конкретного кика остаются открытыми задачами; общая цель не завершена. Новых live login и GPU-запусков не было.

Release обновлён в .cache/OptiMC.app/Contents/MacOS/OptiMC атомарно без запуска и совпадает с target/release/optimc по SHA-256. Backup executable: .cache/OptiMC-before-scoreboard. Уже открытое приложение не перезапускалось; пользовательские accounts/settings/servers не менялись.

## 21 октября: официальный значок приложения, Login и причина серверного отказа

По снимку 16:39 проверен journal session-1792589942963752000-31315.json: Login, два исходящих пакета (handshake/hello), один входящий SetCompression, затем EOF через 386 мс без disconnect reason. Формат ServerboundHelloPacket сверён с JAR: UTF имени до 16 и UUID, текущий encoder соответствует. Данных, подтверждающих причину именно этого EOF, нет; не объявляем его исправленным. Автоматические повторные подключения или обход серверных проверок не добавлялись.

Найдено отдельное реальное расхождение Login: оригинальный Connection.setupCompression при отрицательном threshold удаляет компрессию, а клиент отклонял такой пакет. Теперь отрицательный порог переводит Wire обратно в None; trailing bytes отвергаются. TCP regression проверяет SetCompression(32), затем сжатый SetCompression(-1), обычный LoginSuccess и дальнейшие Configuration/Play. Этот фикс не объявляется причиной старого EOF: старый журнал содержит только размер поля, не его значение.

Journal теперь дополнительно сохраняет compression_threshold и ограниченный 4096 символами декодированный server_disconnect_reason. Это намеренное расширение прежнего metadata-only журнала: сохраняется только текст сообщения об отключении, не raw packet bodies, аккаунтные данные или токены. Текст форматированного JSON/NBT сначала проходит обычный component decoder. Проверены локальным TCP fixture rich Login disconnect -> UI error -> завершённый journal и Unicode bound в unit test.

Новый journal session-1792592309896853000-34160.json от пользовательского запуска уже содержит точную причину: вход в Play успешен, через 77575 мс сервер прислал «Вы провалили проверку. Возможно вы бот…». compression_threshold=256, максимум исходящих=44/сек. Это подтверждает серверный отказ антибота, но не устанавливает, какая конкретно проверка не прошла. Новых внешних логинов агент не делал; прочитал текущие journals и проверил активные TCP-соединения без закрытия пользовательского процесса. Пользователь сообщил о невидимых рамках с картами; отсутствие map/entity rendering может мешать видеть серверный контент, но причинная связь с антиботом пока не доказана.

Иконка приложения — неизменённая официальная icons/minecraft.icns, SHA-1 f00657542252858a721e715a2e888a9226404e35, совпадает с downloaded asset index. src/assets/icons.rs проверяет размер/hash ICNS/PNG; PNG используется как Window icon Windows/Linux, AppKit NSApplication icon устанавливается на main thread и при запуске напрямую из target/release. macOS зависимости objc2/AppKit/Foundation уже были в lockfile транзитивно; теперь объявлены напрямую для native icon API. GPU для проверки не запускался.

Rust CLI package-app собирает/обновляет .app без запуска окна/GPU: копирует официальный icon в Contents/Resources/OptiMC.icns, добавляет CFBundleIconFile и атомарно заменяет executable/Info.plist. CPU unit test проверяет копирование, plist и отказ от повреждённого asset до изменения готового пакета. Документация docs/APP-PACKAGING.md. Упаковка реального .cache/OptiMC.app выполнена, plist lint OK, icon hash совпадает, binary SHA-256 c438e1b3fe124ff11cf4cbb66b3c82307c95fe8e1359784c260c6fbb9f5ad78c совпадает с target/release. Backup .cache/OptiMC-before-icon. Уже запущенный процесс не перезапускался.

Последняя проверка: 140 library CPU tests + 11 TCP + 5 Naga = **156 passed**, 8 ignored. Clippy all-targets -D warnings прошёл; release собран. Логи .cache/icon-network-lib-tests.log, icon-network-tcp-tests.log, icon-network-tests.log (первый общий прогон до добавления нового compression fixture), icon-network-clippy.log и icon-network-release.log. Пользовательские settings/accounts/servers не менялись.

Следующий приоритет по последнему сообщению пользователя — рамки с картами. Проверено по исходникам: Tracker хранит spawn/pose, но world render сейчас подключён только к terrain и локальному avatar. set_entity_data 0x63 и map_item_data 0x33 отсутствуют; нет связки item stack/map_id/rotation -> frame mesh/map texture. Это недостающая реализация, не выключенная настройка. Оригинальный bytecode EntityDataSerializers/ItemFrameRenderer/ItemFrame/ClientboundMapItemDataPacket сохранён в .cache/protocol-audit/item-frame-reference.txt. Предварительное исследование player-info для Tab сохранено в player-info-reference.txt; код Tab ещё не добавлен. Общая цель активна, полный перенос протокола/HUD/entities и устранение серверного отказа не завершены.
# 22 октября: частичный нативный перенос четырёх модов

По локальным JAR добавлены Zoomify C+колесо и настройки, кэшированные объёмные
слои локального скина, отсечение серверных секций по графу Sodium. Отдельно
перенесены mapping v2 codecs и данные ViaVersion из ViaFabricPlus; полный
перевод протоколов и поддержка старых серверов ещё не реализованы. Подробные
границы переноса и эталоны: [MOD-PORTS-2026-10-22.md](MOD-PORTS-2026-10-22.md).
Лицензии компонентов сохранены отдельно и включены в packager.

Исправлен ID standalone block_entity_data (0x06 вместо block_event 0x07),
покрыто loopback TCP regression. 196 CPU/TCP/Naga/ресурсных проверок проходят;
Clippy all-targets -D warnings и fmt проходят. Графический запуск, внешние
серверы и FPS benchmark в этом цикле не выполнялись. Полный перенос модов,
протокола, HUD и взаимодействий остаётся активной задачей.
