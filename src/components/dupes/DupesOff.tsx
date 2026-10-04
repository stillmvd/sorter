import { CopyX, Search } from "lucide-react";
import { Button } from "../ui/Button";

export function DupesOff({ onEnable, onFind }: { onEnable: () => void; onFind: () => void }) {
  return (
    <div className="grid min-h-0 flex-1 place-items-center">
      <div className="flex w-[540px] max-w-full flex-col items-center gap-3.5 rounded-3xl bg-raised px-8 py-[30px] text-center">
        <span className="grid h-[52px] w-[52px] place-items-center rounded-full bg-strong">
          <CopyX size={24} strokeWidth={1.5} />
        </span>
        <h2 className="text-[28px] leading-[1.1] tracking-[-0.02em]">
          <span className="font-light">Поиск дублей</span> <span className="font-bold">выключен</span>
        </h2>
        <p className="m-0 text-sm leading-normal text-dim">
          Поэтому разобрать дубли не получится — файлы колоды ни с чем не сравнивались. Включи поиск: колода разберётся ещё раз,
          только этап «Дубли». Раскладывать можно будет, когда он закончится.
        </p>
        <div className="mt-1 flex flex-wrap justify-center gap-2.5">
          <Button variant="primary" size={48} hotkey="Enter" onClick={onEnable}>
            Включить поиск
          </Button>
          <Button size={48} hotkey="F" icon={<Search size={16} strokeWidth={1.5} />} className="bg-strong! hover:bg-line!" onClick={onFind}>
            Найти дубли…
          </Button>
        </div>
        <p className="m-0 text-[13px] leading-normal text-dim">
          «Найти дубли…» — отдельный поиск среди стопок и любых папок, переключатель он не трогает.
        </p>
      </div>
    </div>
  );
}
