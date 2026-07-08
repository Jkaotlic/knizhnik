use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum IsbnError {
    Empty,
    BadLength,
    BadChecksum,
}

impl fmt::Display for IsbnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            IsbnError::Empty => "ISBN пуст",
            IsbnError::BadLength => "Неверная длина ISBN",
            IsbnError::BadChecksum => "Неверная контрольная сумма ISBN",
        };
        f.write_str(msg)
    }
}

pub fn normalize(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

pub fn is_valid_isbn10(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let mut sum = 0i32;
    for (i, c) in s.chars().enumerate() {
        let v = if c == 'X' && i == 9 {
            10
        } else if let Some(d) = c.to_digit(10) {
            d as i32
        } else {
            return false;
        };
        sum += (10 - i as i32) * v;
    }
    sum % 11 == 0
}

pub fn is_valid_isbn13(s: &str) -> bool {
    if s.len() != 13 || !s.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let mut sum = 0i32;
    for (i, c) in s.chars().enumerate() {
        let d = c.to_digit(10).unwrap() as i32;
        sum += if i % 2 == 0 { d } else { 3 * d };
    }
    sum % 10 == 0
}

fn isbn10_to_isbn13(s: &str) -> String {
    let core: String = format!("978{}", &s[..9]);
    let mut sum = 0i32;
    for (i, c) in core.chars().enumerate() {
        let d = c.to_digit(10).unwrap() as i32;
        sum += if i % 2 == 0 { d } else { 3 * d };
    }
    let check = (10 - (sum % 10)) % 10;
    format!("{}{}", core, check)
}

pub fn normalize_and_validate(raw: &str) -> Result<String, IsbnError> {
    let n = normalize(raw);
    if n.is_empty() {
        return Err(IsbnError::Empty);
    }
    match n.len() {
        10 => {
            if is_valid_isbn10(&n) {
                Ok(isbn10_to_isbn13(&n))
            } else {
                Err(IsbnError::BadChecksum)
            }
        }
        13 => {
            if is_valid_isbn13(&n) {
                Ok(n)
            } else {
                Err(IsbnError::BadChecksum)
            }
        }
        _ => Err(IsbnError::BadLength),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_hyphens_and_spaces() {
        assert_eq!(normalize("978-5-17-118366-3"), "9785171183663");
        assert_eq!(normalize(" 0-306-40615-2 "), "0306406152");
        assert_eq!(normalize("080442957x"), "080442957X");
    }

    #[test]
    fn validates_isbn13_checksum() {
        assert!(is_valid_isbn13("9785171183660"));
        assert!(!is_valid_isbn13("9785171183664"));
        assert!(!is_valid_isbn13("978517118366")); // 12 цифр
    }

    #[test]
    fn validates_isbn10_checksum() {
        assert!(is_valid_isbn10("0306406152"));
        assert!(is_valid_isbn10("080442957X"));
        assert!(!is_valid_isbn10("0306406153"));
    }

    #[test]
    fn converts_valid_input_to_isbn13() {
        assert_eq!(normalize_and_validate("978-5-17-118366-0").unwrap(), "9785171183660");
        // валидный ISBN-10 → ISBN-13 с префиксом 978
        assert_eq!(normalize_and_validate("0-306-40615-2").unwrap(), "9780306406157");
    }

    #[test]
    fn rejects_bad_checksum_without_panic() {
        assert_eq!(normalize_and_validate("9785171183664"), Err(IsbnError::BadChecksum));
        assert_eq!(normalize_and_validate(""), Err(IsbnError::Empty));
        assert_eq!(normalize_and_validate("12345"), Err(IsbnError::BadLength));
    }
}
