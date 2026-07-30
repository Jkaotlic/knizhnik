use crate::domain::matching::MetadataCandidate;
use crate::error::AppError;
use crate::providers::MetadataProvider;
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;

// SRU — стандартный протокол библиотечных каталогов поверх обычного HTTP,
// без ключей и регистрации. Оба подключённых каталога отдают Dublin Core,
// только DNB с префиксом (`dc:title`), а LoC без него (`title` в дефолтном
// namespace), поэтому теги сопоставляем по локальному имени.

/// Поля одной записи Dublin Core. Повторяющиеся теги (авторы, идентификаторы)
/// копим списком, остальное — первое непустое значение.
#[derive(Default, Debug)]
struct DcRecord {
    title: Option<String>,
    creators: Vec<String>,
    publisher: Option<String>,
    date: Option<String>,
    language: Option<String>,
    format: Option<String>,
    identifiers: Vec<String>,
}

fn local_name(qname: &[u8]) -> String {
    let name = String::from_utf8_lossy(qname);
    name.rsplit(':').next().unwrap_or("").to_string()
}

pub fn parse_dc_records(xml: &str, source: &str) -> Vec<MetadataCandidate> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut records: Vec<DcRecord> = Vec::new();
    let mut current: Option<DcRecord> = None;
    let mut field: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = local_name(e.name().as_ref());
                match tag.as_str() {
                    // и `<srw_dc:dc>` (LoC), и `<dc>` (DNB) — начало записи
                    "dc" => current = Some(DcRecord::default()),
                    _ if current.is_some() => field = Some(tag),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let tag = local_name(e.name().as_ref());
                if tag == "dc" {
                    if let Some(rec) = current.take() {
                        records.push(rec);
                    }
                } else if field.as_deref() == Some(tag.as_str()) {
                    field = None;
                }
            }
            Ok(Event::Text(t)) => {
                let (Some(rec), Some(f)) = (current.as_mut(), field.as_deref()) else {
                    continue;
                };
                // unescape разворачивает &amp; и &#252; — без этого немецкие
                // и русские заголовки приезжают побитыми
                let Ok(value) = t.unescape() else { continue };
                let value = value.trim().to_string();
                if value.is_empty() {
                    continue;
                }
                match f {
                    "title" => rec.title.get_or_insert(value),
                    "creator" | "contributor" => {
                        rec.creators.push(value);
                        continue;
                    }
                    "publisher" => rec.publisher.get_or_insert(value),
                    "date" => rec.date.get_or_insert(value),
                    "language" => rec.language.get_or_insert(value),
                    "format" | "extent" => rec.format.get_or_insert(value),
                    "identifier" => {
                        rec.identifiers.push(value);
                        continue;
                    }
                    _ => continue,
                };
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    records.into_iter().filter_map(|r| to_candidate(r, source)).collect()
}

fn to_candidate(r: DcRecord, source: &str) -> Option<MetadataCandidate> {
    let title = clean_title(&r.title?);
    if title.is_empty() {
        return None;
    }
    let authors: Vec<String> = r.creators.iter().map(|c| clean_author(c)).filter(|c| !c.is_empty()).collect();
    Some(MetadataCandidate {
        title,
        authors: (!authors.is_empty()).then(|| authors.join(", ")),
        isbn: r.identifiers.iter().find_map(|i| extract_isbn(i)),
        year: r.date.as_deref().and_then(first_year),
        publisher: r.publisher.as_deref().map(clean_publisher).filter(|p| !p.is_empty()),
        pages: r.format.as_deref().and_then(first_number),
        language: r.language.as_deref().map(normalise_language),
        cover_url: None, // библиотечные каталоги обложек не отдают
        source: source.to_string(),
    })
}

