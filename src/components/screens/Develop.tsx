import { Copy, Film, Pause, Play } from "lucide-react";
import { useEffect, useState } from "react";
import { ipc, onProgress, type AppState, type Developing } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { Scanner } from "../fx/Scanner";
import { Sieve } from "../fx/Sieve";
import { Button } from "../ui/Button";
import { Heading } from "../ui/PageHeader";

const nf = (n: number) => String(n).replace(/\B(?=(\d{3})+(?!\d))/g, " ");

type Purpose = "deck" | "dupes" | "search";
type Status = "done" | "run" | "pause" | "wait";
type Stage = { key: string; name: string; hint: string; status: Status; done: number; total: number; value?: string };

function speedText(p: Developing) {
  const [one, few, many] = p.stage === "dupes" ? ["файл", "файла", "файлов"] : ["карта", "карты", "карт"];
  if (p.speed <= 0) return "считаю скорость…";
  if (p.speed < 1) return `${p.speed.toFixed(1).replace(".", ",")} ${few}/с`;
  const n = Math.round(p.speed);
  return `${n} ${plural(n, one, few, many)}/с`;
}

function percent(p: Developing) {
  if (p.ready) return 100;
  const r = (a: number, b: number) => (b ? Math.min(1, a / b) : 1);
  if (p.search) return Math.min(99, Math.floor(r(p.printed, p.prints) * 100));
  const dupes = p.scope !== "off";
  const w = [1, dupes ? 3 : 0, p.hints ? 0.8 : 0];
  const sum = w[0] + w[1] + w[2];
  const v =
    (w[0] * r(p.done, p.total) + w[1] * (p.stage === "frames" ? 0 : r(p.printed, p.prints)) + w[2] * (p.stage === "hints" ? r(p.embedded, p.total) : 0)) / sum;
  return Math.min(99, Math.floor(v * 100));
}

const DUPES_HINT = "Сравниваю кадры и звук: копии, пережатые, обрезки";
const groupsWord = (n: number) => `${nf(n)} ${plural(n, "группа", "группы", "групп")}`;
const filesWord = (n: number) => `${nf(n)} ${plural(n, "файл", "файла", "файлов")}`;

function stagesOf(p: Developing, groups: number | null): Stage[] {
  const order = ["frames", "dupes", "hints", "done"];
  const at = order.indexOf(p.stage);
  const status = (key: string): Status => {
    const i = order.indexOf(key);
    if (i < at) return "done";
    if (i > at) return "wait";
    return p.paused ? "pause" : "run";
  };
  const dupes: Stage = {
    key: "dupes",
    name: "Дубли",
    hint: DUPES_HINT,
    status: status("dupes"),
    done: p.printed,
    total: p.prints,
    value: groups === null ? undefined : groupsWord(groups),
  };
  if (p.search) {
    return [{ key: "files", name: "Файлы", hint: "", status: "done", done: p.prints, total: p.prints, value: filesWord(p.prints) }, dupes];
  }
  const list: Stage[] = [
    { key: "files", name: "Файлы", hint: "Ищу видео и фото в папке", status: "done", done: p.total, total: p.total, value: filesWord(p.total) },
    { key: "frames", name: "Кадры", hint: "8 кадров с видео, 1 с фото, размер и поворот", status: status("frames"), done: p.done, total: p.total },
  ];
  if (p.scope !== "off") list.push(dupes);
  if (p.hints) list.push({ key: "hints", name: "Подсказки", hint: "Учу видеокарту узнавать твои стопки", status: status("hints"), done: p.embedded, total: p.total });
  return list;
}

function StageCard({ s }: { s: Stage }) {
  const n = `${nf(s.done)} / ${nf(s.total)}`;
  const value = s.status === "done" ? (s.value ?? "готово") : s.status === "wait" ? "ждёт" : s.status === "pause" ? `${n} · пауза` : n;
  const pct = s.status === "done" ? 100 : s.total ? Math.round((s.done / s.total) * 100) : 0;
  return (
    <div className={`flex flex-col rounded-[20px] bg-raised px-[18px] ${s.status === "done" ? "gap-[7px] py-3" : "gap-[9px] py-[15px]"} ${s.status === "wait" ? "opacity-55" : ""}`}>
      <div className="flex items-baseline gap-2">
        <b className="flex-1 text-[15px] font-bold">{s.name}</b>
        <span className="whitespace-nowrap text-[13px] font-bold">{value}</span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-strong">
        <div className={`h-1.5 rounded-full transition-[width] duration-300 ease-trail ${s.status === "pause" ? "bg-dim" : "bg-fg"}`} style={{ width: `${pct}%` }} />
      </div>
      {s.status !== "done" && s.hint && <div className="text-[13px] leading-[1.35] text-dim">{s.hint}</div>}
    </div>
  );
}

