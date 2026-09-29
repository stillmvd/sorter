import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { FolderOpen, LayoutGrid, Layers, Plus } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent } from "react";
import { errorText, ipc, onCard, onDeckChanged, onPilesChanged, onProgress, type AppState, type Card, type Method, type Move, type Pile } from "../../lib/ipc";
import { isTypingChar, keyOf } from "../../lib/keys";
import { cardsWord } from "../../lib/plural";
import { useHints } from "../../lib/useHints";
import { useDupes } from "../../lib/useDupes";
import { HintBox } from "./HintBox";
import { DupeBadge } from "./DupeBadge";
import { play } from "../../lib/sound";
import { pressToDrag } from "../fx/Drag";
import { Flights, motionOff, pileElement, shake, snapshot, type Flight } from "../fx/Flight";
import { FilmStrip, seekFrame } from "./FilmStrip";
import { KeyLegend } from "./KeyLegend";
import { PileMenu } from "./PileMenu";
import { VolumeRow } from "./VolumeRow";
import { Button } from "../ui/Button";
import { Heading, PageHeader, Tag } from "../ui/PageHeader";
import { SearchField } from "../ui/SearchField";
import { CardStack } from "./Card";
import { LastMove, LastMoveLine } from "./LastMove";
import { ContactSheet } from "../table/ContactSheet";
import { Segment } from "../ui/Segment";
import { PilesRow } from "./PilesRow";

const mb = (b: number) => `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ`;
const date = (ms: number) => new Date(ms).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric" });

type Mode = "deck" | "table";

const MODES: { value: Mode; label: string; hint: string }[] = [
  { value: "deck", label: "Колода", hint: "По одной карте — Ctrl 1" },
  { value: "table", label: "Стол", hint: "Пачкой — Ctrl 2" },
];

function rank(piles: Pile[], query: string) {
  const q = query.trim().toLowerCase();
  if (!q) return null;
  return piles
    .filter((p) => !p.isTrash && p.name.toLowerCase().includes(q))
    .sort((a, b) => Number(!a.name.toLowerCase().startsWith(q)) - Number(!b.name.toLowerCase().startsWith(q)));
}

