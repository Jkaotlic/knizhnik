use crate::domain::matching::{self, MetadataCandidate};
use crate::error::AppError;
use async_trait::async_trait;

pub mod googlebooks;
pub mod openlibrary;
pub mod sru;
pub mod wildberries;

/// Разделяемый API-ключ (напр. Google Books), обновляемый из настроек в рантайме.
pub type SharedApiKey = std::sync::Arc<std::sync::RwLock<Option<String>>>;

#[async_trait]
pub trait MetadataProvider: Send + Sync {
    async fn lookup_isbn(&self, isbn: &str) -> Result<Vec<MetadataCandidate>, AppError>;
    async fn lookup_title(&self, title: &str) -> Result<Vec<MetadataCandidate>, AppError>;
    fn name(&self) -> &'static str;
}

pub struct MetadataService {
    /// Опрашиваются одновременно, ответы склеиваются.
    providers: Vec<Box<dyn MetadataProvider>>,
    /// Спрашиваются, только когда основные ничего не нашли. Сюда попадают
    /// источники, которые нельзя дёргать на каждый запрос — например, витрины
    /// маркетплейсов с антиботом.
    fallback: Vec<Box<dyn MetadataProvider>>,
}

impl MetadataService {
    pub fn new(providers: Vec<Box<dyn MetadataProvider>>) -> Self {
        Self { providers, fallback: Vec::new() }
    }

    pub fn with_fallback(mut self, fallback: Vec<Box<dyn MetadataProvider>>) -> Self {
        self.fallback = fallback;
        self
    }

    /// Все каталоги описывают одну и ту же книгу, поэтому ответы не
    /// конкурируют, а дополняют друг друга: обложка от одного, страницы
    /// от второго, год от третьего. Раньше побеждал первый ответивший —
    /// и Open Library, отвечающая почти всегда, закрывала собой остальных.
    pub async fn lookup_isbn(&self, isbn: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        let (found, outcome) = self.gather(|p| p.lookup_isbn(isbn), &self.providers).await;
        if let Some(merged) = matching::merge_same_book(&found) {
            if !matching::is_thin(&merged) {
                return Ok(vec![merged]);
            }
        }
        // Дошли сюда — либо не нашли ничего, либо нашли пустышку (Open Library
        // на русские книги отдаёт название в романизации и больше ничего).
        // Спрашиваем запасной источник и склеиваем с тем, что уже есть.
        let (spare, spare_outcome) = self.gather(|p| p.lookup_isbn(isbn), &self.fallback).await;
        let mut all = found;
        all.extend(spare);
        if let Some(merged) = matching::merge_same_book(&all) {
            return Ok(vec![merged]);
        }
        outcome.or(spare_outcome).finish()
    }

    pub async fn lookup_title(&self, title: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        let (found, outcome) = self.gather(|p| p.lookup_title(title), &self.providers).await;
        if found.iter().any(|c| !matching::is_thin(c)) {
            return Ok(matching::merge_title_results(found));
        }
        let (spare, spare_outcome) = self.gather(|p| p.lookup_title(title), &self.fallback).await;
        let mut all = found;
        all.extend(spare);
        if !all.is_empty() {
            return Ok(matching::merge_title_results(all));
        }
        outcome.or(spare_outcome).finish()
    }

    async fn gather<'a, F, Fut>(
        &'a self,
        call: F,
        from: &'a [Box<dyn MetadataProvider>],
    ) -> (Vec<MetadataCandidate>, Outcome)
    where
        F: Fn(&'a dyn MetadataProvider) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<MetadataCandidate>, AppError>> + 'a,
    {
        let results = futures_util::future::join_all(from.iter().map(|p| {
            let name = p.name();
            let fut = call(p.as_ref());
            async move { (name, fut.await) }
        }))
        .await;

        let mut outcome = Outcome::default();
        let mut all = Vec::new();
        for (name, result) in results {
            match result {
                Ok(c) if !c.is_empty() => all.extend(c),
                Ok(_) => outcome.answered = true,
                Err(e) => outcome.last_err = Some(with_source(name, e)),
            }
        }
        (all, outcome)
    }
}

/// Каталоги опрашиваются разом, поэтому голое «error sending request» не
/// говорит, кто именно отвалился. Подписываем — если провайдер не назвался сам.
fn with_source(name: &str, e: AppError) -> AppError {
    match e {
        AppError::Network(msg) if !msg.to_lowercase().contains(&name.to_lowercase()) => {
            AppError::Network(format!("{name}: {msg}"))
        }
        other => other,
    }
}

