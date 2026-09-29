import { memo, useRef, useState, type MouseEvent, type PointerEvent } from "react";
import { frameSrc, ipc, media, type Card } from "../../lib/ipc";
import { FRAMES } from "../deck/FilmStrip";

const tc = (ms: number) => `${Math.floor(ms / 60000)}:${String(Math.floor((ms % 60000) / 1000)).padStart(2, "0")}`;

const chip = "absolute flex h-5 items-center rounded-md bg-[rgb(12_12_14/72%)] px-1.5 font-mono text-[10px] font-medium tracking-[0.06em]";

export const Tile = memo(function Tile({
  card,
  no,
  w,
  h,
  cacheDir,
  selected,
  onPick,
  onPress,
}: {
  card: Card;
  no: number;
  w: number;
  h: number;
  cacheDir: string;
  selected: boolean;
  onPick: (e: MouseEvent, card: Card) => void;
  onPress: (e: PointerEvent, card: Card) => void;
}) {
  const [pct, setPct] = useState<number | null>(null);
  const [live, setLive] = useState(false);
  const [src, setSrc] = useState(() => media(card.path));
  const fixed = useRef(false);
  const video = useRef<HTMLVideoElement>(null);
  const want = useRef(0);

  const seek = () => {
    const v = video.current;
    if (!v || !v.duration || v.seeking) return;
    const at = want.current * v.duration;
    if (Math.abs(v.currentTime - at) > 0.04) v.currentTime = at;
  };

  const move = (e: MouseEvent<HTMLButtonElement>) => {
    if ("dragging" in document.body.dataset) return;
    const r = e.currentTarget.getBoundingClientRect();
    const p = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    want.current = p;
    setPct(p);
    seek();
  };

  const scrubbing = pct !== null;
  const frame = scrubbing ? Math.min(card.frames - 1, Math.floor(pct * FRAMES)) : Math.min(card.frames - 1, FRAMES / 2 - 1);
  const d = card.durationMs ?? 0;

  return (
    <button
      type="button"
      data-tile-id={card.id}
      aria-pressed={selected}
      aria-label={card.fileName}
      title={card.fileName}
      onClick={(e) => onPick(e, card)}
      onMouseMove={move}
      onMouseLeave={() => {
        setPct(null);
        setLive(false);
      }}
      onPointerDown={(e) => onPress(e, card)}
      className={`relative shrink-0 select-none overflow-hidden rounded-xl bg-[#26262a] ${
        selected ? "outline outline-[1.5px] outline-offset-[3px] outline-[#ececef]" : ""
      }`}
      style={{ width: w, height: h }}
    >
      {card.frames > 0 ? (
        <img src={frameSrc(cacheDir, card, frame)} alt="" draggable={false} className="absolute inset-0 h-full w-full object-cover" />
      ) : (
        <span className="absolute inset-2 grid place-items-center rounded-lg border border-dashed border-[#3d3d44] font-mono text-[10px] tracking-[0.06em] text-[#86868d]">
          {card.stage === "broken" ? "не проявилось" : "проявляется…"}
        </span>
      )}
      {scrubbing && (
        <video
          ref={video}
          src={src}
          muted
          preload="auto"
          onError={() => {
            if (fixed.current) return;
            fixed.current = true;
            void ipc.playable(card.id).then((p) => setSrc(`${media(p)}?v=${Date.now()}`), () => undefined);
          }}
          onLoadedMetadata={seek}
          onSeeked={() => {
            setLive(true);
            seek();
          }}
          className={`absolute inset-0 h-full w-full object-cover ${live ? "" : "opacity-0"}`}
        />
      )}
      <span className={`${chip} left-2 top-2 ${selected ? "text-[#ececef]" : "text-[#a2a2a9]"}`}>{String(no).padStart(3, "0")}</span>
      {d > 0 && (
        <span className={`${chip} right-2 text-[#ececef] ${scrubbing ? "bottom-5" : "bottom-2"}`}>
          {scrubbing ? `${tc(pct * d)} / ${tc(d)}` : tc(d)}
        </span>
      )}
      {selected && (
        <span className="absolute right-1.5 top-1.5 grid h-8 w-8 place-items-center rounded-full bg-[rgb(12_12_14/72%)]">
          <svg width="20" height="22" viewBox="0 0 26 30" fill="none" stroke="#ececef" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 16c3 3 5 6 7 10C13 16 17 8 23 3" />
          </svg>
        </span>
      )}
      {scrubbing && (
        <>
          <span className="pointer-events-none absolute inset-y-0 w-px bg-[rgb(236_236_239/35%)]" style={{ left: `${pct * 100}%` }} />
          <span className="pointer-events-none absolute inset-x-2 bottom-2 h-[3px] rounded-full bg-[rgb(236_236_239/25%)]">
            <span className="block h-full rounded-full bg-[#ececef]" style={{ width: `${pct * 100}%` }} />
          </span>
          <span
            className="pointer-events-none absolute bottom-[5px] h-[9px] w-[9px] rounded-full bg-[#ececef]"
            style={{ left: `calc(8px + (100% - 16px) * ${pct} - 4.5px)` }}
          />
        </>
      )}
    </button>
  );
});
