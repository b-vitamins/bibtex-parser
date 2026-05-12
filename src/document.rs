//! Tooling-oriented parsed bibliography model.
//!
//! [`Library`](crate::Library) remains the compact, ergonomic API for normal
//! bibliography work. [`ParsedDocument`] is the richer model for tools that
//! need source-order blocks, per-item metadata, retained raw text, diagnostics,
//! or partial parse results.

use crate::database::BlockKind;
use crate::database::RawBuildItem;
use crate::{
    Comment, Entry, EntryType, FailedBlock, Field, Library, Preamble, SourceId, SourceMap,
    SourceSpan, StringDefinition, Value,
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
///
/// The initial parser diagnostic codes are:
/// `missing-entry-key`, `missing-field-separator`, `expected-field-name`,
/// `empty-field-value`, `expected-value-atom`, `bad-field-boundary`,
/// `bad-value-boundary`, `unclosed-entry`, `unclosed-braced-value`, and
/// `unclosed-quoted-value`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiagnosticCode(Cow<'static, str>);

impl DiagnosticCode {
    /// Generic parse error code used before finer-grained recovery classifies a failure.
    pub const PARSE_ERROR: Self = Self(Cow::Borrowed("parse-error"));
    /// Entry body did not contain a citation key.
    pub const MISSING_ENTRY_KEY: Self = Self(Cow::Borrowed("missing-entry-key"));
    /// Expected a comma after an entry key or `=` after a field name.
    pub const MISSING_FIELD_SEPARATOR: Self = Self(Cow::Borrowed("missing-field-separator"));
    /// Expected a field name inside an entry body.
    pub const EXPECTED_FIELD_NAME: Self = Self(Cow::Borrowed("expected-field-name"));
    /// Field separator was present but no value was provided.
    pub const EMPTY_FIELD_VALUE: Self = Self(Cow::Borrowed("empty-field-value"));
    /// Expected a literal, number, variable, quoted value, or braced value.
    pub const EXPECTED_VALUE_ATOM: Self = Self(Cow::Borrowed("expected-value-atom"));
    /// Expected a comma or entry close after a field value.
    pub const BAD_FIELD_BOUNDARY: Self = Self(Cow::Borrowed("bad-field-boundary"));
    /// Expected a value atom after a concatenation operator.
    pub const BAD_VALUE_BOUNDARY: Self = Self(Cow::Borrowed("bad-value-boundary"));
    /// Entry ended before its closing delimiter was found.
    pub const UNCLOSED_ENTRY: Self = Self(Cow::Borrowed("unclosed-entry"));
    /// Braced field value ended before its closing brace was found.
    pub const UNCLOSED_BRACED_VALUE: Self = Self(Cow::Borrowed("unclosed-braced-value"));
    /// Quoted field value ended before its closing quote was found.
    pub const UNCLOSED_QUOTED_VALUE: Self = Self(Cow::Borrowed("unclosed-quoted-value"));

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
    /// Short source context suitable for display, when available.
    pub snippet: Option<String>,
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
            snippet: None,
        }
    }

    /// Attach source context to this diagnostic.
    #[must_use]
    pub fn with_snippet(mut self, snippet: impl Into<String>) -> Self {
        self.snippet = Some(snippet.into());
        self
    }
}

/// Summary counts for a parsed document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseSummary {
    /// File-level parse status.
    pub status: ParseStatus,
    /// Number of parsed entries.
    pub entries: usize,
    /// Number of warning diagnostics.
    pub warnings: usize,
    /// Number of error diagnostics.
    pub errors: usize,
    /// Number of informational diagnostics.
    pub infos: usize,
    /// Number of failed blocks.
    pub failed_blocks: usize,
    /// Number of entries recovered as partial entries.
    pub recovered_blocks: usize,
}

/// Source metadata associated with a parsed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSource<'a> {
    /// Source index inside the document.
    pub id: SourceId,
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

/// Delimiter used by a BibTeX entry body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryDelimiter {
    /// Entry used `{ ... }`.
    Braces,
    /// Entry used `( ... )`.
    Parentheses,
}

