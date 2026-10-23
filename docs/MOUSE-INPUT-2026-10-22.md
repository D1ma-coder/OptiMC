# ЛКМ/ПКМ и хотбар, 22 октября 2026

Причины из текущего исходного кода: ПКМ без block/entity hit возвращал управление
без пакета; ЛКМ по блоку создавал только Swing. Существующие атаки и взаимодействия
с сущностями, ПКМ по блокам сохраняются. Добавлены UseItem в воздухе и
START/ABORT/STOP_DESTROY_BLOCK по ЛКМ. Пустая основная рука выбирает непустую
вторую; полностью пустые руки не создают UseItem в воздухе.

UseItem содержит hand, общий prediction sequence, yaw/pitch в игровых градусах.
START/STOP используют тот же счётчик, ABORT — штатный sequence 0. Выбранный слот
и input отправляются до действий и взмаха. Worker проверяет world epoch, teleport
и готовность игрока. Все поля UseItem и три action × шесть направлений сравниваются
с байтами официального JAR. TCP проверяет смешанную последовательность
START/ABORT/START/STOP/UseItem и отбрасывание устаревшего ввода.

Добыча продвигается по client physics ticks, не кадрам; отпускание ЛКМ,
смена цели/инструмента и потеря захвата отменяют текущую добычу. После завершения
есть задержка 5 тиков. Неизменившийся блок после STOP не спамится повторно до
нового нажатия/смены цели. Это консервативное поведение до полного block prediction.
В creative учитывается tool.canDestroyBlocksInCreative. Adventure/spectator не
превращаются в свободное разрушение блоков.

Из официального клиента экспортированы параметры всех 32366 block states:
block ID, hardness, requiresCorrectToolForDrops, air и базовая высота воды.
Tool rules и теги берутся из текущего серверного состояния. Формула учитывает
MINING_EFFICIENCY, BLOCK_BREAK_SPEED, SUBMERGED_MINING_SPEED, haste/conduit power,
mining fatigue, нахождение глаз в воде и onGround. Эффекты сервера принимаются
и удаляются атомарно, с amplifier/duration/flags. Эталонные значения твёрдости
не выдаются за серверные overrides: протокол не передаёт их для обычных блоков.
Источник и точные wire fixtures — `tools/reference/ExportMining.java`;
данные и их SHA-1 — `data/26.2/mining-reference.json.gz`, `reference.json`.

Розово-чёрная иконка: range_dispatch/select ошибочно безусловно искали fallback.
У официального compass его может не быть. Теперь выбирается запись threshold
для нейтрального GUI sample, а display_context учитывает gui. CPU-проверка
compass/recovery_compass/clock использует официальные PNG; preview компаса
просмотрен. Динамическое направление стрелки/время и все специальные модели
ещё не перенесены полностью — это исправление выбора доступной GUI-модели.

Подпись выбранного предмета рисуется на height−59 в survival и height−45 без
survival HUD; старое height−35 накладывалось на здоровье/голод. Перенесены
двухсекундное отображение и затухание за последние десять тиков, с перезапуском
по изменению предмета/hover name, без перезапуска от количества.

**Не завершено:** локальное удаление/откат блоков по ACK, crack overlays,
все item-specific can_break/adventure predicates, полный InteractionResult
fallback блок → предмет/обе руки, use-duration/release для еды/лука/щита и
остальные эффекты. Оценка воды пока использует базовую высоту состояния без
полного vanilla fluid/entity eye update. Серверные container clicks пока read-only.
Эффекты ещё не имеют полного HUD/particles и всех игровых потребителей.
Графический runtime после системного сбоя не запускался; на настоящем сервере
изменения этого цикла не проверены. Полная parity и общая цель не достигнуты.


Контрольная точка: 232 passed (200 library, 16 TCP, 5 Naga, 8 CPU geometry,
2 CPU icons, 1 CPU atlas), Clippy all-targets -D warnings и fmt проходят.
Логи `.cache/input-current-*.log`. Release упакован без графического запуска:
`.cache/OptiMC.app`, executable SHA-256
`9f2ca456f62a3bff0b520f75317f3d06a859a213c7ead6a4d79b5e666c078352`.
