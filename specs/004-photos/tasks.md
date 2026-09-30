# Tasks: Фото

**Input**: `specs/004-photos/` — spec, plan, research, data-model, contracts/ui.md, contracts/ipc.md, quickstart

**Tests**: `cargo test` там, где есть логика без UI (EXIF, серии, ход серии), — как в фичах 001–002.

## Phase 1: Setup

- [X] T001 Скрипт тестового набора `scripts/make-photo-set.py` (Pillow): в `G:\sorter-test\photos` — 3 серии по 3–6 снимков из разных фото `G:\vk photos` (сдвиг 1–4 %, яркость ±8 %, EXIF DateTimeOriginal с шагом 1–3 с), 6 одиночных снимков с тем же шагом, но других сцен; копии одного фото — уменьшенная до 50 %, пережатая q=60, кадрированная до 80 %; снимок с поворотом пикселей и EXIF Orientation=6; JPG без EXIF; PNG с прозрачностью; анимированный GIF из 8 кадров; GIF из 1 кадра; `broken.jpg` (мусорные байты); `manifest.json` с ожидаемыми сериями и дублями
- [X] T002 Фичи `png`, `webp`, `gif` у `image` в `src-tauri/Cargo.toml`, сборка проходит

## Phase 2: Foundational

- [X] T003 Миграция `src-tauri/src/db.rs`: колонки `card.kind TEXT NOT NULL DEFAULT 'video' CHECK (kind IN ('video','photo'))`, `camera TEXT`, `taken_from TEXT CHECK (taken_from IN ('exif','name','file'))`, `phash INTEGER`, `sharp REAL`, `series_id INTEGER`, `solo INTEGER NOT NULL DEFAULT 0`; индекс `card_series ON card(series_id) WHERE series_id IS NOT NULL`; проверка через `pragma_table_info`, как `group_id`
- [X] T004 `src-tauri/src/deck.rs`: `PHOTO_EXT` (jpg, jpeg, png, webp, gif), `is_photo`, `is_media`, `kind_of(path)`; `sync` берёт `is_media` и пишет `kind`, `taken_from` = `name`/`file`; `CardView` + `kind`, `camera`, `taken_from`, `series_id`, `CARD_COLUMNS` и `card_from_row`; тест `sync` с фото и видео
- [X] T005 [P] Заменить отбор файлов на медиа: `src-tauri/src/hints.rs` (`cold_files`), `src-tauri/src/dupes/mod.rs` (`files`), `src-tauri/src/piles.rs` (счёт)
- [X] T006 [P] `src-tauri/src/media.rs`: mime `image/png`, `image/webp`, `image/gif`; для картинок без `Range` отдавать файл целиком (не 16 МБ частью); тест на mime
- [X] T007 `src-tauri/src/photo.rs`: `read(path) -> PhotoMeta { width, height (после поворота), orientation, taken (Option<ms>), camera, thumb: RgbImage 320, phash, sharp, animated }` на `image::ImageReader` + `ImageDecoder::orientation()` / `exif_metadata()`; разбор IFD: 0x010F, 0x0110, 0x0132, 0x8769 → 0x9003, порядок байт II/MM; pHash — `dupes::print::phash` по серой 128×128; резкость — дисперсия лапласиана; тесты на собранных вручную байтах EXIF (оба порядка байт, без Exif IFD, мусор) и игнорируемый на `G:\sorter-test\photos`
- [X] T008 `src-tauri/src/develop.rs`: карта фото проявляется через `photo::read` — `0.jpg` в кэше, `frames = 1`, размеры, `orientation`, `camera`, `phash`, `sharp`, `taken_at`/`taken_from = 'exif'` если есть; ошибка — `stage = 'broken'` с текстом «Не удалось открыть это фото — разложить его всё равно можно.»; `read_frames` для фото отдаёт миниатюру (холодные примеры)
- [X] T009 `src-tauri/src/hints.rs`: `card_frames` для фото — `0.jpg`; `learn`/`suggest` без изменений
- [X] T010 `src/lib/ipc.ts`: типы `Card.kind/camera/takenFrom/seriesId`, `Pile.videos/photos`, `SeriesView`, `Kind`; `deckWindow(from, count, kind)`, `deckSeries`, `placeSeries`, `splitSeries`, `onSeries`; настройки `kind_filter`, `series_rest`

