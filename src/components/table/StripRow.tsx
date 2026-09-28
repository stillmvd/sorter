import { memo, type DragEvent, type MouseEvent } from "react";
import { frameSrc, media, type Card } from "../../lib/ipc";

const SHOWN = [0, 1, 3, 4, 6, 7];

const tc = (ms: number) => `${Math.floor(ms / 60000)}:${String(Math.floor((ms % 60000) / 1000)).padStart(2, "0")}`;

export const STRIP_H = 72;

export const StripRow = memo(function StripRow({
  card,
  no,
  cacheDir,
  selected,
  playing,
  onPick,
  onHover,
  onDragStart,
}: {
  card: Card;
  no: number;
  cacheDir: string;
  selected: boolean;
  playing: boolean;
  onPick: (e: MouseEvent, card: Card) => void;
  onHover: (id: number | null) => void;
  onDragStart: (e: DragEvent, card: Card) => void;
}) {
  const ready = card.frames > 0;
  return (
    <button
      type="button"
      draggable
      aria-pressed={selected}
      aria-label={card.fileName}
      onClick={(e) => onPick(e, card)}
      onMouseEnter={() => onHover(card.id)}
      onMouseLeave={() => onHover(null)}
      onDragStart={(e) => onDragStart(e, card)}
      className={`relative flex w-full min-w-0 select-none flex-col gap-1 rounded-lg pb-1.5 pl-9 pr-1.5 pt-1 text-left ${
        selected ? "outline outline-[1.5px] outline-[#ececef]" : ""
      }`}
      style={{ height: STRIP_H }}
    >
      <span className="flex h-3.5 items-center gap-2.5 font-mono text-[10px] font-medium leading-[14px] tracking-[0.06em] text-[#6d6d74]">
        <span className={selected ? "text-[#ececef]" : ""}>{String(no).padStart(3, "0")}</span>
        <span className="truncate">{card.fileName}</span>
        <span className="flex-1" />
        {card.durationMs ? <span>{tc(card.durationMs)}</span> : null}
      </span>
      <span className="relative flex h-11 gap-1">
        {SHOWN.map((f) => (
          <span
            key={f}
            className={`h-11 min-w-0 flex-1 overflow-hidden rounded-[3px] ${ready ? "bg-[#2b2b30]" : "border border-dashed border-[#2a2a2f]"}`}
          >
            {ready && f < card.frames && (
              <img src={frameSrc(cacheDir, card, f)} alt="" draggable={false} className="h-full w-full object-cover" />
            )}
          </span>
        ))}
        {playing && (
          <video src={media(card.path)} autoPlay muted loop className="absolute inset-0 h-full w-full rounded-[3px] bg-film object-cover" />
        )}
      </span>
      {selected && (
        <svg
          className="pointer-events-none absolute left-1 top-[18px]"
          width="26"
          height="30"
          viewBox="0 0 26 30"
          fill="none"
          stroke="#ececef"
          strokeWidth="2.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M3 16c3 3 5 6 7 10C13 16 17 8 23 3" />
        </svg>
      )}
      {playing && !selected && (
        <svg className="pointer-events-none absolute left-3 top-[30px]" width="10" height="12" viewBox="0 0 10 12" fill="#ececef">
          <path d="M1 1l8 5-8 5z" />
        </svg>
      )}
    </button>
  );
});
