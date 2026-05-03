//! Entry parsing for BibTeX

use super::{lexer, value, Cursor, PResult};
use crate::model::{Entry, EntryType, Field};
use std::borrow::Cow;

const INITIAL_FIELD_CAPACITY: usize = 17;
const TARGET_FIELD_CAPACITY: usize = 17;
const SMALL_FIELD_CAPACITY: usize = 8;

#[inline]
fn reserve_field_slot(fields: &mut Vec<Field<'_>>) {
    if fields.len() == fields.capacity() && fields.capacity() < TARGET_FIELD_CAPACITY {
        fields.reserve_exact(TARGET_FIELD_CAPACITY - fields.capacity());
    }
}

#[inline]
fn shrink_small_field_vec(fields: &mut Vec<Field<'_>>) {
    let max_reasonable_capacity = (fields.len() * 2).max(SMALL_FIELD_CAPACITY);
    if fields.capacity() > max_reasonable_capacity {
        fields.shrink_to(max_reasonable_capacity);
    }
}

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
        _ => super::backtrack(),
    }
}

/// Parse an entry directly from the streaming parser cursor.
#[inline]
pub(crate) fn parse_entry_fast<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, Entry<'a>> {
    match cursor.remaining_bytes().first() {
        Some(b'@') => cursor.bump(1),
        _ => {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        }
    }

    parse_entry_content_cursor(cursor)
}

#[inline]
fn parse_entry_content<'a>(input: &mut &'a str) -> PResult<'a, Entry<'a>> {
    let entry_type_str = lexer::identifier(input)?;
    let entry_type = EntryType::parse(entry_type_str);

    lexer::skip_whitespace(input);

    let closing_delimiter = match input.as_bytes().first() {
        Some(b'{') => b'}',
        Some(b'(') => b')',
        _ => return super::backtrack(),
    };
    *input = &input[1..];

    parse_entry_body(input, entry_type, closing_delimiter)
}

#[inline]
fn parse_entry_content_cursor<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, Entry<'a>> {
    let entry_type_str = cursor
        .take_identifier()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::default()))?;
    let entry_type = EntryType::parse(entry_type_str);

    cursor.skip_whitespace();

    let closing_delimiter = match cursor.remaining_bytes().first() {
        Some(b'{') => b'}',
        Some(b'(') => b')',
        _ => {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        }
    };
    cursor.bump(1);

    parse_entry_body_cursor(cursor, entry_type, closing_delimiter)
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
    expect_byte(input, closing_delimiter)?;

    Ok(Entry {
        ty: entry_type,
        key: Cow::Borrowed(key),
        fields,
    })
}

#[inline]
fn parse_entry_body_cursor<'a>(
    cursor: &mut Cursor<'a>,
    entry_type: EntryType<'a>,
    closing_delimiter: u8,
) -> PResult<'a, Entry<'a>> {
    cursor.skip_whitespace();
    let key = cursor
        .take_identifier()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::default()))?;

    cursor.skip_whitespace();
    expect_byte_cursor(cursor, b',')?;

    let fields = parse_fields_cursor(cursor, closing_delimiter)?;

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
        _ => super::backtrack(),
    }
}

#[inline]
fn expect_byte_cursor<'a>(cursor: &mut Cursor<'a>, byte: u8) -> PResult<'a, ()> {
    match cursor.remaining_bytes().first() {
        Some(&b) if b == byte => {
            cursor.bump(1);
            Ok(())
        }
        _ => Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        )),
    }
}

/// Parse all fields in an entry.
fn parse_fields<'a>(input: &mut &'a str, closing_delimiter: u8) -> PResult<'a, Vec<Field<'a>>> {
    let mut fields = Vec::with_capacity(INITIAL_FIELD_CAPACITY);

    loop {
        let Some(first) = lexer::skip_whitespace_peek(input) else {
            break;
        };
        if first == closing_delimiter {
            break;
        }

        let name = lexer::field_name(input)?;
        lexer::skip_whitespace(input);
        expect_byte(input, b'=')?;
        lexer::skip_whitespace(input);
        let value = value::parse_value_field(input)?;

        reserve_field_slot(&mut fields);
        fields.push(Field {
            name: Cow::Borrowed(name),
            value,
        });

        match input.as_bytes().first() {
            Some(b',') => {
                *input = &input[1..];
            }
            Some(&b) if b == closing_delimiter => {}
            _ => return super::backtrack(),
        }
    }

    shrink_small_field_vec(&mut fields);
    Ok(fields)
}

fn parse_fields_cursor<'a>(
    cursor: &mut Cursor<'a>,
    closing_delimiter: u8,
) -> PResult<'a, Vec<Field<'a>>> {
    let mut fields = Vec::with_capacity(INITIAL_FIELD_CAPACITY);

    loop {
        cursor.skip_whitespace();

        let Some(&first) = cursor.remaining_bytes().first() else {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ));
        };

        if first == closing_delimiter {
            cursor.bump(1);
            break;
        }

        let name = cursor.take_identifier().ok_or_else(|| {
            winnow::error::ErrMode::Backtrack(winnow::error::ContextError::default())
        })?;
        cursor.skip_whitespace();
        expect_byte_cursor(cursor, b'=')?;
        cursor.skip_whitespace();
        let value = value::parse_value_field_cursor(cursor)?;

        reserve_field_slot(&mut fields);
        fields.push(Field {
            name: Cow::Borrowed(name),
            value,
        });

        match cursor.remaining_bytes().first() {
            Some(b',') => {
                cursor.bump(1);
            }
            Some(&b) if b == closing_delimiter => {
                cursor.bump(1);
                break;
            }
            _ => {
                return Err(winnow::error::ErrMode::Backtrack(
                    winnow::error::ContextError::default(),
                ))
            }
        }
    }

    shrink_small_field_vec(&mut fields);
    Ok(fields)
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
