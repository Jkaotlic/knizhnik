mod capture;
mod commands;
mod db;
mod domain;
mod error;
mod export;
mod providers;

use commands::AppState;
use providers::{googlebooks::GoogleBooks, openlibrary::OpenLibrary, MetadataService};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            use tauri::Manager;
            let dir = app.path().app_data_dir().expect("нет app data dir");
            std::fs::create_dir_all(&dir).ok();
            let conn = db::open_at(&dir.join("knizhnik.sqlite3")).expect("не открыть БД");
            let saved_key = db::settings::get(&conn, "google_books_api_key").unwrap_or(None);
            let google_key: providers::SharedApiKey =
                std::sync::Arc::new(std::sync::RwLock::new(saved_key));
            let client = reqwest::Client::new();
            let metadata = MetadataService::new(vec![
                Box::new(OpenLibrary::new(client.clone())),
                Box::new(GoogleBooks::new(client, google_key.clone())),
            ]);
            app.manage(AppState { db: std::sync::Mutex::new(conn), metadata, google_key });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::locations_all,
            commands::location_create,
            commands::location_update,
            commands::location_move,
            commands::location_delete,
            commands::location_breadcrumb,
            commands::book_create,
            commands::book_update,
            commands::book_delete,
            commands::book_set_shelf,
            commands::book_set_availability,
            commands::books_on_shelf,
            commands::books_search,
            commands::metadata_lookup_isbn,
            commands::metadata_lookup_title,
            commands::capture,
            commands::stats_summary,
            commands::export_csv,
            commands::settings_get_google_key,
            commands::settings_set_google_key,
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска Tauri");
}