**Checkpoint**: фото попадают в колоду и проявляются, видео работают как раньше (`cargo test` зелёный).

## Phase 3: User Story 1 — Фото в колоде (P1) 🎯 MVP

**Goal**: карта фото с правильной ориентацией, сведениями и ходами.

**Independent Test**: колода — копия `G:\vk photos`: 266 карт, поворот, сведения, ход клавишей и Ctrl+Z.

- [X] T011 [US1] `src/components/deck/Card.tsx`: карта фото — `<img>` с `media(path)`, `object-fit: contain` в области `#141417` r 16, углы снимка r 6, ширина карты 500 (contracts/ui.md), GIF играет; верх — номер и «Фото · JPG»; битый файл — текст ошибки; видео без изменений
- [X] T012 [US1] Предзагрузка следующей карты фото `new Image()` + `decode()` в `src/components/deck/Card.tsx` (или `CardStack`)
- [X] T013 [P] [US1] `src/components/deck/PhotoFacts.tsx`: блок сведений `raised` r 20 — «Снято» / «Дата из имени» / «Дата файла» с примечанием «даты съёмки в файле нет», «Камера», «Разрешение» с «N Мп», «Размер» с форматом (contracts/ui.md)
- [X] T014 [US1] `src/components/deck/DeckScreen.tsx`: для фото вместо плёнки — `PhotoFacts`; ← → и Пробел не действуют на фото; подсказка «дальше …» отдельной строкой, кнопки «Положить», «В конец» без сжатия (`src/components/deck/HintBox.tsx`); строка клавиш для фото
- [X] T015 [P] [US1] `src/components/deck/LastMove.tsx`: превью хода фото 64×48 из `0.jpg`
- [X] T016 [US1] Приёмка US1 через CDP: пункты 1–2 quickstart

## Phase 4: User Story 2 — Фильтр по типу (P1)

**Goal**: «Все / Видео / Фото» в колоде и на столе, запоминается.

**Independent Test**: смешанная колода, фильтр «Фото» — только фото; перезапуск — фильтр тот же.

- [X] T017 [US2] `src-tauri/src/deck.rs` + `src-tauri/src/commands.rs`: `window(conn, deck, from, count, kind)` с `kind` = `all`/`video`/`photo`; `counts_by_kind`; `get_state` отдаёт `deck.byKind`; `kind_filter` в `SETTING_KEYS` и разрешённых ключах `set_setting_cmd`; тест окна с фильтром
- [X] T018 [P] [US2] `src/components/ui/KindFilter.tsx`: сегмент «Все N · Видео N · Фото N» (счётчик 11 px, opacity .7), вид как `Segment`
- [X] T019 [US2] `src/components/deck/DeckScreen.tsx`: фильтр в шапке слева от режима (колода и стол), `refill`/`loadSheet` с `kind`, заголовок «Фото в колоде N» / «Видео в колоде N», плашка «Разложено X из Y фото»; сохранение `kind_filter`; фильтр прячется, если в колоде один тип
- [X] T020 [US2] `src/components/screens/Done.tsx`: при пустом выбранном типе и оставшемся другом — «Фото кончились — осталось N видео» и кнопка «Показать видео»
- [X] T021 [US2] Приёмка US2 через CDP: пункт 3 quickstart

## Phase 5: Стопки и стол с фото (FR-006a, общее для US1–US3)

