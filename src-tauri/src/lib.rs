mod commands;
mod db;
mod deck;
mod develop;
mod error;
mod media;
mod moves;
mod piles;
mod watch;

use commands::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("media", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || responder.respond(media::serve(&app, &request)));
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(data_dir.join("cache"))?;
            let conn = db::open(&data_dir.join("sorter.db"))?;
            if let Err(e) = moves::recover(&conn) {
                eprintln!("recover: {e}");
            }
            let paused = Arc::new(AtomicBool::new(false));
            let wake = Arc::new(AtomicBool::new(true));
            develop::spawn(app.handle().clone(), data_dir.join("sorter.db"), data_dir.join("cache"), paused.clone(), wake.clone());
            watch::spawn(app.handle().clone(), data_dir.join("sorter.db"), wake.clone());
            app.manage(AppState { db: Mutex::new(conn), data_dir, paused, wake });
            if let Some(window) = app.get_webview_window("main") {
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
            commands::create_pile,
            commands::rename_pile,
            commands::set_pile_key,
            commands::remove_pile,
            commands::develop_control,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
