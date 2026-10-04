import { useEffect, useMemo, useRef, useState } from "react";
import { isPhotoPath, media, placeLabel, type DupeGroup, type GroupItem } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { Button } from "../ui/Button";

export type Filter = "exact" | "same" | "crop" | "maybe";

export const SURE = 80;

const clock = (ms: number) => {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};
const mb = (b: number) => `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ`;
const name = (path: string) => path.split("\\").pop() ?? path;

const KIND = { exact: "Точная копия", same: "Та же запись", trim: "Обрезка", crop: "Кадрирована" } as const;

export const inFilter = (g: DupeGroup, f: Filter | null) => {
  if (f === "maybe") return g.confidence < SURE;
  if (g.confidence < SURE) return false;
  if (f === "exact") return g.kind === "exact";
  if (f === "same") return g.kind === "same" || g.kind === "trim";
  if (f === "crop") return g.kind === "crop";
  return true;
};

export const extraExact = (groups: DupeGroup[]) =>
  groups.filter((g) => g.kind === "exact" && g.confidence >= SURE).flatMap((g) => g.items.filter((i) => !i.best).map((i) => i.path));

function note(g: DupeGroup) {
  if (g.trim) return `${g.confidence}% · ${clock(g.trim[0])}⁠–⁠${clock(g.trim[1])} из ${clock(g.trim[2])}`;
  const n = g.items.length;
  return `${g.confidence}% · ${n} ${plural(n, "копия", "копии", "копий")}`;
}

export const groupTitle = (g: DupeGroup) => (g.kind === "same" && isPhotoPath(g.items[0]?.path ?? "") ? "Та же фотография" : KIND[g.kind]);

function meta(i: GroupItem) {
  return [i.width && i.height ? `${i.width}×${i.height}` : null, i.durationMs ? clock(i.durationMs) : null, mb(i.size)].filter(Boolean).join(" · ");
}

const FILTERS: { value: Filter; label: string }[] = [
  { value: "exact", label: "Точные" },
  { value: "same", label: "Та же запись" },
  { value: "crop", label: "Кадрированные" },
  { value: "maybe", label: "Возможно" },
];