/// Delimiter or source shape used by a BibTeX value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueDelimiter {
    /// Value used `{ ... }`.
    Braces,
    /// Value used `" ... "`.
    Quotes,
    /// Value was a bare number or identifier.
    Bare,
    /// Value used one or more `#` concatenation separators.
    Concatenation,
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
    /// Original value delimiter or source shape, when retained.
    pub delimiter: Option<ValueDelimiter>,
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
            delimiter: None,
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
    /// Source location for the entry type token, when available.
    pub entry_type_source: Option<SourceSpan>,
    /// Source location for the citation key token, when available.
    pub key_source: Option<SourceSpan>,
    /// Entry body delimiter, when retained.
    pub delimiter: Option<EntryDelimiter>,
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
            entry_type_source: None,
            key_source: None,
            delimiter: None,
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
    pub fn from_failed_block(
        index: usize,
        failed: FailedBlock<'a>,
        source_map: Option<&SourceMap<'_>>,
    ) -> Self {
        let diagnostic = diagnostic_for_failed_block(index, &failed, source_map);

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
    pub fn from_library(library: Library<'a>) -> Self {
        Self::from_library_with_sources(
            library,
            vec![ParsedSource {
                id: SourceId::new(0),
                name: None,
            }],
        )
    }

    pub(crate) fn from_library_with_sources(
        library: Library<'a>,
        sources: Vec<ParsedSource<'a>>,
    ) -> Self {
        Self::from_library_with_source_map(library, sources, None)
    }

    pub(crate) fn from_library_with_source_map(
        library: Library<'a>,
        sources: Vec<ParsedSource<'a>>,
        source_map: Option<&SourceMap<'_>>,
    ) -> Self {
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
            .map(|(index, failed)| ParsedFailedBlock::from_failed_block(index, failed, source_map))
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

    pub(crate) fn apply_entry_locations(
        &mut self,
        entry_index: usize,
        raw: &'a str,
        source_map: &SourceMap<'a>,
        preserve_raw: bool,
    ) {
        let Some(entry) = self.entries.get_mut(entry_index) else {
            return;
        };
        let Some(entry_span) = entry.source else {
            return;
        };
        let Some(locations) = locate_entry(raw, entry_span.byte_start, entry.fields.len()) else {
            return;
        };

        entry.entry_type_source =
            Some(source_map.span(locations.entry_type.0, locations.entry_type.1));
        entry.key_source = Some(source_map.span(locations.key.0, locations.key.1));
        entry.delimiter = Some(locations.delimiter);
        if preserve_raw {
            entry.raw = Some(Cow::Borrowed(raw));
        }

        for (field, location) in entry.fields.iter_mut().zip(locations.fields) {
            field.source = Some(source_map.span(location.whole.0, location.whole.1));
            field.name_source = Some(source_map.span(location.name.0, location.name.1));
            field.value.source = Some(source_map.span(location.value.0, location.value.1));
            field.value_source = field.value.source;
            field.value.delimiter = Some(location.value_delimiter);

            if preserve_raw {
                if let Some(source) = field.source.and_then(|span| source_map.slice(span)) {
                    field.raw = Some(Cow::Borrowed(source));
                }
                if let Some(source) = field.value_source.and_then(|span| source_map.slice(span)) {
                    field.value.raw = Some(Cow::Borrowed(source));
                }
            }
        }
    }

    pub(crate) fn apply_raw_items(&mut self, raw_items: &[RawBuildItem<'a>]) {
        let mut string_index = 0;
        let mut preamble_index = 0;
        let mut comment_index = 0;

        for raw_item in raw_items {
            match raw_item {
                RawBuildItem::Parsed(crate::parser::ParsedItem::String(_, _), _, raw) => {
                    if let Some(parsed) = self.strings.get_mut(string_index) {
                        parsed.raw = Some(Cow::Borrowed(raw));
                        if let Some(value_raw) = locate_definition_value(raw) {
                            parsed.value.raw = Some(Cow::Borrowed(value_raw));
                            parsed.value.delimiter = Some(value_delimiter(value_raw));
                        }
                    }
                    string_index += 1;
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::Preamble(_), _, raw) => {
                    if let Some(parsed) = self.preambles.get_mut(preamble_index) {
                        parsed.raw = Some(Cow::Borrowed(raw));
                        if let Some(value_raw) = locate_preamble_value(raw) {
                            parsed.value.raw = Some(Cow::Borrowed(value_raw));
                            parsed.value.delimiter = Some(value_delimiter(value_raw));
                        }
                    }
                    preamble_index += 1;
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::Comment(_), _, raw) => {
                    if let Some(parsed) = self.comments.get_mut(comment_index) {
                        parsed.raw = Some(Cow::Borrowed(raw));
                    }
                    comment_index += 1;
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::Entry(_), _, _)
                | RawBuildItem::Failed(_) => {}
            }
        }
    }

    pub(crate) fn recover_partial_entries(
        &mut self,
        source_map: &SourceMap<'a>,
        preserve_raw: bool,
    ) {
        let old_entries = std::mem::take(&mut self.entries);
        let old_failed_blocks = std::mem::take(&mut self.failed_blocks);
        let old_blocks = std::mem::take(&mut self.blocks);
        let mut new_entries = Vec::with_capacity(old_entries.len());
        let mut new_failed_blocks = Vec::new();
        let mut new_blocks = Vec::with_capacity(old_blocks.len());

        for block in old_blocks {
            match block {
                ParsedBlock::Entry(index) => {
                    let new_index = new_entries.len();
                    if let Some(entry) = old_entries.get(index) {
                        new_entries.push(entry.clone());
                        new_blocks.push(ParsedBlock::Entry(new_index));
                    }
                }
                ParsedBlock::Failed(index) => {
                    let Some(failed) = old_failed_blocks.get(index) else {
                        continue;
                    };
                    let new_index = new_entries.len();
                    if let Some(partial) =
                        recover_partial_entry(failed, source_map, new_index, preserve_raw)
                    {
                        new_entries.push(partial);
                        new_blocks.push(ParsedBlock::Entry(new_index));
                    } else {
                        let failed_index = new_failed_blocks.len();
                        new_failed_blocks.push(failed.clone());
                        new_blocks.push(ParsedBlock::Failed(failed_index));
                    }
                }
                ParsedBlock::String(index) => new_blocks.push(ParsedBlock::String(index)),
                ParsedBlock::Preamble(index) => new_blocks.push(ParsedBlock::Preamble(index)),
                ParsedBlock::Comment(index) => new_blocks.push(ParsedBlock::Comment(index)),
            }
        }

        self.entries = new_entries;
        self.failed_blocks = new_failed_blocks;
        self.blocks = new_blocks;
        self.rebuild_diagnostics_and_status();
    }

    fn rebuild_diagnostics_and_status(&mut self) {
        self.diagnostics.clear();
        self.diagnostics.extend(
            self.entries
                .iter()
                .flat_map(|entry| entry.diagnostics.iter().cloned()),
        );
        self.diagnostics.extend(
            self.failed_blocks
                .iter()
                .flat_map(|failed| failed.diagnostics.iter().cloned()),
        );

        self.status = if self.diagnostics.is_empty() {
            ParseStatus::Ok
        } else if self.entries.is_empty() && self.strings.is_empty() && self.preambles.is_empty() {
            ParseStatus::Failed
        } else {
            ParseStatus::Partial
        };
    }

    pub(crate) fn failed_from_error(
        sources: Vec<ParsedSource<'a>>,
        source_map: &SourceMap<'a>,
        error: &crate::Error,
    ) -> Self {
        let (byte, message, fallback_snippet) = match error {
            crate::Error::ParseError {
                line,
                column,
                message,
                snippet,
            } => (
                source_map.byte_at_line_column(*line, *column).unwrap_or(0),
                message.clone(),
                snippet.clone(),
            ),
            other => (0, other.to_string(), None),
        };
        let raw = source_map.input().get(byte..).unwrap_or_default();
        let failed_source = source_map.span(byte, source_map.len());
        let failed = FailedBlock {
            raw: Cow::Borrowed(raw),
            error: message.clone(),
            source: Some(failed_source),
        };
        let diagnostic = diagnostic_for_raw_failure(
            0,
            raw,
            message,
            Some(failed_source),
            Some(source_map),
            byte,
            fallback_snippet,
        );
        let failed_block = ParsedFailedBlock {
            raw: failed.raw,
            error: failed.error,
            source: failed.source,
            diagnostics: vec![diagnostic.clone()],
        };

        Self {
            library: Library::new(),
            sources,
            entries: Vec::new(),
            strings: Vec::new(),
            preambles: Vec::new(),
            comments: Vec::new(),
            failed_blocks: vec![failed_block],
            blocks: vec![ParsedBlock::Failed(0)],
            diagnostics: vec![diagnostic],
            status: ParseStatus::Failed,
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

    /// Return summary counts for the parsed document.
    #[must_use]
    pub fn summary(&self) -> ParseSummary {
        let mut warnings = 0;
        let mut errors = 0;
        let mut infos = 0;

        for diagnostic in &self.diagnostics {
            match diagnostic.severity {
                DiagnosticSeverity::Error => errors += 1,
                DiagnosticSeverity::Warning => warnings += 1,
                DiagnosticSeverity::Info => infos += 1,
            }
        }

        ParseSummary {
            status: self.status,
            entries: self.entries.len(),
            warnings,
            errors,
            infos,
            failed_blocks: self.failed_blocks.len(),
            recovered_blocks: self
                .entries
                .iter()
                .filter(|entry| entry.status == ParsedEntryStatus::Partial)
                .count(),
        }
    }
}

#[derive(Debug, Clone)]
struct EntryLocations {
    entry_type: (usize, usize),
    key: (usize, usize),
    delimiter: EntryDelimiter,
    fields: Vec<FieldLocations>,
}

#[derive(Debug, Clone, Copy)]
struct FieldLocations {
    whole: (usize, usize),
    name: (usize, usize),
    value: (usize, usize),
    value_delimiter: ValueDelimiter,
}

#[derive(Debug, Clone)]
struct FailureClassification {
    code: DiagnosticCode,
    range: (usize, usize),
}

fn diagnostic_for_failed_block(
    index: usize,
    failed: &FailedBlock<'_>,
    source_map: Option<&SourceMap<'_>>,
) -> Diagnostic {
    let absolute_start = failed.source.map_or(0, |source| source.byte_start);
    diagnostic_for_raw_failure(
        index,
        &failed.raw,
        failed.error.clone(),
        failed.source,
        source_map,
        absolute_start,
        None,
    )
}

fn diagnostic_for_raw_failure(
    index: usize,
    raw: &str,
    fallback_message: String,
    fallback_source: Option<SourceSpan>,
    source_map: Option<&SourceMap<'_>>,
    absolute_start: usize,
    fallback_snippet: Option<String>,
) -> Diagnostic {
    let classification = classify_failure(raw);
    let source = source_map
        .map(|map| {
            map.span(
                absolute_start + classification.range.0,
                absolute_start + classification.range.1,
            )
        })
        .or(fallback_source);
    let snippet = source
        .and_then(|span| source_map.and_then(|map| map.snippet(span, 160)))
        .or(fallback_snippet)
        .or_else(|| Some(raw.chars().take(160).collect()));

    let mut diagnostic = Diagnostic::error(
        classification.code.clone(),
        diagnostic_message(&classification.code, fallback_message),
        DiagnosticTarget::FailedBlock(index),
        source,
    );
    diagnostic.snippet = snippet;
    diagnostic
}

fn recover_partial_entry<'a>(
    failed: &ParsedFailedBlock<'a>,
    source_map: &SourceMap<'a>,
    entry_index: usize,
    preserve_raw: bool,
) -> Option<ParsedEntry<'a>> {
    let raw: &'a str = match &failed.raw {
        Cow::Borrowed(raw) => raw,
        Cow::Owned(_) => return None,
    };
    let absolute_start = failed.source?.byte_start;
    let header = parse_partial_header(raw, source_map, absolute_start)?;
    let fields = recover_partial_fields(
        raw,
        source_map,
        absolute_start,
        header.field_start,
        header.closing,
        preserve_raw,
    );
    if fields.is_empty() {
        return None;
    }

    let diagnostic = diagnostic_for_partial_entry(entry_index, failed, source_map);

    Some(ParsedEntry {
        ty: header.ty,
        key: header.key,
        fields,
        status: ParsedEntryStatus::Partial,
        source: failed.source,
        entry_type_source: header.entry_type_source,
        key_source: header.key_source,
        delimiter: Some(header.delimiter),
        raw: preserve_raw.then(|| failed.raw.clone()),
        diagnostics: vec![diagnostic],
    })
}

struct PartialHeader<'a> {
    ty: EntryType<'a>,
    key: Cow<'a, str>,
    entry_type_source: Option<SourceSpan>,
    key_source: Option<SourceSpan>,
    delimiter: EntryDelimiter,
    field_start: usize,
    closing: u8,
}

