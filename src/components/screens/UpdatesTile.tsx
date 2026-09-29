import { useEffect, useState } from "react";
import { errorText, ipc, onUpdateState, type UpdateState } from "../../lib/ipc";
import { Button } from "../ui/Button";
import { Toggle } from "../ui/Toggle";

const AUTO = "Проверяю при запуске и раз в сутки, скачиваю в фоне. Ставится при закрытии — Windows один раз спросит разрешение.";

function when(ms: number | null) {
  if (!ms) return "";
  const d = new Date(ms);
  const today = new Date().toDateString() === d.toDateString();
  return today ? "сегодня" : d.toLocaleDateString("ru-RU", { day: "numeric", month: "long" });
}

function lines(notes: string | null) {
  return (notes ?? "")
    .split("\n")
    .map((l) => l.replace(/^\s*[-*•]\s*/, "").trim())
    .filter(Boolean)
    .slice(0, 5);
}

export function UpdatesTile({ on, onToggle }: { on: boolean; onToggle: (v: boolean) => void }) {
  const [s, setS] = useState<UpdateState | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    void ipc.updateState().then(setS);
    const off = onUpdateState(setS);
    return () => void off.then((f) => f());
  }, []);

  if (!s) return null;

  const check = async () => {
    setBusy(true);
    setS(await ipc.updateCheck());
    setBusy(false);
  };

  const restart = async () => {
    setFailure(null);
    try {
      await ipc.updateInstall();
    } catch (e) {
      setFailure(errorText(e));
    }
  };

  const phase = s.phase;
  const ready = phase === "ready";
  const notes = ready || (s.installed && phase !== "downloading") ? lines(s.notes) : [];
  const version = ready ? (s.version ?? s.current) : s.current;
  const note =
    phase === "downloading"
      ? `скачивается ${s.version ?? ""} · ${Math.round((s.progress ?? 0) * 100)} %`
      : ready
        ? "скачано, поставится при закрытии"
        : phase === "failed"
          ? "не удалось проверить"
          : phase === "off"
            ? "проверка выключена"
            : phase === "checking" || busy
              ? "проверяю…"
              : s.installed
                ? "установлено"
                : phase === "latest"
                  ? `последняя версия · проверено ${when(s.checkedAt)}`
                  : "ещё не проверялось";
  const alert = failure ?? (ready || phase === "failed" ? s.error : null);
  const text = alert
    ? alert
    : ready
      ? "Или перезапусти сейчас — разложенное не потеряется. Windows спросит разрешение на установку."
      : phase === "off"
        ? "Sorter не выходит в сеть. Включи, чтобы получать новые версии."
        : AUTO;

  return (
    <section className="flex flex-col justify-between gap-[18px] rounded-[20px] bg-raised px-[22px] py-5">
      <div className="flex items-start gap-4">
        <div className="flex flex-1 flex-col gap-1">
          <h2 className="text-lg font-bold">Версия и обновления</h2>
          <p role={alert ? "alert" : undefined} className={`m-0 max-w-[460px] text-[13px] leading-[1.5] ${alert ? "font-medium text-fg" : "text-dim"}`}>
            {text}
          </p>
        </div>
        <Toggle label="Проверять обновления" checked={on} onChange={onToggle} />
      </div>
      {notes.length > 0 && (
        <div className="flex flex-col gap-1.5 rounded-[14px] bg-cosmic px-3.5 py-3">
          <div className="text-xs font-medium text-dim">Что нового в {ready ? s.version : s.current}</div>
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
            <span className="text-[28px] leading-none font-bold tracking-[-0.02em]">{version}</span>
            <span className="truncate text-[13px] font-medium text-dim">{note}</span>
          </div>
          {ready ? (
            <Button variant="primary" size={44} onClick={() => void restart()}>
              Перезапустить
            </Button>
          ) : (
            phase !== "downloading" &&
            phase !== "off" && (
              <Button size={44} className="bg-strong! hover:bg-line!" disabled={busy || phase === "checking"} onClick={() => void check()}>
                Проверить сейчас
              </Button>
            )
          )}
        </div>
        {phase === "downloading" && (
          <div className="h-1 rounded-full bg-strong">
            <div className="h-1 rounded-full bg-fg" style={{ width: `${Math.round((s.progress ?? 0) * 100)}%` }} />
          </div>
        )}
      </div>
    </section>
  );
}
