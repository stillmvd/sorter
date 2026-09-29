import { openPath } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { ipc, type Pile } from "../../lib/ipc";
import { Button } from "../ui/Button";
import { Mark } from "../ui/Mark";
import { Heading } from "../ui/PageHeader";

export function Done({
  placed,
  piles,
  deckPath,
  tablePath,
  hintsOn,
  onJournal,
}: {
  placed: number;
  piles: Pile[];
  deckPath: string;
  tablePath: string;
  hintsOn: boolean;
  onJournal: () => void;
}) {
  const [stats, setStats] = useState<{ cards: number; hinted: number } | null>(null);

  useEffect(() => {
    void ipc.journalStats().then(setStats);
  }, []);

  const list = piles.filter((p) => !p.isTrash).sort((a, b) => b.count - a.count);
  const top = Math.max(1, ...list.map((p) => p.count));
  const guessed = hintsOn && stats?.cards ? `${Math.round((stats.hinted / stats.cards) * 100)}%` : "—";
  const tiles = [
    { k: "Разложено видео", v: String(placed) },
    { k: "Стопок", v: String(list.length) },
    { k: "Угадано подсказкой", v: guessed },
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-8 px-6 pt-7 pb-2">
      <div className="flex items-end gap-12">
        <div className="flex flex-1 flex-col gap-3.5">
          <Mark size={56} />
          <Heading light="Колода" bold="пуста" size={56} />
          <p className="m-0 max-w-[560px] text-[15px] leading-[1.55] text-dim">
            Все {placed} видео лежат по стопкам. Новые видео в {deckPath} появятся в колоде при следующем запуске.
          </p>
        </div>
        <div className="flex gap-2">
          <Button size={48} onClick={onJournal} hotkey="Ctrl J">
            Открыть журнал
          </Button>
          <Button size={48} variant="primary" onClick={() => void openPath(tablePath)}>
            Открыть стол в проводнике
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-3">
        {tiles.map((t) => (
          <div key={t.k} className="flex flex-col gap-1.5 rounded-[20px] bg-raised px-[22px] py-5">
            <span className="text-[13px] font-medium text-dim">{t.k}</span>
            <span className="text-[42px] leading-none font-bold tracking-[-0.02em]">{t.v}</span>
          </div>
        ))}
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-3.5">
        <span className="text-[13px] font-medium text-dim">Стопки на столе</span>
        <div className="flex min-h-0 flex-1 items-end gap-3">
          {list.map((p, i) => (
            <div key={p.id} className="flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-2">
              <span className="text-[13px] font-bold">{p.count}</span>
              <div
                className={`w-full rounded-xl ${i === 0 ? "bg-fg" : "bg-strong"}`}
                style={{ height: Math.max(4, Math.round((p.count / top) * 260)) }}
              />
              <span className="max-w-full truncate text-xs font-medium text-dim">{p.name}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
