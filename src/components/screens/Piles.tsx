import { openPath } from "@tauri-apps/plugin-opener";
import { FolderOpen, Pencil, Plus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { errorText, ipc, onPilesChanged, type Pile } from "../../lib/ipc";
import { keyOf, PILE_KEYS } from "../../lib/keys";
import { Button } from "../ui/Button";
import { BackButton, Heading } from "../ui/PageHeader";

const MIN_EXAMPLES = 3;

function hintState(p: Pile) {
  if (p.count === 0) return "пусто — подсказок пока нет";
  if (p.examples < MIN_EXAMPLES) return "мало примеров для подсказок";
  return "подсказки учатся на этих картах";
}

function stack(count: number) {
  return (
    Array.from(
      { length: Math.min(count, 4) },
      (_, k) => `0 ${-(k + 1) * 4}px 0 ${-(k + 1)}px var(--${k % 2 ? "line" : "strong"})`,
    ).join(", ") || "none"
  );
}

function NameField({ initial, onSave, onCancel }: { initial: string; onSave: (name: string) => void; onCancel: () => void }) {
  const [value, setValue] = useState(initial);
  return (
    <input
      autoFocus
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onFocus={(e) => e.target.select()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter" && value.trim()) onSave(value.trim());
        if (e.key === "Escape") onCancel();
      }}
      onBlur={onCancel}
      aria-label="Имя стопки"
      className="h-9 min-w-0 flex-1 rounded-full bg-cosmic px-3.5 text-[16px] font-bold text-fg outline-none ring-[1.5px] ring-fg"
    />
  );
}

