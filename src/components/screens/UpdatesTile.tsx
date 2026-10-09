import { checkNow, installNow, noteLines, useShip } from "@stillmvd/tauri-ship";
import { useEffect, useState } from "react";
import { Button } from "../ui/Button";

const NOTE = "Проверяю при запуске, раз в час и когда возвращаешься в окно, скачиваю в фоне. Перезапуск ставит новую версию — Windows один раз спросит разрешение.";

function checkedAgo(ms: number) {
  const minutes = Math.floor((Date.now() - ms) / 60_000);
  if (minutes < 1) return "проверено только что";
  if (minutes < 60) return `проверено ${minutes} мин назад`;
  return `проверено в ${new Date(ms).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" })}`;
}

export function UpdatesTile() {
  const { status } = useShip();
  const [checking, setChecking] = useState(false);
  const [, tick] = useState(0);

  useEffect(() => {
    const id = window.setInterval(() => tick((n) => n + 1), 30_000);
    return () => window.clearInterval(id);
  }, []);

  if (!status) return null;

  const check = async () => {
    setChecking(true);
    await checkNow().catch(() => {});
    setChecking(false);
  };

  const { phase, available, error } = status;
  const ready = phase === "ready" || phase === "installing";
  const percent = status.total ? Math.round((status.downloaded / status.total) * 100) : 0;
  const notes = ready ? noteLines(available?.notes ?? null).slice(0, 5) : [];
  const note =
    phase === "installing"
      ? "ставится, Sorter перезапустится"
      : ready
        ? `скачана ${available?.version}`
        : phase === "downloading"
          ? `скачивается ${available?.version ?? ""} · ${percent} %`
          : checking || phase === "checking"
            ? "проверяю…"
            : status.lastCheck
              ? `${available || error ? "" : "последняя версия · "}${checkedAgo(status.lastCheck)}`
              : "ещё не проверялось";

  return (
    <section className="flex flex-col justify-between gap-[18px] rounded-[20px] bg-raised px-[22px] py-5">
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-bold">Версия и обновления</h2>
        <p className="m-0 max-w-[460px] text-[13px] leading-[1.5] text-dim">{NOTE}</p>
        {error && (
          <p role="alert" className="m-0 max-w-[460px] text-[13px] leading-[1.5] font-medium text-fg [overflow-wrap:anywhere]">
            {error}
          </p>
        )}
      </div>
      {notes.length > 0 && (
        <div className="flex flex-col gap-1.5 rounded-[14px] bg-cosmic px-3.5 py-3">
          <div className="text-xs font-medium text-dim">Что нового в {available?.version}</div>
          {notes.map((n) => (
            <div key={n} className="flex gap-2 text-[13px] leading-[1.45]">
              <span className="mt-2 h-1 w-1 shrink-0 rounded-full bg-fg" />
              {n}
            </div>
          ))}
        </div>
      )}
      <div className="flex flex-col gap-2.5">
        <div className="flex min-h-11 items-center gap-4">
          <div className="flex min-w-0 flex-1 items-baseline gap-2.5 whitespace-nowrap">
            <span className="text-[28px] leading-none font-bold tracking-[-0.02em]">{status.current}</span>
            <span className="truncate text-[13px] font-medium text-dim">{note}</span>
          </div>
          {ready ? (
            <Button variant="primary" size={44} disabled={phase === "installing"} onClick={() => void installNow().catch(() => {})}>
              Перезапустить для обновления
            </Button>
          ) : (
            <Button
              size={44}
              className="bg-strong! hover:bg-line!"
              disabled={checking || phase === "checking" || phase === "downloading"}
              onClick={() => void check()}
            >
              Проверить
            </Button>
          )}
        </div>
        {phase === "downloading" && (
          <div className="h-1 rounded-full bg-strong">
            <div className="h-1 rounded-full bg-fg" style={{ width: `${percent}%` }} />
          </div>
        )}
      </div>
    </section>
  );
}
