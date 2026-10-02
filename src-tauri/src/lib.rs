mod commands;
mod db;
mod deck;
mod develop;
mod dupes;
mod error;
mod hints;
mod media;
mod moves;
mod photo;
mod series;
mod piles;
mod playable;
mod taskbar_icon;
mod updates;
mod watch;

use commands::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

fn fit_to_screen(window: &tauri::WebviewWindow) {
    if window.is_maximized().unwrap_or(false) {
        return;
    }
    let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) else { return };
    let area = monitor.work_area();
    let w = size.width.min(area.size.width * 96 / 100);
    let h = size.height.min(area.size.height * 96 / 100);
    if (w, h) != (size.width, size.height) {
        let _ = window.set_size(tauri::PhysicalSize::new(w, h));
    }
    let x = area.position.x + (area.size.width as i32 - w as i32) / 2;
    let y = area.position.y + (area.size.height as i32 - h as i32) / 2;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            if let Some(path) = commands::folder_arg(args) {
                let _ = app.emit("open://folder", path);
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("media", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || responder.respond(media::serve(&app, &request)));
        })
        .setup(|app| {
            let data_dir = match std::env::var_os("SORTER_DATA") {
                Some(dir) => dir.into(),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(data_dir.join("cache"))?;
            let conn = db::open(&data_dir.join("sorter.db"))?;
            if let Err(e) = moves::recover(&conn) {
                eprintln!("recover: {e}");
            }
            let _ = hints::prune(&conn);
            let paused = Arc::new(AtomicBool::new(false));
            let wake = Arc::new(AtomicBool::new(true));
            develop::spawn(app.handle().clone(), data_dir.join("sorter.db"), data_dir.join("cache"), paused.clone(), wake.clone());
            watch::spawn(app.handle().clone(), data_dir.join("sorter.db"), wake.clone());
            let incoming = Mutex::new(commands::folder_arg(std::env::args()));
            app.manage(AppState { db: Mutex::new(conn), db_path: data_dir.join("sorter.db"), data_dir, paused, wake, incoming });
            app.manage(updates::Updates::default());
            updates::init(app.handle());
            if let Some(window) = app.get_webview_window("main") {
                taskbar_icon::apply(&window);
                fit_to_screen(&window);
                let target = window.clone();
                let handle = app.handle().clone();
                window.on_window_event(move |event| match event {
                    tauri::WindowEvent::ScaleFactorChanged { .. } => taskbar_icon::apply(&target),
                    tauri::WindowEvent::CloseRequested { .. } => {
                        let _ = updates::install(&handle, false);
                    }
                    _ => {}
                });
                let _ = window.show();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::choose_deck,
            commands::choose_table,
            commands::set_setting_cmd,
            commands::deck_window,
            commands::place,
            commands::defer,
            commands::undo_last,
            commands::undo_move,
            commands::undo_since,
            commands::journal,
            commands::journal_stats,
            commands::cache_info,
            commands::clear_cache,
            commands::hints,
            commands::create_pile,
            commands::rename_pile,
            commands::set_pile_key,
            commands::remove_pile,
            commands::develop_control,
            commands::dupes_for,
            commands::dismiss_dupe,
            commands::dupe_groups,
            commands::deck_series,
            commands::place_series,
            commands::split_series,
            commands::playable,
            commands::trash_copies,
            commands::replace_copy,
            commands::take_incoming,
            updates::update_state,
            updates::update_check,
            updates::update_install,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