fn parse_partial_header<'a>(
    raw: &'a str,
    source_map: &SourceMap<'a>,
    absolute_start: usize,
) -> Option<PartialHeader<'a>> {
    let bytes = raw.as_bytes();
    let mut pos = bytes.iter().position(|byte| *byte == b'@')? + 1;

    let entry_type_start = pos;
    pos += scan_identifier(&bytes[pos..]);
    if pos == entry_type_start {
        return None;
    }
    let ty = EntryType::parse(&raw[entry_type_start..pos]);
    let entry_type_source =
        Some(source_map.span(absolute_start + entry_type_start, absolute_start + pos));

    pos = skip_ascii_whitespace(bytes, pos);
    let (delimiter, closing) = match *bytes.get(pos)? {
        b'{' => (EntryDelimiter::Braces, b'}'),
        b'(' => (EntryDelimiter::Parentheses, b')'),
        _ => return None,
    };
    pos += 1;
    pos = skip_ascii_whitespace(bytes, pos);

    let key_start = pos;
    pos += scan_identifier(&bytes[pos..]);
    if pos == key_start {
        return None;
    }
    let key = Cow::Borrowed(&raw[key_start..pos]);
    let key_source = Some(source_map.span(absolute_start + key_start, absolute_start + pos));

    pos = skip_ascii_whitespace(bytes, pos);
    if bytes.get(pos) != Some(&b',') {
        return None;
    }

    Some(PartialHeader {
        ty,
        key,
        entry_type_source,
        key_source,
        delimiter,
        field_start: pos + 1,
        closing,
    })
}

