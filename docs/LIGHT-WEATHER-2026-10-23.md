# Серверный свет и погода: состояние и кодеки, 23 октября 2026

Источник — официальный client.jar 26.2 / protocol 776,
SHA-1 `2dc72797acbc1b63fc16a11c4ac393605f453754`.
Это этап переноса сетевого состояния; визуальное освещение и дождь ещё не завершены.

## Реализовано

`light_update` 0x30 читает VarInt x/z, четыре BitSet и два списка 2048-байтных
DataLayer. Тот же payload теперь разбирается после block entities полного
`level_chunk_with_light` 0x2d; раньше хвост вообще не применялся.
Игровой путь требует световой хвост. У вспомогательных decode/decode_for_dimension
сохранена поддержка неполных CPU fixtures без хвоста; они не используются для
приёма серверных чанков в SessionMachine.

Слои адресуются от minSection-1 до maxSection включительно (две padding sections).
Старые значения не стираются при частичном обновлении; empty masks явно задают 0.
Data mask имеет приоритет над overlapping empty bit, как readSectionList.
Вне диапазона измерения bits и лишние data entries не применяются. Неверный
размер массива или недостающий слой отклоняются до публикации изменений.
Текущий decoder требует 2048 байт также у surplus arrays, хотя оригинальный
wire byte-array codec допускает более короткие массивы и listener не проверяет
их, если соответствующий bit находится вне диапазона. Эта более строгая
проверка ещё требует выравнивания с оригиналом.
Nibbles читаются по исходному y*256 + z*16 + x, младшая половина байта первой.
Отсутствующее значение остаётся None, а не подменяется нулём или дневным светом.

Store использует Arc-колонки и Arc-секции/массивы. Частичное обновление копирует
карту затронутой колонки; другие колонки и неизменившиеся секции делят данные
между immutable snapshots. Нулевые слои не выделяют массив 2048 байт.
Unload освобождает свет колонки; новый мир/reconfiguration сбрасывают хранилище.
F3 показывает полученные sky/block values и число сохранённых световых секций.

Game events 1/2/7/8 обновляют rain/thunder level по фактическому байткоду listener
и Level setters. В этом JAR action 1 устанавливает rain=0, action 2 — rain=1;
это не догадка по именам START_RAINING/STOP_RAINING. Действия 7/8 используют
Mth.clamp; отрицательный ноль и NaN проверены по исходным raw float bits.
Setter одновременно меняет предыдущее/текущее значение: дополнительная анимация
на самом приёме пакета не вводится. Новый ClientLevel начинает погоду с 0.

## Проверки

`tools/reference/ExportLightUpdates.java` вызывает оригинальные codec decode/
encode, DataLayer.set/get и Level weather setters. Последние выполняются на
выделенном без конструктора ClientLevel: поля погоды не требуют Minecraft/GPU.
Не запускается игровой клиент, сервер или локальная симуляция света.
Fixture `data/26.2/light-updates.json.gz`, SHA-1 сверяется с reference.json.

243 library tests (22 ignored), включая 4096 source nibble samples, partial/empty/
overlap/padding, все усечения тела, атомарность, snapshot identity и 28 weather
cases. 17 localhost TCP tests (2 GPU ignored) проверяют свет внутри чанка и
последующие отдельные обновления, сохранение sky/block и отсутствие unhandled
для этих реализованных действий. Clippy all-targets -D warnings и fmt check проходят.

## Не завершено

Свет ещё не подключён к terrain/entity shaders, vanilla lightmap, ambient
occlusion, emissive rules или dirty-neighbor remeshing. Нет локального
LayerLightEngine propagation/queued-section reconciliation; прямое сохранение
сетевых arrays — необходимая основа, но не эквивалент всей системы света.
Погода пока не управляет небом, каплями, звуком, туманом или освещением.
Никаких заявлений о визуально исправленном освещении этим этапом не делается.
GPU/окно/публичные серверы не запускались согласно GPU-INCIDENT-2026-10-20.md.

`set_passengers`, `update_advancements` и неподдержанные entity metadata остаются
в работе. Проверка respawn, начатая до нового пользовательского лога, выявила
неверное сохранение abilities/vitals/experience и очистку remote effects;
исправление ещё не внесено. KEEP_ENTITY_DATA переносит только non-default metadata:
health default в SynchedEntityData = 1, тогда как LivingEntity constructor задаёт
max health. Поэтому простое «сохранить старое здоровье» тоже не воспроизводит
все случаи оригинала. Для следующего этапа сохранён respawn-lifecycle-review-26.2.txt.
Полная цель остаётся активной.

Release `.cache/OptiMC.app`, executable SHA-256 `406a871b815dcc8fc876a129df696809feba69fe34436af009cd6ec391051cef`.
Backup `.cache/OptiMC-before-light-state`; release/cmp/plist подтверждены.
