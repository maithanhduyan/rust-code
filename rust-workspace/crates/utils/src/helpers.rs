//! Helper functions

use std::time::{SystemTime, UNIX_EPOCH};

/// Lấy timestamp hiện tại (Unix epoch milliseconds)
pub fn current_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
}

/// Lấy timestamp hiện tại (Unix epoch seconds)
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Slugify một string (chuyển thành URL-friendly)
pub fn slugify(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Truncate string với suffix
pub fn truncate(s: &str, max_len: usize, suffix: &str) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncate_at = max_len.saturating_sub(suffix.len());
        format!("{}{}", &s[..truncate_at], suffix)
    }
}

/// Generate random ID đơn giản
pub fn generate_id() -> String {
    format!("{:x}", current_timestamp_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("Rust is Awesome!"), "rust-is-awesome-");
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Hello", 10, "..."), "Hello");
        assert_eq!(truncate("Hello World", 8, "..."), "Hello...");
    }

    #[test]
    fn test_timestamp() {
        let ts = current_timestamp_secs();
        assert!(ts > 0);
    }
}
