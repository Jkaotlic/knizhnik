pub mod backup;
pub mod books;
pub mod cache;
pub mod locations;
pub mod models;
pub mod settings;

use crate::error::AppError;
use rusqlite::Connection;
use std::path::Path;

/// Версия схемы = номер последней применённой миграции.
/// Добавляя миграцию, дописывай её в конец MIGRATIONS и никогда не правь
/// уже выпущенные: у пользователей они уже применены.
pub const SCHEMA_VERSION: i64 = LATEST;

/// Конструктор для тестов: их модули разбросаны по файлам, поэтому в обычной
/// сборке функция не вызывается ниоткуда.
#[allow(dead_code)]
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

#[allow(dead_code)] // проверяется тестами миграций
pub fn schema_version(conn: &Connection) -> Result<i64, AppError> {
    // Таблица — журнал применённых миграций, поэтому MAX, а не первая строка.
    let v: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |r| r.get(0),
    )?;
    Ok(v)
}

/// Каждая миграция применяется ровно один раз, в транзакции.
/// Прежний `migrate()` умел только `CREATE TABLE IF NOT EXISTS` — на уже
/// созданной базе он молчал, и добавить колонку в новой версии было нечем.
const MIGRATIONS: &[(i64, &str)] = &[(1, MIGRATION_V1), (2, MIGRATION_V2)];
const LATEST: i64 = MIGRATIONS[MIGRATIONS.len() - 1].0;

pub fn migrate(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")?;
    // Старые сборки писали сюда единственную строку со своей версией —
    // MAX по этой же таблице читает и их состояние тоже.
    let current: i64 =
        conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |r| r.get(0))?;

    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute("INSERT INTO schema_version (version) VALUES (?1)", [version])?;
        tx.commit()?;
    }
    Ok(())
}

const MIGRATION_V1: &str = r#"
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

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT
        );
"#;

const MIGRATION_V2: &str = r#"
        -- Когда книгу начали и дочитали: без этого статистика знает «сколько»,
        -- но не знает «когда», а полка «прочитано в 2026» невозможна.
        ALTER TABLE books ADD COLUMN started_at  TEXT;
        ALTER TABLE books ADD COLUMN finished_at TEXT;

        -- Выдача: кому — было, а когда отдал и когда ждём назад — нет.
        ALTER TABLE books ADD COLUMN lent_at TEXT;
        ALTER TABLE books ADD COLUMN due_at  TEXT;

        -- Путь к скачанной обложке относительно каталога данных приложения:
        -- cover_url ведёт в сеть, и без интернета полка становится серой.
        ALTER TABLE books ADD COLUMN cover_path TEXT;

        -- Один ISBN запрашивается у провайдеров ровно один раз за всё время.
        CREATE TABLE IF NOT EXISTS metadata_cache (
            isbn       TEXT PRIMARY KEY,
            payload    TEXT NOT NULL,
            fetched_at TEXT NOT NULL
        );
"#;

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
        for t in ["locations", "books", "schema_version", "metadata_cache"] {
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

    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).unwrap();
        stmt.query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    /// Главный сценарий ради которого всё затевалось: база, созданная старой
    /// сборкой (v1), должна доехать до текущей схемы без потери данных.
    #[test]
    fn upgrades_a_v1_database_in_place() {
        let conn = Connection::open_in_memory().unwrap();
        // ровно то, что делала прежняя версия
        conn.execute_batch("CREATE TABLE schema_version (version INTEGER NOT NULL);").unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute("INSERT INTO schema_version (version) VALUES (1)", []).unwrap();
        conn.execute(
            "INSERT INTO books (title, availability, added_at, updated_at) \
             VALUES ('Будущее', 'on_shelf', 'вчера', 'вчера')",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();

        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
        let cols = columns(&conn, "books");
        for c in ["started_at", "finished_at", "lent_at", "due_at", "cover_path"] {
            assert!(cols.contains(&c.to_string()), "миграция не добавила {c}");
        }
        // книга на месте
        let title: String =
            conn.query_row("SELECT title FROM books", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "Будущее");
    }

    #[test]
    fn migrations_are_applied_once_and_only_once() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        // ALTER TABLE ADD COLUMN упал бы при повторном прогоне — значит не прогнали
        let applied: i64 =
            conn.query_row("SELECT count(*) FROM schema_version", [], |r| r.get(0)).unwrap();
        assert_eq!(applied, MIGRATIONS.len() as i64);
    }

    #[test]
    fn schema_version_matches_last_migration() {
        assert_eq!(SCHEMA_VERSION, MIGRATIONS.last().unwrap().0);
    }
}
