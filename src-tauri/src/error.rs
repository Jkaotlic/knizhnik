use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Ошибка базы данных: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Ошибка ISBN: {0}")]
    Isbn(String),
    #[error("Ошибка сети: {0}")]
    Network(String),
    #[error("{0}")]
    Rule(String),
}

// Во фронт ошибки уходят строкой.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
