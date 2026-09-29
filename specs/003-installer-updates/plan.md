# Implementation Plan: Установщик и обновления

**Branch**: `003-installer-updates` | **Date**: 2026-09-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/003-installer-updates/spec.md`

## Summary

Sorter собирается NSIS-установщиком Tauri на всю машину (русский мастер, ярлыки, удаление с галочкой данных)
и обновляется из публичных GitHub Releases `stillmvd/sorter`: проверка при запуске и раз в сутки, скачивание
в фоне, установка при закрытии окна или по «Перезапустить» — схема Markdown. Хуки установщика добавляют
«Разобрать с Sorter» на папку и на фон папки; папка из аргументов (или из второго запуска через single-instance)
становится колодой тем же путём, что «Сменить» в настройках. Блок «Версия и обновления» в настройках — по кадру
HSettings. Первый релиз 1.0.0 — через GitHub Actions по тегу. Решения — [research.md](research.md).

## Technical Context

**Language/Version**: Rust (stable, edition 2021), TypeScript 5.8

**Primary Dependencies**: новые — `tauri-plugin-updater` 2 (Rust) и `@tauri-apps/api` `getVersion`; уже есть —
`tauri-plugin-single-instance`, `rusqlite`; сборка — NSIS бандлер Tauri, `tauri-apps/tauri-action@v1`

**Storage**: SQLite `setting` — ключи `updates`, `update_version`, `update_notes`; состояние обновления в памяти

**Testing**: `cargo test` — разбор аргумента запуска, переходы состояния обновления; ручная приёмка по
[quickstart.md](quickstart.md) с локальным сервером `latest.json`

**Target Platform**: Windows 11 x64

**Project Type**: desktop-app (Tauri)

**Performance Goals**: проверка и скачивание не влияют на смену карты (p95 ≤ 300 мс); окно из Проводника ≤ 5 с

**Constraints**: сеть только для обновлений и выключается (принцип III 3.0.0); ставится только подписанное и новее;
установка не прерывает ход

**Scale/Scope**: установщик ~70 МБ (модель DINOv2 и ORT внутри exe); одна линия релизов

## Constitution Check

| Принцип | Как соблюдён | Статус |
|---------|--------------|--------|
| I. Файлы в безопасности | установка ждёт блокировку базы — ход и журнал дописываются; установщик и удаление не трогают папки пользователя | ✅ |
| II. Одно решение — одно нажатие | обновления без участия; пункт Проводника — одно действие до колоды | ✅ |
| III. Установщик, приватно, сеть только для обновлений | сеть — `check`/`download` updater'а, выключается `updates=off`; подпись ключом проекта; данные в `%APPDATA%` | ✅ |
| IV. Дизайн семейства | блок «Версия и обновления» — по кадру HSettings; смена стола из окна Проводника — сначала кадр на холсте | ✅ (T до вёрстки) |
| V. Нативно и понятно | мастер Windows на русском, классический пункт меню, состояния обновления словами | ✅ |
| VI. Простота кода | стандартный шаблон NSIS + хуки, без форка; схема обновлений Markdown; выбор колоды — существующий `choose` | ✅ |

Повторная проверка после дизайна: нарушений нет.

## Project Structure

### Documentation (this feature)

```text
specs/003-installer-updates/
├── spec.md, plan.md, research.md, data-model.md, quickstart.md
├── contracts/ipc.md
└── checklists/requirements.md
```

### Source Code (repository root)

```text
src-tauri/
├── tauri.conf.json      # bundle nsis perMachine, createUpdaterArtifacts, plugins.updater, версия 1.0.0
├── windows/hooks.nsh    # «Разобрать с Sorter»: Directory\shell и Directory\Background\shell
├── Cargo.toml           # tauri-plugin-updater, версия
└── src/
    ├── updates.rs       # состояние, планировщик (старт + 24 ч), check/download, install с ожиданием хода
    ├── lib.rs           # плагин updater, incoming из args, single_instance → open://folder, CloseRequested → install
    └── commands.rs      # update_state, update_check, update_install, take_incoming, ключ updates
src/
├── lib/ipc.ts           # типы и вызовы
├── App.tsx              # take_incoming / open://folder → выбор колоды, стартовый экран без стола
└── components/screens/
    ├── Settings.tsx     # блок «Версия и обновления» вместо TODO(003)
    └── Start.tsx        # «Откуда брать видео» / «Куда раскладывать» с пояснениями
.github/workflows/release.yml   # тег v* → fetch-runtime → tauri-action → релиз с latest.json
```

## Порядок работ (для /speckit-tasks)

1. Установщик: конфиг бандла, хуки NSIS, версия 1.0.0, локальная сборка и установка (US1).
2. Аргумент запуска и single-instance → колода; стартовый экран с заполненной колодой (US3 без смены стола).
3. Холст: кадр смены стола в окне из Проводника; сверка твика `update` на HSettings → одобрение пользователя.
4. Rust: `updates.rs` + команды, ключ подписи, конфиг updater (US2), приёмка на локальном сервере.
5. Блок «Версия и обновления» в настройках; смена стола из окна (US3); подписи первого запуска (US4).
6. Репозиторий `stillmvd/sorter`, секрет, workflow; релиз 1.0.0 — только по явному «публикуй» пользователя
   (внешняя публикация); README без упоминаний ассистента. Прогон quickstart.

## Complexity Tracking

Нет нарушений.
