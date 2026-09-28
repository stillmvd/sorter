# Contract: команды и события Rust ↔ интерфейс

Все команды — `invoke` Tauri 2, ответы JSON (`camelCase`). Ошибка — `{ code, message }`, `message` на русском,
пригоден для показа как есть. Типы полей — см. [data-model.md](../data-model.md).

## Команды

| Команда | Вход | Выход | Заметки |
|---------|------|-------|---------|
| `get_state` | — | `{ settings, deck: { total, left, placed }, piles: Pile[] }` | стартовый снимок |
| `choose_deck` / `choose_table` | `{ path }` | `{ ok, count? , problem? }` | проверка: не совпадают, стол не внутри колоды и наоборот |
| `set_setting` | `{ key, value }` | — | |
| `deck_window` | `{ from, count }` | `Card[]` | карты колоды по порядку, для колоды и стола |
| `place` | `{ cardIds, pileId, method }` | `Move` с итоговыми именами | один ход на пачку; ошибка по файлу — весь ход `failed`, уже перенесённые возвращаются |
| `defer` | `{ cardId }` | — | Tab |
| `undo_last` | — | `Move?` | Ctrl+Z |
| `undo_move` | `{ moveId }` | `Move` | из журнала; `FILE_NOT_THERE`, если файла на месте нет |
| `undo_today` | — | `{ undone, failed }` | |
| `journal` | `{ before?, limit }` | `Move[]` с items | |
| `create_pile` | `{ name }` | `Pile` | создаёт папку; `BAD_NAME` с перечнем запрещённых символов |
| `rename_pile` | `{ pileId, name }` | `Pile` | переименовывает папку, обновляет `to_path` в журнале |
| `set_pile_key` | `{ pileId, key }` | `Pile[]` | возвращает все, т.к. клавиша могла сняться с другой |
| `hints` | `{ cardIds }` | `Hint[]` | ≤ 2 |
| `develop_control` | `{ action: pause | resume }` | — | |
| `clear_cache` | — | `{ freedBytes }` | |
| `open_in_explorer` | `{ path }` | — | |

## События (Rust → интерфейс)

| Событие | Данные | Когда |
|---------|--------|-------|
| `develop://progress` | `{ done, total, stage, paused }` | не чаще 4 раз/с |
| `develop://card` | `Card` | карта получила кадры/сведения/вектор |
| `deck://changed` | `{ added: Card[], gone: number[] }` | слежение за папкой колоды |
| `piles://changed` | `Pile[]` | папки стола изменились извне |

## Медиа

Видео карты отдаётся в `<video>` по локальному пути через asset-протокол Tauri (с Range); кадры плёнки —
файлы кэша тем же протоколом. Область доступа протокола — только папки колоды, стола и кэша.