export function DeckScreen({ initial, onReload }: { initial: AppState; onReload: () => void }) {
  const [cards, setCards] = useState<Card[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [piles, setPiles] = useState(initial.piles);
  const [counts, setCounts] = useState(initial.deck);
  const [last, setLast] = useState<Move | null>(null);
  const [query, setQuery] = useState("");
  const [toast, setToast] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);
  const [muted, setMuted] = useState(initial.settings.muted !== "0");
  const [volume, setVolume] = useState(() => Number(initial.settings.volume ?? "0.7"));
  const [menu, setMenu] = useState<{ pile: Pile; x: number; y: number } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const [broken, setBroken] = useState<Set<number>>(() => new Set());
  const [zone, setZone] = useState(420);
  const [flights, setFlights] = useState<Flight[]>([]);
  const [developing, setDeveloping] = useState(initial.developing);
  const [mode, setMode] = useState<Mode>(initial.settings.mode === "table" ? "table" : "deck");
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
  const search = useRef<HTMLInputElement>(null);
  const pilesRef = useRef(piles);
  pilesRef.current = piles;
  const cardsRef = useRef(cards);
  cardsRef.current = cards;
  const sheetRef = useRef(sheet);
  sheetRef.current = sheet;
  const selectedRef = useRef(selected);
  selectedRef.current = selected;

  const refill = useCallback(async () => {
    const fresh = await ipc.deckWindow(0, 30);
    setCards(fresh.filter((c) => !pending.current.has(c.id)));
    setLoaded(true);
  }, []);

  const loadSheet = useCallback(async () => {
    const all = await ipc.deckWindow(0, 100000);
    const fresh = all.filter((c) => !pending.current.has(c.id));
    setSheet(fresh);
    setSelected((sel) => new Set(fresh.filter((c) => sel.has(c.id)).map((c) => c.id)));
    setSheetLoaded(true);
  }, []);

  useEffect(() => {
    if (mode === "table") void loadSheet();
    else void refill();
  }, [mode, refill, loadSheet]);

  useEffect(() => {
    void ipc.getState().then((st) => {
      setCounts(st.deck);
      setPiles(st.piles);
    });
    void ipc.journal(null, 20).then((recent) => setLast(recent.find((m) => m.state === "done") ?? null));
  }, []);

  useEffect(() => {
    const offCard = onCard((card) => {
      setCards((cs) => cs.map((c) => (c.id === card.id ? { ...c, ...card } : c)));
      setSheet((cs) => cs.map((c) => (c.id === card.id ? { ...c, ...card } : c)));
    });
    const offProgress = onProgress(setDeveloping);
    const offDeck = onDeckChanged(({ added, gone }) => {
      setCards((cs) => [...cs.filter((c) => !gone.includes(c.id)), ...added.filter((a) => !cs.some((c) => c.id === a.id))]);
      setSheet((cs) => [...cs.filter((c) => !gone.includes(c.id)), ...added.filter((a) => !cs.some((c) => c.id === a.id))]);
      setSelected((sel) => (gone.some((id) => sel.has(id)) ? new Set([...sel].filter((id) => !gone.includes(id))) : sel));
      void ipc.getState().then((st) => setCounts(st.deck));
    });
    const offPiles = onPilesChanged(setPiles);
    return () => {
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
  const hintIds = useMemo(
    () => (mode === "deck" ? (current ? [current.id] : []) : sheet.filter((c) => selected.has(c.id)).map((c) => c.id)),
    [mode, current, sheet, selected],
  );
  const hints = useHints(hintIds, piles);
  const hintPile = hints.hints[0] ? (piles.find((p) => p.id === hints.hints[0].pileId) ?? null) : null;
  const hot = matches?.[0]?.id ?? (query ? null : (hintPile?.id ?? null));
  const dupes = useDupes(mode === "deck" && current ? current.id : null, piles);
  const dupe = dupes[0] ?? null;

  const place = useCallback(
    async (pile: Pile, method: Method) => {
      const card = cardsRef.current[0];
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
      setCounts((c) => ({ ...c, left: c.left - 1, placed: c.placed + 1 }));
      setQuery("");
      search.current?.blur();
      try {
        const r = await ipc.place([card.id], pile.id, method);
        setPiles(r.piles);
        setLast(r.move);
        if (cardsRef.current.length === 0) play("empty");
      } catch (e) {
        play("error");
        setCards((cs) => [card, ...cs.filter((c) => c.id !== card.id)]);
        setCounts((c) => ({ ...c, left: c.left + 1, placed: c.placed - 1 }));
        setToast(errorText(e));
      } finally {
        pending.current.delete(card.id);
      }
      if (cardsRef.current.length < 12) void refill();
    },
    [refill],
  );

  const placeBatch = useCallback(
    async (pile: Pile, method: Method) => {
      const batch = sheetRef.current.filter((c) => selectedRef.current.has(c.id));
      if (!batch.length) {
        setToast("Сначала отметь плёнки: клик — одна, Shift клик — диапазон, Ctrl A — все на экране.");
        return;
      }
      const ids = batch.map((c) => c.id);
      play(pile.isTrash ? "trash" : method === "new_pile" ? "new_pile" : batch.length > 1 ? "batch" : "place");
      shake(pile.id);
      ids.forEach((id) => pending.current.add(id));
      setSheet((cs) => cs.filter((c) => !ids.includes(c.id)));
      setCards((cs) => cs.filter((c) => !ids.includes(c.id)));
      setSelected(new Set());
      setCounts((c) => ({ ...c, left: c.left - ids.length, placed: c.placed + ids.length }));
      setQuery("");
      search.current?.blur();
      let failed = false;
      try {
        const r = await ipc.place(ids, pile.id, method);
        setPiles(r.piles);
        setLast(r.move);
        if (sheetRef.current.length === 0) play("empty");
      } catch (e) {
        failed = true;
        play("error");
        setCounts((c) => ({ ...c, left: c.left + ids.length, placed: c.placed - ids.length }));
        setToast(errorText(e));
      } finally {
        ids.forEach((id) => pending.current.delete(id));
      }
      if (failed) {
        await loadSheet();
        setSelected(new Set(ids));
      }
    },
    [loadSheet],
  );

  const trashCopies = useCallback(
    async (ids: number[]) => {
      const trash = pilesRef.current.find((p) => p.isTrash);
      if (!trash || !ids.length) return;
      play("trash");
      shake(trash.id);
      ids.forEach((id) => pending.current.add(id));
      setCards((cs) => cs.filter((c) => !ids.includes(c.id)));
      setCounts((c) => ({ ...c, left: c.left - ids.length, placed: c.placed + ids.length }));
      try {
        const r = await ipc.place(ids, trash.id, "key");
        setPiles(r.piles);
        setLast(r.move);
      } catch (e) {
        play("error");
        setCounts((c) => ({ ...c, left: c.left + ids.length, placed: c.placed - ids.length }));
        setToast(errorText(e));
      } finally {
        ids.forEach((id) => pending.current.delete(id));
      }
      void refill();
    },
    [refill],
  );

  const resolveDupe = useCallback(() => {
    const trash = pilesRef.current.find((p) => p.isTrash);
    if (!dupe || !trash) return;
    if (dupe.better) return place(trash, "key");
    const ids = dupes.filter((d) => !d.better && d.cardId !== null).map((d) => d.cardId!);
    if (!ids.length) {
      setToast("Лучшая копия — эта карта, а вторая лежит в стопке. Замену сделаем в сравнении — скоро.");
      return;
    }
    return trashCopies(ids);
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
      if (mode === "deck") return place(pile, method);
      return placeBatch(pile, method === "key" || method === "search" ? "table" : method);
    },
    [mode, place, placeBatch],
  );

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
      if (!r.move) {
        setToast("Забирать нечего — ходов ещё не было.");
        return;
      }
      play("undo");
      const back = r.cards;
      setCards((cs) => [...back, ...cs.filter((c) => !back.some((b) => b.id === c.id))]);
      setCounts((c) => ({ ...c, left: c.left + back.length, placed: c.placed - back.length }));
      setPiles(r.piles);
      if (mode === "table") {
        await loadSheet();
        setSelected(new Set(back.map((c) => c.id)));
      }
      const recent = await ipc.journal(null, 20);
      setLast(recent.find((m) => m.state === "done") ?? null);
    } catch (e) {
      play("error");
      setToast(errorText(e));
    }
  }, [mode, loadSheet]);

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
    const focus = mode === "deck" ? current : sheet.find((c) => selected.has(c.id));
    if (e.ctrlKey && (e.code === "Digit1" || e.code === "Digit2")) {
      e.preventDefault();
      switchMode(e.code === "Digit1" ? "deck" : "table");
      return;
    }
    if (e.ctrlKey && e.code === "KeyZ") {
      e.preventDefault();
      void undo();
      return;
    }
    if (e.ctrlKey && e.code === "KeyE") {
      e.preventDefault();
      if (focus) void revealItemInDir(focus.path);
      return;
    }
    if (e.ctrlKey && e.code === "KeyO") {
      e.preventDefault();
      if (focus) void openPath(focus.path);
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
      else if (mode === "table") setSelected(new Set());
      return;
    }
    if (mode === "table" && (e.key === " " || e.key === "Tab" || e.key.startsWith("Arrow"))) return;
    if (e.key === " ") {
      e.preventDefault();
      setPaused((p) => !p);
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
    if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && current) {
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
    if (mode === "deck" && dupe && (k === "D" || k === "N")) {
      e.preventDefault();
      void (k === "D" ? resolveDupe() : dismissDupe());
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
  const picked = sheet.reduce((n, c) => n + Number(selected.has(c.id)), 0);

  return (
    <main
      className="relative mx-2 mb-2 flex min-h-0 flex-1 flex-col gap-[18px] rounded-[28px] bg-cosmic px-10 py-7"
    >
      <PageHeader
        tag={
          mode === "deck" ? (
            <Tag icon={<Layers strokeWidth={1.5} />}>
              Разложено {counts.placed} из {counts.placed + counts.left}
              {developing.done < developing.total && ` · проявлено ${developing.done} из ${developing.total}`}
            </Tag>
          ) : (
            <Tag icon={<LayoutGrid strokeWidth={1.5} />}>
              Стол · {onScreen} из {sheet.length} на экране · веди мышью по плитке — перемотка
              {developing.done < developing.total && ` · проявлено ${developing.done} из ${developing.total}`}
            </Tag>
          )
        }
        light={mode === "deck" ? "В колоде" : "На столе,"}
        bold={mode === "deck" ? String(Math.max(counts.left, 0)) : picked ? `отмечено ${picked}` : cardsWord(sheet.length)}
        actions={<Segment label="Режим" options={MODES} value={mode} onChange={switchMode} />}
      />

      {empty ? (
        <div className="flex min-h-0 flex-1 flex-col items-start justify-center gap-4">
          <Heading light="Колода" bold="пуста" size={56} />
          <p className="m-0 max-w-[60ch] text-[15px] leading-[1.55] text-dim">
            Все видео лежат по стопкам. Новые видео из папки колоды появятся здесь при следующем запуске.
          </p>
          <Button onClick={onReload}>Проверить папку ещё раз</Button>
        </div>
      ) : mode === "table" ? (
        <ContactSheet
          cards={sheet}
          selected={selected}
          cacheDir={initial.cacheDir}
          onSelect={setSelected}
          onPress={pressTile}
          onVisible={setOnScreen}
        />
      ) : (
        <div ref={center} className="flex min-h-0 flex-1 items-stretch gap-12">
          <LastMove move={last} onUndo={undo} />
          {current && (
            <CardStack
              cards={cards}
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
            <div className="flex min-w-0 flex-1 flex-col justify-center gap-3">
              <div className="flex flex-col gap-1">
                <div className="flex min-w-0 items-center gap-2">
                  <div className="truncate text-lg font-bold">{current.fileName}</div>
                  <button
                    type="button"
                    aria-label="Показать в проводнике"
                    title="Показать в проводнике — Ctrl E"
                    onClick={() => revealItemInDir(current.path)}
                    className="grid h-8 w-8 shrink-0 place-items-center rounded-full text-dim transition-colors duration-200 ease-trail hover:bg-raised hover:text-fg"
                  >
                    <FolderOpen className="h-4 w-4" strokeWidth={1.5} />
                  </button>
                </div>
                <div className="text-[13px] font-medium text-dim">
                  {[
                    current.takenAt ? date(current.takenAt) : null,
                    mb(current.size),
                    current.width && current.height ? `${current.width}×${current.height}` : null,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </div>
              </div>
              <FilmStrip card={current} cacheDir={initial.cacheDir} />
              {dupe ? (
                <DupeBadge
                  dupe={dupe}
                  more={dupes.length - 1}
                  hint={hintPile ? { pile: hintPile, score: hints.hints[0].score } : null}
                  canDefer={cards.length > 1}
                  onTrash={() => void resolveDupe()}
                  onDismiss={() => void dismissDupe()}
                  onDefer={() => void defer()}
                />
              ) : (
                <HintBox hints={hints} piles={piles} onPlace={(p) => put(p, "hint")} />
              )}
              <div className="flex items-center gap-2">
                {!dupe && (
                  <Button onClick={defer} hotkey="Tab" disabled={cards.length < 2}>
                    В конец колоды
                  </Button>
                )}
                {broken.has(current.id) && (
                  <Button onClick={() => openPath(current.path)} hotkey="Ctrl O">
                    Открыть в плеере
                  </Button>
                )}
                <div className="flex-1" />
                <VolumeRow muted={muted} volume={volume} onMute={toggleMute} onVolume={changeVolume} />
              </div>
              <KeyLegend muted={muted} dupe={!!dupe} />
            </div>
          )}
        </div>
      )}

      <div className="flex items-center gap-3">
        {mode === "table" && picked > 0 ? (
          <span className="flex h-9 shrink-0 items-center rounded-full bg-fg px-3.5 text-[13px] font-bold text-ink">Отмечено {picked}</span>
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
    </main>
  );
}