fn recover_partial_fields<'a>(
    raw: &'a str,
    source_map: &SourceMap<'a>,
    absolute_start: usize,
    mut pos: usize,
    closing: u8,
    preserve_raw: bool,
) -> Vec<ParsedField<'a>> {
    let bytes = raw.as_bytes();
    let mut fields = Vec::new();

    loop {
        pos = skip_ascii_whitespace(bytes, pos);
        let Some(&byte) = bytes.get(pos) else {
            break;
        };
        if byte == closing || byte == b'@' {
            break;
        }

        let field_start = pos;
        let name_start = pos;
        pos += scan_identifier(&bytes[pos..]);
        if pos == name_start {
            break;
        }
        let name_end = pos;
        let name = Cow::Borrowed(&raw[name_start..name_end]);

        pos = skip_ascii_whitespace(bytes, pos);
        if bytes.get(pos) != Some(&b'=') {
            break;
        }
        pos += 1;
        pos = skip_ascii_whitespace(bytes, pos);

        let value_start = pos;
        let tail = &raw[value_start..];
        let mut value_input = tail;
        let Ok(value) = crate::parser::value::parse_value_field(&mut value_input) else {
            break;
        };
        let consumed = tail.len() - value_input.len();
        let value_end = trim_ascii_whitespace_end(bytes, value_start, value_start + consumed);
        let boundary = value_start + consumed;
        let field_end = match bytes.get(boundary) {
            Some(b',') => boundary + 1,
            Some(byte) if *byte == closing => boundary,
            Some(_) | None => boundary,
        };

        let field_source =
            source_map.span(absolute_start + field_start, absolute_start + field_end);
        let value_source =
            source_map.span(absolute_start + value_start, absolute_start + value_end);
        fields.push(ParsedField {
            name,
            value: ParsedValue {
                value,
                raw: preserve_raw.then(|| Cow::Borrowed(&raw[value_start..value_end])),
                source: Some(value_source),
                expanded: None,
                delimiter: Some(value_delimiter(&raw[value_start..value_end])),
            },
            raw: preserve_raw.then(|| Cow::Borrowed(&raw[field_start..field_end])),
            source: Some(field_source),
            name_source: Some(
                source_map.span(absolute_start + name_start, absolute_start + name_end),
            ),
            value_source: Some(value_source),
        });

        match bytes.get(boundary) {
            Some(b',') => pos = boundary + 1,
            Some(byte) if *byte == closing => break,
            _ => break,
        }
    }

    fields
}

