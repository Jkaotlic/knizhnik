use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataCandidate {
    pub title: String,
    pub authors: Option<String>,
    pub isbn: Option<String>,
    pub year: Option<i64>,
    pub publisher: Option<String>,
    pub pages: Option<i64>,
    pub language: Option<String>,
    pub cover_url: Option<String>,
    pub source: String,
}

pub fn pick_best(candidates: &[MetadataCandidate]) -> Option<&MetadataCandidate> {
    candidates
        .iter()
        .enumerate()
        .max_by_key(|(idx, c)| {
            let score = [
                c.authors.is_some(),
                c.cover_url.is_some(),
                c.year.is_some(),
                c.publisher.is_some(),
                c.pages.is_some(),
            ]
            .iter()
            .filter(|b| **b)
            .count();
            // при равенстве score — меньший индекс выигрывает
            (score, std::cmp::Reverse(*idx))
        })
        .map(|(_, c)| c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare(title: &str, source: &str) -> MetadataCandidate {
        MetadataCandidate {
            title: title.into(),
            authors: None,
            isbn: None,
            year: None,
            publisher: None,
            pages: None,
            language: None,
            cover_url: None,
            source: source.into(),
        }
    }

    #[test]
    fn empty_gives_none() {
        assert!(pick_best(&[]).is_none());
    }

    #[test]
    fn prefers_more_complete_candidate() {
        let sparse = bare("Дюна", "google");
        let mut rich = bare("Дюна", "openlibrary");
        rich.authors = Some("Фрэнк Герберт".into());
        rich.cover_url = Some("http://x/c.jpg".into());
        rich.year = Some(2019);
        let list = vec![sparse, rich.clone()];
        assert_eq!(pick_best(&list).unwrap(), &rich);
    }

    #[test]
    fn tie_returns_first() {
        let a = bare("A", "openlibrary");
        let b = bare("B", "google");
        let list = vec![a.clone(), b];
        assert_eq!(pick_best(&list).unwrap(), &a);
    }
}
