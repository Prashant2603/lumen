/// Log severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Detect the log level from a line using fast byte-prefix matching.
    /// Checks common log format patterns like `[INFO]`, `INFO`, `WARN`, etc.
    pub fn detect(line: &str) -> Option<LogLevel> {
        let bytes = line.as_bytes();
        // Skip leading whitespace and timestamp-like prefixes
        // Look for level keywords anywhere in the first 80 bytes
        let search_region = if bytes.len() > 80 { &bytes[..80] } else { bytes };

        // Check for common patterns: LEVEL, [LEVEL], level
        if contains_word(search_region, b"ERROR") || contains_word(search_region, b"error") {
            Some(LogLevel::Error)
        } else if contains_word(search_region, b"WARN") || contains_word(search_region, b"warn") {
            Some(LogLevel::Warn)
        } else if contains_word(search_region, b"INFO") || contains_word(search_region, b"info") {
            Some(LogLevel::Info)
        } else if contains_word(search_region, b"DEBUG") || contains_word(search_region, b"debug") {
            Some(LogLevel::Debug)
        } else if contains_word(search_region, b"TRACE") || contains_word(search_region, b"trace") {
            Some(LogLevel::Trace)
        } else {
            None
        }
    }
}

/// Check if a byte slice contains a word (surrounded by non-alpha boundaries).
fn contains_word(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    for i in 0..=(haystack.len() - needle.len()) {
        if &haystack[i..i + needle.len()] == needle {
            // Check boundaries
            let before_ok = i == 0 || !haystack[i - 1].is_ascii_alphabetic();
            let after = i + needle.len();
            let after_ok = after >= haystack.len() || !haystack[after].is_ascii_alphabetic();
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_levels() {
        assert_eq!(
            LogLevel::detect("[2025-01-01] INFO  Starting server"),
            Some(LogLevel::Info)
        );
        assert_eq!(
            LogLevel::detect("[2025-01-01] ERROR Failed to connect"),
            Some(LogLevel::Error)
        );
        assert_eq!(
            LogLevel::detect("WARN: disk space low"),
            Some(LogLevel::Warn)
        );
        assert_eq!(
            LogLevel::detect("DEBUG some internal state"),
            Some(LogLevel::Debug)
        );
        assert_eq!(
            LogLevel::detect("TRACE entering function"),
            Some(LogLevel::Trace)
        );
        assert_eq!(LogLevel::detect("no level here"), None);
    }
}
