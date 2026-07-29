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
    /// Где уже лежат экземпляры с этим ISBN — по всему каталогу, не только
    /// на текущей полке.
    pub duplicate_at: Vec<String>,
    pub source: String,
    /// Почему метаданные не подтянулись, если причина — не «книги нет в базах».
    pub note: Option<String>,
}

pub async fn capture_scan(
    conn: &Mutex<rusqlite::Connection>,
    svc: &MetadataService,
    shelf_id: i64,
    raw_isbn: &str,
) -> Result<CaptureResult, AppError> {
    let isbn13 = isbn::normalize_and_validate(raw_isbn).map_err(|e| AppError::Isbn(e.to_string()))?;
    // Сбой сети не должен рвать поток сканирования: книга всё равно встаёт
    // на полку заглушкой, а причину показываем в ленте.
    let (candidates, note) = match crate::metadata::lookup_isbn_cached(conn, svc, &isbn13).await {
        Ok(c) => (c, None),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };
    let (input, source) = match pick_best(&candidates) {
        Some(c) => (candidate_to_input(c, &isbn13, shelf_id), c.source.clone()),
        None => (placeholder_input(&isbn13, shelf_id), "none".to_string()),
    };
    let guard = crate::metadata::lock(conn)?;
    let duplicate_at = books::find_isbn_duplicates(&guard, &isbn13)?;
    let book = books::insert(&guard, &input)?;
    Ok(CaptureResult {
        book,
        is_possible_duplicate: !duplicate_at.is_empty(),
        duplicate_at,
        source,
        note,
    })
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
        started_at: None,
        finished_at: None,
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

    struct Offline;
    #[async_trait]
    impl MetadataProvider for Offline {
        async fn lookup_isbn(&self, _i: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            Err(AppError::Network("соединение закрыто".into()))
        }
        async fn lookup_title(&self, _t: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            Err(AppError::Network("соединение закрыто".into()))
        }
        fn name(&self) -> &'static str { "offline" }
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
        assert_eq!(r2.duplicate_at, vec!["Полка".to_string()]);
        // всё равно вторая запись создана
        let count = books::on_shelf(&conn.lock().unwrap(), shelf).unwrap().len();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn duplicate_on_another_shelf_is_reported_with_its_place() {
        let (conn, shelf) = setup();
        let other = {
            let guard = conn.lock().unwrap();
            let root: i64 = guard
                .query_row("SELECT id FROM locations WHERE kind='root'", [], |r| r.get(0))
                .unwrap();
            locations::create(&guard, Some(root), "Кабинет", "shelf", None).unwrap().id
        };
        let svc = MetadataService::new(vec![Box::new(Mock(vec![cand()]))]);
        capture_scan(&conn, &svc, other, "9785171183660").await.unwrap();

        // сканируем на ДРУГУЮ полку — прежняя проверка тут молчала
        let r = capture_scan(&conn, &svc, shelf, "9785171183660").await.unwrap();
        assert!(r.is_possible_duplicate, "дубль на соседней полке не замечен");
        assert_eq!(r.duplicate_at, vec!["Кабинет".to_string()]);
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
    async fn network_failure_still_shelves_the_book_and_explains_why() {
        let (conn, shelf) = setup();
        let svc = MetadataService::new(vec![Box::new(Offline)]);
        let r = capture_scan(&conn, &svc, shelf, "9785171183660").await.unwrap();
        assert_eq!(r.source, "none");
        assert_eq!(r.book.shelf_id, Some(shelf)); // поток сканирования не прерван
        assert!(r.note.is_some(), "причина сбоя должна дойти до ленты");
    }

    #[tokio::test]
    async fn clean_miss_leaves_no_note() {
        let (conn, shelf) = setup();
        let svc = MetadataService::new(vec![Box::new(Mock(vec![]))]);
        let r = capture_scan(&conn, &svc, shelf, "9785171183660").await.unwrap();
        assert_eq!(r.note, None);
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
