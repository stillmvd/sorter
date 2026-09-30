import { frameSrc, type SeriesView } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { Button } from "../ui/Button";
import { Segment } from "../ui/Segment";

export type Rest = "trash" | "keep";

const Check = ({ size }: { size: number }) => (
  <svg width={size} height={size + 2} viewBox="0 0 26 30" fill="none" stroke="#ececef" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3 16c3 3 5 6 7 10C13 16 17 8 23 3" />
  </svg>
);

export function seriesSpan(s: SeriesView) {
  const times = s.cards.map((c) => c.takenAt ?? 0);
  const naive = s.cards[0]?.takenFrom === "exif";
  const t = (ms: number) =>
    new Date(ms).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit", second: "2-digit", timeZone: naive ? "UTC" : undefined });
  return { from: t(Math.min(...times)), to: t(Math.max(...times)), seconds: Math.round((Math.max(...times) - Math.min(...times)) / 1000), naive };
}

export function SeriesPanel({
  series,
  cacheDir,
  focus,
  marked,
  rest,
  onFocus,
  onToggle,
  onRest,
  onSplit,
}: {
  series: SeriesView;
  cacheDir: string;
  focus: number;
  marked: Set<number>;
  rest: Rest;
  onFocus: (i: number) => void;
  onToggle: (id: number) => void;
  onRest: (r: Rest) => void;
  onSplit: () => void;
}) {
  const n = series.cards.length;
  const span = seriesSpan(series);
  const first = series.cards[0];
  const day = first?.takenAt
    ? new Date(first.takenAt).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric", timeZone: span.naive ? "UTC" : undefined })
    : null;
  const left = n - marked.size;
  return (
    <div className="flex flex-col gap-3">
      <div className="flex min-w-0 items-baseline gap-2.5">
        <div className="shrink-0 text-lg font-bold">
          Серия из {n} {plural(n, "снимка", "снимков", "снимков")}
        </div>
        <div className="truncate text-[13px] font-medium text-dim">
          {[day, `за ${span.seconds} с`, `отмечено ${marked.size}`].filter(Boolean).join(" · ")}
        </div>
      </div>
      <div className="grid max-h-[196px] grid-cols-4 gap-2 overflow-y-auto rounded-[20px] bg-film p-2.5">
        {series.cards.map((c, i) => {
          const on = marked.has(c.id);
          const focused = i === focus;
          return (
            <button
              key={c.id}
              type="button"
              aria-pressed={on}
              aria-label={`Снимок ${i + 1}${on ? ", отмечен" : ""}`}
              onClick={() => (focused ? onToggle(c.id) : onFocus(i))}
              onDoubleClick={() => onToggle(c.id)}
              className={`relative h-[84px] overflow-hidden rounded-[10px] bg-[#26262a] ${
                focused ? "outline outline-[1.5px] outline-offset-2 outline-[#ececef]" : on ? "outline outline-[1.5px] outline-offset-2 outline-[#6a6a72]" : ""
              }`}
            >
              {c.frames > 0 && <img src={frameSrc(cacheDir, c, 0)} alt="" draggable={false} className="absolute inset-0 h-full w-full object-cover" />}
              <span className="absolute left-1.5 top-1.5 flex h-[18px] items-center rounded-md bg-[rgb(12_12_14/72%)] px-1.5 font-mono text-[10px] font-medium text-[#a2a2a9]">
                {i + 1}
              </span>
              {c.id === series.best && (
                <span className="absolute bottom-1.5 left-1.5 flex h-[18px] items-center rounded-md bg-[rgb(12_12_14/72%)] px-1.5 text-[10px] font-bold text-[#ececef]">
                  лучший
                </span>
              )}
              {on && (
                <span className="absolute right-1 top-1 grid h-[26px] w-[26px] place-items-center rounded-full bg-[rgb(12_12_14/72%)]">
                  <Check size={15} />
                </span>
              )}
            </button>
          );
        })}
      </div>
      <div className="flex flex-wrap items-center gap-3">
        {marked.size > 0 && left > 0 && (
          <>
            <div className="text-[13px] font-medium text-dim">Остальные {left}</div>
            <Segment
              label="Куда остальные"
              value={rest}
              onChange={onRest}
              options={[
                { value: "trash", label: "в Корзину" },
                { value: "keep", label: "оставить в колоде" },
              ]}
            />
          </>
        )}
        {marked.size === 0 && <div className="text-[13px] font-medium text-dim">Ничего не отмечено — в стопку уйдёт вся серия</div>}
        <div className="flex-1" />
        <Button size={36} onClick={onSplit} hotkey="S">
          Разбить серию
        </Button>
      </div>
    </div>
  );
}

export { Check as SeriesCheck };
