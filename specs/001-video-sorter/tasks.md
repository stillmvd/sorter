# Tasks: Sorter — раскладка видео по папкам

**Input**: `specs/001-video-sorter/` — plan.md, spec.md, research.md, data-model.md, contracts/ipc.md, contracts/keyboard.md, quickstart.md

**Tests**: тесты Rust для ядра ходов обязательны (конституция, принцип I и «Рабочий процесс»); остальное — ручная приёмка по quickstart.md.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно параллельно (разные файлы, нет зависимостей)
- **[Story]**: US1…US6 — истории из spec.md

## Path Conventions

Tauri-проект в корне `Sorter/`: фронт `src/`, Rust `src-tauri/src/`. Образцы — `../Demo Prep/` (каркас, стили) и
`../Echo Studio/src/components/layout/` (заголовок окна). Дизайн — `.planning/DESIGN.md`.

---

## Phase 1: Setup (Shared Infrastructure)

- [X] T001 `git init`, `.gitignore` (node_modules, dist, src-tauri/target, `.claude/`, `.specify/`, `CLAUDE.md`, `HANDOFF.md`), каркас Tauri 2 + React 19 + Vite 7 + Tailwind 4 + TypeScript по образцу `../Demo Prep` (package.json, vite.config.ts, tsconfig*.json, index.html, pnpm-workspace.yaml) в `Sorter/`
- [X] T002 `src-tauri/tauri.conf.json`: productName `Sorter`, identifier `com.stillmvd.sorter`, окно 1440×900 min 1100×720, `decorations: false`; `src-tauri/Cargo.toml` с зависимостями из plan.md (tauri 2 + plugin-dialog, plugin-opener, plugin-window-state; rusqlite 0.32 bundled; serde; serde_json; windows 0.62; image; trash 5.2; notify 8.2; notify-debouncer-full) — без `ort` (добавит US4)
- [X] T003 [P] Шрифты: скопировать `C:\Users\stillmvd\Fonts\KockersSans-{Light,Regular,Medium,Bold}.woff2` в `src/fonts/`, `@font-face` 300/400/500/700 в `src/styles.css`
- [X] T004 [P] `src/tokens.css`: токены из `.planning/DESIGN.md` — тёмная тема голым `:root` (`--ground #141416 --cosmic #1c1c1f --raised #242427 --line #2e2e33 --strong #3c3c3f --fg #ececef --dim #a2a2a9 --ink #141416`), светлая в `@media (prefers-color-scheme: light) :root:not([data-theme="dark"])` и `:root[data-theme="light"]` (`#f4f4f6 #ffffff #ebebee #dedee2 #d2d2d5 #17171a #56565e`, `--ink #ffffff`); плёнка `--film #0c0c0e --hole #26262a`; easing `cubic-bezier(0.2, 0, 0, 1)`, 120/200/320 мс
- [X] T005 [P] `src/styles.css`: глобальный скроллбар (`*::-webkit-scrollbar-track:vertical { margin-block: var(--scroll-inset, 24px) }`, `:horizontal { margin-inline: … }`, ширина 10, бегунок `--strong` с прозрачной рамкой 2), `prefers-reduced-motion`, нажатие `scale(.96)`, фокус — кольцо 2 `--fg`, `tabular-nums`
- [X] T006 [P] Заголовок окна: перенести `../Echo Studio/src/components/layout/Titlebar.tsx` и `WindowControls.tsx` в `src/components/`, заменить акцентную точку на точку 8 `--fg`, подпись «Sorter» 14/700 и путь колоды 13/500 `--dim`
- [X] T007 [P] UI-примитивы по DESIGN.md в `src/components/ui/`: `Button.tsx` (капсула 40, primary — инверсия `--fg`/`--ink`, secondary — `--raised`), `IconButton.tsx` (круг 40, `aria-label`, `title`), `Segment.tsx`, `SearchField.tsx` (капсула 44/36), `Toggle.tsx`, `Kbd.tsx`, `PageHeader.tsx` (метка-капсула 28 + заголовок 42 с частями 300/700)

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: ядро ходов и хранилище — до любой истории.

