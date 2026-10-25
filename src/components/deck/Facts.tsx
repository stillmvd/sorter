import type { Card } from "../../lib/ipc";

const mb = (b: number) => (b >= 1024 ** 2 ? `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ` : `${Math.max(1, Math.round(b / 1024))} КБ`);

function length(ms: number) {
  const s = Math.round(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

function when(card: Card) {
  if (!card.takenAt) return null;
  const naive = card.takenFrom === "exif" || (card.takenFrom === "name" && !/^\d{13}\./.test(card.fileName));
  const d = new Date(card.takenAt);
  const timeZone = naive ? "UTC" : undefined;
  const day = d.toLocaleDateString("ru-RU", { day: "numeric", month: "long", timeZone });
  const time = d.toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit", timeZone });
  return `${day} ${naive ? d.getUTCFullYear() : d.getFullYear()}, ${time}`;
}

export function Facts({ card }: { card: Card }) {
  const date = when(card);
  const format = card.fileName.split(".").pop()?.toUpperCase() ?? "";
  const photo = card.kind === "photo";
  const rows: { k: string; v: string; note?: string }[] = [];
  if (date)
    rows.push(
      card.takenFrom === "file"
        ? { k: "Дата файла", v: date, note: "даты съёмки в файле нет" }
        : { k: "Снято", v: date, note: card.takenFrom === "name" ? "по имени файла" : undefined },
    );
  if (card.camera) rows.push({ k: "Камера", v: card.camera });
  if (!photo && card.durationMs) rows.push({ k: "Длина", v: length(card.durationMs) });
  if (card.width && card.height)
    rows.push({
      k: "Разрешение",
      v: `${card.width} × ${card.height}`,
      note: photo ? `${((card.width * card.height) / 1e6).toFixed(1).replace(".", ",").replace(",0", "")} Мп` : undefined,
    });
  rows.push({ k: "Размер", v: mb(card.size), note: format });
  return (
    <div className="flex flex-col rounded-[20px] bg-raised px-4 py-1.5">
      {rows.map((r, i) => (
        <div key={r.k} className={`flex items-baseline gap-3 py-[9px] ${i < rows.length - 1 ? "border-b border-line" : ""}`}>
          <div className="w-[92px] shrink-0 text-[13px] font-medium text-dim">{r.k}</div>
          <div className={`text-sm font-medium ${r.note ? "shrink-0" : "min-w-0 truncate"}`}>{r.v}</div>
          {r.note && <div className="min-w-0 truncate text-xs text-dim">{r.note}</div>}
        </div>
      ))}
    </div>
  );
}
