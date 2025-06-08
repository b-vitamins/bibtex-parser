//! Entry parsing for BibTeX

use super::{lexer, utils, value, PResult};
use crate::model::{Entry, EntryType, Field};
use winnow::prelude::*;
use winnow::{ascii::multispace0, combinator::preceded};

/// Parse a bibliography entry
pub fn parse_entry<'a>(input: &mut &'a str) -> PResult<'a, Entry<'a>> {
    preceded((multispace0, '@'), parse_entry_content).parse_next(input)
}

/// Parse the content of an entry after the @
fn parse_entry_content<'a>(input: &mut &'a str) -> PResult<'a, Entry<'a>> {
    // Parse entry type
    let entry_type_str = lexer::identifier.parse_next(input)?;
    let entry_type = EntryType::parse(entry_type_str);

    // Skip whitespace
    lexer::skip_whitespace(input);

    // Check delimiter and parse accordingly
    if input.starts_with('{') {
        *input = &input[1..];
        let entry = parse_entry_body(input, entry_type)?;
        utils::ws('}').parse_next(input)?;
        Ok(entry)
    } else if input.starts_with('(') {
        *input = &input[1..];
        let entry = parse_entry_body(input, entry_type)?;
        utils::ws(')').parse_next(input)?;
        Ok(entry)
    } else {
        Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::default(),
        ))
    }
}

/// Parse the body of an entry (key and fields)
fn parse_entry_body<'a>(input: &mut &'a str, entry_type: EntryType<'a>) -> PResult<'a, Entry<'a>> {
    // Parse citation key
    let key = utils::ws(lexer::identifier).parse_next(input)?;

    // Parse comma
    utils::ws(',').parse_next(input)?;

    // Parse fields
    let fields = parse_fields.parse_next(input)?;

    Ok(Entry {
        ty: entry_type,
        key,
        fields,
    })
}

/// Parse all fields in an entry
fn parse_fields<'a>(input: &mut &'a str) -> PResult<'a, Vec<Field<'a>>> {
    let mut fields = Vec::new();

    loop {
        // Skip whitespace
        lexer::skip_whitespace(input);

        // Check if we've reached the end
        if input.starts_with('}') || input.starts_with(')') || input.is_empty() {
            break;
        }

        // Try to parse a field
        match parse_field(input) {
            Ok(field) => {
                fields.push(field);

                // Look for comma
                lexer::skip_whitespace(input);
                if input.starts_with(',') {
                    *input = &input[1..];
                } else {
                    // No comma, we should be at the end
                    lexer::skip_whitespace(input);
                    if !input.starts_with('}') && !input.starts_with(')') {
                        return Err(winnow::error::ErrMode::Backtrack(
                            winnow::error::ContextError::default(),
                        ));
                    }
                }
            }
            Err(_) => break,
        }
    }

    Ok(fields)
}

/// Parse a single field (name = value)
fn parse_field<'a>(input: &mut &'a str) -> PResult<'a, Field<'a>> {
    let name = utils::ws(lexer::field_name).parse_next(input)?;
    utils::ws('=').parse_next(input)?;
    let value = utils::ws(value::parse_value).parse_next(input)?;

    Ok(Field { name, value })
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
        assert_eq!(entry.key, "einstein1905");
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
        assert_eq!(entry.key, "test");
        assert_eq!(entry.fields.len(), 2);

        match &entry.fields[0].value {
            Value::Concat(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0], Value::Variable("name"));
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
}
