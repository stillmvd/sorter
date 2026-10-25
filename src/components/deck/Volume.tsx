import { Volume1, Volume2, VolumeX } from "lucide-react";
import { useEffect, useRef, useState, type PointerEvent } from "react";

export function Volume({
  muted,
  volume,
  onMute,
  onVolume,
  poke,
}: {
  muted: boolean;
  volume: number;
  onMute: () => void;
  onVolume: (v: number) => void;
  poke: number;
}) {
  const level = muted ? 0 : volume;
  const pct = Math.round(level * 100);
  const Icon = level === 0 ? VolumeX : level < 0.5 ? Volume1 : Volume2;
  const [lit, setLit] = useState(false);
  const prev = useRef(`${level}:${poke}`);

  useEffect(() => {
    const key = `${level}:${poke}`;
    if (prev.current === key) return;
    prev.current = key;
    setLit(true);
    const t = window.setTimeout(() => setLit(false), 1500);
    return () => window.clearTimeout(t);
  }, [level, poke]);

  const set = (e: PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    onVolume(1 - (e.clientY - r.top) / r.height);
  };

  return (
    <div
      className={`absolute right-3.5 top-1/2 flex w-10 -translate-y-1/2 flex-col-reverse items-center gap-2.5 rounded-full bg-[rgb(12_12_14/72%)] py-3 text-[11px] font-bold tabular-nums text-[#ececef] backdrop-blur-[8px] transition-opacity duration-200 ease-trail ${
        lit ? "opacity-100" : "opacity-0 group-hover:opacity-100"
      }`}
    >
      <button
        type="button"
        aria-label={muted ? "Включить звук" : "Выключить звук"}
        title={muted ? "Включить звук — M" : "Выключить звук — M"}
        onClick={onMute}
        className="grid h-4 w-4 place-items-center"
      >
        <Icon className="block h-4 w-4" strokeWidth={1.5} />
      </button>
      <div
        role="slider"
        tabIndex={-1}
        aria-label="Громкость видео — ↑ ↓"
        aria-orientation="vertical"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          set(e);
        }}
        onPointerMove={(e) => {
          if (e.currentTarget.hasPointerCapture(e.pointerId)) set(e);
        }}
        className="grid h-28 w-10 cursor-pointer justify-items-center"
      >
        <span className="relative h-full w-1 rounded-full bg-[rgb(255_255_255/26%)]">
          <span className="absolute inset-x-0 bottom-0 rounded-full bg-[#ececef]" style={{ height: `${pct}%` }} />
          <span className="absolute left-1/2 -mb-1.5 -ml-1.5 h-3 w-3 rounded-full bg-[#ececef]" style={{ bottom: `${pct}%` }} />
        </span>
      </div>
      <span>{pct}</span>
    </div>
  );
}