/// «Dune /» и «[Aurora] ; Aurora : Roman / Kim Stanley Robinson ; …»
/// В MARC после « / » идут сведения об ответственности — они не часть названия.
fn clean_title(raw: &str) -> String {
    let head = raw.split(" / ").next().unwrap_or(raw);
    // у DNB встречается «[Ключевое название] ; Настоящее название»
    let head = head.rsplit(" ; ").next().unwrap_or(head);
    head.trim().trim_end_matches(['/', ':', ';', ',']).trim().to_string()
}

/// «Robinson, Kim Stanley [Verfasser]» → «Kim Stanley Robinson»,
/// «Herbert, Frank.» → «Frank Herbert».
fn clean_author(raw: &str) -> String {
    let mut s = raw.to_string();
    if let Some(bracket) = s.find(" [") {
        s.truncate(bracket);
    }
    let s = s.trim().trim_end_matches(['.', ',']).trim();
    match s.split_once(", ") {
        Some((last, first)) if !first.is_empty() => format!("{first} {last}"),
        _ => s.to_string(),
    }
}

/// «München : Wilhelm Heyne Verlag» и «New York : Ace Books,» — до двоеточия
/// стоит место издания, издатель после.
fn clean_publisher(raw: &str) -> String {
    let tail = raw.rsplit(" : ").next().unwrap_or(raw);
    tail.trim().trim_end_matches([',', ';', ':']).trim().to_string()
}

/// «2005, c1965.» → 2005: первый год в строке — год этого издания.
fn first_year(raw: &str) -> Option<i64> {
    let digits: Vec<char> = raw.chars().collect();
    digits
        .windows(4)
        .find(|w| w.iter().all(char::is_ascii_digit))
        .map(|w| w.iter().collect::<String>())
        .and_then(|s| s.parse().ok())
        .filter(|y: &i64| (1400..=2200).contains(y))
}

/// «555 Seiten» → 555.
fn first_number(raw: &str) -> Option<i64> {
    let mut num = String::new();
    for c in raw.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else if !num.is_empty() {
            break;
        }
    }
    num.parse().ok().filter(|n: &i64| *n > 0)
}

/// Каталоги пишут язык тремя буквами (ger/eng/rus), остальные провайдеры —
/// двумя. Приводим к двум, чтобы поле выглядело одинаково.
fn normalise_language(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "ger" | "deu" => "de",
        "eng" => "en",
        "rus" => "ru",
        "fre" | "fra" => "fr",
        "spa" => "es",
        "ita" => "it",
        other => return other.to_string(),
    }
    .to_string()
}

/// «URN:ISBN:9780441013593» и «978-3-453-31724-6 Broschur : EUR 15.50 (AT)» →
/// голый ISBN. Идентификатор бывает в любом месте строки, поэтому перебираем
/// все похожие куски, а не режем по разделителю.
fn extract_isbn(raw: &str) -> Option<String> {
    let marked = raw.to_uppercase().contains("ISBN");
    let mut token = String::new();
    for c in raw.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || c == '-' || c == 'X' || c == 'x' {
            token.push(c);
            continue;
        }
        if plausible_isbn(&token, marked) {
            if let Ok(isbn) = crate::domain::isbn::normalize_and_validate(&token) {
                return Some(isbn);
            }
        }
        token.clear();
    }
    None
}

/// Голые десять цифр — чаще внутренний номер записи каталога, чем ISBN:
/// у DNB, например, номер 1098326865 случайно проходит контрольную сумму
/// ISBN-10. У настоящего ISBN в выдаче есть дефисы или пометка ISBN рядом.
fn plausible_isbn(token: &str, marked: bool) -> bool {
    let significant = token.chars().filter(|c| c.is_ascii_digit() || c.eq_ignore_ascii_case(&'x')).count();
    match significant {
        13 => marked || token.starts_with("978") || token.starts_with("979"),
        10 => marked || token.contains('-'),
        _ => false,
    }
}

pub struct Sru {
    client: reqwest::Client,
    endpoint: &'static str,
    isbn_index: &'static str,
    title_index: &'static str,
    record_schema: &'static str,
    source: &'static str,
    label: &'static str,
}

