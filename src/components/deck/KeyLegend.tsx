import { Keyboard } from "lucide-react";

function Cap({ children }: { children: string }) {
  return (
    <kbd className="inline-grid h-[22px] min-w-[22px] place-items-center rounded-md border-b-2 border-line bg-raised px-1.5 font-sans text-[11px] leading-none font-bold text-fg">
      {children}
    </kbd>
  );
}

const DUPE_ROWS: { keys: string[]; label: string }[] = [
  { keys: ["D"], label: "убрать лишнюю копию в Корзину" },
  { keys: ["N"], label: "это разные видео — не дубль" },
];

const ROWS: { keys: string[]; label: string }[] = [
  { keys: ["1–9", "Q…P"], label: "положить в стопку" },
  { keys: ["Enter"], label: "в подсказанную или найденную стопку" },
  { keys: ["Shift", "Enter"], label: "новая стопка из поиска" },
  { keys: ["Del"], label: "в корзину" },
  { keys: ["Tab"], label: "в конец колоды" },
  { keys: ["Ctrl", "Z"], label: "забрать ход — можно много раз" },
  { keys: ["Ctrl", "K"], label: "стопки и клавиши" },
  { keys: ["Space"], label: "пауза — или клик по видео" },
  { keys: ["←", "→"], label: "кадр плёнки назад / вперёд" },
  { keys: ["M"], label: "звук" },
  { keys: ["↑", "↓"], label: "громкость видео" },
  { keys: ["/"], label: "искать стопку" },
  { keys: ["Ctrl", "2"], label: "стол — разложить пачкой" },
];

export function KeyLegend({ muted, dupe = false }: { muted: boolean; dupe?: boolean }) {
  return (
    <div className="flex flex-col gap-1">
      {(dupe ? [...DUPE_ROWS, ...ROWS] : ROWS).map((r) => (
        <div key={r.label} className="flex items-center gap-3 text-[13px] text-dim">
          <span className="flex w-[104px] shrink-0 items-center gap-1">
            {r.keys.map((k, i) => (
              <span key={k} className="flex items-center gap-1">
                {i > 0 && <span className="text-[11px]">{r.keys[0] === "1–9" || r.keys[0] === "←" || r.keys[0] === "↑" ? "" : "+"}</span>}
                <Cap>{k}</Cap>
              </span>
            ))}
          </span>
          <span>{r.label === "звук" ? (muted ? "включить звук" : "выключить звук") : r.label}</span>
        </div>
      ))}
    </div>
  );
}

export function KeysHint({ muted, dupe = false }: { muted: boolean; dupe?: boolean }) {
  return (
    <div className="group relative">
      <button
        type="button"
        aria-label="Горячие клавиши"
        className="grid h-9 w-9 place-items-center rounded-full bg-raised text-dim transition-colors duration-200 ease-trail hover:bg-strong hover:text-fg focus-visible:text-fg"
      >
        <Keyboard size={16} strokeWidth={1.5} />
      </button>
      <div className="pointer-events-none invisible absolute right-0 bottom-full z-30 pb-3 opacity-0 transition-[opacity,translate,visibility] duration-200 ease-trail translate-y-1 group-hover:pointer-events-auto group-hover:visible group-hover:opacity-100 group-hover:translate-y-0 group-focus-within:visible group-focus-within:opacity-100 group-focus-within:translate-y-0">
        <div className="w-max rounded-[20px] border border-line bg-cosmic p-4 shadow-[0_20px_50px_rgb(0_0_0/35%)]">
          <KeyLegend muted={muted} dupe={dupe} />
        </div>
      </div>
    </div>
  );
}
