pub mod isbn;
pub mod breadcrumb;
pub mod matching;

#[cfg(test)]
mod smoke {
    #[test]
    fn scaffold_builds() {
        assert_eq!(2 + 2, 4);
    }
}
