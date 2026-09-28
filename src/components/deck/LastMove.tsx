import { Undo2 } from "lucide-react";
import type { Move } from "../../lib/ipc";
import { Button } from "../ui/Button";

export function LastMove({ move, onUndo }: { move: Move | null; onUndo: () => void }) {
  const first = move?.items[0];
  const renamed = first?.finalName && first.finalName !== first.fileName;
  return (
    <div className="flex w-[210px] shrink-0 flex-col justify-center gap-3">
      <div className="text-[13px] font-medium text-dim">Последний ход</div>
      {move && first ? (
        <div className="flex items-center gap-3">
          <div className="h-[72px] w-12 shrink-0 -rotate-6 rounded-[10px] bg-raised" />
          <div className="flex min-w-0 flex-col gap-1">
            <div className="truncate text-[13px] font-bold">
              {move.items.length > 1 ? `${move.items.length} карт` : first.fileName}
            </div>
            <div className="truncate text-[13px] text-dim">→ {move.pileName}</div>
            {renamed && <div className="truncate text-xs text-dim">как «{first.finalName}»</div>}
          </div>
        </div>
      ) : (
        <div className="text-[13px] leading-normal text-dim">Здесь появится карта, которую ты положил последней.</div>
      )}
      <Button
        className="self-start"
        disabled={!move}
        onClick={onUndo}
        hotkey="Ctrl Z"
        icon={<Undo2 className="h-4 w-4" strokeWidth={1.5} />}
      >
        Забрать
      </Button>
    </div>
  );
}
