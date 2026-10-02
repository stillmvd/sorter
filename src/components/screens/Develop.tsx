import { Film, Pause, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { frameSrc, ipc, onCards, onProgress, type AppState, type Card, type Developing } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { Button } from "../ui/Button";
import { Heading } from "../ui/PageHeader";

const nf = (n: number) => String(n).replace(/\B(?=(\d{3})+(?!\d))/g, " ");

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
  const [wf, wd, wh] = p.hints ? [0.25, 0.55, 0.2] : [0.25, 0.75, 0];
  const v = wf * r(p.done, p.total) + wd * (p.stage === "frames" ? 0 : r(p.printed, p.prints)) + wh * (p.stage === "hints" ? r(p.embedded, p.total) : 0);
  return Math.min(99, Math.floor(v * 100));
}

function stagesOf(p: Developing, groups: number | null): Stage[] {
  const order = ["frames", "dupes", "hints", "done"];
  const at = order.indexOf(p.stage);
  const status = (key: string): Status => {
    const i = order.indexOf(key);
    if (i < at) return "done";
    if (i > at) return "wait";
    return p.paused ? "pause" : "run";
  };
  const list: Stage[] = [
    { key: "files", name: "Файлы", hint: "Ищу видео и фото в папке", status: "done", done: p.total, total: p.total, value: `${nf(p.total)} ${plural(p.total, "файл", "файла", "файлов")}` },
    { key: "frames", name: "Кадры", hint: "8 кадров с видео, 1 с фото, размер и поворот", status: status("frames"), done: p.done, total: p.total },
    {
      key: "dupes",
      name: "Дубли",
      hint: "Сравниваю кадры и звук: копии, пережатые, обрезки",
      status: status("dupes"),
      done: p.printed,
      total: p.prints,
      value: groups === null ? undefined : `${nf(groups)} ${plural(groups, "группа", "группы", "групп")}`,
    },
  ];
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
      {s.status !== "done" && <div className="text-[13px] leading-[1.35] text-dim">{s.hint}</div>}
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

type Shot = { src: string | null; label: string; pending?: boolean };

const shotOf = (cacheDir: string, card: Card | null | undefined, label: string): Shot => ({
  src: card && card.frames > 0 ? frameSrc(cacheDir, card, Math.min(3, card.frames - 1)) : null,
  label,
});

function useSynced(shots: Shot[]) {
  const [shown, setShown] = useState(shots);
  const seq = useRef({ asked: 0, shown: 0 });
  const key = JSON.stringify(shots);
  useEffect(() => {
    const my = ++seq.current.asked;
    const loads = shots.map((s) => {
      if (!s.src) return Promise.resolve();
      const img = new Image();
      img.src = s.src;
      return img.decode().catch(() => undefined);
    });
    void Promise.all(loads).then(() => {
      if (my < seq.current.shown) return;
      seq.current.shown = my;
      setShown(shots);
    });
  }, [key]);
  return shown;
}

function Thumb({ shot, big }: { shot: Shot; big?: boolean }) {
  const size = big ? "h-[230px] w-[150px] rounded-2xl shadow-[0_0_0_1px_#2e2e33]" : "h-[114px] w-[74px] rounded-[10px] opacity-45";
  if (shot.pending) return <i className={`block border border-dashed border-[#2a2a2f] ${size} ${big ? "" : "opacity-100"}`} />;
  return (
    <i className={`block overflow-hidden bg-[linear-gradient(150deg,#46464c,#26262a)] ${size}`}>
      {shot.src && <img src={shot.src} alt="" className="h-full w-full object-cover" />}
    </i>
  );
}

const name = "max-w-[200px] truncate font-mono text-[10px] tracking-[0.06em] text-[#a2a2a9]";

function Live({ p, recent, lastPair, cacheDir }: { p: Developing; recent: Card[]; lastPair: Developing["pair"]; cacheDir: string }) {
  const scan = p.total === 0 && p.stage === "frames";
  const pair = p.stage === "dupes" ? (p.pair ?? lastPair ?? [{ name: "", card: null }]) : null;
  const caption = scan ? "ИЩУ ФАЙЛЫ" : pair ? "СРАВНИВАЮ" : "ТОЛЬКО ЧТО";
  const [last, prev] = [recent[0], recent[1]];
  const shots = useSynced(
    pair
      ? [shotOf(cacheDir, pair[0]?.card, pair[0]?.name ?? ""), pair[1] ? shotOf(cacheDir, pair[1].card, pair[1].name) : { src: null, label: "ищу похожие…", pending: true }]
      : [
          { ...shotOf(cacheDir, prev, ""), pending: !prev },
          { ...shotOf(cacheDir, last, scan ? "ищу файлы…" : (last?.fileName ?? "готовлю первую карту…")), pending: !last },
        ],
  );
  const [a, b] = shots;
  return (
    <div className="relative min-h-0 flex-1 overflow-hidden rounded-[20px] bg-film">
      <span className="absolute left-4 top-3 font-mono text-[10px] tracking-[0.06em] text-[var(--film-text)]">{caption}</span>
      <div className="flex h-full items-center justify-center gap-[22px] px-4 pb-4 pt-[30px]">
        {pair ? (
          <>
            <span className="flex flex-col items-center gap-2">
              <Thumb shot={a} big />
              <span className={name}>{a.label}</span>
            </span>
            <span className="text-[22px] text-[#ececef]">≈</span>
            <span className="flex flex-col items-center gap-2">
              <Thumb shot={b} big />
              <span className={name}>{b.label}</span>
            </span>
          </>
        ) : (
          <>
            <Thumb shot={a} />
            <span className="flex flex-col items-center gap-2">
              <Thumb shot={b} big />
              <span className={name}>{b.label}</span>
            </span>
            <Thumb shot={{ src: null, label: "", pending: true }} />
          </>
        )}
      </div>
    </div>
  );
}

function Summary({ total, video, photo, groups }: { total: number; video: number; photo: number; groups: number }) {
  const tiles = [
    [video, "видео"],
    [photo, "фото"],
    [groups, plural(groups, "группа дублей", "группы дублей", "групп дублей")],
  ] as const;
  return (
    <div className="flex min-h-0 flex-1 flex-col justify-center rounded-[20px] bg-ground">
      <h3 className="mx-[18px] text-[22px] font-light">
        Разобрал <b className="font-bold">{nf(total)} {plural(total, "карту", "карты", "карт")}</b>
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

export function Develop({ state, onDone, onCancel }: { state: AppState; onDone: () => void; onCancel: () => void }) {
  const [p, setP] = useState(state.developing);
  const [recent, setRecent] = useState<Card[]>([]);
  const [groups, setGroups] = useState<number | null>(null);
  const [lastPair, setLastPair] = useState<Developing["pair"]>(null);
  const [kinds, setKinds] = useState<{ video: number; photo: number } | null>(null);

  useEffect(() => {
    void ipc.getState().then((s) => setP(s.developing));
    const a = onCards((fresh) => {
      const shown = fresh.filter((c) => c.frames > 0 && (c.status === "in_deck" || c.status === "deferred"));
      if (shown.length) setRecent((r) => [...shown.slice(-2).reverse(), ...r].slice(0, 2));
    });
    const b = onProgress((x) => {
      setP(x);
      if (x.pair) setLastPair(x.pair);
    });
    return () => {
      void a.then((f) => f());
      void b.then((f) => f());
    };
  }, []);

  const dupesDone = p.stage === "hints" || p.stage === "done";
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

  const pct = percent(p);
  const stages = stagesOf(p, groups);
  const title = p.ready ? ["Колода", "готова"] : p.paused ? ["Разбор", "на паузе"] : ["Разбираю", "колоду"];

  return (
    <main className="mx-2 mb-2 flex min-h-0 flex-1 gap-2">
      <section className="flex min-w-0 flex-1 flex-col gap-5 rounded-[28px] bg-cosmic px-9 py-7">
        <div className="flex h-7 items-center gap-2 self-start rounded-full bg-raised px-3 text-xs font-medium text-dim [&_svg]:h-3.5 [&_svg]:w-3.5 [&_svg]:text-fg">
          <Film strokeWidth={1.5} />
          Разбор колоды
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
        {p.ready ? (
          <Summary total={p.total} video={kinds?.video ?? 0} photo={kinds?.photo ?? 0} groups={groups ?? 0} />
        ) : (
          <Live p={p} recent={recent} lastPair={lastPair} cacheDir={state.cacheDir} />
        )}
      </section>
      <aside className="flex w-[380px] shrink-0 flex-col gap-3 rounded-[28px] bg-cosmic px-[26px] pb-[26px] pt-7">
        <Heading light="Что" bold="происходит" size={28} />
        {stages.map((s) => (
          <StageCard key={s.key} s={s} />
        ))}
        <div className="flex-1" />
        {!p.ready && <p className="m-0 text-[13px] leading-[1.4] text-dim">Колода откроется, когда разбор закончится.</p>}
        <Button variant={p.ready ? "primary" : "secondary"} size={48} hotkey="Enter" disabled={!p.ready} onClick={onDone}>
          Раскладывать
        </Button>
        <Button variant="ghost" size={40} hotkey="Esc" onClick={onCancel} className="-mt-1 text-dim">
          Выбрать другую папку
        </Button>
      </aside>
    </main>
  );
}
