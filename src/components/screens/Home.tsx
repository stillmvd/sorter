import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, FolderPlus, History, RefreshCw, Settings, Undo2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  errorText,
  frameSrc,
  ipc,
  media,
  onReading,
  onRoots,
  type AppState,
  type Card,
  type Draft,
  type Home as HomeData,
  type Merged,
  type ReviewView,
  type RootView,
} from "../../lib/ipc";
import { nf, plural } from "../../lib/plural";
import { Button } from "../ui/Button";
import { IconButton } from "../ui/IconButton";
import { Heading } from "../ui/PageHeader";
import { Toggle } from "../ui/Toggle";

type Zone = "src" | "dst" | `r:${number}`;

const drive = (path: string) => path.slice(0, 2);
const goneText = (path: string) => `Папки нет — подключи диск ${drive(path)}`;
const cards = (n: number) => `${nf(n)} ${plural(n, "карта", "карты", "карт")}`;
const folders = (n: number) => `${n} ${plural(n, "папку", "папки", "папок")}`;
const movable = (r: ReviewView) => !!r.gone && r.roots.some((x) => x.path.toLowerCase() === r.gone?.toLowerCase());

function counts(r: RootView) {
  const parts: string[] = [];
  if (r.dirs > 1) parts.push(`${nf(r.dirs)} ${plural(r.dirs, "папка", "папки", "папок")}`);
  if (r.video) parts.push(`${nf(r.video)} видео`);
  if (r.photo) parts.push(`${nf(r.photo)} фото`);
  return parts.join(" · ");
}

function day(at: number) {
  const d = new Date(at);
  const now = new Date();
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  if (at >= start) return "сегодня";
  if (at >= start - 86_400_000) return "вчера";
  return d.toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: d.getFullYear() === now.getFullYear() ? undefined : "numeric" });
}

const pct = (r: ReviewView) => (r.total ? Math.round(((r.total - r.left) / r.total) * 100) : 0);

function Bar({ r }: { r: ReviewView }) {
  return (
    <span className="hm-bar">
      <i style={{ width: `${pct(r)}%` }} />
    </span>
  );
}

function Remove({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      className="hm-x"
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
    >
      <X size={16} strokeWidth={1.5} />
    </button>
  );
}

function Again({ onClick }: { onClick: () => void }) {
  return (
    <Button
      size={36}
      className="bg-strong! hover:bg-line!"
      icon={<RefreshCw size={15} strokeWidth={1.5} />}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
    >
      Проверить снова
    </Button>
  );
}

function Thumb({ cacheDir, card }: { cacheDir: string; card: Card | null }) {
  const [broken, setBroken] = useState(false);
  const src = !card
    ? null
    : card.frames > 0
      ? frameSrc(cacheDir, card, card.kind === "photo" ? 0 : Math.floor(card.frames / 2))
      : card.kind === "photo"
        ? media(card.path)
        : null;
  return (
    <span className="thumb">
      <span className="b" />
      <span>{src && !broken && <img src={src} alt="" draggable={false} onError={() => setBroken(true)} />}</span>
    </span>
  );
}

function Name({ r }: { r: ReviewView }) {
  return (
    <span className="hm-nm">
      <Folder size={16} strokeWidth={1.5} />
      <b>{r.label}</b>
      <span className="t">→ {r.table}</span>
    </span>
  );
}

function RootRow({ root, found, reading, onRemove, onStop, onRecheck }: { root: RootView; found?: number; reading: boolean; onRemove: () => void; onStop: () => void; onRecheck: () => void }) {
  const n = Math.max(found ?? 0, root.reading ?? 0);
  return (
    <div className={`hm-row${root.online ? "" : " gone"}`}>
      <Folder size={18} strokeWidth={1.5} />
      <span className="tx">
        <span className="l1">
          <b>{root.name}</b>
          <span>{root.path}</span>
        </span>
        {!root.online ? (
          <span className="l2 err hm-err">{goneText(root.path)}</span>
        ) : reading ? (
          <>
            <span className="l2">
              Читаю… нашёл {nf(n)} {plural(n, "файл", "файла", "файлов")}
            </span>
            <span className="reading-bar" aria-hidden="true" />
          </>
        ) : (
          <span className="l2">{counts(root)}</span>
        )}
      </span>
      {!root.online && <Again onClick={onRecheck} />}
      {reading ? (
        <Button size={36} className="bg-strong! hover:bg-line!" hotkey="Esc" onClick={onStop}>
          Остановить
        </Button>
      ) : (
        <Remove label="Убрать папку" onClick={onRemove} />
      )}
    </div>
  );
}

