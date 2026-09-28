mod commands;
mod db;
mod deck;
mod error;
mod media;
mod moves;
mod piles;

use commands::AppState;
use std::sync::Mutex;
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
            app.manage(AppState { db: Mutex::new(conn), data_dir });
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
