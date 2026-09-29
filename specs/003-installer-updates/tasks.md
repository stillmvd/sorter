# Tasks: Установщик и обновления

**Input**: `specs/003-installer-updates/` — spec.md, plan.md, research.md, data-model.md, contracts/ipc.md, quickstart.md

**Tests**: Rust-тесты — разбор папки из аргументов и переходы состояния обновления; установщик, Проводник и
обновление — ручная приёмка по quickstart.md (сборка, установка, локальный сервер `latest.json`).

**Дизайн**: блок «Версия и обновления» — по кадру HSettings (твик `update`); смена стола в окне из Проводника —
кадра нет, сначала холст и одобрение.

**Внешние действия** (только по явному разрешению пользователя на конкретный шаг): создание публичного репозитория,
секрет, пуш тега, публикация релиза.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [X] T001 Версия `1.0.0` синхронно в `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `package.json`
- [X] T002 [P] `src-tauri/Cargo.toml`: `tauri-plugin-updater = "2"`; `pnpm add @tauri-apps/plugin-updater` не нужен — вызовы идут через свои команды (research R1)
- [X] T003 [P] Ключ подписи: `pnpm tauri signer generate -w ~/.tauri/sorter.key` без пароля; публичный ключ — в `plugins.updater.pubkey` `src-tauri/tauri.conf.json`; приватный не коммитить

---

## Phase 2: Foundational

- [X] T004 `src-tauri/tauri.conf.json`: `bundle.active: true`, `targets: ["nsis"]`, `createUpdaterArtifacts: true`, `windows.nsis` — `installMode: "perMachine"`, `languages: ["Russian"]`, `displayLanguageSelector: false`, `installerIcon: "icons/icon.ico"`, `installerHooks: "./windows/hooks.nsh"`; `plugins.updater` — `endpoints: ["https://github.com/stillmvd/sorter/releases/latest/download/latest.json"]`, `windows.installMode: "quiet"`
- [X] T005 `src-tauri/src/lib.rs`: подключить `tauri_plugin_updater::Builder::new().build()`; в `AppState` — `incoming: Mutex<Option<String>>` и состояние обновления (T013)

**Checkpoint**: `pnpm tauri build` собирает `Sorter_1.0.0_x64-setup.exe` и `.sig` (с `TAURI_SIGNING_PRIVATE_KEY`).

---

## Phase 3: User Story 1 — Установка (P1) 🎯 MVP

**Goal**: один установщик на русском ставит Sorter на машину; удаление не трогает файлы пользователя.

**Independent Test**: quickstart §1 и §5.

- [X] T006 [US1] `src-tauri/windows/hooks.nsh`: `NSIS_HOOK_POSTINSTALL` — ключи `Directory\shell\Sorter` и `Directory\Background\shell\Sorter` под `SHCTX\Software\Classes` («MUIVerb» = «Разобрать с Sorter», `Icon` = `"$INSTDIR\${MAINBINARYNAME}.exe",0`, `MultiSelectModel` = `Single`, `command` — `"%1"` и `"%V"` соответственно), `SHChangeNotify`; `NSIS_HOOK_PREUNINSTALL` — удаление обоих ключей (contracts/ipc.md «Реестр»)
- [ ] T007 [US1] Собрать установщик, поставить, проверить quickstart §1: «Пуск», «Приложения» (знак, версия, издатель `stillmvd`), запуск; удалить без галочки данных — видео, стопки на месте, ключей реестра нет; поставить снова — колода, стол, журнал, подсказки на месте (§5)

**Checkpoint**: US1 проходит на установленной сборке.

---

## Phase 4: User Story 2 — Тихие обновления (P1)

**Goal**: скачивание в фоне, установка при закрытии или по «Перезапустить», блок в настройках.

**Independent Test**: quickstart §2 на локальном сервере.

- [X] T008 [P] [US2] Холст: сверить твик `update` кадра HSettings (https://claude.ai/artifact/DShPQtS1SVorfBa5UuxzqY) — состояния `latest`, `downloading`, `ready`, `failed`, `off` и «Что нового»; недостающие состояния дорисовать и показать пользователю до вёрстки
- [X] T009 [US2] `src-tauri/src/updates.rs`: `UpdateState { phase: "idle"|"checking"|"latest"|"downloading"|"ready"|"failed"|"off", version?, progress? «0–1», notes?, error?, checkedAt?, current }`; `PendingUpdate(Mutex<Option<(Update, Vec<u8>)>>)`; `check_and_download(app)` — только release, при `updates=off` фаза `off` без сети; прогресс в `update://state` не чаще 250 мс; при `ready` писать `update_version`, `update_notes` в `setting`
- [X] T010 [US2] `updates.rs`: планировщик — поток: проверка через 10 с после старта, затем раз в 24 ч; перепроверка ошибки при следующем цикле
- [X] T011 [US2] `updates.rs`: `install(app, restart)` — взять `AppState.db` (ждёт конец хода, research R2), затем `update.restart_after_install(restart).install(bytes)`; ошибка → фаза `failed` с `UPDATE_INSTALL`
- [X] T012 [US2] Тексты ошибок (contracts/ipc.md): нет сети → `UPDATE_NET`, подпись → `UPDATE_SIGNATURE`, установка/UAC → `UPDATE_INSTALL`; тест разбора ошибок updater в `updates.rs`
- [X] T013 [US2] `src-tauri/src/lib.rs`: состояние и `PendingUpdate` в `manage`; `on_window_event(CloseRequested)` главного окна → `install(app, false)`, если `ready`; запуск планировщика в `setup`
- [X] T014 [US2] `src-tauri/src/commands.rs`: `update_state`, `update_check`, `update_install`; `set_setting_cmd` принимает `updates` ∈ `on|off`
- [X] T015 [US2] `src/lib/ipc.ts`: тип `UpdateState`, вызовы, `onUpdateState`; версия — `getVersion()`
- [X] T016 [US2] `src/components/screens/Settings.tsx`: вместо `TODO(003)` — блок «Версия и обновления» по кадру (T008): версия, состояние словами, «Проверить сейчас», «Перезапустить» при `ready`, переключатель «Проверять обновления», «Что нового»; «Установлено X» после обновления, если текущая версия = `update_version`
- [ ] T017 [US2] Приёмка quickstart §2 (1–8): сборки 1.0.0 и 1.0.1 с `--config` на `http://localhost:8765/latest.json`, скачивание, замер `sc002.js` во время скачивания (p95 ≤ 300 мс, SC-003), UAC при закрытии, чужая подпись, отказ UAC, нет сети, выключенная проверка + сэмплер соединений

