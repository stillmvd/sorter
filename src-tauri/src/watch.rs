use crate::db;
use crate::deck::{self, CardView};
use crate::piles;
use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeckChanged {
    added: Vec<CardView>,
    gone: Vec<i64>,
}

type Watched = (Vec<(String, bool)>, Option<String>);

fn current(conn: &Connection) -> Watched {
    let roots: Vec<(String, bool)> = conn
        .prepare(
            "SELECT path, online FROM review_root
             WHERE review_id = (SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'review_id') ORDER BY ord",
        )
        .and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect())
        .unwrap_or_default();
    let table = if roots.is_empty() { None } else { db::get_setting(conn, "table_path").ok().flatten() };
    (roots, table)
}

fn refresh_online(conn: &Connection, roots: &[(String, bool)]) -> bool {
    let mut changed = false;
    for (p, online) in roots {
        let now = Path::new(p).is_dir();
        if now != *online {
            let _ = conn.execute("UPDATE review_root SET online = ?2 WHERE path = ?1", params![p, now]);
            changed = true;
        }
    }
    changed
}

pub fn spawn(app: AppHandle, db_path: PathBuf, wake: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let Ok(conn) = db::open(&db_path) else { return };
        let (tx, rx) = mpsc::channel::<()>();
        let _ = tx.send(());
        let mut watched: Watched = (Vec::new(), None);
        let mut _debouncer = None;
        let mut retry = false;
        let stop = AtomicBool::new(false);
        loop {
            let mut now = current(&conn);
            if refresh_online(&conn, &now.0) {
                now = current(&conn);
                let _ = app.emit("deck://roots", ());
                wake.store(true, Ordering::Relaxed);
            }
            if now != watched {
                let sender = tx.clone();
                _debouncer = new_debouncer(Duration::from_millis(1500), None, move |r: DebounceEventResult| {
                    if r.is_ok() {
                        let _ = sender.send(());
                    }
                })
                .ok()
                .map(|mut d| {
                    for (p, online) in &now.0 {
                        if *online {
                            let _ = d.watch(Path::new(p), RecursiveMode::Recursive);
                        }
                    }
                    if let Some(t) = &now.1 {
                        let _ = d.watch(Path::new(t), RecursiveMode::NonRecursive);
                    }
                    d
                });
                watched = now.clone();
                retry = true;
            }
            let timeout = if retry { Duration::from_secs(3) } else { Duration::from_secs(2) };
            let fired = rx.recv_timeout(timeout).is_ok();
            while rx.try_recv().is_ok() {}
            if !fired && !retry {
                continue;
            }
            retry = false;
            if current(&conn) != watched {
                retry = true;
                continue;
            }
            let mut added = Vec::new();
            let mut gone = Vec::new();
            for (root, _) in watched.0.iter().filter(|(_, online)| *online) {
                if let Ok(r) = deck::sync_seen(&conn, root, &stop, &mut |_| {}) {
                    retry |= r.pending;
                    added.extend(r.added);
                    gone.extend(r.gone);
                }
            }
            if !added.is_empty() || !gone.is_empty() {
                let added = deck::cards(&conn, &added).unwrap_or_default();
                let _ = app.emit("deck://changed", DeckChanged { added, gone });
                wake.store(true, Ordering::Relaxed);
                if crate::series::regroup(&conn).unwrap_or(false) {
                    let _ = app.emit("deck://series", ());
                }
            }
            if let Some(t) = &watched.1
                && piles::sync(&conn, t).is_ok()
                && let Ok(list) = piles::list(&conn, t)
            {
                let _ = app.emit("piles://changed", list);
            }
        }
    });
}