function Ring({ pct, paused }: { pct: number; paused: boolean }) {
  const r = 48;
  const len = 2 * Math.PI * r;
  return (
    <span className="relative h-28 w-28 shrink-0">
      <svg viewBox="0 0 112 112" className="h-28 w-28 -rotate-90">
        <circle cx="56" cy="56" r={r} fill="none" strokeWidth="9" className="stroke-raised" />
        <circle
          cx="56"
          cy="56"
          r={r}
          fill="none"
          strokeWidth="9"
          strokeLinecap="round"
          strokeDasharray={len}
          strokeDashoffset={len * (1 - pct / 100)}
          className={`transition-[stroke-dashoffset] duration-300 ease-trail ${paused ? "stroke-dim" : "stroke-fg"}`}
        />
      </svg>
      <b className="absolute inset-0 grid place-items-center text-[26px] font-bold">{pct}%</b>
    </span>
  );
}

function Live({ p }: { p: Developing }) {
  const scan = p.total === 0 && p.stage === "frames" && !p.search;
  const dupes = p.stage === "dupes";
  const caption = scan ? "ИЩУ ФАЙЛЫ" : dupes ? "СРАВНИВАЮ" : "ПРОЯВЛЯЮ КАДРЫ";
  return (
    <div className="relative grid min-h-0 flex-1 place-items-center overflow-hidden rounded-[20px] bg-film">
      <span className="absolute left-4 top-3 z-1 font-mono text-[10px] tracking-[0.06em] text-[var(--film-text)]">{caption}</span>
      {dupes ? <Sieve paused={p.paused} /> : <Scanner paused={p.paused} />}
    </div>
  );
}

