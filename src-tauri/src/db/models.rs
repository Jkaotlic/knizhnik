use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub kind: String,        // root | room | bookcase | shelf
    pub label: Option<String>,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Book {
    pub id: i64,
    pub title: String,
    pub authors: Option<String>,
    pub isbn: Option<String>,
    pub year: Option<i64>,
    pub publisher: Option<String>,
    pub pages: Option<i64>,
    pub language: Option<String>,
    pub genre: Option<String>,
    pub annotation: Option<String>,
    pub cover_url: Option<String>,
    pub shelf_id: Option<i64>,
    pub status: Option<String>,      // want | reading | read | NULL
    pub rating: Option<i64>,
    pub notes: Option<String>,
    pub availability: String,        // on_shelf | lent | away
    pub lent_to: Option<String>,
    pub added_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookInput {
    pub title: String,
    pub authors: Option<String>,
    pub isbn: Option<String>,
    pub year: Option<i64>,
    pub publisher: Option<String>,
    pub pages: Option<i64>,
    pub language: Option<String>,
    pub genre: Option<String>,
    pub annotation: Option<String>,
    pub cover_url: Option<String>,
    pub shelf_id: Option<i64>,
    pub status: Option<String>,
    pub rating: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub total: i64,
    pub by_status: Vec<(String, i64)>, // ("read", 12), ("не указан", 5)...
    pub pages_read: i64,
    pub top_genres: Vec<(String, i64)>,
}
