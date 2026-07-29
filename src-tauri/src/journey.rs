//! Сквозной тест пути пользователя. Юнит-тесты проверяют куски по отдельности,
//! а здесь — что они стыкуются: от пустой базы до экспорта и восстановления.

use crate::db::models::BookInput;
use crate::db::{backup, books, cache, locations, open_in_memory};
use crate::domain::matching::MetadataCandidate;
use crate::export;

fn book(title: &str, isbn: Option<&str>, shelf: Option<i64>) -> BookInput {
    BookInput {
        title: title.into(),
        authors: Some("Дмитрий Глуховский".into()),
        isbn: isbn.map(str::to_string),
        year: Some(2019),
        pages: Some(480),
        shelf_id: shelf,
        ..Default::default()
    }
}

#[test]
fn from_empty_catalogue_to_backup_and_back() {
    let mut conn = open_in_memory().unwrap();

    // 1. Полка заводится одним действием — служебные уровни поднимаются сами.
    let case = locations::create_bookcase(&conn, "Шкаф у окна").unwrap();
    let shelf = locations::create_shelf(&conn, case.id, "Верхняя", Some("В-1")).unwrap();
    assert_eq!(locations::all(&conn).unwrap().len(), 4, "дом и комната должны появиться");
    assert_eq!(
        locations::breadcrumb(&conn, shelf.id).unwrap(),
        "Шкаф у окна › Верхняя",
        "единственная комната в пути не показывается"
    );

    // 2. Книга на полке и книга без полки живут раздельно.
    books::insert(&conn, &book("Будущее", Some("978-5-17-118366-0"), Some(shelf.id))).unwrap();
    let lost = books::insert(&conn, &book("Метро 2033", None, None)).unwrap();
    assert_eq!(books::on_shelf(&conn, shelf.id).unwrap().len(), 1);
    assert_eq!(books::without_shelf(&conn).unwrap().len(), 1);

    // 3. Забытую книгу раскладываем — и она уходит из списка потерянных.
    books::set_shelf(&conn, lost.id, Some(shelf.id)).unwrap();
    assert!(books::without_shelf(&conn).unwrap().is_empty());
    assert_eq!(books::on_shelf(&conn, shelf.id).unwrap().len(), 2);

    // 4. Поиск: ISBN нормализован при вставке, поэтому находится и с дефисами.
    assert_eq!(books::search(&conn, "978-5-17-118366-0").unwrap().len(), 1);
    assert_eq!(books::search(&conn, "глуховск").unwrap().len(), 2);
    let hit = &books::search(&conn, "будущее").unwrap()[0];
    assert_eq!(hit.breadcrumb, "Шкаф у окна › Верхняя");

    // 5. Дубль виден по всему каталогу, а не только по текущей полке.
    let other_case = locations::create_bookcase(&conn, "Шкаф в коридоре").unwrap();
    let other = locations::create_shelf(&conn, other_case.id, "Нижняя", None).unwrap();
    books::insert(&conn, &book("Будущее", Some("9785171183660"), Some(other.id))).unwrap();
    assert_eq!(books::find_isbn_duplicates(&conn, "9785171183660").unwrap().len(), 2);

    // 6. Выдача со сроком попадает в статистику как просрочка.
    books::set_availability(&conn, lost.id, "lent", Some("Маша"), Some("2000-01-01")).unwrap();
    let st = books::stats(&conn).unwrap();
    assert_eq!(st.total, 3);
    assert_eq!(st.lent_out, 1);
    assert_eq!(st.overdue, 1, "срок в прошлом — книга просрочена");

    // 7. Экспорт CSV: BOM для Excel и человекочитаемый путь к полке.
    let csv = export::export_csv(&conn).unwrap();
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.lines().any(|l| l.ends_with("Шкаф у окна › Верхняя")));

    // 8. Кэш метаданных переживает круг.
    cache::put(
        &conn,
        "9785171183660",
        &[MetadataCandidate {
            title: "Будущее".into(),
            authors: None, isbn: None, year: None, publisher: None,
            pages: None, language: None, cover_url: None, source: "test".into(),
        }],
    )
    .unwrap();
    assert_eq!(cache::get(&conn, "9785171183660").unwrap().unwrap()[0].title, "Будущее");

    // 9. Резервная копия восстанавливает каталог до последней связи.
    let copy = backup::export(&conn).unwrap();
    let json = serde_json::to_string(&copy).unwrap();
    let parsed: backup::Backup = serde_json::from_str(&json).unwrap();

    let mut fresh = open_in_memory().unwrap();
    let summary = backup::import(&mut fresh, &parsed).unwrap();
    assert_eq!(summary.books, 3);
    assert_eq!(books::on_shelf(&fresh, shelf.id).unwrap().len(), 2);
    assert_eq!(locations::breadcrumb(&fresh, shelf.id).unwrap(), "Шкаф у окна › Верхняя");
    let restored = books::get(&fresh, lost.id).unwrap();
    assert_eq!(restored.lent_to.as_deref(), Some("Маша"), "выдача должна пережить копию");
    assert_eq!(restored.due_at.as_deref(), Some("2000-01-01"));

    // 10. Импорт CSV дополняет каталог и не плодит дубли по ISBN.
    let csv_in = "Title,Author,ISBN13,My Rating,Exclusive Shelf\n\
                  Дюна,Фрэнк Герберт,=\"9780441013593\",5,read\n\
                  Будущее,Глуховский,=\"9785171183660\",4,read\n";
    let parsed_csv = crate::import::parse(csv_in).unwrap();
    let report = crate::import::apply(&conn, &parsed_csv, Some(shelf.id)).unwrap();
    assert_eq!(report.added, 1, "Дюна новая");
    assert_eq!(report.skipped_duplicates, 1, "Будущее уже есть по ISBN");

    // 11. Удалить полку с книгами нельзя, пустую — можно.
    assert!(locations::delete(&conn, shelf.id).is_err());
    assert!(locations::delete(&conn, other_case.id).is_err(), "внутри шкафа книга");
}

#[test]
fn breadcrumb_starts_showing_rooms_once_a_second_one_appears() {
    let conn = open_in_memory().unwrap();
    let case = locations::create_bookcase(&conn, "Шкаф").unwrap();
    let shelf = locations::create_shelf(&conn, case.id, "Полка", None).unwrap();
    assert_eq!(locations::breadcrumb(&conn, shelf.id).unwrap(), "Шкаф › Полка");

    // появилась вторая комната — путь стал однозначным сам собой
    let root = locations::all(&conn).unwrap().iter().find(|l| l.kind == "root").unwrap().id;
    locations::create(&conn, Some(root), "Спальня", "room", None).unwrap();
    assert_eq!(locations::breadcrumb(&conn, shelf.id).unwrap(), "Комната › Шкаф › Полка");
}
