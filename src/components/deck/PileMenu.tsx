import { Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import type { Pile } from "../../lib/ipc";

export function PileMenu({
  pile,
  x,
  y,
  onRemove,
  onClose,
}: {
  pile: Pile;
  x: number;
  y: number;
  onRemove: () => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const close = (e: Event) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", esc, true);
    ref.current?.querySelector("button")?.focus();
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", esc, true);
    };
  }, [onClose]);

  const empty = pile.count === 0;
  return (
    <div
      ref={ref}
      role="menu"
      className="fixed z-50 flex w-[260px] flex-col gap-1 rounded-[20px] bg-raised p-2 shadow-[0_20px_50px_rgb(0_0_0/40%)]"
      style={{ left: Math.min(x, window.innerWidth - 276), top: Math.max(8, y - 120) }}
    >
      <div className="truncate px-3 pt-1.5 pb-1 text-[13px] font-bold">{pile.name}</div>
      <button
        type="button"
        role="menuitem"
        disabled={!empty}
        onClick={onRemove}
        className="flex h-10 items-center gap-2.5 rounded-full px-3 text-left text-sm font-medium hover:bg-strong disabled:cursor-default disabled:text-dim disabled:hover:bg-transparent"
      >
        <Trash2 className="h-4 w-4 shrink-0" strokeWidth={1.5} />
        Удалить стопку
      </button>
      <div className="px-3 pb-1.5 text-xs leading-normal text-dim">
        {empty
          ? "Пустая папка удалится с диска, клавиша освободится."
          : `В стопке ${pile.count} видео — удалить можно только пустую. Сначала забери или переложи их.`}
      </div>
    </div>
  );
}
