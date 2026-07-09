use crate::db::models::{Book, Location};
use crate::db::{books, locations};
use crate::error::AppError;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Полная резервная копия каталога (портируется между версиями по известным полям).
/// Настройки (в т.ч. API-ключ) намеренно НЕ включаются.
#[derive(Debug, Serialize, Deserialize)]
pub struct Backup {
    pub app: String,
    pub schema: i64,
    pub exported_at: String,
    pub locations: Vec<Location>,
    pub books: Vec<Book>,
}

#[derive(Debug, Serialize)]
pub struct ImportSummary {
    pub locations: usize,
    pub books: usize,
}

pub fn export(conn: &Connection) -> Result<Backup, AppError> {
    Ok(Backup {
        app: "knizhnik".into(),
        schema: 1,
        exported_at: chrono::Utc::now().to_rfc3339(),
        locations: locations::all(conn)?,
        books: books::all(conn)?,
    })
}

/// Заменяет текущий каталог данными из копии (в транзакции, с сохранением id).
pub fn import(conn: &mut Connection, backup: &Backup) -> Result<ImportSummary, AppError> {
    if backup.app != "knizhnik" {
        return Err(AppError::Rule("Это не файл резервной копии Книжника".into()));
    }
    // FK нельзя переключать внутри транзакции — делаем это снаружи.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = replace_all(conn, backup);
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    result
}

fn replace_all(conn: &mut Connection, backup: &Backup) -> Result<ImportSummary, AppError> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM books", [])?;
    tx.execute("DELETE FROM locations", [])?;

    for l in &backup.locations {
        tx.execute(
            "INSERT INTO locations (id, parent_id, name, kind, label, position) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![l.id, l.parent_id, l.name, l.kind, l.label, l.position],
        )?;
    }
    for b in &backup.books {
        tx.execute(
            "INSERT INTO books \
             (id, title, authors, isbn, year, publisher, pages, language, genre, annotation, \
              cover_url, shelf_id, status, rating, notes, availability, lent_to, added_at, updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            params![
                b.id, b.title, b.authors, b.isbn, b.year, b.publisher, b.pages, b.language,
                b.genre, b.annotation, b.cover_url, b.shelf_id, b.status, b.rating, b.notes,
                b.availability, b.lent_to, b.added_at, b.updated_at
            ],
        )?;
    }
    tx.commit()?;
    Ok(ImportSummary { locations: backup.locations.len(), books: backup.books.len() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::BookInput;
    use crate::db::open_in_memory;

    #[test]
    fn export_import_round_trip_preserves_links() {
        let mut conn = open_in_memory().unwrap();
        let root = locations::create(&conn, None, "Дом", "root", None).unwrap().id;
        let case = locations::create(&conn, Some(root), "Шкаф A", "bookcase", None).unwrap().id;
        let shelf = locations::create(&conn, Some(case), "Полка", "shelf", Some("A-3")).unwrap().id;
        let mut i = BookInput::default();
        i.title = "Дюна".into();
        i.isbn = Some("9785171183660".into());
        i.shelf_id = Some(shelf);
        let book = books::insert(&conn, &i).unwrap();

        let backup = export(&conn).unwrap();
        assert_eq!(backup.books.len(), 1);
        assert_eq!(backup.locations.len(), 3);

        // сносим всё и восстанавливаем из копии
        conn.execute("DELETE FROM books", []).unwrap();
        conn.execute("DELETE FROM locations", []).unwrap();
        let summary = import(&mut conn, &backup).unwrap();
        assert_eq!(summary.books, 1);
        assert_eq!(summary.locations, 3);

        // id и связи сохранены → книга на той же полке, брейдкрамб цел
        let restored = books::get(&conn, book.id).unwrap();
        assert_eq!(restored.shelf_id, Some(shelf));
        assert_eq!(restored.title, "Дюна");
        assert_eq!(locations::breadcrumb(&conn, shelf).unwrap(), "Шкаф A › Полка");
    }

    #[test]
    fn import_rejects_foreign_file() {
        let mut conn = open_in_memory().unwrap();
        let bad = Backup {
            app: "other".into(),
            schema: 1,
            exported_at: "".into(),
            locations: vec![],
            books: vec![],
        };
        assert!(import(&mut conn, &bad).is_err());
    }
}
