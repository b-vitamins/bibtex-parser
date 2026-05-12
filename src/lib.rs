#![deny(clippy::all)]
//! # bibtex-parser
//!
//! Fast BibTeX parsing with a Rust-first [`Library`] API.
//!
//! `bibtex-parser` is built for applications that need both throughput and a
//! practical user-facing API: strict parsing by default, explicit tolerant
//! recovery when a corpus is messy, string and month expansion, comments and
//! preambles, validation, query/edit helpers, and configurable writing.
//!
//! ## Features
//!
//! - Borrowed values where possible for low-allocation parsing.
//! - String variables, concatenation, and standard month constants.
//! - Entries, strings, preambles, comments, and tolerant failures in source order.
//! - Opt-in source-span capture.
//! - DOI normalization, duplicate detection, validation, sorting, and field normalization.
//! - Configurable writer for formatting and file output.
//! - Optional `parallel` feature for parsing multiple files concurrently.
//! - Optional `latex_to_unicode` feature for LaTeX accent conversion helpers.
//!
//! ## Parse
//!
//! ```
//! use bibtex_parser::Library;
//!
//! let input = r#"
//!     @string{venue = "VLDB"}
//!     @article{paper,
//!         author = "Jane Doe and John Smith",
//!         title = "Fast BibTeX",
//!         journal = venue,
//!         year = 2026
//!     }
//! "#;
//!
//! let library = Library::parse(input)?;
//! let entry = library.find_by_key("paper").unwrap();
//!
//! assert_eq!(entry.get("journal"), Some("VLDB"));
//! assert_eq!(entry.year(), Some("2026".to_string()));
//! assert_eq!(entry.authors().len(), 2);
//! # Ok::<(), bibtex_parser::Error>(())
//! ```
//!
//! ## Tolerant Recovery
//!
//! ```
//! use bibtex_parser::{Block, Library};
//!
//! let library = Library::parser()
//!     .tolerant()
//!     .capture_source()
//!     .parse(r#"
//!         @article{ok, title = "Good"}
//!         @article{bad, title = "Missing close"
//!         @book{recovered, title = "Recovered"}
//!     "#)?;
//!
//! assert_eq!(library.entries().len(), 2);
//! assert_eq!(library.failed_blocks().len(), 1);
//!
//! let has_failure_span = library.blocks().iter().any(|block| {
//!     matches!(block, Block::Failed(failed) if failed.source.is_some())
//! });
//! assert!(has_failure_span);
//! # Ok::<(), bibtex_parser::Error>(())
//! ```
//!
//! ## Write
//!
//! ```
//! use bibtex_parser::{Library, Writer, WriterConfig};
//!
//! let library = Library::parse(r#"@article{paper, title = "Fast BibTeX"}"#)?;
//! let mut output = Vec::new();
//! let config = WriterConfig {
//!     align_values: true,
//!     ..Default::default()
//! };
//!
//! Writer::with_config(&mut output, config).write_library(&library)?;
//! assert!(String::from_utf8(output).unwrap().contains("@article{paper"));
//! # Ok::<(), bibtex_parser::Error>(())
//! ```
//!
//! ## `Library` Versus `ParsedDocument`
//!
//! Use [`Library`] when application code wants structured bibliography data.
//! Use [`ParsedDocument`] when tooling needs source-order blocks, diagnostics,
//! partial results, or source-preserving metadata.
//!
//! ```
//! use bibtex_parser::{ParsedBlock, Parser};
//!
//! let input = r#"
//!     % retained comment
//!     @article{paper, title = "Fast BibTeX"}
//! "#;
//!
//! let document = Parser::new()
//!     .capture_source()
//!     .parse_document(input)?;
//!
//! assert_eq!(document.library().entries().len(), 1);
//! assert_eq!(document.entries()[0].key(), "paper");
//! assert!(matches!(document.blocks()[0], ParsedBlock::Comment(0)));
//! assert!(document.entries()[0].source.is_some());
//! # Ok::<(), bibtex_parser::Error>(())
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
    clippy::missing_panics_doc,
    clippy::multiple_crate_versions
)]

pub mod document;
pub mod error;
pub mod model;
pub mod parser;
pub mod source;

#[cfg(feature = "latex_to_unicode")]
pub mod latex_unicode;

mod database;
mod writer;

pub use database::{
    Block, Comment, FailedBlock, FieldNameCase, FieldNormalizeOptions, IssueSummary, Library,
    LibraryBuilder, LibraryStats, MonthStyle, Parser, Preamble, SortOptions, StringDefinition,
    ValidationReport,
};
pub use document::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, DiagnosticTarget, EntryDelimiter, ParseStatus,
    ParseSummary, ParsedBlock, ParsedComment, ParsedDocument, ParsedEntry, ParsedEntryStatus,
    ParsedFailedBlock, ParsedField, ParsedPreamble, ParsedSource, ParsedString, ParsedValue,
    ValueDelimiter,
};
pub use error::{Error, Result, SourceId, SourceSpan};
pub use model::{
    normalize_doi, parse_names, Entry, EntryType, Field, PersonName, ValidationError,
    ValidationLevel, ValidationSeverity, Value,
};
pub use parser::{parse_bibtex, ParsedItem};
pub use source::SourceMap;
pub use writer::{to_file, to_string, Writer, WriterConfig};

/// Re-export of common parser functions
pub mod prelude {
    pub use crate::{
        normalize_doi, parse_bibtex, parse_names, Block, Comment, Diagnostic, DiagnosticCode,
        DiagnosticSeverity, DiagnosticTarget, Entry, EntryDelimiter, EntryType, Error, FailedBlock,
        Field, FieldNameCase, FieldNormalizeOptions, IssueSummary, Library, LibraryBuilder,
        LibraryStats, MonthStyle, ParseStatus, ParseSummary, ParsedBlock, ParsedComment,
        ParsedDocument, ParsedEntry, ParsedEntryStatus, ParsedFailedBlock, ParsedField, ParsedItem,
        ParsedPreamble, ParsedSource, ParsedString, ParsedValue, Parser, PersonName, Preamble,
        Result, SortOptions, SourceId, SourceMap, SourceSpan, StringDefinition, ValidationError,
        ValidationLevel, ValidationReport, ValidationSeverity, Value, ValueDelimiter, Writer,
        WriterConfig,
    };
}

/// Parse a BibTeX library from a string.
pub fn parse(input: &str) -> Result<Library<'_>> {
    Library::parser().parse(input)
}

/// Parse a BibTeX library from a file.
pub fn parse_file(path: impl AsRef<std::path::Path>) -> Result<Library<'static>> {
    let content = std::fs::read_to_string(path)?;
    parse(&content).map(Library::into_owned)
}
