pub fn format_breadcrumb(segments: &[(String, String)]) -> String {
    segments
        .iter()
        .filter(|(_, kind)| kind != "root")
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

    #[test]
    fn builds_path_leaf_to_root_excluding_root() {
        let segments = vec![
            seg("Полка", "shelf"),
            seg("Шкаф", "bookcase"),
            seg("Комната", "room"),
            seg("Дом", "root"),
        ];
        assert_eq!(format_breadcrumb(&segments), "Комната › Шкаф › Полка");
    }

    #[test]
    fn single_shelf_under_root() {
        let segments = vec![seg("Полка A-3", "shelf"), seg("Дом", "root")];
        assert_eq!(format_breadcrumb(&segments), "Полка A-3");
    }

    #[test]
    fn empty_input_gives_empty_string() {
        assert_eq!(format_breadcrumb(&[]), "");
    }
}
