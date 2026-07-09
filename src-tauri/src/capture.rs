use crate::db::books;
use crate::db::models::{Book, BookInput};
use crate::domain::isbn;
use crate::domain::matching::{pick_best, MetadataCandidate};
use crate::error::AppError;
use crate::providers::MetadataService;
use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, Serialize)]
pub struct CaptureResult {
    pub book: Book,
    pub is_possible_duplicate: bool,
    pub source: String,
}

pub async fn capture_scan(
    conn: &Mutex<rusqlite::Connection>,
    svc: &MetadataService,
    shelf_id: i64,
    raw_isbn: &str,
) -> Result<CaptureResult, AppError> {
    let isbn13 = isbn::normalize_and_validate(raw_isbn).map_err(|e| AppError::Isbn(e.to_string()))?;
    let candidates = svc.lookup_isbn(&isbn13).await;
    let (input, source) = match pick_best(&candidates) {
        Some(c) => (candidate_to_input(c, &isbn13, shelf_id), c.source.clone()),
        None => (placeholder_input(&isbn13, shelf_id), "none".to_string()),
    };
    let guard = conn.lock().expect("mutex соединения отравлен");
    let is_possible_duplicate = books::exists_isbn_on_shelf(&guard, &isbn13, shelf_id)?;
    let book = books::insert(&guard, &input)?;
    Ok(CaptureResult { book, is_possible_duplicate, source })
}

fn candidate_to_input(c: &MetadataCandidate, isbn13: &str, shelf_id: i64) -> BookInput {
    BookInput {
        title: c.title.clone(),
        authors: c.authors.clone(),
        isbn: Some(isbn13.to_string()),
        year: c.year,
        publisher: c.publisher.clone(),
        pages: c.pages,
        language: c.language.clone(),
        genre: None,
        annotation: None,
        cover_url: c.cover_url.clone(),
        shelf_id: Some(shelf_id),
        status: None,
        rating: None,
        notes: None,
    }
}

fn placeholder_input(isbn13: &str, shelf_id: i64) -> BookInput {
    BookInput {
        title: isbn13.to_string(),
        isbn: Some(isbn13.to_string()),
        shelf_id: Some(shelf_id),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{locations, open_in_memory};
    use crate::providers::MetadataProvider;
    use async_trait::async_trait;

    struct Mock(Vec<MetadataCandidate>);
    #[async_trait]
    impl MetadataProvider for Mock {
        async fn lookup_isbn(&self, _i: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            Ok(self.0.clone())
        }
        async fn lookup_title(&self, _t: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            Ok(self.0.clone())
        }
        fn name(&self) -> &'static str { "mock" }
    }

    fn setup() -> (Mutex<rusqlite::Connection>, i64) {
        let conn = open_in_memory().unwrap();
        let root = locations::create(&conn, None, "Дом", "root", None).unwrap().id;
        let shelf = locations::create(&conn, Some(root), "Полка", "shelf", Some("A-3")).unwrap().id;
        (Mutex::new(conn), shelf)
    }

    fn cand() -> MetadataCandidate {
        MetadataCandidate {
            title: "Дюна".into(),
            authors: Some("Фрэнк Герберт".into()),
            isbn: Some("9785171183660".into()),
            year: Some(2019), publisher: Some("АСТ".into()), pages: Some(704),
            language: Some("ru".into()), cover_url: Some("http://x/c.jpg".into()),
            source: "openlibrary".into(),
        }
    }

    #[tokio::test]
    async fn places_book_on_shelf_with_metadata() {
        let (conn, shelf) = setup();
        let svc = MetadataService::new(vec![Box::new(Mock(vec![cand()]))]);
        let r = capture_scan(&conn, &svc, shelf, "978-5-17-118366-0").await.unwrap();
        assert_eq!(r.book.title, "Дюна");
        assert_eq!(r.book.shelf_id, Some(shelf));
        assert_eq!(r.book.status, None);
        assert_eq!(r.book.availability, "on_shelf");
        assert_eq!(r.book.isbn.as_deref(), Some("9785171183660"));
        assert_eq!(r.source, "openlibrary");
        assert!(!r.is_possible_duplicate);
    }

    #[tokio::test]
    async fn second_scan_same_isbn_flags_duplicate() {
        let (conn, shelf) = setup();
        let svc = MetadataService::new(vec![Box::new(Mock(vec![cand()]))]);
        capture_scan(&conn, &svc, shelf, "9785171183660").await.unwrap();
        let r2 = capture_scan(&conn, &svc, shelf, "9785171183660").await.unwrap();
        assert!(r2.is_possible_duplicate);
        // всё равно вторая запись создана
        let count = books::on_shelf(&conn.lock().unwrap(), shelf).unwrap().len();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn no_metadata_creates_placeholder() {
        let (conn, shelf) = setup();
        let svc = MetadataService::new(vec![Box::new(Mock(vec![]))]);
        let r = capture_scan(&conn, &svc, shelf, "9785171183660").await.unwrap();
        assert_eq!(r.source, "none");
        assert_eq!(r.book.title, "9785171183660"); // ISBN как плейсхолдер
        assert_eq!(r.book.shelf_id, Some(shelf));
    }

    #[tokio::test]
    async fn invalid_isbn_errors_without_insert() {
        let (conn, shelf) = setup();
        let svc = MetadataService::new(vec![Box::new(Mock(vec![]))]);
        let err = capture_scan(&conn, &svc, shelf, "12345").await;
        assert!(err.is_err());
        assert_eq!(books::on_shelf(&conn.lock().unwrap(), shelf).unwrap().len(), 0);
    }
}
