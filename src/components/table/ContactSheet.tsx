import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type DragEvent, type MouseEvent } from "react";
import type { Card } from "../../lib/ipc";
import { STRIP_H, StripRow } from "./StripRow";

const GAP = 8;
const ROW = STRIP_H + GAP;
const PAD = 12;
const OVERSCAN = 4;

export function ContactSheet({
  cards,
  selected,
  cacheDir,
  onSelect,
  onDragStart,
  onVisible,
}: {
  cards: Card[];
  selected: Set<number>;
  cacheDir: string;
  onSelect: (next: Set<number>) => void;
  onDragStart: (e: DragEvent, card: Card) => void;
  onVisible: (count: number) => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState(0);
  const [height, setHeight] = useState(0);
  const [hover, setHover] = useState<number | null>(null);
  const timer = useRef(0);
  const anchor = useRef<number | null>(null);
  const cardsRef = useRef(cards);
  cardsRef.current = cards;
  const selectedRef = useRef(selected);
  selectedRef.current = selected;

  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setHeight(entry.contentRect.height));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const rows = Math.ceil(cards.length / 2);
  const top = Math.max(0, Math.floor((scroll - PAD) / ROW));
  const bottom = Math.min(rows, Math.ceil((scroll + height - PAD) / ROW));
  const from = Math.max(0, top - OVERSCAN);
  const to = Math.min(rows, bottom + OVERSCAN);
  const visibleRef = useRef<Card[]>([]);
  visibleRef.current = cards.slice(top * 2, bottom * 2);
  const onScreen = visibleRef.current.length;

  useEffect(() => onVisible(onScreen), [onScreen, onVisible]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  useEffect(() => {
    const listen = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.code !== "KeyA" || document.activeElement instanceof HTMLInputElement) return;
      e.preventDefault();
      onSelect(new Set([...selectedRef.current, ...visibleRef.current.map((c) => c.id)]));
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

  const hovered = useCallback((id: number | null) => {
    window.clearTimeout(timer.current);
    if (id === null) setHover(null);
    else timer.current = window.setTimeout(() => setHover(id), 150);
  }, []);

  const lines = [];
  for (let r = from; r < to; r++) {
    lines.push(
      <div key={r} className="absolute inset-x-4 grid grid-cols-2 gap-x-4" style={{ top: PAD + r * ROW }}>
        {cards.slice(r * 2, r * 2 + 2).map((card, k) => (
          <StripRow
            key={card.id}
            card={card}
            no={r * 2 + k + 1}
            cacheDir={cacheDir}
            selected={selected.has(card.id)}
            playing={hover === card.id}
            onPick={pick}
            onHover={hovered}
            onDragStart={onDragStart}
          />
        ))}
      </div>,
    );
  }

  return (
    <div
      ref={box}
      onScroll={(e) => setScroll(e.currentTarget.scrollTop)}
      className="relative min-h-0 flex-1 overflow-y-auto rounded-[20px] bg-film [--scroll-inset:20px]"
      style={{ "--strong": "#3c3c3f" } as CSSProperties}
    >
      <div style={{ height: rows ? PAD * 2 + rows * ROW - GAP : 0 }} />
      {lines}
    </div>
  );
}
