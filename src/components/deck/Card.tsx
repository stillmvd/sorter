import { useEffect, useRef, useState } from "react";
import { media, type Card as CardData } from "../../lib/ipc";

const tc = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

type Size = { w: number; h: number };

function fit(zone: number, ratio: number): Size {
  return ratio >= 1 ? { w: zone, h: Math.round(zone / ratio) } : { w: Math.round(zone * ratio), h: zone };
}

function ratioOf(card: CardData, measured: Record<number, number>) {
  if (measured[card.id]) return measured[card.id];
  if (card.width && card.height) return card.width / card.height;
  return 9 / 16;
}

function Face({
  card,
  active,
  muted,
  paused,
  number,
  onRatio,
  onBroken,
  broken,
}: {
  card: CardData;
  active: boolean;
  muted: boolean;
  paused: boolean;
  number: number;
  onRatio: (r: number) => void;
  onBroken: () => void;
  broken: boolean;
}) {
  const ref = useRef<HTMLVideoElement>(null);
  const [time, setTime] = useState({ t: 0, d: 0 });

  useEffect(() => {
    const v = ref.current;
    if (!v) return;
    if (active && !paused) void v.play().catch(() => undefined);
    else v.pause();
  }, [active, paused]);

  const brokenRef = useRef(onBroken);
  brokenRef.current = onBroken;

  useEffect(() => {
    if (!active) return;
    const v = ref.current;
    const timer = window.setTimeout(() => {
      if (v && v.readyState < 2) brokenRef.current();
    }, 1500);
    return () => window.clearTimeout(timer);
  }, [active]);

  return (
    <div className="flex h-full flex-col gap-2.5 p-3">
      <div className="flex items-center justify-between px-1 pt-0.5 text-[#ececef]">
        <span className="text-[22px] leading-none font-bold">{number}</span>
        <span className="text-[11px] font-medium text-[#a2a2a9]">
          {time.d ? `${tc(time.t)} / ${tc(time.d)}` : ""}
          {muted ? "" : " · звук"}
        </span>
      </div>
      <div className="relative min-h-0 flex-1 overflow-hidden rounded-2xl bg-[#1d1d21]">
        {!broken && (
          <video
            ref={ref}
            src={media(card.path)}
            muted={muted}
            loop
            playsInline
            preload="auto"
            className="h-full w-full object-contain"
            onLoadedMetadata={(e) => {
              const v = e.currentTarget;
              if (v.videoWidth && v.videoHeight) onRatio(v.videoWidth / v.videoHeight);
              setTime({ t: 0, d: v.duration || 0 });
            }}
            onTimeUpdate={(e) => setTime({ t: e.currentTarget.currentTime, d: e.currentTarget.duration || 0 })}
            onError={onBroken}
          />
        )}
        {broken && (
          <div className="absolute inset-0 grid place-items-center p-4 text-center text-[13px] font-medium text-[#a2a2a9]">
            Это видео не играет внутри Sorter.
            <br />
            Ctrl+O — открыть в плеере
          </div>
        )}
      </div>
      <div className="h-1 rounded-full bg-[#2e2e33]">
        <div className="h-1 rounded-full bg-[#ececef]" style={{ width: time.d ? `${(time.t / time.d) * 100}%` : 0 }} />
      </div>
    </div>
  );
}

export function CardStack({
  cards,
  number,
  zone,
  muted,
  paused,
  broken,
  onBroken,
  onDragStart,
}: {
  cards: CardData[];
  number: number;
  zone: number;
  muted: boolean;
  paused: boolean;
  broken: Set<number>;
  onBroken: (id: number) => void;
  onDragStart: (e: React.DragEvent) => void;
}) {
  const [measured, setMeasured] = useState<Record<number, number>>({});
  const top = useRef<HTMLDivElement>(null);
  const current = cards[0];
  const size = current ? fit(zone, ratioOf(current, measured)) : { w: 0, h: 0 };
  const landscape = size.w > size.h;
  const prevLandscape = useRef(landscape);

  useEffect(() => {
    const turned = prevLandscape.current !== landscape;
    prevLandscape.current = landscape;
    if (!turned || !top.current || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    top.current.animate(
      [{ transform: `rotate(${landscape ? -90 : 90}deg) scale(0.92)` }, { transform: "none" }],
      { duration: 320, easing: "cubic-bezier(0.2, 0, 0, 1)" },
    );
  }, [landscape, current?.id]);

  return (
    <div className="relative grid shrink-0 place-items-center" style={{ width: zone, height: zone }}>
      {cards.length > 1 && (
        <>
          <div
            className="absolute rounded-[28px] bg-line transition-[width,height] duration-[320ms] ease-trail"
            style={{ width: size.w, height: size.h, transform: "rotate(7deg) translate(16px, 6px)" }}
          />
          <div
            className="absolute rounded-[28px] bg-strong transition-[width,height] duration-[320ms] ease-trail"
            style={{ width: size.w, height: size.h, transform: "rotate(3.5deg) translate(8px, 3px)" }}
          />
        </>
      )}
      {cards.slice(0, 2).map((card, i) => (
        <div
          key={card.id}
          ref={i === 0 ? top : undefined}
          draggable={i === 0}
          onDragStart={onDragStart}
          aria-hidden={i !== 0}
          className={`absolute overflow-hidden rounded-[28px] border border-line bg-film shadow-[0_24px_60px_rgb(0_0_0/35%)] transition-[width,height] duration-[320ms] ease-trail ${
            i === 0 ? "z-10 cursor-grab active:cursor-grabbing" : "pointer-events-none opacity-0"
          }`}
          style={{ width: size.w, height: size.h }}
        >
          <Face
            card={card}
            active={i === 0}
            muted={muted}
            paused={paused}
            number={number + i}
            broken={broken.has(card.id)}
            onBroken={() => onBroken(card.id)}
            onRatio={(r) => setMeasured((m) => (m[card.id] === r ? m : { ...m, [card.id]: r }))}
          />
        </div>
      ))}
    </div>
  );
}
