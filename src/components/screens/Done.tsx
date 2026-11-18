import { openPath } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useState } from "react";
import { errorText, ipc, type AppError, type Pile } from "../../lib/ipc";
import { filesWord, plural } from "../../lib/plural";
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
  onHome,
  rest,
}: {
  rest?: { gone: string; left: string; show: string; onShow: () => void };
  placed: number;
  piles: Pile[];
  deckPath: string;
  tablePath: string;
  hintsOn: boolean;
  onJournal: () => void;
  onHome: () => void;
}) {
  const [stats, setStats] = useState<{ cards: number; hinted: number } | null>(null);

  const [dirs, setDirs] = useState<string[]>([]);
  const [tidied, setTidied] = useState<{ id: number; n: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState("");

  useEffect(() => {
    void ipc.journalStats().then(setStats);
  }, []);

  useEffect(() => {
    if (rest) return;
    void ipc.emptyDirs().then(setDirs).catch(() => setDirs([]));
  }, [rest]);

  const tidy = useCallback(async () => {
    if (busy || !dirs.length) return;
    setBusy(true);
    setProblem("");
    try {
      const r = await ipc.trashDirs(dirs);
      setTidied({ id: r.move.id, n: r.move.items.length });
      setDirs([]);
    } catch (e) {
      setProblem(errorText(e));
      setDirs(await ipc.emptyDirs().catch(() => []));
    } finally {
      setBusy(false);
    }
  }, [busy, dirs]);

  const bring = useCallback(async () => {
    if (busy || !tidied) return;
    setBusy(true);
    setProblem("");
    try {
      await ipc.undoMove(tidied.id).catch((e: unknown) => {
        if ((e as AppError | null)?.code !== "NOT_UNDOABLE") throw e;
      });
      setTidied(null);
      setDirs(await ipc.emptyDirs());
    } catch (e) {
      setProblem(errorText(e));
    } finally {
      setBusy(false);
    }
  }, [busy, tidied]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Delete" && dirs.length && !tidied) {
        e.preventDefault();
        e.stopImmediatePropagation();
        void tidy();
      } else if (e.ctrlKey && e.code === "KeyZ" && tidied) {
        e.preventDefault();
        e.stopImmediatePropagation();
        void bring();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [dirs.length, tidied, tidy, bring]);

  const list = piles.filter((p) => !p.isTrash).sort((a, b) => b.count - a.count);
  const top = Math.max(1, ...list.map((p) => p.count));
  const guessed = hintsOn && stats?.cards ? `${Math.round((stats.hinted / stats.cards) * 100)}%` : "—";
  const tiles = [
    { k: "Разложено", v: String(placed) },
    { k: "Стопок", v: String(list.length) },
    { k: "Угадано подсказкой", v: guessed },
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-8 px-6 pt-7 pb-2">
      <div className="flex items-end gap-12">
        <div className="flex flex-1 flex-col gap-3.5">
          <Mark size={56} />
          {rest ? (
            <>
              <Heading light={rest.gone} bold="кончились" size={56} />
              <p className="m-0 max-w-[560px] text-[15px] leading-[1.55] text-dim">В колоде осталось {rest.left} — переключись, чтобы разложить их.</p>
            </>
          ) : (
            <>
              <Heading light="Колода" bold="пуста" size={56} />
              <p className="m-0 max-w-[560px] text-[15px] leading-[1.55] text-dim">
                Все {filesWord(placed)} лежат по стопкам. Новые файлы в «{deckPath}» появятся в колоде сами.
              </p>
              {(dirs.length > 0 || tidied) && (
                <div className="flex items-center gap-3 pt-1.5">
                  <span className="text-[15px] font-medium">
                    {tidied
                      ? `${tidied.n} ${plural(tidied.n, "пустая папка", "пустые папки", "пустых папок")} в корзине Windows`
                      : `${plural(dirs.length, "Осталась", "Остались", "Остались")} ${dirs.length} ${plural(dirs.length, "пустая папка", "пустые папки", "пустых папок")}`}
                  </span>
                  {tidied ? (
                    <Button size={36} hotkey="Ctrl Z" disabled={busy} onClick={() => void bring()}>
                      Вернуть
                    </Button>
                  ) : (
                    <Button size={36} hotkey="Del" disabled={busy} onClick={() => void tidy()} title={dirs.join("\n")}>
                      Убрать в корзину
                    </Button>
                  )}
                </div>
              )}
              {problem && <p className="m-0 text-[13px] text-danger">{problem}</p>}
            </>
          )}
        </div>
        <div className="flex gap-2">
          <Button size={48} variant="ghost" onClick={onHome} hotkey="Esc">
            На главный
          </Button>
          <Button size={48} onClick={onJournal} hotkey="Ctrl J">
            Открыть журнал
          </Button>
          {rest ? (
            <Button size={48} variant="primary" onClick={rest.onShow}>
              {rest.show}
            </Button>
          ) : (
            <Button size={48} variant="primary" onClick={() => void openPath(tablePath)}>
              Открыть стол в проводнике
            </Button>
          )}
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
