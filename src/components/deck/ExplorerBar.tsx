import { Folder, X } from "lucide-react";
import { useEffect } from "react";
import { Button } from "../ui/Button";

export function ExplorerBar({
  deck,
  table,
  error,
  onPickTable,
  onClose,
}: {
  deck: string;
  table: string;
  error?: string;
  onPickTable: () => void;
  onClose: () => void;
}) {
  useEffect(() => {
    if (error) return;
    const listen = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || document.activeElement?.tagName === "INPUT") return;
      onClose();
    };
    window.addEventListener("keydown", listen);
    return () => window.removeEventListener("keydown", listen);
  }, [error, onClose]);

  return (
    <div
      className={`flex items-center gap-4 rounded-[20px] bg-raised py-3 pr-3 pl-[18px] ${error ? "outline-[1.5px] -outline-offset-[1.5px] outline-fg outline-solid" : ""}`}
    >
      <Folder size={18} strokeWidth={1.5} className="shrink-0" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="truncate text-[13px] font-medium text-dim">
          Открыто из Проводника · {error ? deck : `колода ${deck}`}
        </div>
        <div className={`truncate text-[15px] ${error ? "font-medium" : "font-bold"}`}>{error ?? `Раскладываю в ${table}`}</div>
      </div>
      {error ? (
        <Button variant="primary" onClick={onPickTable}>
          Выбрать другой стол
        </Button>
      ) : (
        <>
          <Button className="bg-strong! hover:bg-line!" onClick={onPickTable}>
            Сменить стол
          </Button>
          <button
            type="button"
            aria-label="Скрыть"
            title="Скрыть · Esc"
            onClick={onClose}
            className="grid h-10 w-10 shrink-0 place-items-center rounded-full text-dim transition-[color,background-color,scale] hover:bg-strong hover:text-fg"
          >
            <X size={16} strokeWidth={1.25} />
          </button>
        </>
      )}
    </div>
  );
}
