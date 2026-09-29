import { useEffect, useState } from "react";
import { ipc, onDupesChanged, type DupeView } from "./ipc";

export function useDupes(cardId: number | null, version: unknown): DupeView[] {
  const [state, setState] = useState<{ id: number | null; dupes: DupeView[] }>({ id: null, dupes: [] });

  useEffect(() => {
    let alive = true;
    const load = () => {
      if (cardId === null) return;
      void ipc
        .dupesFor(cardId)
        .then((d) => alive && setState({ id: cardId, dupes: d }))
        .catch(() => alive && setState({ id: cardId, dupes: [] }));
    };
    load();
    const off = onDupesChanged(load);
    return () => {
      alive = false;
      void off.then((f) => f());
    };
  }, [cardId, version]);

  return state.id === cardId ? state.dupes : [];
}
