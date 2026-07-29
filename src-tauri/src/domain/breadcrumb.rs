/// Корень («Дом») не показываем никогда — он служебный.
/// Комнату показываем, только если их несколько: при единственной комнате
/// «Библиотека › Шкаф1 › А1» — это шум, а «Шкаф1 › А1» говорит ровно то же.
pub fn format_breadcrumb(segments: &[(String, String)], show_rooms: bool) -> String {
    segments
        .iter()
        .filter(|(_, kind)| kind != "root" && (show_rooms || kind != "room"))
        .rev()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(" › ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(name: &str, kind: &str) -> (String, String) {
        (name.to_string(), kind.to_string())
    }

    fn full() -> Vec<(String, String)> {
        vec![
            seg("Полка", "shelf"),
            seg("Шкаф", "bookcase"),
            seg("Комната", "room"),
            seg("Дом", "root"),
        ]
    }

    #[test]
    fn single_room_is_omitted_as_noise() {
        assert_eq!(format_breadcrumb(&full(), false), "Шкаф › Полка");
    }

    #[test]
    fn room_is_shown_when_there_are_several() {
        assert_eq!(format_breadcrumb(&full(), true), "Комната › Шкаф › Полка");
    }

    #[test]
    fn root_is_never_shown() {
        for show_rooms in [false, true] {
            assert!(!format_breadcrumb(&full(), show_rooms).contains("Дом"));
        }
    }

    #[test]
    fn single_shelf_under_root() {
        let segments = vec![seg("Полка A-3", "shelf"), seg("Дом", "root")];
        assert_eq!(format_breadcrumb(&segments, false), "Полка A-3");
    }

    #[test]
    fn empty_input_gives_empty_string() {
        assert_eq!(format_breadcrumb(&[], false), "");
    }
}
