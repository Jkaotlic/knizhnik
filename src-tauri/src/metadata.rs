use crate::db::cache;
use crate::domain::matching::MetadataCandidate;
use crate::error::AppError;
use crate::providers::MetadataService;
use std::sync::Mutex;

/// Поиск по ISBN через кэш. Замок берётся двумя короткими заходами —
/// до сети и после: держать `MutexGuard` через `.await` нельзя, он не Send,
/// и async-команда Tauri с ним просто не скомпилируется.
pub async fn lookup_isbn_cached(
    db: &Mutex<rusqlite::Connection>,
    svc: &MetadataService,
    isbn: &str,
) -> Result<Vec<MetadataCandidate>, AppError> {
    {
        let conn = lock(db)?;
        if let Some(hit) = cache::get(&conn, isbn)? {
            return Ok(hit);
        }
    }

    let fresh = svc.lookup_isbn(isbn).await?;

    {
        let conn = lock(db)?;
        cache::put(&conn, isbn, &fresh)?;
    }
    Ok(fresh)
}

pub fn lock(
    db: &Mutex<rusqlite::Connection>,
) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, AppError> {
    db.lock().map_err(|_| {
        AppError::Rule("Соединение с базой в нерабочем состоянии, перезапусти приложение".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::providers::MetadataProvider;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct Counting {
        calls: Arc<AtomicUsize>,
        result: Vec<MetadataCandidate>,
    }

    #[async_trait]
    impl MetadataProvider for Counting {
        async fn lookup_isbn(&self, _i: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.result.clone())
        }
        async fn lookup_title(&self, _t: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            Ok(self.result.clone())
        }
        fn name(&self) -> &'static str {
            "counting"
        }
    }

    fn cand() -> MetadataCandidate {
        MetadataCandidate {
            title: "Будущее".into(),
            authors: Some("Дмитрий Глуховский".into()),
            isbn: Some("9785171183660".into()),
            year: Some(2019),
            publisher: Some("АСТ".into()),
            pages: Some(480),
            language: Some("ru".into()),
            cover_url: None,
            source: "google".into(),
        }
    }

    #[tokio::test]
    async fn second_lookup_does_not_touch_the_network() {
        let db = Mutex::new(open_in_memory().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let svc = MetadataService::new(vec![Box::new(Counting {
            calls: calls.clone(),
            result: vec![cand()],
        })]);

        let a = lookup_isbn_cached(&db, &svc, "9785171183660").await.unwrap();
        let b = lookup_isbn_cached(&db, &svc, "9785171183660").await.unwrap();
        assert_eq!(a[0].title, "Будущее");
        assert_eq!(b[0].title, "Будущее");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "второй раз ушли в сеть");
    }

    #[tokio::test]
    async fn a_miss_is_retried_next_time() {
        let db = Mutex::new(open_in_memory().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let svc = MetadataService::new(vec![Box::new(Counting {
            calls: calls.clone(),
            result: vec![],
        })]);

        assert!(lookup_isbn_cached(&db, &svc, "9785171183660").await.unwrap().is_empty());
        assert!(lookup_isbn_cached(&db, &svc, "9785171183660").await.unwrap().is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 2, "промах залип в кэше");
    }
}
