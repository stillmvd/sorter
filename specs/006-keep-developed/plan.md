# Implementation Plan: Проявленное не стирается

**Branch**: `main` | **Date**: 2026-11-18 | **Spec**: [spec.md](spec.md)

## Summary

Повторное открытие папки не проявляет заново то, что уже проявлено. Убираем все пути, которые стирают проявленное
при смене колоды и выходе с экрана проявки, сверка узнаёт файлы по имени, размеру и времени (уже так), проявщик
работает только для открытой колоды, «Очистить кэш» не трогает открытую колоду и решения пользователя. Попутно —
сверка без мьютекса основного соединения (M5) и временный сбой чтения не делает карту битой (M7). Схема базы не
меняется. Решения — [research.md](research.md).

## Technical Context

**Language/Version**: Rust 1.100 (edition 2024), TypeScript 7, React 19
**Primary Dependencies**: Tauri 2, rusqlite 0.43, Media Foundation (проявка), ort (подсказки)
**Storage**: SQLite (`card`, `fingerprint`, `dupe`, `dupe_dismissed`, `embedding`, `example`, `settings`), кадры
`<data>/cache/<card_id>/<i>.jpg`
**Testing**: `cargo test --release --lib` (через `pnpm run check`), CDP-прогон dev
**Target Platform**: Windows 10/11 x64
**Project Type**: desktop-app
**Performance Goals**: повторное открытие проявленной папки 2000+ видео — первая карта ≤ 5 с (SC-001)
**Constraints**: ходы синхронные на основном соединении; журнал хода — до перемещения; отключённый диск ≠ пропавший
файл (`deck::surely_gone`)
**Scale/Scope**: колоды до 5000 файлов, кеш — сотни МБ на тысячи видео

## Constitution Check

| Принцип | Проверка | Итог |
|---|---|---|
| I. Файлы в безопасности | файлы пользователя не трогаются; меняется только своё: кадры и строки базы | ✓ |
| II. Одно решение — одно нажатие | повторное открытие — без экрана проявки; «Очистить кэш» — два нажатия как подтверждение необратимого | ✓ |
| III. Приватно | без сети | ✓ |
| IV. Дизайн семейства | новых экранов нет; плитка «Кэш проявки» прежняя, меняется только подпись кнопки на подтверждение | ✓ |
| V. Нативно и понятно | окно не замирает на сверке (async + своё соединение) | ✓ |
| VI. Простота кода | удаляем `forget_deck` и семидневную чистку, новых таблиц нет | ✓ |

После дизайна — без изменений.

## Project Structure

### Documentation (this feature)

```text
specs/006-keep-developed/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/commands.md
└── tasks.md            # /speckit-tasks
```

### Source Code

```text
src-tauri/src/
├── commands.rs   # choose_deck/choose_table async, cancel_deck без стирания, clear_cache(open_deck)
├── develop.rs    # удалить forget_deck; tidy без 7 дней; next только открытой колоды; save без broken при недоступной папке; clear_cache
├── deck.rs       # sync_seen подхватывает вернувшиеся gone
└── watch.rs      # повтор сверки по pending из choose_deck
src/
├── App.tsx                         # cancelDevelop → главный экран без потери
└── components/screens/Settings.tsx # подтверждение «Очистить кэш»
```

**Structure Decision**: существующая раскладка Tauri, новых модулей нет.

## Complexity Tracking

Нарушений нет.