fn diagnostic_for_partial_entry(
    entry_index: usize,
    failed: &ParsedFailedBlock<'_>,
    source_map: &SourceMap<'_>,
) -> Diagnostic {
    let absolute_start = failed.source.map_or(0, |source| source.byte_start);
    let mut diagnostic = diagnostic_for_raw_failure(
        entry_index,
        &failed.raw,
        failed.error.clone(),
        failed.source,
        Some(source_map),
        absolute_start,
        None,
    );
    diagnostic.target = DiagnosticTarget::Entry(entry_index);
    diagnostic
}

fn diagnostic_message(code: &DiagnosticCode, fallback: String) -> String {
    match code.as_str() {
        "missing-entry-key" => "missing citation key".to_string(),
        "missing-field-separator" => "missing field separator".to_string(),
        "expected-field-name" => "expected field name".to_string(),
        "empty-field-value" => "empty field value".to_string(),
        "expected-value-atom" => "expected value atom".to_string(),
        "bad-field-boundary" => "expected comma or entry close after field value".to_string(),
        "bad-value-boundary" => "expected value after concatenation operator".to_string(),
        "unclosed-entry" => "entry ended before its closing delimiter".to_string(),
        "unclosed-braced-value" => "braced value ended before its closing brace".to_string(),
        "unclosed-quoted-value" => "quoted value ended before its closing quote".to_string(),
        _ => fallback,
    }
}

fn classify_failure(raw: &str) -> FailureClassification {
    classify_failure_inner(raw).unwrap_or_else(|| FailureClassification {
        code: DiagnosticCode::PARSE_ERROR,
        range: empty_range(0),
    })
}

fn classify_failure_inner(raw: &str) -> Option<FailureClassification> {
    let bytes = raw.as_bytes();
    let header = match parse_failure_header(bytes)? {
        Ok(header) => header,
        Err(classification) => return Some(classification),
    };

    classify_failure_fields(bytes, header.pos, header.closing)
}

