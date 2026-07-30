use crate::db::locations::breadcrumb;
use crate::db::models::{Book, BookInput, Stats};
use crate::error::AppError;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookHit {
    pub book: Book,
    pub breadcrumb: String,
    pub off_shelf: bool,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn clean_title(title: &str) -> Result<&str, AppError> {
    let t = title.trim();
    if t.is_empty() {
        return Err(AppError::Rule("У книги должно быть название".into()));
    }
    Ok(t)
}

/// Приводим ISBN к той же форме, в которой его пишет сканирование (ISBN-13),
/// иначе «978-5-…», введённый руками, не совпадёт с отсканированным дублем.
/// Невалидный ISBN не выбрасываем — сохраняем как есть, вдруг это внутренний код.
fn clean_isbn(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    Some(crate::domain::isbn::normalize_and_validate(s).unwrap_or_else(|_| s.to_string()))
}

fn clean_rating(rating: Option<i64>) -> Option<i64> {
    rating.map(|v| v.clamp(0, 5))
}

/// Даты храним строго как ГГГГ-ММ-ДД, иначе сортировка и группировка по годам
/// в статистике начнут врать на первом же «12.03.2026».
fn clean_date(raw: Option<&str>) -> Result<Option<String>, AppError> {
    let s = match raw.map(str::trim) {
        None | Some("") => return Ok(None),
        Some(s) => s,
    };
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::Rule(format!("Дата «{s}» не в формате ГГГГ-ММ-ДД")))?;
    Ok(Some(s.to_string()))
}

