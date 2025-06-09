//! Optimized delimiter finding using memchr

/// Find the next BibTeX delimiter (@, {, }, =, ,) using memchr
/// Uses two passes but returns the earliest delimiter found
#[must_use]
pub fn find_delimiter(haystack: &[u8], start: usize) -> Option<(usize, u8)> {
    if start >= haystack.len() {
        return None;
    }

    let search_bytes = &haystack[start..];

    // First pass: most common delimiters {, }, , (based on profiling)
    let result1 =
        memchr::memchr3(b'{', b'}', b',', search_bytes).map(|pos| (start + pos, search_bytes[pos]));

    // Second pass: less common delimiters @, =
    let result2 =
        memchr::memchr2(b'@', b'=', search_bytes).map(|pos| (start + pos, search_bytes[pos]));

    // Return whichever delimiter appears first
    match (result1, result2) {
        (Some((pos1, byte1)), Some((pos2, byte2))) => {
            if pos1 <= pos2 {
                Some((pos1, byte1))
            } else {
                Some((pos2, byte2))
            }
        }
        (Some(r), None) | (None, Some(r)) => Some(r),
        (None, None) => None,
    }
}

/// Find brace or backslash for balanced brace parsing
#[must_use]
pub fn find_brace_delimiter(haystack: &[u8], start: usize) -> Option<(usize, u8)> {
    if start >= haystack.len() {
        return None;
    }

    memchr::memchr3(b'{', b'}', b'\\', &haystack[start..])
        .map(|pos| (start + pos, haystack[start + pos]))
}

/// Find delimiters in quoted strings (\, ", {, })
#[must_use]
pub fn find_quote_delimiter(haystack: &[u8], start: usize) -> Option<(usize, u8)> {
    if start >= haystack.len() {
        return None;
    }

    let search_bytes = &haystack[start..];

    // Search for \, ", { (most common in quoted strings)
    let result1 = memchr::memchr3(b'\\', b'"', b'{', search_bytes)
        .map(|pos| (start + pos, search_bytes[pos]));

    // Also need to check for } when inside braces
    let result2 = memchr::memchr(b'}', search_bytes).map(|pos| (start + pos, b'}'));

    // Return whichever delimiter appears first
    match (result1, result2) {
        (Some((pos1, byte1)), Some((pos2, _))) => {
            if pos1 <= pos2 {
                Some((pos1, byte1))
            } else {
                Some((pos2, b'}'))
            }
        }
        (Some(r), None) | (None, Some(r)) => Some(r),
        (None, None) => None,
    }
}

/// Find a single specific delimiter
#[must_use]
pub fn find_byte(haystack: &[u8], needle: u8, start: usize) -> Option<usize> {
    if start >= haystack.len() {
        return None;
    }

    memchr::memchr(needle, &haystack[start..]).map(|pos| start + pos)
}

/// Find any of 2 delimiters
#[must_use]
pub fn find_bytes2(haystack: &[u8], needle1: u8, needle2: u8, start: usize) -> Option<(usize, u8)> {
    if start >= haystack.len() {
        return None;
    }

    memchr::memchr2(needle1, needle2, &haystack[start..])
        .map(|pos| (start + pos, haystack[start + pos]))
}

/// Find any of 3 delimiters
#[must_use]
pub fn find_bytes3(
    haystack: &[u8],
    needle1: u8,
    needle2: u8,
    needle3: u8,
    start: usize,
) -> Option<(usize, u8)> {
    if start >= haystack.len() {
        return None;
    }

    memchr::memchr3(needle1, needle2, needle3, &haystack[start..])
        .map(|pos| (start + pos, haystack[start + pos]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_delimiter() {
        let input = b"hello @ world { test } = value, end";

        assert_eq!(find_delimiter(input, 0), Some((6, b'@')));
        assert_eq!(find_delimiter(input, 7), Some((14, b'{')));
        assert_eq!(find_delimiter(input, 15), Some((21, b'}')));
        assert_eq!(find_delimiter(input, 22), Some((23, b'=')));
        assert_eq!(find_delimiter(input, 24), Some((30, b',')));
        assert_eq!(find_delimiter(input, 31), None);
    }

    #[test]
    fn test_specialized_searches() {
        let input = b"test {nested} string";

        assert_eq!(find_brace_delimiter(input, 0), Some((5, b'{')));
        assert_eq!(find_brace_delimiter(input, 6), Some((12, b'}')));
        assert_eq!(find_byte(input, b'}', 0), Some(12));
        assert_eq!(find_byte(input, b'}', 13), None);
    }
}
