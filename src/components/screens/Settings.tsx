import { useEffect, useRef, useState } from "react";
import { ipc, type AppState } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { Button } from "../ui/Button";
import { BackButton, Heading } from "../ui/PageHeader";
import { Segment } from "../ui/Segment";
import { Toggle } from "../ui/Toggle";
import { DupesStep, widened, type DupesSetting } from "./DupesStep";
import { UpdatesTile } from "./UpdatesTile";

type Theme = "system" | "dark" | "light";

const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "Системная" },
  { value: "dark", label: "Тёмная" },
  { value: "light", label: "Светлая" },
];

function mb(bytes: number) {
  const v = bytes / 1024 / 1024;
  return `${v < 10 && v > 0 ? v.toFixed(1).replace(".", ",") : Math.round(v)} МБ`;
}

function Tile({ title, note, children, wide = false }: { title: string; note: string; children: React.ReactNode; wide?: boolean }) {
  return (
    <section className={`flex flex-col justify-between gap-[18px] rounded-[20px] bg-raised px-[22px] py-5 ${wide ? "col-span-2" : ""}`}>
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-bold">{title}</h2>
        <p className={`m-0 text-[13px] leading-[1.5] text-dim ${wide ? "max-w-[80ch]" : "max-w-[460px]"}`}>{note}</p>
      </div>
      {children}
    </section>
  );
}

export function SettingsScreen({
  state,
  onChange,
  onBack,
  onWiden,
}: {
  state: AppState;
  onChange: () => Promise<void>;
  onBack: () => void;
  onWiden: (before: DupesSetting) => void;
}) {
  const [cache, setCache] = useState<{ bytes: number; cards: number } | null>(null);
  const [freed, setFreed] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const confirmTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => () => clearTimeout(confirmTimer.current), []);

  useEffect(() => {
    void ipc.cacheInfo().then(setCache);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onBack();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack]);

  const theme = (state.settings.theme as Theme | undefined) ?? "system";
  const hints = state.settings.hints_enabled === "1";
  const piles = state.piles.filter((p) => !p.isTrash);
  const examples = piles.reduce((n, p) => n + p.examples, 0);

  const set = async (key: string, value: string) => {
    await ipc.setSetting(key, value);
    await onChange();
  };

  const clear = async () => {
    clearTimeout(confirmTimer.current);
    if (!confirm) {
      setConfirm(true);
      confirmTimer.current = setTimeout(() => setConfirm(false), 3000);
      return;
    }
    setConfirm(false);
    setBusy(true);
    try {
      setFreed(await ipc.clearCache());
      setCache(await ipc.cacheInfo());
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="mx-2 mb-2 flex min-h-0 flex-1 flex-col gap-7 overflow-y-auto rounded-[28px] bg-cosmic px-10 py-7">
      <div className="flex flex-col gap-3.5">
        <BackButton onClick={onBack} />
        <Heading light="Настройки" bold="Sorter" />
        <p className="m-0 text-[15px] leading-[1.55] text-dim">Всё сохраняется сразу. Файлы в папках настройки не трогают.</p>
      </div>

      <div className="grid grid-cols-2 content-start gap-3">
        <Tile title="Тема" note="Системная повторяет Windows и меняется вместе с ней.">
          <div className="self-start">
            <Segment label="Тема" className="bg-cosmic" options={THEMES} value={theme} onChange={(v) => void set("theme", v)} />
          </div>
        </Tile>

        <section className="flex flex-col justify-between gap-4 rounded-[20px] bg-raised px-[22px] py-5">
          <div className="flex items-start gap-4">
            <div className="flex flex-1 flex-col gap-1">
              <h2 className="text-lg font-bold">Подсказки по кадрам</h2>
              <p className="m-0 max-w-[440px] text-[13px] leading-[1.5] text-dim">
                Считаются на этом компьютере, без сети. Выключишь — блок подсказки пропадёт, новые кадры перестанут изучаться.
              </p>
            </div>
            <Toggle label="Подсказки по кадрам" checked={hints} onChange={(v) => void set("hints_enabled", v ? "1" : "0")} />
          </div>
          <p className="m-0 text-[13px] text-dim">
            {hints ? (
              <>
                <span className="font-bold text-fg">Учится на {examples} {plural(examples, "карте", "картах", "картах")}</span> ·{" "}
                {piles.length} {plural(piles.length, "стопка", "стопки", "стопок")}
              </>
            ) : (
              "Выключены — раскладывай клавишами и поиском."
            )}
          </p>
        </section>

        <div className="col-span-2">
          <DupesStep state={state} onChanged={(before, after) => (widened(before, after) ? onWiden(before) : void onChange())} />
        </div>

        <Tile title="Кэш проявки" note="Раскадровки и починенные копии видео. Карты открытой колоды останутся, остальное проявится заново, когда понадобится.">
          <div className="flex items-center gap-4">
            <div className="flex min-w-0 flex-1 items-baseline gap-2.5 whitespace-nowrap">
              <span className="text-[28px] leading-none font-bold tracking-[-0.02em]">{cache ? mb(cache.bytes) : "…"}</span>
              <span className="truncate text-[13px] font-medium text-dim">
                {freed !== null ? `освобождено ${mb(freed)}` : cache ? `${cache.cards} ${plural(cache.cards, "карта", "карты", "карт")}` : ""}
              </span>
            </div>
            <Button size={44} className="bg-strong! hover:bg-line!" disabled={busy || !cache?.bytes} onClick={() => void clear()}>
              {confirm ? "Точно очистить?" : freed !== null && !cache?.bytes ? "Очищено" : "Очистить кэш"}
            </Button>
          </div>
        </Tile>

        <UpdatesTile />
      </div>
    </main>
  );
}
