//! Value parsing for BibTeX fields

use super::{lexer, PResult};
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

/// Parse a concatenated value (value # value # ...)
#[inline]
fn parse_concatenated_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let first = parse_single_value(input)?;

    // Fast path: most fields are a single value with no concatenation.
    if !consume_concat_separator(input) {
        return Ok(first);
    }

    // Slow path: parse one or more `# value` segments.
    let mut parts = Vec::with_capacity(3);
    parts.push(first);

    loop {
        let part = parse_single_value(input)?;
        parts.push(part);

        if !consume_concat_separator(input) {
            break;
        }
    }

    Ok(Value::Concat(parts.into_boxed_slice()))
}

/// Parse a concatenated value and consume trailing ASCII whitespace.
#[inline]
fn parse_concatenated_value_field<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let first = parse_single_value(input)?;

    if !consume_concat_separator_field(input) {
        return Ok(first);
    }

    // Slow path: parse one or more `# value` segments.
    let mut parts = Vec::with_capacity(3);
    parts.push(first);

    loop {
        let part = parse_single_value(input)?;
        parts.push(part);

        if !consume_concat_separator_field(input) {
            break;
        }
    }

    Ok(Value::Concat(parts.into_boxed_slice()))
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

/// Consume optional trailing whitespace and a field-value concatenation marker.
///
/// Unlike `consume_concat_separator`, this variant keeps the field parser's
/// contract: trailing whitespace is consumed even when no `#` follows.
#[inline]
fn consume_concat_separator_field(input: &mut &str) -> bool {
    match input.as_bytes().first() {
        Some(b'#') => {
            *input = &input[1..];
            lexer::skip_whitespace(input);
            true
        }
        Some(b' ' | b'\t' | b'\n' | b'\r') => {
            lexer::skip_whitespace(input);
            if input.as_bytes().first() == Some(&b'#') {
                *input = &input[1..];
                lexer::skip_whitespace(input);
                true
            } else {
                false
            }
        }
        Some(_) | None => false,
    }
}

/// Parse a single value component
#[inline]
fn parse_single_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    // Fast dispatch based on first character
    let bytes = input.as_bytes();
    if let Some(&first) = bytes.first() {
        match first {
            b'"' => {
                super::simd::find_balanced_quotes(bytes).map_or_else(super::backtrack, |end_pos| {
                    let content = &input[1..end_pos - 1];
                    *input = &input[end_pos..];
                    Ok(Value::Literal(Cow::Borrowed(content)))
                })
            }
            b'{' => {
                super::simd::find_balanced_braces(bytes).map_or_else(super::backtrack, |end_pos| {
                    let content = &input[1..end_pos - 1];
                    *input = &input[end_pos..];
                    Ok(Value::Literal(Cow::Borrowed(content)))
                })
            }
            b'0'..=b'9' | b'+' | b'-' => parse_number_or_digit_string(input),
            _ => parse_variable_value(input),
        }
    } else {
        super::backtrack()
    }
}

/// Parse either a number or a string that starts with digits
/// This handles cases like "2024a", "12b", "1.2.3", etc.
#[inline]
fn parse_number_or_digit_string<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let bytes = input.as_bytes();
    let Some(&first) = bytes.first() else {
        return super::backtrack();
    };

    let len = super::simd::scan_identifier(bytes);
    if len == 0 {
        return super::backtrack();
    }

    let token = &input[..len];
    let token_bytes = token.as_bytes();

    // Signed values must be strict integers (e.g., +42, -1).
    // Non-digit suffixes after a sign are rejected.
    if first == b'+' || first == b'-' {
        if token_bytes.len() <= 1 || !token_bytes[1..].iter().all(u8::is_ascii_digit) {
            return super::backtrack();
        }
        let num = parse_i64_ascii(token)?;
        *input = &input[len..];
        return Ok(Value::Number(num));
    }

    // Digit-starting tokens parse as numbers when fully numeric,
    // otherwise as literals (e.g. 2024a).
    if !first.is_ascii_digit() {
        return super::backtrack();
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
fn parse_i64_ascii(token: &str) -> PResult<'_, i64> {
    let bytes = token.as_bytes();
    let (negative, start) = match bytes.first() {
        Some(b'-') => (true, 1),
        Some(b'+') => (false, 1),
        _ => (false, 0),
    };

    if start >= bytes.len() {
        return super::backtrack();
    }

    let mut value: i64 = 0;
    for &byte in &bytes[start..] {
        if !byte.is_ascii_digit() {
            return super::backtrack();
        }

        let digit = i64::from(byte - b'0');
        value = if negative {
            value
                .checked_mul(10)
                .and_then(|v| v.checked_sub(digit))
                .ok_or_else(super::backtrack_err)?
        } else {
            value
                .checked_mul(10)
                .and_then(|v| v.checked_add(digit))
                .ok_or_else(super::backtrack_err)?
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
