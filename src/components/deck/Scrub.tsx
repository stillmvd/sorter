import { useState, type PointerEvent } from "react";
import { frameSrc, type Card } from "../../lib/ipc";

export const FRAMES = 8;

export function clock(s: number) {
  const n = Math.floor(s);
  const h = Math.floor(n / 3600);
  const ss = String(n % 60).padStart(2, "0");
  return h ? `${h}:${String(Math.floor((n % 3600) / 60)).padStart(2, "0")}:${ss}` : `${Math.floor(n / 60)}:${ss}`;
}

export function Scrub({
  card,
  cacheDir,
  tall,
  t,
  d,
  onSeek,
}: {
  card: Card;
  cacheDir: string;
  tall: boolean;
  t: number;
  d: number;
  onSeek: (s: number) => void;
}) {
  const [at, setAt] = useState<number | null>(null);
  const pos = (e: PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    return Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
  };
  const half = (tall ? 27 : 56) - 14;
  const frame = at === null || card.frames === 0 ? -1 : Math.min(card.frames - 1, Math.floor(at * FRAMES));

  return (
    <div
      role="slider"
      tabIndex={-1}
      aria-label="Перемотка — ← →"
      aria-valuemin={0}
      aria-valuemax={Math.round(d)}
      aria-valuenow={Math.round(t)}
      aria-valuetext={`${clock(t)} из ${clock(d)}`}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        onSeek(pos(e) * d);
      }}
      onPointerMove={(e) => {
        const p = pos(e);
        setAt(p);
        if (e.currentTarget.hasPointerCapture(e.pointerId)) onSeek(p * d);
      }}
      onPointerLeave={() => setAt(null)}
      className="absolute inset-x-3.5 bottom-1 h-7 cursor-pointer"
    >
      <div className="absolute inset-x-0 bottom-2.5 h-1 rounded-full bg-[rgb(255_255_255/26%)] transition-[height,bottom] duration-200 ease-trail group-hover:bottom-2 group-hover:h-2">
        <div className="absolute inset-y-0 left-0 rounded-full bg-[#ececef]" style={{ width: `${(t / d) * 100}%` }} />
        <div
          className="absolute top-1/2 -ml-[7px] -mt-[7px] h-3.5 w-3.5 rounded-full bg-[#ececef] opacity-0 shadow-[0_1px_4px_rgb(0_0_0/40%)] transition-opacity duration-200 ease-trail group-hover:opacity-100"
          style={{ left: `${(t / d) * 100}%` }}
        />
        {at !== null && (
          <div
            className="pointer-events-none absolute bottom-[18px] flex -translate-x-1/2 flex-col items-center gap-1"
            style={{ left: frame < 0 ? `${at * 100}%` : `clamp(${half}px, ${at * 100}%, calc(100% - ${half}px))` }}
          >
            {frame >= 0 && (
              <span className={`block overflow-hidden rounded-lg shadow-[0_0_0_2px_#ececef,0_8px_20px_rgb(0_0_0/45%)] ${tall ? "h-24 w-[54px]" : "h-[63px] w-28"}`}>
                <img src={frameSrc(cacheDir, card, frame)} alt="" draggable={false} className="h-full w-full object-cover" />
              </span>
            )}
            <b className="rounded-md bg-[rgb(12_12_14/82%)] px-1.5 py-0.5 text-[11px] tabular-nums text-[#ececef]">{clock(at * d)}</b>
          </div>
        )}
      </div>
    </div>
  );
}
