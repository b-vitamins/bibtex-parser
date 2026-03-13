//! Value parsing for BibTeX fields

use super::{lexer, Cursor, PResult};
use crate::model::Value;
use std::borrow::Cow;

/// Parse a BibTeX value (string, number, variable, or concatenation)
#[inline]
pub fn parse_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    parse_concatenated_value(input)
}

/// Parse a field value and consume trailing ASCII whitespace.
///
/// This variant is used by entry parsing so the field loop can read the
/// delimiter directly without re-scanning whitespace.
#[inline]
pub(crate) fn parse_value_field<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    parse_concatenated_value_field(input)
}

/// Parse a field value directly from the streaming parser cursor.
#[inline]
pub(crate) fn parse_value_field_cursor<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, Value<'a>> {
    let first = parse_single_value_cursor(cursor)?;
    cursor.skip_whitespace();

    if cursor.remaining_bytes().first() != Some(&b'#') {
        return Ok(first);
    }

    let mut parts = Vec::with_capacity(4);
    parts.push(first);

    loop {
        cursor.bump(1);
        cursor.skip_whitespace();

        let part = parse_single_value_cursor(cursor)?;
        parts.push(part);
        cursor.skip_whitespace();

        if cursor.remaining_bytes().first() != Some(&b'#') {
            break;
        }
    }

    Ok(Value::Concat(Box::new(parts)))
}

/// Parse a concatenated value (value # value # ...)
#[inline]
fn parse_concatenated_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let first = parse_single_value(input)?;

    // Fast path: most fields are a single value with no concatenation.
    if !consume_concat_separator(input) {
        return Ok(first);
    }

    // Slow path: parse one or more `# value` segments.
    let mut parts = Vec::with_capacity(4);
    parts.push(first);

    loop {
        let part = parse_single_value(input)?;
        parts.push(part);

        if !consume_concat_separator(input) {
            break;
        }
    }

    Ok(Value::Concat(Box::new(parts)))
}

/// Parse a concatenated value and consume trailing ASCII whitespace.
#[inline]
fn parse_concatenated_value_field<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let first = parse_single_value(input)?;
    lexer::skip_whitespace(input);

    if input.as_bytes().first() != Some(&b'#') {
        return Ok(first);
    }

    // Slow path: parse one or more `# value` segments.
    let mut parts = Vec::with_capacity(4);
    parts.push(first);

    loop {
        // Consume '#'
        *input = &input[1..];
        lexer::skip_whitespace(input);

        let part = parse_single_value(input)?;
        parts.push(part);
        lexer::skip_whitespace(input);

        if input.as_bytes().first() != Some(&b'#') {
            break;
        }
    }

    Ok(Value::Concat(Box::new(parts)))
}

/// Consume optional whitespace + `#` + optional whitespace.
///
/// Returns `true` if a concatenation separator was consumed. If no separator
/// is present, input is left untouched.
#[inline]
fn consume_concat_separator(input: &mut &str) -> bool {
    let mut probe = *input;
    lexer::skip_whitespace(&mut probe);
    if probe.as_bytes().first() != Some(&b'#') {
        return false;
    }

    probe = &probe[1..];
    lexer::skip_whitespace(&mut probe);
    *input = probe;
    true
}

/// Parse a single value component
#[inline]
fn parse_single_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    // Fast dispatch based on first character
    let bytes = input.as_bytes();
    if let Some(&first) = bytes.first() {
        match first {
            b'"' => parse_quoted_value(input),
            b'{' => parse_braced_value(input),
            b'0'..=b'9' | b'+' | b'-' => parse_number_or_digit_string(input),
            _ => parse_variable_value(input),
        }
    } else {
        Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ))
    }
}

#[inline]
fn parse_single_value_cursor<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, Value<'a>> {
    let bytes = cursor.remaining_bytes();
    if let Some(&first) = bytes.first() {
        match first {
            b'"' => parse_quoted_value_cursor(cursor),
            b'{' => parse_braced_value_cursor(cursor),
            b'0'..=b'9' | b'+' | b'-' => parse_number_or_digit_string_cursor(cursor),
            _ => parse_variable_value_cursor(cursor),
        }
    } else {
        Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ))
    }
}

/// Parse a quoted string value
#[inline]
fn parse_quoted_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    // Quick check using byte access
    if input.as_bytes().first() != Some(&b'"') {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    // Use the lexer to parse the quoted string (it handles the quotes)
    let s = lexer::quoted_string(input)?;
    Ok(Value::Literal(Cow::Borrowed(s)))
}

#[inline]
fn parse_quoted_value_cursor<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, Value<'a>> {
    let bytes = cursor.remaining_bytes();
    if bytes.first() != Some(&b'"') {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    super::simd::find_balanced_quotes(bytes).map_or_else(
        || {
            Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        },
        |end_pos| {
            let content = &cursor.remaining()[1..end_pos - 1];
            cursor.bump(end_pos);
            Ok(Value::Literal(Cow::Borrowed(content)))
        },
    )
}

/// Parse a braced string value
#[inline]
fn parse_braced_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    // Quick check using byte access
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'{') {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    // Use SIMD-accelerated balanced brace finding
    super::simd::find_balanced_braces(bytes).map_or_else(
        || {
            Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        },
        |end_pos| {
            // Extract content (skip opening and closing braces)
            let content = &input[1..end_pos - 1];
            *input = &input[end_pos..];
            Ok(Value::Literal(Cow::Borrowed(content)))
        },
    )
}

