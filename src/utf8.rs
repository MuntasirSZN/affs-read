//! UTF-8 validation utilities.

/// Validate and convert bytes to UTF-8 string.
///
/// Uses simdutf8 for fast validation when available.
#[inline]
pub fn from_utf8(bytes: &[u8]) -> Option<&str> {
    #[cfg(not(miri))]
    {
        // Use simdutf8 for fast validation
        simdutf8::basic::from_utf8(bytes).ok()
    }

    #[cfg(miri)]
    {
        // Fall back to std validation under miri
        core::str::from_utf8(bytes).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_utf8() {
        assert_eq!(from_utf8(b"hello"), Some("hello"));
    }

    #[test]
    fn test_invalid_utf8() {
        // Invalid UTF-8 sequence
        assert_eq!(from_utf8(&[0xFF, 0xFE]), None);
    }

    #[test]
    fn test_utf8_multibyte() {
        assert_eq!(from_utf8("café".as_bytes()), Some("café"));
    }
}
