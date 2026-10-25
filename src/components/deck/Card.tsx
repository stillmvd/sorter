import { useEffect, useRef, useState } from "react";
import { ipc, media, type Card as CardData } from "../../lib/ipc";
import { Scrub, clock } from "./Scrub";
import { Volume } from "./Volume";

export type Size = { w: number; h: number };

function fit(box: Size, ratio: number): Size {
  const w = Math.min(box.w, box.h * ratio);
  return { w: Math.round(w), h: Math.round(w / ratio) };
}

function ratioOf(card: CardData, measured: Record<number, number>) {
  const r = measured[card.id] ?? (card.width && card.height ? card.width / card.height : card.kind === "photo" ? 4 / 3 : 9 / 16);
  return card.kind === "photo" ? Math.max(0.62, Math.min(1.6, r)) : r;
}

export const seekStep = (seconds: number) => (seconds <= 20 ? 3 : seconds <= 60 ? 5 : seconds <= 300 ? 10 : 30);

export function seekBy(sign: number) {
  const v = document.querySelector<HTMLVideoElement>("[data-card-top] video");
  if (!v || !Number.isFinite(v.duration)) return;
  v.currentTime = Math.max(0, Math.min(v.duration - 0.05, v.currentTime + sign * seekStep(v.duration)));
}

export type SeriesBadge = { count: number; marked: boolean };

function PhotoFace({
  card,
  onRatio,
  onBroken,
  broken,
  series,
}: {
  series?: SeriesBadge;
  card: CardData;
  onRatio: (r: number) => void;
  onBroken: () => void;
  broken: boolean;
}) {
  const failed = broken || card.stage === "broken";
  return (
    <div className="relative grid h-full place-items-center">
      {!failed && (
        <img
          src={media(card.path)}
          alt=""
          draggable={false}
          decoding="async"
          className="h-full w-full object-contain"
          onLoad={(e) => {
            const img = e.currentTarget;
            if (img.naturalWidth && img.naturalHeight) onRatio(img.naturalWidth / img.naturalHeight);
          }}
          onError={onBroken}
        />
      )}
      {series?.marked && (
        <span className="absolute right-3.5 top-3.5 grid h-10 w-10 place-items-center rounded-full bg-[rgb(12_12_14/72%)]">
          <svg width="24" height="26" viewBox="0 0 26 30" fill="none" stroke="#ececef" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 16c3 3 5 6 7 10C13 16 17 8 23 3" />
          </svg>
        </span>
      )}
      {failed && (
        <div className="absolute inset-0 grid place-items-center p-4 text-center text-[13px] font-medium text-[#a2a2a9]">
          {card.error ?? "Это фото не открывается внутри Sorter."}
          <br />
          Ctrl+O — открыть в просмотрщике
        </div>
      )}
    </div>
  );
}

function Face({
  card,
  cacheDir,
  tall,
  active,
  muted,
  paused,
  onRatio,
  onBroken,
  broken,
  onTogglePause,
  volume,
  onVolume,
  onMute,
  poke,
}: {
  card: CardData;
  cacheDir: string;
  tall: boolean;
  active: boolean;
  muted: boolean;
  paused: boolean;
  onRatio: (r: number) => void;
  onBroken: () => void;
  broken: boolean;
  onTogglePause: () => void;
  volume: number;
  onVolume: (v: number) => void;
  onMute: () => void;
  poke: number;
}) {
  const ref = useRef<HTMLVideoElement>(null);
  const [time, setTime] = useState({ t: 0, d: 0 });
  const [src, setSrc] = useState(() => media(card.path));
  const fixing = useRef<"no" | "busy" | "done">("no");
  const shown = useRef(false);
  const resume = useRef(0);
  const [stoppedAt, setStoppedAt] = useState<number | null>(null);

  useEffect(() => {
    const v = ref.current;
    if (!v) return;
    if (active && !paused) void v.play().catch(() => undefined);
    else v.pause();
  }, [active, paused, src]);

  useEffect(() => {
    if (ref.current) ref.current.volume = volume;
  }, [volume]);

  const brokenRef = useRef(onBroken);
  brokenRef.current = onBroken;

  const fail = () => {
    const v = ref.current;
    if (shown.current && v) {
      resume.current = v.currentTime;
      setStoppedAt(v.currentTime);
    }
    if (fixing.current !== "no") {
      if (fixing.current === "done") brokenRef.current();
      return;
    }
    fixing.current = "busy";
    ipc
      .playable(card.id)
      .then((path) => {
        fixing.current = "done";
        setSrc(`${media(path)}?v=${Date.now()}`);
      })
      .catch(() => brokenRef.current());
  };

  useEffect(() => {
    if (!active) return;
    const v = ref.current;
    const timer = window.setTimeout(() => {
      if (v && !shown.current && v.readyState < 2 && fixing.current !== "busy") brokenRef.current();
    }, 5000);
    return () => window.clearTimeout(timer);
  }, [active, src]);

  const seek = (s: number) => {
    const v = ref.current;
    if (!v || !time.d) return;
    v.currentTime = Math.max(0, Math.min(time.d - 0.05, s));
    setTime({ t: v.currentTime, d: time.d });
  };

  return (
    <div className="relative h-full">
      {!broken && (
        <video
          ref={ref}
          src={src}
          crossOrigin="anonymous"
          muted={muted}
          loop
          playsInline
          preload="auto"
          className="h-full w-full cursor-pointer object-contain"
          onClick={active ? onTogglePause : undefined}
          onLoadedMetadata={(e) => {
            const v = e.currentTarget;
            if (v.videoWidth && v.videoHeight) onRatio(v.videoWidth / v.videoHeight);
            if (resume.current) v.currentTime = resume.current;
            setTime({ t: resume.current, d: v.duration || 0 });
          }}
          onLoadedData={() => {
            shown.current = true;
          }}
          onTimeUpdate={(e) => setTime({ t: e.currentTarget.currentTime, d: e.currentTarget.duration || 0 })}
          onError={fail}
        />
      )}
      {paused && active && !broken && (
        <div className="pointer-events-none absolute inset-0 grid place-items-center">
          <span className="grid h-14 w-14 place-items-center rounded-full bg-[rgb(12_12_14/70%)] text-[#ececef]">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
          </span>
        </div>
      )}
      {broken && (
        <div className="absolute inset-0 grid place-items-center p-4 text-center text-[13px] font-medium text-[#a2a2a9]">
          {stoppedAt === null ? "Это видео не играет внутри Sorter." : `Видео оборвалось на ${clock(stoppedAt)} — дальше файл не читается.`}
          <br />
          Ctrl+O — открыть в плеере
        </div>
      )}
      {active && !broken && <Volume muted={muted} volume={volume} onMute={onMute} onVolume={onVolume} poke={poke} />}
      {active && !broken && time.d > 0 && <Scrub card={card} cacheDir={cacheDir} tall={tall} t={time.t} d={time.d} onSeek={seek} />}
    </div>
  );
}