#[inline]
fn parse_braced_value_cursor<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, Value<'a>> {
    let bytes = cursor.remaining_bytes();
    if bytes.first() != Some(&b'{') {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    super::simd::find_balanced_braces(bytes).map_or_else(
        || {
            Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        },
        |end_pos| {
            let content = &cursor.remaining()[1..end_pos - 1];
            cursor.bump(end_pos);
            Ok(Value::Literal(Cow::Borrowed(content)))
        },
    )
}

/// Parse either a number or a string that starts with digits
/// This handles cases like "2024a", "12b", "1.2.3", etc.
#[inline]
fn parse_number_or_digit_string<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let bytes = input.as_bytes();
    let Some(&first) = bytes.first() else {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    };

    let len = super::simd::scan_identifier(bytes);
    if len == 0 {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    let token = &input[..len];
    let token_bytes = token.as_bytes();

    // Signed values must be strict integers (e.g., +42, -1).
    // Non-digit suffixes after a sign are rejected for compatibility.
    if first == b'+' || first == b'-' {
        if token_bytes.len() <= 1 || !token_bytes[1..].iter().all(u8::is_ascii_digit) {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ));
        }
        let num = parse_i64_ascii(token)?;
        *input = &input[len..];
        return Ok(Value::Number(num));
    }

    // Digit-starting tokens parse as numbers when fully numeric,
    // otherwise as literals (e.g. 2024a).
    if !first.is_ascii_digit() {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    *input = &input[len..];
    if token_bytes.iter().all(u8::is_ascii_digit) {
        let num = parse_i64_ascii(token)?;
        Ok(Value::Number(num))
    } else {
        Ok(Value::Literal(Cow::Borrowed(token)))
    }
}

#[inline]
fn parse_number_or_digit_string_cursor<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, Value<'a>> {
    let bytes = cursor.remaining_bytes();
    let Some(&first) = bytes.first() else {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    };

    let len = super::simd::scan_identifier(bytes);
    if len == 0 {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    let token = &cursor.remaining()[..len];
    let token_bytes = token.as_bytes();

    if first == b'+' || first == b'-' {
        if token_bytes.len() <= 1 || !token_bytes[1..].iter().all(u8::is_ascii_digit) {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ));
        }
        let number = parse_i64_ascii(token)?;
        cursor.bump(len);
        return Ok(Value::Number(number));
    }

    if !first.is_ascii_digit() {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    cursor.bump(len);
    if token_bytes.iter().all(u8::is_ascii_digit) {
        Ok(Value::Number(parse_i64_ascii(token)?))
    } else {
        Ok(Value::Literal(Cow::Borrowed(token)))
    }
}

#[inline]
fn parse_i64_ascii(token: &str) -> PResult<'_, i64> {
    let bytes = token.as_bytes();
    let (negative, start) = match bytes.first() {
        Some(b'-') => (true, 1),
        Some(b'+') => (false, 1),
        _ => (false, 0),
    };

    if start >= bytes.len() {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    let mut value: i64 = 0;
    for &byte in &bytes[start..] {
        if !byte.is_ascii_digit() {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ));
        }

        let digit = i64::from(byte - b'0');
        value = if negative {
            value
                .checked_mul(10)
                .and_then(|v| v.checked_sub(digit))
                .ok_or_else(|| {
                    winnow::error::ErrMode::Backtrack(winnow::error::ContextError::default())
                })?
        } else {
            value
                .checked_mul(10)
                .and_then(|v| v.checked_add(digit))
                .ok_or_else(|| {
                    winnow::error::ErrMode::Backtrack(winnow::error::ContextError::default())
                })?
        };
    }

    Ok(value)
}

/// Parse a variable reference
#[inline]
fn parse_variable_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    // Parse as identifier - digit-starting values are handled by parse_number_or_digit_string
    let ident = lexer::identifier(input)?;
    Ok(Value::Variable(Cow::Borrowed(ident)))
}

#[inline]
fn parse_variable_value_cursor<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, Value<'a>> {
    let ident = cursor
        .take_identifier()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::default()))?;
    Ok(Value::Variable(Cow::Borrowed(ident)))
}

/// Normalize a string value (remove excessive whitespace, handle LaTeX)
#[must_use]
pub fn normalize_value(s: &str) -> String {
    // Basic normalization - can be extended with LaTeX processing
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quoted_value() {
        let mut input = r#""hello world" xxx"#;
        let value = parse_value(&mut input).unwrap();
        assert_eq!(value, Value::Literal(Cow::Borrowed("hello world")));
        assert_eq!(input, " xxx");
    }

    #[test]
    fn test_parse_braced_value() {
        let mut input = "{hello world} xxx";
        let value = parse_value(&mut input).unwrap();
        assert_eq!(value, Value::Literal(Cow::Borrowed("hello world")));
        assert_eq!(input, " xxx");
    }

    #[test]
    fn test_parse_number_value() {
        let mut input = "2023 xxx";
        let value = parse_value(&mut input).unwrap();
        assert_eq!(value, Value::Number(2023));
        assert_eq!(input, " xxx");
    }

    #[test]
    fn test_parse_variable_value() {
        let mut input = "myvar xxx";
        let value = parse_value(&mut input).unwrap();
        assert_eq!(value, Value::Variable(Cow::Borrowed("myvar")));
        assert_eq!(input, " xxx");
    }

    #[test]
    fn test_parse_concatenated_value() {
        let mut input = r#""hello" # myvar # {world} xxx"#;
        let value = parse_value(&mut input).unwrap();
        match value {
            Value::Concat(parts) => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], Value::Literal(Cow::Borrowed("hello")));
                assert_eq!(parts[1], Value::Variable(Cow::Borrowed("myvar")));
                assert_eq!(parts[2], Value::Literal(Cow::Borrowed("world")));
            }
            _ => panic!("Expected concatenated value"),
        }
        assert_eq!(input, " xxx");
    }
}
