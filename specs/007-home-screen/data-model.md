# Data Model: Главный экран и разбор нескольких папок

## Новые таблицы (миграция в `db::init`)

```sql
CREATE TABLE review (
  id INTEGER PRIMARY KEY,
  table_path TEXT,
  started INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  opened_at INTEGER
);
CREATE UNIQUE INDEX review_draft ON review(started) WHERE started = 0;
CREATE TABLE review_root (
  review_id INTEGER NOT NULL REFERENCES review(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  ord INTEGER NOT NULL,
  online INTEGER NOT NULL DEFAULT 1
);
CREATE UNIQUE INDEX review_root_path ON review_root(path COLLATE NOCASE);
CREATE VIEW open_root AS
  SELECT path FROM review_root
  WHERE online = 1 AND review_id = (SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'review_id');
ALTER TABLE move ADD COLUMN review_id INTEGER;
```

- Черновик — не больше одной строки `started = 0` (индекс `review_draft`).
- Незаконченный разбор — `started = 1` и есть карты `in_deck|deferred` в его корнях.
- `settings.review_id` — открытый разбор; `settings.deck_path` после миграции не используется и удаляется.
- `settings.table_path` остаётся: стол открытого разбора (= `review.table_path`), им пользуются стопки и ходы.

## Изменения `card`

- `deck_path` — канонический путь корня. `file_name` — путь от корня, разделитель `\` (`тред 312\a.mp4`); у карт
  до фичи — просто имя.
- Позиция новой карты — `MAX(position)` по всем корням её разбора + 1 (R13).

## Миграция существующей базы

1. Канонизировать `card.deck_path`, `pile.table_path`, `settings.table_path` (только доступные папки; конфликт
   уникальности — оставить как есть).
2. Если есть `settings.deck_path`: `review(started=1, table_path, created_at=opened_at=now)` + корень; `review_id` =
   новый id; все `move.review_id` = этот id; удалить `settings.deck_path`.
3. Создать пустой черновик со столом из `settings.table_path`.

## Производные значения

| Значение | Откуда |
|---|---|
| Корень на плитке: папок, видео, фото | `card` корня `in_deck|deferred` по `kind`; папок — число разных родителей в `file_name` |
| Итог плитки «N карт» | сумма по доступным корням черновика |
| Остаток разбора «осталось X из Y» | X — `in_deck|deferred`, Y — X + `placed` с `move.review_id` разбора |
| Кадр «Продолжить» | первая карта колоды разбора по позиции |
| Папка на карте | имя корня + родители из `file_name` через « › » |
