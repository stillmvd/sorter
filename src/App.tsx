import { useCallback, useEffect, useState } from "react";
import { Titlebar } from "./components/Titlebar";
import { DeckScreen } from "./components/deck/DeckScreen";
import { Develop } from "./components/screens/Develop";
import { Start } from "./components/screens/Start";
import { errorText, ipc, type AppState } from "./lib/ipc";

export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [started, setStarted] = useState(false);
  const [developView, setDevelopView] = useState(false);

  const reload = useCallback(async () => {
    try {
      setState(await ipc.getState());
    } catch (e) {
      setFailure(errorText(e));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    const theme = state?.settings.theme;
    if (theme === "dark" || theme === "light") document.documentElement.dataset.theme = theme;
    else delete document.documentElement.dataset.theme;
  }, [state?.settings.theme]);

  const ready = !!state?.settings.deck_path && !!state?.settings.table_path;
  const showStart = state && (!ready || (!started && !state.settings.mode));

  return (
    <div className="flex h-full flex-col">
      <Titlebar path={ready ? state?.settings.deck_path : undefined} />
      {failure && <p className="m-auto max-w-[60ch] text-center text-[15px] text-dim">{failure}</p>}
      {state && showStart && (
        <Start
          state={state}
          onChange={reload}
          onStart={async () => {
            await ipc.setSetting("mode", "deck");
            setStarted(true);
            setDevelopView(true);
            await reload();
          }}
        />
      )}
      {state && !showStart && ready && developView && <Develop state={state} onDone={() => setDevelopView(false)} />}
      {state && !showStart && ready && !developView && <DeckScreen initial={state} onReload={reload} />}
    </div>
  );
}
