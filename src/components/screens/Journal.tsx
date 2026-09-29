import { ArrowRight, ChevronLeft, Trash2, Undo2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { errorText, ipc, media, type Move, type Pile } from "../../lib/ipc";
import { cardsWord, plural } from "../../lib/plural";
import { Button } from "../ui/Button";
import { Heading } from "../ui/PageHeader";

const PAGE = 60;

function dayLabel(at: number) {
  const d = new Date(at);
  const today = new Date();
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  if (at >= start) return "Сегодня";
  if (at >= start - 86_400_000) return "Вчера";
  return d.toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: d.getFullYear() === today.getFullYear() ? undefined : "numeric" });
}

function how(move: Move, piles: Pile[]) {
  switch (move.method) {
    case "key": {
      const key = piles.find((p) => p.id === move.pileId)?.key;
      return key ? `клавиша ${key === "Delete" ? "Del" : key}` : "клавиша";
    }
    case "hint":
      return "подсказка · Enter";
    case "search":
      return "поиск";
    case "table":
      return `стол · ${cardsWord(move.items.length)}`;
    case "drag":
      return "перетаскивание";
    case "new_pile":
      return "новая стопка";
  }
}

function Thumb({ cacheDir, cardId }: { cacheDir: string; cardId: number | null }) {
  const [broken, setBroken] = useState(false);
  return (
    <span className="h-10 w-[26px] shrink-0 overflow-hidden rounded-md bg-film">
      {cardId !== null && !broken && (
        <img src={media([cacheDir, cardId, "3.jpg"].join("\\"))} alt="" onError={() => setBroken(true)} className="h-full w-full object-cover" />
      )}
    </span>
  );
}

export function JournalScreen({ cacheDir, deckPath, onBack }: { cacheDir: string; deckPath: string; onBack: () => void }) {
  const [moves, setMoves] = useState<Move[]>([]);
  const [piles, setPiles] = useState<Pile[]>([]);
  const [total, setTotal] = useState(0);
  const [done, setDone] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const loading = useRef(false);

  const refresh = useCallback(async (count: number) => {
    const [list, stats, state] = await Promise.all([ipc.journal(null, Math.max(count, PAGE)), ipc.journalStats(), ipc.getState()]);
    setMoves(list);
    setDone(list.length < Math.max(count, PAGE));
    setTotal(stats.moves);
    setPiles(state.piles);
  }, []);

  useEffect(() => {
    void refresh(PAGE);
  }, [refresh]);

  const more = async () => {
    if (loading.current || done || !moves.length) return;
    loading.current = true;
    const next = await ipc.journal(moves[moves.length - 1].id, PAGE);
    setMoves((m) => [...m, ...next]);
    setDone(next.length < PAGE);
    loading.current = false;
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onBack();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack]);

  const undo = async (move: Move) => {
    try {
      await ipc.undoMove(move.id);
      setNotice(null);
    } catch (e) {
      setNotice(errorText(e));
    }
    await refresh(moves.length);
  };

  const undoToday = async () => {
    const now = new Date();
    const since = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    try {
      const r = await ipc.undoSince(since);
      setNotice(
        r.failed.length
          ? `Забрано ${r.undone} ${plural(r.undone, "ход", "хода", "ходов")}, не вышло ${r.failed.length}: ${r.failed[0]}`
          : `Забрано ${r.undone} ${plural(r.undone, "ход", "хода", "ходов")} — карты вернулись в колоду.`,
      );
    } catch (e) {
      setNotice(errorText(e));
    }
    await refresh(moves.length);
  };

  const todayStart = new Date(new Date().setHours(0, 0, 0, 0)).getTime();
  const hasToday = moves.some((m) => m.state === "done" && m.at >= todayStart);
  const rows = moves.flatMap((m) => m.items.map((item, i) => ({ move: m, item, first: i === 0 })));
  let lastDay = "";
  let stripe = 0;

  return (
    <main className="mx-2 mb-2 flex min-h-0 flex-1 flex-col gap-[22px] rounded-[28px] bg-cosmic px-10 py-7">
      <div className="flex items-end gap-3">
        <div className="flex flex-1 flex-col gap-3.5">
          <button
            type="button"
            onClick={onBack}
            title="К колоде — Esc"
            className="flex h-7 items-center gap-2 self-start rounded-full bg-raised px-3 text-xs font-medium text-dim transition-colors duration-200 ease-trail hover:bg-strong [&_svg]:h-3.5 [&_svg]:w-3.5 [&_svg]:text-fg"
          >
            <ChevronLeft strokeWidth={1.5} />К колоде
          </button>
          <Heading light="Журнал," bold={`${total} ${plural(total, "ход", "хода", "ходов")}`} />
          <p className="m-0 max-w-[80ch] text-[15px] leading-[1.55] text-dim">
            {notice ?? `Любой ход можно забрать: карта вернётся в колоду, файл — в ${deckPath}.`}
          </p>
        </div>
        <Button icon={<Undo2 size={16} strokeWidth={1.5} />} onClick={() => void undoToday()} disabled={!hasToday}>
          Забрать всё за сегодня
        </Button>
      </div>

      <div
        className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto pr-2"
        onScroll={(e) => {
          const el = e.currentTarget;
          if (el.scrollHeight - el.scrollTop - el.clientHeight < 400) void more();
        }}
      >
        {!rows.length && <p className="m-0 pl-4 text-[15px] text-dim">Ходов пока нет — разложи первую карту, и она появится здесь.</p>}
        {rows.map(({ move, item, first }, idx) => {
          const day = dayLabel(move.at);
          const header = day !== lastDay;
          lastDay = day;
          if (header) stripe = 0;
          const bg = stripe++ % 2 ? "" : "bg-raised";
          const off = move.state === "undone" || move.state === "failed" || move.state === "undoing";
          const status = move.state === "undone" ? "забрано" : move.state === "failed" ? `не вышло${move.error ? ` — ${move.error}` : ""}` : null;
          return (
            <div key={`${move.id}-${item.fromPath}`} className="contents">
              {header && <div className={`pb-2 pl-4 text-[13px] font-medium text-dim ${idx ? "pt-4" : ""}`}>{day}</div>}
              <div className={`flex h-14 shrink-0 items-center gap-4 rounded-full pr-2 pl-4 ${bg} ${off ? "opacity-50" : ""}`}>
                <span className="w-12 text-[13px] font-medium text-dim">
                  {new Date(move.at).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" })}
                </span>
                <Thumb cacheDir={cacheDir} cardId={item.cardId} />
                <span className="w-[320px] truncate text-[15px] font-bold" title={item.fromPath}>
                  {item.finalName ?? item.fileName}
                </span>
                <ArrowRight size={16} strokeWidth={1.5} className="shrink-0 text-dim" />
                <span className="flex min-w-0 flex-1 items-center gap-2 truncate text-[15px]">
                  {move.isTrash && <Trash2 size={15} strokeWidth={1.5} className="shrink-0" />}
                  {move.pileName}
                </span>
                <span className="shrink-0 text-xs font-medium text-dim">{status ?? how(move, piles)}</span>
                {move.state === "done" && first ? (
                  <Button size={40} className="bg-strong! hover:bg-line!" onClick={() => void undo(move)}>
                    Забрать
                  </Button>
                ) : (
                  <span className="w-[88px] shrink-0" />
                )}
              </div>
            </div>
          );
        })}
      </div>
    </main>
  );
}
