//! # bibtex-parser
//!
//! A fast, modern BibTeX parser with excellent error handling and zero-copy parsing.
//!
//! ## Features
//!
//! - Zero-copy parsing for optimal performance
//! - Excellent error messages with source locations
//! - Support for all standard BibTeX entry types
//! - String variable expansion
//! - Comment preservation
//! - Streaming support for large files
//!
//! ## Example
//!
//! ```
//! use bibtex_parser::{Database, Entry};
//!
//! let input = r#"
//!     @article{einstein1905,
//!         author = "Albert Einstein",
//!         title = "Zur Elektrodynamik bewegter Körper",
//!         journal = "Annalen der Physik",
//!         year = 1905
//!     }
//! "#;
//!
//! let db = Database::parse(input)?;
//! assert_eq!(db.entries().len(), 1);
//!
//! let entry = &db.entries()[0];
//! assert_eq!(entry.key(), "einstein1905");
//! assert_eq!(entry.get("author"), Some("Albert Einstein"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    missing_docs,
    missing_debug_implementations
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

pub mod error;
pub mod model;
pub mod parser;

mod database;
mod writer;

pub use database::{Database, DatabaseBuilder};
pub use error::{Error, Result};
pub use model::{Entry, EntryType, Field, Value};
pub use writer::{to_file, to_string, Writer};

/// Re-export of common parser functions
pub mod prelude {
    pub use crate::{Database, DatabaseBuilder, Entry, EntryType, Error, Result, Value};
}

/// Parse a BibTeX database from a string
pub fn parse(input: &str) -> Result<Database> {
    Database::parse(input)
}

/// Parse a BibTeX database from a file
pub fn parse_file(path: impl AsRef<std::path::Path>) -> Result<Database<'static>> {
    let content = std::fs::read_to_string(path)?;
    parse(&content).map(database::Database::into_owned)
}
