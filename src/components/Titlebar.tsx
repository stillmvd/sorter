import { UpdateBadge } from "@stillmvd/tauri-ship";
import { Mark } from "./ui/Mark";
import { WindowControls } from "./WindowControls";

export function Titlebar({ path }: { path?: string }) {
  return (
    <header data-tauri-drag-region className="flex h-12 shrink-0 items-center pr-2 pl-5">
      <div data-tauri-drag-region className="flex items-center gap-2.5 select-none [&>*]:pointer-events-none">
        <Mark size={16} />
        <span className="text-sm font-bold">Sorter</span>
        {path && <span className="text-[13px] font-medium text-dim">{path}</span>}
      </div>
      <div data-tauri-drag-region className="min-w-12 flex-1 self-stretch" />
      <span className="mr-2 flex empty:hidden">
        <UpdateBadge lang="ru" />
      </span>
      <WindowControls />
    </header>
  );
}
