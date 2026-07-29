use crate::domain::matching::MetadataCandidate;
use crate::error::AppError;
use crate::providers::MetadataProvider;
use async_trait::async_trait;

// Единственный найденный источник с приличным покрытием русских книг, который
// отдаёт JSON, а не вёрстку. Но это витрина маркетплейса, а не библиография:
//
//  * знает только название и издательство — ни года, ни страниц, ни ISBN назад;
//  * по запросу «Метро 2033» первыми идут кулоны и брелоки, поэтому фильтруем
//    по категории «Книги»;
//  * при частых запросах вместо JSON прилетает HTML-заглушка антибота.
//
// Из-за последнего пункта провайдер подключён запасным: его спрашивают, только
// когда основные каталоги ничего не нашли.

/// subjectId категории «Книги» в каталоге Wildberries.
const BOOKS_SUBJECT: i64 = 381;

pub fn parse_search_response(json: &str) -> Vec<MetadataCandidate> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let products = v
        .get("data")
        .and_then(|d| d.get("products"))
        .or_else(|| v.get("products"))
        .and_then(|p| p.as_array());
    let Some(products) = products else {
        return Vec::new();
    };

    products
        .iter()
        .filter(|p| p.get("subjectId").and_then(|s| s.as_i64()) == Some(BOOKS_SUBJECT))
        .filter_map(|p| {
            let title = p.get("name")?.as_str()?.trim();
            if title.is_empty() {
                return None;
            }
            Some(MetadataCandidate {
                title: title.to_string(),
                authors: None,
                isbn: None,
                year: None,
                // «brand» на витрине книг — это издательство
                publisher: p
                    .get("brand")
                    .and_then(|b| b.as_str())
                    .map(clean_publisher)
                    .filter(|s| !s.is_empty()),
                pages: None,
                language: None,
                cover_url: None,
                source: "wildberries".into(),
            })
        })
        .collect()
}

/// «Издательство АСТ» и «АСТ Издательство» — одно и то же; слово-довесок
/// только мешает сравнивать и выглядит шумно в карточке.
fn clean_publisher(raw: &str) -> String {
    let s = raw.trim();
    let without = s
        .trim_start_matches("Издательство")
        .trim_end_matches("Издательство")
        .trim();
    if without.is_empty() { s.to_string() } else { without.to_string() }
}

pub struct Wildberries {
    client: reqwest::Client,
}

impl Wildberries {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    async fn search(&self, query: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        let resp = self
            .client
            .get("https://search.wb.ru/exactmatch/ru/common/v4/search")
            .query(&[
                ("appType", "1"),
                ("curr", "rub"),
                ("dest", "-1257786"),
                ("resultset", "catalog"),
                ("limit", "10"),
                ("query", query),
            ])
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Network(format!("Wildberries ответил {}", resp.status())));
        }
        let body = resp.text().await.map_err(|e| AppError::Network(e.to_string()))?;
        // Антибот отдаёт HTML с кодом 200. Это не «книга не найдена», а отказ —
        // сообщаем честно, иначе пользователь решит, что книги не существует.
        if body.trim_start().starts_with('<') {
            return Err(AppError::Network(
                "Wildberries временно отклонил запрос (слишком часто)".into(),
            ));
        }
        Ok(parse_search_response(&body))
    }
}

#[async_trait]
impl MetadataProvider for Wildberries {
    /// Поиском по ISBN витрина не занимается: на запрос `9785171183660` она
    /// возвращала то нужную книгу, то MacBook Pro, то заглушку антибота.
    /// Отвечаем «не знаю» вместо того, чтобы играть в рулетку и жечь лимит.
    async fn lookup_isbn(&self, _isbn: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        Ok(Vec::new())
    }

    async fn lookup_title(&self, title: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        self.search(title).await
    }

    fn name(&self) -> &'static str {
        "wildberries"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"{"data":{"products":[
        {"id":1,"name":"Будущее","brand":"АСТ Издательство","subjectId":381},
        {"id":2,"name":"Кулон Метро 2033","brand":"АнимеАкс","subjectId":298},
        {"id":3,"name":"Метро 2033","brand":"Издательство АСТ","subjectId":381}
    ]}}"#;

    #[test]
    fn keeps_books_and_drops_merchandise() {
        let out = parse_search_response(RESPONSE);
        // кулон из категории бижутерии выброшен
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Будущее");
        assert_eq!(out[1].title, "Метро 2033");
        assert!(out.iter().all(|c| c.source == "wildberries"));
    }

    #[test]
    fn publisher_is_normalised_both_ways() {
        let out = parse_search_response(RESPONSE);
        assert_eq!(out[0].publisher.as_deref(), Some("АСТ")); // «АСТ Издательство»
        assert_eq!(out[1].publisher.as_deref(), Some("АСТ")); // «Издательство АСТ»
    }

    #[test]
    fn marketplace_gives_nothing_beyond_title_and_publisher() {
        let c = &parse_search_response(RESPONSE)[0];
        assert_eq!(c.year, None);
        assert_eq!(c.pages, None);
        assert_eq!(c.isbn, None);
    }

    #[test]
    fn flat_products_shape_is_also_understood() {
        let flat = r#"{"products":[{"name":"Дюна","brand":"АСТ","subjectId":381}]}"#;
        assert_eq!(parse_search_response(flat).len(), 1);
    }

    #[test]
    fn antibot_html_and_garbage_yield_nothing_rather_than_panicking() {
        assert!(parse_search_response("<!DOCTYPE html><html>").is_empty());
        assert!(parse_search_response("").is_empty());
        assert!(parse_search_response(r#"{"data":{}}"#).is_empty());
    }

    #[tokio::test]
    async fn isbn_lookup_is_deliberately_a_no_op() {
        let wb = Wildberries::new(reqwest::Client::new());
        // без сети: провайдер обязан ответить пустотой, не делая запроса
        assert!(wb.lookup_isbn("9785171183660").await.unwrap().is_empty());
    }

    #[test]
    fn publisher_that_is_only_the_word_itself_is_kept_as_is() {
        let json = r#"{"products":[{"name":"Книга","brand":"Издательство","subjectId":381}]}"#;
        assert_eq!(parse_search_response(json)[0].publisher.as_deref(), Some("Издательство"));
    }
}
