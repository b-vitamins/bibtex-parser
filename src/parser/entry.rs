//! Entry parsing for BibTeX

use super::{lexer, value, PResult};
use crate::model::{Entry, EntryType, Field};
use std::borrow::Cow;

/// Parse a bibliography entry
#[inline]
pub fn parse_entry<'a>(input: &mut &'a str) -> PResult<'a, Entry<'a>> {
    lexer::skip_whitespace(input);
    parse_entry_at(input)
}

/// Parse a bibliography entry when `input` is already positioned at `@`.
#[inline]
pub fn parse_entry_at<'a>(input: &mut &'a str) -> PResult<'a, Entry<'a>> {
    match input.as_bytes().first() {
        Some(b'@') => {
            *input = &input[1..];
            parse_entry_content(input)
        }
        _ => Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        )),
    }
}

fn parse_entry_content<'a>(input: &mut &'a str) -> PResult<'a, Entry<'a>> {
    let entry_type_str = lexer::identifier(input)?;
    let entry_type = EntryType::parse(entry_type_str);

    lexer::skip_whitespace(input);

    let closing_delimiter = match input.as_bytes().first() {
        Some(b'{') => b'}',
        Some(b'(') => b')',
        _ => {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        }
    };
    *input = &input[1..];

    parse_entry_body(input, entry_type, closing_delimiter)
}

/// Parse the body of an entry (key and fields)
#[inline]
fn parse_entry_body<'a>(
    input: &mut &'a str,
    entry_type: EntryType<'a>,
    closing_delimiter: u8,
) -> PResult<'a, Entry<'a>> {
    lexer::skip_whitespace(input);
    let key = lexer::identifier(input)?;

    lexer::skip_whitespace(input);
    expect_byte(input, b',')?;

    let fields = parse_fields(input, closing_delimiter)?;

    lexer::skip_whitespace(input);
    expect_byte(input, closing_delimiter)?;

    Ok(Entry {
        ty: entry_type,
        key: Cow::Borrowed(key),
        fields,
    })
}

#[inline]
fn expect_byte<'a>(input: &mut &'a str, byte: u8) -> PResult<'a, ()> {
    match input.as_bytes().first() {
        Some(&b) if b == byte => {
            *input = &input[1..];
            Ok(())
        }
        _ => Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        )),
    }
}

/// Parse all fields in an entry.
fn parse_fields<'a>(input: &mut &'a str, closing_delimiter: u8) -> PResult<'a, Vec<Field<'a>>> {
    let mut fields = Vec::new();

    loop {
        lexer::skip_whitespace(input);

        let first = match input.as_bytes().first() {
            Some(&b) => b,
            None => break,
        };
        if first == closing_delimiter {
            break;
        }

        let field = parse_field(input)?;
        fields.push(field);

        lexer::skip_whitespace(input);
        match input.as_bytes().first() {
            Some(b',') => {
                *input = &input[1..];
            }
            Some(&b) if b == closing_delimiter => {}
            _ => {
                return Err(winnow::error::ErrMode::Backtrack(
                    winnow::error::ContextError::default(),
                ))
            }
        }
    }

    Ok(fields)
}

/// Parse a single field (name = value)
#[inline]
fn parse_field<'a>(input: &mut &'a str) -> PResult<'a, Field<'a>> {
    lexer::skip_whitespace(input);
    let name = lexer::field_name(input)?;
    lexer::skip_whitespace(input);
    expect_byte(input, b'=')?;
    lexer::skip_whitespace(input);
    let value = value::parse_value(input)?;

    Ok(Field {
        name: Cow::Borrowed(name),
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Value;
    use std::borrow::Cow;

    #[test]
    fn test_parse_simple_entry() {
        let mut input = r#"@article{einstein1905,
            author = "Albert Einstein",
            title = {Zur Elektrodynamik bewegter Körper},
            year = 1905
        }"#;

        let entry = parse_entry(&mut input).unwrap();
        assert_eq!(entry.ty, EntryType::Article);
        assert_eq!(entry.key, Cow::Borrowed("einstein1905"));
        assert_eq!(entry.fields.len(), 3);

        assert_eq!(entry.fields[0].name, "author");
        assert_eq!(
            entry.fields[0].value,
            Value::Literal(Cow::Borrowed("Albert Einstein"))
        );

        assert_eq!(entry.fields[1].name, "title");
        assert_eq!(
            entry.fields[1].value,
            Value::Literal(Cow::Borrowed("Zur Elektrodynamik bewegter Körper"))
        );

        assert_eq!(entry.fields[2].name, "year");
        assert_eq!(entry.fields[2].value, Value::Number(1905));
    }

    #[test]
    fn test_parse_entry_with_concatenation() {
        let mut input = r#"@misc{test,
            author = name # " et al.",
            note = "See " # url
        }"#;

        let entry = parse_entry(&mut input).unwrap();
        assert_eq!(entry.ty, EntryType::Misc);
        assert_eq!(entry.key, Cow::Borrowed("test"));
        assert_eq!(entry.fields.len(), 2);

        match &entry.fields[0].value {
            Value::Concat(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0], Value::Variable(Cow::Borrowed("name")));
                assert_eq!(parts[1], Value::Literal(Cow::Borrowed(" et al.")));
            }
            _ => panic!("Expected concatenated value"),
        }
    }

    #[test]
    fn test_parse_entry_with_trailing_comma() {
        let mut input = r#"@book{knuth1984,
            author = "Donald Knuth",
            title = "The TeXbook",
            year = 1984,
        }"#;

        let entry = parse_entry(&mut input).unwrap();
        assert_eq!(entry.fields.len(), 3);
    }

    #[test]
    fn test_parse_entry_with_parentheses() {
        let mut input = r#"@article(einstein1905,
            author = "Albert Einstein",
            title = {Zur Elektrodynamik bewegter Körper},
            year = 1905
        )"#;

        let entry = parse_entry(&mut input).unwrap();
        assert_eq!(entry.ty, EntryType::Article);
        assert_eq!(entry.key, Cow::Borrowed("einstein1905"));
        assert_eq!(entry.fields.len(), 3);

        assert_eq!(entry.fields[0].name, "author");
        assert_eq!(
            entry.fields[0].value,
            Value::Literal(Cow::Borrowed("Albert Einstein"))
        );

        assert_eq!(entry.fields[1].name, "title");
        assert_eq!(
            entry.fields[1].value,
            Value::Literal(Cow::Borrowed("Zur Elektrodynamik bewegter Körper"))
        );

        assert_eq!(entry.fields[2].name, "year");
        assert_eq!(entry.fields[2].value, Value::Number(1905));
    }

    #[test]
    fn test_parse_entry_mixed_delimiters() {
        // Entry uses parentheses, but field values can use braces
        let mut input = r#"@book(test2024,
            title = {A Title with {Nested} Braces},
            author = "John Doe"
        )"#;

        let entry = parse_entry(&mut input).unwrap();
        assert_eq!(entry.ty, EntryType::Book);
        assert_eq!(entry.key, Cow::Borrowed("test2024"));
        assert_eq!(entry.fields.len(), 2);

        assert_eq!(entry.fields[0].name, "title");
        assert_eq!(
            entry.fields[0].value,
            Value::Literal(Cow::Borrowed("A Title with {Nested} Braces"))
        );

        assert_eq!(entry.fields[1].name, "author");
        assert_eq!(
            entry.fields[1].value,
            Value::Literal(Cow::Borrowed("John Doe"))
        );
    }
}
