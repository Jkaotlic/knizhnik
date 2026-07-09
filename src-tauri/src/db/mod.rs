pub mod books;
pub mod locations;
pub mod models;

use crate::error::AppError;
use rusqlite::Connection;
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

pub fn open_in_memory() -> Result<Connection, AppError> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_at(path: &Path) -> Result<Connection, AppError> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn schema_version(conn: &Connection) -> Result<i64, AppError> {
    let v: i64 = conn.query_row(
        "SELECT version FROM schema_version LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    Ok(v)
}

pub fn migrate(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS locations (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            parent_id INTEGER REFERENCES locations(id),
            name      TEXT NOT NULL,
            kind      TEXT NOT NULL CHECK (kind IN ('root','room','bookcase','shelf')),
            label     TEXT,
            position  INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS books (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            title        TEXT NOT NULL,
            authors      TEXT,
            isbn         TEXT,
            year         INTEGER,
            publisher    TEXT,
            pages        INTEGER,
            language     TEXT,
            genre        TEXT,
            annotation   TEXT,
            cover_url    TEXT,
            shelf_id     INTEGER REFERENCES locations(id),
            status       TEXT CHECK (status IN ('want','reading','read')),
            rating       INTEGER,
            notes        TEXT,
            availability TEXT NOT NULL DEFAULT 'on_shelf'
                         CHECK (availability IN ('on_shelf','lent','away')),
            lent_to      TEXT,
            added_at     TEXT NOT NULL,
            updated_at   TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_books_shelf ON books(shelf_id);
        CREATE INDEX IF NOT EXISTS idx_books_isbn ON books(isbn);
        "#,
    )?;

    let count: i64 =
        conn.query_row("SELECT count(*) FROM schema_version", [], |r| r.get(0))?;
    if count == 0 {
        conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [SCHEMA_VERSION])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_is_idempotent_and_sets_version() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap(); // второй прогон не падает
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn tables_exist_after_migrate() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        for t in ["locations", "books", "schema_version"] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "нет таблицы {t}");
        }
    }
}
