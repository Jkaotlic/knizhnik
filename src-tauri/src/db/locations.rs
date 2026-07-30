use crate::db::models::Location;
use crate::domain::breadcrumb::format_breadcrumb;
use crate::error::AppError;
use rusqlite::{params, Connection};

/// Пустая строка в метке — это «метки нет», а не метка из пробелов.
fn clean_label(label: Option<&str>) -> Option<&str> {
    label.map(str::trim).filter(|s| !s.is_empty())
}

/// Человеческое название уровня — ошибки читает не программист.
fn kind_ru(kind: &str) -> &str {
    match kind {
        "root" => "дом",
        "room" => "комната",
        "bookcase" => "шкаф",
        "shelf" => "полка",
        other => other,
    }
}

fn clean_name(name: &str) -> Result<&str, AppError> {
    let n = name.trim();
    if n.is_empty() {
        return Err(AppError::Rule("Название не может быть пустым".into()));
    }
    Ok(n)
}

pub fn create(
    conn: &Connection,
    parent_id: Option<i64>,
    name: &str,
    kind: &str,
    label: Option<&str>,
) -> Result<Location, AppError> {
    let name = clean_name(name)?;
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM locations \
         WHERE parent_id IS ?1",
        params![parent_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO locations (parent_id, name, kind, label, position) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![parent_id, name, kind, clean_label(label), position],
    )?;
    let id = conn.last_insert_rowid();
    get(conn, id)
}

fn first_of_kind(conn: &Connection, kind: &str) -> Result<Option<i64>, AppError> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT id FROM locations WHERE kind = ?1 ORDER BY id LIMIT 1",
            params![kind],
            |r| r.get(0),
        )
        .optional()?)
}

/// Возвращает комнату, в которой живут шкафы, создавая «Дом» и «Комнату», если
/// их ещё нет. Это единственное место, знающее про служебные уровни: в обычном
/// интерфейсе их не заводят руками и не показывают.
/// Существующие корень и комната переиспользуются — дубликаты не плодятся.
pub fn ensure_home(conn: &Connection) -> Result<i64, AppError> {
    if let Some(room) = first_of_kind(conn, "room")? {
        return Ok(room);
    }
    let root = match first_of_kind(conn, "root")? {
        Some(id) => id,
        None => create(conn, None, "Дом", "root", None)?.id,
    };
    Ok(create(conn, Some(root), "Комната", "room", None)?.id)
}

pub fn create_bookcase(conn: &Connection, name: &str) -> Result<Location, AppError> {
    let room = ensure_home(conn)?;
    create(conn, Some(room), name, "bookcase", None)
}

pub fn create_shelf(
    conn: &Connection,
    bookcase_id: i64,
    name: &str,
    label: Option<&str>,
) -> Result<Location, AppError> {
    // Без этой проверки полка молча уехала бы в комнату или внутрь другой полки,
    // и брейдкрамб начал бы врать.
    match get(conn, bookcase_id) {
        Ok(parent) if parent.kind == "bookcase" => {}
        Ok(parent) => {
            return Err(AppError::Rule(format!(
                "«{}» — это не шкаф, полку туда положить нельзя",
                parent.name
            )))
        }
        Err(_) => return Err(AppError::Rule("Шкаф не найден".into())),
    }
    create(conn, Some(bookcase_id), name, "shelf", label)
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

/// `name = None` — не трогать. `label = Some("")` — снять метку.
pub fn update(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    label: Option<&str>,
) -> Result<Location, AppError> {
    if let Some(n) = name {
        let n = clean_name(n)?;
        conn.execute("UPDATE locations SET name = ?1 WHERE id = ?2", params![n, id])?;
    }
    if label.is_some() {
        conn.execute(
            "UPDATE locations SET label = ?1 WHERE id = ?2",
            params![clean_label(label), id],
        )?;
    }
    get(conn, id)
}

/// Уровни жёсткие: полка живёт в шкафу, шкаф — в комнате, комната — в доме.
/// `None` — только у дома.
fn expected_parent_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "room" => Some("root"),
        "bookcase" => Some("room"),
        "shelf" => Some("bookcase"),
        _ => None, // root
    }
}

