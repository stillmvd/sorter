# Data Model: Фото

## card (новые колонки, миграция в `db::init` через `pragma_table_info`)

| Колонка | Тип | Смысл |
|---------|-----|-------|
| `kind` | TEXT NOT NULL DEFAULT 'video' CHECK (kind IN ('video','photo')) | тип по расширению при `deck::sync` |
| `camera` | TEXT | «Make Model» из EXIF (Make не дублируется, если Model уже начинается с него) |
| `taken_from` | TEXT CHECK (taken_from IN ('exif','name','file')) | источник `taken_at` |
| `phash` | INTEGER | 64-битный pHash миниатюры (фото), для серий |
| `sharp` | REAL | резкость миниатюры (дисперсия лапласиана), для лучшего снимка серии |
| `series_id` | INTEGER | id первого снимка серии; NULL — не в серии |
| `solo` | INTEGER NOT NULL DEFAULT 0 | 1 — снимок вынут из серий («Разбить серию») |

Существующие: `taken_at` (для фото — по `taken_from`), `width`/`height` (после поворота), `orientation`,
`frames` (для фото 1), `duration_ms` (для фото NULL), `stage` (`new` → `frames` → `embedded`; `broken` с текстом).

Индекс: `card_series ON card(series_id) WHERE series_id IS NOT NULL`.

## Жизненный цикл серии

```
проявлены фото колоды → series::regroup(deck) → series_id у членов (≥ 2)
ход серии → отмеченные placed, остальные placed (Корзина) или in_deck → regroup
«Разбить» → solo = 1 у членов → regroup (series_id = NULL)
отмена хода → карты in_deck → regroup (solo сохраняется)
```

## CardView (IPC)

Добавляются `kind`, `camera`, `takenFrom`, `seriesId`.

## SeriesView (IPC)

`{ id, best, cards: CardView[] }` — `id` = `series_id`, `best` — id лучшего снимка, `cards` по времени.

## PileView (IPC)

`count` остаётся общим (видео + фото); добавляются `videos`, `photos`.

## settings

`kind_filter`: `all` | `video` | `photo` (по умолчанию `all`); `series_rest`: `trash` | `keep` (по умолчанию `trash`).

## fingerprint / dupe

Схема прежняя. Для фото: `duration_ms` = 0, `frames` = один pHash, `audio` пусто, `semantic` = один вектор,
`bitrate` = NULL. Вид `dupe.kind` для фото: `exact`, `same`, `crop`.
