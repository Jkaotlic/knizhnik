use crate::capture::{capture_scan, CaptureResult};
use crate::db::books::{self, BookHit};
use crate::db::locations;
use crate::db::models::{Book, BookInput, Location, Stats};
use crate::db::settings;
use crate::error::AppError;
use crate::export;
use crate::providers::{MetadataService, SharedApiKey};
use serde::Serialize;
use std::sync::{Mutex, MutexGuard};
use tauri::State;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub metadata: MetadataService,
    pub google_key: SharedApiKey,
    pub app_data: std::path::PathBuf,
    pub http: reqwest::Client,
}

impl AppState {
    /// Отравленный мьютекс раньше ронял всё приложение через `unwrap()`.
    /// Теперь это обычная ошибка с понятным текстом.
    fn conn(&self) -> Result<MutexGuard<'_, rusqlite::Connection>, AppError> {
        self.db.lock().map_err(|_| {
            AppError::Rule("Соединение с базой в нерабочем состоянии, перезапусти приложение".into())
        })
    }
}

fn write_file(path: &str, contents: &str) -> Result<(), AppError> {
    std::fs::write(path, contents)
        .map_err(|e| AppError::Rule(format!("Не удалось записать файл: {e}")))
}

// --- Локации ---
#[tauri::command]
pub fn locations_all(state: State<AppState>) -> Result<Vec<Location>, AppError> {
    let conn = state.conn()?;
    locations::all(&conn)
}

#[tauri::command]
pub fn location_create(
    state: State<AppState>,
    parent_id: Option<i64>,
    name: String,
    kind: String,
    label: Option<String>,
) -> Result<Location, AppError> {
    let conn = state.conn()?;
    locations::create(&conn, parent_id, &name, &kind, label.as_deref())
}

#[tauri::command]
pub fn location_update(
    state: State<AppState>,
    id: i64,
    name: Option<String>,
    label: Option<String>,
) -> Result<Location, AppError> {
    let conn = state.conn()?;
    locations::update(&conn, id, name.as_deref(), label.as_deref())
}

#[tauri::command]
pub fn location_move(
    state: State<AppState>,
    id: i64,
    new_parent_id: Option<i64>,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    locations::move_to(&conn, id, new_parent_id)
}

#[tauri::command]
pub fn location_delete(state: State<AppState>, id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    locations::delete(&conn, id)
}

/// Заводит шкаф. «Дом» и «Комната» при необходимости создаются молча —
/// в обычном интерфейсе этих уровней нет.
#[tauri::command]
pub fn bookcase_create(state: State<AppState>, name: String) -> Result<Location, AppError> {
    let conn = state.conn()?;
    locations::create_bookcase(&conn, &name)
}

#[tauri::command]
pub fn shelf_create(
    state: State<AppState>,
    bookcase_id: i64,
    name: String,
    label: Option<String>,
) -> Result<Location, AppError> {
    let conn = state.conn()?;
    locations::create_shelf(&conn, bookcase_id, &name, label.as_deref())
}

#[derive(Debug, Serialize)]
pub struct SubtreeInfo {
    pub locations: i64,
    pub books: i64,
}

/// Что именно зацепит удаление — чтобы спросить подтверждение по делу.
#[tauri::command]
pub fn location_subtree_info(state: State<AppState>, id: i64) -> Result<SubtreeInfo, AppError> {
    let conn = state.conn()?;
    let (locations, books) = locations::subtree_info(&conn, id)?;
    Ok(SubtreeInfo { locations, books })
}

#[tauri::command]
pub fn location_breadcrumb(state: State<AppState>, shelf_id: i64) -> Result<String, AppError> {
    let conn = state.conn()?;
    locations::breadcrumb(&conn, shelf_id)
}

// --- Книги ---
#[tauri::command]
pub fn book_create(state: State<AppState>, input: BookInput) -> Result<Book, AppError> {
    let conn = state.conn()?;
    books::insert(&conn, &input)
}

#[tauri::command]
pub fn book_update(state: State<AppState>, id: i64, input: BookInput) -> Result<Book, AppError> {
    let conn = state.conn()?;
    books::update(&conn, id, &input)
}