export function PilesScreen({ table, onBack }: { table: string; onBack: () => void }) {
  const [piles, setPiles] = useState<Pile[]>([]);
  const [assigning, setAssigning] = useState<number | null>(null);
  const [editing, setEditing] = useState<number | "new" | null>(null);
  const [notice, setNotice] = useState<{ pileId: number | "new" | null; text: string } | null>(null);
  const noticeTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    void ipc.getState().then((s) => setPiles(s.piles));
    const off = onPilesChanged(setPiles);
    return () => void off.then((f) => f());
  }, []);

  const say = (pileId: number | "new" | null, text: string) => {
    setNotice({ pileId, text });
    window.clearTimeout(noticeTimer.current);
    noticeTimer.current = window.setTimeout(() => setNotice(null), 6000);
  };

  const run = async (pileId: number | "new", action: () => Promise<Pile[]>) => {
    try {
      setPiles(await action());
      return true;
    } catch (e) {
      say(pileId, errorText(e));
      return false;
    }
  };

  const assign = async (pile: Pile, key: string | null) => {
    const holder = key ? piles.find((p) => p.key === key && p.id !== pile.id) : undefined;
    setAssigning(null);
    if (await run(pile.id, () => ipc.setPileKey(pile.id, key))) {
      if (holder) say(holder.id, `Клавиша ${key} теперь у «${pile.name}», у «${holder.name}» клавиши нет — назначь другую.`);
      else setNotice(null);
    }
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (editing !== null) return;
      const pile = piles.find((p) => p.id === assigning);
      if (pile) {
        e.preventDefault();
        if (e.key === "Escape") return setAssigning(null);
        if (e.key === "Backspace" || e.key === "Delete") return void assign(pile, null);
        const key = keyOf(e);
        if (key && PILE_KEYS.includes(key)) void assign(pile, key);
        else say(pile.id, "Эту клавишу нельзя назначить — подойдут 1–9 и Q–P.");
        return;
      }
      if (e.key === "Escape") onBack();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const list = piles.filter((p) => !p.isTrash);

  return (
    <main className="mx-2 mb-2 flex min-h-0 flex-1 flex-col gap-[22px] rounded-[28px] bg-cosmic px-10 py-7">
      <div className="flex items-end gap-3">
        <div className="flex flex-1 flex-col gap-3.5">
          <BackButton onClick={onBack} />
          <Heading light="Стопки" bold="и клавиши" />
          <p className="m-0 max-w-[80ch] text-[15px] leading-[1.55] text-dim">
            Стол: {table} · каждая стопка — папка внутри. Клавиша кладёт карту сразу, без поиска.
          </p>
        </div>
        <Button icon={<FolderOpen size={16} strokeWidth={1.5} />} onClick={() => void openPath(table)}>
          Открыть стол в проводнике
        </Button>
      </div>

      <div className="grid min-h-0 flex-1 auto-rows-[132px] grid-cols-4 content-start gap-x-3 gap-y-3.5 overflow-y-auto px-1 pt-5 pb-2">
        {list.map((p) => {
          const listening = assigning === p.id;
          const note = notice?.pileId === p.id ? notice.text : null;
          return (
            <div
              key={p.id}
              style={{ boxShadow: stack(p.count) }}
              className={`flex flex-col justify-between rounded-[20px] bg-raised px-[18px] py-4 ${listening || editing === p.id ? "outline-[1.5px] outline-offset-[3px] outline-fg outline-solid" : ""}`}
            >
              <div className="flex items-center gap-3">
                <button
                  type="button"
                  onClick={() => setAssigning(listening ? null : p.id)}
                  title={listening ? "Нажми 1–9 или Q–P · Backspace — снять · Esc — отмена" : "Назначить клавишу"}
                  className={`grid h-9 min-w-9 shrink-0 place-items-center rounded-full px-2.5 font-bold whitespace-nowrap transition-colors duration-200 ease-trail ${
                    listening ? "bg-fg text-xs text-ink" : p.key ? "bg-strong text-sm text-fg" : "border-[1.5px] border-dashed border-strong text-xs text-dim"
                  }`}
                >
                  {listening ? "Нажми клавишу…" : (p.key ?? "Клавиша")}
                </button>
                {editing === p.id ? (
                  <NameField
                    initial={p.name}
                    onCancel={() => setEditing(null)}
                    onSave={async (name) => {
                      if (await run(p.id, () => ipc.renamePile(p.id, name))) setEditing(null);
                    }}
                  />
                ) : (
                  <>
                    <span className="min-w-0 flex-1 truncate text-lg font-bold">{p.name}</span>
                    <button
                      type="button"
                      aria-label="Переименовать"
                      title="Переименовать папку"
                      onClick={() => setEditing(p.id)}
                      className="grid h-8 w-8 shrink-0 place-items-center rounded-full text-dim transition-colors duration-200 ease-trail hover:bg-strong hover:text-fg"
                    >
                      <Pencil size={16} strokeWidth={1.5} />
                    </button>
                  </>
                )}
              </div>
              {note ? (
                <p role="alert" className="m-0 line-clamp-2 text-[13px] leading-[1.4] font-medium text-fg">
                  {note}
                </p>
              ) : (
                <div className="flex items-baseline gap-2.5 text-[13px] text-dim">
                  <span className="text-[22px] font-bold text-fg">{p.count}</span>
                  <span className="truncate">{hintState(p)}</span>
                </div>
              )}
            </div>
          );
        })}
        {editing === "new" ? (
          <div className="flex flex-col justify-center gap-2.5 rounded-[20px] border-[1.5px] border-dashed border-line px-[18px]">
            <NameField
              initial=""
              onCancel={() => setEditing(null)}
              onSave={async (name) => {
                if (await run("new", () => ipc.createPile(name))) setEditing(null);
              }}
            />
            <span className="text-[13px] text-dim">
              {notice?.pileId === "new" ? notice.text : "Enter — создать папку на столе · Esc — отмена"}
            </span>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setEditing("new")}
            className="flex items-center justify-center gap-2 rounded-[20px] border-[1.5px] border-dashed border-line text-[15px] font-medium text-dim transition-colors duration-200 ease-trail hover:border-strong hover:text-fg"
          >
            <Plus size={18} strokeWidth={1.5} />
            Новая стопка
          </button>
        )}
      </div>
    </main>
  );
}
