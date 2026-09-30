import { Image, Trash2, Video } from "lucide-react";
import type { Pile } from "../../lib/ipc";

function stack(count: number) {
  const layers = Math.min(count, 5);
  return (
    Array.from({ length: layers }, (_, k) => `0 ${-(k + 1) * 4}px 0 ${-(k + 1)}px var(${k % 2 ? "--line" : "--strong"})`).join(", ") ||
    "none"
  );
}

export function PilesRow({
  piles,
  hot,
  dim,
  onPick,
  dragOver,
  onMenu,
}: {
  onMenu: (pile: Pile, x: number, y: number) => void;
  piles: Pile[];
  hot: number | null;
  dim: Set<number> | null;
  onPick: (pile: Pile) => void;
  dragOver: number | null;
}) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(96px,1fr))] items-end gap-2.5 pt-3">
      {piles.map((p) => {
        const lit = p.id === hot || p.id === dragOver;
        return (
          <button
            key={p.id}
            type="button"
            data-pile-id={p.id}
            title={p.isTrash ? "Отправить в корзину Windows — ход можно забрать" : `Положить в «${p.name}»`}
            onClick={() => onPick(p)}
            onContextMenu={(e) => {
              e.preventDefault();
              if (!p.isTrash) onMenu(p, e.clientX, e.clientY);
            }}
            className={`flex h-[84px] flex-col justify-between rounded-2xl px-3 py-2.5 text-left transition-[transform,background-color,opacity] duration-200 ease-trail ${
              lit ? "-translate-y-2 bg-fg text-ink outline-[1.5px] outline-offset-4 outline-fg outline-dashed" : p.isTrash ? "border-[1.5px] border-dashed border-line bg-transparent" : "bg-raised"
            } ${dim && !dim.has(p.id) && !lit ? "opacity-30" : ""}`}
            style={{ boxShadow: p.isTrash ? "none" : stack(p.count) }}
          >
            <span className="flex items-center justify-between gap-1">
              <span className="text-base font-bold">{p.key === "Delete" ? "Del" : (p.key ?? "·")}</span>
              <span className="flex items-center gap-1.5 text-[11px] font-medium opacity-75">
                {p.videos > 0 && (
                  <span className="flex items-center gap-0.5" title={`Видео: ${p.videos}`}>
                    <Video className="h-[11px] w-[11px]" strokeWidth={2} />
                    {p.videos}
                  </span>
                )}
                {p.photos > 0 && (
                  <span className="flex items-center gap-0.5" title={`Фото: ${p.photos}`}>
                    <Image className="h-[11px] w-[11px]" strokeWidth={2} />
                    {p.photos}
                  </span>
                )}
                {p.count === 0 && "0"}
              </span>
            </span>
            <span className={`flex items-center gap-1.5 truncate text-[13px] ${lit ? "font-bold" : "font-medium"}`}>
              {p.isTrash && <Trash2 className="h-3.5 w-3.5 shrink-0" strokeWidth={1.5} />}
              {p.name}
            </span>
          </button>
        );
      })}
    </div>
  );
}