#[derive(Debug, Clone, Copy)]
struct FailureHeader {
    pos: usize,
    closing: u8,
}

fn parse_failure_header(bytes: &[u8]) -> Option<Result<FailureHeader, FailureClassification>> {
    let mut pos = bytes.iter().position(|byte| *byte == b'@')?;
    pos += 1;
    pos += scan_identifier(&bytes[pos..]);
    pos = skip_ascii_whitespace(bytes, pos);

    let opening = *bytes.get(pos)?;
    let closing = match opening {
        b'{' => b'}',
        b'(' => b')',
        _ => {
            return Some(Err(classification(
                DiagnosticCode::UNCLOSED_ENTRY,
                pos,
                bytes.len(),
            )));
        }
    };
    pos += 1;
    pos = skip_ascii_whitespace(bytes, pos);

    let key_len = scan_identifier(&bytes[pos..]);
    if key_len == 0 {
        return Some(Err(classification(
            DiagnosticCode::MISSING_ENTRY_KEY,
            pos,
            bytes.len(),
        )));
    }
    pos += key_len;
    pos = skip_ascii_whitespace(bytes, pos);
    if bytes.get(pos) != Some(&b',') {
        return Some(Err(classification(
            DiagnosticCode::MISSING_FIELD_SEPARATOR,
            pos,
            bytes.len(),
        )));
    }
    pos += 1;

    Some(Ok(FailureHeader { pos, closing }))
}

fn classify_failure_fields(
    bytes: &[u8],
    mut pos: usize,
    closing: u8,
) -> Option<FailureClassification> {
    loop {
        pos = skip_ascii_whitespace(bytes, pos);
        let Some(&byte) = bytes.get(pos) else {
            return Some(classification(
                DiagnosticCode::UNCLOSED_ENTRY,
                pos,
                bytes.len(),
            ));
        };
        if byte == closing {
            return None;
        }
        if byte == b'@' {
            return Some(classification(
                DiagnosticCode::UNCLOSED_ENTRY,
                pos,
                bytes.len(),
            ));
        }

        let field_name_len = scan_identifier(&bytes[pos..]);
        if field_name_len == 0 {
            return Some(classification(
                DiagnosticCode::EXPECTED_FIELD_NAME,
                pos,
                bytes.len(),
            ));
        }
        pos += field_name_len;
        pos = skip_ascii_whitespace(bytes, pos);
        if bytes.get(pos) != Some(&b'=') {
            return Some(classification(
                DiagnosticCode::MISSING_FIELD_SEPARATOR,
                pos,
                bytes.len(),
            ));
        }
        pos += 1;
        pos = skip_ascii_whitespace(bytes, pos);

        let Some(&value_start) = bytes.get(pos) else {
            return Some(classification(
                DiagnosticCode::EMPTY_FIELD_VALUE,
                pos,
                bytes.len(),
            ));
        };
        if value_start == b',' || value_start == closing {
            return Some(classification(
                DiagnosticCode::EMPTY_FIELD_VALUE,
                pos,
                bytes.len(),
            ));
        }
        if value_start == b'#' {
            return Some(classification(
                DiagnosticCode::EXPECTED_VALUE_ATOM,
                pos,
                bytes.len(),
            ));
        }

        match scan_value_sequence(bytes, pos, closing) {
            Ok(next_pos) => pos = next_pos,
            Err(classification) => return Some(classification),
        }
    }
}

