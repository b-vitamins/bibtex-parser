//! BibTeX parser implementation using winnow

pub mod entry;
pub mod lexer;
pub mod utils;
pub mod value;

use crate::{Error, Result};
use winnow::ascii::multispace0;
use winnow::prelude::*;

pub use entry::parse_entry;

/// Internal parser result type
pub type PResult<'a, O> = winnow::PResult<O, winnow::error::ContextError>;

/// Parse a complete BibTeX database
pub fn parse_bibtex(input: &str) -> Result<Vec<ParsedItem>> {
    let mut items = Vec::new();
    let mut remaining = input;

    while !remaining.trim().is_empty() {
        // Skip only whitespace (not comments!)
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }

        // Try to parse an item (including comments)
        match parse_item(&mut remaining) {
            Ok(item) => items.push(item),
            Err(e) => {
                // Calculate line/column for error
                let consumed = input.len() - remaining.len();
                let (line, column) = calculate_position(input, consumed);

                return Err(Error::ParseError {
                    line,
                    column,
                    message: format!("Failed to parse entry: {e}"),
                    snippet: Some(get_snippet(remaining, 40)),
                });
            }
        }
    }

    Ok(items)
}

/// A parsed item from the BibTeX file
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedItem<'a> {
    /// A bibliography entry
    Entry(crate::Entry<'a>),
    /// A string definition
    String(&'a str, crate::Value<'a>),
    /// A preamble
    Preamble(crate::Value<'a>),
    /// A comment
    Comment(&'a str),
}

/// Parse a single item (entry, string, preamble, or comment)
fn parse_item<'a>(input: &mut &'a str) -> PResult<'a, ParsedItem<'a>> {
    winnow::combinator::alt((
        entry::parse_entry.map(ParsedItem::Entry),
        parse_string.map(|(k, v)| ParsedItem::String(k, v)),
        parse_preamble.map(ParsedItem::Preamble),
        parse_comment.map(ParsedItem::Comment),
    ))
    .parse_next(input)
}

/// Parse a @string definition
fn parse_string<'a>(input: &mut &'a str) -> PResult<'a, (&'a str, crate::Value<'a>)> {
    use winnow::combinator::{alt, delimited, preceded};

    preceded(
        (multispace0, '@', utils::tag_no_case("string"), multispace0),
        alt((
            delimited('{', parse_string_content, '}'),
            delimited('(', parse_string_content, ')'),
        )),
    )
    .parse_next(input)
}

/// Parse the content of a @string definition
fn parse_string_content<'a>(input: &mut &'a str) -> PResult<'a, (&'a str, crate::Value<'a>)> {
    use winnow::combinator::separated_pair;

    separated_pair(
        utils::ws(lexer::identifier),
        utils::ws('='),
        utils::ws(value::parse_value),
    )
    .parse_next(input)
}

/// Parse a @preamble
fn parse_preamble<'a>(input: &mut &'a str) -> PResult<'a, crate::Value<'a>> {
    use winnow::combinator::{alt, delimited, preceded};

    preceded(
        (
            multispace0,
            '@',
            utils::tag_no_case("preamble"),
            multispace0,
        ),
        alt((
            delimited('{', parse_preamble_value, '}'),
            delimited('(', parse_preamble_value, ')'),
        )),
    )
    .parse_next(input)
}

/// Helper function to parse preamble value
fn parse_preamble_value<'a>(input: &mut &'a str) -> PResult<'a, crate::Value<'a>> {
    utils::ws(value::parse_value).parse_next(input)
}

/// Parse a comment (different formats)
fn parse_comment<'a>(input: &mut &'a str) -> PResult<'a, &'a str> {
    use winnow::ascii::till_line_ending;
    use winnow::combinator::{alt, delimited, preceded};
    use winnow::token::take_until;

    alt((
        // @comment{...}
        preceded(
            (multispace0, '@', utils::tag_no_case("comment"), multispace0),
            alt((
                delimited('{', lexer::balanced_braces, '}'),
                delimited('(', take_until(0.., ")"), ')'),
            )),
        ),
        // % line comment
        preceded('%', till_line_ending),
        // Any text before @ is considered a comment
        take_until(1.., "@").verify(|s: &str| !s.trim().is_empty()),
    ))
    .parse_next(input)
}

/// Calculate line and column from position
fn calculate_position(input: &str, pos: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;

    for (i, ch) in input.chars().enumerate() {
        if i >= pos {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

/// Get a snippet of input for error messages
fn get_snippet(input: &str, max_len: usize) -> String {
    let snippet: String = input.chars().take(max_len).collect();
    if input.len() > max_len {
        format!("{snippet}...")
    } else {
        snippet
    }
}
