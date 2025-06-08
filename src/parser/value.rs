//! Value parsing for BibTeX fields

use super::{lexer, utils, PResult};
use crate::model::Value;
use std::borrow::Cow;
use winnow::combinator::{alt, separated};
use winnow::prelude::*;

/// Parse a BibTeX value (string, number, variable, or concatenation)
pub fn parse_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    parse_concatenated_value.parse_next(input)
}

/// Parse a concatenated value (value # value # ...)
fn parse_concatenated_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let parts: Vec<Value<'a>> =
        separated(1.., parse_single_value, utils::ws('#')).parse_next(input)?;

    match parts.len() {
        1 => Ok(parts.into_iter().next().unwrap()),
        _ => Ok(Value::Concat(parts)),
    }
}

/// Parse a single value component
fn parse_single_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    alt((
        parse_quoted_value,
        parse_braced_value,
        parse_number_value,
        parse_variable_value,
    ))
    .parse_next(input)
}

/// Parse a quoted string value
fn parse_quoted_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    if !input.starts_with('"') {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    // Use the lexer to parse the quoted string (it handles the quotes)
    let s = lexer::quoted_string(input)?;
    Ok(Value::Literal(Cow::Borrowed(s)))
}

/// Parse a braced string value
fn parse_braced_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    if !input.starts_with('{') {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    // Consume opening brace
    *input = &input[1..];

    // Parse balanced braces content
    let content = lexer::balanced_braces(input)?;

    // Consume closing brace
    if input.starts_with('}') {
        *input = &input[1..];
    } else {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    Ok(Value::Literal(Cow::Borrowed(content)))
}

/// Parse a number value
fn parse_number_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    let num = lexer::number(input)?;
    Ok(Value::Number(num))
}

/// Parse a variable reference
fn parse_variable_value<'a>(input: &mut &'a str) -> PResult<'a, Value<'a>> {
    // First check if it looks like an identifier and not a number
    if input.chars().next().map_or(true, char::is_numeric) {
        return Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ));
    }

    let ident = lexer::identifier(input)?;
    Ok(Value::Variable(ident))
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
        assert_eq!(value, Value::Variable("myvar"));
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
                assert_eq!(parts[1], Value::Variable("myvar"));
                assert_eq!(parts[2], Value::Literal(Cow::Borrowed("world")));
            }
            _ => panic!("Expected concatenated value"),
        }
        assert_eq!(input, " xxx");
    }
}