- [X] T008 `src-tauri/src/db.rs`: открыть `%APPDATA%\com.stillmvd.sorter\sorter.db` (WAL, `synchronous=FULL`), схема из data-model.md — `settings(key PK, value)`, `card` (`stage` ∈ `new|meta|frames|embedded|broken`, `status` ∈ `in_deck|deferred|placed|gone`, `position real`, «уникально вместе с `deck_path` пока `status in (in_deck, deferred)`»), `pile` (`name` «уникально в пределах стола без учёта регистра», `key` «уникальна в пределах стола», `is_trash` «ровно одна на стол», `exists`), `move` (`method` ∈ `key|hint|search|table|drag|new_pile`, `state` ∈ `pending|done|undoing|undone|failed`), `move_item` (`step` ∈ `planned|copied|done|restoring|restored`), `example(card_id?, pile_id, path, vector blob)`; версия схемы в `settings`
- [X] T009 `src-tauri/src/moves.rs`: `free_name(dir, name)` → `имя (2).ext`, `имя (3).ext` — первое свободное; `validate_pile_name` — запрет `<>:"/\|?*`, завершающих точки/пробела, зарезервированных имён Windows
- [X] T010 `src-tauri/src/moves.rs`: `place(card_ids, pile_id, method)` — транзакция `Move(pending)` + `MoveItem(planned)` **до** файловых операций; тот же том — `fs::rename`; `ERROR_NOT_SAME_DEVICE` (17) — копия в `.<имя>.sorter-part` → `sync_all` → сверка размера → `rename` в итоговое имя → `step=copied` → удаление исходника → `done`; ошибка любого файла пачки — вернуть уже перенесённые, `failed` с человеческим `error`; повтор до 3 раз по 100 мс при блокировке файла (research R8)
- [X] T011 `src-tauri/src/moves.rs`: корзина — `trash::delete` для стопки `is_trash`, `to_path = null`; отмена — `trash::os_limited::list()` → элемент по исходному пути и времени → `restore_all`; занятый путь → ошибка `FILE_IN_THE_WAY`
- [X] T012 `src-tauri/src/moves.rs`: `undo(move_id)` (`undoing` → обратный перенос в `from_path` → `undone`; файла нет на месте — `FILE_NOT_THERE`, ничего не менять), `undo_last`, `undo_today`; `recover()` при запуске — `pending/planned` → откат, `copied` → сверить копию, удалить исходник, `done`, `undoing` → довести
- [X] T013 Тесты `src-tauri/src/moves.rs` (`#[cfg(test)]`, временные папки): перенос; суффикс при совпадении; пачка с ошибкой на 3-м файле откатывается целиком; отмена; `FILE_NOT_THERE`; recover после искусственной остановки на каждом шаге (`planned`, `copied`, `undoing`); перенос между томами — через подменяемую функцию rename, возвращающую ошибку 17
- [X] T014 `src-tauri/src/piles.rs`: скан подпапок первого уровня стола, стопка «Корзина» (`is_trash`, клавиша `Delete`, всегда последняя), раздача клавиш по `order`: `1…9`, затем `Q W E R T Y U I O P`; `set_key` снимает клавишу с прежней стопки; `create`, `rename` (папка на диске + `to_path` в журнале)
- [X] T015 `src-tauri/src/deck.rs`: скан верхнего уровня колоды (mp4, mov, mkv, webm, avi, m4v), `Card` new/обновление по `size`+`mtime`, `status gone` для исчезнувших; порядок `position`, `defer` → `max + 1`; дата из имени (`VIDyyyyMMddHHmmss`, 13-значный unix ms) иначе `mtime`
- [X] T016 `src-tauri/src/media.rs`: протокол `media://` — отдать файл колоды/стола/кэша с `Range` (206, чтение только куска), доступ только внутри этих трёх корней
- [X] T017 `src-tauri/src/commands.rs` + `lib.rs`: команды из contracts/ipc.md (`get_state`, `choose_deck`, `choose_table` с проверкой «не совпадают, стол не внутри колоды и наоборот», `set_setting`, `deck_window`, `place`, `defer`, `undo_last`, `undo_move`, `undo_today`, `journal`, `create_pile`, `rename_pile`, `set_pile_key`, `open_in_explorer`); ошибки `{ code, message }` на русском; `recover()` до показа окна
- [X] T018 [P] `src/lib/ipc.ts`: типы Card/Pile/Move/Hint из data-model.md и обёртки команд/событий; `src/lib/keys.ts`: сопоставление по `KeyboardEvent.code` (работает в русской раскладке), список клавиш стопок

**Checkpoint**: `cargo test` зелёный, команды отвечают из devtools.

---

## Phase 3: User Story 1 — Разложить колоду по одной карте (P1) 🎯 MVP

