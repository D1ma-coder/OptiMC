# Серверные предметы, E и базовые анимации

Исправлена устаревшая блокировка физической KeyE при наличии remote session.
E открывает инвентарь игрока и закрывает инвентарь/контейнер через существующий
CloseMenu с menu serial fence; раскладка клавиатуры не влияет на KeyCode.

Причина, способная оставить серверное меню пустым: недобавленный компонент
одного стека отклоняет весь ContainerSetContent. Добавлены stream codecs
custom_model_data (четыре списка), profile (resolved/name/UUID, свойства и skin
patch), potion_contents (optional potion/color/name и рекурсивные эффекты),
potion_duration_scale, typed entity_data/block_entity_data и bucket_entity_data.
Патчи сохраняются в исходных байтах; никаких выдуманных предметов/слотов.

CPU helper ExportMenuItems.java использует оригинальный 26.2 client.jar
2dc72797acbc1b63fc16a11c4ac393605f453754, protocol 776. Эталонные восемь стеков
и меню с 63 слотами лежат в data/26.2/menu-items.json.gz. Проверены все восемь
стеков до последнего байта, каждый усечённый префикс, заполнение меню и хотбара.
Предыдущий fixture намеренно неподдерживаемого custom_model_data заменён на
пока неподдерживаемый bees: неизвестный неделимитированный payload по-прежнему
нельзя безопасно пропускать. Отклонение остаётся атомарным, сохраняет предыдущие
слоты, теперь также сохраняет имя компонента в report.unhandled и печатает
первую такую ошибку. Все component codecs ещё не перенесены. На живом сервере
именно этот снимок не воспроизводился: проверен класс ошибки и эталонные пакеты.

Пустая рука теперь получает анимацию взмаха, в том числе при ЛКМ в воздух.
Применяется animated-prefix delta из ItemInHandRenderer.renderPlayerArm
относительно прежнего rest pose нашей skin mesh. Матрицы сравнены с оригинальным
PoseStack на семи фазах; геометрия/rest pose руки ещё не объявляются полностью
тождественными оригиналу. Период базового взмаха — шесть тиков.
ПКМ при отправке взаимодействия/UseItem запускает базовый взмах нужной руки.
Это не полная система use animations: еда, питьё, лук, щит, копьё, кисть,
серверные флаги использования, длительность от эффектов и полный InteractionResult
fallback остаются в работе. Server container clicks/HashOps также ещё не готовы;
ранее созданные HashOps fixtures сохранены, но runtime не объявляется добавленным.

Проверено: 206 library + 16 localhost TCP + 5 Naga + 1 CPU held projection +
1 CPU held contours/models = 229 passed. Clippy all-targets -D warnings, fmt,
release build и bundle plist проходят. Логи .cache/menu-current-*.log.
Окно/GPU и внешние серверы не запускались после прежнего системного сбоя.
Настройки, аккаунты, серверный список и пользовательский процесс не менялись.

Обновлён .cache/OptiMC.app; backup .cache/OptiMC-before-menu-items.
SHA-256 executable:
0c1b35549a95817c0b307e0429d2f3228f87b8e89590147f37caebe9a8e5bfdc.
Общая цель полной совместимости остаётся активной.