pub fn insert(conn: &Connection, i: &BookInput) -> Result<Book, AppError> {
    let ts = now();
    conn.execute(
        "INSERT INTO books \
         (title, authors, isbn, year, publisher, pages, language, genre, annotation, \
          cover_url, shelf_id, status, rating, notes, availability, lent_to, added_at, \
          updated_at, started_at, finished_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'on_shelf',NULL,?15,?15,?16,?17)",
        params![
            clean_title(&i.title)?, i.authors, clean_isbn(i.isbn.as_deref()), i.year,
            i.publisher, i.pages, i.language, i.genre, i.annotation, i.cover_url,
            i.shelf_id, i.status, clean_rating(i.rating), i.notes, ts,
            clean_date(i.started_at.as_deref())?, clean_date(i.finished_at.as_deref())?
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

/// Смена `cover_url` снимает с учёта скачанный файл: он остался от прежней
/// ссылки, а `Cover` во фронте предпочитает локальный файл сетевому — без
/// сброса книга навсегда показывала бы старую картинку, и «Скачать все
/// обложки» её бы не догнало (там выбираются только пустые `cover_path`).
/// SQLite считает правые части SET по исходной строке, поэтому `cover_url`
/// в CASE — ещё старый.
pub fn update(conn: &Connection, id: i64, i: &BookInput) -> Result<Book, AppError> {
    conn.execute(
        "UPDATE books SET title=?2, authors=?3, isbn=?4, year=?5, publisher=?6, pages=?7, \
         language=?8, genre=?9, annotation=?10, \
         cover_path = CASE WHEN COALESCE(cover_url,'') = COALESCE(?11,'') \
                           THEN cover_path ELSE NULL END, \
         cover_url=?11, shelf_id=?12, status=?13, \
         rating=?14, notes=?15, updated_at=?16, started_at=?17, finished_at=?18 WHERE id=?1",
        params![
            id, clean_title(&i.title)?, i.authors, clean_isbn(i.isbn.as_deref()), i.year,
            i.publisher, i.pages, i.language, i.genre, i.annotation, i.cover_url,
            i.shelf_id, i.status, clean_rating(i.rating), i.notes, now(),
            clean_date(i.started_at.as_deref())?, clean_date(i.finished_at.as_deref())?
        ],
    )?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute("DELETE FROM books WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_cover_path(conn: &Connection, id: i64, name: Option<&str>) -> Result<(), AppError> {
    conn.execute("UPDATE books SET cover_path=?1 WHERE id=?2", params![name, id])?;
    Ok(())
}

/// Книги с сетевой обложкой, которую мы ещё не забрали к себе.
pub fn needing_covers(conn: &Connection) -> Result<Vec<(i64, String)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, cover_url FROM books \
         WHERE cover_url IS NOT NULL AND cover_url <> '' AND cover_path IS NULL ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn set_shelf(conn: &Connection, id: i64, shelf_id: Option<i64>) -> Result<(), AppError> {
    conn.execute(
        "UPDATE books SET shelf_id=?1, updated_at=?2 WHERE id=?3",
        params![shelf_id, now(), id],
    )?;
    Ok(())
}

/// Возврат на полку стирает всё, что относилось к выдаче, — иначе в карточке
/// остаётся «ждём назад до 3 марта» у книги, которая уже стоит на месте.
pub fn set_availability(
    conn: &Connection,
    id: i64,
    availability: &str,
    lent_to: Option<&str>,
    due_at: Option<&str>,
) -> Result<(), AppError> {
    if !matches!(availability, "on_shelf" | "lent" | "away") {
        return Err(AppError::Rule(format!("Неизвестный статус наличия: {availability}")));
    }
    let ts = now();
    if availability == "on_shelf" {
        conn.execute(
            "UPDATE books SET availability='on_shelf', lent_to=NULL, lent_at=NULL, \
             due_at=NULL, updated_at=?1 WHERE id=?2",
            params![ts, id],
        )?;
        return Ok(());
    }
    conn.execute(
        "UPDATE books SET availability=?1, lent_to=?2, due_at=?3, \
         lent_at=COALESCE(lent_at, ?4), updated_at=?5 WHERE id=?6",
        params![availability, lent_to, clean_date(due_at)?, today(), ts, id],
    )?;
    Ok(())
}

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

pub fn get(conn: &Connection, id: i64) -> Result<Book, AppError> {
    Ok(conn.query_row(&format!("{SELECT_BOOK_COLS} WHERE id = ?1"), params![id], row_to_book)?)
}

pub fn all(conn: &Connection) -> Result<Vec<Book>, AppError> {
    let mut stmt = conn.prepare(&format!("{SELECT_BOOK_COLS} ORDER BY id"))?;
    let rows = stmt.query_map([], row_to_book)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn on_shelf(conn: &Connection, shelf_id: i64) -> Result<Vec<Book>, AppError> {
    let mut stmt = conn.prepare(&format!("{SELECT_BOOK_COLS} WHERE shelf_id=?1 ORDER BY title"))?;
    let rows = stmt.query_map(params![shelf_id], row_to_book)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Книги, которым не указали полку. Без такого списка они пропадали из виду
/// совсем: на полках их нет, а в поиске находятся, только если помнишь название.
pub fn without_shelf(conn: &Connection) -> Result<Vec<Book>, AppError> {
    let mut stmt =
        conn.prepare(&format!("{SELECT_BOOK_COLS} WHERE shelf_id IS NULL ORDER BY added_at DESC"))?;
    let rows = stmt.query_map([], row_to_book)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Дубли ищем по всему каталогу, а не только на текущей полке: человеку важно
/// «эта книга у меня уже есть, вон там», а не «на этой полке её нет».
/// Возвращает путь к каждому уже имеющемуся экземпляру.
pub fn find_isbn_duplicates(conn: &Connection, isbn: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare("SELECT shelf_id FROM books WHERE isbn = ?1 ORDER BY id")?;
    let shelves = stmt
        .query_map(params![isbn], |r| r.get::<_, Option<i64>>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(shelves.len());
    for shelf in shelves {
        out.push(match shelf {
            Some(sid) => breadcrumb(conn, sid)?,
            None => "без полки".to_string(),
        });
    }
    Ok(out)
}

// SQLite's built-in `COLLATE NOCASE` only case-folds ASCII (A-Z), so plain
// `LIKE ... COLLATE NOCASE` misses Cyrillic (title/authors/genre are Russian
// text). Register a Unicode-aware lowercasing scalar function and match on
// its output instead, keeping the query a prepared statement.
fn register_unicode_lower(conn: &Connection) -> Result<(), AppError> {
    conn.create_scalar_function(
        "lower_uc",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let s: Option<String> = ctx.get(0)?;
            Ok(s.map(|v| v.to_lowercase()))
        },
    )?;
    Ok(())
}

/// `%` и `_` в запросе — обычные символы, а не подстановочные знаки LIKE.
/// Без этого поиск по «100%» или «_» возвращал весь каталог.
fn like_pattern(query: &str) -> String {
    let escaped = query
        .to_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// В базе ISBN лежит нормализованным, а вводят его с дефисами и пробелами —
/// поэтому по ISBN ищем отдельно, по одним цифрам.
///
/// Но только если весь запрос и есть ISBN. Раньше цифры выдёргивались из
/// любого текста: «Дюна 2» превращалось в `isbn LIKE '%2%'` и вытаскивало
/// почти весь каталог — двойка есть в большинстве ISBN.
fn isbn_fragment(query: &str) -> Option<String> {
    let looks_like_isbn = query
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == ' ' || c == 'x' || c == 'X');
    if !looks_like_isbn {
        return None;
    }
    let digits: String = query.chars().filter(char::is_ascii_digit).collect();
    // «978» — самый короткий осмысленный префикс; короче начинается шум
    (digits.len() >= 3).then_some(digits)
}

pub fn search(conn: &Connection, query: &str) -> Result<Vec<BookHit>, AppError> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    register_unicode_lower(conn)?;
    let query = query.trim();
    let like = like_pattern(query);
    let isbn_like = match isbn_fragment(query) {
        Some(digits) => format!("%{digits}%"),
        None => like.clone(),
    };
    let mut stmt = conn.prepare(&format!(
        "{SELECT_BOOK_COLS} WHERE \
         lower_uc(title) LIKE ?1 ESCAPE '\\' OR lower_uc(authors) LIKE ?1 ESCAPE '\\' OR \
         lower_uc(genre) LIKE ?1 ESCAPE '\\' OR isbn LIKE ?2 ESCAPE '\\' \
         ORDER BY title"
    ))?;
    let books = stmt
        .query_map(params![like, isbn_like], row_to_book)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut hits = Vec::with_capacity(books.len());
    for b in books {
        let crumb = match b.shelf_id {
            Some(sid) => breadcrumb(conn, sid)?,
            None => String::new(),
        };
        let off = b.availability != "on_shelf";
        hits.push(BookHit { book: b, breadcrumb: crumb, off_shelf: off });
    }
    Ok(hits)
}

pub fn stats(conn: &Connection) -> Result<Stats, AppError> {
    let total: i64 = conn.query_row("SELECT count(*) FROM books", [], |r| r.get(0))?;
    let pages_read: i64 = conn.query_row(
        "SELECT COALESCE(SUM(pages),0) FROM books WHERE status='read'",
        [],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT COALESCE(status,'не указан') AS s, count(*) FROM books GROUP BY s ORDER BY count(*) DESC",
    )?;
    let by_status = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut stmt2 = conn.prepare(
        "SELECT genre, count(*) FROM books WHERE genre IS NOT NULL AND genre <> '' \
         GROUP BY genre ORDER BY count(*) DESC LIMIT 5",
    )?;
    let top_genres = stmt2
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    // Даты лежат как ГГГГ-ММ-ДД, поэтому год — это просто первые четыре символа.
    let mut stmt3 = conn.prepare(
        "SELECT substr(finished_at, 1, 4) AS y, count(*) FROM books \
         WHERE finished_at IS NOT NULL AND finished_at <> '' GROUP BY y ORDER BY y DESC LIMIT 10",
    )?;
    let by_year = stmt3
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    let lent_out: i64 =
        conn.query_row("SELECT count(*) FROM books WHERE availability='lent'", [], |r| r.get(0))?;
    let overdue: i64 = conn.query_row(
        "SELECT count(*) FROM books WHERE availability='lent' \
         AND due_at IS NOT NULL AND due_at <> '' AND due_at < ?1",
        params![today()],
        |r| r.get(0),
    )?;
    Ok(Stats { total, by_status, pages_read, top_genres, by_year, lent_out, overdue })
}

const SELECT_BOOK_COLS: &str = "SELECT id, title, authors, isbn, year, publisher, pages, \
    language, genre, annotation, cover_url, shelf_id, status, rating, notes, availability, \
    lent_to, added_at, updated_at, started_at, finished_at, lent_at, due_at, cover_path \
    FROM books";

fn row_to_book(r: &rusqlite::Row) -> rusqlite::Result<Book> {
    Ok(Book {
        id: r.get(0)?,
        title: r.get(1)?,
        authors: r.get(2)?,
        isbn: r.get(3)?,
        year: r.get(4)?,
        publisher: r.get(5)?,
        pages: r.get(6)?,
        language: r.get(7)?,
        genre: r.get(8)?,
        annotation: r.get(9)?,
        cover_url: r.get(10)?,
        shelf_id: r.get(11)?,
        status: r.get(12)?,
        rating: r.get(13)?,
        notes: r.get(14)?,
        availability: r.get(15)?,
        lent_to: r.get(16)?,
        added_at: r.get(17)?,
        updated_at: r.get(18)?,
        started_at: r.get(19)?,
        finished_at: r.get(20)?,
        lent_at: r.get(21)?,
        due_at: r.get(22)?,
        cover_path: r.get(23)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{locations, open_in_memory};

    fn shelf(conn: &Connection) -> i64 {
        let root = locations::create(conn, None, "Дом", "root", None).unwrap().id;
        let room = locations::create(conn, Some(root), "Гостиная", "room", None).unwrap().id;
        let case = locations::create(conn, Some(room), "Шкаф A", "bookcase", None).unwrap().id;
        locations::create(conn, Some(case), "Полка", "shelf", Some("A-3")).unwrap().id
    }

    fn book_on(conn: &Connection, shelf: i64, title: &str, authors: &str) -> Book {
        let mut i = BookInput::titled(title);
        i.authors = Some(authors.into());
        i.shelf_id = Some(shelf);
        insert(conn, &i).unwrap()
    }

    #[test]
    fn insert_defaults_status_null_availability_on_shelf() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let b = book_on(&conn, s, "Дюна", "Фрэнк Герберт");
        assert_eq!(b.status, None);
        assert_eq!(b.availability, "on_shelf");
        assert_eq!(b.shelf_id, Some(s));
    }

    #[test]
    fn update_overwrites_fields() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let b = book_on(&conn, s, "Дюна", "Герберт");
        let mut i = BookInput::titled("Дюна");
        i.authors = Some("Фрэнк Герберт".into());
        i.year = Some(2019);
        i.status = Some("read".into());
        i.shelf_id = Some(s);
        let upd = update(&conn, b.id, &i).unwrap();
        assert_eq!(upd.year, Some(2019));
        assert_eq!(upd.status, Some("read".into()));
    }

    #[test]
    fn search_by_author_substring_returns_breadcrumb() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        book_on(&conn, s, "Дюна", "Фрэнк Герберт");
        let hits = search(&conn, "гербе").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].breadcrumb, "Шкаф A › Полка");
        assert!(!hits[0].off_shelf);
    }

    #[test]
    fn lent_book_marked_off_shelf() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let b = book_on(&conn, s, "Дюна", "Герберт");
        set_availability(&conn, b.id, "lent", Some("Маша"), None).unwrap();
        let hits = search(&conn, "дюна").unwrap();
        assert!(hits[0].off_shelf);
        assert_eq!(hits[0].book.lent_to, Some("Маша".into()));
    }

    #[test]
    fn exists_isbn_on_shelf_detects_duplicate() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::titled("Дюна");
        i.isbn = Some("9785171183660".into());
        i.shelf_id = Some(s);
        insert(&conn, &i).unwrap();
        assert_eq!(find_isbn_duplicates(&conn, "9785171183660").unwrap(), vec!["Шкаф A › Полка"]);
        assert!(find_isbn_duplicates(&conn, "9780000000000").unwrap().is_empty());
    }

    #[test]
    fn duplicates_are_found_across_the_whole_catalogue() {
        let conn = open_in_memory().unwrap();
        let a = shelf(&conn);
        let root = locations::create(&conn, None, "Дача", "root", None).unwrap().id;
        let b = locations::create(&conn, Some(root), "Веранда", "shelf", None).unwrap().id;
        for shelf_id in [a, b] {
            let mut i = BookInput::titled("Будущее");
            i.isbn = Some("9785171183660".into());
            i.shelf_id = Some(shelf_id);
            insert(&conn, &i).unwrap();
        }
        // прежняя проверка смотрела только одну полку и второй экземпляр не видела
        let places = find_isbn_duplicates(&conn, "9785171183660").unwrap();
        assert_eq!(places, vec!["Шкаф A › Полка".to_string(), "Веранда".to_string()]);
    }

    #[test]
    fn duplicate_without_a_shelf_is_still_reported() {
        let conn = open_in_memory().unwrap();
        let mut i = BookInput::titled("Будущее");
        i.isbn = Some("9785171183660".into());
        insert(&conn, &i).unwrap();
        assert_eq!(find_isbn_duplicates(&conn, "9785171183660").unwrap(), vec!["без полки"]);
    }

    #[test]
    fn unshelved_books_are_listed_newest_first() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        book_on(&conn, s, "На полке", "Автор");

        let mut i = BookInput::titled("Забыл полку");
        insert(&conn, &i).unwrap();
        i.title = "Тоже забыл".into();
        insert(&conn, &i).unwrap();

        let out = without_shelf(&conn).unwrap();
        assert_eq!(out.len(), 2, "книги с полкой сюда попадать не должны");
        // свежие сверху — так их проще подобрать сразу после добавления
        assert_eq!(out[0].title, "Тоже забыл");
        assert!(out.iter().all(|b| b.shelf_id.is_none()));
    }

    #[test]
    fn assigning_a_shelf_removes_a_book_from_the_unshelved_list() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let i = BookInput::titled("Забыл полку");
        let b = insert(&conn, &i).unwrap();
        assert_eq!(without_shelf(&conn).unwrap().len(), 1);

        set_shelf(&conn, b.id, Some(s)).unwrap();
        assert!(without_shelf(&conn).unwrap().is_empty());
        assert_eq!(on_shelf(&conn, s).unwrap().len(), 1);
    }

    #[test]
    fn like_wildcards_in_query_are_literal() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        book_on(&conn, s, "Дюна", "Герберт");
        book_on(&conn, s, "Скидка 100% на всё", "Автор");
        // «%» раньше означал «что угодно» и вытаскивал весь каталог
        let hits = search(&conn, "100%").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].book.title, "Скидка 100% на всё");
        assert!(search(&conn, "_").unwrap().is_empty());
    }

    #[test]
    fn isbn_is_normalised_so_manual_entry_matches_scan() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::titled("Дюна");
        i.isbn = Some("978-5-17-118366-0".into()); // как вводят руками
        i.shelf_id = Some(s);
        let b = insert(&conn, &i).unwrap();
        assert_eq!(b.isbn.as_deref(), Some("9785171183660"));
        assert_eq!(find_isbn_duplicates(&conn, "9785171183660").unwrap(), vec!["Шкаф A › Полка"]);
    }

    #[test]
    fn isbn_search_ignores_hyphens() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::titled("Дюна");
        i.isbn = Some("9785171183660".into());
        i.shelf_id = Some(s);
        insert(&conn, &i).unwrap();
        assert_eq!(search(&conn, "978-5-17-118366-0").unwrap().len(), 1);
        assert_eq!(search(&conn, "9785171183660").unwrap().len(), 1);
        assert_eq!(search(&conn, "978517").unwrap().len(), 1); // частичный ввод
    }

    /// Цифра внутри обычного текстового запроса — не ISBN. Раньше из «Дюна 2»
    /// вытаскивалась двойка, запрос превращался в `isbn LIKE '%2%'`, и в выдачу
    /// падал весь каталог: двойка есть почти в каждом ISBN.
    #[test]
    fn a_digit_inside_a_text_query_does_not_match_isbns() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::titled("Метро 2033");
        i.isbn = Some("9785171183660".into());
        i.shelf_id = Some(s);
        insert(&conn, &i).unwrap();
        book_on(&conn, s, "Дюна", "Фрэнк Герберт");

        let hits = search(&conn, "дюна 2").unwrap();
        assert!(hits.is_empty(), "текстовый запрос с цифрой выдал книги по ISBN: {hits:?}");

        // а сам по себе цифровой запрос по-прежнему ищет по ISBN
        assert_eq!(search(&conn, "9785171").unwrap().len(), 1);
        // и название с цифрами продолжает находиться по названию
        assert_eq!(search(&conn, "метро 2033").unwrap().len(), 1);
    }

    /// Правка ссылки на обложку должна снимать локальный файл с учёта: иначе
    /// `cover_path` продолжает указывать на прежнюю картинку, вид книги
    /// не меняется никогда, а «Скачать все обложки» такую книгу не видит —
    /// она выбирает только те, у кого `cover_path` пуст.
    #[test]
    fn changing_the_cover_url_forgets_the_downloaded_file() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::titled("Дюна");
        i.cover_url = Some("http://x/old.jpg".into());
        i.shelf_id = Some(s);
        let b = insert(&conn, &i).unwrap();
        set_cover_path(&conn, b.id, Some("1.jpg")).unwrap();
        assert_eq!(get(&conn, b.id).unwrap().cover_path.as_deref(), Some("1.jpg"));

        i.cover_url = Some("http://x/new.jpg".into());
        let upd = update(&conn, b.id, &i).unwrap();
        assert_eq!(upd.cover_path, None, "старый файл остался за новой ссылкой");
        assert_eq!(needing_covers(&conn).unwrap().len(), 1, "книга должна попасть в докачку");
    }

    #[test]
    fn editing_other_fields_keeps_the_downloaded_cover() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::titled("Дюна");
        i.cover_url = Some("http://x/c.jpg".into());
        i.shelf_id = Some(s);
        let b = insert(&conn, &i).unwrap();
        set_cover_path(&conn, b.id, Some("1.jpg")).unwrap();

        i.authors = Some("Фрэнк Герберт".into());
        let upd = update(&conn, b.id, &i).unwrap();
        assert_eq!(upd.cover_path.as_deref(), Some("1.jpg"), "перекачивать было незачем");
    }

    #[test]
    fn rating_is_clamped_and_blank_title_rejected() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::titled("Дюна");
        i.rating = Some(99);
        i.shelf_id = Some(s);
        assert_eq!(insert(&conn, &i).unwrap().rating, Some(5));
        i.rating = Some(-3);
        assert_eq!(insert(&conn, &i).unwrap().rating, Some(0));

        let blank = BookInput::titled("   ");
        assert!(insert(&conn, &blank).is_err());
    }

    #[test]
    fn stats_counts_by_status_and_pages() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::titled("A");
        i.status = Some("read".into());
        i.pages = Some(300);
        i.genre = Some("Фантастика".into());
        i.shelf_id = Some(s);
        insert(&conn, &i).unwrap();
        let st = stats(&conn).unwrap();
        assert_eq!(st.total, 1);
        assert_eq!(st.pages_read, 300);
        assert!(st.by_status.iter().any(|(k, n)| k == "read" && *n == 1));
        assert!(st.top_genres.iter().any(|(g, n)| g == "Фантастика" && *n == 1));
    }
}

