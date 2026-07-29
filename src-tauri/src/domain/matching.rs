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

/// Сколько полезных полей заполнено. Чем больше — тем ценнее кандидат.
pub fn completeness(c: &MetadataCandidate) -> usize {
    [
        c.authors.is_some(),
        c.cover_url.is_some(),
        c.year.is_some(),
        c.publisher.is_some(),
        c.pages.is_some(),
        c.isbn.is_some(),
        c.language.is_some(),
    ]
    .iter()
    .filter(|b| **b)
    .count()
}

/// Одно название и больше ничего. ISBN намеренно не считается: при поиске по
/// ISBN мы его и так знаем, он не признак того, что каталог что-то нашёл.
///
/// Такие пустышки Open Library отдаёт на русские книги — название в
/// романизации ALA-LC («Bydyshee» вместо «Будущее») и все поля пустые.
/// Формально это «нашли», по сути — нет, поэтому запасной источник должен
/// включаться и здесь.
pub fn is_thin(c: &MetadataCandidate) -> bool {
    c.authors.is_none() && c.year.is_none() && c.publisher.is_none() && c.pages.is_none()
}

pub fn pick_best(candidates: &[MetadataCandidate]) -> Option<&MetadataCandidate> {
    candidates
        .iter()
        .enumerate()
        // при равенстве score — меньший индекс выигрывает
        .max_by_key(|(idx, c)| (completeness(c), std::cmp::Reverse(*idx)))
        .map(|(_, c)| c)
}

/// Все кандидаты по одному ISBN описывают одну и ту же книгу, просто разные
/// каталоги знают разное: у Open Library бывает обложка, у DNB — страницы и
/// издатель, у Google — всё сразу. Склеиваем в одну запись, чтобы форма
/// заполнилась настолько, насколько вообще возможно.
pub fn merge_same_book(candidates: &[MetadataCandidate]) -> Option<MetadataCandidate> {
    let best = pick_best(candidates)?;
    let mut merged = best.clone();
    let mut sources: Vec<&str> = vec![best.source.as_str()];

    for c in candidates {
        let mut used = false;
        // макрос не нужен — полей немного, зато видно, что происходит
        if merged.authors.is_none() && c.authors.is_some() {
            merged.authors = c.authors.clone();
            used = true;
        }
        if merged.isbn.is_none() && c.isbn.is_some() {
            merged.isbn = c.isbn.clone();
            used = true;
        }
        if merged.year.is_none() && c.year.is_some() {
            merged.year = c.year;
            used = true;
        }
        if merged.publisher.is_none() && c.publisher.is_some() {
            merged.publisher = c.publisher.clone();
            used = true;
        }
        if merged.pages.is_none() && c.pages.is_some() {
            merged.pages = c.pages;
            used = true;
        }
        if merged.language.is_none() && c.language.is_some() {
            merged.language = c.language.clone();
            used = true;
        }
        if merged.cover_url.is_none() && c.cover_url.is_some() {
            merged.cover_url = c.cover_url.clone();
            used = true;
        }
        if used && !sources.contains(&c.source.as_str()) {
            sources.push(c.source.as_str());
        }
    }
    // в карточке будет видно, что данные собраны из нескольких каталогов
    merged.source = sources.join("+");
    Some(merged)
}

/// Ключ для склейки одинаковых книг в выдаче по названию: регистр и пробелы
/// у разных каталогов пишутся по-разному.
fn dedupe_key(c: &MetadataCandidate) -> String {
    let norm = |s: &str| s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}|{}", norm(&c.title), c.authors.as_deref().map(norm).unwrap_or_default())
}

/// Выдача по названию из нескольких каталогов: одинаковые книги схлопываем
/// (забирая лучшие поля), остальные сортируем от самых полных к пустым.
pub fn merge_title_results(candidates: Vec<MetadataCandidate>) -> Vec<MetadataCandidate> {
    let mut groups: Vec<(String, Vec<MetadataCandidate>)> = Vec::new();
    for c in candidates {
        let key = dedupe_key(&c);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, group)) => group.push(c),
            None => groups.push((key, vec![c])),
        }
    }
    let mut out: Vec<MetadataCandidate> =
        groups.iter().filter_map(|(_, g)| merge_same_book(g)).collect();
    // сортировка устойчивая — при равной полноте порядок провайдеров сохраняется
    out.sort_by_key(|c| std::cmp::Reverse(completeness(c)));
    out
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

    #[test]
    fn merge_collects_fields_scattered_across_catalogues() {
        // ровно тот случай, ради которого всё затевалось: Open Library знает
        // обложку, DNB — страницы и издателя, Google — год
        let mut ol = bare("Будущее", "openlibrary");
        ol.cover_url = Some("http://x/c.jpg".into());
        let mut dnb = bare("Будущее", "dnb");
        dnb.pages = Some(480);
        dnb.publisher = Some("АСТ".into());
        dnb.authors = Some("Дмитрий Глуховский".into());
        let mut google = bare("Будущее", "google");
        google.year = Some(2019);

        let m = merge_same_book(&[ol, dnb, google]).unwrap();
        assert_eq!(m.cover_url.as_deref(), Some("http://x/c.jpg"));
        assert_eq!(m.pages, Some(480));
        assert_eq!(m.publisher.as_deref(), Some("АСТ"));
        assert_eq!(m.year, Some(2019));
        assert_eq!(m.authors.as_deref(), Some("Дмитрий Глуховский"));
        assert!(m.source.contains('+'), "источник должен показывать склейку: {}", m.source);
    }

    #[test]
    fn merge_prefers_the_richest_candidate_and_never_overwrites_it() {
        let mut rich = bare("Будущее", "google");
        rich.publisher = Some("АСТ".into());
        rich.year = Some(2019);
        let mut poor = bare("Будущее", "wildberries");
        poor.publisher = Some("Издательство АСТ".into()); // хуже, не должно победить

        let m = merge_same_book(&[poor, rich]).unwrap();
        assert_eq!(m.publisher.as_deref(), Some("АСТ"));
    }

    #[test]
    fn merge_of_nothing_is_none() {
        assert!(merge_same_book(&[]).is_none());
    }

    #[test]
    fn title_results_dedupe_across_catalogues_and_rank_by_completeness() {
        let thin = bare("Дюна", "openlibrary");
        let mut same_but_rich = bare("  дюна  ", "dnb"); // другой регистр и пробелы
        same_but_rich.publisher = Some("АСТ".into());
        same_but_rich.pages = Some(704);
        let other = bare("Мессия Дюны", "loc");

        let out = merge_title_results(vec![thin, other, same_but_rich]);
        assert_eq!(out.len(), 2, "одинаковые книги не схлопнулись: {out:?}");
        // самая полная — первой
        assert_eq!(out[0].pages, Some(704));
        assert_eq!(out[1].title, "Мессия Дюны");
    }

    #[test]
    fn different_authors_are_not_merged() {
        let mut a = bare("Дюна", "loc");
        a.authors = Some("Фрэнк Герберт".into());
        let mut b = bare("Дюна", "dnb");
        b.authors = Some("Брайан Герберт".into());
        assert_eq!(merge_title_results(vec![a, b]).len(), 2);
    }
}
