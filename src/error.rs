//! Error types for the bibtex-parser crate

use std::fmt;
use thiserror::Error;

/// Result type for bibtex-parser operations
pub type Result<T> = std::result::Result<T, Error>;

/// The main error type for bibtex-parser
#[derive(Error, Debug)]
pub enum Error {
    /// Parse error with location information
    #[error("Parse error at line {line}, column {column}: {message}")]
    ParseError {
        /// Line number (1-indexed)
        line: usize,
        /// Column number (1-indexed)
        column: usize,
        /// Error message
        message: String,
        /// Optional source snippet
        snippet: Option<String>,
    },

    /// Undefined string variable
    #[error("Undefined string variable '{0}'")]
    UndefinedVariable(String),

    /// Circular reference in string variables
    #[error("Circular reference detected in string variables: {0}")]
    CircularReference(String),

    /// Invalid entry type
    #[error("Invalid entry type '{0}'")]
    InvalidEntryType(String),

    /// Missing required field
    #[error("Missing required field '{field}' in {entry_type} entry")]
    MissingRequiredField {
        /// The entry type
        entry_type: String,
        /// The missing field
        field: String,
    },

    /// Duplicate entry key
    #[error("Duplicate entry key '{0}'")]
    DuplicateKey(String),

    /// Invalid field name
    #[error("Invalid field name '{0}'")]
    InvalidFieldName(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic parse error from winnow
    #[error("Parse error: {0}")]
    WinnowError(String),
}

/// Parse context for better error messages
#[derive(Debug, Clone)]
pub struct ParseContext {
    /// The full input string being parsed
    pub input: String,
    /// Current line number (1-indexed)
    pub line: usize,
    /// Current column number (1-indexed)
    pub column: usize,
}

impl ParseContext {
    /// Create a new parse context
    #[must_use]
    pub fn new(input: &str) -> Self {
        Self {
            input: input.to_string(),
            line: 1,
            column: 1,
        }
    }

    /// Get a snippet of the input around the current position
    #[must_use]
    pub fn snippet(&self, pos: usize, context_size: usize) -> String {
        let start = pos.saturating_sub(context_size);
        let end = (pos + context_size).min(self.input.len());
        let snippet = &self.input[start..end];
        let relative_pos = pos - start;
        format!("{}\n{}^", snippet, " ".repeat(relative_pos))
    }

    /// Update position based on consumed input
    pub fn advance(&mut self, consumed: &str) {
        for ch in consumed.chars() {
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }
}

/// Convert winnow errors to our error type
impl From<winnow::error::ContextError> for Error {
    fn from(err: winnow::error::ContextError) -> Self {
        Self::WinnowError(err.to_string())
    }
}

/// Location information for errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: usize,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}
