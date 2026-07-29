use crate::db::books;
use crate::db::models::BookInput;
use crate::error::AppError;
use rusqlite::Connection;
use serde::Serialize;

// Наполнять каталог сканером по одной книге долго. У большинства список уже
// где-то есть — в Goodreads или LiveLib, — и обе выгружают CSV. Разбираем
// оба формата одним кодом: колонки ищем по заголовку, а не по номеру.

#[derive(Debug, Serialize, PartialEq)]
pub struct ImportReport {
    pub added: usize,
    pub skipped_duplicates: usize,
    pub skipped_invalid: usize,
    /// Первые несколько причин пропуска — чтобы человек понял, что случилось.
    pub problems: Vec<String>,
}

/// Разделитель определяем по первой строке: Goodreads пишет запятыми,
/// русские выгрузки часто точкой с запятой.
fn detect_delimiter(header: &str) -> char {
    let commas = header.matches(',').count();
    let semis = header.matches(';').count();
    let tabs = header.matches('\t').count();
    if tabs > commas && tabs > semis {
        '\t'
    } else if semis > commas {
        ';'
    } else {
        ','
    }
}

/// Разбор строки CSV с кавычками и удвоенными кавычками внутри поля.
fn split_row(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            c if c == delim && !in_quotes => out.push(std::mem::take(&mut field)),
            c => field.push(c),
        }
    }
    out.push(field);
    out
}

/// Разбивает файл на строки, уважая переводы строк внутри кавычек:
/// в аннотациях Goodreads они встречаются постоянно.
fn logical_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in text.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            '\n' if !in_quotes => {
                lines.push(std::mem::take(&mut current));
            }
            '\r' if !in_quotes => {}
            c => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        lines.push(current);
    }
    lines
}

/// Названия колонок у Goodreads и LiveLib разные — держим синонимы.
const TITLE_KEYS: &[&str] = &["title", "название", "книга", "name"];
const AUTHOR_KEYS: &[&str] = &["author", "authors", "автор", "авторы", "author l-f"];
const ISBN_KEYS: &[&str] = &["isbn13", "isbn-13", "isbn", "исбн"];
const YEAR_KEYS: &[&str] =
    &["year published", "original publication year", "год", "год издания", "year"];
const PUBLISHER_KEYS: &[&str] = &["publisher", "издательство", "издатель"];
const PAGES_KEYS: &[&str] = &["number of pages", "pages", "страниц", "страницы"];
const RATING_KEYS: &[&str] = &["my rating", "оценка", "rating", "моя оценка"];
const STATUS_KEYS: &[&str] = &["exclusive shelf", "bookshelves", "статус", "полка", "список"];
const FINISHED_KEYS: &[&str] = &["date read", "дата прочтения", "прочитано"];
const STARTED_KEYS: &[&str] = &["date started", "дата начала"];

fn column(header: &[String], keys: &[&str]) -> Option<usize> {
    // Точное совпадение важнее: «isbn13» не должен проиграть «isbn».
    for key in keys {
        if let Some(i) = header.iter().position(|h| h == key) {
            return Some(i);
        }
    }
    keys.iter()
        .find_map(|key| header.iter().position(|h| h.contains(key)))
}