export function DupesScreen({
  groups,
  filter,
  onFilter,
  onKeep,
  onDismiss,
  onTrashExact,
  onCompare,
}: {
  groups: DupeGroup[];
  filter: Filter | null;
  onFilter: (f: Filter | null) => void;
  onKeep: (keep: GroupItem, drop: GroupItem[]) => void;
  onDismiss: (g: DupeGroup) => void;
  onTrashExact: (paths: string[]) => void;
  onCompare: (g: DupeGroup) => void;
}) {
  const shown = useMemo(() => groups.filter((g) => inFilter(g, filter)), [groups, filter]);
  const [focus, setFocus] = useState(0);
  const [picks, setPicks] = useState<Record<string, number>>({});
  const rows = useRef<(HTMLDivElement | null)[]>([]);
  const extra = extraExact(groups);
  const at = Math.min(focus, Math.max(shown.length - 1, 0));
  const keyOf = (g: DupeGroup) => g.pairs.map((p) => p.join("|")).join(";");
  const picked = (g: DupeGroup) => Math.min(picks[keyOf(g)] ?? 0, g.items.length - 1);

  useEffect(() => {
    rows.current[at]?.scrollIntoView({ block: "nearest" });
  }, [at]);

  const keep = (g: DupeGroup) => {
    const i = picked(g);
    onKeep(g.items[i], g.items.filter((_, j) => j !== i));
  };

  const keys = useRef<(e: KeyboardEvent) => void>(() => {});
  keys.current = (e) => {
    if (e.altKey || e.metaKey) return;
    if (e.ctrlKey) {
      if (e.code === "KeyD" && extra.length) {
        e.preventDefault();
        onTrashExact(extra);
      }
      return;
    }
    const g = shown[at];
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setFocus(Math.max(0, Math.min(shown.length - 1, at + (e.key === "ArrowDown" ? 1 : -1))));
    } else if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && g) {
      e.preventDefault();
      const next = Math.max(0, Math.min(g.items.length - 1, picked(g) + (e.key === "ArrowRight" ? 1 : -1)));
      setPicks((p) => ({ ...p, [keyOf(g)]: next }));
    } else if (e.key === "Enter" && g) {
      e.preventDefault();
      keep(g);
    } else if (e.code === "KeyN" && g) {
      e.preventDefault();
      onDismiss(g);
    } else if (e.code === "KeyC" && g) {
      e.preventDefault();
      onCompare(g);
    }
  };
  useEffect(() => {
    const listen = (e: KeyboardEvent) => keys.current(e);
    window.addEventListener("keydown", listen);
    return () => window.removeEventListener("keydown", listen);
  }, []);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-[18px]">
      <div className="flex flex-wrap items-center gap-2.5">
        {FILTERS.map((f) => {
          const n = groups.filter((g) => inFilter(g, f.value)).length;
          const on = filter === f.value;
          return (
            <button
              key={f.value}
              type="button"
              aria-pressed={on}
              onClick={() => onFilter(on ? null : f.value)}
              className={`h-9 rounded-full px-3.5 text-[13px] whitespace-nowrap transition-colors duration-200 ease-trail ${
                on ? "bg-fg font-bold text-ink" : f.value === "maybe" ? "font-medium text-dim hover:text-fg" : "bg-raised font-medium text-fg hover:bg-strong"
              }`}
            >
              {f.label} {n}
            </button>
          );
        })}
        <div className="flex-1" />
        <Button hotkey="Ctrl D" disabled={!extra.length} onClick={() => onTrashExact(extra)}>
          Убрать все точные копии · {extra.length} {plural(extra.length, "файл", "файла", "файлов")}
        </Button>
      </div>

      {shown.length === 0 ? (
        <div className="flex min-h-0 flex-1 items-center text-[15px] text-dim">
          {groups.length ? "В этом фильтре групп нет — выбери другой." : "Дублей не нашлось."}
        </div>
      ) : (
        <div className="-m-1.5 flex min-h-0 flex-1 flex-col gap-2.5 overflow-y-auto p-1.5 [--scroll-inset:20px]">
          {shown.map((g, gi) => {
            const pick = picked(g);
            return (
              <div
                key={keyOf(g)}
                ref={(el) => {
                  rows.current[gi] = el;
                }}
                onClick={() => setFocus(gi)}
                className={`flex shrink-0 items-center gap-[18px] rounded-[20px] bg-raised px-4 py-3.5 ${
                  gi === at ? "outline-[1.5px] outline-offset-[3px] outline-fg outline-dashed" : ""
                }`}
              >
                <div className="flex w-[200px] shrink-0 flex-col gap-1">
                  <div className="text-xs font-medium text-dim">{note(g)}</div>
                  <div className="text-lg font-bold">{groupTitle(g)}</div>
                </div>
                <div className="flex min-w-0 flex-1 flex-wrap gap-3">
                  {g.items.map((it, ii) => (
                    <button
                      key={it.path}
                      type="button"
                      aria-pressed={ii === pick}
                      onClick={() => {
                        setFocus(gi);
                        setPicks((p) => ({ ...p, [keyOf(g)]: ii }));
                      }}
                      className={`flex w-[260px] min-w-0 items-center gap-2.5 text-left transition-opacity duration-200 ${ii === pick ? "" : "opacity-60"}`}
                    >
                      <span
                        className={`relative h-[66px] w-11 shrink-0 overflow-hidden rounded-[10px] bg-[#1d1d21] ${
                          ii === pick ? "outline-[1.5px] outline-offset-2 outline-fg outline-solid" : ""
                        }`}
                      >
                        {isPhotoPath(it.path) ? (
                          <img src={media(it.path)} alt="" draggable={false} className="h-full w-full object-cover" />
                        ) : (
                          <video src={`${media(it.path)}#t=0.5`} muted preload="metadata" className="h-full w-full object-cover" />
                        )}
                      </span>
                      <span className="flex min-w-0 flex-col gap-[3px]">
                        <span className="truncate text-[13px] font-bold" title={it.path}>
                          {name(it.path)}
                        </span>
                        <span className="text-xs text-dim">{meta(it)}</span>
                        <span className={`truncate text-xs ${it.where === "folder" ? "text-fg" : "text-dim"}`} title={placeLabel(it)}>
                          {placeLabel(it)}
                        </span>
                      </span>
                    </button>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      )}
      <div className="text-[13px] text-dim">
        ↑ ↓ — группа · ← → — какую оставить · Enter — остальные в Корзину · C — сравнить крупно · N — это разные файлы · Ctrl Z — забрать
      </div>
    </div>
  );
}