pub fn move_to(conn: &Connection, id: i64, new_parent_id: Option<i64>) -> Result<(), AppError> {
    // Перенос локации внутрь самой себя порвал бы дерево: рекурсивный брейдкрамб
    // и обход в UI зациклились бы. Проверяем до записи.
    if let Some(target) = new_parent_id {
        if target == id || subtree_ids(conn, id)?.contains(&target) {
            return Err(AppError::Rule(
                "Нельзя перенести локацию внутрь самой себя".into(),
            ));
        }
    }
    // `create_shelf` уровни держит, а перенос их обходил: шкаф уезжал внутрь
    // полки, и путь к книге начинал врать «Полка › Шкаф › Полка».
    let moved = get(conn, id)?;
    let wanted = expected_parent_kind(&moved.kind);
    match (wanted, new_parent_id) {
        (None, None) => {}
        (None, Some(_)) => {
            return Err(AppError::Rule(format!("«{}» — это дом, его некуда вкладывать", moved.name)))
        }
        (Some(kind), None) => {
            return Err(AppError::Rule(format!(
                "«{}» не может остаться без родителя: {} живёт внутри «{}»",
                moved.name,
                kind_ru(&moved.kind),
                kind_ru(kind)
            )))
        }
        (Some(kind), Some(target)) => {
            let parent = get(conn, target)
                .map_err(|_| AppError::Rule("Новый родитель не найден".into()))?;
            if parent.kind != kind {
                return Err(AppError::Rule(format!(
                    "«{}» ({}) нельзя положить в «{}» ({}) — только в «{}»",
                    moved.name,
                    kind_ru(&moved.kind),
                    parent.name,
                    kind_ru(&parent.kind),
                    kind_ru(kind)
                )));
            }
        }
    }
    conn.execute(
        "UPDATE locations SET parent_id = ?1 WHERE id = ?2",
        params![new_parent_id, id],
    )?;
    Ok(())
}

