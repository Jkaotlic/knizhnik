use crate::error::AppError;
use rusqlite::Connection;

pub fn export_csv(conn: &Connection) -> Result<String, AppError> {
    let header = "id;title;authors;isbn;year;publisher;pages;language;genre;\
                  status;rating;availability;lent_to;shelf_id;added_at";
    let mut out = String::from(header);
    out.push('\n');
    let mut stmt = conn.prepare(
        "SELECT id, title, authors, isbn, year, publisher, pages, language, genre, \
         status, rating, availability, lent_to, shelf_id, added_at FROM books ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        let cell = |v: rusqlite::types::Value| -> String {
            match v {
                rusqlite::types::Value::Null => String::new(),
                rusqlite::types::Value::Integer(n) => n.to_string(),
                rusqlite::types::Value::Real(f) => f.to_string(),
                rusqlite::types::Value::Text(s) => s,
                rusqlite::types::Value::Blob(_) => String::new(),
            }
        };
        let mut fields = Vec::with_capacity(15);
        for idx in 0..15 {
            fields.push(escape(&cell(r.get::<_, rusqlite::types::Value>(idx)?)));
        }
        Ok(fields.join(";"))
    })?;
    for line in rows {
        out.push_str(&line?);
        out.push('\n');
    }
    Ok(out)
}

fn escape(field: &str) -> String {
    if field.contains(';') || field.contains('"') || field.contains('\n') {
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
        let mut i = BookInput::default();
        i.title = "Дюна; часть 1".into(); // проверяем экранирование
        i.authors = Some("Фрэнк Герберт".into());
        i.isbn = Some("9785171183660".into());
        i.shelf_id = Some(shelf);
        books::insert(&conn, &i).unwrap();

        let csv = export_csv(&conn).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2); // заголовок + 1 книга
        assert!(lines[0].starts_with("id;title;authors;isbn"));
        assert!(lines[1].contains("\"Дюна; часть 1\"")); // поле экранировано
        assert!(lines[1].contains("9785171183660"));
    }

    #[test]
    fn empty_catalog_has_only_header() {
        let conn = open_in_memory().unwrap();
        let csv = export_csv(&conn).unwrap();
        assert_eq!(csv.lines().count(), 1);
    }
}
