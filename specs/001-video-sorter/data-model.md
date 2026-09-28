# Data Model: Sorter

Хранилище — один SQLite-файл `%APPDATA%\com.stillmvd.sorter\sorter.db` (WAL, `synchronous=FULL` — журнал ходов
переживает выключение питания). Кэш проявки — `%APPDATA%\com.stillmvd.sorter\cache\<card_id>\<idx>.jpg`.
В папках пользователя Sorter ничего не создаёт.

## Settings

| Поле | Тип | Правило |
|------|-----|---------|
| key | text PK | `deck_path`, `table_path`, `hints_enabled`, `theme` (`system`/`dark`/`light`), `muted`, `mode` (`deck`/`table`) |
| value | text | |

## Card — видео колоды

| Поле | Тип | Правило |
|------|-----|---------|
| id | integer PK | |
| deck_path | text | папка колоды, в которой карта появилась |
| file_name | text | имя в колоде; уникально вместе с `deck_path` пока `status in (in_deck, deferred)` |
| size | integer | байты; вместе с `mtime` — отпечаток: изменился → проявить заново |
| mtime | integer | unix ms |
| taken_at | integer? | дата съёмки из метаданных, иначе `mtime` |
| duration_ms | integer? | |
| width, height | integer? | уже с учётом поворота из метаданных |
| orientation | text? | `portrait` (h > w·1,05) / `landscape` (w > h·1,05) / `square` |
| frames | integer | число готовых кадров плёнки, 0–8 |
| stage | text | `new` → `meta` → `frames` → `embedded`; `broken` — проявить не удалось |
| error | text? | человеческая причина для `broken` |
| status | text | `in_deck` · `deferred` (Tab, в конце) · `placed` · `gone` (исчез из колоды извне) |
| position | real | порядок в колоде; Tab → `max + 1` |
| pile_id | integer? | где лежит, если `placed` |
| current_path | text? | полный путь сейчас (для `placed` — в стопке) |

Переходы `status`: `in_deck ⇄ deferred`, `in_deck|deferred → placed` (ход), `placed → in_deck` (забрать ход),
`in_deck|deferred → gone` (файл удалён извне). Проявка идёт независимо от `status`.

## Pile — стопка

| Поле | Тип | Правило |
|------|-----|---------|
| id | integer PK | |
| table_path | text | стол, к которому относится |
| name | text | имя подпапки первого уровня; уникально в пределах стола без учёта регистра; без `<>:"/\|?*` и завершающих точки/пробела |
| key | text? | `1`–`9`, `Q`…`P`, `A`…`L`, `Z`…`M`; уникальна в пределах стола; `Delete` — только у корзины |
| is_trash | bool | ровно одна на стол; папки на диске нет |
| ord | integer | порядок в ряду; корзина всегда последняя (`ord = 1 000 000`), в БД имя `:trash` |
| exists_on_disk | bool | папка есть на диске; `false` — стопка скрыта из ряда, ходы в журнале помечены «папки нет» |

Клавиши по умолчанию раздаются по `order`: `1…9`, затем `Q W E R T Y U I O P`. Назначение занятой клавиши
снимает её с прежней стопки.

## Move — ход (журнал)

| Поле | Тип | Правило |
|------|-----|---------|
| id | integer PK | |
| at | integer | unix ms |
| method | text | `key` · `hint` · `search` · `table` · `drag` · `new_pile` |
| pile_id | integer | куда |
| state | text | `pending` → `done`; `done` → `undoing` → `undone`; сбой → `failed` с `error` |
| error | text? | |

## MoveItem — файл в ходе

| Поле | Тип | Правило |
|------|-----|---------|
| move_id | integer FK | |
| card_id | integer FK | |
| from_path | text | исходный полный путь |
| to_path | text? | итоговый полный путь (с суффиксом при совпадении); для корзины — `null` |
| cross_volume | bool | копия + сверка + удаление исходника |
| step | text | `planned` → `copied` (только cross_volume) → `done`; при отмене `restoring` → `restored` |

Порядок записи: строка `Move(pending)` и все `MoveItem(planned)` фиксируются транзакцией **до** операций с файлами.
Восстановление при запуске: `pending` с `planned` → откат (файлы не трогались или исходник цел); `copied` →
сверить копию, удалить исходник, `done`; `undoing` → довести возврат.

Имя при совпадении: `имя (2).ext`, `имя (3).ext` — первое свободное.

## Example — пример для подсказок

| Поле | Тип | Правило |
|------|-----|---------|
| card_id | integer? FK | карта Sorter; `null` — файл, найденный в стопке при сканировании |
| pile_id | integer FK | |
| path | text | путь файла-примера |
| vector | blob | эмбеддинг (усреднённый по кадрам, L2-нормированный) |

Примеры появляются из двух мест: каждый ход в стопку (вектор карты уже посчитан) и фоновое сканирование видео,
которые уже лежали в стопках до Sorter (холодный старт). Забранный ход удаляет свой пример. Порог «учится»:
меньше 3 примеров в стопке — стопка не подсказывается.

## Hint — подсказка (вычисляется, не хранится)

`{ pile_id, score 0..1 }` × до 2. Счёт — сходство вектора карты (для пачки — среднего вектора отмеченных) с примерами
стопки: среднее из k = 3 ближайших. Показывается, если `score ≥ 0,5`; `n_examples` — общее число примеров.