- [X] T022 [P] [US1] `src-tauri/src/piles.rs`: `PileView.videos`, `photos`, `count` = сумма; Корзина — по ходам как раньше; тест
- [X] T023 [US1] `src/components/deck/PilesRow.tsx`: значки видео и фото со счётчиками справа от клавиши (11 px, gap 6, opacity .75), нет типа — нет значка; толщина по сумме
- [X] T024 [US1] `src/components/table/Tile.tsx`, `src/components/table/ContactSheet.tsx`: плитка фото — миниатюра `0.jpg` по пропорциям, без перемотки; видео как раньше

## Phase 6: User Story 3 — Серия одной картой (P2)

**Goal**: серия — одна карта, отметки, один ход, отмена целиком, «Разбить».

**Independent Test**: серия из 5 в `G:\sorter-test\photos`: отметить 2 → 2 в стопке, 3 в корзине, Ctrl+Z → 5 на месте.

- [X] T025 [US3] `src-tauri/src/series.rs`: `regroup(conn, deck) -> bool` (изменился ли состав): фото колоды `in_deck`/`deferred`, `solo = 0`, с `phash`, по `taken_at`; ребро при разнице ≤ 10 с и `SERIES_MIN < ham ≤ SERIES_HAM` (18; для `taken_from = 'file'` — 10); цепочки ≥ 2 → `series_id` = id первого; лучший — max `sharp × √(w·h)`; `list(conn, deck) -> Vec<SeriesView>`; `split(conn, id)` ставит `solo = 1`; тесты на синтетике и игнорируемый `series_on_set` по `manifest.json` (SC-003)
- [X] T026 [US3] Вызовы `series::regroup` + событие `deck://series`: `src-tauri/src/develop.rs` после партии, `src-tauri/src/watch.rs` после `sync`, `src-tauri/src/commands.rs` после `place`/`undo`/`split`
- [X] T027 [US3] `src-tauri/src/deck.rs`: окно колоды отдаёт из серии только первый снимок (`series_id IS NULL OR series_id = id`); счётчики — по файлам
- [X] T028 [US3] `src-tauri/src/moves.rs`: `place_series(conn, series_id, keep, pile_id, rest, method)` — отмеченные (или все) `move_with` в стопку, остальные при `rest = trash` — второй ход в Корзину с тем же `group_id`, ошибка второго откатывает первый; тесты: ход, `keep`, отмена группой
- [X] T029 [US3] `src-tauri/src/commands.rs` + `src-tauri/src/lib.rs`: команды `deck_series`, `place_series`, `split_series`; `series_rest` в настройках
- [X] T030 [US3] `src/components/deck/SeriesPanel.tsx`: заголовок «Серия из N снимков · дата · за T с · отмечено M», лента 4 колонки (номер, «лучший», галочка, фокус), «Остальные N» + сегмент «в Корзину / оставить в колоде», «Разбить серию S» (contracts/ui.md)
- [X] T031 [US3] `src/components/deck/Card.tsx`: карта серии 440×380, снимок в фокусе, подложки влево по числу снимков, «снимок K из N», «Серия · ЧЧ:ММ:СС – ЧЧ:ММ:СС», галочка отмеченного
- [X] T032 [US3] `src/components/deck/DeckScreen.tsx`: карта серии — ← → по снимкам, Пробел — отметить, клавиша стопки / Enter / перетаскивание → `placeSeries`, S → `splitSeries`, Tab — серия в конец (все снимки), подсказка по лучшему снимку «Отмеченные M просятся в стопку»; `onSeries` → перечитать окно
- [X] T033 [US3] Стол: `src/components/table/Tile.tsx` + `src/components/deck/DeckScreen.tsx` — плитка серии (лучший снимок, подложки 4/−4 и 8/−8, «Серия · N»), отметка серии = все её снимки, «Отмечено K · F файлов», подсказка строки «серия уходит вся · выбрать лучшие — в колоде»
- [X] T034 [US3] Приёмка US3 через CDP: пункты 5–6 quickstart

## Phase 7: User Story 4 — Подсказки для фото (P2)

