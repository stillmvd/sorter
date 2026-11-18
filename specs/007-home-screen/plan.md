# Implementation Plan: Главный экран и разбор нескольких папок

**Branch**: `main` | **Date**: 2026-11-18 | **Spec**: [spec.md](spec.md)

## Summary

Вместо стартового экрана — главный экран по стенду 009: плитка «Папка для разбора» со списком корней, плитка стола с
переключателями, «Разобрать» (Enter) и «Продолжить разбор». Разбор — набор корней и стол (`review`, `review_root`);
карта по-прежнему живёт при корне, `file_name` становится путём от корня — склейки путей во всём коде не меняются.
Колода берёт все уровни вложенности, стол пропускается, вложенные корни поглощаются. Папки перетаскиваются из
Проводника, приходят из его меню, пропавшая папка больше не закрывает приложение. Решения — [research.md](research.md),
схема — [data-model.md](data-model.md), команды — [contracts/commands.md](contracts/commands.md).

## Technical Context

**Language/Version**: Rust 1.100 (edition 2024), TypeScript 7, React 19
**Primary Dependencies**: Tauri 2 (`onDragDropEvent`), rusqlite 0.43, notify 8 (рекурсивное наблюдение), trash
**Storage**: SQLite — новые `review`, `review_root`, представление `open_root`, `move.review_id`
**Testing**: `cargo test --release --lib` (`pnpm run check`), CDP-прогон dev на `G:\sorter-test` и копии `2ch`
**Target Platform**: Windows 10/11 x64
**Project Type**: desktop-app
**Performance Goals**: обход 5000 файлов в 40 папках ≤ 2 с на SSD, окно не замирает (обход в async-команде)
**Constraints**: ходы синхронные на основном соединении; журнал до перемещения; отключённый диск ≠ пропавший файл;
проявленное хранится (006)
**Scale/Scope**: до 10 корней, тысячи вложенных папок, 5000+ карт

## Constitution Check

| Принцип | Проверка | Итог |
|---|---|---|
| I. Файлы в безопасности | файлы только перемещаются; пустые папки — ходом в «Корзину» с журналом и отменой | ✓ |
| II. Одно решение — одно нажатие | перетащил — выбрано; «Продолжить» — Enter | ✓ |
| III. Приватно | сеть не нужна | ✓ |
| IV. Дизайн семейства | форма — утверждённый стенд 009 | ✓ |
| V. Нативно и понятно | перетаскивание из Проводника, меню Проводника | ✓ |
| VI. Простота | без новых зависимостей; карта при корне, склейки не меняются | ✓ |

## Project Structure

```text
src-tauri/src/
  db.rs            миграция: review, review_root, open_root, move.review_id, канонизация
  paths.rs         новый: canon, is_inside, walk (обход с пропуском стола и ссылок)
  review.rs        новый: черновик, add_roots, start/open/drop, список незаконченных, empty_dirs
  deck.rs          sync_seen по дереву корня, позиция по разбору, origin в CardView
  commands.rs      новые команды, удаление choose_deck/cancel_deck/choose_table, folder_arg → черновик
  moves.rs         basename в стопке, review_id хода, undo в рамках разбора, каталоги в корзину
  develop.rs, hints.rs, dupes/*, series.rs, watch.rs   deck_path из open_root
src/
  components/screens/Home.tsx      новый: главный экран (вместо Start.tsx)
  components/home/*                плитки, строки корней, «Продолжить», рамка перетаскивания
  components/deck/Facts.tsx        строка папки над именем
  components/screens/Done.tsx      «Убрать N пустых папок»
  App.tsx, lib/ipc.ts              маршрут главного экрана, Enter, home://incoming, onDragDropEvent
src-tauri/tauri.conf.json          dragDropEnabled: true
```

## Complexity Tracking

Нет нарушений.