/// Локация + все потомки, от самых глубоких к корню поддерева.
/// Такой порядок позволяет удалять, не нарушая внешний ключ parent_id.
fn subtree_ids(conn: &Connection, id: i64) -> Result<Vec<i64>, AppError> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE sub(id, depth) AS ( \
            SELECT id, 0 FROM locations WHERE id = ?1 \
            UNION ALL \
            SELECT l.id, s.depth + 1 FROM locations l JOIN sub s ON l.parent_id = s.id \
         ) SELECT id FROM sub ORDER BY depth DESC",
    )?;
    let ids = stmt
        .query_map(params![id], |r| r.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// Сколько вложенных локаций и книг зацепит удаление — чтобы спросить осмысленно.
pub fn subtree_info(conn: &Connection, id: i64) -> Result<(i64, i64), AppError> {
    let ids = subtree_ids(conn, id)?;
    if ids.is_empty() {
        return Err(AppError::Rule("Локация не найдена".into()));
    }
    let books = books_in_subtree(conn, id)?;
    Ok((ids.len() as i64 - 1, books))
}

fn books_in_subtree(conn: &Connection, id: i64) -> Result<i64, AppError> {
    let n: i64 = conn.query_row(
        "WITH RECURSIVE sub(id) AS ( \
            SELECT id FROM locations WHERE id = ?1 \
            UNION ALL \
            SELECT l.id FROM locations l JOIN sub s ON l.parent_id = s.id \
         ) SELECT count(*) FROM books WHERE shelf_id IN (SELECT id FROM sub)",
        params![id],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Удаляет локацию вместе со всем поддеревом. Раньше удалялась только сама
/// строка — на комнате со шкафами это упиралось в FOREIGN KEY и вылезало
/// пользователю как «Ошибка базы данных».
pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
    let ids = subtree_ids(conn, id)?;
    if ids.is_empty() {
        return Err(AppError::Rule("Локация не найдена".into()));
    }
    let books = books_in_subtree(conn, id)?;
    if books > 0 {
        return Err(AppError::Rule(format!(
            "Внутри ещё {books} кн. — сначала перенесите или удалите их"
        )));
    }
    let tx = conn.unchecked_transaction()?;
    for child in &ids {
        tx.execute("DELETE FROM locations WHERE id = ?1", params![child])?;
    }
    tx.commit()?;
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
    // Комната имеет смысл в пути, только когда комнат несколько.
    let rooms: i64 =
        conn.query_row("SELECT count(*) FROM locations WHERE kind = 'room'", [], |r| r.get(0))?;
    Ok(format_breadcrumb(&segments, rooms > 1))
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
    fn breadcrumb_hides_the_only_room() {
        let conn = open_in_memory().unwrap();
        let (_r, _room, _case, shelf) = tree(&conn);
        assert_eq!(breadcrumb(&conn, shelf).unwrap(), "Шкаф A › Полка");
    }

    #[test]
    fn breadcrumb_shows_the_room_once_there_are_two() {
        let conn = open_in_memory().unwrap();
        let (root, _room, _case, shelf) = tree(&conn);
        create(&conn, Some(root), "Спальня", "room", None).unwrap();
        assert_eq!(breadcrumb(&conn, shelf).unwrap(), "Гостиная › Шкаф A › Полка");
    }

    #[test]
    fn ensure_home_reuses_what_already_exists() {
        let conn = open_in_memory().unwrap();
        let (_root, room, _case, _shelf) = tree(&conn);
        assert_eq!(ensure_home(&conn).unwrap(), room);
        assert_eq!(ensure_home(&conn).unwrap(), room);
        // ни одного лишнего дома или комнаты
        assert_eq!(all(&conn).unwrap().iter().filter(|l| l.kind == "root").count(), 1);
        assert_eq!(all(&conn).unwrap().iter().filter(|l| l.kind == "room").count(), 1);
    }

    #[test]
    fn a_shelf_can_be_created_on_an_empty_catalogue_in_one_go() {
        let conn = open_in_memory().unwrap();
        let case = create_bookcase(&conn, "Шкаф у окна").unwrap();
        let shelf = create_shelf(&conn, case.id, "Верхняя", Some("В-1")).unwrap();

        assert_eq!(shelf.kind, "shelf");
        assert_eq!(shelf.parent_id, Some(case.id));
        assert_eq!(shelf.label.as_deref(), Some("В-1"));
        // служебные уровни поднялись сами и в пути не видны
        assert_eq!(all(&conn).unwrap().len(), 4);
        assert_eq!(breadcrumb(&conn, shelf.id).unwrap(), "Шкаф у окна › Верхняя");
    }

    #[test]
    fn second_bookcase_lands_in_the_same_room() {
        let conn = open_in_memory().unwrap();
        let a = create_bookcase(&conn, "Шкаф A").unwrap();
        let b = create_bookcase(&conn, "Шкаф B").unwrap();
        assert_eq!(a.parent_id, b.parent_id);
        assert_eq!(all(&conn).unwrap().iter().filter(|l| l.kind == "room").count(), 1);
    }

    #[test]
    fn a_shelf_refuses_to_live_anywhere_but_a_bookcase() {
        let conn = open_in_memory().unwrap();
        let (root, room, _case, shelf) = tree(&conn);
        for wrong in [root, room, shelf] {
            assert!(
                create_shelf(&conn, wrong, "Полка", None).is_err(),
                "полка не должна создаваться внутри id={wrong}"
            );
        }
        assert!(create_shelf(&conn, 9999, "Полка", None).is_err());
    }

    #[test]
    fn move_changes_parent_only() {
        let conn = open_in_memory().unwrap();
        let (root, _room, _case, shelf) = tree(&conn);
        let case2 = create(&conn, Some(root), "Шкаф B", "bookcase", None).unwrap().id;
        move_to(&conn, shelf, Some(case2)).unwrap();
        assert_eq!(get(&conn, shelf).unwrap().parent_id, Some(case2));
    }

    #[test]
    fn delete_shelf_with_books_is_rejected() {
        let conn = open_in_memory().unwrap();
        let (_r, _room, _case, shelf) = tree(&conn);
        let mut input = crate::db::models::BookInput::titled("Дюна");
        input.shelf_id = Some(shelf);
        crate::db::books::insert(&conn, &input).unwrap();
        let err = delete(&conn, shelf).unwrap_err();
        assert!(matches!(err, AppError::Rule(_)));
        // книга цела
        assert_eq!(crate::db::books::on_shelf(&conn, shelf).unwrap().len(), 1);
    }

    #[test]
    fn delete_removes_whole_subtree() {
        let conn = open_in_memory().unwrap();
        let (root, room, _case, _shelf) = tree(&conn);
        delete(&conn, room).unwrap();
        let left = all(&conn).unwrap();
        assert_eq!(left.len(), 1); // остался только корень
        assert_eq!(left[0].id, root);
    }

    #[test]
    fn delete_subtree_with_books_deeper_down_is_rejected() {
        let conn = open_in_memory().unwrap();
        let (_root, room, _case, shelf) = tree(&conn);
        let mut input = crate::db::models::BookInput::titled("Дюна");
        input.shelf_id = Some(shelf);
        crate::db::books::insert(&conn, &input).unwrap();
        // удаляем комнату, книга лежит на полке двумя уровнями ниже
        let err = delete(&conn, room).unwrap_err();
        assert!(matches!(err, AppError::Rule(_)), "ожидали понятное правило, а не ошибку БД");
        assert_eq!(all(&conn).unwrap().len(), 4); // ничего не снесли
    }

    #[test]
    fn subtree_info_counts_descendants_and_books() {
        let conn = open_in_memory().unwrap();
        let (_root, room, _case, shelf) = tree(&conn);
        let mut input = crate::db::models::BookInput::titled("Дюна");
        input.shelf_id = Some(shelf);
        crate::db::books::insert(&conn, &input).unwrap();
        assert_eq!(subtree_info(&conn, room).unwrap(), (2, 1)); // шкаф + полка, 1 книга
    }

    /// `create_shelf` держит уровни строго (полка живёт только в шкафу),
    /// а перенос эту же проверку обходил: шкаф уезжал внутрь полки, и путь
    /// к книге начинал врать «Полка › Шкаф › Полка».
    #[test]
    fn move_refuses_to_break_the_level_order() {
        let conn = open_in_memory().unwrap();
        let (root, room, case, shelf) = tree(&conn);
        let case2 = create(&conn, Some(room), "Шкаф B", "bookcase", None).unwrap().id;

        assert!(move_to(&conn, case2, Some(shelf)).is_err(), "шкаф внутрь полки");
        assert!(move_to(&conn, shelf, Some(room)).is_err(), "полка прямо в комнату");
        assert!(move_to(&conn, shelf, Some(root)).is_err(), "полка в дом");
        assert!(move_to(&conn, room, Some(case)).is_err(), "комната в шкаф");
        // дерево не тронуто
        assert_eq!(get(&conn, case2).unwrap().parent_id, Some(room));
        assert_eq!(get(&conn, shelf).unwrap().parent_id, Some(case));

        // законный перенос по-прежнему проходит
        move_to(&conn, shelf, Some(case2)).unwrap();
        assert_eq!(get(&conn, shelf).unwrap().parent_id, Some(case2));
    }

    #[test]
    fn only_a_root_may_end_up_without_a_parent() {
        let conn = open_in_memory().unwrap();
        let (_root, room, _case, shelf) = tree(&conn);
        assert!(move_to(&conn, shelf, None).is_err(), "полка не может висеть в воздухе");
        assert!(move_to(&conn, room, None).is_err());
    }

    #[test]
    fn move_into_own_descendant_is_rejected() {
        let conn = open_in_memory().unwrap();
        let (_root, room, case, _shelf) = tree(&conn);
        assert!(move_to(&conn, room, Some(case)).is_err());
        assert!(move_to(&conn, room, Some(room)).is_err());
        // дерево не тронуто
        assert_eq!(get(&conn, room).unwrap().parent_id, Some(_root));
    }

    #[test]
    fn empty_label_clears_it_and_blank_name_is_rejected() {
        let conn = open_in_memory().unwrap();
        let (_r, _room, _case, shelf) = tree(&conn);
        assert_eq!(get(&conn, shelf).unwrap().label.as_deref(), Some("A-3"));
        assert_eq!(update(&conn, shelf, None, Some("  ")).unwrap().label, None);
        assert_eq!(update(&conn, shelf, None, Some("B-1")).unwrap().label.as_deref(), Some("B-1"));
        assert!(update(&conn, shelf, Some("   "), None).is_err());
    }
}
