use crate::error::AppError;
use rusqlite::{params, Connection, OptionalExtension};

/// Прочитать значение настройки; None, если не задано.
pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    let v = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .optional()?;
    Ok(v)
}

/// Записать (Some) или удалить (None) настройку.
pub fn set(conn: &Connection, key: &str, value: Option<&str>) -> Result<(), AppError> {
    match value {
        Some(v) => conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, v],
        )?,
        None => conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?,
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn set_get_update_delete() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get(&conn, "k").unwrap(), None);
        set(&conn, "k", Some("v1")).unwrap();
        assert_eq!(get(&conn, "k").unwrap(), Some("v1".into()));
        set(&conn, "k", Some("v2")).unwrap();
        assert_eq!(get(&conn, "k").unwrap(), Some("v2".into()));
        set(&conn, "k", None).unwrap();
        assert_eq!(get(&conn, "k").unwrap(), None);
    }
}
