# Показ кадров без разрывов и ровные сроки FPS

Ранее VSync Off выбирал Mailbox → Immediate → FIFO. На MoltenVK без Mailbox
это приводило к Immediate, который по спецификации допускает tearing.
Теперь VSync On выбирает FIFO, Off выбирает поддерживаемый Mailbox, иначе FIFO.
Ни Immediate, ни FifoRelaxed, ни AutoNoVsync не выбираются. FIFO гарантирован
API даже при отсутствии его в искусственном тестовом списке capabilities.

Это осознанный fallback с ограничением: на системе без Mailbox (в сохранённой
диагностике этого Mac — MoltenVK с FIFO/Immediate) показ кадров синхронизирован,
а рендер может ограничиваться частотой экрана. Это не реализация независимого
software mailbox и не решение задачи unlimited FPS на данном Mac. Настройка
vsync пользователя не переписывается; Graphics показывает реальный режим и
«Mailbox недоступен: FIFO, предел частоты экрана», заголовок —
«VSync Off / FIFO fallback». Старое сообщение в общем Ui.message убрано:
оно могло затирать полезный статус другой операции на каждом кадре.

Срок следующего кадра раньше вычислялся как фактическое время начала + период:
каждое опоздание пробуждения сдвигало сетку. Новый Pacer сохраняет фазу сроков,
пропускает опоздавшие интервалы и сбрасывает фазу после длительной паузы или
смены ограничения. Ранний redraw от resize/expose не сдвигает срок. Режим без
лимита не сохраняет ожидание от предыдущего лимита. Не добавлены spin-wait,
таймер из предположения о 120 Hz, ручная задержка GPU, MoltenVK env overrides,
unsafe CAMetalLayer mutations или глобальный device.poll(Wait) в цикле.

Проверки CPU: все 16 комбинаций четырёх explicit present modes для обоих
состояний VSync, раскрытие fallback в UI; 1000 опозданий при 120 FPS без
накопления дрейфа, ранний redraw, пропуск сроков, пауза, смена/снятие лимита.
210 library tests passed; Clippy all-targets -D warnings, fmt и release проверены.
Окно/GPU не запускались согласно GPU-INCIDENT-2026-10-20.md. Фактические
tearing/мерцание/плавность и влияние драйвера на FPS не измерены; мерцание иной
природы (например, текстуры/освещение) этим изменением не объявляется исправленным.
Пользовательский процесс, settings, accounts и server list не менялись.

Основания:
- https://docs.rs/wgpu/24.0.5/wgpu/enum.PresentMode.html
- https://github.khronos.org/Vulkan-Site/refpages/latest/refpages/source/VkPresentModeKHR.html

Логи .cache/presentation-current-{lib,clippy,release,package}.log.
Общая цель полной совместимости остаётся активной.

Обновлён .cache/OptiMC.app; backup .cache/OptiMC-before-presentation.
SHA-256 executable: 96a69c9eaac98802f20bf5f22d7fc61e52570904dda3231cd0be2eb187f7151f.
Bundle plist и совпадение executable/bundle проверены; приложение не запускалось.