**Goal**: первый запуск → колода → клавиши/поиск/Enter/Tab/Ctrl+Z/перетаскивание, файлы реально переезжают.

**Independent Test**: quickstart.md сценарии 1–5, 11, 13, 14.

- [X] T019 [US1] Замер тестовой колоды: скрипт `src-tauri/examples/probe.rs` (Media Foundation: кодек и контейнер каждого файла `G:\vk videos`) → доля HEVC записывается в research.md R1
- [X] T020 [US1] `src/App.tsx`: маршрут по состоянию — нет колоды/стола → Start; иначе Deck; тема из настроек → `data-theme`
- [X] T021 [P] [US1] `src/components/screens/Start.tsx` по холсту «Первый запуск»: 3 шага-карточки (колода — число видео и размер, стол — «Папки внутри станут стопками», подсказки — тумблер), системный диалог папки, «Проявить плёнку» неактивна до выбора стола, пояснение почему; ошибки проверки папок текстом под шагом
- [X] T022 [US1] `src/components/deck/Card.tsx`: карта с `<video muted autoplay loop>` через `media://`, форма по `orientation` (`portrait`/`landscape`/`square`) в квадратной зоне, поворот 90° за 320 мс, при `prefers-reduced-motion` — сразу; под картой колода — 2 карты с поворотами 3,5° и 7°; три `<video>` (текущая, следующая предзагружена, предыдущая), ход меняет роли, а не `src` (research R8); пауза Пробел, звук M (состояние `muted` в настройках)
- [X] T023 [US1] Плёночный режим в `Card.tsx`: `<video>` дал `error` или нет кадров за 1,5 с → смена кадров плёнки каждые 400 мс, подсказка «Ctrl+O — открыть в плеере»; `open_in_explorer`/opener для файла
- [X] T024 [P] [US1] `src/components/deck/PilesRow.tsx` + `Pile.tsx`: 12 колонок, высота 84, r16, клавиша 16/700 и число, толщина = тени-слои по 4 px (до 5), подсветка «похоже» — инверсия + пунктирное кольцо `--fg` 1,5 с отступом 4 + `translateY(-8px)`; «Корзина» последней с клавишей Delete (форма — по T049); строка над рядом: «Стопки · N», поле поиска 36 с `/`, «Новая стопка»
- [X] T025 [P] [US1] `src/components/deck/LastMove.tsx`: мини-карта, файл, «→ стопка», итоговое имя при суффиксе, «Забрать» Ctrl Z
- [X] T026 [US1] `src/components/deck/DeckScreen.tsx`: раскладка по холсту «Колода» (шапка: метка «Разложено N из M», заголовок «В колоде N», сегмент Колода/Стол, круглые кнопки Журнал/Стопки/Настройки); клавиатура по contracts/keyboard.md — клавиша стопки, Delete, печать → поиск, Enter (поиск → первая подходящая), Shift+Enter (новая стопка из текста), Esc, Tab, Ctrl+Z; ход оптимистичный — карта уходит сразу, ошибка возвращает её с сообщением
- [X] T027 [US1] `src/components/dnd.ts` + подключение в Card/PilesRow: перетаскивание карты на стопку (подсветка «похоже» под курсором), мимо — возврат за 200 мс, ход `drag`
- [X] T028 [US1] Пустой стол: ряд стопок с приглашением «Напечатай название — создашь первую стопку»; проверка сценариев quickstart 1–5, 11, 13, 14 на копии колоды

**Checkpoint**: MVP — колода раскладывается клавишами, всё забирается.

---

## Phase 4: User Story 2 — Проявка (P2)

**Goal**: плёнки и сведения в фоне, раскладка не ждёт.

**Independent Test**: quickstart 7, 8.

