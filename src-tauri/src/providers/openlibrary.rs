use crate::domain::matching::MetadataCandidate;
use crate::error::AppError;
use crate::providers::MetadataProvider;
use async_trait::async_trait;

pub fn parse_isbn_response(json: &str, isbn: &str) -> Vec<MetadataCandidate> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let key = format!("ISBN:{isbn}");
    let Some(obj) = v.get(&key) else {
        return Vec::new();
    };
    let Some(title) = obj.get("title").and_then(|t| t.as_str()) else {
        return Vec::new();
    };
    let authors = obj
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get("name").and_then(|n| n.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty());
    let year = obj
        .get("publish_date")
        .and_then(|d| d.as_str())
        .and_then(|d| {
            d.split(|c: char| !c.is_ascii_digit())
                .filter(|p| p.len() == 4)
                .find_map(|p| p.parse::<i64>().ok())
        });
    let publisher = obj
        .get("publishers")
        .and_then(|p| p.as_array())
        .and_then(|p| p.first())
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let pages = obj.get("number_of_pages").and_then(|p| p.as_i64());
    let cover_url = obj
        .get("cover")
        .and_then(|c| c.get("medium").or_else(|| c.get("large")).or_else(|| c.get("small")))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());
    vec![MetadataCandidate {
        title: title.to_string(),
        authors,
        isbn: Some(isbn.to_string()),
        year,
        publisher,
        pages,
        language: None,
        cover_url,
        source: "openlibrary".into(),
    }]
}

pub struct OpenLibrary {
    client: reqwest::Client,
}

impl OpenLibrary {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MetadataProvider for OpenLibrary {
    async fn lookup_isbn(&self, isbn: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        let url = format!(
            "https://openlibrary.org/api/books?bibkeys=ISBN:{isbn}&format=json&jscmd=data"
        );
        let body = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?
            .text()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        Ok(parse_isbn_response(&body, isbn))
    }

    async fn lookup_title(&self, title: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        let url = format!(
            "https://openlibrary.org/search.json?title={}&limit=5",
            urlencoding_min(title)
        );
        let body = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?
            .text()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        Ok(parse_search_response(&body))
    }

    fn name(&self) -> &'static str {
        "openlibrary"
    }
}

pub fn parse_search_response(json: &str) -> Vec<MetadataCandidate> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    v.get("docs")
        .and_then(|d| d.as_array())
        .map(|docs| {
            docs.iter()
                .filter_map(|d| {
                    let title = d.get("title")?.as_str()?.to_string();
                    let authors = d
                        .get("author_name")
                        .and_then(|a| a.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .filter(|s| !s.is_empty());
                    Some(MetadataCandidate {
                        title,
                        authors,
                        isbn: None,
                        year: d.get("first_publish_year").and_then(|y| y.as_i64()),
                        publisher: None,
                        pages: None,
                        language: None,
                        cover_url: None,
                        source: "openlibrary".into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

// минимальный urlencode пробелов; для v1 достаточно
fn urlencoding_min(s: &str) -> String {
    s.replace(' ', "+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_isbn_fixture_into_candidate() {
        let json = include_str!("../../tests/fixtures/openlibrary_isbn.json");
        let out = parse_isbn_response(json, "9785171183660");
        assert_eq!(out.len(), 1);
        let c = &out[0];
        assert_eq!(c.title, "Дюна");
        assert_eq!(c.authors.as_deref(), Some("Фрэнк Герберт"));
        assert_eq!(c.year, Some(2019));
        assert_eq!(c.publisher.as_deref(), Some("АСТ"));
        assert_eq!(c.pages, Some(704));
        assert_eq!(c.cover_url.as_deref(), Some("https://covers.openlibrary.org/b/id/123-M.jpg"));
        assert_eq!(c.source, "openlibrary");
        assert_eq!(c.isbn.as_deref(), Some("9785171183660"));
    }

    #[test]
    fn empty_fixture_yields_empty() {
        let json = include_str!("../../tests/fixtures/openlibrary_empty.json");
        assert!(parse_isbn_response(json, "9785171183660").is_empty());
    }
}
