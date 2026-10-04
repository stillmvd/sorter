import { useEffect, useRef, useState } from "react";
import { plural } from "../../lib/plural";
import { filesWord, useEstimate } from "../screens/DupesStep";
import { Button } from "../ui/Button";

type Scope = "deck" | "all";

export function useModalKeys(fn: (e: KeyboardEvent) => void) {
  const ref = useRef(fn);
  ref.current = fn;
  useEffect(() => {
    const listen = (e: KeyboardEvent) => {
      e.stopPropagation();
      ref.current(e);
    };
    window.addEventListener("keydown", listen, true);
    return () => window.removeEventListener("keydown", listen, true);
  }, []);
}

export function EnableDupes({
  scope: initial,
  piles,
  onStart,
  onClose,
}: {
  scope: Scope;
  piles: number;
  onStart: (scope: Scope) => void;
  onClose: () => void;
}) {
  const [scope, setScope] = useState<Scope>(initial);
  const deck = useEstimate("deck", true, "");
  const all = useEstimate("all", true, "");
  const minutes = (scope === "deck" ? deck : all)?.minutes ?? 0;

  useModalKeys((e) => {
    if (e.key === "Escape") onClose();
    else if (e.key === "Enter") onStart(scope);
    else if (e.key === "ArrowLeft" || e.code === "Digit1") setScope("deck");
    else if (e.key === "ArrowRight" || e.code === "Digit2") setScope("all");
    else return;
    e.preventDefault();
  });

  const areas: { value: Scope; name: string; hint: string; files: number | null }[] = [
    { value: "deck", name: "Внутри колоды", hint: "Копии среди файлов этой папки", files: deck ? deck.deck : null },
    {
      value: "all",
      name: "Колода и стол",
      hint: `Ещё и с тем, что разложено: ${piles} ${plural(piles, "стопка", "стопки", "стопок")}`,
      files: all ? all.deck + all.table : null,
    },
  ];

  return (
    <div className="fixed inset-x-0 top-12 bottom-0 z-40 grid place-items-center bg-[rgb(8_8_10/52%)]" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Где искать дубли"
        onClick={(e) => e.stopPropagation()}
        className="flex w-[560px] max-w-[calc(100%-32px)] flex-col gap-4 rounded-[28px] bg-cosmic px-7 pt-[26px] pb-6 shadow-[0_30px_80px_rgb(0_0_0/45%),0_0_0_1px_var(--line)]"
      >
        <h2 className="text-[28px] leading-[1.1] tracking-[-0.02em]">
          <span className="font-light">Где искать</span> <span className="font-bold">дубли?</span>
        </h2>
        <p className="-mt-1.5 m-0 text-[13px] leading-[1.45] text-dim">Пока ищу — раскладывать нельзя: пауза Space, отмена Esc.</p>
        <div role="radiogroup" aria-label="Где искать" className="grid grid-cols-2 gap-2.5">
          {areas.map((a) => (
            <button
              key={a.value}
              type="button"
              role="radio"
              aria-checked={scope === a.value}
              onClick={() => setScope(a.value)}
              className={`flex flex-col gap-[5px] rounded-[20px] bg-raised px-4 py-[15px] text-left transition-shadow duration-200 ease-trail ${
                scope === a.value ? "shadow-[inset_0_0_0_1.5px_var(--fg)]" : ""
              }`}
            >
              <b className="text-[15px]">{a.name}</b>
              <span className="text-[12.5px] leading-[1.35] text-dim">{a.hint}</span>
              <span className="text-[12.5px] font-bold tabular-nums">{a.files === null ? "считаю…" : filesWord(a.files)}</span>
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2.5">
          <span className="mr-auto text-[13px] text-dim">
            {minutes > 0 ? (
              <>
                Займёт около <b className="text-fg">{minutes} мин</b>
              </>
            ) : (
              "Отпечатки уже сняты — это быстро"
            )}
          </span>
          <Button variant="ghost" hotkey="Esc" onClick={onClose}>
            Отмена
          </Button>
          <Button variant="primary" hotkey="Enter" onClick={() => onStart(scope)}>
            Начать поиск
          </Button>
        </div>
      </div>
    </div>
  );
}
