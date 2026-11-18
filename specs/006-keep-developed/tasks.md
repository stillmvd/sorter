# Tasks: Проявленное не стирается

**Input**: `specs/006-keep-developed/` — plan.md, research.md (R1–R8), data-model.md, contracts/commands.md, quickstart.md
**Tests**: юнит-тесты Rust в `develop.rs`/`deck.rs` (`#[cfg(test)]`) — там, где меняется логика базы.

## Phase 1: Setup

Нет — схема и зависимости не меняются.

## Phase 2: Foundational

- [x] T001 Проявщик берёт карты только открытой колоды: в `develop::next` (`src-tauri/src/develop.rs:203`) условие
  `deck_path = (SELECT value FROM settings WHERE key='deck_path')`; нет открытой колоды — `None`, поток спит до `wake` (R3)
- [x] T002 Тест в `src-tauri/src/develop.rs`: две колоды с картами `stage='new'`, открыта одна — `next` отдаёт только её; `deck_path` пуст — `None`

## Phase 3: User Story 1 — вернулся в ту же папку (P1)

**Цель**: повторное открытие проявленной папки — без экрана проявки. **Проверка**: quickstart 1, 3, 4.

- [x] T003 [US1] В `choose_deck` (`src-tauri/src/commands.rs:176`) убрать `hints_enabled = "0"` и вызов `forget_deck` (R1, FR-005)
- [x] T004 [US1] Удалить `develop::forget_deck` (`src-tauri/src/develop.rs:552`) и его тесты (`develop.rs:846`)
- [x] T005 [US1] `choose_deck` → `#[tauri::command(async)]`: под `state.conn()` только запись `settings` и
  `paused.store(false)`; `deck::sync_seen` на своём соединении `db::open` (как `watch.rs`); затем `wake` (R6, M5, m7, m8)
- [x] T006 [US1] `choose_table` → `async`, `piles::sync` на своём соединении, `wake` (`commands.rs:208`)
- [x] T007 [US1] В `src-tauri/src/develop.rs` `tidy` убрать семидневную чистку `embedding`/`stage='meta'` для `gone` и `placed`
  (`KEEP_PLACED_MS`); уборку папок-сирот и чистку по `deck::surely_gone` оставить (R1, FR-006)
- [x] T008 [US1] Проверить `dupes::forget_missing`: удаляет только по `deck::surely_gone`; иначе перевести на него (FR-006)
- [x] T009 [US1] Тест: колода проявлена → `choose_deck` другой папки → снова первой — `stage` карт прежний, `embedding` и `fingerprint` на месте, `hints_enabled` не изменился

## Phase 4: User Story 2 — прервал разбор, продолжаю с места (P1)

**Цель**: Esc на проявке не теряет прогресс. **Проверка**: quickstart 2.

- [x] T010 [US2] `cancel_deck` (`src-tauri/src/commands.rs:196`): только `deck_path` пуст и `wake`, без стирания (R4)
- [x] T011 [US2] `cancelDevelop` в `src/App.tsx:112`: после `cancelDeck` — главный экран (`setHome(true)`), состояние проявки сбросить; ветки `search` и `dupes` не трогать — не понадобилось: `cancelDeck` и так ведёт на стартовый экран
- [x] T012 [US2] Тест: карты в `frames` и `new` → `cancel_deck` → `choose_deck` той же папки — `frames` сохранились, `new` проявляются дальше

## Phase 5: User Story 3 — изменилось немного (P2)

**Цель**: проявляются только новые, изменённые, вернувшиеся без кадров. **Проверка**: quickstart 5, 6.

- [x] T013 [US3] `deck::sync_seen` (`src-tauri/src/deck.rs:147`): искать и среди `status='gone'`; файл вернулся — `in_deck`,
  при совпадении `size` и `mtime` `stage` прежний, иначе `stage='new', frames=0` (R2)
- [x] T014 [US3] После сверки в `choose_deck`: один `read_dir(cache)` в множество id; карты колоды в `frames|embedded` без
  папки → `stage='new', frames=0`, строку `embedding` удалить (R5, FR-009)
- [x] T015 [US3] `SyncResult.pending` из `choose_deck` → флаг повтора у watcher (`src-tauri/src/watch.rs:65`, механизм `retry`) (R7, m2) — после смены папки watcher ставит `retry` и пересверяет без fs-события
- [x] T016 [US3] `develop::save` (`src-tauri/src/develop.rs:414`): если папка колоды недоступна или файл не открылся на чтение — `stage` остаётся `new`, без `broken`; проявщик ждёт `wake` (R7, M7, FR-012)
- [x] T017 [US3] Тесты: вернувшийся `gone` с тем же `size/mtime` — `in_deck` без перепроявки; с другим — `new`; карта без папки кадров — `new`

## Phase 6: User Story 4 — вижу объём и чищу сам (P3)

**Цель**: «Очистить кэш» не трогает открытую колоду и решения. **Проверка**: quickstart 7.

- [x] T018 [US4] `develop::clear_cache(conn, cache, open_deck)` (`src-tauri/src/develop.rs:536`): пропускать карты открытой
  колоды; у остальных — папки кадров, `embedding`, `fingerprint` по их путям, `dupe` с хотя бы одним таким путём;
  `dupe_dismissed` и `example` не трогать; вызов в `commands.rs:387` (R8, FR-008)
- [x] T019 [US4] Плитка «Кэш проявки» (`src/components/screens/Settings.tsx:192`): первое нажатие — кнопка «Точно очистить?» на 3 с, второе — очистка (FR-007)
- [x] T020 [US4] Тест: две колоды, открыта одна — после `clear_cache` у неё кадры и `fingerprint` целы, у другой — нет; `dupe_dismissed` на месте

## Phase 7: Polish

- [x] T021 `pnpm run check` и `pnpm build` (при запущенном dev — `CARGO_TARGET_DIR=target-test`)
- [x] T022 CDP-прогон quickstart 1–8 на dev (`SORTER_DATA`), замер SC-001 на `G:\sorter-test\big`
- [ ] T023 Вычитка новых строк UI агентом `ru-editor`, `/adversary` на дифф, коммит `feat: …`

## Dependencies

- T001 → все фазы. US1 (T003–T009) → US2 (T010–T012, `cancel_deck` опирается на удалённый `forget_deck`) → US3 → US4.
- T005 и T014, T015 правят одну функцию `choose_deck` — последовательно.
- [P] нет: почти всё в `commands.rs` и `develop.rs`.

## Implementation Strategy

MVP — фазы 2–4 (US1 + US2): повторное открытие без проявки и Esc без потерь. US3 и US4 — тем же коммитом, если
проверки зелёные.
