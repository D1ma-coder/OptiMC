# Протокол Minecraft Java 26.2

Первичный источник — официальный client.jar Mojang, без использования готового Minecraft-клиента или готовой библиотеки протокола в runtime OptiMC.

* [Официальный релиз 26.2](https://www.minecraft.net/en-us/article/minecraft-java-edition-26-2)
* [Манифест версий](https://piston-meta.mojang.com/mc/game/version_manifest_v2.json)
* [Метаданные 26.2](https://piston-meta.mojang.com/v1/packages/91ef51b7d36ca092c5c02aba42f0f40c7cc1db84/26.2.json)
* [Официальный client.jar](https://piston-data.mojang.com/v1/objects/2dc72797acbc1b63fc16a11c4ac393605f453754/client.jar), SHA-1 `2dc72797acbc1b63fc16a11c4ac393605f453754`.

`version.json` из архива указывает `protocol_version=776`, `world_version=4903`, resource pack `88.0`, data pack `107.1`.

Порядок зарегистрированных пакетов проверен при разработке через `javap -p -c` для `net.minecraft.network.protocol.game.GameProtocols` и `net.minecraft.network.protocol.configuration.ConfigurationProtocols`. JDK используется исключительно как инструмент исследования первичного источника, не как зависимость сборки или запуска OptiMC. В репозитории нет скопированных классов Minecraft.

В [protocol-776.json](protocol-776.json) сохранена таблица packet ID, прочитанная из порядка регистрации в этих классах. Она является справочником, а не заявлением, что все эти пакеты реализованы. Login использует отдельно проверяемые стандартные ID 0–5. Все игровые обработчики сейчас находятся в `src/net/mod.rs`.

Особенности чанков подтверждены `ClientboundLevelChunkPacketData`, `LevelChunkSection`, `PalettedContainer` и `FriendlyByteBuf`:

1. X и Z — big-endian i32.
2. Heightmaps — карта enum → long array, а не старый корневой NBT compound.
3. Длина буфера секций — VarInt.
4. В каждой секции два short: non-empty block count и fluid count.
5. Палитры блоков (4096 записей) и биомов (64 записи).
6. Нулевой bits-per-entry: один VarInt state ID, без long array.
7. Ненулевой bits-per-entry: локальная либо глобальная палитра; число packed u64 выводится из ёмкости и bits, отдельного VarInt длины нет. Значения не пересекают границы u64.
8. Далее читается список block entities (packed XZ, signed Y, registry type, network NBT); light data пока не применяется.

Состояния блоков сопоставляются с block names/properties по встроенным официальным отчётам 26.2 (32366 states). Configuration registry_data разбирается с unnamed network NBT, dimension holder в CommonPlayerSpawnInfo — прямой VarInt registry ID. Login/respawn определяют min_y/height; число секций проверяется по текущему измерению. Клиентский RemoteWorld сохраняет chunks, применяет single/section updates и forget_level_chunk. Uniform секции хранят одно значение и разворачиваются при первом отличающемся изменении.

Section update: packed section position X22/Z22/Y20 (signed), VarInt count, VarLong records `stateID << 12 | localX << 8 | localZ << 4 | localY`. ChunkPos unload — packed long с X в младших 32 битах, Z в старших. Тест TCP проверяет два chunk load, один unload и оба вида block update, включая отрицательные координаты.

До полноценной игры нужны live renderer/управление, light/block entity data, metadata/equipment/визуальные модели сущностей и inventory/game synchronization. NBT сохраняет все 12 payload types и Java UTF-16 units; лимит глубины считается по контейнерам как в NbtAccounter (512), а общий memory/node budget действует на все NBT entries одного registry packet.