fn cell(row: &[String], idx: Option<usize>) -> Option<String> {
    let value = row.get(idx?)?.trim();
    // Goodreads заворачивает ISBN в ="9785..." чтобы Excel не съел ведущие нули
    let value = value.trim_start_matches('=').trim_matches('"').trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn number(row: &[String], idx: Option<usize>) -> Option<i64> {
    let raw = cell(row, idx)?;
    let digits: String = raw.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Goodreads: «read» / «currently-reading» / «to-read».
/// LiveLib и ручные выгрузки: «прочитано», «читаю», «хочу прочитать».
fn map_status(raw: &str) -> Option<String> {
    let s = raw.to_lowercase();
    if s.contains("currently") || s.contains("читаю") {
        Some("reading".into())
    } else if s.contains("to-read") || s.contains("хочу") || s.contains("планирую") {
        Some("want".into())
    } else if s.contains("read") || s.contains("прочит") {
        Some("read".into())
    } else {
        None
    }
}

/// Goodreads пишет «2026/03/12», нам нужно «2026-03-12».
fn map_date(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split(['/', '-', '.']).map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let (y, m, d) = if parts[0].len() == 4 {
        (parts[0], parts[1], parts[2])
    } else {
        // «12.03.2026» — день первым
        (parts[2], parts[1], parts[0])
    };
    let (y, m, d) = (y.parse::<i32>().ok()?, m.parse::<u32>().ok()?, d.parse::<u32>().ok()?);
    chrono::NaiveDate::from_ymd_opt(y, m, d).map(|date| date.format("%Y-%m-%d").to_string())
}

pub fn parse(text: &str) -> Result<Vec<BookInput>, AppError> {
    let text = text.trim_start_matches('\u{feff}'); // BOM из Excel
    let lines = logical_lines(text);
    let Some(header_line) = lines.first() else {
        return Err(AppError::Rule("Файл пуст".into()));
    };
    let delim = detect_delimiter(header_line);
    let header: Vec<String> = split_row(header_line, delim)
        .into_iter()
        .map(|h| h.trim().trim_matches('"').to_lowercase())
        .collect();

    let (title_i, author_i) = (column(&header, TITLE_KEYS), column(&header, AUTHOR_KEYS));
    if title_i.is_none() {
        return Err(AppError::Rule(
            "В файле нет колонки с названием книги. Нужен CSV-экспорт из Goodreads или LiveLib"
                .into(),
        ));
    }
    let (isbn_i, year_i) = (column(&header, ISBN_KEYS), column(&header, YEAR_KEYS));
    let (pub_i, pages_i) = (column(&header, PUBLISHER_KEYS), column(&header, PAGES_KEYS));
    let (rating_i, status_i) = (column(&header, RATING_KEYS), column(&header, STATUS_KEYS));
    let (started_i, finished_i) = (column(&header, STARTED_KEYS), column(&header, FINISHED_KEYS));

    let mut out = Vec::new();
    for line in lines.iter().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let row = split_row(line, delim);
        let Some(title) = cell(&row, title_i) else { continue };
        out.push(BookInput {
            title,
            authors: cell(&row, author_i),
            isbn: cell(&row, isbn_i),
            year: number(&row, year_i),
            publisher: cell(&row, pub_i),
            pages: number(&row, pages_i),
            language: None,
            genre: None,
            annotation: None,
            cover_url: None,
            shelf_id: None,
            status: cell(&row, status_i).as_deref().and_then(map_status),
            // ноль в Goodreads означает «не оценил», а не «оценка 0»
            rating: number(&row, rating_i).filter(|r| *r > 0),
            notes: None,
            started_at: cell(&row, started_i).as_deref().and_then(map_date),
            finished_at: cell(&row, finished_i).as_deref().and_then(map_date),
        });
    }
    Ok(out)
}

/// Заливает разобранные книги в каталог. Импорт **дополняет** каталог, а не
/// заменяет его (в отличие от восстановления из резервной копии), поэтому
/// книги с уже известным ISBN пропускаем, чтобы не наплодить дублей.
pub fn apply(
    conn: &Connection,
    parsed: &[BookInput],
    shelf_id: Option<i64>,
) -> Result<ImportReport, AppError> {
    let mut report = ImportReport {
        added: 0,
        skipped_duplicates: 0,
        skipped_invalid: 0,
        problems: Vec::new(),
    };
    for input in parsed {
        if let Some(isbn) = input.isbn.as_deref() {
            let norm = crate::domain::isbn::normalize_and_validate(isbn)
                .unwrap_or_else(|_| isbn.trim().to_string());
            if !books::find_isbn_duplicates(conn, &norm)?.is_empty() {
                report.skipped_duplicates += 1;
                continue;
            }
        }
        let mut input = input.clone();
        input.shelf_id = shelf_id;
        match books::insert(conn, &input) {
            Ok(_) => report.added += 1,
            Err(e) => {
                report.skipped_invalid += 1;
                if report.problems.len() < 5 {
                    report.problems.push(format!("«{}»: {e}", input.title));
                }
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{locations, open_in_memory};

    const GOODREADS: &str = "Book Id,Title,Author,Author l-f,ISBN,ISBN13,My Rating,Publisher,Number of Pages,Year Published,Date Read,Exclusive Shelf\n\
1,Dune,Frank Herbert,\"Herbert, Frank\",=\"0441013597\",=\"9780441013593\",5,Ace Books,412,2005,2026/03/12,read\n\
2,\"Aurora: Roman\",Kim Stanley Robinson,\"Robinson, Kim\",=\"\",=\"9783453317246\",0,Heyne,555,2016,,to-read\n";

    #[test]
    fn parses_goodreads_export() {
        let out = parse(GOODREADS).unwrap();
        assert_eq!(out.len(), 2);

        let dune = &out[0];
        assert_eq!(dune.title, "Dune");
        assert_eq!(dune.authors.as_deref(), Some("Frank Herbert"));
        assert_eq!(dune.isbn.as_deref(), Some("9780441013593")); // взят ISBN13, не ISBN10
        assert_eq!(dune.rating, Some(5));
        assert_eq!(dune.publisher.as_deref(), Some("Ace Books"));
        assert_eq!(dune.pages, Some(412));
        assert_eq!(dune.year, Some(2005));
        assert_eq!(dune.status.as_deref(), Some("read"));
        assert_eq!(dune.finished_at.as_deref(), Some("2026-03-12"));

        let aurora = &out[1];
        assert_eq!(aurora.title, "Aurora: Roman");
        assert_eq!(aurora.status.as_deref(), Some("want"));
        assert_eq!(aurora.rating, None, "ноль в Goodreads — это «не оценил»");
        assert_eq!(aurora.finished_at, None);
    }

    #[test]
    fn parses_semicolon_separated_russian_export() {
        let csv = "Название;Автор;ISBN;Год;Оценка;Статус\n\
                   Будущее;Дмитрий Глуховский;9785171183660;2019;4;прочитано\n\
                   Метро 2033;Дмитрий Глуховский;;2005;;читаю\n";
        let out = parse(csv).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Будущее");
        assert_eq!(out[0].isbn.as_deref(), Some("9785171183660"));
        assert_eq!(out[0].rating, Some(4));
        assert_eq!(out[0].status.as_deref(), Some("read"));
        assert_eq!(out[1].status.as_deref(), Some("reading"));
        assert_eq!(out[1].isbn, None);
    }

    #[test]
    fn handles_newlines_and_quotes_inside_fields() {
        let csv = "Title,Author\n\"Многострочное\nназвание\",\"Автор, с запятой\"\n";
        let out = parse(csv).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "Многострочное\nназвание");
        assert_eq!(out[0].authors.as_deref(), Some("Автор, с запятой"));
    }

    #[test]
    fn file_without_a_title_column_is_rejected_with_a_clear_reason() {
        let err = parse("foo,bar\n1,2\n").unwrap_err();
        assert!(matches!(err, AppError::Rule(m) if m.contains("названием")));
    }

    #[test]
    fn dates_in_both_orders_are_understood() {
        assert_eq!(map_date("2026/03/12").as_deref(), Some("2026-03-12"));
        assert_eq!(map_date("12.03.2026").as_deref(), Some("2026-03-12"));
        assert_eq!(map_date("2026-03-12").as_deref(), Some("2026-03-12"));
        assert_eq!(map_date("непонятно"), None);
        assert_eq!(map_date("2026/13/45"), None); // несуществующая дата
    }

    fn shelf(conn: &Connection) -> i64 {
        let root = locations::create(conn, None, "Дом", "root", None).unwrap().id;
        locations::create(conn, Some(root), "Полка", "shelf", None).unwrap().id
    }

    #[test]
    fn apply_puts_books_on_the_chosen_shelf() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let parsed = parse(GOODREADS).unwrap();
        let report = apply(&conn, &parsed, Some(s)).unwrap();
        assert_eq!(report.added, 2);
        assert_eq!(report.skipped_duplicates, 0);
        assert_eq!(books::on_shelf(&conn, s).unwrap().len(), 2);
    }

    #[test]
    fn apply_is_additive_and_skips_isbn_already_in_the_catalogue() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let parsed = parse(GOODREADS).unwrap();
        apply(&conn, &parsed, Some(s)).unwrap();

        // повторный импорт того же файла не должен удваивать каталог
        let again = apply(&conn, &parsed, Some(s)).unwrap();
        assert_eq!(again.added, 0);
        assert_eq!(again.skipped_duplicates, 2);
        assert_eq!(books::all(&conn).unwrap().len(), 2);
    }

    #[test]
    fn books_without_isbn_are_not_treated_as_duplicates_of_each_other() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let csv = "Название;Автор\nПервая;А\nВторая;Б\nТретья;В\n";
        let report = apply(&conn, &parse(csv).unwrap(), Some(s)).unwrap();
        assert_eq!(report.added, 3);
    }

    #[test]
    fn broken_rows_are_counted_and_explained_without_aborting_the_import() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        // пустое название отбраковывается на уровне БД
        let mut parsed = parse("Название\nХорошая\n").unwrap();
        parsed.push(BookInput { title: "   ".into(), ..Default::default() });
        let report = apply(&conn, &parsed, Some(s)).unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped_invalid, 1);
        assert_eq!(report.problems.len(), 1);
    }
}
