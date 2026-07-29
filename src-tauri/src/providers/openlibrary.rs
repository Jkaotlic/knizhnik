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

impl OpenLibrary {
    /// Параметры уходят через `query`, а не склейкой строк: название с `&`,
    /// `#` или кириллицей раньше превращалось в битый URL.
    async fn fetch(&self, url: &str, params: &[(&str, &str)]) -> Result<String, AppError> {
        let resp = self
            .client
            .get(url)
            .query(params)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(AppError::Network(format!("Open Library ответил {status}")));
        }
        resp.text().await.map_err(|e| AppError::Network(e.to_string()))
    }
}

#[async_trait]
impl MetadataProvider for OpenLibrary {
    async fn lookup_isbn(&self, isbn: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        let bibkeys = format!("ISBN:{isbn}");
        let body = self
            .fetch(
                "https://openlibrary.org/api/books",
                &[("bibkeys", &bibkeys), ("format", "json"), ("jscmd", "data")],
            )
            .await?;
        Ok(parse_isbn_response(&body, isbn))
    }

    async fn lookup_title(&self, title: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        // Без `fields` ответ приходит урезанным — ни обложки, ни числа страниц.
        let body = self
            .fetch(
                "https://openlibrary.org/search.json",
                &[
                    ("title", title),
                    ("limit", "5"),
                    ("fields", "title,author_name,first_publish_year,number_of_pages_median,cover_i"),
                ],
            )
            .await?;
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
                    // Обложка и медиана страниц описывают произведение целиком —
                    // их брать честно. А publisher/language/isbn в этом ответе
                    // сложены по ВСЕМ изданиям сразу (у «Дюны» — 48 издателей,
                    // 140 ISBN, и первый язык вообще польский), поэтому «взять
                    // первый» означало бы тихо записать чужое издание.
                    // Эти поля дозаполнят DNB, LoC и Google — они уровня издания.
                    let cover_url = d
                        .get("cover_i")
                        .and_then(|c| c.as_i64())
                        .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-M.jpg"));
                    Some(MetadataCandidate {
                        title,
                        authors,
                        isbn: None,
                        year: d.get("first_publish_year").and_then(|y| y.as_i64()),
                        publisher: None,
                        pages: d.get("number_of_pages_median").and_then(|p| p.as_i64()),
                        language: None,
                        cover_url,
                        source: "openlibrary".into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
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

    #[test]
    fn title_search_takes_work_level_fields() {
        let json = r#"{"docs":[{
            "title":"Dune","author_name":["Frank Herbert"],"first_publish_year":1965,
            "number_of_pages_median":535,"cover_i":8100921
        }]}"#;
        let c = &parse_search_response(json)[0];
        assert_eq!(c.title, "Dune");
        assert_eq!(c.authors.as_deref(), Some("Frank Herbert"));
        assert_eq!(c.year, Some(1965));
        assert_eq!(c.pages, Some(535)); // медиана по изданиям — величина осмысленная
        assert_eq!(
            c.cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/8100921-M.jpg")
        );
    }

    /// Поиск по названию отвечает на уровне произведения: publisher, language
    /// и isbn там сложены по всем изданиям сразу. У настоящей «Дюны» это 48
    /// издателей и первый язык `pol` — записать такое в карточку значит
    /// подсунуть человеку чужое издание.
    #[test]
    fn cross_edition_aggregates_are_deliberately_ignored() {
        let json = r#"{"docs":[{
            "title":"Children of Dune","author_name":["Frank Herbert"],
            "publisher":["Orion Publishing Group, Limited","Ace","Гослитиздат"],
            "language":["pol","eng","rus"],
            "isbn":["0425071790","9780441013593"]
        }]}"#;
        let c = &parse_search_response(json)[0];
        assert_eq!(c.publisher, None, "издатель случайного издания не должен попасть в карточку");
        assert_eq!(c.language, None, "первый язык из списка — польский, это не язык книги");
        assert_eq!(c.isbn, None, "ISBN чужого издания хуже, чем пустое поле");
    }

    #[test]
    fn title_search_survives_a_response_with_only_a_title() {
        let json = r#"{"docs":[{"title":"Голая запись"}]}"#;
        let c = &parse_search_response(json)[0];
        assert_eq!(c.title, "Голая запись");
        assert_eq!(c.publisher, None);
        assert_eq!(c.cover_url, None);
    }
}