impl Sru {
    /// Deutsche Nationalbibliothek — вся немецкая книгоиздательская продукция.
    pub fn dnb(client: reqwest::Client) -> Self {
        Self {
            client,
            endpoint: "https://services.dnb.de/sru/dnb",
            isbn_index: "NUM",
            title_index: "TIT",
            record_schema: "oai_dc",
            source: "dnb",
            label: "Deutsche Nationalbibliothek",
        }
    }

    /// Library of Congress — крупнейший англоязычный каталог.
    pub fn loc(client: reqwest::Client) -> Self {
        Self {
            client,
            endpoint: "http://lx2.loc.gov:210/lcdb",
            isbn_index: "bath.isbn",
            title_index: "dc.title",
            record_schema: "dc",
            source: "loc",
            label: "Library of Congress",
        }
    }

    /// ISBN — одно слово из цифр, кавычки ему не нужны.
    fn isbn_query(&self, isbn: &str) -> String {
        format!("{}={}", self.isbn_index, isbn)
    }

    /// В CQL пробел разделяет термы, поэтому `TIT=Метро 2033` — это не поиск
    /// фразы, а синтаксическая ошибка. Название всегда уходит в кавычках,
    /// а сами кавычки и слэши из него вычищаются: закрыв фразу посреди
    /// запроса, они превратили бы остаток названия в синтаксис.
    fn title_query(&self, title: &str) -> String {
        let cleaned: String = title.chars().filter(|c| *c != '"' && *c != '\\').collect();
        format!("{}=\"{}\"", self.title_index, cleaned.trim())
    }

    async fn search(&self, query: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        let resp = self
            .client
            .get(self.endpoint)
            .query(&[
                ("version", "1.1"),
                ("operation", "searchRetrieve"),
                ("query", query),
                ("recordSchema", self.record_schema),
                ("maximumRecords", "5"),
            ])
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Network(format!(
                "{} ответил {}",
                self.label,
                resp.status()
            )));
        }
        let body = resp.text().await.map_err(|e| AppError::Network(e.to_string()))?;
        Ok(parse_dc_records(&body, self.source))
    }
}

#[async_trait]
impl MetadataProvider for Sru {
    async fn lookup_isbn(&self, isbn: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        self.search(&self.isbn_query(isbn)).await
    }

    async fn lookup_title(&self, title: &str) -> Result<Vec<MetadataCandidate>, AppError> {
        self.search(&self.title_query(title)).await
    }

    fn name(&self) -> &'static str {
        self.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CQL разбирает пробел как разделитель термов, поэтому `TIT=Метро 2033` —
    /// синтаксическая ошибка, а не поиск фразы. Каталог отвечал ошибкой или
    /// мусором на любое название длиннее одного слова.
    #[test]
    fn a_multiword_title_is_sent_as_a_quoted_phrase() {
        let dnb = Sru::dnb(reqwest::Client::new());
        assert_eq!(dnb.title_query("Метро 2033"), "TIT=\"Метро 2033\"");
        let loc = Sru::loc(reqwest::Client::new());
        assert_eq!(loc.title_query("The Dispossessed"), "dc.title=\"The Dispossessed\"");
    }

    #[test]
    fn quotes_inside_a_title_do_not_break_the_query() {
        let dnb = Sru::dnb(reqwest::Client::new());
        // кавычка закрыла бы фразу и всё, что дальше, каталог принял бы за синтаксис
        assert_eq!(dnb.title_query("  Он сказал \"да\"  "), "TIT=\"Он сказал да\"");
        assert_eq!(dnb.title_query("a\\b"), "TIT=\"ab\"");
    }

    #[test]
    fn isbn_query_stays_a_bare_term() {
        let dnb = Sru::dnb(reqwest::Client::new());
        assert_eq!(dnb.isbn_query("9783453317246"), "NUM=9783453317246");
    }

