# Выбор, удар и использование сущностей

Предыдущий цикл цели — продвижение: базовый ПКМ по блокам был подключён и
упакован. Текущий цикл устраняет подтверждённое расхождение: луч мог выбирать
блок за рамкой/стойкой, поскольку не рассматривал сущности.

## Подключено

- Общий выбор Entity/Block по направлению игрока с отдельными серверными
  block_interaction_range и entity_interaction_range. Луч ограничивается
  ближайшим блоком, сущность перед ним получает приоритет. Если такая сущность
  вне своей дальности, результат становится miss, не блоком за ней.
- Данные всех 158 EntityTypes: официальные размеры типа, классы, владельцы
  isPickable/getPickRadius и metadata indices. Поддержанные poses используют
  размеры JAR; масштабы LivingEntity берутся из server scale attribute.
  Получены 139 таблиц poses и 42 таблицы baby/small poses. Невидимость не
  выключает picking. Спектаторы из player-info и marker-стойки не выбираются.
- Рамки отдельно: все шесть сторон, толщина 1/16 блока, ширина/высота 3/4 без
  карты и 1 блок при наличии MAP_ID. Small-стойки используют оригинальные
  размеры 0.25 × 0.9875; marker исключён из picking независимо от видимости.
  Interaction entity читает серверные float width/height из metadata.
- Приняты Configuration update_tags 0x0d и Play update_tags 0x86: замена по
  registry, отсутствие registry сохраняет прежнее значение; строгая атомарная
  проверка полного пакета, общие бюджеты members/tags/names. Projectile picking
  использует именно серверный redirectable_projectile, Arrow учитывает IN_GROUND.
- ЛКМ по сущности: ServerboundAttackPacket 0x01, затем Swing 0x3f. ПКМ:
  ServerboundInteractPacket 0x1a с относительной LP-точкой и secondary flag.
  Отправка подключена только к пользовательскому вводу, главная рука.
  Удержание ПКМ сохраняет задержку четыре physics ticks; автоатаки по удержанию
  ЛКМ не добавлено. ЛКМ по miss/блоку пока отправляет только swing, не добычу.
- Перед действием синхронизируются selected slot и input/sneak. Проверяются
  world epoch, teleport version и актуальные ID+UUID цели. Старый ID не может
  направить отложенное действие на заменившую сущность. Удалённая цель отбрасывается.
- Определение типа выбирается за O(1) по protocol ID, соответствие всех ID
  официальному builtin registry проверяется при загрузке. Каталог и outline
  shapes загружаются до создания окна, не при первом клике.

## Доказательства

Источник: локальный официальный JAR 26.2, SHA-1 из data/26.2/reference.json.
tools/reference/ExportEntityPicking.java — CPU-only development exporter.
Runtime Rust; JDK, Unsafe и конструкторы Minecraft в runtime не используются.
Экспорт инициирует factory с null Level только для определения класса по
constructor stack, затем читает нужные статические данные/методы на отдельно
подготовленных объектах. Это не запуск сервера или графического клиента.
19 типов требуют полноценного registry/level context для получения poses;
ошибки явно записаны в данных, вместо объявления полного результата.

data/26.2/entity-picking.json.gz хранит эталоны и SHA-1 проверяется при загрузке.
1956 поддержанных пар type/pose проверены по IEEE double bits исходного
EntityDimensions.makeBoundingBox: отдельные hex-эталоны исключают округление
при JSON-decoding. 48 frame boxes совпадают для map/non-map, шести направлений
и координат −30 млн…+30 млн. 1000 AABB.clip проверены с точностью 1e-10.
Проверены marker/small, невидимая рамка, стена перед сущностью, range miss,
серверный projectile tag и Arrow IN_GROUND. Четыре interaction fixtures
побайтно проверяют обе руки/secondary и ненулевой LP-вектор из оригинала.

Loopback TCP проверяет slot/input → interact → attack → swing; stale world,
teleport, удалённый ID и несовпавший UUID не попадают в сеть. Configuration tags
доходят до snapshot; Play replacement удаляет прежнюю membership. Другие TCP
проверки, shader validation и официальная CPU geometry остаются зелёными.

186 library + 14 loopback TCP + 5 Naga + 8 CPU geometry = **213 passed**.
Clippy all-targets -D warnings и fmt проходят. Логи .cache/entity-pick-*-tests.log
и .cache/entity-pick-clippy.log. GPU, окно, внешние серверы не запускались;
визуальное поведение и live-server interaction пока не подтверждены.

## Что не завершено

Это не полный перенос всех entity interactions. Требуются species-specific
динамические размеры (size/variants, pose failures для 19 типов), painting/leash
attachments, EnderDragon parts, пассажиры/root-vehicle exclusion, equipment
правило SulfurCube canBePickedFromInside, точный vanilla traversal для equal
distance ties (пока стабильный ID order), клиентская интерполяция entity hitboxes.
Для отсутствующего pose используется базовый размер типа; для отсутствующей
baby-таблицы — стандартный half scale. Не все такие fallback проверены для
каждого вида и состояния. Это явное ограничение, не заявленная vanilla parity.

Также остаются item ATTACK_RANGE, item InteractionResult/offhand fallback,
damage/mining/cooldown prediction, локальная hand animation, inventory/equipment
и container synchronization/UI. Локального нанесения damage/удаления сущностей
нет: изменения продолжают поступать от сервера. Добыча блоков ещё не подключена.
