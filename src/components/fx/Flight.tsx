import { useLayoutEffect, useRef } from "react";

export type Flight = { id: number; pileId: number; from: DOMRect; to: DOMRect; image: string | null };

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

export function pileElement(pileId: number) {
  return document.querySelector<HTMLElement>(`[data-pile-id="${pileId}"]`);
}

export function shake(pileId: number) {
  const el = pileElement(pileId);
  if (!el || reduced()) return;
  el.animate(
    [
      { transform: "translateY(0) rotate(0)" },
      { transform: "translateY(4px) rotate(-2.5deg)" },
      { transform: "translateY(-2px) rotate(2deg)" },
      { transform: "translateY(0) rotate(0)" },
    ],
    { duration: 220, easing: "cubic-bezier(0.2, 0, 0, 1)", composite: "add" },
  );
}

export function snapshot(): { rect: DOMRect; image: string | null } | null {
  const card = document.querySelector<HTMLElement>("[data-card-top]");
  if (!card) return null;
  const video = card.querySelector("video");
  let image: string | null = null;
  if (video && video.videoWidth) {
    try {
      const canvas = document.createElement("canvas");
      const scale = 240 / Math.max(video.videoWidth, video.videoHeight);
      canvas.width = Math.round(video.videoWidth * scale);
      canvas.height = Math.round(video.videoHeight * scale);
      canvas.getContext("2d")!.drawImage(video, 0, 0, canvas.width, canvas.height);
      image = canvas.toDataURL("image/jpeg", 0.7);
    } catch {
      image = null;
    }
  }
  return { rect: card.getBoundingClientRect(), image };
}

function Ghost({ flight, onDone }: { flight: Flight; onDone: (f: Flight) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { from, to } = flight;
    const dx = to.left + to.width / 2 - (from.left + from.width / 2);
    const dy = to.top + to.height / 2 - (from.top + from.height / 2);
    const s = Math.min(to.width / from.width, to.height / from.height) * 0.9;
    const anim = el.animate(
      [
        { transform: "translate(0, 0) scale(1) rotate(0)", opacity: 1 },
        { transform: `translate(${dx * 0.55}px, ${dy * 0.35 - 30}px) scale(${(1 + s) / 2}) rotate(-6deg)`, opacity: 1, offset: 0.55 },
        { transform: `translate(${dx}px, ${dy}px) scale(${s}) rotate(4deg)`, opacity: 0.2 },
      ],
      { duration: 260, easing: "cubic-bezier(0.4, 0, 0.9, 1)", fill: "forwards" },
    );
    anim.finished.then(() => onDone(flight)).catch(() => onDone(flight));
    return () => anim.cancel();
  }, [flight, onDone]);

  return (
    <div
      ref={ref}
      className="pointer-events-none fixed z-50 overflow-hidden rounded-[28px] border border-line bg-film shadow-[0_24px_60px_rgb(0_0_0/35%)]"
      style={{ left: flight.from.left, top: flight.from.top, width: flight.from.width, height: flight.from.height }}
    >
      {flight.image && <img src={flight.image} alt="" className="h-full w-full object-contain p-3" />}
    </div>
  );
}

export function Flights({ flights, onDone }: { flights: Flight[]; onDone: (f: Flight) => void }) {
  return (
    <>
      {flights.map((f) => (
        <Ghost key={f.id} flight={f} onDone={onDone} />
      ))}
    </>
  );
}

export const motionOff = reduced;
