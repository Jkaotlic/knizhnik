use crate::domain::matching::MetadataCandidate;
use crate::error::AppError;
use async_trait::async_trait;

pub mod googlebooks;
pub mod openlibrary;

/// Разделяемый API-ключ (напр. Google Books), обновляемый из настроек в рантайме.
pub type SharedApiKey = std::sync::Arc<std::sync::RwLock<Option<String>>>;

#[async_trait]
pub trait MetadataProvider: Send + Sync {
    async fn lookup_isbn(&self, isbn: &str) -> Result<Vec<MetadataCandidate>, AppError>;
    async fn lookup_title(&self, title: &str) -> Result<Vec<MetadataCandidate>, AppError>;
    fn name(&self) -> &'static str;
}

pub struct MetadataService {
    providers: Vec<Box<dyn MetadataProvider>>,
}

impl MetadataService {
    pub fn new(providers: Vec<Box<dyn MetadataProvider>>) -> Self {
        Self { providers }
    }

    pub async fn lookup_isbn(&self, isbn: &str) -> Vec<MetadataCandidate> {
        for p in &self.providers {
            if let Ok(c) = p.lookup_isbn(isbn).await {
                if !c.is_empty() {
                    return c;
                }
            }
        }
        Vec::new()
    }

    pub async fn lookup_title(&self, title: &str) -> Vec<MetadataCandidate> {
        for p in &self.providers {
            if let Ok(c) = p.lookup_title(title).await {
                if !c.is_empty() {
                    return c;
                }
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mock {
        name: &'static str,
        result: Result<Vec<MetadataCandidate>, ()>,
    }

    #[async_trait]
    impl MetadataProvider for Mock {
        async fn lookup_isbn(&self, _isbn: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            self.result.clone().map_err(|_| AppError::Network("mock".into()))
        }
        async fn lookup_title(&self, _t: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            self.result.clone().map_err(|_| AppError::Network("mock".into()))
        }
        fn name(&self) -> &'static str {
            self.name
        }
    }

    fn cand(source: &str) -> MetadataCandidate {
        MetadataCandidate {
            title: "Дюна".into(),
            authors: None, isbn: None, year: None, publisher: None,
            pages: None, language: None, cover_url: None, source: source.into(),
        }
    }

    #[tokio::test]
    async fn returns_first_nonempty_provider() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Ok(vec![cand("openlibrary")]) }),
            Box::new(Mock { name: "google", result: Ok(vec![cand("google")]) }),
        ]);
        let out = svc.lookup_isbn("9785171183660").await;
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, "openlibrary");
    }

    #[tokio::test]
    async fn falls_back_when_first_empty() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Ok(vec![]) }),
            Box::new(Mock { name: "google", result: Ok(vec![cand("google")]) }),
        ]);
        let out = svc.lookup_isbn("9785171183660").await;
        assert_eq!(out[0].source, "google");
    }

    #[tokio::test]
    async fn skips_erroring_provider() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Err(()) }),
            Box::new(Mock { name: "google", result: Ok(vec![cand("google")]) }),
        ]);
        let out = svc.lookup_isbn("9785171183660").await;
        assert_eq!(out[0].source, "google");
    }

    #[tokio::test]
    async fn both_empty_returns_empty() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Ok(vec![]) }),
            Box::new(Mock { name: "google", result: Ok(vec![]) }),
        ]);
        assert!(svc.lookup_isbn("9785171183660").await.is_empty());
    }
}
