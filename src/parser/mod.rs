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

pub use entry::parse_entry;

/// Internal parser result type
pub type PResult<'a, O> = winnow::PResult<O, winnow::error::ContextError>;

/// Cursor over the original input used by the streaming parser.
///
/// Keeping a byte index lets the top-level parser avoid repeatedly rebuilding
/// `&str` state for every item and makes manual special-form parsing cheap.
pub(super) struct Cursor<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    #[inline]
    pub(super) const fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    #[inline]
    pub(super) fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    #[inline]
    pub(super) fn remaining_bytes(&self) -> &'a [u8] {
        &self.input.as_bytes()[self.pos..]
    }

    #[inline]
    pub(super) fn skip_whitespace(&mut self) {
        self.pos += simd::scan_whitespace(self.remaining_bytes());
    }

    #[inline]
    pub(super) const fn is_empty(&self) -> bool {
        self.pos >= self.input.len()
    }

    #[inline]
    pub(super) fn bump(&mut self, len: usize) {
        self.pos += len;
    }

    #[inline]
    pub(super) fn take_identifier(&mut self) -> Option<&'a str> {
        let len = simd::scan_identifier(self.remaining_bytes());
        if len == 0 {
            return None;
        }

        let ident = &self.remaining()[..len];
        self.bump(len);
        Some(ident)
    }

    #[inline]
    fn take_comment_until_at(&mut self) -> &'a str {
        let start = self.pos;
        if let Some(offset) = delimiter::find_byte(self.remaining_bytes(), b'@', 0) {
            self.pos += offset;
        } else {
            self.pos = self.input.len();
        }
        &self.input[start..self.pos]
    }
}

#[inline]
fn backtrack<'a, O>() -> PResult<'a, O> {
    Err(winnow::error::ErrMode::Backtrack(
        winnow::error::ContextError::default(),
    ))
}

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
pub fn parse_bibtex(input: &str) -> Result<Vec<ParsedItem<'_>>> {
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
    let mut cursor = Cursor::new(input);

    loop {
        cursor.skip_whitespace();
        if cursor.is_empty() {
            break;
        }

        // Try to parse an item (including comments)
        match parse_item(&mut cursor) {
            Ok(item) => on_item(item)?,
            Err(e) => {
                let (line, column) = calculate_position(input, cursor.pos);

                return Err(Error::ParseError {
                    line,
                    column,
                    message: format!("Failed to parse entry: {e}"),
                    snippet: Some(get_snippet(cursor.remaining(), 40)),
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
fn parse_item<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, ParsedItem<'a>> {
    let bytes = cursor.remaining_bytes();

    if bytes.first() != Some(&b'@') {
        return Ok(ParsedItem::Comment(cursor.take_comment_until_at()));
    }

    let second = bytes.get(1).copied().unwrap_or_default();
    match ascii_lower(second) {
        b's' if starts_with_keyword(bytes, b"string") => {
            parse_string(cursor).map(|(k, v)| ParsedItem::String(k, v))
        }
        b'p' if starts_with_keyword(bytes, b"preamble") => {
            parse_preamble(cursor).map(ParsedItem::Preamble)
        }
        b'c' if starts_with_keyword(bytes, b"comment") => {
            parse_comment(cursor).map(ParsedItem::Comment)
        }
        _ => entry::parse_entry_fast(cursor).map(ParsedItem::Entry),
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

/// Parse a `@string` definition.
fn parse_string<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, (&'a str, crate::Value<'a>)> {
    cursor.bump(1 + "string".len());
    cursor.skip_whitespace();

    let closing = match cursor.remaining_bytes().first() {
        Some(b'{') => b'}',
        Some(b'(') => b')',
        _ => return backtrack(),
    };
    cursor.bump(1);

    let start = cursor.remaining();
    let mut remaining = start;
    lexer::skip_whitespace(&mut remaining);
    let name = lexer::identifier(&mut remaining)?;
    lexer::skip_whitespace(&mut remaining);

    match remaining.as_bytes().first() {
        Some(b'=') => remaining = &remaining[1..],
        _ => return backtrack(),
    }

    lexer::skip_whitespace(&mut remaining);
    let value = value::parse_value(&mut remaining)?;
    lexer::skip_whitespace(&mut remaining);

    match remaining.as_bytes().first() {
        Some(&byte) if byte == closing => remaining = &remaining[1..],
        _ => return backtrack(),
    }

    cursor.bump(start.len() - remaining.len());
    Ok((name, value))
}

/// Parse a `@preamble`.
fn parse_preamble<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, crate::Value<'a>> {
    cursor.bump(1 + "preamble".len());
    cursor.skip_whitespace();

    let closing = match cursor.remaining_bytes().first() {
        Some(b'{') => b'}',
        Some(b'(') => b')',
        _ => return backtrack(),
    };
    cursor.bump(1);

    let start = cursor.remaining();
    let mut remaining = start;
    lexer::skip_whitespace(&mut remaining);
    let value = value::parse_value(&mut remaining)?;
    lexer::skip_whitespace(&mut remaining);

    match remaining.as_bytes().first() {
        Some(&byte) if byte == closing => remaining = &remaining[1..],
        _ => return backtrack(),
    }

    cursor.bump(start.len() - remaining.len());
    Ok(value)
}

/// Parse an `@comment{...}` or `@comment(...)`.
fn parse_comment<'a>(cursor: &mut Cursor<'a>) -> PResult<'a, &'a str> {
    cursor.bump(1 + "comment".len());
    cursor.skip_whitespace();

    let closing = match cursor.remaining_bytes().first() {
        Some(b'{') => b'}',
        Some(b'(') => b')',
        _ => return backtrack(),
    };
    cursor.bump(1);

    let start = cursor.remaining();
    let mut remaining = start;
    let text = match closing {
        b'}' => lexer::balanced_braces(&mut remaining)?,
        b')' => lexer::balanced_parentheses(&mut remaining)?,
        _ => unreachable!(),
    };

    match remaining.as_bytes().first() {
        Some(&byte) if byte == closing => remaining = &remaining[1..],
        _ => return backtrack(),
    }

    cursor.bump(start.len() - remaining.len());
    Ok(text)
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
