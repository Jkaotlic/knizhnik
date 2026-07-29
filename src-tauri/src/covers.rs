use crate::error::AppError;
use std::path::{Path, PathBuf};

// Приложение называет себя офлайновым, но обложки тянулись из сети при каждом
// рендере: без интернета полка становилась серой, а если источник убирал
// картинку — она пропадала навсегда. Качаем один раз и держим у себя.

const MAX_BYTES: u64 = 4 * 1024 * 1024;

pub fn dir(app_data: &Path) -> PathBuf {
    app_data.join("covers")
}

/// Имя файла выводим из id книги, а не из URL: URL меняется, id — нет.
fn file_name(book_id: i64, content_type: Option<&str>) -> String {
    let ext = match content_type {
        Some(t) if t.contains("png") => "png",
        Some(t) if t.contains("webp") => "webp",
        Some(t) if t.contains("gif") => "gif",
        _ => "jpg",
    };
    format!("{book_id}.{ext}")
}

/// Скачивает обложку и возвращает имя файла внутри каталога обложек.
/// Сетевой сбой — не ошибка сценария: книга просто останется без картинки.
pub async fn fetch(
    client: &reqwest::Client,
    app_data: &Path,
    book_id: i64,
    url: &str,
) -> Result<String, AppError> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::Rule("Обложку можно скачать только по http(s)".into()));
    }
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!("обложка: сервер ответил {}", resp.status())));
    }
    // Отсекаем неожиданно огромный ответ до чтения тела в память.
    if let Some(len) = resp.content_length() {
        if len > MAX_BYTES {
            return Err(AppError::Rule("Обложка слишком большая".into()));
        }
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = resp.bytes().await.map_err(|e| AppError::Network(e.to_string()))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(AppError::Rule("Обложка слишком большая".into()));
    }
    if !looks_like_image(&bytes) {
        return Err(AppError::Rule("По ссылке не картинка".into()));
    }

    let covers = dir(app_data);
    std::fs::create_dir_all(&covers)
        .map_err(|e| AppError::Rule(format!("Не создать папку обложек: {e}")))?;
    let name = file_name(book_id, content_type.as_deref());
    std::fs::write(covers.join(&name), &bytes)
        .map_err(|e| AppError::Rule(format!("Не сохранить обложку: {e}")))?;
    Ok(name)
}

pub fn remove(app_data: &Path, name: &str) {
    // Имя строим сами, но на всякий случай не даём вылезти из каталога.
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return;
    }
    let _ = std::fs::remove_file(dir(app_data).join(name));
}

/// Доверять Content-Type нельзя — проверяем сигнатуру файла.
fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])                    // jpeg
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")            // png
        || bytes.starts_with(b"GIF8")                          // gif
        || (bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_follows_content_type() {
        assert_eq!(file_name(7, Some("image/png")), "7.png");
        assert_eq!(file_name(7, Some("image/webp")), "7.webp");
        assert_eq!(file_name(7, Some("image/jpeg")), "7.jpg");
        assert_eq!(file_name(7, None), "7.jpg"); // разумный запасной вариант
    }

    #[test]
    fn recognises_real_image_signatures() {
        assert!(looks_like_image(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(looks_like_image(b"\x89PNG\r\n\x1a\n____"));
        assert!(looks_like_image(b"GIF89a___"));
        assert!(looks_like_image(b"RIFF____WEBPVP8 "));
    }

    #[test]
    fn rejects_html_error_pages_pretending_to_be_covers() {
        // источники любят отдавать 200 с заглушкой вместо 404
        assert!(!looks_like_image(b"<!DOCTYPE html><html>Not found"));
        assert!(!looks_like_image(b""));
        assert!(!looks_like_image(b"RIFF____AVI "));
    }

    #[test]
    fn remove_refuses_to_escape_the_covers_folder() {
        let tmp = std::env::temp_dir().join("knizhnik-covers-test");
        std::fs::create_dir_all(&tmp).unwrap();
        let victim = tmp.join("не-трогать.txt");
        std::fs::write(&victim, "важное").unwrap();

        remove(&tmp, "../не-трогать.txt");
        remove(&tmp, "covers/../../не-трогать.txt");

        assert!(victim.exists(), "обход каталога сработал");
        std::fs::remove_file(&victim).ok();
    }
}