export function Home({
  state,
  onOpened,
  onJournal,
  onSettings,
  onChange,
  incoming,
}: {
  state: AppState;
  onOpened: () => void;
  onJournal: () => void;
  onSettings: () => void;
  onChange: () => void;
  incoming: string[] | null;
}) {
  const [data, setData] = useState<HomeData | null>(null);
  const [errors, setErrors] = useState<Draft["errors"]>([]);
  const [failure, setFailure] = useState<string | null>(null);
  const [found, setFound] = useState<Record<string, number>>({});
  const [busy, setBusy] = useState(false);
  const [over, setOver] = useState<{ zone: Zone; count: number } | null>(null);
  const [dropped, setDropped] = useState<{ r: ReviewView; at: number } | null>(null);
  const [merged, setMerged] = useState<Merged[]>([]);
  const [tidying, setTidying] = useState<number | null>(null);
  const dragCount = useRef(0);
  const known = useRef(new Set<string>());
  const goneReviews = useRef<ReviewView[]>([]);

  const load = useCallback(async () => {
    try {
      const h = await ipc.home();
      known.current = new Set(h.draft.roots.map((r) => r.path.toLowerCase()));
      setData(h);
    } catch (e) {
      setFailure(errorText(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const offReading = onReading(({ path, found }) => {
      setFound((f) => ({ ...f, [path]: found }));
      if (!known.current.has(path.toLowerCase())) {
        known.current.add(path.toLowerCase());
        void load();
      }
    });
    const offRoots = onRoots(() => void load());
    return () => {
      void offReading.then((f) => f());
      void offRoots.then((f) => f());
    };
  }, [load]);

  const apply = useCallback(
    (d: Draft) => {
      setErrors(d.errors);
      setDropped(null);
      if (d.merged.length) setMerged((m) => [...m, ...d.merged]);
      setData((h) => (h ? { ...h, draft: d } : h));
      known.current = new Set(d.roots.map((r) => r.path.toLowerCase()));
      onChange();
    },
    [onChange],
  );

  const run = useCallback(
    async (job: () => Promise<Draft>) => {
      setFailure(null);
      try {
        apply(await job());
      } catch (e) {
        setFailure(errorText(e));
      }
    },
    [apply],
  );

  const queue = useRef<Promise<void>>(Promise.resolve());
  const pending = useRef(0);
  const addRoots = useCallback(
    (paths: string[]) => {
      if (!paths.length) return queue.current;
      if (pending.current === 0) setMerged([]);
      pending.current += 1;
      setBusy(true);
      queue.current = queue.current.then(async () => {
        await run(() => ipc.addRoots(paths));
        pending.current -= 1;
        if (pending.current === 0) {
          setFound({});
          setBusy(false);
          void load();
        }
      });
      return queue.current;
    },
    [run, load],
  );

  const setTable = useCallback((path: string) => run(() => ipc.setDraftTable(path)), [run]);

  const relocate = useCallback(async (r: ReviewView, path: string) => {
    if (!r.gone) return;
    setFailure(null);
    try {
      setData(await ipc.relocateRoot(r.id, r.gone, path));
    } catch (e) {
      setFailure(errorText(e));
    }
  }, []);

  const pickMoved = async (r: ReviewView) => {
    const path = await open({ directory: true, title: `Где теперь папка «${r.label}»?` });
    if (typeof path === "string") await relocate(r, path);
  };

  const taken = useRef<string[] | null>(null);
  useEffect(() => {
    if (!incoming?.length || taken.current === incoming) return;
    taken.current = incoming;
    void addRoots(incoming);
  }, [incoming, addRoots]);

  const pick = async (zone: Zone) => {
    const path = await open({ directory: true, multiple: zone === "src", title: zone === "src" ? "Что разобрать?" : "Куда раскладывать?" });
    if (!path) return;
    const list = Array.isArray(path) ? path : [path];
    if (zone === "src") await addRoots(list);
    else if (list[0]) await setTable(list[0]);
  };

  useEffect(() => {
    const zoneAt = (x: number, y: number): Zone | null => {
      const r = window.devicePixelRatio || 1;
      const el = document.elementFromPoint(x / r, y / r)?.closest<HTMLElement>("[data-drop]");
      return (el?.dataset.drop as Zone | undefined) ?? null;
    };
    const off = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter") {
        dragCount.current = payload.paths.length;
        const zone = zoneAt(payload.position.x, payload.position.y);
        setOver(zone ? { zone, count: dragCount.current } : null);
      } else if (payload.type === "over") {
        const zone = zoneAt(payload.position.x, payload.position.y);
        setOver((o) => (zone ? (o?.zone === zone ? o : { zone, count: dragCount.current }) : null));
      } else if (payload.type === "leave") {
        setOver(null);
      } else {
        setOver(null);
        const zone = zoneAt(payload.position.x, payload.position.y);
        const path = payload.paths[0];
        if (zone === "src") void addRoots(payload.paths);
        else if (zone === "dst" && path) void setTable(path);
        else if (zone?.startsWith("r:") && path) {
          const r = goneReviews.current.find((x) => `r:${x.id}` === zone);
          if (r) void relocate(r, path);
        }
      }
    });
    return () => void off.then((f) => f());
  }, [addRoots, setTable, relocate]);

  const draft = data?.draft;
  const roots = draft?.roots ?? [];
  const table = draft?.table ?? null;
  const reviews = useMemo(() => {
    const into = new Set(merged.map((m) => m.reviewId));
    return (data?.reviews ?? []).filter((r) => r.id !== dropped?.r.id).sort((a, b) => Number(into.has(b.id)) - Number(into.has(a.id)));
  }, [data, merged, dropped]);
  const tidy = data?.tidy ?? [];
  goneReviews.current = reviews.filter(movable);
  const reading = busy || roots.some((r) => r.reading !== null);
  const gone = roots.find((r) => !r.online) ?? (table && !table.online ? { path: table.path } : undefined);
  const ready = roots.length > 0 && !!table && !reading && !gone;
  const resumeOnEnter = reviews.length > 0 && !roots.length;
  const total = roots.filter((r) => r.online).reduce((n, r) => n + r.video + r.photo, 0);
  const foundAll = roots.reduce((n, r) => n + Math.max(found[r.path] ?? 0, r.reading ?? 0), 0);

  const start = useCallback(async () => {
    setFailure(null);
    try {
      await ipc.startReview();
      onOpened();
    } catch (e) {
      setFailure(errorText(e));
    }
  }, [onOpened]);

  const resume = useCallback(
    async (r: ReviewView) => {
      setFailure(null);
      try {
        await ipc.openReview(r.id);
        onOpened();
      } catch (e) {
        setFailure(errorText(e));
        void load();
      }
    },
    [onOpened, load],
  );

  const drop = useCallback(
    async (r: ReviewView, at: number) => {
      setFailure(null);
      try {
        await ipc.dropReview(r.id);
        setDropped({ r, at });
      } catch (e) {
        setFailure(errorText(e));
      }
      void load();
    },
    [load],
  );

  const restore = useCallback(async () => {
    if (!dropped) return;
    try {
      await ipc.restoreReview(dropped.r.id);
      await load();
    } catch (e) {
      setFailure(errorText(e));
      void load();
    }
    setDropped(null);
  }, [dropped, load]);

  const tidyUp = useCallback(
    async (r: ReviewView, keep: boolean) => {
      setFailure(null);
      setTidying(r.id);
      try {
        await (keep ? ipc.keepDirs(r.id) : ipc.tidyReview(r.id));
      } catch (e) {
        setFailure(errorText(e));
      }
      setTidying(null);
      void load();
    },
    [load],
  );

  const recheck = useCallback(async () => {
    try {
      setData(await ipc.recheck());
    } catch (e) {
      setFailure(errorText(e));
    }
  }, []);

  useEffect(() => {
    if (reading || !(gone || reviews.some((r) => r.gone))) return;
    const onFocus = () => void recheck();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [reading, gone, reviews, recheck]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      if (e.key === "Escape" && reading) {
        e.preventDefault();
        void ipc.stopReading();
      } else if (e.key === "Enter" && !e.repeat) {
        if (ready) {
          e.preventDefault();
          void start();
        } else if (resumeOnEnter && reviews[0]?.online) {
          e.preventDefault();
          void resume(reviews[0]);
        }
      } else if (e.ctrlKey && e.code === "KeyZ" && dropped) {
        e.preventDefault();
        void restore();
      } else if (e.ctrlKey && e.code === "KeyJ") {
        e.preventDefault();
        onJournal();
      } else if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        onSettings();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [reading, ready, resumeOnEnter, reviews, start, resume, dropped, restore, onJournal, onSettings]);

  let note = "";
  if (gone) note = `Подключи диск ${drive(gone.path)} или убери папку`;
  else if (!table) note = "Сначала выбери стол";

  const hints = state.settings.hints_enabled === "1";
  const dupes = state.settings.dupes_enabled === "1";
  const setOpt = async (key: "hints_enabled" | "dupes_enabled", on: boolean) => {
    await ipc.setSetting(key, on ? "1" : "0");
    onChange();
  };

  const srcOver = over?.zone === "src";
  const dstOver = over?.zone === "dst";
  const first = reviews[0]?.online ? reviews[0] : null;
  const rest: { r: ReviewView; undo: boolean }[] = reviews.filter((r) => r !== first).map((r) => ({ r, undo: false }));
  if (dropped) rest.splice(Math.min(rest.length, Math.max(0, dropped.at - (first ? 1 : 0))), 0, { r: dropped.r, undo: true });
  const into = first ? merged.filter((m) => m.reviewId === first.id) : [];
  const placed = first ? first.total - first.left : 0;
  const mergedNote = into.length
    ? `Добавил ${into.map((m) => m.name).join(", ")} к начатому разбору ${first?.label}${placed ? ` · ${cards(placed)} уже ${plural(placed, "разложена", "разложены", "разложено")}` : ""}`
    : "";

  return (
    <main className="mx-2 mb-2 flex min-h-0 flex-1">
      <section className="flex min-h-0 flex-1 flex-col gap-[22px] overflow-y-auto rounded-[28px] bg-cosmic px-12 py-8 [--scroll-inset:28px]">
        <div className="flex shrink-0 items-center gap-3">
          <div className="flex-1">
            <Heading light="Разложим" bold="видео и фото" size={42} />
          </div>
          <IconButton label="Журнал" hint="Журнал ходов — Ctrl J" onClick={onJournal}>
            <History size={18} strokeWidth={1.5} />
          </IconButton>
          <IconButton label="Настройки" hint="Настройки — Ctrl ," onClick={onSettings}>
            <Settings size={18} strokeWidth={1.5} />
          </IconButton>
        </div>

        <div className="hm-tiles">
          <div data-drop="src" className={`hm-tile${roots.length ? "" : " empty"}${srcOver ? " over" : ""}`}>
            <div className="hm-th">
              <span className="hm-lbl">Папка для разбора</span>
              {reading ? <b>нашёл {nf(foundAll)}</b> : roots.length > 0 && <b>{cards(total)}</b>}
            </div>
            {!roots.length || srcOver ? (
              <div className="hm-void">
                <FolderPlus size={28} strokeWidth={1.5} />
                {srcOver ? (
                  <>
                    <b>Отпусти — добавлю {folders(over.count)}</b>
                    <span className="text-[13px] text-dim">со всеми вложенными</span>
                  </>
                ) : (
                  <>
                    <b>Перетащи папки из Проводника</b>
                    <Button size={36} variant="primary" onClick={() => void pick("src")}>
                      Выбрать папку
                    </Button>
                  </>
                )}
              </div>
            ) : (
              <>
                <div>
                  {roots.map((r) => (
                    <RootRow
                      key={r.path}
                      root={r}
                      found={found[r.path]}
                      reading={r.reading !== null || (busy && found[r.path] !== undefined)}
                      onRemove={() => void run(() => ipc.removeRoot(r.path))}
                      onStop={() => void ipc.stopReading()}
                      onRecheck={() => void recheck()}
                    />
                  ))}
                </div>
                <div className="hm-tf">
                  <Button size={36} className="bg-strong! hover:bg-line!" icon={<FolderPlus size={15} strokeWidth={1.5} />} disabled={busy} onClick={() => void pick("src")}>
                    Добавить папку
                  </Button>
                  <span className="text-[13px] text-dim">или перетащи из Проводника</span>
                  <div className="hm-go">
                    {note && <span className="text-[13px] text-dim">{note}</span>}
                    <Button size={40} variant="primary" hotkey={ready ? "Enter" : undefined} disabled={!ready} onClick={() => void start()}>
                      Разобрать
                    </Button>
                  </div>
                </div>
              </>
            )}
          </div>

          <div data-drop="dst" className={`hm-tile${table ? "" : " empty"}${dstOver ? " over" : ""}`}>
            <div className="hm-th">
              <span className="hm-lbl">Куда раскладывать</span>
            </div>
            {!table ? (
              <div className="hm-void">
                <Folder size={28} strokeWidth={1.5} />
                <b>Перетащи папку стола</b>
                <Button size={36} onClick={() => void pick("dst")}>
                  Выбрать папку
                </Button>
              </div>
            ) : (
              <div className={`hm-row${table.online ? "" : " gone"}`}>
                <Folder size={18} strokeWidth={1.5} />
                <span className="tx">
                  <span className="l1">
                    <b>{table.name}</b>
                    <span>{table.path}</span>
                  </span>
                  {table.online ? (
                    <span className="l2">{table.piles ? `${table.piles} ${plural(table.piles, "стопка", "стопки", "стопок")}` : "Стопок пока нет — создашь по ходу"}</span>
                  ) : (
                    <span className="l2 err hm-err">{goneText(table.path)}</span>
                  )}
                </span>
                {table.online ? (
                  <Button size={36} className="bg-strong! hover:bg-line!" onClick={() => void pick("dst")}>
                    Другая папка
                  </Button>
                ) : (
                  <Again onClick={() => void recheck()} />
                )}
              </div>
            )}
            <div className="hm-opts">
              <div className="hm-optrow">
                <span className="tx">
                  <b>Подсказки</b>
                  <span>угадывать стопку по кадрам</span>
                </span>
                <Toggle label="Подсказки" checked={hints} onChange={(v) => void setOpt("hints_enabled", v)} />
              </div>
              <div className="hm-optrow">
                <span className="tx">
                  <b>Дубли</b>
                  <span>искать копии</span>
                </span>
                <Toggle label="Дубли" checked={dupes} onChange={(v) => void setOpt("dupes_enabled", v)} />
              </div>
            </div>
          </div>
        </div>

        {(errors.length > 0 || failure) && (
          <div className="hm-errs">
            {errors.map((e) => (
              <span key={`${e.code}${e.path}`} className="text-danger">
                <b>{e.path}</b> — {e.message}
              </span>
            ))}
            {failure && <span className="text-danger">{failure}</span>}
          </div>
        )}

        <div className="flex-1" />

        {tidy.length > 0 && (
          <div className="flex shrink-0 flex-col gap-3">
            <span className="hm-lbl text-[13px]">Разбор закончен</span>
            {tidy.map((t) => (
              <div key={t.id} className="hm-tidy">
                <span className="n">
                  <b>{nf(t.empty)}</b>
                  <span>{plural(t.empty, "пустая папка", "пустые папки", "пустых папок")}</span>
                </span>
                <span className="tx">
                  <Name r={t} />
                  <span className="text-[13px] text-dim">Разложено всё: {cards(t.total)}</span>
                  <span className="ft">
                    <Button variant="primary" disabled={tidying === t.id} onClick={() => void tidyUp(t, false)}>
                      Убрать {folders(t.empty)}
                    </Button>
                    <Button className="bg-strong! hover:bg-line!" disabled={tidying === t.id} onClick={() => void tidyUp(t, true)}>
                      Не убирать
                    </Button>
                  </span>
                </span>
              </div>
            ))}
          </div>
        )}

        {(first || rest.length > 0) && (
          <div className="flex shrink-0 flex-col gap-3">
            <span className="hm-lbl text-[13px]">Продолжить разбор</span>
            <div className="hm-rstack">
              {first && (
                <div className="hm-rbig">
                  <Thumb cacheDir={state.cacheDir} card={first.next} />
                  <span className="tx">
                    <span className="flex min-w-0 items-center gap-2">
                      <Name r={first} />
                      <span className="flex-1" />
                      <Remove label="Убрать разбор" onClick={() => void drop(first, 0)} />
                    </span>
                    {mergedNote && (
                      <span className="hm-note">
                        <FolderPlus size={14} strokeWidth={1.5} />
                        {mergedNote}
                      </span>
                    )}
                    <div className="hm-left">
                      <b>{nf(first.left)}</b>
                      <span>осталось из {nf(first.total)}</span>
                    </div>
                    <Bar r={first} />
                    <span className="ft">
                      <Button variant="primary" hotkey={resumeOnEnter ? "Enter" : undefined} onClick={() => void resume(first)}>
                        Продолжить
                      </Button>
                      <span>{day(first.openedAt)}</span>
                    </span>
                  </span>
                </div>
              )}
              {rest.length > 0 && (
                <div className="hm-rrs">
                  {rest.map((it) => {
                    const r = it.r;
                    if (it.undo)
                      return (
                        <div key={`u${r.id}`} role="status" className="hm-rr undo">
                          <span className="tx">
                            <span className="hm-nm">Разбор «{r.label}» убран</span>
                          </span>
                          <Button size={36} className="bg-strong! hover:bg-line!" icon={<Undo2 size={15} strokeWidth={1.5} />} hotkey="Ctrl Z" onClick={() => void restore()}>
                            Вернуть
                          </Button>
                        </div>
                      );
                    const at = reviews.indexOf(r);
                    const zone = movable(r) ? (`r:${r.id}` as const) : undefined;
                    const hot = !!zone && over?.zone === zone;
                    return (
                      <div
                        key={r.id}
                        role="button"
                        tabIndex={0}
                        data-drop={zone}
                        className={`hm-rr cursor-pointer${r.online ? "" : " gone"}${hot ? " over" : ""}`}
                        onClick={() => r.online && void resume(r)}
                        onKeyDown={(e) => e.key === " " && r.online && void resume(r)}
                      >
                        <span className="tx">
                          <Name r={r} />
                          {hot ? (
                            <span className="hm-drop">
                              <FolderPlus size={14} strokeWidth={1.5} />
                              Отпусти — продолжу разбор из этой папки
                            </span>
                          ) : r.online ? (
                            <span className="sub">
                              осталось {nf(r.left)} из {nf(r.total)} · {day(r.openedAt)}
                            </span>
                          ) : (
                            <span className="sub hm-err">
                              {goneText(r.gone ?? r.roots[0]?.path ?? "")} · {day(r.openedAt)}
                            </span>
                          )}
                          {!r.online && !hot && (
                            <span className="btns">
                              <Again onClick={() => void recheck()} />
                              {zone && (
                                <Button
                                  size={36}
                                  className="bg-strong! hover:bg-line!"
                                  icon={<Folder size={15} strokeWidth={1.5} />}
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    void pickMoved(r);
                                  }}
                                >
                                  Указать папку
                                </Button>
                              )}
                            </span>
                          )}
                        </span>
                        {r.online && <Bar r={r} />}
                        {!hot && (
                          <span className="flex shrink-0 items-center gap-1">
                            <Remove label="Убрать разбор" onClick={() => void drop(r, at)} />
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        )}
      </section>
    </main>
  );
}