fn scan_value_sequence(
    bytes: &[u8],
    mut pos: usize,
    closing: u8,
) -> Result<usize, FailureClassification> {
    loop {
        pos = skip_ascii_whitespace(bytes, pos);
        let atom_start = pos;
        let Some(&byte) = bytes.get(pos) else {
            return Err(classification(
                DiagnosticCode::EXPECTED_VALUE_ATOM,
                pos,
                bytes.len(),
            ));
        };

        match byte {
            b'"' => {
                pos = skip_quoted_checked(bytes, pos + 1).ok_or_else(|| {
                    classification(
                        DiagnosticCode::UNCLOSED_QUOTED_VALUE,
                        atom_start,
                        bytes.len(),
                    )
                })?;
            }
            b'{' => {
                pos = skip_braced_checked(bytes, pos + 1).ok_or_else(|| {
                    classification(
                        DiagnosticCode::UNCLOSED_BRACED_VALUE,
                        atom_start,
                        bytes.len(),
                    )
                })?;
            }
            b',' => {
                return Err(classification(
                    DiagnosticCode::EMPTY_FIELD_VALUE,
                    pos,
                    bytes.len(),
                ));
            }
            b if b == closing => {
                return Err(classification(
                    DiagnosticCode::EMPTY_FIELD_VALUE,
                    pos,
                    bytes.len(),
                ));
            }
            b'#' => {
                return Err(classification(
                    DiagnosticCode::EXPECTED_VALUE_ATOM,
                    pos,
                    bytes.len(),
                ));
            }
            _ => {
                let identifier_len = scan_identifier(&bytes[pos..]);
                if identifier_len == 0 {
                    return Err(classification(
                        DiagnosticCode::EXPECTED_VALUE_ATOM,
                        pos,
                        bytes.len(),
                    ));
                }
                pos += identifier_len;
            }
        }

        pos = skip_ascii_whitespace(bytes, pos);
        let Some(&boundary) = bytes.get(pos) else {
            return Err(classification(
                DiagnosticCode::UNCLOSED_ENTRY,
                pos,
                bytes.len(),
            ));
        };

        match boundary {
            b'#' => {
                let hash = pos;
                pos += 1;
                pos = skip_ascii_whitespace(bytes, pos);
                if matches!(bytes.get(pos), None | Some(b',' | b'#'))
                    || bytes.get(pos) == Some(&closing)
                {
                    return Err(classification(
                        DiagnosticCode::BAD_VALUE_BOUNDARY,
                        hash,
                        bytes.len(),
                    ));
                }
            }
            b',' => return Ok(pos + 1),
            b if b == closing => return Ok(pos),
            _ => {
                return Err(classification(
                    DiagnosticCode::BAD_FIELD_BOUNDARY,
                    pos,
                    bytes.len(),
                ));
            }
        }
    }
}

fn classification(code: DiagnosticCode, pos: usize, len: usize) -> FailureClassification {
    FailureClassification {
        code,
        range: single_byte_range(pos, len),
    }
}

const fn empty_range(pos: usize) -> (usize, usize) {
    (pos, pos)
}

fn single_byte_range(pos: usize, len: usize) -> (usize, usize) {
    let start = pos.min(len);
    (start, (start + 1).min(len))
}

fn locate_entry(raw: &str, absolute_start: usize, field_count: usize) -> Option<EntryLocations> {
    let bytes = raw.as_bytes();
    let mut pos = 0;
    if bytes.get(pos) != Some(&b'@') {
        return None;
    }
    pos += 1;

    let entry_type_start = pos;
    pos += scan_identifier(&bytes[pos..]);
    if pos == entry_type_start {
        return None;
    }
    let entry_type = (absolute_start + entry_type_start, absolute_start + pos);

    pos = skip_ascii_whitespace(bytes, pos);
    let opening = *bytes.get(pos)?;
    let (delimiter, closing) = match opening {
        b'{' => (EntryDelimiter::Braces, b'}'),
        b'(' => (EntryDelimiter::Parentheses, b')'),
        _ => return None,
    };
    pos += 1;
    pos = skip_ascii_whitespace(bytes, pos);

    let key_start = pos;
    pos += scan_identifier(&bytes[pos..]);
    if pos == key_start {
        return None;
    }
    let key = (absolute_start + key_start, absolute_start + pos);

    pos = skip_ascii_whitespace(bytes, pos);
    if bytes.get(pos) != Some(&b',') {
        return Some(EntryLocations {
            entry_type,
            key,
            delimiter,
            fields: Vec::new(),
        });
    }
    pos += 1;

    let mut fields = Vec::with_capacity(field_count);
    while fields.len() < field_count {
        pos = skip_ascii_whitespace(bytes, pos);
        if bytes.get(pos) == Some(&closing) || pos >= bytes.len() {
            break;
        }

        let field_start = pos;
        let name_start = pos;
        pos += scan_identifier(&bytes[pos..]);
        if pos == name_start {
            break;
        }
        let name_end = pos;

        pos = skip_ascii_whitespace(bytes, pos);
        if bytes.get(pos) != Some(&b'=') {
            break;
        }
        pos += 1;
        pos = skip_ascii_whitespace(bytes, pos);

        let value_start = pos;
        let boundary = find_value_boundary(bytes, pos, closing);
        let value_end = trim_ascii_whitespace_end(bytes, value_start, boundary);
        let mut whole_end = value_end;
        pos = boundary;
        if bytes.get(pos) == Some(&b',') {
            whole_end = pos + 1;
            pos += 1;
        }

        fields.push(FieldLocations {
            whole: (absolute_start + field_start, absolute_start + whole_end),
            name: (absolute_start + name_start, absolute_start + name_end),
            value: (absolute_start + value_start, absolute_start + value_end),
            value_delimiter: value_delimiter(&raw[value_start..value_end]),
        });
    }

    Some(EntryLocations {
        entry_type,
        key,
        delimiter,
        fields,
    })
}

