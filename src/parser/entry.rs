//! Entry parsing for BibTeX

use super::{lexer, value, PResult};
use crate::model::{Entry, EntryType, Field};
use crate::{EntryDelimiter, Value, ValueDelimiter};
use std::borrow::Cow;

const DEFAULT_FIELD_CAPACITY: usize = 17;

#[derive(Debug, Clone)]
pub(crate) struct LocatedEntry<'a> {
    pub(crate) entry: Entry<'a>,
    pub(crate) entry_type: (usize, usize),
    pub(crate) key: (usize, usize),
    pub(crate) delimiter: EntryDelimiter,
    pub(crate) fields: Vec<LocatedField>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocatedField {
    pub(crate) whole: (usize, usize),
    pub(crate) name: (usize, usize),
    pub(crate) value: (usize, usize),
    pub(crate) value_delimiter: ValueDelimiter,
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

#[inline]
pub(crate) fn parse_entry_at_with_locations<'a>(
    input: &mut &'a str,
    absolute_start: usize,
) -> PResult<'a, LocatedEntry<'a>> {
    let root = *input;
    match input.as_bytes().first() {
        Some(b'@') => {
            *input = &input[1..];
            parse_entry_content_with_locations(input, root, absolute_start)
        }
        _ => super::backtrack(),
    }
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
fn parse_entry_content_with_locations<'a>(
    input: &mut &'a str,
    root: &'a str,
    absolute_start: usize,
) -> PResult<'a, LocatedEntry<'a>> {
    let entry_type_start = source_offset(root, input, absolute_start);
    let entry_type_str = lexer::identifier(input)?;
    let entry_type_end = source_offset(root, input, absolute_start);
    let entry_type = EntryType::parse(entry_type_str);

    lexer::skip_whitespace(input);

    let opening = match input.as_bytes().first() {
        Some(b'{') => b'{',
        Some(b'(') => b'(',
        _ => return super::backtrack(),
    };
    let (delimiter, closing_delimiter) = match opening {
        b'{' => (EntryDelimiter::Braces, b'}'),
        b'(' => (EntryDelimiter::Parentheses, b')'),
        _ => unreachable!(),
    };
    *input = &input[1..];

    parse_entry_body_with_locations(
        input,
        root,
        absolute_start,
        entry_type,
        (entry_type_start, entry_type_end),
        delimiter,
        closing_delimiter,
    )
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
fn parse_entry_body_with_locations<'a>(
    input: &mut &'a str,
    root: &'a str,
    absolute_start: usize,
    entry_type: EntryType<'a>,
    entry_type_location: (usize, usize),
    delimiter: EntryDelimiter,
    closing_delimiter: u8,
) -> PResult<'a, LocatedEntry<'a>> {
    lexer::skip_whitespace(input);
    let key_start = source_offset(root, input, absolute_start);
    let key = lexer::identifier(input)?;
    let key_end = source_offset(root, input, absolute_start);

    lexer::skip_whitespace(input);
    expect_byte(input, b',')?;

    let (fields, field_locations) =
        parse_fields_with_locations(input, root, absolute_start, closing_delimiter)?;
    expect_byte(input, closing_delimiter)?;

    Ok(LocatedEntry {
        entry: Entry {
            ty: entry_type,
            key: Cow::Borrowed(key),
            fields,
        },
        entry_type: entry_type_location,
        key: (key_start, key_end),
        delimiter,
        fields: field_locations,
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

/// Parse all fields in an entry.
#[inline]
fn parse_fields<'a>(input: &mut &'a str, closing_delimiter: u8) -> PResult<'a, Vec<Field<'a>>> {
    let mut fields = Vec::with_capacity(DEFAULT_FIELD_CAPACITY);

    while let Some(first) = lexer::skip_whitespace_peek(input) {
        if first == closing_delimiter {
            break;
        }

        let name = lexer::field_name(input)?;
        lexer::skip_whitespace(input);
        expect_byte(input, b'=')?;
        lexer::skip_whitespace(input);
        let value = value::parse_value_field(input)?;

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

    let max_reasonable_capacity = (fields.len() * 2).max(8);
    if fields.capacity() > max_reasonable_capacity {
        fields.shrink_to_fit();
    }

    Ok(fields)
}

#[inline]
fn parse_fields_with_locations<'a>(
    input: &mut &'a str,
    root: &'a str,
    absolute_start: usize,
    closing_delimiter: u8,
) -> PResult<'a, (Vec<Field<'a>>, Vec<LocatedField>)> {
    let mut fields = Vec::with_capacity(DEFAULT_FIELD_CAPACITY);
    let mut locations = Vec::with_capacity(DEFAULT_FIELD_CAPACITY);
    let root_bytes = root.as_bytes();

    while let Some(first) = lexer::skip_whitespace_peek(input) {
        if first == closing_delimiter {
            break;
        }

        let field_start = source_offset(root, input, absolute_start);
        let name_start = field_start;
        let name = lexer::field_name(input)?;
        let name_end = source_offset(root, input, absolute_start);

        lexer::skip_whitespace(input);
        expect_byte(input, b'=')?;
        lexer::skip_whitespace(input);

        let value_start = source_offset(root, input, absolute_start);
        let parsed_value = value::parse_value_field(input)?;
        let value_boundary = source_offset(root, input, absolute_start);
        let value_end = trim_ascii_whitespace_end_absolute(
            root_bytes,
            absolute_start,
            value_start,
            value_boundary,
        );
        let value_delimiter = value_delimiter_from_parse(
            &parsed_value,
            root_bytes,
            absolute_start,
            value_start,
            value_end,
        );

        let mut whole_end = value_end;
        match input.as_bytes().first() {
            Some(b',') => {
                whole_end = source_offset(root, input, absolute_start) + 1;
                *input = &input[1..];
            }
            Some(&b) if b == closing_delimiter => {}
            _ => return super::backtrack(),
        }

        fields.push(Field {
            name: Cow::Borrowed(name),
            value: parsed_value,
        });
        locations.push(LocatedField {
            whole: (field_start, whole_end),
            name: (name_start, name_end),
            value: (value_start, value_end),
            value_delimiter,
        });
    }

    let max_reasonable_capacity = (fields.len() * 2).max(8);
    if fields.capacity() > max_reasonable_capacity {
        fields.shrink_to_fit();
    }
    if locations.capacity() > max_reasonable_capacity {
        locations.shrink_to_fit();
    }

    Ok((fields, locations))
}

#[inline]
const fn source_offset(root: &str, input: &str, absolute_start: usize) -> usize {
    absolute_start + root.len() - input.len()
}

#[inline]
fn trim_ascii_whitespace_end_absolute(
    bytes: &[u8],
    absolute_start: usize,
    start: usize,
    end: usize,
) -> usize {
    let mut pos = end - absolute_start;
    let start = start - absolute_start;
    while pos > start && bytes[pos - 1].is_ascii_whitespace() {
        pos -= 1;
    }
    absolute_start + pos
}

#[inline]
fn value_delimiter_from_parse(
    value: &Value<'_>,
    bytes: &[u8],
    absolute_start: usize,
    start: usize,
    end: usize,
) -> ValueDelimiter {
    if matches!(value, Value::Concat(_)) {
        return ValueDelimiter::Concatenation;
    }

    let start = start - absolute_start;
    let end = end - absolute_start;
    match bytes.get(start..end).and_then(|raw| raw.first()).copied() {
        Some(b'{') => ValueDelimiter::Braces,
        Some(b'"') => ValueDelimiter::Quotes,
        _ => ValueDelimiter::Bare,
    }
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
