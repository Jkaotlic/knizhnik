use crate::db::models::Location;
use crate::domain::breadcrumb::format_breadcrumb;
use crate::error::AppError;
use rusqlite::{params, Connection};

pub fn create(
    conn: &Connection,
    parent_id: Option<i64>,
    name: &str,
    kind: &str,
    label: Option<&str>,
) -> Result<Location, AppError> {
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM locations \
         WHERE parent_id IS ?1",
        params![parent_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO locations (parent_id, name, kind, label, position) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![parent_id, name, kind, label, position],
    )?;
    let id = conn.last_insert_rowid();
    get(conn, id)
}

pub fn get(conn: &Connection, id: i64) -> Result<Location, AppError> {
    let loc = conn.query_row(
        "SELECT id, parent_id, name, kind, label, position FROM locations WHERE id = ?1",
        params![id],
        row_to_location,
    )?;
    Ok(loc)
}

pub fn all(conn: &Connection) -> Result<Vec<Location>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, name, kind, label, position FROM locations \
         ORDER BY parent_id, position",
    )?;
    let rows = stmt.query_map([], row_to_location)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn update(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    label: Option<&str>,
) -> Result<Location, AppError> {
    if let Some(n) = name {
        conn.execute("UPDATE locations SET name = ?1 WHERE id = ?2", params![n, id])?;
    }
    if let Some(l) = label {
        conn.execute("UPDATE locations SET label = ?1 WHERE id = ?2", params![l, id])?;
    }
    get(conn, id)
}

pub fn move_to(conn: &Connection, id: i64, new_parent_id: Option<i64>) -> Result<(), AppError> {
    conn.execute(
        "UPDATE locations SET parent_id = ?1 WHERE id = ?2",
        params![new_parent_id, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
    let kind: String =
        conn.query_row("SELECT kind FROM locations WHERE id = ?1", params![id], |r| r.get(0))?;
    if kind == "shelf" {
        let books: i64 = conn.query_row(
            "SELECT count(*) FROM books WHERE shelf_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        if books > 0 {
            return Err(AppError::Rule(
                "На полке есть книги, сначала перенесите их".to_string(),
            ));
        }
    }
    conn.execute("DELETE FROM locations WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn breadcrumb(conn: &Connection, shelf_id: i64) -> Result<String, AppError> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE path(id, name, kind, parent_id) AS ( \
            SELECT id, name, kind, parent_id FROM locations WHERE id = ?1 \
            UNION ALL \
            SELECT l.id, l.name, l.kind, l.parent_id \
            FROM locations l JOIN path p ON l.id = p.parent_id \
         ) SELECT name, kind FROM path",
    )?;
    let segments = stmt
        .query_map(params![shelf_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format_breadcrumb(&segments))
}

fn row_to_location(r: &rusqlite::Row) -> rusqlite::Result<Location> {
    Ok(Location {
        id: r.get(0)?,
        parent_id: r.get(1)?,
        name: r.get(2)?,
        kind: r.get(3)?,
        label: r.get(4)?,
        position: r.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn tree(conn: &Connection) -> (i64, i64, i64, i64) {
        let root = create(conn, None, "Дом", "root", None).unwrap().id;
        let room = create(conn, Some(root), "Гостиная", "room", None).unwrap().id;
        let case = create(conn, Some(room), "Шкаф A", "bookcase", None).unwrap().id;
        let shelf = create(conn, Some(case), "Полка", "shelf", Some("A-3")).unwrap().id;
        (root, room, case, shelf)
    }

    #[test]
    fn breadcrumb_builds_full_path() {
        let conn = open_in_memory().unwrap();
        let (_r, _room, _case, shelf) = tree(&conn);
        assert_eq!(breadcrumb(&conn, shelf).unwrap(), "Гостиная › Шкаф A › Полка");
    }

    #[test]
    fn move_changes_parent_only() {
        let conn = open_in_memory().unwrap();
        let (root, _room, _case, shelf) = tree(&conn);
        let case2 = create(&conn, Some(root), "Шкаф B", "bookcase", None).unwrap().id;
        move_to(&conn, shelf, Some(case2)).unwrap();
        assert_eq!(get(&conn, shelf).unwrap().parent_id, Some(case2));
    }

    // re-enable in Task 6 (needs db::books)
    // #[test]
    // fn delete_shelf_with_books_is_rejected() {
    //     let conn = open_in_memory().unwrap();
    //     let (_r, _room, _case, shelf) = tree(&conn);
    //     let mut input = crate::db::models::BookInput::default();
    //     input.title = "Дюна".into();
    //     input.shelf_id = Some(shelf);
    //     crate::db::books::insert(&conn, &input).unwrap();
    //     let err = delete(&conn, shelf).unwrap_err();
    //     assert!(matches!(err, AppError::Rule(_)));
    //     // книга цела
    //     assert_eq!(crate::db::books::on_shelf(&conn, shelf).unwrap().len(), 1);
    // }
}
