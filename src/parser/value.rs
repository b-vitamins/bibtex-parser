//! Value parsing for BibTeX fields

use super::{lexer, PResult};
use crate::model::Value;
use std::borrow::Cow;

/// Parse a BibTeX value (string, number, variable, or concatenation)
#[inline]
pub fn parse_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    parse_concatenated_value(input)
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

/// Parse either a number or a string that starts with digits
/// This handles cases like "2024a", "12b", "1.2.3", etc.
#[inline]
fn parse_number_or_digit_string<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let start_input = *input;

    // Try to parse as a pure number first, but only if it consumes a complete token
    if let Ok(num) = lexer::number(input) {
        // Check if number consumed entire token (next char should be whitespace, delimiter, or end)
        if input.is_empty()
            || input.chars().next().map_or(true, |c| {
                c.is_whitespace() || c == ',' || c == '}' || c == ')' || c == '#'
            })
        {
            return Ok(Value::Number(num));
        }
    }

    // Reset input and try to parse as identifier starting with digit
    *input = start_input;

    // Check if first character is a digit - if not, this parser doesn't apply
    if !input.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    // Parse as identifier (allows digits, letters, hyphens, dots, etc.)
    let ident = lexer::identifier(input)?;

    // Since it starts with a digit, treat as string literal
    Ok(Value::Literal(Cow::Borrowed(ident)))
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
