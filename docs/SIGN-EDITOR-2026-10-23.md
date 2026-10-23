# Редактор табличек и ListTag 26.2, 23 октября 2026

Источник: официальный client.jar 26.2, protocol 776, SHA-1
`2dc72797acbc1b63fc16a11c4ac393605f453754`.
Сверены AbstractSignEditScreen, SignEditScreen, HangingSignEditScreen,
TextFieldHelper, SignBlockEntity, ListTag и оба сетевых sign packet codec.

## Рабочая цепочка

OpenSignEditor 0x3c читает packed BlockPos и front flag. Для sign block открывается
редактор четырёх строк соответствующей стороны. Если block entity ещё не пришла,
но в мире уже есть sign block, используется пустой текст: оригинал создаёт такую
block entity по block state. Неверный block/type игнорируется, как в listener.
Ранее sign editor вообще отсутствовал.

GUI использует 24 официальные textures gui/signs и gui/hanging_signs для 12 wood
families. Масштаб/смещения фона, wall crop, normal text scale .9765628 и hanging
text scale 1, межстрочные интервалы 10/9 взяты из JAR. Цвет/светящийся текст берутся
из NBT. Заголовки/кнопка пока локализованы существующим русским GUI клиента.

Enter/Down и Up переключают строки по кольцу из четырёх, курсор переходит в конец.
Есть Unicode-ввод/IME commit, стрелки, Home/End, Delete/Backspace, выбор текста с
Shift, выбор слова с Ctrl/Cmd и Select All. Предикат использует ширину шрифта GUI
с пределом 90/60 px. Отклонённая по ширине вставка снимает выделение так же, как
TextFieldHelper; пробелы сохраняются и не нормализуются как в чате.
Escape отправляет изменения, как «Готово». Done возвращает mouse capture.

Request SignUpdate кодирует serverbound 0x3d: position, side, четыре строки,
предел 384 UTF-16 units соответствует серверному decoder. Worker проверяет epoch
и активные serial/position/side серверного запроса. Устаревший или повторный
submit не отправляется. Состояние закрытия публикуется и без нового ответа
сервера. Sign submit разрешён и до player_loaded: серверный запрос редактора
не должен блокироваться из-за ещё загружающейся сцены. Общий игровой ввод своих
проверок готовности не теряет.

## Исправление пропадающего текста

При тестировании исходного front_text (bold «Привет», plain «Front») выяснилось,
что plain строка превращалась в пустую. ListTag 26.2 умеет heterogeneous lists:
для wire type COMPOUND он оборачивает scalar в compound с единственным пустым
ключом. Reader вызывает addAndUnwrap. Наш decoder оставлял wrapper как данные,
поэтому RichText не видел строки. Это затрагивало и существующий world sign renderer.

Общий NBT reader теперь снимает ровно один такой wrapper только внутри списка.
Writer вычисляет фактический raw element type и оборачивает элементы по исходному
wrapIfNeeded. Настоящий compound с единственным пустым ключом экранируется ещё
одной обёрткой и после чтения сохраняется. Empty lists пишутся с canonical END type.
Защиты глубины/памяти остаются. Исправление применяется также к другим NBT consumers.

## Проверки

`tools/reference/ExportSignEditor.java` CPU-only вызывает оригинальные wire codecs,
TextFieldHelper и NbtIo/ListTag, без игрового окна, GPU или сервера.
`data/26.2/sign-editor.json.gz` содержит 24 пары open/update, UTF-16 boundary cases,
13 editing transitions и пять NBT cases (mixed/scalar/wrapper/nested/empty).
Hash фиксируется в reference.json и проверяется native tests.

Прошли 246 library tests (22 ignored), 18 localhost TCP (2 GPU ignored), отдельно
CPU atlas и CPU world-sign mesh tests. Итого 266 проверок; Clippy all-targets
-D warnings и fmt check проходят. TCP проверяет исходные bytes submit, early
submit до загрузки, world/serial fences, ровно одну отправку и closure publication.
Source sign NBT обеих сторон проверен по полным строкам, включая ранее потерянную
«Front». Обе стороны строят CPU glyph meshes и правильно инвалидируют кэш.
Графический результат и подключение к публичному серверу не проверялись.

## Что ещё требуется

Clipboard cut/copy/paste, IME preedit/placement, полные unihex/font shaping/bidi,
точное оформление курсора/выделения и keyboard widget focus пока не совпадают
полностью. Изменение текста в world block entity до server echo не предсказывается.
Автоматическое закрытие при removed sign/дистанции ещё не перенесено: оригинал
проверяет Player.isWithinBlockInteractionRange(pos,4), учитывая взаимодействие
с серверными атрибутами. При замене одного server editor другим сохраняется
только последнее разрешение; full removed-screen semantics ещё требуют проверки.
Поэтому полной parity редактора этим этапом не заявляется.

Свет/погода из предыдущего этапа пока остаются сетевым состоянием без lightmap/
визуальных эффектов. Enabled features, recipes/book, world border, ticking,
server data, equipment, particles, entity events и прочие пакеты последнего лога
остаются в общем плане. Полная цель активна; одиночный сервер не интегрируется.

Release `.cache/OptiMC.app`, executable SHA-256 `1068925c4b4095cbc8a84e0a299a6e231ed263eb0e621f7843d2aeb2453c2610`.
Backup `.cache/OptiMC-before-sign-editor`; release/cmp/plist подтверждены.
