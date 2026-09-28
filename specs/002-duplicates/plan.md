# Implementation Plan: Дубли

**Branch**: `002-duplicates` | **Date**: 2026-09-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/002-duplicates/spec.md`

## Summary

Фоновый отпечаток каждого видео колоды и стопок из пяти составляющих (хеш файла, сведения, кадровые хеши во
времени, смысловые векторы DINOv2, звуковой отпечаток) и сравнение с выравниванием по времени. Совпадения
сводятся в группы с видом, уверенностью и лучшей копией. В колоде — плашка на карте и сравнение бок о бок; экран
«Дубли» — разбор группами. Убрать копию — ход в «Корзину»; «Заменить» — два хода одной группой. Правило решения
и пороги подобраны прототипом на 195 видео — [research.md](research.md).

## Technical Context

**Language/Version**: Rust (stable, edition 2021), TypeScript 5.8

**Primary Dependencies**: без новых: `windows` (Media Foundation — кадры и звук; `sha2` — хеш файла, лёгкая
чистая зависимость), `ort` (DINOv2, уже есть), `rusqlite`; фронт — React, Tailwind

**Storage**: SQLite — таблицы `fingerprint`, `dupe`, `dupe_dismissed`; колонка `move.group_id`

**Testing**: `cargo test` — сравнение последовательностей и правило решения на синтетике; игнорируемый тест на
наборе `G:\sorter-test\variants` + реальные пары из research; ручная приёмка по [quickstart.md](quickstart.md)

**Target Platform**: Windows 11 x64

**Project Type**: desktop-app (Tauri)

**Performance Goals**: отпечаток минутного видео ≤ 3 с в release (SC-004); сравнение новой карты с 5 000
отпечатками ≤ 200 мс; раскладка не подтормаживает (фон ниже проявки и подсказок)

**Constraints**: офлайн; без ffmpeg в поставке; удаление только ходом в «Корзину»

**Scale/Scope**: до ~5 000 видео; отпечаток ≈ 2–5 КБ на минуту видео

## Constitution Check

| Принцип | Как соблюдён | Статус |
|---------|--------------|--------|
| I. Файлы в безопасности | лишняя копия — ход в «Корзину» через журнал; «Заменить» — два хода одной группой, отмена забирает оба; ничего не удаляется мимо корзины | ✅ |
| II. Одно решение — одно нажатие | плашка с клавишей на карте; группа на экране «Дубли» — Enter; отпечатки фоном, не блокируют раскладку | ✅ |
| III. Portable, offline, приватно | Media Foundation и DINOv2 локально; ffmpeg только для тестовых вариантов у разработчика | ✅ |
| IV. Дизайн семейства | плашка, сравнение, экран «Дубли» — сначала холст | ✅ (T до вёрстки) |
| V. Нативно и понятно | вид совпадения и уверенность словами («обрезка: 0:12–0:19 из 0:46») | ✅ |
| VI. Простота кода | без индексов-библиотек: свои 4 куска по 16 бит в HashMap; правило решения — явные пороги | ✅ |

## Project Structure

### Documentation (this feature)

```text
specs/002-duplicates/
├── spec.md, plan.md, research.md, data-model.md, quickstart.md
├── contracts/ipc.md
└── checklists/requirements.md
```

### Source Code (repository root)

```text
src-tauri/src/
├── dupes/mod.rs        # правило решения, группы, лучшая копия, команды-запросы
├── dupes/print.rs      # отпечаток: MF кадры 4/с + звук, срез полей, pHash, аудио-хеш, DINOv2 1/с
├── dupes/matcher.rs    # выравнивание последовательностей, индекс кандидатов
├── develop.rs          # очередь отпечатков после подсказок; уборка отпечатков
├── moves.rs            # group_id, undo группой
└── commands.rs         # dupes_for, dupe_groups, dismiss_dupe, resolve_dupe
src/components/
├── deck/DupeBadge.tsx  # плашка на карте
├── dupes/Compare.tsx   # сравнение бок о бок, синхронное воспроизведение
└── dupes/DupesScreen.tsx
```

## Порядок работ (для /speckit-tasks)

1. Холст: плашка на карте, сравнение, экран «Дубли» — до вёрстки.
2. Rust: схема, `matcher` с тестами на синтетике, `print` (кадры + звук + DINOv2), замер SC-004.
3. Очередь в фоне, сравнение, группы, лучшая копия; игнорируемый тест на `variants` (SC-002) и на всей папке (SC-001, SC-003).
4. Ходы: `group_id`, «Заменить», «Убрать все точные».
5. US1 плашка (точные) → US2 сравнение и прочие виды → US3 экран «Дубли».

## Complexity Tracking

Нет нарушений.