- [X] T029 [US2] `src-tauri/src/develop.rs`: Media Foundation (`windows` 0.62, `IMFSourceReader`) — длительность, размер кадра с учётом поворота, `orientation` (portrait: h > w·1,05; landscape: w > h·1,05; иначе square); 8 кадров равномерно по длине → 320 px по длинной стороне → JPEG q80 в `%APPDATA%\com.stillmvd.sorter\cache\<card_id>\<idx>.jpg`; нечитаемый файл → `stage = broken` + `error`
- [X] T030 [US2] `develop.rs`: фоновая очередь с приоритетом (ближайшие 20 карт колоды первыми, потом остальные), пауза/продолжить, события `develop://progress` (≤ 4/с) и `develop://card`; пропуск карт с `stage ≥ frames` при том же `size`+`mtime`; команда `develop_control`
- [X] T031 [US2] `src-tauri/src/deck.rs`: слежение `notify` + `notify-debouncer-full` (тишина 1,5 с) за колодой; готовность — открытие без `FILE_SHARE_WRITE`, `ERROR_SHARING_VIOLATION` → повтор с нарастающей паузой до 60 с; событие `deck://changed`; то же для первого уровня стола → `piles://changed`
- [X] T032 [P] [US2] `src/components/deck/FilmStrip.tsx`: плёнка `--film` r20, перфорация 10×6 r2 с шагом 10 сверху и снизу, 8 кадров h62 с таймкодом (Cascadia Mono 10), текущий кадр обведён «карандашом» (SVG, штрих `#ececef` 2,5 round); ← → — кадр и перемотка видео
- [X] T033 [P] [US2] `src/components/screens/Develop.tsx` по холсту «Проявка»: «Проявляю N из M», полоса 8, плёнки проявляются (пустые кадры — пунктир), правая колонка задач (раскадровки / сведения / подсказки), «Начать раскладывать», «Пауза»; показывается при первой проявке колоды, дальше проявка — тихо в фоне
- [X] T034 [US2] Сведения под картой в `DeckScreen.tsx`: имя, «дата · длина · размер · разрешение»; метка «не удалось проявить» для `broken`

---

## Phase 5: User Story 3 — Стол (P3)

**Goal**: пачка одним ходом.

**Independent Test**: quickstart 9, 10.

- [X] T035 [P] [US3] `src/components/table/ContactSheet.tsx` + `StripRow.tsx` по холсту «Стол»: 2 колонки плёнок по 6 кадров h44, подпись номера/имени/длины моно 10, отметка — галочка «карандашом» + обводка 1,5, наведение — `<video>` поверх плёнки без звука; виртуализация списка (колода до 5 000)
- [X] T036 [US3] `src/components/table/TableScreen.tsx`: клик/Shift+клик/Ctrl+A/Esc по contracts/keyboard.md, строка «Отмечено N» + поиск (остальные стопки opacity .3), клавиша стопки/Delete/Enter/Shift+Enter → `place(card_ids, …, method: table|new_pile)` одним ходом; перетаскивание пачки на стопку
- [X] T037 [US3] Переключение Колода/Стол (сегмент, Ctrl+1/Ctrl+2), режим в настройках; `LastMove` для пачки — «N карт → стопка»

---

## Phase 6: User Story 4 — Подсказки (P4)

**Goal**: «Просится в стопку X, N%», Enter кладёт.

**Independent Test**: quickstart 12.

- [X] T038 [US4] Runtime: скачать вручную (один раз, разработчиком) `onnxruntime.dll` + `DirectML.dll` (версия под `ort` 2.0.0-rc.13) и `dinov2-small` ONNX fp16 в `src-tauri/resources/`; `hints.rs` вшивает их `include_bytes!` и распаковывает в `%APPDATA%\com.stillmvd.sorter\runtime\` при первом запуске (сверка размера); `ort` с `load-dynamic`, `directml` → откат на CPU
- [X] T039 [US4] `src-tauri/src/hints.rs`: вектор карты — среднее по 4 центральным кадрам, L2-норма, `stage = embedded`; в очередь проявки после кадров
- [X] T040 [US4] `hints.rs`: примеры — после каждого хода (вектор карты → `example`), забранный ход удаляет пример; фоновое сканирование видео, уже лежащих в стопках (холодный старт), с низким приоритетом
- [X] T041 [US4] `hints.rs`: `hints(card_ids)` — косинус, k = 3 ближайших по стопке, среднее; стопки с < 3 примерами не подсказываются; показывать при `score ≥ 0,5`; для пачки — средний вектор
- [X] T042 [US4] `src/components/deck/HintBox.tsx` по холсту: «Просится в стопку · учится на N картах», название 28/700, процент, «дальше X N%», «Положить» Enter (инверсия); без подсказки — «учится · N примеров», Enter при пустом поиске ничего не делает; подсветка «похоже» у стопки в ряду; общая подсказка для пачки на столе
- [X] T043 [US4] Выключение подсказок (первый запуск и настройки): блок скрыт, эмбеддинги не считаются — первый запуск и бэкенд; переключатель в настройках — в T048

---

## Phase 7: User Story 5 — Стопки и клавиши (P5)

**Independent Test**: сменить клавишу, переименовать, создать — видно в проводнике и в ряду.

- [X] T044 [P] [US5] `src/components/screens/Piles.tsx` по холсту «Стопки и клавиши»: «К колоде», сетка 4 колонки карточек (`--raised` r20, тени-слои), клавиша-капсула 36, имя 18/700, число 22/700 и состояние подсказок («пусто — подсказок пока нет» / «мало примеров для подсказок» / «подсказки учатся на этих картах»), переименование, «Нажми клавишу…» при назначении, «Новая стопка» пунктиром, «Открыть стол в проводнике»
- [X] T045 [US5] Ошибки имени (`BAD_NAME` со списком символов), конфликт клавиши (прежняя стопка остаётся без клавиши — видно), реакция на `piles://changed`

