//! Tooling-oriented parsed bibliography model.
//!
//! [`Library`](crate::Library) remains the compact, ergonomic API for normal
//! bibliography work. [`ParsedDocument`] is the richer model for tools that
//! need source-order blocks, per-item metadata, retained raw text, diagnostics,
//! or partial parse results.

use crate::database::BlockKind;
use crate::{
    Comment, Entry, EntryType, FailedBlock, Field, Library, Preamble, SourceSpan, StringDefinition,
    Value,
};
use std::borrow::Cow;
use std::fmt;

/// Parse status for a parsed bibliography document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseStatus {
    /// The document parsed without diagnostics that affect recovered content.
    Ok,
    /// The document contains useful parsed data plus recovered or failed blocks.
    Partial,
    /// The document could not produce meaningful bibliography data.
    Failed,
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    /// A problem that prevents some requested parse result from being valid.
    Error,
    /// A recoverable problem that callers may want to show or test.
    Warning,
    /// Additional parse information that is not itself a problem.
    Info,
}

/// Stable machine-readable diagnostic code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiagnosticCode(Cow<'static, str>);

impl DiagnosticCode {
    /// Generic parse error code used before finer-grained recovery classifies a failure.
    pub const PARSE_ERROR: Self = Self(Cow::Borrowed("parse-error"));

    /// Create a borrowed static diagnostic code.
    #[must_use]
    pub const fn borrowed(code: &'static str) -> Self {
        Self(Cow::Borrowed(code))
    }

    /// Create an owned diagnostic code.
    #[must_use]
    pub fn custom(code: impl Into<String>) -> Self {
        Self(Cow::Owned(code.into()))
    }

    /// Return the diagnostic code as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Location target for a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticTarget {
    /// The whole input file or source.
    File,
    /// A source-order block by index.
    Block(usize),
    /// An entry by parsed-entry index.
    Entry(usize),
    /// A field by parsed-entry and field index.
    Field {
        /// Parsed-entry index.
        entry: usize,
        /// Field index inside the parsed entry.
        field: usize,
    },
    /// A value by parsed-entry and field index.
    Value {
        /// Parsed-entry index.
        entry: usize,
        /// Field index inside the parsed entry.
        field: usize,
    },
    /// A failed block by failed-block index.
    FailedBlock(usize),
}

/// Structured diagnostic emitted while building a parsed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Diagnostic severity.
    pub severity: DiagnosticSeverity,
    /// Stable machine-readable code.
    pub code: DiagnosticCode,
    /// Human-readable message.
    pub message: String,
    /// Bibliography object targeted by this diagnostic.
    pub target: DiagnosticTarget,
    /// Source location, when available.
    pub source: Option<SourceSpan>,
}

impl Diagnostic {
    /// Create an error diagnostic.
    #[must_use]
    pub fn error(
        code: DiagnosticCode,
        message: impl Into<String>,
        target: DiagnosticTarget,
        source: Option<SourceSpan>,
    ) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code,
            message: message.into(),
            target,
            source,
        }
    }
}

/// Source metadata associated with a parsed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSource<'a> {
    /// Source index inside the document.
    pub id: usize,
    /// Human-readable source name or path, when known.
    pub name: Option<Cow<'a, str>>,
}

impl ParsedSource<'_> {
    /// Return `true` when this source has no caller-provided name.
    #[must_use]
    pub const fn is_anonymous(&self) -> bool {
        self.name.is_none()
    }
}

/// Source-order block in a parsed document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedBlock {
    /// A regular or partial bibliography entry by parsed-entry index.
    Entry(usize),
    /// A string definition by parsed-string index.
    String(usize),
    /// A preamble by parsed-preamble index.
    Preamble(usize),
    /// A comment by parsed-comment index.
    Comment(usize),
    /// A failed block by failed-block index.
    Failed(usize),
}

/// Status of a parsed entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedEntryStatus {
    /// Entry parsed completely.
    Complete,
    /// Entry has a recovered type or key plus at least some usable content.
    Partial,
}

/// Parsed BibTeX value plus optional source-preserving metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedValue<'a> {
    /// Structured value.
    pub value: Value<'a>,
    /// Exact raw value text, when retained by the parser mode.
    pub raw: Option<Cow<'a, str>>,
    /// Source location for the value, when available.
    pub source: Option<SourceSpan>,
    /// Expanded text projection, when a parser mode computes it separately.
    pub expanded: Option<Cow<'a, str>>,
}

impl<'a> ParsedValue<'a> {
    /// Create parsed-value metadata from a structured value.
    #[must_use]
    pub const fn new(value: Value<'a>) -> Self {
        Self {
            value,
            raw: None,
            source: None,
            expanded: None,
        }
    }

    /// Convert this parsed value into the structured value.
    #[must_use]
    pub fn into_value(self) -> Value<'a> {
        self.value
    }
}