    #[test]
    fn parses_dnb_record_with_prefixed_tags() {
        let xml = include_str!("../../tests/fixtures/dnb_isbn.xml");
        let out = parse_dc_records(xml, "dnb");
        assert_eq!(out.len(), 1);
        let c = &out[0];
        assert_eq!(c.title, "Aurora : Roman");
        // «Robinson, Kim Stanley [Verfasser]» разворачивается в человеческий вид
        assert!(c.authors.as_deref().unwrap().starts_with("Kim Stanley Robinson"));
        assert_eq!(c.year, Some(2016));
        assert_eq!(c.publisher.as_deref(), Some("Wilhelm Heyne Verlag"));
        assert_eq!(c.pages, Some(555));
        assert_eq!(c.language.as_deref(), Some("de"));
        assert_eq!(c.isbn.as_deref(), Some("9783453317246"));
        assert_eq!(c.source, "dnb");
    }

    #[test]
    fn parses_loc_record_with_unprefixed_tags() {
        let xml = include_str!("../../tests/fixtures/loc_isbn.xml");
        let out = parse_dc_records(xml, "loc");
        assert_eq!(out.len(), 1);
        let c = &out[0];
        assert_eq!(c.title, "Dune"); // хвостовой « /» из MARC убран
        assert!(c.authors.as_deref().unwrap().contains("Frank Herbert"));
        assert_eq!(c.year, Some(2005)); // из «2005, c1965.»
        assert_eq!(c.publisher.as_deref(), Some("Ace Books"));
        assert_eq!(c.language.as_deref(), Some("en"));
        assert_eq!(c.isbn.as_deref(), Some("9780441013593")); // из URN:ISBN:
        assert_eq!(c.source, "loc");
    }

    #[test]
    fn empty_response_yields_nothing() {
        let xml = include_str!("../../tests/fixtures/sru_empty.xml");
        assert!(parse_dc_records(xml, "loc").is_empty());
    }

    #[test]
    fn garbage_input_does_not_panic() {
        assert!(parse_dc_records("не xml вовсе", "loc").is_empty());
        assert!(parse_dc_records("<dc><title>", "loc").is_empty());
    }

    #[test]
    fn xml_entities_are_decoded() {
        let xml = r#"<dc><title>Krieg &amp; Frieden</title><creator>Tolstoi, Lew</creator></dc>"#;
        let out = parse_dc_records(xml, "dnb");
        assert_eq!(out[0].title, "Krieg & Frieden");
        assert_eq!(out[0].authors.as_deref(), Some("Lew Tolstoi"));
    }

    #[test]
    fn title_statement_of_responsibility_is_stripped() {
        assert_eq!(clean_title("Dune /"), "Dune");
        assert_eq!(clean_title("Aurora : Roman / Kim Stanley Robinson ; übersetzt"), "Aurora : Roman");
        assert_eq!(clean_title("Простое название"), "Простое название");
    }

    #[test]
    fn year_picks_the_edition_not_the_copyright() {
        assert_eq!(first_year("2005, c1965."), Some(2005));
        assert_eq!(first_year("2016"), Some(2016));
        assert_eq!(first_year("без даты"), None);
        assert_eq!(first_year("9999"), None); // не год
    }

    #[test]
    fn isbn_is_pulled_out_of_noisy_identifiers() {
        assert_eq!(extract_isbn("URN:ISBN:9780441013593").as_deref(), Some("9780441013593"));
        assert_eq!(
            extract_isbn("978-3-453-31724-6 Broschur : EUR 15.50 (AT)").as_deref(),
            Some("9783453317246")
        );
        // 1098326865 — внутренний номер записи DNB, но он случайно проходит
        // контрольную сумму ISBN-10; без пометки и дефисов не принимаем
        assert_eq!(extract_isbn("1098326865"), None);
        assert_eq!(extract_isbn("(DE-101)1098326865"), None);
        // а настоящий ISBN-10 с дефисами — принимаем
        assert_eq!(extract_isbn("0-441-01359-7").as_deref(), Some("9780441013593"));
        assert_eq!(extract_isbn("ISBN 0441013597").as_deref(), Some("9780441013593"));
    }
}