function Summary({ title, tiles }: { title: [string, string]; tiles: [number, string][] }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col justify-center rounded-[20px] bg-ground">
      <h3 className="mx-[18px] text-[22px] font-light">
        {title[0]} <b className="font-bold">{title[1]}</b>
      </h3>
      <div className="grid grid-cols-3 gap-2.5 p-[18px]">
        {tiles.map(([n, label]) => (
          <div key={label} className="flex flex-col gap-1 rounded-[20px] bg-raised px-[18px] py-4">
            <b className="text-[34px] font-light leading-none tracking-[-0.02em]">{nf(n)}</b>
            <span className="text-[13px] font-medium text-dim">{label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function searchWhere(raw: string | undefined) {
  try {
    const s = JSON.parse(raw ?? "") as { piles?: string[]; deck?: boolean; folders?: string[]; skipped?: string[] };
    return [...(s.piles ?? []), ...(s.deck ? ["колода"] : []), ...(s.folders ?? []).filter((f) => !s.skipped?.includes(f))].join(", ");
  } catch {
    return "";
  }
}

const SCOPE_CHIP = { off: "", deck: " · дубли внутри колоды", all: " · дубли в колоде и на столе", search: "" } as const;

export function Develop({ state, purpose, onDone, onCancel }: { state: AppState; purpose: Purpose; onDone: () => void; onCancel: () => void }) {
  const [p, setP] = useState(state.developing);
  const [groups, setGroups] = useState<number | null>(null);
  const [kinds, setKinds] = useState<{ video: number; photo: number } | null>(null);

  useEffect(() => {
    void ipc.getState().then((s) => setP(s.developing));
    const off = onProgress(setP);
    return () => void off.then((f) => f());
  }, []);

  const dupesDone = p.scope !== "off" && (p.stage === "hints" || p.stage === "done");
  useEffect(() => {
    if (!dupesDone) return;
    void ipc.dupeGroups().then((g) => setGroups(g.groups.length));
  }, [dupesDone]);

  useEffect(() => {
    if (!p.ready) return;
    void ipc.getState().then((s) => setKinds({ video: s.deck.byKind.video.left, photo: s.deck.byKind.photo.left }));
  }, [p.ready]);

  const toggle = () => {
    void ipc.developControl(!p.paused);
    setP((x) => ({ ...x, paused: !x.paused }));
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat) return;
      if (e.code === "Space" && !p.ready) {
        e.preventDefault();
        toggle();
      } else if (e.key === "Enter" && p.ready) {
        e.preventDefault();
        onDone();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const search = purpose === "search";
  const pct = percent(p);
  const stages = stagesOf(p, groups);
  const title: [string, string] = search
    ? p.ready
      ? ["Поиск", "закончен"]
      : p.paused
        ? ["Поиск", "на паузе"]
        : ["Ищу", "дубли"]
    : p.ready
      ? ["Колода", "готова"]
      : p.paused
        ? ["Разбор", "на паузе"]
        : ["Разбираю", "колоду"];
  const where = search ? searchWhere(state.settings.dupe_search) : "";
  const summary: { title: [string, string]; tiles: [number, string][] } = search
    ? { title: ["Сравнил", filesWord(p.prints)], tiles: [[groups ?? 0, plural(groups ?? 0, "группа дублей", "группы дублей", "групп дублей")]] }
    : {
        title: ["Разобрал", `${nf(p.total)} ${plural(p.total, "карту", "карты", "карт")}`],
        tiles: [
          [kinds?.video ?? 0, "видео"],
          [kinds?.photo ?? 0, "фото"],
          ...(p.scope !== "off" ? [[groups ?? 0, plural(groups ?? 0, "группа дублей", "группы дублей", "групп дублей")] as [number, string]] : []),
        ],
      };

  return (
    <main className="mx-2 mb-2 flex min-h-0 flex-1 gap-2">
      <section className="flex min-w-0 flex-1 flex-col gap-5 rounded-[28px] bg-cosmic px-9 py-7">
        <div className="flex h-7 max-w-full items-center gap-2 self-start rounded-full bg-raised px-3 text-xs font-medium text-dim [&_svg]:h-3.5 [&_svg]:w-3.5 [&_svg]:shrink-0 [&_svg]:text-fg">
          {search ? <Copy strokeWidth={1.5} /> : <Film strokeWidth={1.5} />}
          <span className="truncate">{search ? `Поиск дублей${where ? ` · ${where}` : ""}` : `Разбор колоды${SCOPE_CHIP[p.scope]}`}</span>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex min-w-0 flex-1 items-center gap-[22px]">
            <Ring pct={pct} paused={p.paused && !p.ready} />
            <div className="flex min-w-0 flex-col gap-1.5">
              <Heading light={title[0]} bold={title[1]} size={28} />
              <span className="flex items-baseline gap-2 text-[13px] font-medium text-dim">
                <b className="text-[15px] font-bold text-fg">{pct} %</b>
                <span>{p.ready ? "всё готово" : `· ${p.paused ? "на паузе" : speedText(p)}`}</span>
              </span>
            </div>
          </div>
          {!p.ready && (
            <button
              type="button"
              onClick={(e) => {
                e.currentTarget.blur();
                toggle();
              }}
              title={p.paused ? "Продолжить — Space" : "Пауза — Space"}
              className="grid h-11 w-11 shrink-0 place-items-center rounded-full bg-raised text-fg transition-colors duration-200 ease-trail hover:bg-strong [&_svg]:h-[18px] [&_svg]:w-[18px]"
            >
              {p.paused ? <Play strokeWidth={1.75} /> : <Pause strokeWidth={1.75} />}
            </button>
          )}
        </div>
        {p.ready ? <Summary title={summary.title} tiles={summary.tiles} /> : <Live p={p} />}
      </section>
      <aside className="flex w-[380px] shrink-0 flex-col gap-3 rounded-[28px] bg-cosmic px-[26px] pb-[26px] pt-7">
        <Heading light="Что" bold="происходит" size={28} />
        {stages.map((s) => (
          <StageCard key={s.key} s={s} />
        ))}
        <div className="flex-1" />
        {!p.ready && (
          <p className="m-0 text-[13px] leading-[1.4] text-dim">
            {search ? "Итоги откроются, когда поиск закончится." : "Колода откроется, когда разбор закончится."}
          </p>
        )}
        <Button variant={p.ready ? "primary" : "secondary"} size={48} hotkey="Enter" disabled={!p.ready} onClick={onDone}>
          {search ? "К дублям" : "Раскладывать"}
        </Button>
        <Button variant="ghost" size={40} hotkey="Esc" onClick={onCancel} className="-mt-1 text-dim">
          {purpose === "deck" ? "Выбрать другую папку" : "Отменить поиск"}
        </Button>
      </aside>
    </main>
  );
}
