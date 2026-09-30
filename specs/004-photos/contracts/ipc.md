# IPC-контракт фичи 004

## Изменённые команды

| Команда | Было | Стало |
|---------|------|-------|
| `deck_window(from, count)` | карты колоды | `deck_window(from, count, kind)`; `kind` = `all`/`video`/`photo`; из серии возвращается только первый снимок (`id = series_id`) |
| `get_state` | `deck: {total,left,placed}` | + `deck.byKind: {video: {left, placed}, photo: {left, placed}}` |
| `set_setting_cmd` | ключи … | + `kind_filter`, `series_rest` |
| `PileView` | `count` | + `videos`, `photos` |
| `CardView` | … | + `kind`, `camera`, `takenFrom`, `seriesId` |

## Новые команды

| Команда | Ответ | Смысл |
|---------|-------|-------|
| `deck_series()` | `SeriesView[]` | все серии текущей колоды |
| `place_series(seriesId, keep: number[], pileId, rest: "trash" \| "keep", method)` | `{ move, piles }` | `keep` пуст — вся серия в стопку; иначе отмеченные в стопку, остальные по `rest`; `move` — ход в стопку, группа общая |
| `split_series(seriesId)` | `CardView[]` | снимки серии становятся обычными картами |

## События

| Событие | Когда |
|---------|-------|
| `deck://series` | после пересчёта серий, если состав изменился — фронт перечитывает окно и серии |