**Checkpoint**: US2 проходит на локальном сервере.

---

## Phase 5: User Story 3 — «Разобрать с Sorter» (P2)

**Goal**: папка из Проводника становится колодой в единственном окне.

**Independent Test**: quickstart §3.

- [X] T018 [US3] `src-tauri/src/lib.rs`: `folder_arg(args) -> Option<String>` — первый аргумент, если это существующая папка (кавычки, кириллица, пробелы, UNC); при старте → `AppState.incoming`; `single_instance` — окно вперёд (`unminimize`, `set_focus`) и `open://folder` с путём; тесты `folder_arg`
- [X] T019 [US3] `src-tauri/src/commands.rs`: `take_incoming` — отдаёт и очищает
- [X] T020 [US3] `src/App.tsx`: при загрузке `take_incoming`, подписка на `open://folder` → `ipc.chooseDeck` (ошибки вложенности — текстом, как в настройках, с предложением выбрать другой стол); нет стола → стартовый экран с заполненной колодой; идёт ход — переключение после него
- [X] T021 [P] [US3] Холст: кадр смены стола в окне, открытом из Проводника (FR-015), — показать пользователю, дождаться одобрения
- [X] T022 [US3] Вёрстка смены стола по кадру T021 (`choose_table`), файл — по кадру (`src/components/deck/DeckScreen.tsx` или `src/components/Titlebar.tsx`)
- [ ] T023 [US3] Приёмка quickstart §3 (1–5) на установленной сборке; SC-005 ≤ 5 с от клика до карты

**Checkpoint**: US3 проходит.

---

## Phase 6: User Story 4 — Первый запуск (P3)

**Goal**: понятные подписи плиток.

**Independent Test**: quickstart §4.

- [X] T024 [US4] `src/components/screens/Start.tsx`: заголовки плиток «Откуда брать видео» / «Куда раскладывать» и по строке пояснения; форма плиток прежняя
- [ ] T025 [US4] Приёмка quickstart §4 через CDP (dev с временной папкой данных)

---

## Phase 7: Polish & Release

- [ ] T026 [P] `.github/workflows/release.yml`: тег `v*`, `windows-latest`, pnpm, Rust cache, шаг `scripts/fetch-runtime.ps1`, `tauri-apps/tauri-action@v1` (`tagName: v__VERSION__`, `releaseName: Sorter __VERSION__`, `releaseBody` — «Что нового» на русском, `uploadUpdaterJson`, `updaterJsonPreferNsis`, секрет `TAURI_SIGNING_PRIVATE_KEY`)
- [ ] T027 [P] `README.md`: что делает Sorter, установка, обновления, пункт Проводника, где данные; без упоминаний ассистента
- [ ] T028 Предложить пользователю: создать публичный `stillmvd/sorter`, запушить, добавить секрет, тег `v1.0.0` — выполнять по шагам только после явного «да» (бывший T053 фичи 001)
- [ ] T029 После релиза: установщик со страницы релиза на чистую установку, `latest.json` доступен, приложение видит «последняя версия»; обновить `HANDOFF.md`

---

## Dependencies & Execution Order

- Setup (T001–T003) → Foundational (T004–T005) → US1 (T006–T007).
- US2 зависит от Foundational; T008 (холст) — до T016. US3 зависит от US1 (пункт реестра ставит установщик); T021 — до T022. US4 независима.
- Polish: T026–T027 в любой момент; T028–T029 — после приёмки US1–US3.

### Parallel Opportunities

- T002 ∥ T003; T008 ∥ T009–T014 (холст и Rust); T021 ∥ T018–T020; T024 ∥ всё в US2/US3; T026 ∥ T027.

## Implementation Strategy

MVP — US1 (установщик) → US2 (обновления: без них установщик теряет смысл) → US3 → US4 → релиз 1.0.0.
Каждая история принимается по своему разделу quickstart до перехода к следующей.
