import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { FolderOpen, Layers, Plus } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { errorText, ipc, onCard, onDeckChanged, onPilesChanged, onProgress, type AppState, type Card, type Method, type Move, type Pile } from "../../lib/ipc";
import { isTypingChar, keyOf } from "../../lib/keys";
import { play } from "../../lib/sound";
import { Flights, motionOff, pileElement, shake, snapshot, type Flight } from "../fx/Flight";
import { FilmStrip, seekFrame } from "./FilmStrip";
import { KeyLegend } from "./KeyLegend";
import { PileMenu } from "./PileMenu";
import { VolumeRow } from "./VolumeRow";
import { Button } from "../ui/Button";
import { Heading, PageHeader, Tag } from "../ui/PageHeader";
import { SearchField } from "../ui/SearchField";
import { CardStack } from "./Card";
import { LastMove } from "./LastMove";
import { PilesRow } from "./PilesRow";

const mb = (b: number) => `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ`;
const date = (ms: number) => new Date(ms).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric" });

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
  const flightSeq = useRef(0);
  const landed = useCallback((f: Flight) => {
    setFlights((list) => list.filter((x) => x.id !== f.id));
    shake(f.pileId);
  }, []);
  const pending = useRef(new Set<number>());
  const search = useRef<HTMLInputElement>(null);
  const center = useRef<HTMLDivElement>(null);
  const cardsRef = useRef(cards);
  cardsRef.current = cards;

  const refill = useCallback(async () => {
    const fresh = await ipc.deckWindow(0, 30);
    setCards(fresh.filter((c) => !pending.current.has(c.id)));
    setLoaded(true);
  }, []);

  useEffect(() => {
    void refill();
    void ipc.getState().then((st) => {
      setCounts(st.deck);
      setPiles(st.piles);
    });
    void ipc.journal(null, 20).then((recent) => setLast(recent.find((m) => m.state === "done") ?? null));
  }, [refill]);

  useEffect(() => {
    const offCard = onCard((card) => setCards((cs) => cs.map((c) => (c.id === card.id ? { ...c, ...card } : c))));
    const offProgress = onProgress(setDeveloping);
    const offDeck = onDeckChanged(({ added, gone }) => {
      setCards((cs) => [...cs.filter((c) => !gone.includes(c.id)), ...added.filter((a) => !cs.some((c) => c.id === a.id))]);
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

  useLayoutEffect(() => {
    const el = center.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setZone(Math.max(240, Math.min(entry.contentRect.height, 560))));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const matches = useMemo(() => rank(piles, query), [piles, query]);
  const hot = matches?.[0]?.id ?? null;
  const current = cards[0];

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

  const newPile = useCallback(async () => {
    const name = query.trim();
    if (!name) return;
    try {
      const list = await ipc.createPile(name);
      setPiles(list);
      const pile = list.find((p) => !p.isTrash && p.name.toLowerCase() === name.toLowerCase());
      if (pile && cardsRef.current[0]) await place(pile, "new_pile");
      else setQuery("");
    } catch (e) {
      setToast(errorText(e));
    }
  }, [query, place]);

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
      const recent = await ipc.journal(null, 20);
      setLast(recent.find((m) => m.state === "done") ?? null);
    } catch (e) {
      play("error");
      setToast(errorText(e));
    }
  }, []);

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
    if (e.ctrlKey && e.code === "KeyZ") {
      e.preventDefault();
      void undo();
      return;
    }
    if (e.ctrlKey && e.code === "KeyE") {
      e.preventDefault();
      if (current) void revealItemInDir(current.path);
      return;
    }
    if (e.ctrlKey && e.code === "KeyO") {
      e.preventDefault();
      if (current) void openPath(current.path);
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (inSearch) {
      if (e.key === "Enter") {
        e.preventDefault();
        if (e.shiftKey) void newPile();
        else if (matches?.[0]) void place(matches[0], "search");
      } else if (e.key === "Escape") {
        setQuery("");
        search.current?.blur();
      }
      return;
    }
    const onControl = active && active !== document.body && active.tagName !== "DIV";
    if (onControl && (e.key === "Enter" || e.key === " " || e.key === "Tab")) return;
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
    if (e.key === "Escape") {
      setQuery("");
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
    const pile = k ? piles.find((p) => p.key === k) : undefined;
    if (pile) {
      e.preventDefault();
      void place(pile, "key");
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
  const empty = loaded && !current;

  return (
    <main
      className="relative mx-2 mb-2 flex min-h-0 flex-1 flex-col gap-[18px] rounded-[28px] bg-cosmic px-10 py-7"
    >
      <PageHeader
        tag={
          <Tag icon={<Layers strokeWidth={1.5} />}>
            Разложено {counts.placed} из {counts.placed + counts.left}
            {developing.done < developing.total && ` · проявлено ${developing.done} из ${developing.total}`}
          </Tag>
        }
        light="В колоде"
        bold={String(Math.max(counts.left, 0))}
      />

      {empty ? (
        <div className="flex min-h-0 flex-1 flex-col items-start justify-center gap-4">
          <Heading light="Колода" bold="пуста" size={56} />
          <p className="m-0 max-w-[60ch] text-[15px] leading-[1.55] text-dim">
            Все видео лежат по стопкам. Новые видео из папки колоды появятся здесь при следующем запуске.
          </p>
          <Button onClick={onReload}>Проверить папку ещё раз</Button>
        </div>
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
              onDragStart={(e) => {
                e.dataTransfer.setData("text/plain", String(current.id));
                e.dataTransfer.effectAllowed = "move";
              }}
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
              <div className="flex items-center gap-2">
                <Button onClick={defer} hotkey="Tab" disabled={cards.length < 2}>
                  В конец колоды
                </Button>
                {broken.has(current.id) && (
                  <Button onClick={() => openPath(current.path)} hotkey="Ctrl O">
                    Открыть в плеере
                  </Button>
                )}
                <div className="flex-1" />
                <VolumeRow muted={muted} volume={volume} onMute={toggleMute} onVolume={changeVolume} />
              </div>
              <KeyLegend muted={muted} />
            </div>
          )}
        </div>
      )}

      <div className="flex items-center gap-3">
        <div className="text-[13px] font-medium text-dim">Стопки · {piles.filter((p) => !p.isTrash).length}</div>
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
        {query && (
          <span className="truncate text-[13px] text-dim">
            {matches?.length ? `Enter — в «${matches[0].name}» · ` : "Такой стопки нет · "}Shift Enter — новая стопка «{query.trim()}»
          </span>
        )}
        <div className="flex-1" />
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
      </div>
      {piles.filter((p) => !p.isTrash).length === 0 && (
        <p className="m-0 -mb-2 text-[13px] text-dim">Стопок пока нет. Напечатай название и нажми Shift Enter — создашь первую.</p>
      )}
      <PilesRow
        piles={piles}
        hot={hot}
        dim={matches ? new Set(matches.map((m) => m.id)) : null}
        onPick={(p) => place(p, "key")}
        onDrop={(p) => place(p, "drag")}
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
