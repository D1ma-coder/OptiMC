# Предметы от первого лица, 22 октября 2026

До изменения серверный хотбар получал предметы, но первый вид всегда строил
пустую правую руку. Теперь основной выбранный стек и offhand читаются из
авторитетного container::Store и передаются в отдельный рендер предметов.
При готовой модели основной руки пустая рука скрывается. F1 и spectator скрывают
первый вид; переключение мира сбрасывает состояние показа/взмаха.

Источник: официальный 26.2 client.jar SHA-1
`2dc72797acbc1b63fc16a11c4ac393605f453754`. CPU-reference helper
`tools/reference/ExportHeldItems.java` извлекает:

- 30 матриц ItemInHandRenderer.applyItemArmTransform/swingArm для обеих рук;
- зеркальные ItemTransform для нестандартных rotation/translation/scale;
- исходные SideFace контуры ItemModelGenerator для diamond, iron_pickaxe,
  compass_16 (50/72/52 ребра).

Java используется только для проверки при разработке, без запуска уровня,
сервера или GPU. Строки/матрицы упакованы в `data/26.2/held-reference.json.gz`,
SHA-1 записан в reference.json. Runtime остаётся на Rust/WGSL.

Обычные JSON-модели используют официальные firstperson_righthand/lefthand,
включая наследование и item_model component override. Generated-слои получают
переднюю/заднюю плоскости на z=7.5/16 и 8.5/16 и отдельные торцы по альфа-контуру,
включая отверстия. Поддержаны обе руки, многослойные текстуры и обычные блочные
модели. Для контуров и обеих рук проверены настоящие локальные ресурсы.

Исправлены два расхождения, обнаруженные эталонами: ItemTransform.apply также
включает финальную трансляцию −0.5; Mth.sin использует квантованную таблицу
65536 значений, а не прямой sin. Матрицы теперь проходят сравнение с JAR.

Геометрия/PNG строятся в worker, не в render loop: очереди по 8, LRU до 64
результатов, неудачные результаты тоже кешируются. Ограничены количество текстур,
контуров и вершин. На кадре переиспользуются векторы и GPU-буферы достаточного
размера; текстуры загружаются только при смене набора моделей. У обеих рук свой
материал в общей D2Array до 32 слоёв. Мир и руки имеют разные uniform-проекции;
руки используют отдельный depth-clear проход до общего post processing.
Текущий view bob передаётся в проекцию предмета. Карты/текст мира не менялись.

Есть тиковое опускание/поднятие при смене модели (шаг не более 0.4 за 50 мс,
смена показанной модели при высоте <0.1) и базовый шеститиковый взмах по ЛКМ.
GPU-ресурсы и worker не зависят от render resolution/FPS cap. Это не измерение
производительности и не полная копия всех веток ItemInHandRenderer.

**Остаётся:** maps in hands, shields/skulls/beds и другие special renderers,
точные component-dependent tints/dispatch и динамические compass/clock properties,
use/eat/drink/bow/spear/brush/spin animations, все изменения длительности взмаха,
attack-strength equip target, точная проверка component equality при re-equip,
полный vanilla lightmap/glint/render layers. Нынешняя подсветка — базовая diffuse,
без полной parity освещения. Рендер held items в третьем лице и оборудование
других сущностей ещё требуется. Анимированные PNG пока используют первый кадр.
Геометрия торцов сравнивалась по контуру; все UV/анимационные ветки полного
ItemModelGenerator ещё не объявляются проверенными.

CPU projection rasterizer строит превью compass/pickaxe/stone в реальной
perspective × source arm matrix и проверяет, что геометрия попадает в экран.
`.cache/held-compass.png` просмотрен; это CPU-превью, не кадр Vulkan.
Окно/GPU не запускались после прежнего системного сбоя, визуальное поведение
драйвера и FPS не проверены. Общая цель остаётся активной.


Итог: 237 passed (203 library, 16 TCP, 5 Naga, 8 CPU world geometry, 2 icons,
1 atlas, 1 held geometry, 1 projection). Clippy all-targets -D warnings/fmt/release/
plist проходят; логи `.cache/held-current-*.log`.
Release упакован без запуска в `.cache/OptiMC.app`, SHA-256 executable
`017a4cef4c93a619dd54a87097e8b031d3a971c9ec5d90173341516ebb31ef05`.
