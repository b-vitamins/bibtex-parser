//! BibTeX parser implementation using winnow
//!
//! This module provides both high-level and low-level APIs for parsing BibTeX files.
//! Most users should use the high-level `Database` API, but the low-level API is available
//! for advanced use cases that require access to raw parsed items.
//!
//! # Low-level API Example
//!
//! ```
//! use bibtex_parser::parser::{parse_bibtex, ParsedItem};
//!
//! let input = r#"
//!     @string{ieee = "IEEE"}
//!     @preamble{"Test preamble"}
//!     % Line comment
//!     @article{test2024,
//!         author = "John Doe",
//!         title = ieee # " Article",
//!         year = 2024
//!     }
//! "#;
//!
//! let items = parse_bibtex(input)?;
//!
//! for item in items {
//!     match item {
//!         ParsedItem::Entry(entry) => {
//!             println!("Found entry: {}", entry.key());
//!             // Variables are not expanded yet - title contains reference to 'ieee'
//!         },
//!         ParsedItem::String(name, value) => {
//!             println!("String definition: {} = {:?}", name, value);
//!         },
//!         ParsedItem::Preamble(value) => {
//!             println!("Preamble: {:?}", value);
//!         },
//!         ParsedItem::Comment(text) => {
//!             println!("Comment: {}", text.trim());
//!         },
//!     }
//! }
//! # Ok::<(), bibtex_parser::Error>(())
//! ```

pub mod delimiter;
pub mod entry;
pub mod lexer;
pub mod simd;
pub mod utils;
pub mod value;

use crate::{Error, Result};
use winnow::ascii::multispace0;
use winnow::prelude::*;

pub use entry::parse_entry;

/// Internal parser result type
pub type PResult<'a, O> = winnow::PResult<O, winnow::error::ContextError>;

/// Parse a BibTeX file into raw items without expansion or processing
///
/// This is a low-level API that returns the raw parsed items before
/// string variable expansion or other processing. Most users should
/// use `Database::parse()` instead.
///
/// The returned items preserve the original structure:
/// - String variables are not expanded
/// - Concatenations are preserved as `Value::Concat`
/// - Comments are included (both `%` line comments and `@comment{}`)
/// - All items are returned in parse order
///
/// # Performance
///
/// This function maintains the same high performance as the high-level API
/// (650-700 MB/s) since it's the same underlying parser with no additional
/// processing overhead.
///
/// # Example
///
/// ```
/// use bibtex_parser::parser::{parse_bibtex, ParsedItem};
/// use bibtex_parser::Value;
///
/// let input = r#"
///     @string{name = "John Doe"}
///     @article{test,
///         author = name,
///         title = "Part 1" # " and " # "Part 2"
///     }
/// "#;
///
/// let items = parse_bibtex(input)?;
///
/// // Find the entry
/// let entry = items.iter().find_map(|item| {
///     if let ParsedItem::Entry(e) = item { Some(e) } else { None }
/// }).unwrap();
///
/// // Author field contains unexpanded variable reference
/// let author_field = entry.fields.iter()
///     .find(|f| f.name == "author").unwrap();
/// match &author_field.value {
///     Value::Variable(var) => println!("Variable reference: {}", var),
///     _ => {}
/// }
///
/// // Title field contains concatenation structure
/// let title_field = entry.fields.iter()
///     .find(|f| f.name == "title").unwrap();
/// match &title_field.value {
///     Value::Concat(parts) => println!("Concatenation with {} parts", parts.len()),
///     _ => {}
/// }
/// # Ok::<(), bibtex_parser::Error>(())
/// ```
pub fn parse_bibtex(input: &str) -> Result<Vec<ParsedItem>> {
    let mut items = Vec::new();
    parse_bibtex_stream(input, |item| {
        items.push(item);
        Ok(())
    })?;
    Ok(items)
}

