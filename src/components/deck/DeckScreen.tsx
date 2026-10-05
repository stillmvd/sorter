import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { Copy, CopyX, Folder, FolderOpen, History, Image as ImageIcon, LayoutGrid, Layers, Loader2, Plus, Search, Settings, Video } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent } from "react";
import { errorText, ipc, mergeCards, onCards, onDeckChanged, onDupesChanged, onPilesChanged, onProgress, onSeries, type AppState, type Card, type DupeGroup, type DupeGroups, type DupeView, type GroupItem, type Kind, type Method, type Move, type Pile, type SeriesView } from "../../lib/ipc";
import { SeriesPanel, seriesSpan, type Rest } from "./SeriesPanel";
import { KindFilter } from "../ui/KindFilter";
import { isPhotoPath } from "../../lib/ipc";
import { PhotoFacts } from "./PhotoFacts";
import { isTypingChar, keyOf } from "../../lib/keys";
import { cardsWord, plural } from "../../lib/plural";
import { useHints } from "../../lib/useHints";
import { useDupes } from "../../lib/useDupes";
import { HintBox } from "./HintBox";
import { Done } from "../screens/Done";
import { IconButton } from "../ui/IconButton";
import { DupeBadge } from "./DupeBadge";
import { play } from "../../lib/sound";
import { pressToDrag } from "../fx/Drag";
import { Flights, motionOff, pileElement, shake, snapshot, type Flight } from "../fx/Flight";
import { FilmStrip, seekFrame } from "./FilmStrip";
import { KeysHint } from "./KeyLegend";
import { PileMenu } from "./PileMenu";
import { VolumeRow } from "./VolumeRow";
import { Button } from "../ui/Button";
import { PageHeader, Tag } from "../ui/PageHeader";
import { SearchField } from "../ui/SearchField";
import { CardStack } from "./Card";
import { LastMove, LastMoveLine } from "./LastMove";
import { ContactSheet } from "../table/ContactSheet";
import { Segment } from "../ui/Segment";
import { PilesRow } from "./PilesRow";
import { Compare } from "../dupes/Compare";
import { DupesScreen, SURE, groupTitle, type Filter } from "../dupes/DupesScreen";
import { GroupCompare } from "../dupes/GroupCompare";
import { DupesOff } from "../dupes/DupesOff";
import { EnableDupes } from "../dupes/EnableDupes";
import { FindDupes, type SearchSet } from "../dupes/FindDupes";
import { SearchBanner } from "../dupes/SearchBanner";
import { dupesSetting, widened, type DupesSetting } from "../screens/DupesStep";

const mb = (b: number) => `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ`;
const date = (ms: number) => new Date(ms).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric" });

type Mode = "deck" | "table" | "dupes";

const modes = (dupes: number): { value: Mode; label: string; hint: string }[] => [
  { value: "deck", label: "Колода", hint: "По одной карте — Ctrl 1" },
  { value: "table", label: "Стол", hint: "Пачкой — Ctrl 2" },
  { value: "dupes", label: dupes ? `Дубли ${dupes}` : "Дубли", hint: "Все дубли разом — Ctrl 3" },
];

const MODE_KEYS: Record<string, Mode> = { Digit1: "deck", Digit2: "table", Digit3: "dupes" };

type Counts = AppState["deck"];

function shift(c: Counts, list: Card[], sign: number): Counts {
  const byKind = { video: { ...c.byKind.video }, photo: { ...c.byKind.photo } };
  for (const card of list) {
    byKind[card.kind].left -= sign;
    byKind[card.kind].placed += sign;
  }
  return { ...c, left: c.left - sign * list.length, placed: c.placed + sign * list.length, byKind };
}

const KIND_WORD: Record<Kind, string> = { all: "", video: "видео", photo: "фото" };

function rank(piles: Pile[], query: string) {
  const q = query.trim().toLowerCase();
  if (!q) return null;
  return piles
    .filter((p) => !p.isTrash && p.name.toLowerCase().includes(q))
    .sort((a, b) => Number(!a.name.toLowerCase().startsWith(q)) - Number(!b.name.toLowerCase().startsWith(q)));
}

