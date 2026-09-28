import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";

const appWindow = getCurrentWindow();

const btn =
  "grid h-8 w-8 place-items-center rounded-full text-dim transition-colors duration-200 ease-trail hover:bg-raised hover:text-fg focus-visible:outline-offset-[-2px]";

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    appWindow.isMaximized().then(setMaximized);
    const pending = appWindow.onResized(() => {
      appWindow.isMaximized().then(setMaximized);
    });
    return () => {
      void pending.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div className="flex items-center gap-1">
      <button type="button" aria-label="Свернуть" onClick={() => appWindow.minimize()} className={btn}>
        <Minus className="h-4 w-4" strokeWidth={1.25} />
      </button>
      <button
        type="button"
        aria-label={maximized ? "Восстановить" : "Развернуть"}
        onClick={() => appWindow.toggleMaximize()}
        className={btn}
      >
        {maximized ? (
          <Copy className="h-3.5 w-3.5" strokeWidth={1.25} />
        ) : (
          <Square className="h-3.5 w-3.5" strokeWidth={1.25} />
        )}
      </button>
      <button
        type="button"
        aria-label="Закрыть"
        onClick={() => appWindow.close()}
        className={`${btn} hover:bg-[color-mix(in_oklab,var(--danger)_15%,transparent)] hover:text-danger`}
      >
        <X className="h-4 w-4" strokeWidth={1.25} />
      </button>
    </div>
  );
}
