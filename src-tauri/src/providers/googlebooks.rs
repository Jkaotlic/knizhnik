use crate::domain::matching::MetadataCandidate;
use crate::error::AppError;
use crate::providers::{MetadataProvider, SharedApiKey};
use async_trait::async_trait;

pub fn parse_volumes_response(json: &str) -> Vec<MetadataCandidate> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let Some(items) = v.get("items").and_then(|i| i.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let info = item.get("volumeInfo")?;
            let title = info.get("title")?.as_str()?.to_string();
            let authors = info
                .get("authors")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|s| !s.is_empty());
            let year = info
                .get("publishedDate")
                .and_then(|d| d.as_str())
                .and_then(|d| d.get(0..4))
                .and_then(|y| y.parse::<i64>().ok());
            let publisher = info
                .get("publisher")
                .and_then(|p| p.as_str())
                .map(|s| s.to_string());
            let pages = info.get("pageCount").and_then(|p| p.as_i64());
            let language = info
                .get("language")
                .and_then(|l| l.as_str())
                .map(|s| s.to_string());
            let cover_url = info
                .get("imageLinks")
                .and_then(|l| l.get("thumbnail").or_else(|| l.get("smallThumbnail")))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            let isbn = info
                .get("industryIdentifiers")
                .and_then(|ids| ids.as_array())
                .and_then(|ids| {
                    ids.iter()
                        .find(|i| i.get("type").and_then(|t| t.as_str()) == Some("ISBN_13"))
                        .or_else(|| ids.first())
                })
                .and_then(|i| i.get("identifier"))
                .and_then(|i| i.as_str())
                .map(|s| s.to_string());
            Some(MetadataCandidate {
                title,
                authors,
                isbn,
                year,
                publisher,
                pages,
                language,
                cover_url,
                source: "google".into(),
            })
        })
        .collect()
}

pub struct GoogleBooks {
    client: reqwest::Client,
    api_key: SharedApiKey,
}

impl GoogleBooks {
    pub fn new(client: reqwest::Client, api_key: SharedApiKey) -> Self {
        Self { client, api_key }
    }

    async fn fetch(&self, q: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        // Параметры уходят через `query`: раньше название склеивалось в URL как
        // есть, и любой `&` или `#` в запросе ломал его.
        let mut params: Vec<(&str, &str)> =
            vec![("q", q), ("maxResults", "5"), ("country", "RU")];
        // ключ (если задан в настройках) снимает анонимный лимит 429 и расширяет покрытие
        let key = self
            .api_key
            .read()
            .map_err(|_| AppError::Rule("Не прочитать API-ключ".into()))?
            .clone()
            .filter(|s| !s.is_empty());
        if let Some(k) = key.as_deref() {
            params.push(("key", k));
        }
        let resp = self
            .client
            .get("https://www.googleapis.com/books/v1/volumes")
            .query(&params)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 429 {
            return Err(AppError::Network(
                "Google Books: превышен лимит запросов. Добавь свой API-ключ в Настройках".into(),
            ));
        }
        if !status.is_success() {
            return Err(AppError::Network(format!("Google Books ответил {status}")));
        }
        let body = resp.text().await.map_err(|e| AppError::Network(e.to_string()))?;
        Ok(parse_volumes_response(&body))
    }
}

#[async_trait]
impl MetadataProvider for GoogleBooks {
    async fn lookup_isbn(&self, isbn: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        self.fetch(&format!("isbn:{isbn}")).await
    }
    async fn lookup_title(&self, title: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        self.fetch(title).await
    }
    fn name(&self) -> &'static str {
        "google"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_volume_into_candidate() {
        let json = include_str!("../../tests/fixtures/google_isbn.json");
        let out = parse_volumes_response(json);
        assert_eq!(out.len(), 1);
        let c = &out[0];
        assert_eq!(c.title, "Дюна");
        assert_eq!(c.authors.as_deref(), Some("Фрэнк Герберт"));
        assert_eq!(c.year, Some(2019));
        assert_eq!(c.publisher.as_deref(), Some("АСТ"));
        assert_eq!(c.pages, Some(704));
        assert_eq!(c.language.as_deref(), Some("ru"));
        assert_eq!(c.cover_url.as_deref(), Some("http://books.google.com/cover.jpg"));
        assert_eq!(c.isbn.as_deref(), Some("9785171183660"));
        assert_eq!(c.source, "google");
    }

    #[test]
    fn empty_yields_empty() {
        let json = include_str!("../../tests/fixtures/google_empty.json");
        assert!(parse_volumes_response(json).is_empty());
    }
}
