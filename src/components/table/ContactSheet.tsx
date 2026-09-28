import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type MouseEvent, type PointerEvent } from "react";
import type { Card } from "../../lib/ipc";
import { Tile } from "./Tile";

const TARGET = 234;
const GAP = 12;
const PAD = 16;
const OVERSCAN = 600;

type Placed = { card: Card; index: number; w: number };
type Row = { items: Placed[]; h: number; top: number };

function aspect(c: Card) {
  const a = c.width && c.height ? c.width / c.height : c.orientation === "landscape" ? 16 / 9 : c.orientation === "square" ? 1 : 9 / 16;
  return Math.max(0.4, Math.min(2.5, a));
}

function layout(cards: Card[], width: number): Row[] {
  const rows: Row[] = [];
  if (width <= 0) return rows;
  let top = PAD;
  let start = 0;
  let sum = 0;
  const close = (end: number, h: number, full: boolean) => {
    const items: Placed[] = [];
    let used = 0;
    for (let i = start; i < end; i++) {
      const last = full && i === end - 1;
      const w = last ? width - used : Math.floor(aspect(cards[i]) * h);
      items.push({ card: cards[i], index: i, w });
      used += w + GAP;
    }
    rows.push({ items, h: Math.round(h), top });
    top += Math.round(h) + GAP;
    start = end;
    sum = 0;
  };
  for (let i = 0; i < cards.length; i++) {
    sum += aspect(cards[i]);
    const gaps = (i - start) * GAP;
    if (sum * TARGET + gaps >= width) close(i + 1, (width - gaps) / sum, true);
  }
  if (start < cards.length) close(cards.length, TARGET, false);
  return rows;
}

export function ContactSheet({
  cards,
  selected,
  cacheDir,
  onSelect,
  onPress,
  onVisible,
}: {
  cards: Card[];
  selected: Set<number>;
  cacheDir: string;
  onSelect: (next: Set<number>) => void;
  onPress: (e: PointerEvent, card: Card) => void;
  onVisible: (count: number) => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState(0);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const anchor = useRef<number | null>(null);
  const cardsRef = useRef(cards);
  cardsRef.current = cards;
  const selectedRef = useRef(selected);
  selectedRef.current = selected;

  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setSize({ w: entry.contentRect.width, h: entry.contentRect.height }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const rows = useMemo(() => layout(cards, size.w - PAD * 2), [cards, size.w]);
  const total = rows.length ? rows[rows.length - 1].top + rows[rows.length - 1].h + PAD : 0;
  const shown = rows.filter((r) => r.top + r.h > scroll - OVERSCAN && r.top < scroll + size.h + OVERSCAN);
  const visibleRef = useRef<Card[]>([]);
  visibleRef.current = rows
    .filter((r) => r.top + r.h / 2 > scroll && r.top + r.h / 2 < scroll + size.h)
    .flatMap((r) => r.items.map((p) => p.card));
  const onScreen = visibleRef.current.length;

  useEffect(() => onVisible(onScreen), [onScreen, onVisible]);

  useEffect(() => {
    const listen = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.code !== "KeyA" || document.activeElement instanceof HTMLInputElement) return;
      e.preventDefault();
      const next = new Set([...selectedRef.current, ...visibleRef.current.map((c) => c.id)]);
      selectedRef.current = next;
      onSelect(next);
    };
    window.addEventListener("keydown", listen);
    return () => window.removeEventListener("keydown", listen);
  }, [onSelect]);

  const pick = useCallback(
    (e: MouseEvent, card: Card) => {
      const list = cardsRef.current;
      const next = new Set(selectedRef.current);
      const i = list.findIndex((c) => c.id === card.id);
      const a = anchor.current === null ? -1 : list.findIndex((c) => c.id === anchor.current);
      if (e.shiftKey && a >= 0 && i >= 0) {
        for (let k = Math.min(a, i); k <= Math.max(a, i); k++) next.add(list[k].id);
      } else {
        if (next.has(card.id)) next.delete(card.id);
        else next.add(card.id);
        anchor.current = card.id;
      }
      selectedRef.current = next;
      onSelect(next);
    },
    [onSelect],
  );

  return (
    <div
      ref={box}
      onScroll={(e) => setScroll(e.currentTarget.scrollTop)}
      className="relative min-h-0 flex-1 overflow-y-auto rounded-[20px] bg-film [--scroll-inset:20px]"
      style={{ "--strong": "#3c3c3f" } as CSSProperties}
    >
      <div style={{ height: total }} />
      {shown.map((r) => (
        <div key={r.items[0].card.id} className="absolute flex" style={{ top: r.top, left: PAD, gap: GAP }}>
          {r.items.map((p) => (
            <Tile
              key={p.card.id}
              card={p.card}
              no={p.index + 1}
              w={p.w}
              h={r.h}
              cacheDir={cacheDir}
              selected={selected.has(p.card.id)}
              onPick={pick}
              onPress={onPress}
            />
          ))}
        </div>
      ))}
    </div>
  );
}