---

## Phase 8: User Story 6 — Журнал и итоги (P6)

**Independent Test**: quickstart 3, 16.

- [X] T046 [P] [US6] `src/components/screens/Journal.tsx` по холсту: группы по дням, строки-капсулы 56 (время, мини-карта, файл → стопка, способ: клавиша N / подсказка · Enter / поиск «…» / стол · N карт / перетаскивание / новая стопка), «Забрать» у каждого хода, «Забрать всё за сегодня»; забранные и сбойные — приглушены с пометкой; бесконечная прокрутка `journal(before, limit)`
- [X] T047 [P] [US6] `src/components/screens/Done.tsx` по холсту «Колода пуста»: 3 плитки итогов, столбики стопок, «Открыть журнал», «Открыть стол в проводнике»; показывать, когда `left = 0`

---

## Phase 9: Polish & Cross-Cutting Concerns

- [X] T048 [P] `src/components/screens/Settings.tsx`: тема (системная/тёмная/светлая), смена колоды и стола, подсказки, «Очистить кэш проявки» (`clear_cache`, показ освобождённого места) — сначала холст (`/design revise`) — холст HSettings; блок «Версия и обновления» — в фиче 003
- [X] T049 Нарисовать на холсте https://claude.ai/artifact/DShPQtS1SVorfBa5UuxzqY: стопку «Корзина», состояние перетаскивания, плёночный режим карты, горизонтальную карту, настройки — до вёрстки T024/T027/T023/T048 — нарисованы только настройки (HSettings), остальное сделано в коде раньше
- [X] T050 Тексты всех ошибок и пустых состояний на «ты» с причиной и следующим шагом (FR-030), проверка контрастов обеих тем
- [X] T051 Производительность: SC-002 (смена карты ≤ 0,3 с p95 — логирование в dev), SC-004 (первые 20 карт ≤ 15 с) на `G:\vk videos`
- [X] T052 Проверка SC-007: Resource Monitor → сеть у `sorter.exe` всю сессию
- [ ] T053 Релиз `pnpm tauri build --no-bundle` → `src-tauri/target/release/sorter.exe`, прогон всех сценариев quickstart.md на копии колоды, README без упоминаний ассистента

---

## Dependencies & Execution Order

### Phase Dependencies

- Setup (T001–T007) → Foundational (T008–T018) → истории.
- US1 зависит только от Foundational. US2–US6 — от US1 (используют экран колоды и ряд стопок).
- US4 требует кадров (US2). US3 использует плёнки (US2) — без них показывает заглушки кадров.
- T049 (холст) — до T023, T024 (часть «Корзина»), T027, T048.

### Within Each User Story

Rust → ipc → экран → проверка по quickstart. Коммит после одобрения пользователя.

### Parallel Opportunities

- Setup: T003–T007 параллельно после T001–T002.
- Foundational: T014–T016 параллельно после T008; T018 параллельно с Rust.
- US1: T021, T024, T025 параллельно; T022 → T023 → T026 → T027.
- US2: T032, T033 параллельно с T029–T031.

## Parallel Example: User Story 1

```text
T021 Start.tsx
T024 PilesRow.tsx + Pile.tsx
T025 LastMove.tsx
```

## Implementation Strategy

### MVP First (User Story 1 Only)

Setup → Foundational (ядро ходов с тестами) → US1 → проверка на копии `G:\vk videos` → показать пользователю.

### Incremental Delivery

US1 → US2 (плёнка, главное ускорение) → US3 → US4 → US5 → US6 → Polish. После каждой истории — демо и коммит
`feat(usN): …` после одобрения.

## Notes

- Файлы пользователя в разрушающих проверках — только копия колоды.
- Экран без утверждённого макета не полируется (конституция IV).
- В коде нет комментариев, в git нет упоминаний ассистента.