/// Parsed field plus optional source-preserving metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedField<'a> {
    /// Field name as it appeared after parsing.
    pub name: Cow<'a, str>,
    /// Parsed field value.
    pub value: ParsedValue<'a>,
    /// Exact raw field text, when retained by the parser mode.
    pub raw: Option<Cow<'a, str>>,
    /// Source location for the whole field, when available.
    pub source: Option<SourceSpan>,
    /// Source location for the field name, when available.
    pub name_source: Option<SourceSpan>,
    /// Source location for the field value, when available.
    pub value_source: Option<SourceSpan>,
}

impl<'a> ParsedField<'a> {
    /// Create parsed-field metadata from a structured field.
    #[must_use]
    pub fn from_field(field: Field<'a>) -> Self {
        Self {
            name: field.name,
            value: ParsedValue::new(field.value),
            raw: None,
            source: None,
            name_source: None,
            value_source: None,
        }
    }

    /// Convert this parsed field into the structured field.
    #[must_use]
    pub fn into_field(self) -> Field<'a> {
        Field {
            name: self.name,
            value: self.value.into_value(),
        }
    }
}

/// Parsed entry plus optional source-preserving metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEntry<'a> {
    /// Entry type.
    pub ty: EntryType<'a>,
    /// Citation key.
    pub key: Cow<'a, str>,
    /// Parsed fields in source order.
    pub fields: Vec<ParsedField<'a>>,
    /// Whether the entry is complete or recovered.
    pub status: ParsedEntryStatus,
    /// Source location for the whole entry, when available.
    pub source: Option<SourceSpan>,
    /// Exact raw entry text, when retained by the parser mode.
    pub raw: Option<Cow<'a, str>>,
    /// Diagnostics attached to this entry.
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> ParsedEntry<'a> {
    /// Create parsed-entry metadata from a structured entry.
    #[must_use]
    pub fn from_entry(entry: Entry<'a>, source: Option<SourceSpan>) -> Self {
        Self {
            ty: entry.ty,
            key: entry.key,
            fields: entry
                .fields
                .into_iter()
                .map(ParsedField::from_field)
                .collect(),
            status: ParsedEntryStatus::Complete,
            source,
            raw: None,
            diagnostics: Vec::new(),
        }
    }

    /// Return the citation key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Convert this parsed entry into the structured entry.
    #[must_use]
    pub fn into_entry(self) -> Entry<'a> {
        Entry {
            ty: self.ty,
            key: self.key,
            fields: self
                .fields
                .into_iter()
                .map(ParsedField::into_field)
                .collect(),
        }
    }
}

/// Parsed string definition plus optional source-preserving metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedString<'a> {
    /// String variable name.
    pub name: Cow<'a, str>,
    /// Parsed string value.
    pub value: ParsedValue<'a>,
    /// Source location for the definition, when available.
    pub source: Option<SourceSpan>,
    /// Exact raw string-definition text, when retained by the parser mode.
    pub raw: Option<Cow<'a, str>>,
}

impl<'a> ParsedString<'a> {
    /// Create parsed-string metadata from a structured string definition.
    #[must_use]
    pub fn from_definition(definition: StringDefinition<'a>) -> Self {
        Self {
            name: definition.name,
            value: ParsedValue::new(definition.value),
            source: definition.source,
            raw: None,
        }
    }
}

/// Parsed preamble plus optional source-preserving metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPreamble<'a> {
    /// Parsed preamble value.
    pub value: ParsedValue<'a>,
    /// Source location for the preamble, when available.
    pub source: Option<SourceSpan>,
    /// Exact raw preamble text, when retained by the parser mode.
    pub raw: Option<Cow<'a, str>>,
}

impl<'a> ParsedPreamble<'a> {
    /// Create parsed-preamble metadata from a structured preamble.
    #[must_use]
    pub fn from_preamble(preamble: Preamble<'a>) -> Self {
        Self {
            value: ParsedValue::new(preamble.value),
            source: preamble.source,
            raw: None,
        }
    }
}

/// Parsed comment plus optional source-preserving metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedComment<'a> {
    /// Comment text.
    pub text: Cow<'a, str>,
    /// Source location for the comment, when available.
    pub source: Option<SourceSpan>,
    /// Exact raw comment text, when retained by the parser mode.
    pub raw: Option<Cow<'a, str>>,
}

impl<'a> ParsedComment<'a> {
    /// Create parsed-comment metadata from a structured comment.
    #[must_use]
    pub fn from_comment(comment: Comment<'a>) -> Self {
        Self {
            text: comment.text,
            source: comment.source,
            raw: None,
        }
    }
}

