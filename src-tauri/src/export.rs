use crate::error::AppError;
use rusqlite::Connection;

const COLS: usize = 15;

/// Excel на macOS и Windows читает CSV без BOM как cp1251 — кириллица
/// превращается в кракозябры. BOM это лечит.
const BOM: &str = "\u{feff}";

pub fn export_csv(conn: &Connection) -> Result<String, AppError> {
    let header = "id;title;authors;isbn;year;publisher;pages;language;genre;\
                  status;rating;availability;lent_to;shelf_id;added_at;shelf";
    let mut out = String::from(BOM);
    out.push_str(header);
    out.push('\n');

    let mut stmt = conn.prepare(
        "SELECT id, title, authors, isbn, year, publisher, pages, language, genre, \
         status, rating, availability, lent_to, shelf_id, added_at FROM books ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let mut fields = Vec::with_capacity(COLS + 1);
            for idx in 0..COLS {
                fields.push(escape(&cell(r.get::<_, rusqlite::types::Value>(idx)?)));
            }
            Ok((r.get::<_, Option<i64>>(13)?, fields))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Голый shelf_id человеку ничего не говорит — дописываем путь к полке.
    for (shelf_id, mut fields) in rows {
        let crumb = match shelf_id {
            Some(sid) => crate::db::locations::breadcrumb(conn, sid)?,
            None => String::new(),
        };
        fields.push(escape(&crumb));
        out.push_str(&fields.join(";"));
        out.push('\n');
    }
    Ok(out)
}

fn cell(v: rusqlite::types::Value) -> String {
    match v {
        rusqlite::types::Value::Null => String::new(),
        rusqlite::types::Value::Integer(n) => n.to_string(),
        rusqlite::types::Value::Real(f) => f.to_string(),
        rusqlite::types::Value::Text(s) => s,
        rusqlite::types::Value::Blob(_) => String::new(),
    }
}

fn escape(field: &str) -> String {
    if field.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{books, locations, models::BookInput, open_in_memory};

    #[test]
    fn exports_header_and_row_per_book() {
        let conn = open_in_memory().unwrap();
        let root = locations::create(&conn, None, "Дом", "root", None).unwrap().id;
        let shelf = locations::create(&conn, Some(root), "Полка", "shelf", Some("A-3")).unwrap().id;
        let mut i = BookInput::titled("Дюна; часть 1"); // точка с запятой — проверка экранирования
        i.authors = Some("Фрэнк Герберт".into());
        i.isbn = Some("9785171183660".into());
        i.shelf_id = Some(shelf);
        books::insert(&conn, &i).unwrap();

        let csv = export_csv(&conn).unwrap();
        assert!(csv.starts_with(BOM), "без BOM Excel ломает кириллицу");
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2); // заголовок + 1 книга
        assert!(lines[0].starts_with("\u{feff}id;title;authors;isbn"));
        assert!(lines[0].ends_with(";shelf"));
        assert!(lines[1].contains("\"Дюна; часть 1\"")); // поле экранировано
        assert!(lines[1].contains("9785171183660"));
        assert!(lines[1].ends_with(";Полка"), "должен быть путь к полке: {}", lines[1]);
    }

    #[test]
    fn empty_catalog_has_only_header() {
        let conn = open_in_memory().unwrap();
        let csv = export_csv(&conn).unwrap();
        assert_eq!(csv.lines().count(), 1);
    }
}
