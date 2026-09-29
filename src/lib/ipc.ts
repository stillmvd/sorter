import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Orientation = "portrait" | "landscape" | "square";

export type Card = {
  id: number;
  fileName: string;
  path: string;
  size: number;
  takenAt: number | null;
  durationMs: number | null;
  width: number | null;
  height: number | null;
  orientation: Orientation | null;
  frames: number;
  stage: "new" | "meta" | "frames" | "embedded" | "broken";
  error: string | null;
  status: "in_deck" | "deferred" | "placed" | "gone";
};

export type Pile = {
  id: number;
  name: string;
  key: string | null;
  isTrash: boolean;
  count: number;
  examples: number;
};

export type Method = "key" | "hint" | "search" | "table" | "drag" | "new_pile";

export type MoveItem = {
  cardId: number | null;
  fileName: string;
  finalName: string | null;
  fromPath: string;
  toPath: string | null;
};

export type Move = {
  id: number;
  at: number;
  method: Method;
  pileId: number;
  pileName: string;
  isTrash: boolean;
  state: "pending" | "done" | "undoing" | "undone" | "failed";
  error: string | null;
  items: MoveItem[];
};

export type Hint = { pileId: number; score: number };

export type Hints = { enabled: boolean; ready: boolean; examples: number; hints: Hint[] };

export type DupeKind = "exact" | "same" | "trim" | "crop";

export type GroupItem = {
  path: string;
  where: "deck" | "pile";
  pileName: string | null;
  cardId: number | null;
  durationMs: number | null;
  width: number | null;
  height: number | null;
  bitrate: number | null;
  size: number;
  best: boolean;
};

export type DupeGroup = {
  kind: DupeKind;
  confidence: number;
  trim: [number, number, number] | null;
  items: GroupItem[];
  pairs: [string, string][];
};

export type DupeGroups = { groups: DupeGroup[]; printed: number; total: number };

export type DupeView = {
  path: string;
  where: "deck" | "pile";
  pileName: string | null;
  cardId: number | null;
  kind: DupeKind;
  confidence: number;
  offsetMs: number | null;
  durationMs: number | null;
  width: number | null;
  height: number | null;
  bitrate: number | null;
  size: number;
  takenAt: number | null;
  visual: number | null;
  audio: number | null;
  semantic: number | null;
  better: boolean;
};

export type Settings = Partial<Record<"deck_path" | "table_path" | "hints_enabled" | "theme" | "muted" | "mode" | "volume", string>>;

export type Developing = { done: number; total: number; paused: boolean; printed: number };

export type AppState = {
  cacheDir: string;
  developing: Developing;
  settings: Settings;
  deck: { total: number; left: number; placed: number };
  piles: Pile[];
};

export type AppError = { code: string; message: string };

export function errorText(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as AppError).message);
  return String(e);
}

export const media = (path: string) => convertFileSrc(path, "media");

export const frameSrc = (cacheDir: string, card: Card, i: number) =>
  `${media([cacheDir, card.id, `${i}.jpg`].join("\\"))}?s=${card.stage}${card.frames}`;

export const onCard = (fn: (card: Card) => void) => listen<Card>("develop://card", (e) => fn(e.payload));
export const onDeckChanged = (fn: (d: { added: Card[]; gone: number[] }) => void) =>
  listen<{ added: Card[]; gone: number[] }>("deck://changed", (e) => fn(e.payload));
export const onPilesChanged = (fn: (p: Pile[]) => void) => listen<Pile[]>("piles://changed", (e) => fn(e.payload));
export const onHintsChanged = (fn: () => void) => listen("hints://changed", () => fn());
export const onDupesChanged = (fn: () => void) => listen("dupes://changed", () => fn());
export const onProgress = (fn: (p: Developing) => void) => listen<Developing>("develop://progress", (e) => fn(e.payload));

export const ipc = {
  getState: () => invoke<AppState>("get_state"),
  chooseDeck: (path: string) => invoke<{ count: number; bytes: number }>("choose_deck", { path }),
  chooseTable: (path: string) => invoke<Pile[]>("choose_table", { path }),
  setSetting: (key: string, value: string) => invoke<void>("set_setting_cmd", { key, value }),
  deckWindow: (from: number, count: number) => invoke<Card[]>("deck_window", { from, count }),
  place: (cardIds: number[], pileId: number, method: Method) =>
    invoke<{ move: Move; piles: Pile[] }>("place", { cardIds, pileId, method }),
  defer: (cardId: number) => invoke<void>("defer", { cardId }),
  undoLast: () => invoke<{ move: Move | null; cards: Card[]; piles: Pile[] }>("undo_last"),
  undoMove: (moveId: number) => invoke<{ move: Move | null; cards: Card[]; piles: Pile[] }>("undo_move", { moveId }),
  undoSince: (since: number) => invoke<{ undone: number; failed: string[] }>("undo_since", { since }),
  journal: (before: number | null, limit: number) => invoke<Move[]>("journal", { before, limit }),
  journalStats: () => invoke<{ moves: number; cards: number; hinted: number }>("journal_stats"),
  createPile: (name: string) => invoke<Pile[]>("create_pile", { name }),
  renamePile: (pileId: number, name: string) => invoke<Pile[]>("rename_pile", { pileId, name }),
  setPileKey: (pileId: number, key: string | null) => invoke<Pile[]>("set_pile_key", { pileId, key }),
  removePile: (pileId: number) => invoke<Pile[]>("remove_pile", { pileId }),
  hints: (cardIds: number[]) => invoke<Hints>("hints", { cardIds }),
  dupesFor: (cardId: number) => invoke<DupeView[]>("dupes_for", { cardId }),
  dismissDupe: (a: string, b: string) => invoke<void>("dismiss_dupe", { a, b }),
  dupeGroups: () => invoke<DupeGroups>("dupe_groups"),
  playable: (cardId: number) => invoke<string>("playable", { cardId }),
  trashCopies: (paths: string[]) => invoke<{ move: Move; piles: Pile[] }>("trash_copies", { paths }),
  replaceCopy: (cardId: number, worsePath: string) =>
    invoke<{ move: Move; piles: Pile[] }>("replace_copy", { cardId, worsePath }),
  developControl: (pause: boolean) => invoke<void>("develop_control", { pause }),
};
