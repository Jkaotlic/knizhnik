mod capture;
mod commands;
mod covers;
mod db;
mod domain;
mod error;
mod export;
mod import;
#[cfg(test)]
mod journey;
mod metadata;
mod providers;

use commands::AppState;
use providers::{googlebooks::GoogleBooks, openlibrary::OpenLibrary, MetadataService};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            #[cfg(desktop)]
            {
                // Ошибку намеренно не пробрасываем: пока в tauri.conf.json нет
                // pubkey, плагин не поднимется — но это не повод не запускать
                // приложение. Кнопка «Проверить обновления» тогда честно скажет,
                // что автообновление не настроено.
                if let Err(e) = app.handle().plugin(tauri_plugin_updater::Builder::new().build()) {
                    eprintln!("автообновление выключено: {e}");
                }
                let _ = app.handle().plugin(tauri_plugin_process::init());
            }
            // `expect` здесь ронял приложение молча: на macOS окно просто
            // не появлялось. Ошибка из setup хотя бы доходит до пользователя.
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open_at(&dir.join("knizhnik.sqlite3"))?;
            let saved_key = db::settings::get(&conn, "google_books_api_key").unwrap_or(None);
            let google_key: providers::SharedApiKey =
                std::sync::Arc::new(std::sync::RwLock::new(saved_key));
            // Без таймаута зависший запрос держал сканирование в «ищу…» вечно.
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(6))
                .timeout(std::time::Duration::from_secs(15))
                .user_agent(concat!("knizhnik/", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_default();
            // Основные каталоги спрашиваются одновременно, ответы склеиваются:
            // у каждого свои сильные поля. Wildberries — запасной: витрина
            // маркетплейса ловит антибот при частых запросах, поэтому её
            // трогаем, только если основные ничего не нашли.
            let metadata = MetadataService::new(vec![
                Box::new(OpenLibrary::new(client.clone())),
                Box::new(providers::sru::Sru::dnb(client.clone())),
                Box::new(providers::sru::Sru::loc(client.clone())),
                Box::new(GoogleBooks::new(client.clone(), google_key.clone())),
            ])
            .with_fallback(vec![Box::new(providers::wildberries::Wildberries::new(
                client.clone(),
            ))]);
            app.manage(AppState {
                db: std::sync::Mutex::new(conn),
                metadata,
                google_key,
                app_data: dir,
                http: client,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::locations_all,
            commands::location_create,
            commands::location_update,
            commands::location_move,
            commands::location_delete,
            commands::bookcase_create,
            commands::shelf_create,
            commands::location_subtree_info,
            commands::location_breadcrumb,
            commands::book_create,
            commands::book_update,
            commands::book_delete,
            commands::book_set_shelf,
            commands::book_set_availability,
            commands::book_duplicates,
            commands::books_on_shelf,
            commands::books_without_shelf,
            commands::books_search,
            commands::metadata_lookup_isbn,
            commands::metadata_lookup_title,
            commands::capture,
            commands::stats_summary,
            commands::export_csv_to,
            commands::settings_get_google_key,
            commands::settings_set_google_key,
            commands::backup_export_to,
            commands::backup_import_from,
            commands::cache_size,
            commands::cache_clear,
            commands::import_csv_preview,
            commands::import_csv_apply,
            commands::covers_dir,
            commands::cover_cache,
            commands::covers_cache_all,
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска Tauri");
}
