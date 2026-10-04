import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import { Titlebar } from "./components/Titlebar";
import { DeckScreen } from "./components/deck/DeckScreen";
import { ExplorerBar } from "./components/deck/ExplorerBar";
import { Develop } from "./components/screens/Develop";
import { JournalScreen } from "./components/screens/Journal";
import { PilesScreen } from "./components/screens/Piles";
import type { DupesSetting } from "./components/screens/DupesStep";
import { SettingsScreen } from "./components/screens/Settings";
import { Start } from "./components/screens/Start";
import { errorText, ipc, onOpenFolder, type AppState } from "./lib/ipc";

export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [started, setStarted] = useState(false);
  const [home, setHome] = useState(false);
  const [develop, setDevelop] = useState<"deck" | "dupes" | "search" | null>(null);
  const revert = useRef<DupesSetting | null>(null);
  const developView = develop !== null;
  const [screen, setScreen] = useState<"deck" | "piles" | "journal" | "settings">("deck");

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

  const [explorer, setExplorer] = useState<{ path: string; error?: string } | null>(null);

  const openFolder = useCallback(
    async (path: string) => {
      try {
        await ipc.chooseDeck(path);
        setExplorer({ path });
      } catch (e) {
        setExplorer({ path, error: errorText(e) });
      }
      setScreen("deck");
      await reload();
    },
    [reload],
  );

  useEffect(() => {
    void ipc.takeIncoming().then((path) => {
      if (path) void openFolder(path);
    });
    const off = onOpenFolder((path) => void openFolder(path));
    return () => void off.then((f) => f());
  }, [openFolder]);

  const pickTable = useCallback(async () => {
    if (!explorer) return;
    const path = await open({ directory: true, title: "Куда раскладывать?" });
    if (typeof path !== "string") return;
    try {
      await ipc.chooseTable(path);
      if (explorer.error) await ipc.chooseDeck(explorer.path);
      setExplorer({ path: explorer.path });
    } catch (e) {
      setExplorer({ ...explorer, error: errorText(e) });
    }
    await reload();
  }, [explorer, reload]);

  useEffect(() => {
    const theme = state?.settings.theme;
    if (theme === "dark" || theme === "light") document.documentElement.dataset.theme = theme;
    else delete document.documentElement.dataset.theme;
  }, [state?.settings.theme]);

  const lastDeck = useRef<string | undefined>(undefined);
  useEffect(() => {
    const deck = state?.settings.deck_path;
    if (!state) return;
    if (!deck) {
      lastDeck.current = undefined;
      return;
    }
    if (lastDeck.current !== deck) {
      setDevelop(state.developing.ready ? null : state.developing.search ? "search" : "deck");
      setScreen("deck");
    }
    lastDeck.current = deck;
  }, [state]);

  const back = useCallback(() => void reload().then(() => setScreen("deck")), [reload]);

  const widen = useCallback(
    (before: DupesSetting) => {
      revert.current = before;
      setDevelop("dupes");
      void reload().then(() => setScreen("deck"));
    },
    [reload],
  );

  const search = useCallback(() => {
    setDevelop("search");
    void reload();
  }, [reload]);

  const cancelDevelop = useCallback(async () => {
    if (develop === "search") await ipc.searchClear();
    else if (develop === "dupes" && revert.current) {
      await ipc.setSetting("dupes_scope", revert.current.scope);
      await ipc.setSetting("dupes_enabled", revert.current.enabled ? "1" : "0");
    } else await ipc.cancelDeck();
    revert.current = null;
    await reload();
    setDevelop(null);
  }, [develop, reload]);

  const ready = !!state?.settings.deck_path && !!state?.settings.table_path;
  const showStart = state && (!ready || home || state.missing.length > 0 || (!started && !state.settings.mode));

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
            setHome(false);
            await reload();
          }}
        />
      )}
      {state && !showStart && ready && develop && (
        <Develop
          state={state}
          purpose={develop}
          onDone={() => {
            revert.current = null;
            setDevelop(null);
            void reload();
          }}
          onCancel={() => void cancelDevelop()}
        />
      )}
      {state && !showStart && ready && !developView && screen === "piles" && (
        <PilesScreen table={state.settings.table_path ?? ""} onBack={back} />
      )}
      {state && !showStart && ready && !developView && screen === "journal" && (
        <JournalScreen cacheDir={state.cacheDir} deckPath={state.settings.deck_path ?? ""} onBack={back} />
      )}
      {state && !showStart && ready && !developView && screen === "settings" && (
        <SettingsScreen state={state} onChange={reload} onBack={back} onWiden={widen} />
      )}
      {state && !showStart && ready && !developView && screen === "deck" && (
        <DeckScreen
          key={state.settings.deck_path}
          initial={state}
          banner={
            explorer && (
              <ExplorerBar
                deck={explorer.path}
                table={state.settings.table_path ?? ""}
                error={explorer.error}
                onPickTable={() => void pickTable()}
                onClose={() => setExplorer(null)}
              />
            )
          }
          onPiles={() => setScreen("piles")}
          onJournal={() => setScreen("journal")}
          onSettings={() => setScreen("settings")}
          onHome={() => setHome(true)}
          onWiden={widen}
          onSearch={search}
        />
      )}
    </div>
  );
}
