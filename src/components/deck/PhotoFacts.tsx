import type { Card } from "../../lib/ipc";

const mb = (b: number) => (b >= 1024 ** 2 ? `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ` : `${Math.max(1, Math.round(b / 1024))} КБ`);

function when(card: Card) {
  if (!card.takenAt) return null;
  const naive = card.takenFrom === "exif" || (card.takenFrom === "name" && !/^\d{13}\./.test(card.fileName));
  return new Date(card.takenAt).toLocaleString("ru-RU", {
    day: "numeric",
    month: "long",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    timeZone: naive ? "UTC" : undefined,
  });
}

export function PhotoFacts({ card }: { card: Card }) {
  const date = when(card);
  const format = card.fileName.split(".").pop()?.toUpperCase() ?? "";
  const rows: { k: string; v: string; note?: string }[] = [];
  if (date)
    rows.push(
      card.takenFrom === "file"
        ? { k: "Дата файла", v: date, note: "даты съёмки в файле нет" }
        : { k: "Снято", v: date, note: card.takenFrom === "name" ? "по имени файла" : undefined },
    );
  if (card.camera) rows.push({ k: "Камера", v: card.camera });
  if (card.width && card.height)
    rows.push({
      k: "Разрешение",
      v: `${card.width} × ${card.height}`,
      note: `${((card.width * card.height) / 1e6).toFixed(1).replace(".", ",").replace(",0", "")} Мп`,
    });
  rows.push({ k: "Размер", v: mb(card.size), note: format });
  return (
    <div className="flex flex-col rounded-[20px] bg-raised px-4 py-1.5">
      {rows.map((r, i) => (
        <div key={r.k} className={`flex items-baseline gap-3 py-[9px] ${i < rows.length - 1 ? "border-b border-line" : ""}`}>
          <div className="w-24 shrink-0 text-[13px] font-medium text-dim">{r.k}</div>
          <div className="min-w-0 truncate text-sm font-medium">{r.v}</div>
          {r.note && <div className="truncate text-xs text-dim">{r.note}</div>}
        </div>
      ))}
    </div>
  );
}
