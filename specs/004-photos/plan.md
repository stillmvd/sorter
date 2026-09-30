# Implementation Plan: Фото

**Branch**: `004-photos` | **Date**: 2026-09-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/004-photos/spec.md`

## Summary

Фото становятся картами той же колоды: `card.kind` = `video` | `photo`, всё, что сейчас отбирает файлы по
`is_video`, берёт `is_media`. Проявка фото — декодер `image` (JPEG/PNG/WebP/GIF): размеры, EXIF (поворот, дата
съёмки, камера — свой разбор IFD из сырых байт, которые отдаёт `image`), миниатюра `0.jpg` с учётом поворота,
pHash и резкость для серий. Карта показывает оригинал через протокол `media` (WebView сам поворачивает по EXIF
и играет GIF). Подсказки: вектор DINOv2 по миниатюре фото в общем пространстве с видео. Дубли: тот же конвейер
`fingerprint` → `dupe`, для фото — один pHash и смысловой вектор, сравнение только внутри типа. Серии считаются
в Rust по колоде (время ≤ 10 с и pHash-близость), хранятся в `card.series_id`; ход серии — два хода одной группой
(`move.group_id`), отмена уже умеет. Фильтр типа — настройка `kind_filter` и параметр окна колоды.

## Technical Context

**Language/Version**: Rust (stable, edition 2021), TypeScript 5.8

**Primary Dependencies**: без новых крейтов: `image` 0.25 (+ фичи `png`, `webp`, `gif`), `ort` (DINOv2, уже
есть), `rusqlite`, `windows`; фронт — React, Tailwind, lucide

**Storage**: SQLite — новые колонки `card.kind`, `card.camera`, `card.taken_from`, `card.phash`, `card.sharp`,
`card.series_id`, `card.solo`; настройка `kind_filter`; `fingerprint` без изменений схемы

**Testing**: `cargo test` — разбор EXIF на байтах, серии на синтетике, ходы серии и отмена; игнорируемые тесты на
`G:\vk photos` и `G:\sorter-test\photos`; приёмка по [quickstart.md](quickstart.md) через CDP

**Target Platform**: Windows 11 x64

**Project Type**: desktop-app (Tauri 2)

**Performance Goals**: переход к следующей карте фото ≤ 100 мс p95 (SC-002, картинка декодируется заранее);
проявка фото ≤ 150 мс на снимок 10 Мп в release; серии пересчитываются ≤ 20 мс на 500 фото

**Constraints**: офлайн, всё локально (конституция III); файлы только перемещаются, удаление — ход в «Корзину»
(конституция I); HEIC/RAW/Live Photo — вне объёма

**Scale/Scope**: сотни фото в колоде; тестовые папки `G:\vk photos` (266) и синтетический набор серий и дублей

## Constitution Check

| Принцип | Как соблюдён | Статус |
|---------|--------------|--------|
| I. Файлы в безопасности | фото ходят тем же `move_with` с журналом; серия — два хода одной группой (стопка + Корзина), Ctrl+Z забирает оба; «оставить в колоде» не двигает файлы | ✅ |
| II. Одно решение — одно нажатие | карта фото и карта серии — клавиша стопки / Enter; следующая картинка декодируется заранее; проявка фоном | ✅ |
| III. Установщик, приватно | декодер, EXIF, DINOv2 — локально; сеть не нужна | ✅ |
| IV. Дизайн семейства | кадры `HPhoto`, `HSeries`, `HPhotoTable` утверждены, числа — [contracts/ui.md](contracts/ui.md) | ✅ |
| V. Нативно и понятно | «Дата файла — даты съёмки в файле нет», пустой фильтр предлагает переключиться, клавиши подписаны | ✅ |
| VI. Простота кода | без новых крейтов: EXIF — ~50 строк разбора IFD из байт, которые уже отдаёт `image`; серии — один проход по отсортированному списку | ✅ |

## Project Structure

### Documentation (this feature)

```text
specs/004-photos/
├── spec.md, plan.md, research.md, data-model.md, quickstart.md
├── contracts/ui.md, contracts/ipc.md
└── checklists/requirements.md
```

### Source Code (repository root)

```text
src-tauri/src/
├── deck.rs            # kind, is_media/is_photo, окно с фильтром и сворачиванием серий, счётчики по типам
├── photo.rs           # новое: декод, EXIF (поворот, дата, камера), миниатюра, pHash, резкость
├── series.rs          # новое: сборка серий по колоде, лучший снимок, разбить
├── develop.rs         # проявка фото через photo.rs, пересчёт серий после партии
├── hints.rs           # кадры карты фото (0.jpg), холодные примеры из фото стопок
├── dupes/{mod,near,print}.rs  # фото в files(), отпечаток фото, сравнение внутри типа, пары одной серии не дубли
├── moves.rs           # place_series — два хода одной группой
├── piles.rs           # счёт видео и фото в стопке
├── media.rs           # mime фото, картинки отдаются целиком
└── commands.rs        # deck_window(kind), deck_series, place_series, split_series
src/
├── lib/ipc.ts
└── components/
    ├── deck/{Card,DeckScreen,PilesRow,LastMove}.tsx, deck/PhotoFacts.tsx, deck/SeriesPanel.tsx
    ├── table/{ContactSheet,Tile}.tsx
    ├── screens/Done.tsx
    └── ui/KindFilter.tsx
scripts/make-photo-set.py   # синтетический набор: серии, дубли, EXIF-поворот, без EXIF, GIF, PNG
```

**Structure Decision**: тот же одиночный Tauri-проект; два новых модуля Rust (`photo.rs`, `series.rs`), чтобы
не раздувать `develop.rs` и `deck.rs`.

## Порядок работ (для /speckit-tasks)

1. Набор `G:\sorter-test\photos` (скрипт) и схема: колонки `card`, `is_media`, `kind`.
2. `photo.rs` с тестами на байтах EXIF → проявка фото, `media.rs`, окно с фильтром → US1 карта фото и US2 фильтр.
3. Стопки со счётом по типам, стол с фото.
4. `series.rs` + `place_series` / `split_series` → US3 карта серии, плитка серии.
5. Подсказки фото (US4) → дубли фото (US5).
6. Замеры SC-002, приёмка quickstart через CDP.

## Complexity Tracking

Нет нарушений.
