# Tasks: Главный экран и разбор нескольких папок

**Input**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md),
[contracts/commands.md](contracts/commands.md), [quickstart.md](quickstart.md)

## Phase 1: Основа (блокирует всё)

- [x] T001 `src-tauri/src/paths.rs`: `canon(&str) -> String` (R5), `is_inside(child, parent)` без учёта регистра,
  `walk(root, skip: &[String], stop: &AtomicBool, seen: &mut dyn FnMut(usize)) -> io::Result<Vec<(String, i64, i64)>>`
  — пути от корня через `\`, без ссылок, `$RECYCLE.BIN`, `System Volume Information`, каталогов из `skip`; тесты
- [x] T002 `src-tauri/src/db.rs`: миграция по [data-model.md](data-model.md) — таблицы, `open_root`,
  `move.review_id`, канонизация путей, разбор из `settings.deck_path`, черновик; тест на базе со старой колодой
- [x] T003 Все запросы колоды с `deck_path = <settings.deck_path>` → `deck_path IN (SELECT path FROM open_root)`:
  `develop.rs` (`next`, `status`, `to_embed` в `hints.rs`, `drop_lost_frames`), `dupes/mod.rs` (`deck_files`,
  `dupes_for`, `groups`), `dupes/near.rs:239`, `series.rs`, `commands.rs` (`paths`, `get_state`, `deck_window`,
  `clear_cache`); `dupes::deck_files` читает файлы корней через `walk`, а не `read_dir(deck)`
- [x] T004 `deck.rs` `sync_seen(conn, root, skip, stop, seen)`: файлы через `paths::walk`, позиция — после
  максимума по корням разбора (R13); `CardView.origin` (« › »)

## Phase 2: US1 + US2 — главный экран, несколько папок (P1)

- [x] T005 [US1] `src-tauri/src/review.rs`: `draft(conn)`, `add_roots` (R2–R4: канон, внутри стола, внутри корня,
  поглощение вложенного корня, `IN_REVIEW`, `NO_MEDIA`, сверка корня), `remove_root`, `set_draft_table`, `start`;
  тесты: поглощение, стол внутри, папка из законченного разбора
- [x] T006 [US1] Команды `home`, `add_roots`, `remove_root`, `set_draft_table`, `stop_reading`, `start_review`
  в `commands.rs` и `lib.rs`; удалить `choose_deck`, `cancel_deck`, `choose_table`; `close_review`
- [x] T007 [US1] `watch.rs`: корни открытого разбора — рекурсивно, `online` каждого корня, sync по корням
- [x] T008 [US1] `tauri.conf.json` `dragDropEnabled: true`; `src/lib/ipc.ts` — новые команды и типы
- [x] T009 [US1] `src/components/screens/Home.tsx` + `src/components/home/*` по стенду 009 (A · A · A · C · B · A):
  плитки, строки корней с «Читаю… нашёл N» и «Остановить Esc», рамка перетаскивания, ошибки строкой в плитке,
  «Разобрать Enter», подписи «Сначала выбери …»; переключатели «Подсказки» и «Дубли» строками в плитке стола
- [x] T010 [US1] `App.tsx`: главный экран при запуске вместо `Start`, `onDragDropEvent` → плитка под курсором,
  Esc с колоды → главный экран; `Start.tsx` удалить
- [x] T011 [US2] `src/components/deck/Facts.tsx`: строка папки над именем файла (вариант A)

## Phase 3: US3 — Продолжить разбор (P1)

- [x] T012 [US3] `review.rs`: `list` незаконченных (остаток, всего, дата, первая карта, `online`), `open`, `drop`;
  `recheck`; тест «законченный не в списке»
- [x] T013 [US3] `moves.rs`: `review_id` у хода, `undo_last`/`undo_since` в рамках разбора, `MoveView.review`;
  basename в стопке (R6), отмена создаёт папку-источник заново
- [x] T014 [US3] Блок «Продолжить разбор» (вариант C) в `Home.tsx`, Enter — последний разбор, когда черновик пуст;
  «Папки нет — подключи диск X:» и «Проверить снова»; журнал — подпись разбора
- [x] T015 [US3] `state.missing` больше не уводит на стартовый экран: только стол; колода без недоступных корней

## Phase 4: US4 — Проводник (P2)

- [x] T016 [US4] `lib.rs` single-instance и аргумент запуска → `add_roots` черновика + `home://incoming`; фронт —
  главный экран, окно на передний план

## Phase 5: US5 — пустые папки (P3)

- [x] T017 [US5] `review::empty_dirs`, `moves::place` — каталоги в корзину; команды `empty_dirs`, `trash_dirs`; тест
- [x] T018 [US5] `Done.tsx`: «Убрать N пустых папок» с отменой

## Phase 6: Проверки

- [x] T019 `pnpm run check`, `pnpm build`
- [x] T020 CDP-прогон [quickstart.md](quickstart.md) на dev, скриншот главного экрана
- [x] T021 ru-editor на новые строки, `/adversary`, коммиты по фазам `feat: …`

## Dependencies

Phase 1 → Phase 2 → Phase 3; Phase 4 и 5 — после Phase 2. Параллельно почти ничего: общие `commands.rs`,
`review.rs`.
