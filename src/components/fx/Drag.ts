import type { PointerEvent as ReactPointerEvent } from "react";

export type DragPiece = { el: HTMLElement; image: string | null };

export type DragSpec = {
  pieces: DragPiece[];
  grabbed: HTMLElement;
  count: number;
  radius: number;
};

type Handlers = {
  prepare: () => DragSpec | null;
  onOver: (pileId: number | null) => void;
  onDrop: (pileId: number) => void;
};

const EASE = "cubic-bezier(0.2, 0, 0, 1)";
const THRESHOLD = 5;
const LIFT_H = 200;
const SHOWN = 5;
const SHRINK = 0.3;

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

function pileAt(x: number, y: number) {
  const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-pile-id]");
  return el ? Number(el.dataset.pileId) : null;
}

function swallowClick() {
  const stop = (e: MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
  };
  window.addEventListener("click", stop, { capture: true, once: true });
  window.setTimeout(() => window.removeEventListener("click", stop, { capture: true }), 0);
}

function lift(spec: DragSpec, x0: number, y0: number, h: Handlers) {
  const dur = (ms: number) => (reduced() ? 0 : ms);
  const grabbedRect = spec.grabbed.getBoundingClientRect();
  const s = Math.min(1, LIFT_H / grabbedRect.height);
  const gx = (x0 - grabbedRect.left) * s;
  const gy = (y0 - grabbedRect.top) * s;

  const layer = document.createElement("div");
  layer.style.cssText = "position:fixed;left:0;top:0;z-index:60;pointer-events:none;will-change:transform";
  layer.style.transform = `translate(${x0}px, ${y0}px)`;
  const group = document.createElement("div");
  group.style.cssText = `position:absolute;left:0;top:0;transform-origin:0 0;transition:transform ${dur(200)}ms ${EASE}`;
  layer.appendChild(group);
  document.body.appendChild(layer);
  document.body.style.cursor = "grabbing";
  document.body.dataset.dragging = "";

  const order = [spec.pieces.find((p) => p.el === spec.grabbed)!, ...spec.pieces.filter((p) => p.el !== spec.grabbed)]
    .filter(Boolean)
    .slice(0, SHOWN)
    .reverse();

  const items = order.map((piece, k) => {
    const depth = order.length - 1 - k;
    const r = piece.el.getBoundingClientRect();
    const node = document.createElement("div");
    node.style.cssText = `position:absolute;left:0;top:0;width:${r.width}px;height:${r.height}px;transform-origin:0 0;border-radius:${spec.radius}px;overflow:hidden;background:#26262a center/cover no-repeat;box-shadow:0 18px 40px rgb(0 0 0/45%)`;
    if (piece.image) node.style.backgroundImage = `url("${piece.image}")`;
    group.appendChild(node);
    const ps = Math.min(1, LIFT_H / r.height);
    const tilt = depth === 0 ? -2 : depth % 2 ? 4 + depth : -3 - depth;
    const to = `translate(${-gx + depth * 7}px, ${-gy + depth * 5}px) scale(${ps}) rotate(${tilt}deg)`;
    node.animate([{ transform: `translate(${r.left - x0}px, ${r.top - y0}px) scale(1) rotate(0deg)` }, { transform: to }], { duration: dur(220), easing: EASE, fill: "forwards" });
    return { node, piece, to, w: r.width * ps, h: r.height * ps };
  });

  let badge: HTMLDivElement | null = null;
  if (spec.count > 1) {
    badge = document.createElement("div");
    badge.textContent = String(spec.count);
    badge.style.cssText = `position:absolute;left:0;top:0;min-width:28px;height:28px;padding:0 8px;border-radius:999px;background:var(--fg);color:var(--ink);font:700 13px 'Kockers Sans',sans-serif;display:grid;place-items:center;box-shadow:0 6px 16px rgb(0 0 0/40%)`;
    badge.style.transition = `transform ${dur(200)}ms ${EASE}`;
    badge.style.transform = `translate(${-gx + items[items.length - 1].w - 16}px, ${-gy - 12}px)`;
    badge.animate([{ opacity: 0, scale: 0.6 }, { opacity: 1, scale: 1 }], { duration: dur(200), delay: dur(120), easing: EASE, fill: "backwards" });
    layer.appendChild(badge);
  }

  for (const p of spec.pieces) p.el.animate([{ opacity: 1 }, { opacity: 0.25 }], { duration: dur(160), fill: "forwards" });
  const restore = () => spec.pieces.forEach((p) => p.el.getAnimations().forEach((a) => a.cancel()));

  const top = items[items.length - 1];
  const shrink = (on: boolean) => {
    group.style.transform = on ? `translate(${14 + gx * SHRINK}px, ${gy * SHRINK - top.h * SHRINK - 14}px) scale(${SHRINK})` : "";
    if (badge)
      badge.style.transform = on
        ? `translate(${14 + top.w * SHRINK - 12}px, ${-top.h * SHRINK - 28}px)`
        : `translate(${-gx + top.w - 16}px, ${-gy - 12}px)`;
  };

  let over: number | null = null;
  const move = (e: PointerEvent) => {
    layer.style.transform = `translate(${e.clientX}px, ${e.clientY}px)`;
    const pile = pileAt(e.clientX, e.clientY);
    if (pile !== over) {
      over = pile;
      h.onOver(pile);
      shrink(pile !== null);
    }
  };

  const finish = () => {
    layer.remove();
    document.body.style.cursor = "";
    delete document.body.dataset.dragging;
  };

  const up = (e: PointerEvent) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("pointercancel", up);
    swallowClick();
    h.onOver(null);
    const pile = e.type === "pointerup" ? pileAt(e.clientX, e.clientY) : null;
    badge?.remove();
    if (pile !== null) {
      const target = document.querySelector(`[data-pile-id="${pile}"]`)!.getBoundingClientRect();
      const cx = target.left + target.width / 2 - e.clientX;
      const cy = target.top + target.height / 2 - e.clientY;
      const from = getComputedStyle(group).transform;
      group.style.transition = "none";
      const anims = [
        group.animate(
          [
            { transform: from, opacity: 1 },
            { transform: `translate(${cx}px, ${cy}px) scale(0.05)`, opacity: 0 },
          ],
          { duration: dur(200), easing: "cubic-bezier(0.4, 0, 0.9, 1)", fill: "forwards" },
        ),
      ];
      h.onDrop(pile);
      void Promise.allSettled(anims.map((a) => a.finished)).then(() => {
        finish();
        restore();
      });
    } else {
      shrink(false);
      const anims = items.map(({ node, piece, to }) => {
        const r = piece.el.getBoundingClientRect();
        const back = `translate(${r.left - e.clientX}px, ${r.top - e.clientY}px) scale(1) rotate(0deg)`;
        return node.animate([{ transform: to }, { transform: back }], { duration: dur(260), easing: EASE, fill: "forwards" });
      });
      void Promise.allSettled(anims.map((a) => a.finished)).then(() => {
        restore();
        finish();
      });
    }
  };

  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("pointercancel", up);
}

export function pressToDrag(e: ReactPointerEvent, h: Handlers) {
  if (e.button !== 0) return;
  const x0 = e.clientX;
  const y0 = e.clientY;
  const move = (ev: PointerEvent) => {
    if (Math.hypot(ev.clientX - x0, ev.clientY - y0) < THRESHOLD) return;
    stop();
    const spec = h.prepare();
    if (spec) lift(spec, x0, y0, h);
  };
  const stop = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", stop);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", stop);
}
