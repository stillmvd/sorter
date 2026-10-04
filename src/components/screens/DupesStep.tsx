import { useEffect, useState } from "react";
import { ipc, type AppState, type DupesEstimate } from "../../lib/ipc";
import { nf, plural } from "../../lib/plural";
import { Toggle } from "../ui/Toggle";

export type DupesSetting = { enabled: boolean; scope: "deck" | "all" };

export const dupesSetting = (state: AppState): DupesSetting => ({
  enabled: state.settings.dupes_enabled === "1",
  scope: state.settings.dupes_scope === "all" ? "all" : "deck",
});

export const widened = (a: DupesSetting, b: DupesSetting) =>
  b.enabled && (!a.enabled || (a.scope === "deck" && b.scope === "all"));


export function useEstimate(scope: "deck" | "all", on: boolean, key: string) {
  const [est, setEst] = useState<DupesEstimate | null>(null);
  useEffect(() => {
    if (!on) return;
    let alive = true;
    void ipc.dupesEstimate(scope).then(
      (e) => alive && setEst(e),
      () => alive && setEst(null),
    );
    return () => {
      alive = false;
    };
  }, [scope, on, key]);
  return est;
}

export const filesWord = (n: number) => `${nf(n)} ${plural(n, "файл", "файла", "файлов")}`;

function note(set: DupesSetting, deck: boolean, est: DupesEstimate | null) {
  if (!set.enabled) return "Выключено — колода откроется сразу после кадров.";
  if (!deck) return "Сколько займёт — скажу, когда выберешь колоду.";
  if (!est) return "Считаю файлы…";
  if (set.scope === "deck") return `Сравню ${filesWord(est.deck)} колоды между собой.`;
  return `Сравню ${filesWord(est.deck)} колоды и ${nf(est.table)} с ${est.piles} ${plural(est.piles, "стопки", "стопок", "стопок")} стола.`;
}

const AREAS = [
  { value: "deck", name: "Внутри колоды", hint: () => "Копии среди файлов этой папки" },
  {
    value: "all",
    name: "Колода и стол",
    hint: (piles: number) => `Ещё и с тем, что уже разложено: ${piles} ${plural(piles, "стопка", "стопки", "стопок")}`,
  },
] as const;

export function DupesStep({
  state,
  n,
  onChanged,
}: {
  state: AppState;
  n?: number;
  onChanged: (before: DupesSetting, after: DupesSetting) => void;
}) {
  const set = dupesSetting(state);
  const deck = !!state.settings.deck_path;
  const piles = state.piles.filter((p) => !p.isTrash).length;
  const est = useEstimate(set.scope, set.enabled && deck, `${state.settings.deck_path}|${state.settings.table_path}`);
  const save = async (next: DupesSetting) => {
    await ipc.setSetting("dupes_scope", next.scope);
    await ipc.setSetting("dupes_enabled", next.enabled ? "1" : "0");
    onChanged(set, next);
  };
  return (
    <div className="flex items-start gap-4 rounded-[20px] bg-raised px-5 py-[18px]">
      {n !== undefined && (
        <span className="grid h-8 w-8 shrink-0 place-items-center rounded-full bg-strong text-[13px] font-bold">{n}</span>
      )}
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="text-[13px] font-medium text-dim">Дубли</div>
        <div className="flex items-center gap-2">
          <span className="text-lg font-bold">Искать копии</span>
          {set.enabled && deck && est && est.minutes > 0 && (
            <span className="inline-flex h-[22px] items-center rounded-full bg-strong px-2.5 text-xs font-bold tabular-nums">
              +{est.minutes} мин
            </span>
          )}
        </div>
        <div className="text-[13px] text-dim">{note(set, deck, est)}</div>
        {set.enabled && (
          <div role="radiogroup" aria-label="Где искать дубли" className="mt-2 flex flex-col gap-0.5 rounded-2xl bg-cosmic p-1">
            {AREAS.map((a) => {
              const on = set.scope === a.value;
              return (
                <button
                  key={a.value}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  onClick={() => !on && void save({ enabled: true, scope: a.value })}
                  className={`flex items-start gap-2.5 rounded-xl px-2.5 py-2 text-left transition-colors duration-200 ease-trail ${on ? "bg-raised" : "hover:bg-raised/60"}`}
                >
                  <span
                    className={`mt-0.5 h-4 w-4 shrink-0 rounded-full ${on ? "shadow-[inset_0_0_0_5px_var(--fg)]" : "shadow-[inset_0_0_0_1.5px_var(--dim)]"}`}
                  />
                  <span className="flex min-w-0 flex-col gap-px">
                    <span className="text-[13.5px] font-bold">{a.name}</span>
                    <span className="text-xs leading-[1.3] text-dim">{a.hint(piles)}</span>
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </div>
      <div className={set.enabled ? "mt-[22px]" : "self-center"}>
        <Toggle label="Искать копии" checked={set.enabled} onChange={(v) => void save({ ...set, enabled: v })} />
      </div>
    </div>
  );
}