/// Failed block retained by a tolerant parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFailedBlock<'a> {
    /// Raw source text for the failed block.
    pub raw: Cow<'a, str>,
    /// Human-readable parse error.
    pub error: String,
    /// Source location for the failed block, when available.
    pub source: Option<SourceSpan>,
    /// Diagnostics attached to this failed block.
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> ParsedFailedBlock<'a> {
    /// Create failed-block metadata from a retained failed block.
    #[must_use]
    pub fn from_failed_block(index: usize, failed: FailedBlock<'a>) -> Self {
        let diagnostic = Diagnostic::error(
            DiagnosticCode::PARSE_ERROR,
            failed.error.clone(),
            DiagnosticTarget::FailedBlock(index),
            failed.source,
        );

        Self {
            raw: failed.raw,
            error: failed.error,
            source: failed.source,
            diagnostics: vec![diagnostic],
        }
    }
}

/// Rich parsed document for tooling-grade bibliography workflows.
#[derive(Debug, Clone)]
pub struct ParsedDocument<'a> {
    library: Library<'a>,
    sources: Vec<ParsedSource<'a>>,
    entries: Vec<ParsedEntry<'a>>,
    strings: Vec<ParsedString<'a>>,
    preambles: Vec<ParsedPreamble<'a>>,
    comments: Vec<ParsedComment<'a>>,
    failed_blocks: Vec<ParsedFailedBlock<'a>>,
    blocks: Vec<ParsedBlock>,
    diagnostics: Vec<Diagnostic>,
    status: ParseStatus,
}

impl<'a> ParsedDocument<'a> {
    /// Build a parsed document from the existing structured library model.
    #[must_use]
    pub(crate) fn from_library(library: Library<'a>) -> Self {
        let sources = vec![ParsedSource { id: 0, name: None }];
        let entries: Vec<ParsedEntry<'a>> = library
            .entries()
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, entry)| ParsedEntry::from_entry(entry, library.entry_source(index)))
            .collect();
        let strings: Vec<ParsedString<'a>> = library
            .strings()
            .iter()
            .cloned()
            .map(ParsedString::from_definition)
            .collect();
        let preambles: Vec<ParsedPreamble<'a>> = library
            .preambles()
            .iter()
            .cloned()
            .map(ParsedPreamble::from_preamble)
            .collect();
        let comments = library
            .comments()
            .iter()
            .cloned()
            .map(ParsedComment::from_comment)
            .collect();
        let failed_blocks = library
            .failed_blocks()
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, failed)| ParsedFailedBlock::from_failed_block(index, failed))
            .collect::<Vec<_>>();
        let diagnostics = failed_blocks
            .iter()
            .flat_map(|failed| failed.diagnostics.iter().cloned())
            .collect::<Vec<_>>();
        let blocks = library
            .block_kinds()
            .iter()
            .map(|kind| match *kind {
                BlockKind::Entry(index) => ParsedBlock::Entry(index),
                BlockKind::String(index) => ParsedBlock::String(index),
                BlockKind::Preamble(index) => ParsedBlock::Preamble(index),
                BlockKind::Comment(index) => ParsedBlock::Comment(index),
                BlockKind::Failed(index) => ParsedBlock::Failed(index),
            })
            .collect();
        let status = if failed_blocks.is_empty() {
            ParseStatus::Ok
        } else if entries.is_empty() && strings.is_empty() && preambles.is_empty() {
            ParseStatus::Failed
        } else {
            ParseStatus::Partial
        };

        Self {
            library,
            sources,
            entries,
            strings,
            preambles,
            comments,
            failed_blocks,
            blocks,
            diagnostics,
            status,
        }
    }

    /// Return the compact structured library view.
    #[must_use]
    pub const fn library(&self) -> &Library<'a> {
        &self.library
    }

    /// Consume this document and return the compact structured library view.
    #[must_use]
    pub fn into_library(self) -> Library<'a> {
        self.library
    }

    /// Return source metadata.
    #[must_use]
    pub fn sources(&self) -> &[ParsedSource<'a>] {
        &self.sources
    }

    /// Return parsed entries.
    #[must_use]
    pub fn entries(&self) -> &[ParsedEntry<'a>] {
        &self.entries
    }

    /// Return parsed string definitions.
    #[must_use]
    pub fn strings(&self) -> &[ParsedString<'a>] {
        &self.strings
    }

    /// Return parsed preambles.
    #[must_use]
    pub fn preambles(&self) -> &[ParsedPreamble<'a>] {
        &self.preambles
    }

    /// Return parsed comments.
    #[must_use]
    pub fn comments(&self) -> &[ParsedComment<'a>] {
        &self.comments
    }

    /// Return failed blocks retained by tolerant parsing.
    #[must_use]
    pub fn failed_blocks(&self) -> &[ParsedFailedBlock<'a>] {
        &self.failed_blocks
    }

    /// Return source-order blocks.
    #[must_use]
    pub fn blocks(&self) -> &[ParsedBlock] {
        &self.blocks
    }

    /// Return document diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Return the document parse status.
    #[must_use]
    pub const fn status(&self) -> ParseStatus {
        self.status
    }
}