fn value_delimiter(raw_value: &str) -> ValueDelimiter {
    let trimmed = raw_value.trim_start();
    if has_top_level_concat(trimmed.as_bytes()) {
        return ValueDelimiter::Concatenation;
    }

    match trimmed.as_bytes().first() {
        Some(b'{') => ValueDelimiter::Braces,
        Some(b'"') => ValueDelimiter::Quotes,
        _ => ValueDelimiter::Bare,
    }
}

fn locate_definition_value(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let equals = bytes.iter().position(|byte| *byte == b'=')?;
    let value_start = skip_ascii_whitespace(bytes, equals + 1);
    let closing = enclosing_close_byte(bytes)?;
    let boundary = find_value_boundary(bytes, value_start, closing);
    let value_end = trim_ascii_whitespace_end(bytes, value_start, boundary);
    raw.get(value_start..value_end)
}

fn locate_preamble_value(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let opening = bytes.iter().position(|byte| matches!(byte, b'{' | b'('))?;
    let closing = match bytes[opening] {
        b'{' => b'}',
        b'(' => b')',
        _ => return None,
    };
    let value_start = skip_ascii_whitespace(bytes, opening + 1);
    let boundary = find_value_boundary(bytes, value_start, closing);
    let value_end = trim_ascii_whitespace_end(bytes, value_start, boundary);
    raw.get(value_start..value_end)
}

fn enclosing_close_byte(bytes: &[u8]) -> Option<u8> {
    let opening = bytes.iter().position(|byte| matches!(byte, b'{' | b'('))?;
    match bytes[opening] {
        b'{' => Some(b'}'),
        b'(' => Some(b')'),
        _ => None,
    }
}

fn has_top_level_concat(bytes: &[u8]) -> bool {
    let mut pos = 0;
    while let Some(&byte) = bytes.get(pos) {
        match byte {
            b'{' => pos = skip_braced(bytes, pos + 1),
            b'"' => pos = skip_quoted(bytes, pos + 1),
            b'#' => return true,
            _ => pos += 1,
        }
    }
    false
}

fn skip_ascii_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while matches!(bytes.get(pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        pos += 1;
    }
    pos
}

fn trim_ascii_whitespace_end(bytes: &[u8], start: usize, mut end: usize) -> usize {
    while end > start && matches!(bytes.get(end - 1), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        end -= 1;
    }
    end
}

fn scan_identifier(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .position(|byte| !is_identifier_byte(*byte))
        .unwrap_or(bytes.len())
}

const fn is_identifier_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-' | b':' | b'.'
    )
}

fn find_value_boundary(bytes: &[u8], mut pos: usize, closing: u8) -> usize {
    while let Some(&byte) = bytes.get(pos) {
        match byte {
            b'{' => pos = skip_braced(bytes, pos + 1),
            b'"' => pos = skip_quoted(bytes, pos + 1),
            b',' => break,
            b if b == closing => break,
            _ => pos += 1,
        }
    }
    pos
}

fn skip_braced(bytes: &[u8], mut pos: usize) -> usize {
    let mut depth = 0usize;
    while let Some(&byte) = bytes.get(pos) {
        match byte {
            b'\\' => pos = (pos + 2).min(bytes.len()),
            b'{' => {
                depth += 1;
                pos += 1;
            }
            b'}' if depth == 0 => return pos + 1,
            b'}' => {
                depth -= 1;
                pos += 1;
            }
            _ => pos += 1,
        }
    }
    pos
}

fn skip_braced_checked(bytes: &[u8], mut pos: usize) -> Option<usize> {
    let mut depth = 0usize;
    while let Some(&byte) = bytes.get(pos) {
        match byte {
            b'\\' => pos = (pos + 2).min(bytes.len()),
            b'{' => {
                depth += 1;
                pos += 1;
            }
            b'}' if depth == 0 => return Some(pos + 1),
            b'}' => {
                depth -= 1;
                pos += 1;
            }
            _ => pos += 1,
        }
    }
    None
}

fn skip_quoted(bytes: &[u8], mut pos: usize) -> usize {
    while let Some(&byte) = bytes.get(pos) {
        match byte {
            b'\\' => pos = (pos + 2).min(bytes.len()),
            b'"' => return pos + 1,
            _ => pos += 1,
        }
    }
    pos
}

fn skip_quoted_checked(bytes: &[u8], mut pos: usize) -> Option<usize> {
    while let Some(&byte) = bytes.get(pos) {
        match byte {
            b'\\' => pos = (pos + 2).min(bytes.len()),
            b'"' => return Some(pos + 1),
            _ => pos += 1,
        }
    }
    None
}