/// Parse a BibTeX file and stream raw items to a callback.
///
/// This avoids allocating an intermediate `Vec<ParsedItem>` when the caller
/// can process items incrementally.
pub(crate) fn parse_bibtex_stream<'a, F>(input: &'a str, mut on_item: F) -> Result<()>
where
    F: FnMut(ParsedItem<'a>) -> Result<()>,
{
    let mut remaining = input;

    loop {
        // Skip ASCII whitespace without Unicode trimming overhead.
        lexer::skip_whitespace(&mut remaining);
        if remaining.is_empty() {
            break;
        }

        // Try to parse an item (including comments)
        match parse_item(&mut remaining) {
            Ok(item) => on_item(item)?,
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

    Ok(())
}

/// A raw parsed item from a BibTeX file before processing
///
/// This represents the different types of items that can appear in a BibTeX file,
/// returned by the low-level `parse_bibtex()` function. These items are in their
/// raw parsed form:
///
/// - String variables are not yet expanded
/// - Field values preserve concatenation structure
/// - Comments are preserved exactly as found
/// - All items maintain their original order
///
/// # Examples
///
/// ```
/// use bibtex_parser::parser::{parse_bibtex, ParsedItem};
///
/// let input = "@string{name = \"John\"}\n@article{key, author = name}";
/// let items = parse_bibtex(input)?;
///
/// match &items[0] {
///     ParsedItem::String(var_name, value) => {
///         println!("String variable: {} = {:?}", var_name, value);
///     },
///     _ => {}
/// }
///
/// match &items[1] {
///     ParsedItem::Entry(entry) => {
///         // The author field contains a variable reference, not the expanded value
///         println!("Entry key: {}", entry.key());
///     },
///     _ => {}
/// }
/// # Ok::<(), bibtex_parser::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedItem<'a> {
    /// A bibliography entry (article, book, inproceedings, etc.)
    ///
    /// Contains the entry in its raw parsed form with field values that may
    /// reference string variables or contain concatenations.
    Entry(crate::Entry<'a>),

    /// A string definition (`@string{name = value}`)
    ///
    /// Contains the variable name and its value. The value itself may contain
    /// references to other string variables or concatenations.
    String(&'a str, crate::Value<'a>),

    /// A preamble (`@preamble{value}`)
    ///
    /// Contains the preamble value, which may reference string variables
    /// or contain concatenations.
    Preamble(crate::Value<'a>),

    /// A comment (both `% line comment` and `@comment{...}`)
    ///
    /// Contains the raw comment text exactly as it appears in the source,
    /// including any whitespace and formatting.
    Comment(&'a str),
}

/// Parse a single item (entry, string, preamble, or comment) with optimized delimiter search
fn parse_item<'a>(input: &mut &'a str) -> PResult<'a, ParsedItem<'a>> {
    // Use optimized delimiter search to find @ or handle as comment
    let bytes = input.as_bytes();

    // Fast path: if we don't start with @, check if this is a comment
    if !bytes.is_empty() && bytes[0] != b'@' {
        // Look for the next @ to treat everything before it as a comment
        if let Some(at_pos) = delimiter::find_byte(bytes, b'@', 0) {
            let comment = &input[..at_pos];
            *input = &input[at_pos..];
            return Ok(ParsedItem::Comment(comment));
        }
        // No @ found, entire remaining input is a comment
        let comment = *input;
        *input = "";
        return Ok(ParsedItem::Comment(comment));
    }

    // We have an @ at the start. For regular entries, avoid testing all special
    // keywords and dispatch directly based on the first letter.
    let second = bytes.get(1).copied().unwrap_or_default();
    match ascii_lower(second) {
        b's' if starts_with_keyword(bytes, b"string") => {
            parse_string(input).map(|(k, v)| ParsedItem::String(k, v))
        }
        b'p' if starts_with_keyword(bytes, b"preamble") => parse_preamble(input).map(ParsedItem::Preamble),
        b'c' if starts_with_keyword(bytes, b"comment") => parse_comment(input).map(ParsedItem::Comment),
        _ => entry::parse_entry_at(input).map(ParsedItem::Entry),
    }
}

#[inline]
fn starts_with_keyword(input: &[u8], keyword: &[u8]) -> bool {
    if input.first() != Some(&b'@') || input.len() < keyword.len() + 1 {
        return false;
    }

    for (offset, &expected) in keyword.iter().enumerate() {
        if ascii_lower(input[offset + 1]) != expected {
            return false;
        }
    }

    if input.len() == keyword.len() + 1 {
        return true;
    }

    !is_identifier_char(input[keyword.len() + 1])
}

#[inline]
const fn ascii_lower(byte: u8) -> u8 {
    if b'A' <= byte && byte <= b'Z' {
        byte + (b'a' - b'A')
    } else {
        byte
    }
}

#[inline]
const fn is_identifier_char(byte: u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-' | b':' | b'.'
    )
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
                delimited('(', lexer::balanced_parentheses, ')'),
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
