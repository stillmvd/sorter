import { Copy } from "lucide-react";
import type { DupeView, Pile } from "../../lib/ipc";
import { Button } from "../ui/Button";
import { Kbd } from "../ui/Kbd";

const clock = (ms: number) => {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

const span = (from: number, to: number) => `${clock(from)}\u2060–\u2060${clock(to)}`;

function kindText(dupe: DupeView, mine: number | null) {
  if (dupe.kind === "exact") return "точная копия";
  if (dupe.kind === "crop") return "кадрирована";
  if (dupe.kind === "same") return `та же запись, ${dupe.better ? "лучше" : "хуже"} качество`;
  const theirs = dupe.durationMs;
  const offset = dupe.offsetMs ?? 0;
  if (mine === null || theirs === null) return "обрезка";
  return theirs <= mine
    ? `обрезка: ${span(offset, offset + theirs)} из\u00a0${clock(mine)}`
    : `эта карта — обрезка: ${span(-offset, -offset + mine)} из\u00a0${clock(theirs)}`;
}

const mb = (b: number) => `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ`;
const name = (path: string) => path.split("\\").pop() ?? path;

export function DupeBadge({
  dupe,
  more,
  durationMs,
  replace,
  hint,
  canDefer,
  onTrash,
  onCompare,
  onDismiss,
  onDefer,
}: {
  dupe: DupeView;
  more: number;
  durationMs: number | null;
  replace: boolean;
  hint: { pile: Pile; score: number } | null;
  canDefer: boolean;
  onTrash: () => void;
  onCompare: () => void;
  onDismiss: () => void;
  onDefer: () => void;
}) {
  const where = dupe.where === "pile" ? `уже лежит в «${dupe.pileName}»` : "копия в колоде";
  const meta = [
    name(dupe.path),
    dupe.width && dupe.height ? `${dupe.width}×${dupe.height}` : null,
    dupe.bitrate ? `${(dupe.bitrate / 1e6).toFixed(1).replace(".", ",")} Мбит/с` : null,
    mb(dupe.size),
    more > 0 ? `и ещё ${more}` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex flex-wrap items-center gap-x-3.5 gap-y-3 rounded-[20px] bg-raised px-4 py-3.5 outline-[1.5px] outline-offset-[-1.5px] outline-fg outline-dashed">
        <div className="flex min-w-0 flex-[1_1_300px] items-center gap-3.5">
          <div className="grid h-11 w-11 shrink-0 place-items-center rounded-full bg-fg text-ink">
            <Copy className="h-[22px] w-[22px]" strokeWidth={1.5} />
          </div>
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <div className="text-xs font-medium text-dim">
              Дубль · {kindText(dupe, durationMs)} · {dupe.confidence}%
            </div>
            <div className="flex min-w-0 items-baseline gap-2.5">
              <span className="min-w-0 truncate text-[22px] font-bold tracking-[-0.02em]">{where}</span>
              <span className="min-w-0 flex-1 truncate text-[13px] text-dim" title={dupe.path}>
                {meta}
              </span>
            </div>
          </div>
        </div>
        <div className="ml-auto flex gap-2">
          <Button size={44} hotkey="C" onClick={onCompare} className="bg-strong! hover:bg-line!">
            Сравнить
          </Button>
          <Button variant="primary" size={44} hotkey="D" onClick={onTrash}>
            {dupe.better ? "Убрать в Корзину" : replace ? "Заменить" : "Убрать копию"}
          </Button>
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <div className="flex min-w-0 flex-[1_1_260px] items-center gap-2.5 rounded-[20px] bg-raised px-4 py-2.5 opacity-55">
          <span className="h-1.5 w-1.5 shrink-0 rounded-full border-[1.5px] border-fg" />
          {hint ? (
            <>
              <span className="shrink-0 text-xs font-medium text-dim">Просится в стопку</span>
              <span className="truncate text-[15px] font-bold">{hint.pile.name}</span>
              <span className="text-xs font-bold">{Math.round(hint.score * 100)}%</span>
              <span className="flex-1" />
              <Kbd>Enter</Kbd>
            </>
          ) : (
            <span className="truncate text-xs font-medium text-dim">Клавиша стопки — положить сюда, а не в Корзину</span>
          )}
        </div>
        <Button onClick={onDefer} hotkey="Tab" disabled={!canDefer}>
          В конец
        </Button>
        <Button variant="ghost" onClick={onDismiss} hotkey="N" className="text-dim">
          Это разные видео
        </Button>
      </div>
    </div>
  );
}
