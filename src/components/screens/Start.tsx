import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { errorText, ipc, type AppState } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { Button } from "../ui/Button";
import { Heading } from "../ui/PageHeader";
import { Toggle } from "../ui/Toggle";

const gb = (bytes: number) =>
  bytes >= 1024 ** 3 ? `${(bytes / 1024 ** 3).toFixed(1).replace(".", ",")} ГБ` : `${Math.round(bytes / 1024 ** 2)} МБ`;

function Step({
  n,
  done,
  title,
  value,
  note,
  error,
  action,
}: {
  n: number;
  done: boolean;
  title: string;
  value: string;
  note: string;
  error?: string | null;
  action: React.ReactNode;
}) {
  return (
    <div className={`flex items-center gap-4 rounded-[20px] px-5 py-[18px] ${done ? "bg-raised" : "border-[1.5px] border-dashed border-line"}`}>
      <span className={`grid h-8 w-8 shrink-0 place-items-center self-start rounded-full text-[13px] font-bold ${done ? "bg-fg text-ink" : "bg-strong"}`}>{n}</span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="text-[13px] font-medium text-dim">{title}</div>
        <div className={`truncate text-lg font-bold ${done ? "" : "text-dim"}`}>{value}</div>
        <div className={`text-[13px] ${error ? "text-danger" : "text-dim"}`}>{error ?? note}</div>
      </div>
      {action}
    </div>
  );
}

export function Start({ state, onChange, onStart }: { state: AppState; onChange: () => void; onStart: () => void }) {
  const [deckInfo, setDeckInfo] = useState<{ count: number; bytes: number } | null>(null);
  const [errors, setErrors] = useState<{ deck?: string; table?: string }>({});
  const deck = state.settings.deck_path;
  const table = state.settings.table_path;
  const hints = state.settings.hints_enabled !== "0";
  const piles = state.piles.filter((p) => !p.isTrash).length;
  const count = deckInfo?.count ?? state.deck.left;

  const pick = async (which: "deck" | "table") => {
    const path = await open({ directory: true, title: which === "deck" ? "Где лежат видео?" : "Куда раскладывать?" });
    if (typeof path !== "string") return;
    try {
      if (which === "deck") setDeckInfo(await ipc.chooseDeck(path));
      else await ipc.chooseTable(path);
      setErrors((e) => ({ ...e, [which]: undefined }));
      onChange();
    } catch (e) {
      setErrors((x) => ({ ...x, [which]: errorText(e) }));
    }
  };

  return (
    <main className="mx-2 mb-2 flex min-h-0 flex-1 gap-2">
      <section className="flex flex-1 flex-col gap-7 rounded-[28px] bg-cosmic px-16 py-14">
        <div className="flex flex-col gap-3">
          <Heading light="Разложим" bold="видео" size={56} />
          <p className="m-0 max-w-[560px] text-[15px] leading-[1.55] text-dim">
            Выбери, откуда брать видео и куда их раскладывать. Каждое видео станет картой, каждая папка — стопкой. Файлы
            переносятся, только когда ты сам кладёшь карту.
          </p>
        </div>
        <div className="flex max-w-[760px] flex-col gap-3">
          <Step
            n={1}
            done={!!deck}
            title="Откуда брать видео"
            value={deck ?? "Папка не выбрана"}
            note={deck ? `Колода — ${count} ${plural(count, "видео", "видео", "видео")}${deckInfo ? ` · ${gb(deckInfo.bytes)}` : ""}. Берутся только видео в самой папке, вложенные не трогаю.` : "Колода — папка с неразобранными видео."}
            error={errors.deck}
            action={
              <Button variant={deck ? "secondary" : "primary"} onClick={() => pick("deck")}>
                {deck ? "Другая папка" : "Выбрать папку"}
              </Button>
            }
          />
          <Step
            n={2}
            done={!!table}
            title="Куда раскладывать"
            value={table ?? "Папка не выбрана"}
            note={
              table
                ? piles
                  ? `Стол — ${piles} ${plural(piles, "стопка", "стопки", "стопок")} по папкам внутри`
                  : "Стол — папок внутри пока нет, стопки создашь по ходу."
                : "Стол — каждая папка внутри станет стопкой. Пустая — не беда, стопки создашь по ходу."
            }
            error={errors.table}
            action={
              <Button variant={table || !deck ? "secondary" : "primary"} onClick={() => pick("table")}>
                {table ? "Другая папка" : "Выбрать папку"}
              </Button>
            }
          />
          <div className="flex items-center gap-4 rounded-[20px] bg-raised px-5 py-[18px]">
            <span className="grid h-8 w-8 shrink-0 place-items-center self-start rounded-full bg-strong text-[13px] font-bold">3</span>
            <div className="flex flex-1 flex-col gap-1">
              <div className="text-[13px] font-medium text-dim">Подсказки</div>
              <div className="text-lg font-bold">Угадывать стопку по кадрам</div>
              <div className="text-[13px] text-dim">
                Работает только на этом компьютере, на видеокарте. Учится на картах, которые ты уже разложил.
              </div>
            </div>
            <Toggle
              label="Угадывать стопку по кадрам"
              checked={hints}
              onChange={async (v) => {
                await ipc.setSetting("hints_enabled", v ? "1" : "0");
                onChange();
              }}
            />
          </div>
        </div>
        <div className="flex-1" />
        <div className="flex items-center gap-3">
          <Button variant="primary" size={48} disabled={!deck || !table} onClick={onStart}>
            Проявить плёнку
          </Button>
          {(!deck || !table) && <span className="text-[13px] text-dim">Сначала выбери {deck ? "стол" : "колоду"}</span>}
        </div>
      </section>
      <aside className="flex w-[480px] shrink-0 flex-col items-center justify-center gap-9 rounded-[28px] bg-cosmic">
        <div className="relative h-[330px] w-[220px]">
          <div className="absolute inset-0 translate-x-[-22px] translate-y-2 -rotate-9 rounded-[28px] bg-line" />
          <div className="absolute inset-0 translate-x-[18px] translate-y-0.5 rotate-5 rounded-[28px] bg-strong" />
          <div className="absolute inset-0 flex flex-col justify-between rounded-[28px] border border-line bg-film p-5 text-[#ececef] shadow-[0_24px_60px_rgb(0_0_0/30%)]">
            <div className="text-[13px] font-medium text-[#a2a2a9]">колода</div>
            <div className="text-[88px] leading-[0.9] font-bold tracking-[-0.04em]">{deck ? count : "—"}</div>
            <div className="text-[13px] font-medium text-[#a2a2a9]">{deck ? plural(count, "карта", "карты", "карт") : "выбери папку"}</div>
          </div>
        </div>
        <p className="m-0 max-w-[300px] text-center text-[13px] leading-normal text-dim">
          Ничего не удаляется без спроса. Любой ход можно забрать обратно.
        </p>
      </aside>
    </main>
  );
}
