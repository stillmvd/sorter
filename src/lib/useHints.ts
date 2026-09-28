import { useEffect, useState } from "react";
import { ipc, onCard, onHintsChanged, type Hints } from "./ipc";

const EMPTY: Hints = { enabled: false, ready: false, examples: 0, hints: [] };

export function useHints(ids: number[], version: unknown): Hints {
  const key = ids.join(",");
  const [state, setState] = useState<{ key: string; hints: Hints }>({ key: "", hints: EMPTY });

  useEffect(() => {
    let alive = true;
    let timer = 0;
    const load = () => {
      if (!key) {
        setState({ key, hints: EMPTY });
        return;
      }
      void ipc.hints(key.split(",").map(Number)).then((h) => alive && setState({ key, hints: h }));
    };
    const soon = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(load, 300);
    };
    load();
    const wanted = new Set(key.split(",").map(Number));
    const offCard = onCard((c) => wanted.has(c.id) && soon());
    const offChanged = onHintsChanged(soon);
    return () => {
      alive = false;
      window.clearTimeout(timer);
      void offCard.then((f) => f());
      void offChanged.then((f) => f());
    };
  }, [key, version]);

  return state.key === key ? state.hints : { ...state.hints, ready: false, hints: [] };
}
