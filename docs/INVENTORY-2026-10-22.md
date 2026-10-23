# Серверный инвентарь, 22 октября 2026

Источник — локальный официальный 26.2 client.jar SHA-1
`2dc72797acbc1b63fc16a11c4ac393605f453754`, protocol 776. CPU-экспорт:
`tools/reference/ExportInventory.java`, `ExportItemContents.java`. Runtime — Rust;
JVM, интегрированный сервер и графика при проверках не запускались.

Переносятся ContainerSetContent/SetSlot/SetData, OpenScreen/Close,
SetCursorItem и SetPlayerInventory. Поля читаются до изменения состояния;
поздние обновления чужого окна игнорируются. ContainerID — VarInt, включая 300.
Содержимое player InventoryMenu, активного контейнера, хотбар и offhand делят
Arc-стеки. Преобразования индексов брони/offhand/hotbar и проекция HUD проверены
по официальным пакетам. Выбор слота сохраняется при обновлении содержимого.

F и Q/Ctrl+Q отправляют штатные SWAP_ITEM_WITH_OFFHAND/DROP_ITEM/DROP_ALL_ITEMS.
Перед ними синхронизируются текущий слот и input; world/teleport fences
отбрасывают устаревшие запросы. Закрытие окна дополнительно проверяет menu serial.
После отправки close состояние публикуется локально даже без входящего ответа.
TCP-тест проверяет байты, порядок, shared slots, состояние меню и этот сценарий.

Встроены 1537 default item definitions; есть исходные translation keys, max stack
size, JSON и успешно экспортированные stream component defaults. Экспорт явно
сохраняет 44 raw и 72 JSON ошибки отдельных динамических/registry компонентов.
Это не полная привязка серверных default components. Компоненты Stack patch
и removals имеют приоритет над эталонными default значениями.

Поддержаны tool rules и holder sets, attack_range, attribute modifiers,
enchantments, food, repairable, tooltip_display и более ранние простые компоненты.
Добавлены writable/written books с фильтрованными страницами, container с пустыми
слотами, bundle_contents и charged_projectiles. Вложенный ItemStackTemplate имеет
порядок item/count, обычный сетевой ItemStack — count/item. Исходные patch bytes
сохраняются; бюджеты ограничивают глубину и число вложенных предметов. Есть JAR
fixtures непустых данных, roundtrip и всех усечений каждого пакета.

Неизвестный компонент нельзя безопасно пропустить внутри неделимого patch.
Такой inventory packet целиком отклоняется без публикации частичного состояния,
со счётчиком `play:inventory:unsupported_component`; соединение продолжается.
Полное покрытие компонентов ещё требуется. TCP-fixture после вложенных предметов
также посылает такой действительный неподдержанный компонент и проверяет состояние.

GUI читает серверные слоты/количества/названия и рисует базовые контейнеры с
официальными фонами. Экспортированы 19 из 25 menu slot layouts; остальные шесть
требуют Level-контекста. Thumbnail worker использует официальные item definitions,
модели и PNG, bounded очереди/кеши; CPU atlas и flat/3D icons проверены.

**Остаётся:** container click prediction/HashedStack transactions, перемещение
предметов мышью, shift/drag/number-key операции, специализированные кнопки/рецепты,
все component codecs и server default rebinding. Серверные окна пока read-only.
Хеши кликов в 26.2 — HashOps CRC32C над codec-представлением компонентов, а не
хеш сырых network bytes; источник сохранён в
`.cache/protocol-audit/container-click-hashes.txt`. Заглушка с пустыми хешами не
используется. Специальные item renderers, полный glint, анимации и рендер в руке
тоже не завершены. CPU-thumbnail lighting остаётся приблизительным.


Контрольная точка: 232 passed (200 library, 16 TCP, 5 Naga, 8 CPU geometry,
2 CPU icons, 1 CPU atlas), Clippy all-targets -D warnings и fmt проходят.
Логи `.cache/input-current-*.log`. Release упакован без графического запуска:
`.cache/OptiMC.app`, executable SHA-256
`9f2ca456f62a3bff0b520f75317f3d06a859a213c7ead6a4d79b5e666c078352`.
