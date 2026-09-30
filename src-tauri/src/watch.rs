use crate::db;
use crate::deck::{self, CardView};
use crate::piles;
use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
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

pub fn spawn(app: AppHandle, db_path: PathBuf, wake: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let Ok(conn) = db::open(&db_path) else { return };
        let (tx, rx) = mpsc::channel::<()>();
        let mut watched: (Option<String>, Option<String>) = (None, None);
        let mut _debouncer = None;
        let mut retry = false;
        loop {
            let deck_path = db::get_setting(&conn, "deck_path").ok().flatten();
            let table_path = db::get_setting(&conn, "table_path").ok().flatten();
            if (deck_path.clone(), table_path.clone()) != watched {
                let sender = tx.clone();
                _debouncer = new_debouncer(Duration::from_millis(1500), None, move |r: DebounceEventResult| {
                    if r.is_ok() {
                        let _ = sender.send(());
                    }
                })
                .ok()
                .map(|mut d| {
                    for p in [&deck_path, &table_path].into_iter().flatten() {
                        let _ = d.watch(Path::new(p), RecursiveMode::NonRecursive);
                    }
                    d
                });
                watched = (deck_path.clone(), table_path.clone());
            }
            let timeout = if retry { Duration::from_secs(3) } else { Duration::from_secs(2) };
            let fired = rx.recv_timeout(timeout).is_ok();
            while rx.try_recv().is_ok() {}
            if !fired && !retry {
                continue;
            }
            retry = false;
            if let Some(d) = &deck_path {
                if let Ok(r) = deck::sync(&conn, d) {
                    retry = r.pending;
                    if !r.added.is_empty() || !r.gone.is_empty() {
                        let added = deck::cards(&conn, &r.added).unwrap_or_default();
                        let _ = app.emit("deck://changed", DeckChanged { added, gone: r.gone });
                        wake.store(true, Ordering::Relaxed);
                        if crate::series::regroup(&conn, d).unwrap_or(false) {
                            let _ = app.emit("deck://series", ());
                        }
                    }
                }
            }
            if let Some(t) = &table_path {
                if piles::sync(&conn, t).is_ok() {
                    if let Ok(list) = piles::list(&conn, t) {
                        let _ = app.emit("piles://changed", list);
                    }
                }
            }
        }
    });
}
