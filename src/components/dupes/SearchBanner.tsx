import { Search } from "lucide-react";
import type { SearchSummary } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { filesWord } from "../screens/DupesStep";
import { Button } from "../ui/Button";

export const searchWhat = (s: SearchSummary) =>
  [...s.piles, ...(s.deck ? ["колода"] : []), ...s.folders.filter((f) => !s.skipped.includes(f))].join(", ");

export function SearchBanner({ search, groups, onNew, onClose }: { search: SearchSummary; groups: number; onNew: () => void; onClose: () => void }) {
  return (
    <div className="flex items-center gap-3 rounded-[20px] bg-raised py-2.5 pr-2.5 pl-4 shadow-[inset_0_0_0_1.5px_var(--strong)]">
      <Search size={18} strokeWidth={1.5} className="shrink-0" />
      <div className="flex min-w-0 flex-1 flex-col gap-px">
        <b className="text-sm">
          Отдельный поиск · {groups} {plural(groups, "группа", "группы", "групп")}
        </b>
        <span className="truncate text-xs text-dim" title={searchWhat(search)}>
          {searchWhat(search)} · {filesWord(search.files)}
          {search.skipped.length > 0 && ` · пропущено: ${search.skipped.join(", ")}`}
        </span>
      </div>
      <Button hotkey="F" className="bg-strong! hover:bg-line!" onClick={onNew}>
        Новый поиск
      </Button>
      <Button hotkey="Esc" className="bg-strong! hover:bg-line!" onClick={onClose}>
        Закрыть результаты
      </Button>
    </div>
  );
}
