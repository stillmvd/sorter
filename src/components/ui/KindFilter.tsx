import type { Kind } from "../../lib/ipc";

const LABELS: Record<Kind, string> = { all: "Все", video: "Видео", photo: "Фото" };

export function KindFilter({ value, counts, onChange }: { value: Kind; counts: Record<Kind, number>; onChange: (k: Kind) => void }) {
  return (
    <div role="radiogroup" aria-label="Что раскладывать" className="flex gap-1 rounded-full bg-raised p-1">
      {(["all", "video", "photo"] as Kind[]).map((k) => {
        const on = k === value;
        return (
          <button
            key={k}
            type="button"
            role="radio"
            aria-checked={on}
            onClick={() => onChange(k)}
            className={`flex h-8 items-center gap-1.5 rounded-full px-3.5 text-[13px] transition-colors duration-200 ease-trail ${
              on ? "bg-fg font-bold text-ink" : "font-medium text-dim hover:text-fg"
            }`}
          >
            {LABELS[k]}
            <span className="text-[11px] font-medium opacity-70">{counts[k]}</span>
          </button>
        );
      })}
    </div>
  );
}
