function Cap({ children }: { children: string }) {
  return (
    <kbd className="inline-grid h-[22px] min-w-[22px] place-items-center rounded-md border-b-2 border-line bg-raised px-1.5 font-sans text-[11px] leading-none font-bold text-fg">
      {children}
    </kbd>
  );
}

const ROWS: { keys: string[]; label: string }[] = [
  { keys: ["1–9", "Q…P"], label: "положить в стопку" },
  { keys: ["Enter"], label: "в найденную стопку" },
  { keys: ["Shift", "Enter"], label: "новая стопка из поиска" },
  { keys: ["Del"], label: "в корзину" },
  { keys: ["Tab"], label: "в конец колоды" },
  { keys: ["Ctrl", "Z"], label: "забрать ход — можно много раз" },
  { keys: ["Space"], label: "пауза — или клик по видео" },
  { keys: ["←", "→"], label: "кадр плёнки назад / вперёд" },
  { keys: ["M"], label: "звук" },
  { keys: ["↑", "↓"], label: "громкость видео" },
  { keys: ["/"], label: "искать стопку" },
  { keys: ["Ctrl", "2"], label: "стол — разложить пачкой" },
];

export function KeyLegend({ muted }: { muted: boolean }) {
  return (
    <div className="flex flex-col gap-1">
      {ROWS.map((r) => (
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
