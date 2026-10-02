import { useEffect, useState } from "react";
import { frameSrc, type Card } from "../../lib/ipc";

export const FRAMES = 8;

const tc = (ms: number) => `${Math.floor(ms / 60000)}:${String(Math.floor((ms % 60000) / 1000)).padStart(2, "0")}`;

export function frameTime(i: number, durationMs: number) {
  return (durationMs * (2 * i + 1)) / (2 * FRAMES);
}

function topVideo() {
  return document.querySelector<HTMLVideoElement>("[data-card-top] video");
}

export function seekFrame(card: Card, step: number) {
  const v = topVideo();
  const d = card.durationMs || (v && Number.isFinite(v.duration) ? v.duration * 1000 : 0);
  if (!v || !d) return;
  const now = Math.min(FRAMES - 1, Math.floor(((v.currentTime * 1000) / d) * FRAMES));
  const next = Math.max(0, Math.min(FRAMES - 1, now + step));
  v.currentTime = frameTime(next, d) / 1000;
}

function Holes() {
  return (
    <div className="flex gap-2.5 overflow-hidden">
      {Array.from({ length: 48 }, (_, i) => (
        <div key={i} className="h-1.5 w-2.5 shrink-0 rounded-[2px] bg-hole" />
      ))}
    </div>
  );
}

export function FilmStrip({ card, cacheDir }: { card: Card; cacheDir: string }) {
  const [active, setActive] = useState(-1);

  useEffect(() => {
    const v = topVideo();
    if (!v) return;
    const update = () => {
      const d = card.durationMs || (Number.isFinite(v.duration) ? v.duration * 1000 : 0);
      if (d) setActive(Math.min(FRAMES - 1, Math.floor(((v.currentTime * 1000) / d) * FRAMES)));
    };
    v.addEventListener("timeupdate", update);
    return () => v.removeEventListener("timeupdate", update);
  }, [card.id, card.durationMs]);

  const ready = card.frames > 0;
  const d = card.durationMs ?? 0;
  return (
    <div className="flex flex-col gap-1.5 rounded-[20px] bg-film px-3 py-2">
      <Holes />
      <div className="flex gap-[5px]">
        {Array.from({ length: FRAMES }, (_, i) => (
          <button
            key={i}
            type="button"
            tabIndex={-1}
            aria-label={`Кадр ${i + 1}`}
            disabled={!ready || i >= card.frames}
            onClick={() => {
              const v = topVideo();
              if (v && d) v.currentTime = frameTime(i, d) / 1000;
            }}
            className={`relative h-[62px] min-w-0 flex-1 overflow-hidden rounded-[4px] ${ready ? "bg-[#2b2b30]" : "border border-dashed border-[#2a2a2f]"}`}
          >
            {ready && i < card.frames && (
              <img src={frameSrc(cacheDir, card, i)} alt="" draggable={false} className="h-full w-full object-cover" />
            )}
            {d > 0 && (
              <span className="absolute bottom-1 left-1.5 font-mono text-[10px] font-medium text-[#ececef] [text-shadow:0_1px_2px_rgb(0_0_0/80%)]">
                {tc(frameTime(i, d))}
              </span>
            )}
            {i === active && ready && (
              <svg className="pointer-events-none absolute -inset-1" viewBox="0 0 110 86" preserveAspectRatio="none" fill="none" stroke="#ececef" strokeWidth="5" strokeLinecap="round">
                <path d="M14 20C28 4 86 2 100 22C112 40 98 78 56 82C16 84 2 62 6 40C8 30 14 22 24 18" vectorEffect="non-scaling-stroke" style={{ strokeWidth: 2.5 }} />
              </svg>
            )}
          </button>
        ))}
      </div>
      <Holes />
      {!ready && (
        <div className="text-center font-mono text-[10px] tracking-[0.06em] text-[#86868d]">
          {card.stage === "broken" ? "не проявилось" : "проявляется…"}
        </div>
      )}
    </div>
  );
}