export function DeckScreen({
  initial,
  onPiles,
  onJournal,
  onSettings,
  onHome,
  onWiden,
  onSearch,
  banner,
}: {
  initial: AppState;
  onPiles: () => void;
  onJournal: () => void;
  onSettings: () => void;
  onHome: () => void;
  onWiden: (before: DupesSetting) => void;
  onSearch: () => void;
  banner?: React.ReactNode;
}) {
  const [cards, setCards] = useState<Card[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [piles, setPiles] = useState(initial.piles);
  const [counts, setCounts] = useState(initial.deck);
  const [last, setLast] = useState<Move | null>(null);
  const [query, setQuery] = useState("");
  const [toast, setToast] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);
  const [comparing, setComparing] = useState(false);
  const [groupCmp, setGroupCmp] = useState<DupeGroup | null>(null);
  const [muted, setMuted] = useState(initial.settings.muted !== "0");
  const [volume, setVolume] = useState(() => Number(initial.settings.volume ?? "0.7"));
  const [menu, setMenu] = useState<{ pile: Pile; x: number; y: number } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const [broken, setBroken] = useState<Set<number>>(() => new Set());
  const [zone, setZone] = useState(420);
  const [flights, setFlights] = useState<Flight[]>([]);
  const [developing, setDeveloping] = useState(initial.developing);
  const [mode, setMode] = useState<Mode>(initial.settings.mode === "table" || initial.settings.mode === "dupes" ? initial.settings.mode : "deck");
  const [kind, setKind] = useState<Kind>(() => {
    const k = initial.settings.kind_filter;
    return k === "video" || k === "photo" ? k : "all";
  });
  const kindRef = useRef(kind);
  kindRef.current = kind;
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const [seriesList, setSeriesList] = useState<SeriesView[]>([]);
  const seriesMap = useMemo(() => new Map(seriesList.map((s) => [s.id, s])), [seriesList]);
  const [focus, setFocus] = useState(0);
  const [marked, setMarked] = useState<Set<number>>(() => new Set());
  const [rest, setRest] = useState<Rest>(initial.settings.series_rest === "keep" ? "keep" : "trash");
  const loadSeries = useCallback(async () => {
    try {
      setSeriesList(await ipc.deckSeries());
    } catch {
      setSeriesList([]);
    }
  }, []);
  const [groups, setGroups] = useState<DupeGroups | null>(null);
  const [dupesSet, setDupesSet] = useState<DupesSetting>(() => dupesSetting(initial));
  const [ask, setAsk] = useState(false);
  const [find, setFind] = useState(false);
  const [filter, setFilter] = useState<Filter | null>(null);
  const [sheet, setSheet] = useState<Card[]>([]);
  const [sheetLoaded, setSheetLoaded] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(() => new Set());
  const [onScreen, setOnScreen] = useState(0);
  const flightSeq = useRef(0);
  const landed = useCallback((f: Flight) => {
    setFlights((list) => list.filter((x) => x.id !== f.id));
    shake(f.pileId);
  }, []);
  const pending = useRef(new Set<number>());
  const settled = useRef(0);
  const settle = (ids: number[]) => {
    ids.forEach((id) => pending.current.delete(id));
    settled.current++;
  };
  const search = useRef<HTMLInputElement>(null);
  const pilesRef = useRef(piles);
  pilesRef.current = piles;
  const cardsRef = useRef(cards);
  cardsRef.current = cards;
  const sheetRef = useRef(sheet);
  sheetRef.current = sheet;
  const selectedRef = useRef(selected);
  selectedRef.current = selected;

  const loadGroups = useCallback(async () => {
    try {
      setGroups(await ipc.dupeGroups());
    } catch (e) {
      setToast(errorText(e));
    }
  }, []);

  useEffect(() => {
    void loadGroups();
    const off = onDupesChanged(() => void loadGroups());
    return () => void off.then((f) => f());
  }, [loadGroups]);

  const startDupes = async (scope: "deck" | "all") => {
    const after: DupesSetting = { enabled: true, scope };
    setAsk(false);
    try {
      await ipc.setSetting("dupes_scope", scope);
      await ipc.setSetting("dupes_enabled", "1");
    } catch (e) {
      setToast(errorText(e));
      return;
    }
    if (widened(dupesSet, after)) onWiden(dupesSet);
    else {
      setDupesSet(after);
      void loadGroups();
    }
  };

  const startSearch = async (s: SearchSet) => {
    setFind(false);
    try {
      await ipc.searchStart(s.piles, s.deck, s.folders);
      onSearch();
    } catch (e) {
      setToast(errorText(e));
    }
  };

  const closeSearch = async () => {
    await ipc.searchClear();
    void loadGroups();
  };

  const trashPaths = useCallback(
    async (paths: string[]) => {
      if (!paths.length) return;
      const trash = pilesRef.current.find((p) => p.isTrash);
      play("trash");
      if (trash) shake(trash.id);
      try {
        const r = await ipc.trashCopies(paths);
        setPiles(r.piles);
        setLast(r.move);
        setCounts((await ipc.getState()).deck);
      } catch (e) {
        play("error");
        setToast(errorText(e));
      }
      void loadGroups();
    },
    [loadGroups],
  );

  const dismissGroup = useCallback(
    async (g: DupeGroup) => {
      try {
        for (const [a, b] of g.pairs) await ipc.dismissDupe(a, b);
        play("defer");
      } catch (e) {
        setToast(errorText(e));
      }
      void loadGroups();
    },
    [loadGroups],
  );

  const resolveGroup = useCallback(
    async (keep: GroupItem[], trash: GroupItem[], not: GroupItem[]) => {
      setGroupCmp(null);
      const rest = [...keep, ...trash];
      try {
        for (const n of not) for (const r of rest) await ipc.dismissDupe(n.path, r.path);
      } catch (e) {
        setToast(errorText(e));
      }
      if (trash.length) await trashPaths(trash.map((t) => t.path));
      else {
        play("defer");
        void loadGroups();
      }
    },
    [trashPaths, loadGroups],
  );

  const refill = useCallback(async () => {
    let fresh;
    for (let seen = -1; seen !== settled.current; ) {
      seen = settled.current;
      fresh = await ipc.deckWindow(0, 30, kindRef.current);
    }
    setCards(fresh!.filter((c) => !pending.current.has(c.id)));
    setLoaded(true);
  }, []);

  const loadSheet = useCallback(async () => {
    let all;
    for (let seen = -1; seen !== settled.current; ) {
      seen = settled.current;
      all = await ipc.deckWindow(0, 100000, kindRef.current);
    }
    const fresh = all!.filter((c) => !pending.current.has(c.id));
    setSheet(fresh);
    setSelected((sel) => new Set(fresh.filter((c) => sel.has(c.id)).map((c) => c.id)));
    setSheetLoaded(true);
  }, []);

  useEffect(() => {
    if (mode === "table") void loadSheet();
    else void refill();
    void loadSeries();
  }, [mode, kind, refill, loadSheet, loadSeries]);

  useEffect(() => {
    void ipc.getState().then((st) => {
      setCounts(st.deck);
      setPiles(st.piles);
    });
    void ipc.journal(null, 20).then((recent) => setLast(recent.find((m) => m.state === "done") ?? null));
  }, []);

  useEffect(() => {
    const offCard = onCards((fresh) => {
      setCards((cs) => mergeCards(cs, fresh));
      setSheet((cs) => mergeCards(cs, fresh));
    });
    const offProgress = onProgress(setDeveloping);
    const offDeck = onDeckChanged(({ added, gone }) => {
      setCards((cs) => [...cs.filter((c) => !gone.includes(c.id)), ...added.filter((a) => !cs.some((c) => c.id === a.id))]);
      setSheet((cs) => [...cs.filter((c) => !gone.includes(c.id)), ...added.filter((a) => !cs.some((c) => c.id === a.id))]);
      setSelected((sel) => (gone.some((id) => sel.has(id)) ? new Set([...sel].filter((id) => !gone.includes(id))) : sel));
      void ipc.getState().then((st) => setCounts(st.deck));
    });
    const offPiles = onPilesChanged(setPiles);
    const offSeries = onSeries(() => {
      void loadSeries();
      if (modeRef.current === "table") void loadSheet();
      else void refill();
    });
    return () => {
      void offSeries.then((f) => f());
      void offDeck.then((f) => f());
      void offPiles.then((f) => f());
      void offCard.then((f) => f());
      void offProgress.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (!toast) return;
    const t = window.setTimeout(() => setToast(null), 6000);
    return () => window.clearTimeout(t);
  }, [toast]);

  const centerObserver = useRef<ResizeObserver | null>(null);
  const center = useCallback((el: HTMLDivElement | null) => {
    centerObserver.current?.disconnect();
    centerObserver.current = null;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      if (entry.contentRect.height > 0) setZone(Math.max(240, Math.min(entry.contentRect.height, 560)));
    });
    ro.observe(el);
    centerObserver.current = ro;
  }, []);

  const matches = useMemo(() => rank(piles, query), [piles, query]);
  const current = cards[0];
  const cur = current && current.seriesId === current.id ? seriesMap.get(current.id) : undefined;
  const focusCard = cur ? (cur.cards[focus] ?? cur.cards[0]) : undefined;
  const shownCards = useMemo(() => (cur && focusCard ? [focusCard, ...cards.slice(1)] : cards), [cur, focusCard, cards]);
  const bestOf = useCallback((s: SeriesView) => s.cards.find((x) => x.id === s.best) ?? s.cards[0], []);
  const sheetShown = useMemo(
    () =>
      sheet.map((c) => {
        const s = c.seriesId === c.id ? seriesMap.get(c.id) : undefined;
        return s ? bestOf(s) : c;
      }),
    [sheet, seriesMap, bestOf],
  );
  const seriesByShown = useMemo(() => new Map(seriesList.map((s) => [s.best, s])), [seriesList]);
  const seriesCounts = useMemo(() => new Map(seriesList.map((s) => [s.best, s.cards.length])), [seriesList]);
  const sheetShownRef = useRef(sheetShown);
  sheetShownRef.current = sheetShown;
  const seriesByShownRef = useRef(seriesByShown);
  seriesByShownRef.current = seriesByShown;
  useEffect(() => {
    if (!cur) return;
    setFocus(Math.max(0, cur.cards.findIndex((c) => c.id === cur.best)));
    setMarked(new Set([cur.best]));
  }, [cur?.id, cur?.cards.length, cur?.best]);
  const hintIds = useMemo(
    () => (mode === "deck" ? (cur ? [cur.best] : current ? [current.id] : []) : sheetShown.filter((c) => selected.has(c.id)).map((c) => c.id)),
    [mode, current, cur, sheetShown, selected],
  );
  const hints = useHints(hintIds, piles);
  const hintPile = hints.hints[0] ? (piles.find((p) => p.id === hints.hints[0].pileId) ?? null) : null;
  const hot = matches?.[0]?.id ?? (query ? null : (hintPile?.id ?? null));
  const dupes = useDupes(mode === "deck" && current ? current.id : null, piles);
  const dupe = dupes[0] ?? null;

  const place = useCallback(
    async (pile: Pile, method: Method, send?: (card: Card) => Promise<{ move: Move; piles: Pile[] }>) => {
      const card = cardsRef.current.find((c) => !pending.current.has(c.id));
      if (!card) return;
      play(pile.isTrash ? "trash" : method === "new_pile" ? "new_pile" : "place");
      const target = pileElement(pile.id);
      const snap = method !== "drag" && !motionOff() ? snapshot() : null;
      if (snap && target) {
        const id = ++flightSeq.current;
        setFlights((f) => [...f, { id, pileId: pile.id, from: snap.rect, to: target.getBoundingClientRect(), image: snap.image }]);
      } else {
        shake(pile.id);
      }
      setPaused(false);
      pending.current.add(card.id);
      setCards((cs) => cs.filter((c) => c.id !== card.id));
      setCounts((c) => shift(c, [card], 1));
      setQuery("");
      search.current?.blur();
      try {
        const r = await (send ? send(card) : ipc.place([card.id], pile.id, method));
        setPiles(r.piles);
        setLast(r.move);
        if (cardsRef.current.length === 0) play("empty");
        void ipc.getState().then((st) => setCounts(st.deck));
      } catch (e) {
        play("error");
        setCards((cs) => [card, ...cs.filter((c) => c.id !== card.id)]);
        setCounts((c) => shift(c, [card], -1));
        setToast(errorText(e));
      } finally {
        settle([card.id]);
      }
      if (cardsRef.current.length < 12 || current?.kind === "photo") void refill();
    },
    [refill, current?.kind],
  );

  const placeBatch = useCallback(
    async (pile: Pile, method: Method) => {
      const picked = sheetShownRef.current.filter((c) => selectedRef.current.has(c.id));
      if (!picked.length) {
        setToast("Сначала отметь плёнки: клик — одна, Shift клик — диапазон, Ctrl A — все на экране.");
        return;
      }
      const batch = picked.flatMap((c) => seriesByShownRef.current.get(c.id)?.cards ?? [c]);
      const ids = batch.map((c) => c.id);
      play(pile.isTrash ? "trash" : method === "new_pile" ? "new_pile" : batch.length > 1 ? "batch" : "place");
      shake(pile.id);
      ids.forEach((id) => pending.current.add(id));
      setSheet((cs) => cs.filter((c) => !ids.includes(c.id)));
      setCards((cs) => cs.filter((c) => !ids.includes(c.id)));
      setSelected(new Set());
      setCounts((c) => shift(c, batch, 1));
      setQuery("");
      search.current?.blur();
      let failed = false;
      try {
        const r = await ipc.place(ids, pile.id, method);
        setPiles(r.piles);
        setLast(r.move);
        if (sheetRef.current.length === 0) play("empty");
        void ipc.getState().then((st) => setCounts(st.deck));
      } catch (e) {
        failed = true;
        play("error");
        setCounts((c) => shift(c, batch, -1));
        setToast(errorText(e));
      } finally {
        settle(ids);
      }
      if (failed) {
        await loadSheet();
        setSelected(new Set(ids));
      }
    },
    [loadSheet],
  );

  const trashCopies = useCallback(
    async (copies: DupeView[]) => {
      const trash = pilesRef.current.find((p) => p.isTrash);
      if (!trash || !copies.length) return;
      const ids = copies.filter((d) => d.where === "deck" && d.cardId !== null).map((d) => d.cardId!);
      play("trash");
      shake(trash.id);
      ids.forEach((id) => pending.current.add(id));
      setCards((cs) => cs.filter((c) => !ids.includes(c.id)));
      setCounts((c) => ({ ...c, left: c.left - ids.length, placed: c.placed + ids.length }));
      try {
        const r = await ipc.trashCopies(copies.map((d) => d.path));
        setPiles(r.piles);
        setLast(r.move);
        void ipc.getState().then((st) => setCounts(st.deck));
      } catch (e) {
        play("error");
        setCounts((c) => ({ ...c, left: c.left + ids.length, placed: c.placed - ids.length }));
        setToast(errorText(e));
      } finally {
        settle(ids);
      }
      void refill();
    },
    [refill],
  );

  useEffect(() => setComparing(false), [current?.id]);

  const openCompare = useCallback(() => {
    setPaused(true);
    setComparing(true);
  }, []);

  const closeCompare = useCallback(() => {
    setComparing(false);
    setPaused(false);
  }, []);

  const keepFromCompare = useCallback(
    (mine: boolean) => {
      const trash = pilesRef.current.find((p) => p.isTrash);
      closeCompare();
      if (!dupe || !trash) return;
      if (!mine) return place(trash, "key");
      const target = dupe.where === "pile" ? pilesRef.current.find((p) => !p.isTrash && p.name === dupe.pileName) : undefined;
      if (target) return place(target, "key", (card) => ipc.replaceCopy(card.id, dupe.path));
      return trashCopies([dupe]);
    },
    [dupe, place, trashCopies, closeCompare],
  );

  const resolveDupe = useCallback(() => {
    const trash = pilesRef.current.find((p) => p.isTrash);
    if (!dupe || !trash) return;
    if (dupe.better) return place(trash, "key");
    const worse = dupes.filter((d) => !d.better);
    const inPile = worse.filter((d) => d.where === "pile");
    const target = inPile.length === 1 ? pilesRef.current.find((p) => !p.isTrash && p.name === inPile[0].pileName) : undefined;
    if (target && worse.length === 1) return place(target, "key", (card) => ipc.replaceCopy(card.id, inPile[0].path));
    return trashCopies(worse);
  }, [dupe, dupes, place, trashCopies]);

  const dismissDupe = useCallback(async () => {
    if (!dupe || !current) return;
    try {
      await ipc.dismissDupe(current.path, dupe.path);
      setPiles((p) => [...p]);
    } catch (e) {
      setToast(errorText(e));
    }
  }, [dupe, current]);

  const put = useCallback(
    (pile: Pile, method: Method) => {
      if (mode === "deck" && cur) {
        const keep = [...marked].filter((id) => cur.cards.some((c) => c.id === id));
        return place(pile, method, () => ipc.placeSeries(cur.id, keep, pile.id, rest, method)).then(() => {
          void loadSeries();
          void refill();
        });
      }
      if (mode === "deck") return place(pile, method);
      return placeBatch(pile, method === "key" || method === "search" ? "table" : method);
    },
    [mode, cur, marked, rest, place, placeBatch, loadSeries, refill],
  );

  const splitSeries = useCallback(async () => {
    if (!cur) return;
    try {
      await ipc.splitSeries(cur.id);
      play("defer");
      await loadSeries();
      await refill();
    } catch (e) {
      setToast(errorText(e));
    }
  }, [cur, loadSeries, refill]);

  const changeRest = useCallback((r: Rest) => {
    setRest(r);
    void ipc.setSetting("series_rest", r);
  }, []);

  const toggleMark = useCallback((id: number) => {
    setMarked((m) => {
      const next = new Set(m);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const switchKind = useCallback((next: Kind) => {
    setKind(next);
    setQuery("");
    setPaused(false);
    setSelected(new Set());
    void ipc.setSetting("kind_filter", next);
  }, []);

  const switchMode = useCallback((next: Mode) => {
    setMode(next);
    setQuery("");
    setPaused(false);
    void ipc.setSetting("mode", next);
  }, []);

  const [dragPile, setDragPile] = useState<number | null>(null);
  const dropTo = useRef<(pile: Pile) => void>(() => undefined);
  dropTo.current = (pile: Pile) => void put(pile, "drag");
  const dropHandlers = {
    onOver: setDragPile,
    onDrop: (id: number) => {
      const pile = pilesRef.current.find((p) => p.id === id);
      if (pile) dropTo.current(pile);
    },
  };

  const pressTile = useCallback((e: PointerEvent, card: Card) => {
    pressToDrag(e, {
      ...dropHandlers,
      prepare: () => {
        let ids = [...selectedRef.current];
        if (!selectedRef.current.has(card.id)) {
          ids = [card.id];
          selectedRef.current = new Set(ids);
          setSelected(selectedRef.current);
        }
        const tile = (id: number) => document.querySelector<HTMLElement>(`[data-tile-id="${id}"]`);
        const grabbed = tile(card.id);
        if (!grabbed) return null;
        const pieces = ids
          .map(tile)
          .filter((el): el is HTMLElement => !!el)
          .map((el) => ({ el, image: el.querySelector("img")?.src ?? null }));
        return { pieces, grabbed, count: ids.length, radius: 12 };
      },
    });
  }, []);

  const pressCard = (e: PointerEvent) => {
    pressToDrag(e, {
      ...dropHandlers,
      prepare: () => {
        const snap = snapshot();
        const grabbed = document.querySelector<HTMLElement>("[data-card-top]");
        if (!grabbed) return null;
        return { pieces: [{ el: grabbed, image: snap?.image ?? null }], grabbed, count: 1, radius: 28 };
      },
    });
  };

  const newPile = useCallback(async () => {
    const name = query.trim();
    if (!name) return;
    try {
      const list = await ipc.createPile(name);
      setPiles(list);
      const pile = list.find((p) => !p.isTrash && p.name.toLowerCase() === name.toLowerCase());
      const target = mode === "deck" ? cardsRef.current.length > 0 : selectedRef.current.size > 0;
      if (pile && target) await put(pile, "new_pile");
      else setQuery("");
    } catch (e) {
      setToast(errorText(e));
    }
  }, [query, mode, put]);

  const undo = useCallback(async () => {
    try {
      const r = await ipc.undoLast();
      settled.current++;
      if (!r.move) {
        setToast("Забирать нечего — ходов ещё не было.");
        return;
      }
      play("undo");
      const back = r.cards;
      if (back.some((c) => c.kind === "photo")) await refill();
      else setCards((cs) => [...back, ...cs.filter((c) => !back.some((b) => b.id === c.id))]);
      void loadSeries();
      void ipc.getState().then((st) => setCounts(st.deck));
      setPiles(r.piles);
      if (mode === "table") {
        await loadSheet();
        setSelected(new Set(back.map((c) => c.id)));
      }
      const recent = await ipc.journal(null, 20);
      setLast(recent.find((m) => m.state === "done") ?? null);
      void loadGroups();
    } catch (e) {
      play("error");
      setToast(errorText(e));
    }
  }, [mode, loadSheet, loadGroups, refill, loadSeries]);

  const defer = useCallback(async () => {
    const card = cardsRef.current[0];
    if (!card || cardsRef.current.length < 2) return;
    play("defer");
    setPaused(false);
    setCards((cs) => [...cs.slice(1), card]);
    try {
      await ipc.defer(card.id);
    } catch (e) {
      setToast(errorText(e));
    }
  }, []);

  const changeVolume = (v: number) => {
    const next = Math.max(0, Math.min(1, Math.round(v * 20) / 20));
    setVolume(next);
    void ipc.setSetting("volume", String(next));
    if (muted && next > 0) {
      setMuted(false);
      void ipc.setSetting("muted", "0");
    }
  };

  const toggleMute = () => {
    setMuted((m) => {
      void ipc.setSetting("muted", m ? "0" : "1");
      return !m;
    });
  };

  const removePile = async (pile: Pile) => {
    setMenu(null);
    try {
      setPiles(await ipc.removePile(pile.id));
      play("defer");
    } catch (e) {
      play("error");
      setToast(errorText(e));
    }
  };

  const handler = useRef<(e: KeyboardEvent) => void>(() => undefined);
  handler.current = (e: KeyboardEvent) => {
    if (menu) return;
    const active = document.activeElement;
    const inSearch = active === search.current;
    const focused = mode === "deck" ? (focusCard ?? current) : sheetShown.find((c) => selected.has(c.id));
    if (e.ctrlKey && MODE_KEYS[e.code]) {
      e.preventDefault();
      switchMode(MODE_KEYS[e.code]);
      return;
    }
    if (e.ctrlKey && e.code === "KeyZ") {
      e.preventDefault();
      void undo();
      return;
    }
    if (e.ctrlKey && e.code === "KeyK") {
      e.preventDefault();
      onPiles();
      return;
    }
    if (e.ctrlKey && e.code === "KeyJ") {
      e.preventDefault();
      onJournal();
      return;
    }
    if (e.ctrlKey && e.code === "Comma") {
      e.preventDefault();
      onSettings();
      return;
    }
    if (mode === "dupes") {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.code === "KeyF") {
        e.preventDefault();
        setFind(true);
      } else if (e.key === "Escape" && groups?.search) {
        e.preventDefault();
        void closeSearch();
      } else if (e.key === "Enter" && !dupesSet.enabled && !groups?.search) {
        e.preventDefault();
        setAsk(true);
      }
      return;
    }
    if (e.ctrlKey && e.code === "KeyE") {
      e.preventDefault();
      if (focused) void revealItemInDir(focused.path);
      return;
    }
    if (e.ctrlKey && e.code === "KeyO") {
      e.preventDefault();
      if (focused) void openPath(focused.path);
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (inSearch) {
      if (e.key === "Enter") {
        e.preventDefault();
        if (e.shiftKey) void newPile();
        else if (matches?.[0]) void put(matches[0], "search");
      } else if (e.key === "Escape") {
        if (!query && mode === "table") setSelected(new Set());
        setQuery("");
        search.current?.blur();
      }
      return;
    }
    const onControl = active && active !== document.body && active.tagName !== "DIV";
    if (onControl && (e.key === "Enter" || e.key === " " || e.key === "Tab")) return;
    if (e.key === "Enter") {
      e.preventDefault();
      if (hintPile) void put(hintPile, "hint");
      return;
    }
    if (e.key === "Escape") {
      if (query) setQuery("");
      else if (empty) onHome();
      else if (mode === "table") setSelected(new Set());
      return;
    }
    if (mode === "table" && (e.key === " " || e.key === "Tab" || e.key.startsWith("Arrow"))) return;
    if (cur && focusCard) {
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        const n = cur.cards.length;
        setFocus((f) => (f + (e.key === "ArrowLeft" ? n - 1 : 1)) % n);
        return;
      }
      if (e.key === " ") {
        e.preventDefault();
        toggleMark(focusCard.id);
        return;
      }
      if (keyOf(e) === "S") {
        e.preventDefault();
        void splitSeries();
        return;
      }
    }
    if (e.key === " ") {
      e.preventDefault();
      if (current?.kind !== "photo") setPaused((p) => !p);
      return;
    }
    if (e.key === "Tab") {
      e.preventDefault();
      void defer();
      return;
    }
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      changeVolume((muted ? 0 : volume) + (e.key === "ArrowUp" ? 0.1 : -0.1));
      return;
    }
    if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && current?.kind === "video") {
      e.preventDefault();
      seekFrame(current, e.key === "ArrowLeft" ? -1 : 1);
      return;
    }
    if (e.key === "/") {
      e.preventDefault();
      search.current?.focus();
      return;
    }
    const k = keyOf(e);
    if (mode === "deck" && dupe && (k === "D" || k === "N" || k === "C")) {
      e.preventDefault();
      if (k === "C") openCompare();
      else void (k === "D" ? resolveDupe() : dismissDupe());
      return;
    }
    const pile = k ? piles.find((p) => p.key === k) : undefined;
    if (pile) {
      e.preventDefault();
      void put(pile, "key");
      return;
    }
    if (k === "M") {
      e.preventDefault();
      toggleMute();
      return;
    }
    if (isTypingChar(e)) {
      e.preventDefault();
      setQuery(e.key);
      search.current?.focus();
    }
  };

  useEffect(() => {
    const listen = (e: KeyboardEvent) => handler.current(e);
    window.addEventListener("keydown", listen);
    return () => window.removeEventListener("keydown", listen);
  }, []);

  const number = counts.placed + 1;
  const empty = mode === "deck" ? loaded && !current : sheetLoaded && sheet.length === 0;
  const kindCounts: Record<Kind, number> = { all: counts.left, video: counts.byKind.video.left, photo: counts.byKind.photo.left };
  const mixed = counts.byKind.video.left + counts.byKind.video.placed > 0 && counts.byKind.photo.left + counts.byKind.photo.placed > 0;
  const shown = kind === "all" ? counts : counts.byKind[kind];
  const other: Kind | null = kind === "video" ? "photo" : kind === "photo" ? "video" : null;
  const restLeft = other ? counts.byKind[other].left : 0;
  const picked = sheetShown.reduce((n, c) => n + Number(selected.has(c.id)), 0);
  const pickedFiles = sheetShown.reduce((n, c) => n + (selected.has(c.id) ? (seriesCounts.get(c.id) ?? 1) : 0), 0);
  const sure = groups?.groups.filter((g) => g.confidence >= SURE).length ?? 0;
  const dupesOff = !dupesSet.enabled && !groups?.search;

  if (empty && mode !== "dupes")
    return (
      <main className="mx-2 mb-2 flex min-h-0 flex-1 flex-col gap-[18px] rounded-[28px] bg-cosmic px-10 py-7">
        {banner}
        <Done
          placed={counts.placed}
          piles={piles}
          deckPath={initial.settings.deck_path ?? ""}
          tablePath={initial.settings.table_path ?? ""}
          hintsOn={initial.settings.hints_enabled === "1"}
          onJournal={onJournal}
          onHome={onHome}
          rest={
            other && restLeft > 0
              ? {
                  gone: kind === "photo" ? "Фото" : "Видео",
                  left: `${restLeft} ${KIND_WORD[other]}`,
                  show: other === "photo" ? "Показать фото" : "Показать видео",
                  onShow: () => switchKind(other),
                }
              : undefined
          }
        />
      </main>
    );

  return (
    <main
      className="relative mx-2 mb-2 flex min-h-0 flex-1 flex-col gap-[18px] rounded-[28px] bg-cosmic px-10 py-7"
    >
      <PageHeader
        tag={
          mode === "dupes" ? (
            groups?.search ? (
              <Tag icon={<Search strokeWidth={1.5} />}>Отдельный поиск</Tag>
            ) : dupesSet.enabled ? (
              <Tag icon={<Copy strokeWidth={1.5} />}>
                Отпечатано {groups?.printed ?? 0} из {groups?.total ?? 0}
              </Tag>
            ) : (
              <Tag icon={<CopyX strokeWidth={1.5} />}>Поиск дублей выключен</Tag>
            )
          ) : mode === "deck" ? (
            <Tag icon={<Layers strokeWidth={1.5} />}>
              Разложено {shown.placed} из {shown.placed + shown.left}
              {kind !== "all" && ` ${KIND_WORD[kind]}`}
              {developing.done < developing.total && ` · проявлено ${developing.done} из ${developing.total}`}
            </Tag>
          ) : (
            <Tag icon={<LayoutGrid strokeWidth={1.5} />}>
              Стол · {onScreen} из {sheet.length} на экране{kind !== "photo" && " · веди мышью по плитке — перемотка"}
              {developing.done < developing.total && ` · проявлено ${developing.done} из ${developing.total}`}
            </Tag>
          )
        }
        light={mode === "dupes" ? (dupesOff ? "Дубли" : "Дубли,") : mode === "deck" ? (kind === "photo" ? "Фото в колоде" : kind === "video" ? "Видео в колоде" : "В колоде") : "На столе,"}
        bold={
          mode === "dupes"
            ? dupesOff
              ? "не ищу"
              : `${sure} ${plural(sure, "группа", "группы", "групп")}`
            : mode === "deck"
              ? String(Math.max(shown.left, 0))
              : picked
                ? `отмечено ${picked}`
                : cardsWord(sheet.length)
        }
        sub={
          mode === "dupes" && dupesSet.enabled && !groups?.search ? (
            <div className="flex items-center gap-2 text-[13px] font-medium text-dim">
              Ищу <b className="text-fg">{dupesSet.scope === "all" ? "в колоде и на столе" : "внутри колоды"}</b>
              <button
                type="button"
                onClick={() => setAsk(true)}
                className="inline-flex h-6 items-center rounded-full bg-raised px-2.5 text-xs font-bold text-fg transition-colors duration-200 ease-trail hover:bg-strong"
              >
                Изменить
              </button>
            </div>
          ) : undefined
        }
        actions={
          <>
            {mode === "dupes" && dupesSet.enabled && !groups?.search && (
              <Button size={40} hotkey="F" icon={<Search size={16} strokeWidth={1.5} />} onClick={() => setFind(true)}>
                Найти дубли…
              </Button>
            )}
            {mixed && <KindFilter value={kind} counts={kindCounts} onChange={switchKind} />}
            <Segment label="Режим" options={modes(sure)} value={mode} onChange={switchMode} />
            <IconButton label="Журнал" hint="Журнал ходов — Ctrl J" onClick={onJournal}>
              <History size={18} strokeWidth={1.5} />
            </IconButton>
            <IconButton label="Стопки" hint="Стопки и клавиши — Ctrl K" onClick={onPiles}>
              <Folder size={18} strokeWidth={1.5} />
            </IconButton>
            <IconButton label="Настройки" hint="Настройки — Ctrl ," onClick={onSettings}>
              <Settings size={18} strokeWidth={1.5} />
            </IconButton>
          </>
        }
      />
      {banner}

      {mode === "dupes" && groups?.search && (
        <SearchBanner search={groups.search} groups={sure} onNew={() => setFind(true)} onClose={() => void closeSearch()} />
      )}
      {mode === "dupes" && dupesOff ? (
        <DupesOff onEnable={() => setAsk(true)} onFind={() => setFind(true)} />
      ) : mode === "dupes" ? (
        <DupesScreen
          groups={(groups?.groups ?? []).filter((g) => kind === "all" || isPhotoPath(g.items[0]?.path ?? "") === (kind === "photo"))}
          filter={filter}
          onFilter={setFilter}
          onKeep={(_: GroupItem, drop: GroupItem[]) => void trashPaths(drop.map((d) => d.path))}
          onDismiss={(g) => void dismissGroup(g)}
          onTrashExact={(paths) => void trashPaths(paths)}
          onCompare={setGroupCmp}
        />
      ) : mode === "table" ? (
        <ContactSheet
          cards={sheetShown}
          series={seriesCounts}
          selected={selected}
          cacheDir={initial.cacheDir}
          onSelect={setSelected}
          onPress={pressTile}
          onVisible={setOnScreen}
        />
      ) : (
        <div ref={center} className="flex min-h-0 flex-1 items-stretch gap-12">
          <LastMove move={last} onUndo={undo} />
          {!loaded && (
            <div className="flex flex-1 items-center justify-center">
              <div className="flex aspect-[0.62] h-[min(64vh,520px)] flex-col items-center justify-center gap-3 rounded-[28px] border border-line bg-film text-[13px] font-medium text-[#a2a2a9]">
                <Loader2 className="h-6 w-6 animate-spin" strokeWidth={1.5} />
                Достаю карты из колоды…
              </div>
            </div>
          )}
          {current && (
            <CardStack
              cards={shownCards}
              series={
                cur
                  ? {
                      index: focus,
                      count: cur.cards.length,
                      span: (({ from, to }) => `${from} – ${to}`)(seriesSpan(cur)),
                      marked: focusCard ? marked.has(focusCard.id) : false,
                    }
                  : undefined
              }
              number={number}
              zone={zone}
              muted={muted}
              paused={paused}
              broken={broken}
              onBroken={(id) => setBroken((b) => new Set(b).add(id))}
              onTogglePause={() => setPaused((p) => !p)}
              volume={volume}
              onPress={pressCard}
            />
          )}
          {current && (
            <div className="flex min-w-0 flex-1 flex-col justify-center-safe gap-3 overflow-y-auto">
              <div className="flex flex-col gap-1">
                <div className="flex min-w-0 items-center gap-2">
                  <div className="truncate text-lg font-bold">{(focusCard ?? current).fileName}</div>
                  <Tag icon={(focusCard ?? current).kind === "photo" ? <ImageIcon strokeWidth={1.5} /> : <Video strokeWidth={1.5} />}>
                    {(focusCard ?? current).kind === "photo" ? "Фото" : "Видео"} · {(focusCard ?? current).fileName.split(".").pop()?.toUpperCase()}
                  </Tag>
                  <button
                    type="button"
                    aria-label="Показать в проводнике"
                    title="Показать в проводнике — Ctrl E"
                    onClick={() => revealItemInDir((focusCard ?? current).path)}
                    className="grid h-8 w-8 shrink-0 place-items-center rounded-full text-dim transition-colors duration-200 ease-trail hover:bg-raised hover:text-fg"
                  >
                    <FolderOpen className="h-4 w-4" strokeWidth={1.5} />
                  </button>
                </div>
                {current.kind === "video" && (
                  <div className="text-[13px] font-medium text-dim">
                    {[
                      current.takenAt ? date(current.takenAt) : null,
                      mb(current.size),
                      current.width && current.height ? `${current.width}×${current.height}` : null,
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </div>
                )}
              </div>
              {cur ? (
                <SeriesPanel
                  series={cur}
                  cacheDir={initial.cacheDir}
                  focus={focus}
                  marked={marked}
                  rest={rest}
                  onFocus={setFocus}
                  onToggle={toggleMark}
                  onRest={changeRest}
                  onSplit={() => void splitSeries()}
                />
              ) : current.kind === "photo" ? (
                <PhotoFacts card={current} />
              ) : (
                <FilmStrip card={current} cacheDir={initial.cacheDir} />
              )}
              {dupe ? (
                <DupeBadge
                  dupe={dupe}
                  more={dupes.length - 1}
                  durationMs={current.durationMs}
                  replace={dupes.length === 1 && dupe.where === "pile"}
                  hint={hintPile ? { pile: hintPile, score: hints.hints[0].score } : null}
                  canDefer={cards.length > 1}
                  onTrash={() => void resolveDupe()}
                  onCompare={openCompare}
                  onDismiss={() => void dismissDupe()}
                  onDefer={() => void defer()}
                />
              ) : (
                <HintBox
                  hints={hints}
                  piles={piles}
                  onPlace={(p) => put(p, "hint")}
                  lead={cur ? (marked.size ? `Отмеченные ${marked.size} просятся в стопку` : "Серия просится в стопку") : undefined}
                />
              )}
              <div className="flex flex-wrap items-center gap-2">
                {!dupe && (
                  <Button onClick={defer} hotkey="Tab" disabled={cards.length < 2}>
                    В конец колоды
                  </Button>
                )}
                {(broken.has(current.id) || current.stage === "broken") && (
                  <Button onClick={() => openPath(current.path)} hotkey="Ctrl O">
                    {current.kind === "photo" ? "Открыть в просмотрщике" : "Открыть в плеере"}
                  </Button>
                )}
                <div className="flex-1" />
                {current.kind === "video" && <VolumeRow muted={muted} volume={volume} onMute={toggleMute} onVolume={changeVolume} />}
              </div>
            </div>
          )}
        </div>
      )}

      {mode !== "dupes" && (
      <>
      <div className="flex items-center gap-3">
        {mode === "table" && picked > 0 ? (
          <span className="flex h-9 shrink-0 items-center rounded-full bg-fg px-3.5 text-[13px] font-bold text-ink">
            Отмечено {picked}
            {pickedFiles !== picked && ` · ${pickedFiles} ${plural(pickedFiles, "файл", "файла", "файлов")}`}
          </span>
        ) : (
          <div className="shrink-0 text-[13px] font-medium text-dim">Стопки · {piles.filter((p) => !p.isTrash).length}</div>
        )}
        <SearchField
          ref={search}
          size={36}
          hotkey="/"
          active={!!query}
          className="w-[300px]"
          placeholder="Печатай — стопки отфильтруются"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {query ? (
          <span className="truncate text-[13px] text-dim">
            {matches?.length ? `Enter — в «${matches[0].name}» · ` : "Такой стопки нет · "}Shift Enter — новая стопка «{query.trim()}»
            {mode === "table" && " · Esc — очистить"}
          </span>
        ) : (
          mode === "table" && (
            <span className="truncate text-[13px] text-dim">
              {picked
                ? hintPile
                  ? `Просятся в «${hintPile.name}» ${Math.round(hints.hints[0].score * 100)}% — Enter · клавиша стопки — в другую · Esc — снять отметки`
                  : "Клавиша стопки — положить отмеченные · Esc — снять отметки"
                : seriesCounts.size
                  ? "Клик — отметить · серия уходит вся · выбрать лучшие — в колоде"
                  : "Клик — отметить · Shift клик — диапазон · Ctrl A — все на экране"}
            </span>
          )
        )}
        <div className="flex-1" />
        {mode === "table" && !query && <LastMoveLine move={last} onUndo={undo} />}
        {mode === "deck" && (
        <Button
          size={36}
          icon={<Plus className="h-3.5 w-3.5" strokeWidth={1.5} />}
          onClick={() => {
            search.current?.focus();
            setToast("Напечатай название и нажми Shift Enter — стопка появится, а карта ляжет в неё.");
          }}
        >
          Новая стопка
        </Button>
        )}
        {mode === "deck" && <KeysHint muted={muted} dupe={!!dupe} face={cur ? "series" : current?.kind === "photo" ? "photo" : "video"} />}
      </div>
      {piles.filter((p) => !p.isTrash).length === 0 && (
        <p className="m-0 -mb-2 text-[13px] text-dim">Стопок пока нет. Напечатай название и нажми Shift Enter — создашь первую.</p>
      )}
      <PilesRow
        piles={piles}
        hot={hot}
        dim={matches ? new Set(matches.map((m) => m.id)) : null}
        onPick={(p) => put(p, "key")}
        dragOver={dragPile}
        onMenu={(pile, x, y) => setMenu({ pile, x, y })}
      />
      </>
      )}

      <Flights flights={flights} onDone={landed} />
      {menu && <PileMenu pile={menu.pile} x={menu.x} y={menu.y} onRemove={() => removePile(menu.pile)} onClose={closeMenu} />}
      {toast && (
        <div
          role="status"
          className="absolute bottom-32 left-1/2 max-w-[640px] -translate-x-1/2 rounded-full bg-fg px-5 py-3 text-sm font-medium text-ink shadow-[0_20px_50px_rgb(0_0_0/40%)]"
        >
          {toast}
        </div>
      )}
      {ask && mode === "dupes" && (
        <EnableDupes
          scope={dupesSet.scope}
          piles={piles.filter((p) => !p.isTrash).length}
          onStart={(scope) => void startDupes(scope)}
          onClose={() => setAsk(false)}
        />
      )}
      {find && mode === "dupes" && (
        <FindDupes
          piles={piles}
          table={initial.settings.table_path ?? ""}
          deckPath={initial.settings.deck_path ?? ""}
          initial={groups?.search ? { piles: groups.search.piles, deck: groups.search.deck, folders: groups.search.folders } : null}
          onStart={(s) => void startSearch(s)}
          onClose={() => setFind(false)}
        />
      )}
      {groupCmp && mode === "dupes" && (
        <GroupCompare
          group={groupCmp}
          title={groupTitle(groupCmp)}
          onDone={(keep, trash, not) => void resolveGroup(keep, trash, not)}
          onClose={() => setGroupCmp(null)}
        />
      )}
      {comparing && mode === "deck" && current && dupe && (
        <Compare
          card={current}
          dupe={dupe}
          volume={muted ? 0 : volume}
          onKeep={(mine) => void keepFromCompare(mine)}
          onDismiss={() => {
            closeCompare();
            void dismissDupe();
          }}
          onClose={closeCompare}
        />
      )}
    </main>
  );
}
