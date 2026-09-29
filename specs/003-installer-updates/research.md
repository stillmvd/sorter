# Research: Установщик и обновления

## R1. Обновления — `tauri-plugin-updater`, схема Markdown

- **Decision**: `tauri-plugin-updater` 2. `check()` → `download()` байтами в память → `install()` при закрытии окна
  или по «Перезапустить» (`restart_after_install(true)`). Проверка при запуске и по таймеру раз в 24 ч; только
  в release-сборке. `plugins.updater.windows.installMode = "quiet"`.
- **Rationale**: ровно так работает Markdown (`src-tauri/src/updates.rs`: `PendingUpdate`, `update_prepare`,
  `install_pending`) — схема проверена в бою. Установщик ставится на всю машину, поэтому `quiet` всё равно
  поднимает запрос UAC — это единственное окно, как решено в уточнениях.
- **Alternatives**: `passive` (Booked, Echo Studio) — лишняя полоска прогресса; скачивание на диск — лишнее, 70 МБ
  в памяти на короткое время допустимы; ставить сразу после скачивания — ломает FR-007 (сам перезапускается).

## R2. Ход во время установки

- **Decision**: `update_install` сначала берёт `AppState.db` (тот же мьютекс, под которым идёт ход и пишется журнал),
  затем вызывает `install`. При закрытии окна — то же через `on_window_event(CloseRequested)`.
- **Rationale**: ход всегда выполняется под блокировкой базы; взяв её, мы ждём конец хода без новых механизмов.
  Журнал пишется до хода (принцип I), так что даже обрыв доводится при следующем запуске.

## R3. Подпись и публикация

- **Decision**: ключ `pnpm tauri signer generate -w ~/.tauri/sorter.key` без пароля (как Markdown), публичный —
  в `plugins.updater.pubkey`, приватный — секрет `TAURI_SIGNING_PRIVATE_KEY` репозитория. Endpoint
  `https://github.com/stillmvd/sorter/releases/latest/download/latest.json`. Релиз — GitHub Actions
  `tauri-apps/tauri-action@v1` по тегу `v*` (`uploadUpdaterJson`, `updaterJsonPreferNsis`), тело релиза —
  «Что нового» на русском.
- **Особенность Sorter**: `src-tauri/resources/` вне git, а `hints.rs` вшивает ORT, DirectML и DINOv2 через
  `include_bytes!` → в workflow шаг `scripts/fetch-runtime.ps1` до сборки. Менеджер — `pnpm`, не `npm`.
- **Alternatives**: локальная сборка и ручная загрузка — нет `latest.json` с подписью без ручной работы.

## R4. Установщик — стандартный шаблон Tauri + хуки

- **Decision**: `bundle.targets: ["nsis"]`, `createUpdaterArtifacts: true`, `nsis.installMode: "perMachine"`,
  `languages: ["Russian"]`, `displayLanguageSelector: false`, `installerIcon`, `installerHooks: "./windows/hooks.nsh"`.
  WebView2 — по умолчанию (`downloadBootstrapper`: ставится, если нет).
- **Rationale**: мастер с выбором ярлыка на рабочем столе и галочкой «удалить данные приложения» при удалении уже
  есть в стандартном шаблоне (FR-002, FR-004). Форк шаблона Echo Studio нужен ради установки без мастера — Sorter
  это не требует. Хуки — как у Markdown.
- **Alternatives**: форк `installer.nsi` — лишняя ноша при обновлении `@tauri-apps/cli`.

## R5. Пункт Проводника

- **Decision**: в `NSIS_HOOK_POSTINSTALL` под `SHCTX\Software\Classes`:
  - `Directory\shell\Sorter` — `MUIVerb` «Разобрать с Sorter», `Icon` `sorter.exe,0`, `MultiSelectModel` `Single`,
    `command` → `"$INSTDIR\sorter.exe" "%1"`;
  - `Directory\Background\shell\Sorter` — то же, `command` → `"$INSTDIR\sorter.exe" "%V"`;
  - `SHChangeNotify`. В `NSIS_HOOK_PREUNINSTALL` — удаление обоих ключей.
- **Rationale**: классический пункт (решение пользователя: Windows 11 показывает его под «Показать дополнительные
  параметры»). `MultiSelectModel=Single` прячет пункт при нескольких выделенных папках.
- **Alternatives**: `IExplorerCommand` + sparse MSIX — отклонено пользователем.

## R6. Папка в аргументах и одно окно

- **Decision**: при старте `std::env::args().nth(1)` → если это папка, кладётся в `AppState.incoming`; фронт при
  загрузке забирает её `take_incoming` и выбирает колоду тем же путём, что «Сменить» в настройках (проверки
  вложенности — `commands::choose`). Уже открытый Sorter получает путь через `single_instance` (плагин уже
  подключён) и шлёт событие `open://folder`; окно выходит вперёд. Нет стола → стартовый экран с заполненной колодой.
- **Rationale**: один путь выбора колоды — одни проверки и тексты ошибок.

## R7. Данные и версия

- **Decision**: `identifier` прежний → `%APPDATA%\com.stillmvd.sorter` подхватывается portable-копией и установкой
  одинаково. Первый релиз — `1.0.0` (синхронно `tauri.conf.json`, `Cargo.toml`, `package.json`). Версия в интерфейсе —
  `getVersion()`.
- **«Что нового»**: при скачивании в настройки пишутся `update_version` и `update_notes` (тело релиза). После
  перезапуска, если текущая версия = `update_version`, блок показывает «Установлено 1.2.0» и заметки.

## R8. Сеть

- **Decision**: настройка `updates` (`on` по умолчанию). При `off` планировщик не вызывает `check()` —
  других сетевых вызовов у Sorter нет (SC-007 фичи 001, фоновая сеть WebView2 уже отключена флагами).

## R9. Смена стола из окна Проводника (FR-015)

- **Decision**: формы нет на холсте → сначала кадр на холсте и одобрение пользователя, потом код (CLAUDE.md проекта).
  Смена выполняется тем же `choose_table`.

## R10. Проверка обновлений без публикации

- **Decision**: для приёмки — локальный сервер с `latest.json` и подписанной сборкой N+1; в тестовой сборке
  endpoint подменяется через `--config` на `http://localhost:8765/latest.json`
  (`plugins.updater.dangerousInsecureTransportProtocol: true` только в этой подмене).
- **Rationale**: публичный релиз — внешняя публикация, делается один раз для 1.0.0 по разрешению пользователя.
