//! Lexical analysis for BibTeX

use super::PResult;
use winnow::prelude::*;
use winnow::{
    ascii::digit1,
    combinator::{alt, opt},
    token::take_while,
};

/// Parse an identifier (letters, numbers, underscores, hyphens, colons)
pub fn identifier<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    take_while(1.., |c: char| {
        c.is_alphanumeric() || c == '_' || c == '-' || c == ':' || c == '.'
    })
    .parse_next(input)
}

/// Parse a field name (same as identifier but typically lowercase)
pub fn field_name<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    identifier.parse_next(input)
}

/// Parse balanced braces { ... }
pub fn balanced_braces<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    let original_input = *input;
    let mut depth = 0;
    let mut pos = 0;
    let bytes = input.as_bytes();

    while pos < bytes.len() {
        match bytes[pos] {
            b'{' => depth += 1,
            b'}' => {
                if depth == 0 {
                    let result = &original_input[..pos];
                    *input = &input[pos..];
                    return Ok(result);
                }
                depth -= 1;
            }
            b'\\' if pos + 1 < bytes.len() => {
                // Skip escaped character
                pos += 2;
                continue;
            }
            _ => {}
        }
        pos += 1;
    }

    // Use memchr for fast scanning of non-brace characters
    while pos < bytes.len() {
        if let Some(next_special) = memchr::memchr3(b'{', b'}', b'\\', &bytes[pos..]) {
            pos += next_special;
            match bytes[pos] {
                b'{' => depth += 1,
                b'}' => {
                    if depth == 0 {
                        let result = &original_input[..pos];
                        *input = &input[pos..];
                        return Ok(result);
                    }
                    depth -= 1;
                }
                b'\\' if pos + 1 < bytes.len() => {
                    pos += 2;
                    continue;
                }
                _ => {}
            }
            pos += 1;
        } else {
            break;
        }
    }

    Err(winnow::error::ErrMode::Backtrack(
        winnow::error::ContextError::default(),
    ))
}

/// Parse a quoted string "..."
pub fn quoted_string<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    let start = *input;
    let bytes = input.as_bytes();

    if bytes.is_empty() || bytes[0] != b'"' {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    let mut pos = 1; // Skip opening quote
    let mut brace_depth = 0;

    while pos < bytes.len() {
        match bytes[pos] {
            b'\\' if pos + 1 < bytes.len() => {
                // Skip escaped character
                pos += 2;
            }
            b'"' if brace_depth == 0 => {
                // Found closing quote
                let result = &start[1..pos];
                *input = &start[pos + 1..];
                return Ok(result);
            }
            b'{' => {
                brace_depth += 1;
                pos += 1;
            }
            b'}' if brace_depth > 0 => {
                brace_depth -= 1;
                pos += 1;
            }
            _ => pos += 1,
        }
    }

    Err(winnow::error::ErrMode::Backtrack(
        winnow::error::ContextError::default(),
    ))
}

/// Parse a number (integer)
pub fn number<'a>(input: &mut &'a str) -> PResult<'a, i64> {
    let sign = opt(alt(('+', '-'))).parse_next(input)?;
    let digits = digit1.parse_next(input)?;

    let mut num = digits
        .parse::<i64>()
        .map_err(|_| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::default()))?;

    if sign == Some('-') {
        num = -num;
    }

    Ok(num)
}

/// Fast whitespace skipping using memchr
pub fn skip_whitespace(input: &mut &str) {
    let bytes = input.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        match bytes[pos] {
            b' ' | b'\t' | b'\n' | b'\r' => pos += 1,
            _ => break,
        }
    }

    *input = &input[pos..];
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
}
