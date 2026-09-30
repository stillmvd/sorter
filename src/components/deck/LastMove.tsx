import { Trash2, Undo2 } from "lucide-react";
import { media, type Move } from "../../lib/ipc";
import { cardsWord } from "../../lib/plural";
import { Button } from "../ui/Button";

function Thumb({ move }: { move: Move }) {
  const item = move.items[0];
  const photo = /\.(jpe?g|png|webp|gif)$/i.test(item.fileName);
  const src = item.toPath ? (photo ? media(item.toPath) : `${media(item.toPath)}#t=0.5`) : null;
  return (
    <div className="relative h-[84px] w-14 shrink-0">
      {move.items.length > 1 && <div className="absolute inset-0 translate-x-1.5 rotate-3 rounded-[10px] bg-strong" />}
      <div className="absolute inset-0 -rotate-6 overflow-hidden rounded-[10px] border border-line bg-film">
        {src && photo ? (
          <img key={src} src={src} alt="" className="h-full w-full object-cover" />
        ) : src ? (
          <video key={src} src={src} muted preload="metadata" className="h-full w-full object-cover" />
        ) : (
          <div className="grid h-full place-items-center text-[#a2a2a9]">
            <Trash2 className="h-5 w-5" strokeWidth={1.5} />
          </div>
        )}
      </div>
    </div>
  );
}

export function LastMoveLine({ move, onUndo }: { move: Move | null; onUndo: () => void }) {
  if (!move) return null;
  const first = move.items[0];
  return (
    <div className="flex min-w-0 items-center gap-3">
      <span className="truncate text-[13px] text-dim">
        Последний ход: <span className="font-bold text-fg">{move.items.length > 1 ? cardsWord(move.items.length) : first?.fileName}</span> →{" "}
        {move.pileName}
      </span>
      <Button size={36} onClick={onUndo} hotkey="Ctrl Z" icon={<Undo2 className="h-3.5 w-3.5" strokeWidth={1.5} />}>
        Забрать
      </Button>
    </div>
  );
}

export function LastMove({ move, onUndo }: { move: Move | null; onUndo: () => void }) {
  const first = move?.items[0];
  const renamed = first?.finalName && first.finalName !== first.fileName;
  return (
    <div className="flex w-[210px] shrink-0 flex-col justify-center gap-3">
      <div className="text-[13px] font-medium text-dim">Последний ход</div>
      {move && first ? (
        <div className="flex items-center gap-3">
          <Thumb move={move} />
          <div className="flex min-w-0 flex-col gap-1">
            <div className="truncate text-[13px] font-bold">
              {move.items.length > 1 ? cardsWord(move.items.length) : first.fileName}
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
