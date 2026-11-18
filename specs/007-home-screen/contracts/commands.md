# Contracts: команды Tauri

## Новые

| Команда | Вход | Выход | Ошибки |
|---|---|---|---|
| `home` (async) | — | `Home { draft: Draft, reviews: ReviewView[], last: Option<i64> }` | — |
| `add_roots` (async) | `paths: string[]` | `Draft` | `NOT_FOLDER`, `NO_MEDIA`, `INSIDE_TABLE`, `INSIDE_ROOT{name}`, `IN_REVIEW{id,label}` — по каждой папке в `Draft.errors`, команда целиком не падает |
| `remove_root` | `path` | `Draft` | — |
| `set_draft_table` (async) | `path` | `Draft` | `NOT_FOLDER`, `TABLE_INSIDE_ROOT` не ошибка — `Draft.table.excludes = true` |
| `stop_reading` | — | `()` | — |
| `start_review` | — | `ReviewView` | `NOT_READY` |
| `open_review` (async) | `id` | `ReviewView` | `NO_REVIEW` |
| `drop_review` | `id` | `()` | — |
| `recheck` (async) | — | `Home` | — |
| `empty_dirs` (async) | — | `string[]` | — |
| `trash_dirs` | `paths: string[]` | `Placed` | `FILE_NOT_THERE` |

```ts
type RootView = { path: string; name: string; dirs: number; video: number; photo: number; online: boolean; reading: number | null };
type Draft = { roots: RootView[]; table: { path: string; name: string; piles: number; online: boolean } | null; errors: { path: string; code: string; message: string; reviewId?: number }[] };
type ReviewView = { id: number; label: string; roots: RootView[]; table: string; left: number; total: number; openedAt: number; next: CardView | null; online: boolean };
```

## События

- `deck://reading` `{ path, found }` — по ходу обхода корня.
- `home://incoming` `Draft` — папка пришла из Проводника.

## Изменённые

- `choose_deck`, `cancel_deck`, `choose_table` — удаляются; их места занимают `add_roots`, `open_review`,
  `set_draft_table`. Возврат с колоды на главный экран — `close_review` (сбросить `review_id`, проявка продолжается
  до конца уже запрошенного).
- `CardView.origin: string` — «2ch › тред 312».
- `MoveView.review: string | null` — подпись разбора.
- `undo_last`, `undo_since` — только ходы открытого разбора.
- `StateView.missing` — только стол; корни — в `Home`/`ReviewView.online`.
