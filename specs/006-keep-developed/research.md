# Research: 006 — проявленное не стирается

Исходная точка — код на `2db43f3`. Номера строк — на нём.

## R1. Кто стирает проявленное

- `develop::forget_deck` (`develop.rs:552`) удаляет `embedding`, `fingerprint`, `dupe`, `dupe_dismissed` и строки `card`
  (или сбрасывает `stage='new'`), папки `cache/<id>` — в отдельном потоке. Вызовы: `choose_deck` (`commands.rs:183`,
  `only_untouched=true`, при смене пути) и `cancel_deck` (`commands.rs:200`, `false`).
- `develop::tidy` (`develop.rs:587`, один раз при старте проявщика) у карт `gone` и `placed` старше 7 дней удаляет
  `embedding` и ставит `stage='meta'`, удаляет папки `cache/` без карты в `frames|embedded`, примеры по
  `deck::surely_gone`, затем `dupes::forget_missing`.
- `choose_deck` при `before != path` ставит `hints_enabled = "0"` (`commands.rs:181`).

**Decision**: оба вызова `forget_deck` убрать, функцию удалить вместе с тестами (`develop.rs:846`). В `tidy` убрать
семидневную чистку `embedding`; остаются уборка папок-сирот (папка без карты — мусор, не проявленное) и
чистки по `surely_gone`. `hints_enabled` не трогать при выборе колоды.
**Rationale**: FR-001, FR-005, FR-006. Сироты в `cache/` появляются только после удаления карты — данных карты
там нет.
**Alternatives**: оставить `forget_deck` для «Очистить кеш» — нет, у очистки своя функция `clear_cache`.

## R2. Узнавание файла

`deck::sync_seen` (`deck.rs:147`) уже ищет карту колоды по `file_name` (нижний регистр) среди `in_deck|deferred` и
сравнивает `size` и `mtime`; при расхождении — `stage='new', frames=0`. Отпечатки дублей (`fingerprint`) хранят
`path, size, mtime`.

**Decision**: схему не менять. Расширить поиск в `sync_seen` на карты `gone`: файл вернулся с теми же `size` и
`mtime` — карта снова `in_deck` со своим `stage`; не совпали — `in_deck` и `stage='new'`.
**Rationale**: FR-002, допущение «вернётся файл — подхватится». Ключ «путь + размер + время» уже есть.
**Alternatives**: хеш содержимого — отказ владельца (волна 1: «Путь + размер + время»).

## R3. Проявка неоткрытой колоды

`develop::next` (`develop.rs:203`) берёт `stage='new' AND status!='gone'` по всем колодам, открытую — первой.

**Decision**: `next` берёт только карты открытой колоды (`deck_path` из `settings`); нет открытой — проявщик спит.
**Rationale**: допущение спеки «пока колода не открыта, её проявка не идёт»; иначе после выхода на главный экран
фоновая проявка чужой колоды грузит диск.

## R4. Выход с экрана проявки

`cancel_deck` (`commands.rs:196`) сейчас стирает колоду и отвязывает `deck_path`. Фронт `cancelDevelop`
(`App.tsx:112`) после него перезагружает состояние.

**Decision**: `cancel_deck` только отвязывает колоду (`deck_path` пуст) и будит проявщик — тот засыпает (R3).
Повторный `choose_deck` той же папки находит карты по R2, `develop::status` сразу `ready`, если `stage='new'`
не осталось.
**Rationale**: FR-003, FR-004, SC-002, SC-003.

## R5. Кадров нет на диске

**Decision**: в `choose_deck` после сверки — один `read_dir(cache)` в множество id; карты колоды в
`frames|embedded` без папки → `stage='new', frames=0` (`embedding` удалить). Дальше обычная проявка.
**Rationale**: FR-009; стоимость — одно чтение каталога, без обхода файлов.

## R6. Сверка не держит базу (M5) и пробуждение (m8)

`choose_deck` держит `state.conn()` (мьютекс) всё время `sync_seen` — ходы и переключатели ждут.

**Decision**: `choose_deck` → `#[tauri::command(async)]`: под мьютексом только запись `settings` и сброс `paused`
(m7); `sync_seen` — на своём соединении `db::open` (как в `watch.rs`), он и так под `BEGIN IMMEDIATE`; затем
проверка R5 и `wake` (m8). То же для `choose_table` (`piles::sync`).
**Rationale**: FR-011; правило памяти «решения gone/добавить — под `BEGIN IMMEDIATE` по свежему `exists()`»
сохраняется — его держит сам `sync_seen`. Фронт ждёт ответа `choose_deck` до первого хода, порядок ходов не
страдает.

## R7. Временный сбой чтения (M7) и потерянный `pending` (m2)

`develop::save` (`develop.rs:414`) ставит `stage='broken'`, если `develop_files` вернул `None` — и при отключённом
диске тоже. `SyncResult.pending` из `choose_deck` теряется: повтор сверки есть только у watcher (`watch.rs:65`).

**Decision**: перед `broken` проверить `deck::surely_gone`-подобно — папка колоды недоступна или файл не открылся
на чтение → `stage` остаётся `new`, проявщик засыпает до `wake` (watcher будит при возвращении). `pending` из
`choose_deck` → флаг повтора в watcher (тот же механизм `retry`).
**Rationale**: FR-012; память «отключённый диск ≠ пропавший файл».

## R8. «Очистить кеш»

`clear_cache` (`develop.rs:536`) удаляет все папки `cache/` и сбрасывает `stage` у всех карт. Плитка «Кэш проявки»
с размером и кнопкой уже есть (`Settings.tsx:192`), подтверждения нет.

**Decision**: `clear_cache(conn, cache, open_deck)`: пропускает карты открытой колоды; у остальных — папки кадров,
`embedding`, `fingerprint` и пары `dupe` их путей; `dupe_dismissed` и `example` не трогает. Подтверждение — второе
нажатие той же кнопки («Точно очистить?» на 3 с), без нового окна: форма уже утверждена, новый элемент не нужен.
**Rationale**: FR-007, FR-008; решения «не дубли» — это решения пользователя, не кеш.
**Alternatives**: модальное окно — новый UX без стенда.
