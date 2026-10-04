import { open } from "@tauri-apps/plugin-dialog";
import { Check, CircleAlert, Folder, Minus, Plus, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { errorText, ipc, type Pile, type SearchPreview } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { filesWord } from "../screens/DupesStep";
import { Button } from "../ui/Button";
import { Kbd } from "../ui/Kbd";
import { useModalKeys } from "./EnableDupes";

export type SearchSet = { piles: string[]; deck: boolean; folders: string[] };

const nf = (n: number) => String(n).replace(/\B(?=(\d{3})+(?!\d))/g, " ");

function Box({ on, mixed = false }: { on: boolean; mixed?: boolean }) {
  return (
    <span
      className={`grid h-5 w-5 shrink-0 place-items-center rounded-md text-ink ${on || mixed ? "bg-fg" : "shadow-[inset_0_0_0_1.5px_var(--dim)]"}`}
    >
      {mixed ? <Minus size={14} strokeWidth={2.5} /> : on && <Check size={14} strokeWidth={2.5} />}
    </span>
  );
}

const row = "flex min-h-10 items-center gap-[11px] rounded-xl px-2.5 py-1.5 text-left text-[13.5px] transition-colors duration-200 ease-trail hover:bg-strong";

export function FindDupes({
  piles,
  table,
  deckPath,
  initial,
  onStart,
  onClose,
}: {
  piles: Pile[];
  table: string;
  deckPath: string;
  initial: SearchSet | null;
  onStart: (s: SearchSet) => void;
  onClose: () => void;
}) {
  const names = useMemo(() => piles.filter((p) => !p.isTrash).map((p) => p.name), [piles]);
  const [chosen, setChosen] = useState<Set<string>>(() => new Set(initial?.piles ?? names));
  const [deck, setDeck] = useState(initial?.deck ?? false);
  const [folders, setFolders] = useState<string[]>(initial?.folders ?? []);
  const [preview, setPreview] = useState<SearchPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const picked = names.filter((n) => chosen.has(n));
  const set: SearchSet = { piles: picked, deck, folders };
  const key = JSON.stringify(set);
  const empty = !picked.length && !deck && !folders.length;

  useEffect(() => {
    let alive = true;
    const t = setTimeout(() => {
      void ipc.searchPreview(set.piles, set.deck, set.folders).then(
        (p) => alive && setPreview(p),
        (e) => alive && setError(errorText(e)),
      );
    }, 150);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [key]);

  const addFolder = async () => {
    const path = await open({ directory: true, title: "Где ещё искать дубли?" });
    if (typeof path !== "string") return;
    setFolders((f) => (f.some((x) => x.toLowerCase() === path.toLowerCase()) ? f : [...f, path]));
  };

  const start = () => {
    if (!empty && (preview?.total ?? 0) > 0) onStart(set);
  };

  useModalKeys((e) => {
    if (e.key === "Escape") onClose();
    else if (e.key === "Enter") start();
    else if (e.ctrlKey && e.code === "KeyO") void addFolder();
    else return;
    e.preventDefault();
  });

  const pileCount = (name: string) => piles.find((p) => p.name === name)?.count ?? 0;
  const root = (path: string) => preview?.roots.find((r) => r.path.toLowerCase() === path.toLowerCase());
  const all = picked.length === names.length;

  return (
    <>
      <div className="fixed inset-x-0 top-12 bottom-0 z-40 bg-[rgb(8_8_10/30%)]" onClick={onClose} />
      <aside
        role="dialog"
        aria-modal="true"
        aria-label="Найти дубли"
        className="fixed top-14 right-2 bottom-2 z-40 flex w-[430px] flex-col gap-4 overflow-y-auto rounded-[28px] bg-cosmic px-6 pt-6 pb-5 shadow-[-24px_0_60px_rgb(0_0_0/40%),0_0_0_1px_var(--line)] [--scroll-inset:20px]"
      >
        <h2 className="text-[28px] leading-[1.1] tracking-[-0.02em]">
          <span className="font-light">Найти</span> <span className="font-bold">дубли</span>
        </h2>
        <p className="-mt-1.5 m-0 text-[13px] leading-[1.45] text-dim">
          Сравню между собой всё, что отметишь. Пока ищу — раскладывать нельзя: пауза Space, отмена Esc.
        </p>

        <section className="flex flex-col gap-2">
          <div className="flex items-baseline gap-2 text-[12.5px] font-medium text-dim">
            <b className="text-sm text-fg">Стопки стола</b>
            <span className="min-w-0 truncate">{table}</span>
            <span className="ml-auto shrink-0">
              {picked.length} из {names.length}
            </span>
          </div>
          {names.length ? (
            <div className="flex flex-col gap-0.5 rounded-[18px] bg-raised p-1">
              <button
                type="button"
                className={`${row} rounded-b-sm border-b border-line`}
                onClick={() => setChosen(new Set(all ? [] : names))}
              >
                <Box on={all} mixed={!all && picked.length > 0} />
                <b className="truncate">Все стопки</b>
                <span className="ml-auto text-[12.5px] text-dim tabular-nums">
                  {filesWord(names.reduce((n, p) => n + pileCount(p), 0))}
                </span>
              </button>
              {names.map((n) => {
                const on = chosen.has(n);
                return (
                  <button
                    key={n}
                    type="button"
                    role="checkbox"
                    aria-checked={on}
                    className={row}
                    onClick={() =>
                      setChosen((s) => {
                        const next = new Set(s);
                        if (on) next.delete(n);
                        else next.add(n);
                        return next;
                      })
                    }
                  >
                    <Box on={on} />
                    <b className="truncate">{n}</b>
                    <span className="ml-auto text-[12.5px] text-dim tabular-nums">{filesWord(pileCount(n))}</span>
                  </button>
                );
              })}
            </div>
          ) : (
            <p className="m-0 text-[13px] text-dim">На столе пока нет стопок.</p>
          )}
        </section>

        <section className="flex flex-col gap-2">
          <div className="flex items-baseline gap-2 text-[12.5px] font-medium text-dim">
            <b className="text-sm text-fg">Колода</b>
            <span>по желанию</span>
          </div>
          <div className="rounded-[18px] bg-raised p-1">
            <button type="button" role="checkbox" aria-checked={deck} className={`${row} w-full`} onClick={() => setDeck((d) => !d)}>
              <Box on={deck} />
              <b className="min-w-0 truncate">{deckPath}</b>
            </button>
          </div>
        </section>

        <section className="flex flex-col gap-2">
          <div className="flex items-baseline gap-2 text-[12.5px] font-medium text-dim">
            <b className="text-sm text-fg">Другие папки</b>
            <span>с подпапками</span>
          </div>
          {folders.length > 0 && (
            <div className="flex flex-col gap-0.5 rounded-[18px] bg-raised p-1">
              {folders.map((f) => {
                const r = root(f);
                const bad = !!r?.error;
                return (
                  <div key={f} className="flex min-h-10 items-center gap-[11px] rounded-xl px-2.5 py-1.5 text-[13.5px]">
                    {bad ? <CircleAlert size={16} strokeWidth={1.5} className="shrink-0 text-dim" /> : <Folder size={16} strokeWidth={1.5} className="shrink-0 text-dim" />}
                    <b className={`min-w-0 truncate ${bad ? "text-dim line-through decoration-1" : ""}`} title={f}>
                      {f}
                    </b>
                    <span className={`ml-auto shrink-0 text-[12.5px] tabular-nums ${bad ? "text-fg" : "text-dim"}`}>
                      {bad ? "нет доступа — пропущу" : r ? filesWord(r.files) : "считаю…"}
                    </span>
                    <button
                      type="button"
                      aria-label={`Убрать ${f}`}
                      className="grid h-7 w-7 shrink-0 place-items-center rounded-full text-dim hover:bg-strong hover:text-fg"
                      onClick={() => setFolders((x) => x.filter((y) => y !== f))}
                    >
                      <X size={16} strokeWidth={1.5} />
                    </button>
                  </div>
                );
              })}
            </div>
          )}
          <button
            type="button"
            onClick={() => void addFolder()}
            className="flex min-h-10 items-center gap-2.5 rounded-full border-[1.5px] border-dashed border-line px-3.5 text-[13px] font-medium text-dim transition-colors duration-200 ease-trail hover:text-fg"
          >
            <Plus size={16} strokeWidth={1.5} />
            Добавить папку…
            <span className="ml-auto">
              <Kbd>Ctrl O</Kbd>
            </span>
          </button>
        </section>

        <div className="mt-auto flex flex-wrap items-center gap-2.5 pt-1">
          <span className="mr-auto text-[13px] text-dim tabular-nums">
            {error ? (
              error
            ) : empty ? (
              "Отметь стопку, колоду или папку"
            ) : preview ? (
              <>
                Сравню <b className="text-fg">{nf(preview.total)} {plural(preview.total, "файл", "файла", "файлов")}</b>
              </>
            ) : (
              "Считаю файлы…"
            )}
          </span>
          <Button variant="ghost" hotkey="Esc" onClick={onClose}>
            Отмена
          </Button>
          <Button variant="primary" hotkey="Enter" disabled={empty || !preview?.total} onClick={start}>
            Искать
          </Button>
        </div>
      </aside>
    </>
  );
}
