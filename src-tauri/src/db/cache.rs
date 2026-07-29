use crate::domain::matching::MetadataCandidate;
use crate::error::AppError;
use rusqlite::{params, Connection, OptionalExtension};

// Один ISBN сканируется один раз в жизни каталога, а запрос к провайдерам
// летел при каждом скане. С кэшем дневная квота Google перестаёт упираться
// в размер библиотеки: 300 книг — это 300 запросов за всё время, не за день.
//
// Промахи (пустой ответ) намеренно НЕ кэшируем: книга может появиться в базах
// позже, и вечный «не найдено» было бы нечем сбросить.

pub fn get(conn: &Connection, isbn: &str) -> Result<Option<Vec<MetadataCandidate>>, AppError> {
    let payload: Option<String> = conn
        .query_row(
            "SELECT payload FROM metadata_cache WHERE isbn = ?1",
            params![isbn],
            |r| r.get(0),
        )
        .optional()?;
    let Some(payload) = payload else {
        return Ok(None);
    };
    // Битую или устаревшую по формату запись молча считаем промахом:
    // перезапросить дешевле, чем уронить сканирование.
    Ok(serde_json::from_str(&payload).ok())
}

pub fn put(conn: &Connection, isbn: &str, candidates: &[MetadataCandidate]) -> Result<(), AppError> {
    if candidates.is_empty() {
        return Ok(());
    }
    let payload =
        serde_json::to_string(candidates).map_err(|e| AppError::Rule(e.to_string()))?;
    conn.execute(
        "INSERT INTO metadata_cache (isbn, payload, fetched_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT(isbn) DO UPDATE SET payload = ?2, fetched_at = ?3",
        params![isbn, payload, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn clear(conn: &Connection) -> Result<usize, AppError> {
    Ok(conn.execute("DELETE FROM metadata_cache", [])?)
}

pub fn size(conn: &Connection) -> Result<i64, AppError> {
    Ok(conn.query_row("SELECT count(*) FROM metadata_cache", [], |r| r.get(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn cand(title: &str) -> MetadataCandidate {
        MetadataCandidate {
            title: title.into(),
            authors: Some("Дмитрий Глуховский".into()),
            isbn: Some("9785171183660".into()),
            year: Some(2019),
            publisher: Some("АСТ".into()),
            pages: Some(480),
            language: Some("ru".into()),
            cover_url: None,
            source: "google".into(),
        }
    }

    #[test]
    fn round_trips_candidates() {
        let conn = open_in_memory().unwrap();
        assert!(get(&conn, "9785171183660").unwrap().is_none());
        put(&conn, "9785171183660", &[cand("Будущее")]).unwrap();
        let hit = get(&conn, "9785171183660").unwrap().unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].title, "Будущее");
        assert_eq!(hit[0].publisher.as_deref(), Some("АСТ"));
    }

    #[test]
    fn empty_result_is_not_cached() {
        let conn = open_in_memory().unwrap();
        put(&conn, "9785171183660", &[]).unwrap();
        // промах должен перезапроситься, а не залипнуть навсегда
        assert!(get(&conn, "9785171183660").unwrap().is_none());
        assert_eq!(size(&conn).unwrap(), 0);
    }

    #[test]
    fn repeated_put_overwrites() {
        let conn = open_in_memory().unwrap();
        put(&conn, "9785171183660", &[cand("Старое")]).unwrap();
        put(&conn, "9785171183660", &[cand("Новое")]).unwrap();
        assert_eq!(size(&conn).unwrap(), 1);
        assert_eq!(get(&conn, "9785171183660").unwrap().unwrap()[0].title, "Новое");
    }

    #[test]
    fn corrupt_payload_reads_as_a_miss() {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO metadata_cache (isbn, payload, fetched_at) VALUES ('x', '{не json', 'now')",
            [],
        )
        .unwrap();
        assert!(get(&conn, "x").unwrap().is_none());
    }

    #[test]
    fn clear_empties_the_cache() {
        let conn = open_in_memory().unwrap();
        put(&conn, "9785171183660", &[cand("Будущее")]).unwrap();
        assert_eq!(clear(&conn).unwrap(), 1);
        assert_eq!(size(&conn).unwrap(), 0);
    }
}
