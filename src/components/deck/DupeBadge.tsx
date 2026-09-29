import { Copy } from "lucide-react";
import type { DupeKind, DupeView, Pile } from "../../lib/ipc";
import { Button } from "../ui/Button";
import { Kbd } from "../ui/Kbd";

const KINDS: Record<DupeKind, string> = {
  exact: "точная копия",
  same: "та же запись, другое качество",
  trim: "обрезка",
  crop: "другое кадрирование",
};

const mb = (b: number) => `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ`;
const name = (path: string) => path.split("\\").pop() ?? path;

export function DupeBadge({
  dupe,
  more,
  hint,
  canDefer,
  onTrash,
  onDismiss,
  onDefer,
}: {
  dupe: DupeView;
  more: number;
  hint: { pile: Pile; score: number } | null;
  canDefer: boolean;
  onTrash: () => void;
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
      <div className="flex items-center gap-3.5 rounded-[20px] bg-raised px-4 py-3.5 outline-[1.5px] outline-offset-[-1.5px] outline-fg outline-dashed">
        <div className="grid h-11 w-11 shrink-0 place-items-center rounded-full bg-fg text-ink">
          <Copy className="h-[22px] w-[22px]" strokeWidth={1.5} />
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="text-xs font-medium text-dim">
            Дубль · {KINDS[dupe.kind]} · {dupe.confidence}%
          </div>
          <div className="truncate text-[22px] font-bold tracking-[-0.02em]">{where}</div>
          <div className="truncate text-[13px] text-dim" title={dupe.path}>
            {meta}
          </div>
        </div>
        <Button variant="primary" size={44} hotkey="D" onClick={onTrash}>
          {dupe.better ? "Убрать в Корзину" : "Убрать копию"}
        </Button>
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