/// Раньше упавший провайдер молча превращался в пустой список, и пользователь
/// без интернета читал «ничего не нашлось». Пустой ответ и сбой — разные вещи:
/// ошибку показываем, только если ни один провайдер не ответил внятно.
#[derive(Default)]
struct Outcome {
    answered: bool,
    last_err: Option<AppError>,
}

impl Outcome {
    /// Складывает итоги основного и запасного круга: внятный ответ хотя бы
    /// от одного каталога снимает ошибку остальных.
    fn or(mut self, other: Outcome) -> Self {
        self.answered |= other.answered;
        self.last_err = self.last_err.or(other.last_err);
        self
    }

    fn finish(self) -> Result<Vec<MetadataCandidate>, AppError> {
        match self.last_err {
            Some(e) if !self.answered => Err(e),
            _ => Ok(Vec::new()),
        }
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
        let out = svc.lookup_isbn("9785171183660").await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, "openlibrary");
    }

    #[tokio::test]
    async fn falls_back_when_first_empty() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Ok(vec![]) }),
            Box::new(Mock { name: "google", result: Ok(vec![cand("google")]) }),
        ]);
        let out = svc.lookup_isbn("9785171183660").await.unwrap();
        assert_eq!(out[0].source, "google");
    }

    #[tokio::test]
    async fn skips_erroring_provider() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Err(()) }),
            Box::new(Mock { name: "google", result: Ok(vec![cand("google")]) }),
        ]);
        let out = svc.lookup_isbn("9785171183660").await.unwrap();
        assert_eq!(out[0].source, "google");
    }

    #[tokio::test]
    async fn both_empty_returns_empty() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Ok(vec![]) }),
            Box::new(Mock { name: "google", result: Ok(vec![]) }),
        ]);
        assert!(svc.lookup_isbn("9785171183660").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn all_providers_failing_surfaces_the_error() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Err(()) }),
            Box::new(Mock { name: "google", result: Err(()) }),
        ]);
        // «нет сети» не должно выглядеть как «книги не существует»
        assert!(matches!(
            svc.lookup_isbn("9785171183660").await,
            Err(AppError::Network(_))
        ));
    }

    /// Мок, который помнит, спрашивали ли его.
    struct Spy {
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        result: Vec<MetadataCandidate>,
    }

    #[async_trait]
    impl MetadataProvider for Spy {
        async fn lookup_isbn(&self, _i: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.result.clone())
        }
        async fn lookup_title(&self, _t: &str) -> Result<Vec<MetadataCandidate>, AppError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.result.clone())
        }
        fn name(&self) -> &'static str {
            "spy"
        }
    }

    fn counter() -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))
    }
    fn calls(c: &std::sync::Arc<std::sync::atomic::AtomicUsize>) -> usize {
        c.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[tokio::test]
    async fn isbn_lookup_merges_fields_from_every_catalogue() {
        let mut with_cover = cand("openlibrary");
        with_cover.cover_url = Some("http://x/c.jpg".into());
        let mut with_pages = cand("dnb");
        with_pages.pages = Some(480);
        with_pages.publisher = Some("АСТ".into());

        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Ok(vec![with_cover]) }),
            Box::new(Mock { name: "dnb", result: Ok(vec![with_pages]) }),
        ]);
        let out = svc.lookup_isbn("9785171183660").await.unwrap();
        assert_eq!(out.len(), 1, "по одному ISBN должна выйти одна склеенная книга");
        // раньше побеждал первый ответивший, и страницы с издателем терялись
        assert_eq!(out[0].cover_url.as_deref(), Some("http://x/c.jpg"));
        assert_eq!(out[0].pages, Some(480));
        assert_eq!(out[0].publisher.as_deref(), Some("АСТ"));
    }

    #[tokio::test]
    async fn fallback_is_left_alone_when_the_main_catalogues_answered() {
        // «Ответили» — значит дали что-то содержательное, а не одно название:
        // на голую пустышку запасной источник как раз нужен.
        let mut useful = cand("openlibrary");
        useful.authors = Some("Фрэнк Герберт".into());
        useful.year = Some(1965);

        let spare = counter();
        let svc = MetadataService::new(vec![Box::new(Mock {
            name: "openlibrary",
            result: Ok(vec![useful]),
        })])
        .with_fallback(vec![Box::new(Spy { calls: spare.clone(), result: vec![] })]);

        svc.lookup_isbn("9785171183660").await.unwrap();
        // витрину с антиботом нельзя дёргать без нужды
        assert_eq!(calls(&spare), 0);
    }

    /// Регрессия на реальный случай: по ISBN «Будущего» Open Library отдаёт
    /// «Bydyshee» и ничего больше. Формально непусто — и запасной источник
    /// раньше не спрашивали, так что в каталог попадала романизация.
    #[tokio::test]
    async fn a_title_only_stub_still_triggers_the_fallback_and_gets_completed() {
        let mut stub = cand("openlibrary");
        stub.title = "Bydyshee".into();
        stub.authors = None;
        stub.year = None;
        stub.publisher = None;
        stub.pages = None;
        stub.cover_url = None;

        let mut real = cand("wildberries");
        real.title = "Будущее".into();
        real.publisher = Some("АСТ".into());
        real.authors = None;
        real.year = None;
        real.pages = None;
        real.cover_url = None;

        let spare = counter();
        let svc = MetadataService::new(vec![Box::new(Mock {
            name: "openlibrary",
            result: Ok(vec![stub]),
        })])
        .with_fallback(vec![Box::new(Spy { calls: spare.clone(), result: vec![real] })]);

        let out = svc.lookup_isbn("9785171183660").await.unwrap();
        assert_eq!(calls(&spare), 1, "пустышку надо дополнять запасным источником");
        assert_eq!(out[0].title, "Будущее", "романизация не должна победить настоящее название");
        assert_eq!(out[0].publisher.as_deref(), Some("АСТ"));
    }

    #[tokio::test]
    async fn fallback_saves_the_day_when_nobody_else_knows_the_book() {
        let spare = counter();
        let svc = MetadataService::new(vec![Box::new(Mock { name: "openlibrary", result: Ok(vec![]) })])
            .with_fallback(vec![Box::new(Spy {
                calls: spare.clone(),
                result: vec![cand("wildberries")],
            })]);

        let out = svc.lookup_isbn("9785171183660").await.unwrap();
        assert_eq!(calls(&spare), 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, "wildberries");
    }

    #[tokio::test]
    async fn title_search_combines_results_instead_of_stopping_at_the_first() {
        let mut a = cand("openlibrary");
        a.title = "Дюна".into();
        let mut b = cand("loc");
        b.title = "Мессия Дюны".into();

        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Ok(vec![a]) }),
            Box::new(Mock { name: "loc", result: Ok(vec![b]) }),
        ]);
        let out = svc.lookup_title("дюна").await.unwrap();
        // прежняя логика вернула бы только ответ Open Library
        assert_eq!(out.len(), 2);
        let titles: Vec<&str> = out.iter().map(|c| c.title.as_str()).collect();
        assert!(titles.contains(&"Дюна") && titles.contains(&"Мессия Дюны"));
    }

    /// Живая проверка всей цепочки против настоящих каталогов.
    /// Помечен `ignore`, чтобы сеть не попадала в обычный прогон:
    /// `cargo test -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "ходит в сеть"]
    async fn live_isbn_lookup_through_the_real_chain() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(concat!("knizhnik/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap();
        let key: SharedApiKey = std::sync::Arc::new(std::sync::RwLock::new(None));
        let svc = MetadataService::new(vec![
            Box::new(crate::providers::openlibrary::OpenLibrary::new(client.clone())),
            Box::new(crate::providers::sru::Sru::dnb(client.clone())),
            Box::new(crate::providers::sru::Sru::loc(client.clone())),
            Box::new(crate::providers::googlebooks::GoogleBooks::new(client.clone(), key)),
        ])
        .with_fallback(vec![Box::new(crate::providers::wildberries::Wildberries::new(
            client.clone(),
        ))]);

        for (isbn, what) in [
            ("9785171183660", "русская (Глуховский, Будущее)"),
            ("9780441013593", "английская (Herbert, Dune)"),
            ("9783453317246", "немецкая (Robinson, Aurora)"),
        ] {
            match svc.lookup_isbn(isbn).await {
                Ok(v) if v.is_empty() => println!("\n{what}: НИЧЕГО НЕ НАЙДЕНО"),
                Ok(v) => {
                    let c = &v[0];
                    println!(
                        "\n{what}\n  источники: {}\n  название:  {}\n  автор:     {:?}\n  \
                         год: {:?}  издатель: {:?}  стр.: {:?}  язык: {:?}  обложка: {}",
                        c.source, c.title, c.authors, c.year, c.publisher, c.pages, c.language,
                        if c.cover_url.is_some() { "есть" } else { "нет" }
                    );
                }
                Err(e) => println!("\n{what}: ОШИБКА {e}"),
            }
        }
    }

    #[tokio::test]
    async fn one_failure_plus_one_clean_miss_is_still_a_miss() {
        let svc = MetadataService::new(vec![
            Box::new(Mock { name: "openlibrary", result: Err(()) }),
            Box::new(Mock { name: "google", result: Ok(vec![]) }),
        ]);
        assert!(svc.lookup_isbn("9785171183660").await.unwrap().is_empty());
    }
}