**Goal**: стопка подсказывается для фото, примеры общие с видео.

**Independent Test**: 10 похожих фото в стопку — следующая похожая просится туда.

- [X] T035 [US4] Проверить `src-tauri/src/develop.rs` (`Learner::step`): фото получают вектор по `0.jpg`, холодные фото стопок учатся через `read_frames`; игнорируемый тест в `src-tauri/src/hints.rs` на `G:\sorter-test\photos`
- [X] T036 [US4] Приёмка US4 через CDP: пункт 7 quickstart (SC-006)

## Phase 8: User Story 5 — Дубли фото (P3)

**Goal**: плашка и экран «Дубли» для фото, только внутри типа, серии не дубли.

**Independent Test**: уменьшенная копия фото — плашка, одно нажатие убирает худшую, Ctrl+Z возвращает.

- [X] T037 [US5] `src-tauri/src/dupes/print.rs`: `print_photo(path)` — миниатюра через `photo::read`, срез полей, один pHash, `stills` = [миниатюра], `duration_ms = 0`; `print` выбирает по типу файла
- [X] T038 [US5] `src-tauri/src/dupes/near.rs`: сравнение только внутри типа; для фото — ham ≤ 8 → `same` с уверенностью `100 − 5·ham`, иначе смысл ≥ 0.9 → `crop`; игнорируемый тест по `manifest.json` (SC-005)
- [X] T039 [US5] `src-tauri/src/dupes/mod.rs`: `dupes_for` и `groups` пропускают пары одной серии; `copy_of` для фото — пиксели без битрейта; лучшая копия — разрешение, затем размер
- [X] T040 [US5] `src/components/deck/DupeBadge.tsx`, `src/components/dupes/Compare.tsx`, `src/components/dupes/DupesScreen.tsx`: тексты для фото («та же фотография, меньше разрешение», «кадрирована»), сравнение двух картинок без воспроизведения, фильтр типа на экране «Дубли»
- [X] T041 [US5] Приёмка US5 через CDP: пункт 8 quickstart; ложные плашки на `G:\vk photos` ≤ 1

## Phase 9: Polish

- [X] T042 Замер SC-002 скриптом CDP (50 переходов по фото, p95 ≤ 100 мс) и SC-007 на `G:\vk videos`
- [X] T043 [P] `src/components/deck/KeyLegend.tsx`: клавиши для фото и серии (← →, Пробел, S)
- [x] T044 [P] `README.md`, `RELEASE_NOTES.md`: фото, серии, фильтр
- [X] T046 Окно при запуске — по центру экрана, не уезжает под панель задач (замечание пользователя 2026-09-30): `src-tauri/tauri.conf.json` / восстановление позиции `tauri-plugin-window-state`
- [x] T045 Полная приёмка quickstart 1–10, `cargo test`, `pnpm build`, проверки из CLAUDE.md проекта; обновить `HANDOFF.md`

## Dependencies & Execution Order

- Setup (T001–T002) → Foundational (T003–T010) → US1 (T011–T016) и US2 (T017–T021) → стопки/стол (T022–T024) →
  US3 (T025–T034) → US4 (T035–T036) → US5 (T037–T041) → Polish.
- US3 зависит от `phash`/`sharp` (T007–T008) и окна колоды (T017). US5 зависит от `photo::read` (T007) и серий (T025).
- US4 почти целиком закрывается T008–T009; фаза — проверка.

## Parallel Opportunities

- T005, T006 параллельно друг другу после T004.
- T013, T015 параллельно T011; T018 параллельно T017; T022 параллельно T023–T024.
- T043, T044 параллельно в Polish.

## Implementation Strategy

MVP — US1 + US2 (фото в колоде и фильтр): уже можно разбирать фото. Затем стопки со счётом по типам, серии
(главная ценность для фотоархива), подсказки и дубли. После каждой фазы — приёмка через CDP и коммит.