#[tauri::command]
pub fn book_delete(state: State<AppState>, id: i64) -> Result<(), AppError> {
    let conn = state.conn()?;
    // Сначала узнаём файл обложки — после удаления строки спросить будет не у кого.
    let cover = books::get(&conn, id).ok().and_then(|b| b.cover_path);
    books::delete(&conn, id)?;
    if let Some(name) = cover {
        crate::covers::remove(&state.app_data, &name);
    }
    Ok(())
}

// --- Обложки ---
/// Абсолютный путь к папке обложек — фронту он нужен, чтобы собрать
/// asset-ссылку на локальный файл.
#[tauri::command]
pub fn covers_dir(state: State<AppState>) -> Result<String, AppError> {
    Ok(crate::covers::dir(&state.app_data).to_string_lossy().to_string())
}

/// Забирает обложку одной книги к себе. Молча возвращает None, если качать
/// нечего или не получилось: обложка — украшение, ронять из-за неё сценарий глупо.
#[tauri::command]
pub async fn cover_cache(state: State<'_, AppState>, id: i64) -> Result<Option<String>, AppError> {
    let url = {
        let conn = state.conn()?;
        let book = books::get(&conn, id)?;
        match (book.cover_path, book.cover_url) {
            (Some(existing), _) => return Ok(Some(existing)),
            (None, Some(url)) if !url.trim().is_empty() => url,
            _ => return Ok(None),
        }
    };
    let Ok(name) = crate::covers::fetch(&state.http, &state.app_data, id, &url).await else {
        return Ok(None);
    };
    let conn = state.conn()?;
    books::set_cover_path(&conn, id, Some(&name))?;
    Ok(Some(name))
}

/// Разом забирает обложки всех книг, у которых их ещё нет локально.
#[tauri::command]
pub async fn covers_cache_all(state: State<'_, AppState>) -> Result<usize, AppError> {
    let pending = {
        let conn = state.conn()?;
        books::needing_covers(&conn)?
    };
    let mut done = 0usize;
    for (id, url) in pending {
        if let Ok(name) = crate::covers::fetch(&state.http, &state.app_data, id, &url).await {
            let conn = state.conn()?;
            books::set_cover_path(&conn, id, Some(&name))?;
            done += 1;
        }
    }
    Ok(done)
}

#[tauri::command]
pub fn book_set_shelf(
    state: State<AppState>,
    id: i64,
    shelf_id: Option<i64>,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    books::set_shelf(&conn, id, shelf_id)
}

#[tauri::command]
pub fn book_set_availability(
    state: State<AppState>,
    id: i64,
    availability: String,
    lent_to: Option<String>,
    due_at: Option<String>,
) -> Result<(), AppError> {
    let conn = state.conn()?;
    books::set_availability(&conn, id, &availability, lent_to.as_deref(), due_at.as_deref())
}

/// Где ещё в каталоге лежит книга с этим ISBN.
#[tauri::command]
pub fn book_duplicates(state: State<AppState>, isbn: String) -> Result<Vec<String>, AppError> {
    let norm = crate::domain::isbn::normalize_and_validate(&isbn)
        .unwrap_or_else(|_| isbn.trim().to_string());
    let conn = state.conn()?;
    books::find_isbn_duplicates(&conn, &norm)
}

#[tauri::command]
pub fn books_on_shelf(state: State<AppState>, shelf_id: i64) -> Result<Vec<Book>, AppError> {
    let conn = state.conn()?;
    books::on_shelf(&conn, shelf_id)
}

/// Книги, которым забыли указать полку.
#[tauri::command]
pub fn books_without_shelf(state: State<AppState>) -> Result<Vec<Book>, AppError> {
    let conn = state.conn()?;
    books::without_shelf(&conn)
}

#[tauri::command]
pub fn books_search(state: State<AppState>, query: String) -> Result<Vec<BookHit>, AppError> {
    let conn = state.conn()?;
    books::search(&conn, &query)
}

// --- Метаданные (ручной режим: список кандидатов) ---
#[tauri::command]
pub async fn metadata_lookup_isbn(
    state: State<'_, AppState>,
    isbn: String,
) -> Result<Vec<crate::domain::matching::MetadataCandidate>, AppError> {
    let norm = crate::domain::isbn::normalize_and_validate(&isbn)
        .map_err(|e| AppError::Isbn(e.to_string()))?;
    crate::metadata::lookup_isbn_cached(&state.db, &state.metadata, &norm).await
}

