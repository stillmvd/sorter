# IPC-контракт фичи 005

## Изменённые команды и типы

| Команда / тип | Было | Стало |
|---------------|------|-------|
| `set_setting_cmd(key, value)` | ключи … | + `dupes_enabled` (`1`/`0` → `0` удаляет ключ), `dupes_scope` (`deck`/`all`) |
| `get_state` → `settings` | … | + `dupes_enabled`, `dupes_scope`, `dupe_search` (строка JSON или нет) |
| `Developing` (`develop://progress`, `get_state.developing`) | `pair` | − `pair`; + `scope: "off"\|"deck"\|"all"\|"search"`, `search: boolean`; этапа `dupes` нет при `scope = "off"` |
| `dupe_groups()` → `DupeGroups` | `groups, printed, total` | + `scope: "off"\|"deck"\|"all"\|"search"`, `search: SearchSummary \| null`; группы — только пары внутри набора |
| `GroupItem.where` | `"deck"\|"pile"` | + `"folder"`; для `folder` — `folder: string` (каталог файла) |
| `dupes_for(cardId)` | пары с колодой и всеми стопками | пары внутри набора колоды; при выключенном поиске — `[]` |
| `trash_copies(paths)` | пути колоды и стопок | без изменений сигнатуры; пути внешних папок — тем же ходом в «Корзину» |

## Новые команды

| Команда | Вид | Ответ | Смысл |
|---------|-----|-------|-------|
| `dupes_estimate(scope: "deck"\|"all")` | async | `{ files, videos, photos, minutes }` | оценка для чипа «+N мин» на старте и в настройках; `files` — без готового отпечатка |
| `search_preview(piles: string[], deck: boolean, folders: string[])` | async | `{ total, roots: { path, files, error?: string }[] }` | счёт до запуска (FR-015); `error` — «нет доступа» / «диск не найден» |
| `search_start(piles, deck, folders)` | sync | `void` | пишет `dupe_search` (`running`), будит цикл; заменяет прежние итоги |
| `search_clear()` | sync | `void` | удаляет `dupe_search`: Esc во время поиска и «Закрыть результаты» |

```ts
type SearchSummary = {
  piles: string[];
  deck: boolean;
  folders: string[];
  skipped: string[];
  state: "running" | "done";
  files: number;
};
```

## События

| Событие | Когда |
|---------|-------|
| `develop://progress` | как сейчас; `stage = "done"` + `search = true` — отдельный поиск закончен, шлюз показывает «К дублям Enter» |
| `dupes://changed` | как сейчас; и после `search_clear`, смены `dupes_enabled` / `dupes_scope` |
