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

pub fn insert(conn: &Connection, i: &BookInput) -> Result<Book, AppError> {
    let ts = now();
    conn.execute(
        "INSERT INTO books \
         (title, authors, isbn, year, publisher, pages, language, genre, annotation, \
          cover_url, shelf_id, status, rating, notes, availability, lent_to, added_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'on_shelf',NULL,?15,?15)",
        params![
            i.title, i.authors, i.isbn, i.year, i.publisher, i.pages, i.language,
            i.genre, i.annotation, i.cover_url, i.shelf_id, i.status, i.rating, i.notes, ts
        ],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, i: &BookInput) -> Result<Book, AppError> {
    conn.execute(
        "UPDATE books SET title=?2, authors=?3, isbn=?4, year=?5, publisher=?6, pages=?7, \
         language=?8, genre=?9, annotation=?10, cover_url=?11, shelf_id=?12, status=?13, \
         rating=?14, notes=?15, updated_at=?16 WHERE id=?1",
        params![
            id, i.title, i.authors, i.isbn, i.year, i.publisher, i.pages, i.language,
            i.genre, i.annotation, i.cover_url, i.shelf_id, i.status, i.rating, i.notes, now()
        ],
    )?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute("DELETE FROM books WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_shelf(conn: &Connection, id: i64, shelf_id: Option<i64>) -> Result<(), AppError> {
    conn.execute(
        "UPDATE books SET shelf_id=?1, updated_at=?2 WHERE id=?3",
        params![shelf_id, now(), id],
    )?;
    Ok(())
}

pub fn set_availability(
    conn: &Connection,
    id: i64,
    availability: &str,
    lent_to: Option<&str>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE books SET availability=?1, lent_to=?2, updated_at=?3 WHERE id=?4",
        params![availability, lent_to, now(), id],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: i64) -> Result<Book, AppError> {
    Ok(conn.query_row(SELECT_BOOK_BY_ID, params![id], row_to_book)?)
}

pub fn on_shelf(conn: &Connection, shelf_id: i64) -> Result<Vec<Book>, AppError> {
    let mut stmt = conn.prepare(&format!("{SELECT_BOOK_COLS} WHERE shelf_id=?1 ORDER BY title"))?;
    let rows = stmt.query_map(params![shelf_id], row_to_book)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn exists_isbn_on_shelf(conn: &Connection, isbn: &str, shelf_id: i64) -> Result<bool, AppError> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM books WHERE isbn=?1 AND shelf_id=?2",
        params![isbn, shelf_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
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

pub fn search(conn: &Connection, query: &str) -> Result<Vec<BookHit>, AppError> {
    register_unicode_lower(conn)?;
    let like = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(&format!(
        "{SELECT_BOOK_COLS} WHERE \
         lower_uc(title) LIKE ?1 OR lower_uc(authors) LIKE ?1 OR \
         lower_uc(isbn) LIKE ?1 OR lower_uc(genre) LIKE ?1 ORDER BY title"
    ))?;
    let books = stmt
        .query_map(params![like], row_to_book)?
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
    Ok(Stats { total, by_status, pages_read, top_genres })
}

const SELECT_BOOK_COLS: &str = "SELECT id, title, authors, isbn, year, publisher, pages, \
    language, genre, annotation, cover_url, shelf_id, status, rating, notes, availability, \
    lent_to, added_at, updated_at FROM books";

const SELECT_BOOK_BY_ID: &str = "SELECT id, title, authors, isbn, year, publisher, pages, \
    language, genre, annotation, cover_url, shelf_id, status, rating, notes, availability, \
    lent_to, added_at, updated_at FROM books WHERE id = ?1";

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
        let mut i = BookInput::default();
        i.title = title.into();
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
        let mut i = BookInput::default();
        i.title = "Дюна".into();
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
        assert_eq!(hits[0].breadcrumb, "Гостиная › Шкаф A › Полка");
        assert!(!hits[0].off_shelf);
    }

    #[test]
    fn lent_book_marked_off_shelf() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let b = book_on(&conn, s, "Дюна", "Герберт");
        set_availability(&conn, b.id, "lent", Some("Маша")).unwrap();
        let hits = search(&conn, "дюна").unwrap();
        assert!(hits[0].off_shelf);
        assert_eq!(hits[0].book.lent_to, Some("Маша".into()));
    }

    #[test]
    fn exists_isbn_on_shelf_detects_duplicate() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::default();
        i.title = "Дюна".into();
        i.isbn = Some("9785171183660".into());
        i.shelf_id = Some(s);
        insert(&conn, &i).unwrap();
        assert!(exists_isbn_on_shelf(&conn, "9785171183660", s).unwrap());
        assert!(!exists_isbn_on_shelf(&conn, "9780000000000", s).unwrap());
    }

    #[test]
    fn stats_counts_by_status_and_pages() {
        let conn = open_in_memory().unwrap();
        let s = shelf(&conn);
        let mut i = BookInput::default();
        i.title = "A".into();
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