/// Сколько ISBN осело в кэше и возможность его сбросить —
/// на случай, если метаданные в источнике поправили.
#[tauri::command]
pub fn cache_size(state: State<AppState>) -> Result<i64, AppError> {
    let conn = state.conn()?;
    crate::db::cache::size(&conn)
}

#[tauri::command]
pub fn cache_clear(state: State<AppState>) -> Result<usize, AppError> {
    let conn = state.conn()?;
    crate::db::cache::clear(&conn)
}

#[tauri::command]
pub async fn metadata_lookup_title(
    state: State<'_, AppState>,
    title: String,
) -> Result<Vec<crate::domain::matching::MetadataCandidate>, AppError> {
    state.metadata.lookup_title(&title).await
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
    let conn = state.conn()?;
    books::stats(&conn)
}

/// Путь приходит из нативного диалога сохранения: в WKWebView на macOS
/// скачивание через `<a download>` ничего не сохраняло.
#[tauri::command]
pub fn export_csv_to(state: State<AppState>, path: String) -> Result<(), AppError> {
    let csv = {
        let conn = state.conn()?;
        export::export_csv(&conn)?
    };
    write_file(&path, &csv)
}

// --- Настройки ---
#[tauri::command]
pub fn settings_get_google_key(state: State<AppState>) -> Result<String, AppError> {
    let key = state
        .google_key
        .read()
        .map_err(|_| AppError::Rule("Не прочитать API-ключ".into()))?;
    Ok(key.clone().unwrap_or_default())
}

#[tauri::command]
pub fn settings_set_google_key(state: State<AppState>, key: String) -> Result<(), AppError> {
    let trimmed = key.trim();
    let value = if trimmed.is_empty() { None } else { Some(trimmed) };
    let conn = state.conn()?;
    settings::set(&conn, "google_books_api_key", value)?;
    *state
        .google_key
        .write()
        .map_err(|_| AppError::Rule("Не сохранить API-ключ".into()))? = value.map(str::to_string);
    Ok(())
}

// --- Резервная копия ---
#[tauri::command]
pub fn backup_export_to(state: State<AppState>, path: String) -> Result<(), AppError> {
    let backup = {
        let conn = state.conn()?;
        crate::db::backup::export(&conn)?
    };
    let json = serde_json::to_string_pretty(&backup).map_err(|e| AppError::Rule(e.to_string()))?;
    write_file(&path, &json)
}

/// Разбирает CSV, ничего не записывая, — чтобы показать «нашлось N книг»
/// до того, как человек согласится их залить.
#[tauri::command]
pub fn import_csv_preview(path: String) -> Result<usize, AppError> {
    let text = read_text(&path)?;
    Ok(crate::import::parse(&text)?.len())
}

#[tauri::command]
pub fn import_csv_apply(
    state: State<AppState>,
    path: String,
    shelf_id: Option<i64>,
) -> Result<crate::import::ImportReport, AppError> {
    let text = read_text(&path)?;
    let parsed = crate::import::parse(&text)?;
    let conn = state.conn()?;
    crate::import::apply(&conn, &parsed, shelf_id)
}

fn read_text(path: &str) -> Result<String, AppError> {
    let bytes = std::fs::read(path)
        .map_err(|e| AppError::Rule(format!("Не удалось прочитать файл: {e}")))?;
    // Выгрузки из русских сервисов нередко в CP1251 — не падаем на них.
    Ok(match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => e
            .as_bytes()
            .iter()
            .map(|&b| if b < 0x80 { b as char } else { cp1251_char(b) })
            .collect(),
    })
}

/// Верхняя половина CP1251 — это кириллица подряд, начиная с А (U+0410),
/// плюс несколько знаков в начале диапазона.
fn cp1251_char(b: u8) -> char {
    match b {
        0xC0..=0xFF => char::from_u32(0x0410 + (b as u32 - 0xC0)).unwrap_or('?'),
        0xA8 => 'Ё',
        0xB8 => 'ё',
        _ => '?',
    }
}

#[tauri::command]
pub fn backup_import_from(
    state: State<AppState>,
    path: String,
) -> Result<crate::db::backup::ImportSummary, AppError> {
    let json = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Rule(format!("Не удалось прочитать файл: {e}")))?;
    let backup: crate::db::backup::Backup = serde_json::from_str(&json)
        .map_err(|_| AppError::Rule("Это не похоже на файл резервной копии Книжника".into()))?;
    let mut guard = state.conn()?;
    crate::db::backup::import(&mut guard, &backup)
}
