use crate::capture::{capture_scan, CaptureResult};
use crate::db::books::{self, BookHit};
use crate::db::locations;
use crate::db::models::{Book, BookInput, Location, Stats};
use crate::error::AppError;
use crate::export;
use crate::db::settings;
use crate::providers::{MetadataService, SharedApiKey};
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub metadata: MetadataService,
    pub google_key: SharedApiKey,
}

// --- Локации ---
#[tauri::command]
pub fn locations_all(state: State<AppState>) -> Result<Vec<Location>, AppError> {
    locations::all(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn location_create(
    state: State<AppState>,
    parent_id: Option<i64>,
    name: String,
    kind: String,
    label: Option<String>,
) -> Result<Location, AppError> {
    locations::create(&state.db.lock().unwrap(), parent_id, &name, &kind, label.as_deref())
}

#[tauri::command]
pub fn location_update(
    state: State<AppState>,
    id: i64,
    name: Option<String>,
    label: Option<String>,
) -> Result<Location, AppError> {
    locations::update(&state.db.lock().unwrap(), id, name.as_deref(), label.as_deref())
}

#[tauri::command]
pub fn location_move(state: State<AppState>, id: i64, new_parent_id: Option<i64>) -> Result<(), AppError> {
    locations::move_to(&state.db.lock().unwrap(), id, new_parent_id)
}

#[tauri::command]
pub fn location_delete(state: State<AppState>, id: i64) -> Result<(), AppError> {
    locations::delete(&state.db.lock().unwrap(), id)
}

#[tauri::command]
pub fn location_breadcrumb(state: State<AppState>, shelf_id: i64) -> Result<String, AppError> {
    locations::breadcrumb(&state.db.lock().unwrap(), shelf_id)
}

// --- Книги ---
#[tauri::command]
pub fn book_create(state: State<AppState>, input: BookInput) -> Result<Book, AppError> {
    books::insert(&state.db.lock().unwrap(), &input)
}

#[tauri::command]
pub fn book_update(state: State<AppState>, id: i64, input: BookInput) -> Result<Book, AppError> {
    books::update(&state.db.lock().unwrap(), id, &input)
}

#[tauri::command]
pub fn book_delete(state: State<AppState>, id: i64) -> Result<(), AppError> {
    books::delete(&state.db.lock().unwrap(), id)
}

#[tauri::command]
pub fn book_set_shelf(state: State<AppState>, id: i64, shelf_id: Option<i64>) -> Result<(), AppError> {
    books::set_shelf(&state.db.lock().unwrap(), id, shelf_id)
}

#[tauri::command]
pub fn book_set_availability(
    state: State<AppState>,
    id: i64,
    availability: String,
    lent_to: Option<String>,
) -> Result<(), AppError> {
    books::set_availability(&state.db.lock().unwrap(), id, &availability, lent_to.as_deref())
}

#[tauri::command]
pub fn books_on_shelf(state: State<AppState>, shelf_id: i64) -> Result<Vec<Book>, AppError> {
    books::on_shelf(&state.db.lock().unwrap(), shelf_id)
}

#[tauri::command]
pub fn books_search(state: State<AppState>, query: String) -> Result<Vec<BookHit>, AppError> {
    books::search(&state.db.lock().unwrap(), &query)
}

// --- Метаданные (ручной режим: список кандидатов) ---
#[tauri::command]
pub async fn metadata_lookup_isbn(
    state: State<'_, AppState>,
    isbn: String,
) -> Result<Vec<crate::domain::matching::MetadataCandidate>, AppError> {
    let norm = crate::domain::isbn::normalize_and_validate(&isbn)
        .map_err(|e| AppError::Isbn(e.to_string()))?;
    Ok(state.metadata.lookup_isbn(&norm).await)
}

#[tauri::command]
pub async fn metadata_lookup_title(
    state: State<'_, AppState>,
    title: String,
) -> Result<Vec<crate::domain::matching::MetadataCandidate>, AppError> {
    Ok(state.metadata.lookup_title(&title).await)
}

// --- Капчур ---
#[tauri::command]
pub async fn capture(
    state: State<'_, AppState>,
    shelf_id: i64,
    isbn: String,
) -> Result<CaptureResult, AppError> {
    capture_scan(&state.db, &state.metadata, shelf_id, &isbn).await
}

// --- Статистика / экспорт ---
#[tauri::command]
pub fn stats_summary(state: State<AppState>) -> Result<Stats, AppError> {
    books::stats(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn export_csv(state: State<AppState>) -> Result<String, AppError> {
    export::export_csv(&state.db.lock().unwrap())
}

// --- Настройки ---
#[tauri::command]
pub fn settings_get_google_key(state: State<AppState>) -> Result<String, AppError> {
    Ok(state.google_key.read().unwrap().clone().unwrap_or_default())
}

#[tauri::command]
pub fn settings_set_google_key(state: State<AppState>, key: String) -> Result<(), AppError> {
    let trimmed = key.trim();
    let value = if trimmed.is_empty() { None } else { Some(trimmed) };
    settings::set(&state.db.lock().unwrap(), "google_books_api_key", value)?;
    *state.google_key.write().unwrap() = value.map(|s| s.to_string());
    Ok(())
}

// --- Резервная копия ---
#[tauri::command]
pub fn backup_export(state: State<AppState>) -> Result<String, AppError> {
    let b = crate::db::backup::export(&state.db.lock().unwrap())?;
    serde_json::to_string_pretty(&b).map_err(|e| AppError::Rule(e.to_string()))
}

#[tauri::command]
pub fn backup_import(
    state: State<AppState>,
    json: String,
) -> Result<crate::db::backup::ImportSummary, AppError> {
    let backup: crate::db::backup::Backup = serde_json::from_str(&json)
        .map_err(|_| AppError::Rule("Не удалось прочитать файл резервной копии".into()))?;
    let mut guard = state.db.lock().unwrap();
    crate::db::backup::import(&mut guard, &backup)
}
