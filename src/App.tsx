import { useCallback, useEffect, useState } from "react";
import { Titlebar } from "./components/Titlebar";
import { DeckScreen } from "./components/deck/DeckScreen";
import { Develop } from "./components/screens/Develop";
import { JournalScreen } from "./components/screens/Journal";
import { PilesScreen } from "./components/screens/Piles";
import { Start } from "./components/screens/Start";
import { errorText, ipc, type AppState } from "./lib/ipc";

export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [started, setStarted] = useState(false);
  const [developView, setDevelopView] = useState(false);
  const [screen, setScreen] = useState<"deck" | "piles" | "journal">("deck");

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

  const back = useCallback(() => void reload().then(() => setScreen("deck")), [reload]);

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
      {state && !showStart && ready && !developView && screen === "piles" && (
        <PilesScreen table={state.settings.table_path ?? ""} onBack={back} />
      )}
      {state && !showStart && ready && !developView && screen === "journal" && (
        <JournalScreen cacheDir={state.cacheDir} deckPath={state.settings.deck_path ?? ""} onBack={back} />
      )}
      {state && !showStart && ready && !developView && screen === "deck" && (
        <DeckScreen initial={state} onPiles={() => setScreen("piles")} onJournal={() => setScreen("journal")} />
      )}
    </div>
  );
}
