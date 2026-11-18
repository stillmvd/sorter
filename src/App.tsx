import { UpdateToast } from "@stillmvd/tauri-ship";
import { useCallback, useEffect, useRef, useState } from "react";
import { Titlebar } from "./components/Titlebar";
import { DeckScreen } from "./components/deck/DeckScreen";
import { Develop } from "./components/screens/Develop";
import { Home } from "./components/screens/Home";
import { JournalScreen } from "./components/screens/Journal";
import { PilesScreen } from "./components/screens/Piles";
import type { DupesSetting } from "./components/screens/DupesStep";
import { SettingsScreen } from "./components/screens/Settings";
import { errorText, ipc, onOpenFolder, type AppState } from "./lib/ipc";

type Screen = "deck" | "piles" | "journal" | "settings";

export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [view, setView] = useState<"home" | "review">("home");
  const [develop, setDevelop] = useState<"deck" | "dupes" | "search" | null>(null);
  const [screen, setScreen] = useState<Screen>("deck");
  const [incoming, setIncoming] = useState<string[] | null>(null);
  const revert = useRef<DupesSetting | null>(null);

  const reload = useCallback(async () => {
    try {
      const s = await ipc.getState();
      setState(s);
      return s;
    } catch (e) {
      setFailure(errorText(e));
      return null;
    }
  }, []);

  useEffect(() => {
    void ipc
      .closeReview()
      .catch(() => undefined)
      .then(reload);
  }, [reload]);

  const goHome = useCallback(async () => {
    await ipc.closeReview().catch(() => undefined);
    revert.current = null;
    setDevelop(null);
    setScreen("deck");
    setView("home");
    await reload();
  }, [reload]);

  const openFolder = useCallback(
    async (path: string) => {
      if (view === "review") await goHome();
      else setScreen("deck");
      setIncoming([path]);
    },
    [view, goHome],
  );

  useEffect(() => {
    void ipc.takeIncoming().then((path) => {
      if (path) void openFolder(path);
    });
    const off = onOpenFolder((path) => void openFolder(path));
    return () => void off.then((f) => f());
  }, [openFolder]);

  useEffect(() => {
    const theme = state?.settings.theme;
    if (theme === "dark" || theme === "light") document.documentElement.dataset.theme = theme;
    else delete document.documentElement.dataset.theme;
  }, [state?.settings.theme]);

  const opened = useCallback(async () => {
    const s = await reload();
    if (!s) return;
    setIncoming(null);
    setDevelop(s.developing.ready ? null : s.developing.search ? "search" : "deck");
    setScreen("deck");
    setView("review");
  }, [reload]);

  useEffect(() => {
    if (view === "review" && state?.missing.length) void goHome();
  }, [view, state?.missing.length, goHome]);

  const refresh = useCallback(() => void reload(), [reload]);
  const toJournal = useCallback(() => setScreen("journal"), []);
  const toSettings = useCallback(() => setScreen("settings"), []);

  const back = useCallback(() => void reload().then(() => setScreen("deck")), [reload]);

  const widen = useCallback(
    (before: DupesSetting) => {
      revert.current = before;
      void reload().then(() => {
        if (view === "review") setDevelop("dupes");
        setScreen("deck");
      });
    },
    [reload, view],
  );

  const search = useCallback(() => {
    setDevelop("search");
    void reload();
  }, [reload]);

  const cancelDevelop = useCallback(async () => {
    if (develop === "deck") return goHome();
    if (develop === "search") await ipc.searchClear();
    else if (develop === "dupes" && revert.current) {
      await ipc.setSetting("dupes_scope", revert.current.scope);
      await ipc.setSetting("dupes_enabled", revert.current.enabled ? "1" : "0");
    }
    revert.current = null;
    await reload();
    setDevelop(null);
  }, [develop, reload, goHome]);

  const review = view === "review" && !!state?.review && !!state.settings.table_path;

  return (
    <div className="flex h-full flex-col">
      <Titlebar path={review ? (state?.review ?? undefined) : undefined} />
      {failure && <p className="m-auto max-w-[60ch] text-center text-[15px] text-dim">{failure}</p>}
      {state && screen === "journal" && <JournalScreen cacheDir={state.cacheDir} onBack={back} />}
      {state && screen === "settings" && <SettingsScreen state={state} onChange={async () => void (await reload())} onBack={back} onWiden={widen} />}
      {state && screen === "deck" && !review && (
        <Home
          state={state}
          incoming={incoming}
          onOpened={opened}
          onJournal={toJournal}
          onSettings={toSettings}
          onChange={refresh}
        />
      )}
      {state && review && develop && screen === "deck" && (
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
      {state && review && !develop && screen === "piles" && <PilesScreen table={state.settings.table_path ?? ""} onBack={back} />}
      {state && review && !develop && screen === "deck" && (
        <DeckScreen
          key={`${state.settings.review_id}|${state.settings.table_path}`}
          initial={state}
          onPiles={() => setScreen("piles")}
          onJournal={() => setScreen("journal")}
          onSettings={() => setScreen("settings")}
          onHome={() => void goHome()}
          onWiden={widen}
          onSearch={search}
        />
      )}
      <UpdateToast lang="ru" />
    </div>
  );
}
