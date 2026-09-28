import { Film } from "lucide-react";
import { useEffect, useState } from "react";
import { frameSrc, ipc, onCard, onProgress, type AppState, type Card } from "../../lib/ipc";
import { FRAMES } from "../deck/FilmStrip";
import { Button } from "../ui/Button";
import { Heading, PageHeader, Tag } from "../ui/PageHeader";

function Strip({ card, cacheDir, no }: { card: Card; cacheDir: string; no: number }) {
  const ready = card.frames > 0;
  const state = card.stage === "broken" ? "не проявилось" : ready ? "готово" : "ждёт";
  return (
    <div className="flex flex-col gap-1 px-1.5 py-1">
      <div className={`flex gap-2.5 font-mono text-[10px] tracking-[0.06em] ${ready ? "text-[#8a8a92]" : "text-[#4a4a50]"}`}>
        <span>{String(no).padStart(3, "0")}</span>
        <span className="truncate">{card.fileName}</span>
        <span className="flex-1" />
        <span>{state}</span>
      </div>
      <div className="flex gap-1">
        {Array.from({ length: FRAMES }, (_, i) => (
          <div key={i} className={`h-11 min-w-0 flex-1 overflow-hidden rounded-[3px] ${ready && i < card.frames ? "bg-[#2b2b30]" : "border border-dashed border-[#2a2a2f]"}`}>
            {ready && i < card.frames && <img src={frameSrc(cacheDir, card, i)} alt="" className="h-full w-full object-cover" />}
          </div>
        ))}
      </div>
    </div>
  );
}

export function Develop({ state, onDone }: { state: AppState; onDone: () => void }) {
  const [progress, setProgress] = useState(state.developing);
  const [cards, setCards] = useState<Card[]>([]);

  useEffect(() => {
    void ipc.deckWindow(0, 14).then(setCards);
    const a = onCard((card) => setCards((cs) => cs.map((c) => (c.id === card.id ? card : c))));
    const b = onProgress(setProgress);
    return () => {
      void a.then((f) => f());
      void b.then((f) => f());
    };
  }, []);

  const pct = progress.total ? Math.round((progress.done / progress.total) * 100) : 0;
  const finished = progress.total > 0 && progress.done >= progress.total;
  const jobs = [
    { name: "Раскадровки", val: `${progress.done} / ${progress.total}`, pct, hint: "По 8 кадров с каждого видео. Хранятся у Sorter, папку с видео не трогаю." },
    { name: "Сведения о файлах", val: `${progress.done} / ${progress.total}`, pct, hint: "Длина, размер кадра и ориентация — чтобы карта сразу встала как надо." },
    { name: "Подсказки", val: "позже", pct: 0, hint: "Начнут угадывать стопку, когда ты разложишь первые карты." },
  ];

  return (
    <main className="mx-2 mb-2 flex min-h-0 flex-1 gap-2">
      <section className="flex min-w-0 flex-1 flex-col gap-5 rounded-[28px] bg-cosmic px-10 py-7">
        <PageHeader
          tag={<Tag icon={<Film strokeWidth={1.5} />}>Проявка плёнки</Tag>}
          light={finished ? "Проявлено" : "Проявляю"}
          bold={`${progress.done} из ${progress.total}`}
        />
        <div className="h-2 rounded-full bg-raised">
          <div className="h-2 rounded-full bg-fg transition-[width] duration-300 ease-trail" style={{ width: `${pct}%` }} />
        </div>
        <div className="grid min-h-0 flex-1 grid-cols-2 content-start gap-x-4 gap-y-2 overflow-hidden rounded-[20px] bg-film px-4 py-3.5">
          {cards.map((c, i) => (
            <Strip key={c.id} card={c} cacheDir={state.cacheDir} no={state.deck.placed + i + 1} />
          ))}
        </div>
      </section>
      <aside className="flex w-[420px] shrink-0 flex-col gap-3.5 rounded-[28px] bg-cosmic p-7">
        <div className="mb-1.5">
          <Heading light="Что" bold="происходит" size={28} />
        </div>
        {jobs.map((j) => (
          <div key={j.name} className="flex flex-col gap-2 rounded-[20px] bg-raised px-[18px] py-4">
            <div className="flex items-baseline gap-2">
              <span className="flex-1 text-[15px] font-bold">{j.name}</span>
              <span className="text-[13px] font-bold">{j.val}</span>
            </div>
            <div className="h-1.5 rounded-full bg-strong">
              <div className="h-1.5 rounded-full bg-fg" style={{ width: `${j.pct}%` }} />
            </div>
            <div className="text-[13px] leading-normal text-dim">{j.hint}</div>
          </div>
        ))}
        <div className="flex-1" />
        <p className="m-0 text-[13px] leading-normal text-dim">
          Можно не ждать: проявленные карты уже в колоде, остальные догонят.
        </p>
        <Button variant="primary" size={48} onClick={onDone}>
          Начать раскладывать
        </Button>
        <Button
          onClick={() => {
            void ipc.developControl(!progress.paused);
            setProgress((p) => ({ ...p, paused: !p.paused }));
          }}
        >
          {progress.paused ? "Продолжить проявку" : "Пауза"}
        </Button>
      </aside>
    </main>
  );
}
