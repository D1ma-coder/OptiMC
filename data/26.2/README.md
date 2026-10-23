# Официальные protocol ID и block state definitions

`entity-picking.json.gz` содержит классы/размеры 158 entity types, metadata IDs,
поддержанные pose/baby dimensions и CPU-эталоны frame boxes, AABB clipping,
tags/interaction packets. ExportEntityPicking.java и подробные ограничения:
docs/ENTITY-PICKING-2026-10-22.md. Каталог не означает перенос всех динамических
размеров, пассажиров, equipment и взаимодействий.

`outline.json.gz` дополнительно содержит формы выбора 32366 block states из
`BlockState.getShape` (empty context); exporter — ExportOutlineShapes.java.
`use-reference.json.gz` — 1800 VoxelShape.clip и 24 ServerboundUseItemOnPacket
эталона из ExportUse.java. Контрольные суммы несжатых JSON записаны в reference.json.
Эти данные не доказывают перенос dynamic/entity-sensitive shape context.

Данные получены разовым запуском **`net.minecraft.data.Main --reports` из официального client.jar 26.2**. Это отчёты, а не исполняемый код Minecraft. Версия, protocol 776, SHA-1 исходного JAR и несжатых отчётов записаны в `reference.json`.

`blocks.json.gz` содержит 1196 блоков / 32366 состояний и их свойств. `registries.json.gz` содержит 95 статических реестров, включая 158 entity types и 1537 item IDs. gzip воспроизводим (`mtime=0`); SHA-1 проверяется Rust-загрузчиком после ограниченной распаковки, данные разбираются один раз в OnceLock. Имена блоков и реестров переиспользуются через Arc.

Для воспроизведения: взять тот же JAR (SHA-1 из manifest), скачать библиотеки из его официальных version metadata с проверкой SHA-1 и запустить JDK, совместимый с этим клиентом, с classpath из JAR и этих библиотек:

```
java -cp <official-client-and-library-classpath> net.minecraft.data.Main --reports --output <temporary-output>
```

Из `<temporary-output>/reports/` нужны только `blocks.json` и `registries.json`; сжать их gzip без timestamp и обновить контрольные суммы manifest. `--server` и `--all` не использовались. JDK и генератор не входят в runtime OptiMC. Configuration registries (dimensions, biomes и т. д.) приходят отдельно от сервера и не заменяются этими статическими таблицами.

`collision.json.gz` — дополнительный экспорт **данных**, не кода: 32366 state rows, 326 уникальных AABB-наборов, 199 rows с `Block.hasDynamicShape()`. Несжатый файл 773940 байт, gzip 10098 байт, SHA-1 в `reference.json`. После `SharedConstants.tryDetectVersion()` и `Bootstrap.bootStrap()` временный development harness перебирает `Block.BLOCK_STATE_REGISTRY` в ID-порядке и вызывает:

```
state.getCollisionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, CollisionContext.empty()).toAabbs()
state.getBlock().getFriction()
state.getBlock().getSpeedFactor()
state.getBlock().getJumpFactor()
state.getBlock().hasDynamicShape()
```

Формат: `shapes` содержит массивы `[minX,minY,minZ,maxX,maxY,maxZ]`; одинаковые `JsonArray.toString()` AABB-наборы получают индекс первого появления через LinkedHashMap. `states` содержит `[shapeIndex,friction,speedFactor,jumpFactor,dynamic]`. Корневой `context` равен `EmptyBlockGetter / BlockPos.ZERO / CollisionContext.empty`; затем `shapes`, затем `states`. Gson из официального classpath записывает compact JSON без newline. Выход сжимается gzip `mtime=0`. Начальная регистрация блоков не запускает сервер, сетевой listener или симуляцию. Harness выполнялся только отдельно при разработке; runtime читает gzip из Rust binary и не требует Java.

Контекст экспорта явно ограничен: dynamic shapes и entity-dependent правила не становятся корректными во всех ситуациях просто от наличия этого отчёта. Runtime должен реализовать такие правила отдельно, а также учитывать supporting block, fluids, attributes/effects и движущиеся формы.

`entities.json.gz` — development-time экспорт `LayerDefinitions.createRoots()` и базовых моделей `LivingEntityRenderer` из того же проверенного client.jar. Содержит 358 model layers, размеры 158 entity types и 90 базовых living-моделей с 87 различными текстурами. Экспортируются иерархия ModelPart, transform (translation/rotation/scale), visible/skipDraw и вершины полигонов с UV. Для базовой позы вызывается `model.setupAnim(renderer.createRenderState())`; slime/magma cube получают size=1, сохраняется матрица renderer.scale. Четыре missingno texture defaults заменены официальным temperate `asset_id` соответствующего variant data; источник отмечен в строках отчёта. Это данные для независимого Rust renderer, без исполнения JVM в клиенте.

У boat water-patch mask нулевые размеры текстуры в исходном LayerDefinition дают 48 неопределённых UV-компонент. Они нормализованы в 0 и отмечены `untextured_uv_normalized`; позиции и остальные UV сохранены. Маска не является обычной textured living-моделью. JSON записывается без NaN, gzip с `mtime=0`, несжатый SHA-1 указан в reference.json. Rust проверяет конечность геометрии и упаковывает текстуры с padding до 256×256, сохраняя исходные пиксели и пропорции UV (включая non-square и 192×192).

Эти 90 базовых поз не означают готовое отображение всех сущностей: подключение renderer к live scene, metadata-driven варианты, анимации, special renderers и дополнительные слои ещё не реализованы.

### Camera visual shapes

`camera_shapes.json.gz` contains 311 unique VISUAL shape lists for all 32366 block states, exported from the verified 26.2 client JAR using `BlockState.getVisualShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, CollisionContext.empty())`. This differs from physical collision geometry (glass can have an empty VISUAL shape; fences have different height). `reference.json` records the report hash. Development exporter and wall-distance reference are preserved in `tools/reference/ExportCameraShapes.java` and `tools/reference/CameraReference.java`; they require the official client and development libraries, and are not runtime dependencies. Entity-dependent contexts remain a separate compatibility gap.

### Attributes and physics references

`attributes.json.gz` contains all 40 official attribute names/IDs, generic defaults, ranges and syncability, generated from `BuiltInRegistries.ATTRIBUTE` in the verified 26.2 JAR by `tools/reference/ExportAttributes.java`. Its uncompressed SHA-1 is recorded in `reference.json` and checked at runtime alongside the pinned registry mapping. Player-specific movement defaults are separate from generic attribute defaults.

`attribute-packet.bin` is a real `ClientboundUpdateAttributesPacket.STREAM_CODEC` encoding from that JAR: entity 42, gravity base 0.08, all three modifier operations, expected original `AttributeInstance.getValue()` = 0.30000000000000004. Runtime uses Rust decoding and modifier evaluation. `physics-golden.json` records 120 empty-world falling steps using the original private `LivingEntity.computeModifiedFriction(0.98f, 1.0f)` and the inspected `aiStep`/`travelInAir` operation order. IEEE-754 bit strings avoid JSON decimal-parser rounding in regression tests. This reference covers dry free fall, not the complete entity travel system with fluids/effects/vehicles. Java is development tooling only.

`signs-reference.json.gz` содержит оригинальные sign transforms, DyeColor text RGB, sign chunk/update wire fixtures и ARMOR_STAND/ARMOR_STAND_SMALL ModelPart с эталонами setupAnim. `commands-reference.json.gz` содержит 57 argument properties и дерево команд с redirect. Экспорт: `tools/reference/ExportSigns.java`, `ExportCommands.java`, CPU only.