export function CardStack({
  cards,
  cacheDir,
  box,
  muted,
  paused,
  broken,
  onBroken,
  onTogglePause,
  volume,
  onVolume,
  onMute,
  poke,
  series,
}: {
  cards: CardData[];
  cacheDir: string;
  box: Size;
  muted: boolean;
  paused: boolean;
  broken: Set<number>;
  onBroken: (id: number) => void;
  onTogglePause: () => void;
  volume: number;
  onVolume: (v: number) => void;
  onMute: () => void;
  poke: number;
  series?: SeriesBadge;
}) {
  const [measured, setMeasured] = useState<Record<number, number>>({});
  const top = useRef<HTMLDivElement>(null);
  const current = cards[0];
  const size = current ? fit(box, ratioOf(current, measured)) : { w: 0, h: 0 };
  const landscape = size.w > size.h;
  const prevLandscape = useRef(landscape);

  useEffect(() => {
    for (const card of cards.slice(1, 4)) {
      if (card.kind !== "photo") continue;
      const img = new Image();
      img.src = media(card.path);
      void img.decode().catch(() => undefined);
    }
  }, [cards]);

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
    <div className="relative grid h-full w-full place-items-center">
      {(cards.length > 1 || (series?.count ?? 0) > 1) && (
        <>
          <div
            className="absolute rounded-[20px] bg-line transition-[width,height] duration-[320ms] ease-trail"
            style={{ width: size.w, height: size.h, transform: "rotate(7deg) translate(16px, 6px) scale(0.94)" }}
          />
          <div
            className="absolute rounded-[20px] bg-strong transition-[width,height] duration-[320ms] ease-trail"
            style={{ width: size.w, height: size.h, transform: "rotate(3.5deg) translate(8px, 3px) scale(0.97)" }}
          />
        </>
      )}
      {cards.slice(0, 2).map((card, i) => (
        <div
          key={card.id}
          ref={i === 0 ? top : undefined}
          data-card-top={i === 0 ? "" : undefined}
          aria-hidden={i !== 0}
          className={`group absolute overflow-hidden rounded-[20px] bg-[#1d1d21] shadow-[0_24px_60px_rgb(0_0_0/35%)] transition-[width,height] duration-[320ms] ease-trail ${
            i === 0 ? "z-10" : "pointer-events-none opacity-0"
          }`}
          style={{ width: size.w, height: size.h }}
        >
          {card.kind === "photo" ? (
            <PhotoFace
              card={card}
              series={i === 0 ? series : undefined}
              broken={broken.has(card.id)}
              onBroken={() => onBroken(card.id)}
              onRatio={(r) => setMeasured((m) => (m[card.id] === r ? m : { ...m, [card.id]: r }))}
            />
          ) : (
            <Face
              card={card}
              cacheDir={cacheDir}
              tall={!landscape}
              active={i === 0}
              muted={muted}
              paused={paused}
              broken={broken.has(card.id)}
              onBroken={() => onBroken(card.id)}
              onTogglePause={onTogglePause}
              volume={volume}
              onVolume={onVolume}
              onMute={onMute}
              poke={poke}
              onRatio={(r) => setMeasured((m) => (m[card.id] === r ? m : { ...m, [card.id]: r }))}
            />
          )}
        </div>
      ))}
    </div>
  );
}
