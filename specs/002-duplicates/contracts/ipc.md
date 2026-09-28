# IPC: Дубли

| Команда | Вход | Выход | Заметки |
|---------|------|-------|---------|
| `dupes_for` | `{ cardId }` | `DupeView[]` | совпадения карты с `confidence ≥ 80`, лучшая первой |
| `dupe_groups` | — | `{ groups: Group[], printed, total }` | для экрана «Дубли»; «Возможно» — группы 50–79 |
| `dismiss_dupe` | `{ a, b }` | — | «Это разные видео» |
| `trash_copies` | `{ paths }` | `Move` | один ход в «Корзину»; карты колоды и файлы стопок |
| `replace_copy` | `{ cardId, worsePath }` | `Move[]` | лучшая карта → стопка худшей, худшая → Корзина; общий `group_id` |

```ts
type DupeView = {
  path: string; where: "deck" | "pile"; pileName?: string; cardId?: number;
  kind: "exact" | "same" | "trim" | "crop"; confidence: number;
  offsetMs?: number; durationMs: number; width: number; height: number; bitrate: number; size: number;
  better: boolean;           // эта копия лучше текущей карты
};
type Group = { items: (DupeView & { best: boolean })[]; kind: DupeView["kind"]; confidence: number };
```

События: `dupes://changed` — появились новые совпадения (перезапросить плашку/экран); `develop://progress`
расширяется полем `printed` (отпечатано).

## Клавиши

| Клавиша | Где | Действие |
|---------|-----|----------|
| `D` | колода, есть плашка | убрать текущую карту в Корзину (если лучшая — вторая копия) / «Заменить» (если лучшая — текущая) |
| `C` | колода, есть плашка | открыть сравнение бок о бок |
| `Ctrl+3` | везде | экран «Дубли» |
| `Enter` | экран «Дубли» | оставить предвыбранную, остальные — в Корзину |
| `← →` | экран «Дубли» | выбрать другую копию группы |
| `N` | плашка / экран | «Это разные видео» |
