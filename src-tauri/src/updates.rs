use crate::commands::AppState;
use crate::db::{get_setting, set_setting};
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Error, Update, UpdaterExt};

const FIRST_CHECK: Duration = Duration::from_secs(10);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateState {
    phase: &'static str,
    version: Option<String>,
    progress: Option<f32>,
    notes: Option<String>,
    error: Option<String>,
    checked_at: Option<i64>,
    installed: bool,
    current: String,
}

#[derive(Default)]
pub struct Updates {
    state: Mutex<UpdateState>,
    pending: Mutex<Option<(Update, Vec<u8>)>>,
    busy: AtomicBool,
}

fn enabled(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let conn = state.conn();
    get_setting(&conn, "updates").ok().flatten().as_deref() != Some("off")
}

fn change(app: &AppHandle, f: impl FnOnce(&mut UpdateState)) {
    let updates = app.state::<Updates>();
    let snapshot = {
        let mut s = updates.state.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s);
        s.clone()
    };
    let _ = app.emit("update://state", snapshot);
}

fn text(e: &Error) -> AppError {
    match e {
        Error::Minisign(_) | Error::Base64(_) | Error::SignatureUtf8(_) => {
            AppError::new("UPDATE_SIGNATURE", "Обновление отклонено: подпись не совпала. Попробую позже.")
        }
        Error::Reqwest(_) | Error::Network(_) => AppError::new("UPDATE_NET", "Не удалось проверить — нет сети. Попробую позже."),
        _ => AppError::new("UPDATE_CHECK", format!("Не удалось проверить обновления ({e}). Попробую позже.")),
    }
}

fn install_error() -> AppError {
    AppError::new(
        "UPDATE_INSTALL",
        "Обновление не поставлено — нужно разрешение Windows. Нажми «Перезапустить», чтобы попробовать снова.",
    )
}

pub fn init(app: &AppHandle) {
    let current = app.package_info().version.to_string();
    let (installed, notes, failed) = {
        let state = app.state::<AppState>();
        let conn = state.conn();
        let version = get_setting(&conn, "update_version").ok().flatten();
        let notes = get_setting(&conn, "update_notes").ok().flatten();
        let failed = get_setting(&conn, "update_error").ok().flatten();
        let _ = conn.execute("DELETE FROM settings WHERE key = 'update_error'", []);
        (version.as_deref() == Some(current.as_str()), notes, failed)
    };
    let on = enabled(app);
    change(app, |s| {
        s.current = current;
        s.phase = if on { "idle" } else { "off" };
        if installed {
            s.installed = true;
            s.notes = notes;
        }
        if let Some(error) = failed {
            s.phase = "failed";
            s.error = Some(error);
        }
    });
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            tauri::async_runtime::block_on(check(&handle));
            std::thread::sleep(EVERY);
        }
    });
}

pub async fn check(app: &AppHandle) {
    let updates = app.state::<Updates>();
    if updates.state.lock().unwrap_or_else(|e| e.into_inner()).phase == "ready" {
        return;
    }
    if !enabled(app) {
        change(app, |s| s.phase = "off");
        return;
    }
    if cfg!(debug_assertions) || updates.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    change(app, |s| {
        s.phase = "checking";
        s.error = None;
    });
    let found = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(None) => change(app, |s| {
            s.phase = "latest";
            s.checked_at = Some(crate::db::now_ms());
        }),
        Ok(Some(update)) => download(app, update).await,
        Err(e) => change(app, |s| {
            s.phase = "failed";
            s.error = Some(text(&e).message);
        }),
    }
    updates.busy.store(false, Ordering::SeqCst);
}

async fn download(app: &AppHandle, update: Update) {
    let version = update.version.clone();
    let notes = update.body.clone();
    change(app, |s| {
        s.phase = "downloading";
        s.version = Some(version.clone());
        s.progress = Some(0.0);
        s.notes = notes.clone();
        s.installed = false;
    });
    let mut got = 0usize;
    let mut last = Instant::now();
    let progress = |chunk: usize, total: Option<u64>| {
        got += chunk;
        if let Some(total) = total.filter(|t| *t > 0) {
            if last.elapsed() >= Duration::from_millis(250) {
                last = Instant::now();
                let p = (got as f32 / total as f32).min(1.0);
                change(app, |s| s.progress = Some(p));
            }
        }
    };
    match update.download(progress, || {}).await {
        Ok(bytes) => {
            {
                let state = app.state::<AppState>();
                let conn = state.conn();
                let _ = set_setting(&conn, "update_version", &version);
                let _ = set_setting(&conn, "update_notes", notes.as_deref().unwrap_or(""));
            }
            *app.state::<Updates>().pending.lock().unwrap_or_else(|e| e.into_inner()) = Some((update, bytes));
            change(app, |s| {
                s.phase = "ready";
                s.progress = None;
                s.checked_at = Some(crate::db::now_ms());
            });
        }
        Err(e) => change(app, |s| {
            s.phase = "failed";
            s.progress = None;
            s.error = Some(text(&e).message);
        }),
    }
}

pub fn install(app: &AppHandle, restart: bool) -> AppResult<()> {
    let Some((update, bytes)) = app.state::<Updates>().pending.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
        return Ok(());
    };
    let state = app.state::<AppState>();
    let conn = state.conn();
    if update.restart_after_install(restart).install(bytes).is_err() {
        let e = install_error();
        if !restart {
            let _ = set_setting(&conn, "update_error", &e.message);
        }
        drop(conn);
        change(app, |s| {
            s.phase = "ready";
            s.error = Some(e.message.clone());
        });
        return Err(e);
    }
    Ok(())
}

pub fn toggled(app: &AppHandle) {
    if enabled(app) {
        change(app, |s| {
            if s.phase == "off" {
                s.phase = "idle";
            }
        });
        let handle = app.clone();
        tauri::async_runtime::spawn(async move { check(&handle).await });
    } else {
        change(app, |s| {
            if s.phase != "ready" {
                s.phase = "off";
                s.error = None;
            }
        });
    }
}

#[tauri::command]
pub fn update_state(updates: tauri::State<Updates>) -> UpdateState {
    updates.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> UpdateState {
    check(&app).await;
    app.state::<Updates>().state.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
pub fn update_install(app: AppHandle) -> AppResult<()> {
    install(&app, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_and_signature_errors_read_as_russian() {
        assert_eq!(text(&Error::Network("x".into())).code, "UPDATE_NET");
        assert_eq!(text(&Error::SignatureUtf8("x".into())).code, "UPDATE_SIGNATURE");
        assert_eq!(text(&Error::ReleaseNotFound).code, "UPDATE_CHECK");
    }
}
