//! Lexical analysis for BibTeX

use super::{delimiter, PResult};
use memchr;
use winnow::prelude::*;
use winnow::{
    ascii::digit1,
    combinator::{alt, opt},
};

/// Parse an identifier (letters, numbers, underscores, hyphens, colons)
#[inline]
pub fn identifier<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    let bytes = input.as_bytes();
    let len = super::simd::scan_identifier(bytes);

    if len == 0 {
        return super::backtrack();
    }

    let result = &input[..len];
    *input = &input[len..];
    Ok(result)
}

/// Parse a field name (same as identifier but typically lowercase)
#[inline]
pub fn field_name<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    identifier(input)
}

/// Parse balanced braces { ... } with SIMD acceleration
#[inline]
pub fn balanced_braces<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    let original_input = *input;
    let bytes = input.as_bytes();
    let mut depth = 0;
    let mut pos = 0;

    // Use SIMD to find delimiters
    while pos < bytes.len() {
        // Find next delimiter using SIMD
        if let Some(offset) = memchr::memchr3(b'{', b'}', b'\\', &bytes[pos..]) {
            let idx = pos + offset;

            // Include content up to delimiter
            match bytes[idx] {
                b'{' => {
                    depth += 1;
                    pos = idx + 1;
                }
                b'}' => {
                    if depth == 0 {
                        let result = &original_input[..idx];
                        *input = &input[idx..];
                        return Ok(result);
                    }
                    depth -= 1;
                    pos = idx + 1;
                }
                b'\\' => {
                    // Skip escaped character
                    pos = idx + 2;
                }
                _ => unreachable!(),
            }
        } else {
            // No more delimiters found
            break;
        }
    }

    super::backtrack()
}

/// Parse a quoted string "..." with SIMD acceleration
#[inline]
pub fn quoted_string<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    let bytes = input.as_bytes();

    // Use SIMD-accelerated quote scanning
    super::simd::find_balanced_quotes(bytes).map_or_else(super::backtrack, |end_pos| {
        // Extract the content (without the quotes)
        let result = &input[1..end_pos - 1];
        *input = &input[end_pos..];
        Ok(result)
    })
}

/// Parse a number (integer)
#[inline]
pub fn number<'a>(input: &mut &'a str) -> PResult<'a, i64> {
    let sign = opt(alt(('+', '-'))).parse_next(input)?;
    let digits = digit1.parse_next(input)?;

    let mut num = digits.parse::<i64>().map_err(|_| super::backtrack_err())?;

    if sign == Some('-') {
        num = -num;
    }

    Ok(num)
}

/// Parse balanced parentheses ( ... ) with SIMD acceleration
#[inline]
pub fn balanced_parentheses<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    let original_input = *input;
    let bytes = input.as_bytes();
    let mut depth = 0;
    let mut pos = 0;

    // Use SIMD to find delimiters
    while pos < bytes.len() {
        // Find next delimiter using SIMD
        if let Some(offset) = memchr::memchr2(b'(', b')', &bytes[pos..]) {
            let idx = pos + offset;

            match bytes[idx] {
                b'(' => {
                    depth += 1;
                    pos = idx + 1;
                }
                b')' => {
                    if depth == 0 {
                        let result = &original_input[..idx];
                        *input = &input[idx..];
                        return Ok(result);
                    }
                    depth -= 1;
                    pos = idx + 1;
                }
                _ => unreachable!(),
            }
        } else {
            // No more delimiters found
            break;
        }
    }

    super::backtrack()
}

/// Fast whitespace skipping (optimal for short runs per profiling)
#[inline]
pub fn skip_whitespace(input: &mut &str) {
    let bytes = input.as_bytes();
    let mut pos = 0;

    while let Some(&byte) = bytes.get(pos) {
        match byte {
            b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
            _ => break,
        }
    }

    *input = &input[pos..];
}

#[inline]
pub(crate) fn skip_whitespace_peek(input: &mut &str) -> Option<u8> {
    let bytes = input.as_bytes();
    let mut pos = 0;

    while let Some(&byte) = bytes.get(pos) {
        match byte {
            b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
            _ => {
                *input = &input[pos..];
                return Some(byte);
            }
        }
    }

    *input = "";
    None
}

/// Fast scan to next BibTeX delimiter - re-export from delimiter module
#[must_use]
pub fn scan_to_bibtex_delimiter(haystack: &[u8], start: usize) -> Option<(usize, u8)> {
    delimiter::find_delimiter(haystack, start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifier() {
        let mut input = "hello-world_123:test.com xxx";
        let result = identifier(&mut input).unwrap();
        assert_eq!(result, "hello-world_123:test.com");
        assert_eq!(input, " xxx");
    }

    #[test]
    fn test_balanced_braces() {
        let mut input = "hello {nested {braces}} world} xxx";
        let result = balanced_braces(&mut input).unwrap();
        assert_eq!(result, "hello {nested {braces}} world");
        assert_eq!(input, "} xxx");
    }

    #[test]
    fn test_balanced_braces_with_spaces() {
        let mut input = "Second preamble} xxx";
        let result = balanced_braces(&mut input).unwrap();
        assert_eq!(result, "Second preamble");
        assert_eq!(input, "} xxx");
    }

    #[test]
    fn test_balanced_parentheses() {
        let mut input = "hello (nested (parens)) world) xxx";
        let result = balanced_parentheses(&mut input).unwrap();
        assert_eq!(result, "hello (nested (parens)) world");
        assert_eq!(input, ") xxx");
    }

    #[test]
    fn test_quoted_string() {
        let mut input = r#""hello \"world\"" xxx"#;
        let result = quoted_string(&mut input).unwrap();
        assert_eq!(result, r#"hello \"world\""#);
        assert_eq!(input, " xxx");

        // Test with nested braces
        let mut input = r#""hello {world}" xxx"#;
        let result = quoted_string(&mut input).unwrap();
        assert_eq!(result, "hello {world}");
    }

    #[test]
    fn test_number() {
        let mut input = "42 xxx";
        assert_eq!(number(&mut input).unwrap(), 42);

        let mut input = "-42 xxx";
        assert_eq!(number(&mut input).unwrap(), -42);

        let mut input = "+42 xxx";
        assert_eq!(number(&mut input).unwrap(), 42);
    }

    #[test]
    fn test_scan_to_bibtex_delimiter() {
        let input = b"hello @ world { test } = value, end";

        assert_eq!(scan_to_bibtex_delimiter(input, 0), Some((6, b'@')));
        assert_eq!(scan_to_bibtex_delimiter(input, 7), Some((14, b'{')));
        assert_eq!(scan_to_bibtex_delimiter(input, 15), Some((21, b'}')));
        assert_eq!(scan_to_bibtex_delimiter(input, 22), Some((23, b'=')));
        assert_eq!(scan_to_bibtex_delimiter(input, 24), Some((30, b',')));
        assert_eq!(scan_to_bibtex_delimiter(input, 31), None);
    }
}
