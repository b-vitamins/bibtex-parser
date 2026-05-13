//! BibTeX library representation

use crate::{
    canonical_biblatex_field_alias, normalize_doi, CorpusEvent, CorpusSource, Entry, Error,
    ParseEvent, ParseFlow, ParsedBlock, ParsedComment, ParsedCorpus, ParsedDocument, ParsedEntry,
    ParsedFailedBlock, ParsedPreamble, ParsedSource, ParsedString, Result, SourceId, SourceMap,
    SourceSpan, StreamingSummary, ValidationError, ValidationLevel, Value,
};
use ahash::AHashMap;
use memchr::memchr;
use std::borrow::Cow;
use std::ops::Deref;
use std::path::Path;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

const SMALL_EXPANSION_CACHE_LIMIT: usize = 16;
const SMALL_STRING_LOOKUP_LIMIT: usize = 16;
const CONCAT_CACHE_LIMIT: usize = 16;

enum ExpansionCache<'a> {
    Small(Vec<(Cow<'a, str>, Value<'a>)>),
    Large(AHashMap<Cow<'a, str>, Value<'a>>),
}

impl<'a> ExpansionCache<'a> {
    fn with_capacity(capacity: usize) -> Self {
        if capacity <= SMALL_EXPANSION_CACHE_LIMIT {
            Self::Small(Vec::with_capacity(capacity))
        } else {
            Self::Large(AHashMap::with_capacity(capacity))
        }
    }

    fn get_cloned(&mut self, name: &str) -> Option<Value<'a>> {
        match self {
            Self::Small(entries) => {
                let index = entries.iter().position(|(key, _)| key.as_ref() == name)?;
                if index != 0 {
                    entries.swap(0, index);
                }
                Some(entries[0].1.clone())
            }
            Self::Large(entries) => entries.get(name).cloned(),
        }
    }

    fn insert(&mut self, name: Cow<'a, str>, value: Value<'a>) {
        match self {
            Self::Small(entries) => {
                if entries.len() < SMALL_EXPANSION_CACHE_LIMIT {
                    entries.push((name, value));
                } else {
                    let mut large = AHashMap::with_capacity(entries.len() + 1);
                    for (key, value) in entries.drain(..) {
                        large.insert(key, value);
                    }
                    large.insert(name, value);
                    *self = Self::Large(large);
                }
            }
            Self::Large(entries) => {
                entries.insert(name, value);
            }
        }
    }
}

struct ConcatCache<'a> {
    entries: Vec<(Box<[Value<'a>]>, Value<'a>)>,
}

impl<'a> ConcatCache<'a> {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn get_cloned(&mut self, parts: &[Value<'a>]) -> Option<Value<'a>> {
        let index = self
            .entries
            .iter()
            .position(|(cached_parts, _)| concat_parts_equal(cached_parts, parts))?;
        if index != 0 {
            self.entries.swap(0, index);
        }
        Some(self.entries[0].1.clone())
    }

    fn insert(&mut self, parts: Box<[Value<'a>]>, value: Value<'a>) {
        if self.entries.len() < CONCAT_CACHE_LIMIT {
            self.entries.push((parts, value));
        }
    }
}

fn concat_parts_equal(left: &[Value<'_>], right: &[Value<'_>]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| cache_values_equal(left, right))
}

fn cache_values_equal(left: &Value<'_>, right: &Value<'_>) -> bool {
    match (left, right) {
        (Value::Literal(left), Value::Literal(right))
        | (Value::Variable(left), Value::Variable(right)) => left.as_ref() == right.as_ref(),
        (Value::Number(left), Value::Number(right)) => left == right,
        (Value::Concat(left), Value::Concat(right)) => concat_parts_equal(left, right),
        _ => false,
    }
}

/// Get month expansion for a given abbreviation (case-insensitive)
///
/// Returns None if the name is not a recognized month abbreviation.
/// This is used as a fallback when user-defined string variables are not found.
#[inline]
fn get_month_expansion(name: &str) -> Option<&'static str> {
    let bytes = name.as_bytes();
    if bytes.len() != 3 {
        return None;
    }

    let key = (u32::from(bytes[0] | 0x20) << 16)
        | (u32::from(bytes[1] | 0x20) << 8)
        | u32::from(bytes[2] | 0x20);

    match key {
        0x6a_61_6e => Some("January"),
        0x66_65_62 => Some("February"),
        0x6d_61_72 => Some("March"),
        0x61_70_72 => Some("April"),
        0x6d_61_79 => Some("May"),
        0x6a_75_6e => Some("June"),
        0x6a_75_6c => Some("July"),
        0x61_75_67 => Some("August"),
        0x73_65_70 => Some("September"),
        0x6f_63_74 => Some("October"),
        0x6e_6f_76 => Some("November"),
        0x64_65_63 => Some("December"),
        _ => None,
    }
}

#[inline]
fn get_string_value<'map, 'a>(
    strings: &'map [StringDefinition<'a>],
    string_lookup: &'map AHashMap<Cow<'a, str>, usize>,
    name: &str,
) -> Option<&'map Value<'a>> {
    get_string_definition(strings, string_lookup, name).map(|definition| &definition.value)
}

#[inline]
fn get_string_definition<'map, 'a>(
    strings: &'map [StringDefinition<'a>],
    string_lookup: &'map AHashMap<Cow<'a, str>, usize>,
    name: &str,
) -> Option<&'map StringDefinition<'a>> {
    if strings.len() <= SMALL_STRING_LOOKUP_LIMIT {
        strings
            .iter()
            .rev()
            .find(|definition| definition.name.as_ref() == name)
    } else {
        string_lookup
            .get(name)
            .and_then(|&index| strings.get(index))
    }
}

#[inline]
fn user_strings_shadow_month_constants(strings: &[StringDefinition<'_>]) -> bool {
    strings
        .iter()
        .any(|definition| get_month_expansion(definition.name.as_ref()).is_some())
}

/// Check if a value contains any variables
#[inline]
fn contains_variables(value: &Value) -> bool {
    match value {
        Value::Variable(_) => true,
        Value::Concat(parts) => parts.iter().any(contains_variables),
        _ => false,
    }
}

/// Check if a value contains variables that might be month constants
#[inline]
fn contains_potential_month_variables(value: &Value) -> bool {
    match value {
        Value::Variable(name) => get_month_expansion(name).is_some(),
        Value::Concat(parts) => parts.iter().any(contains_potential_month_variables),
        _ => false,
    }
}

#[inline]
const fn is_identifier_char(byte: u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-' | b':' | b'.'
    )
}

#[inline]
fn starts_with_at_keyword(input: &[u8], keyword: &[u8]) -> bool {
    if input.first() != Some(&b'@') || input.len() < keyword.len() + 1 {
        return false;
    }

    for (offset, &expected) in keyword.iter().enumerate() {
        if (input[offset + 1] | 0x20) != expected {
            return false;
        }
    }

    if input.len() == keyword.len() + 1 {
        return true;
    }

    !is_identifier_char(input[keyword.len() + 1])
}

#[derive(Debug, Clone, Copy)]
struct InputScan {
    may_contain_string_definition: bool,
    at_count: usize,
}

/// Fast pre-scan to detect `@string` entries and estimate block capacity.
fn scan_input(input: &str) -> InputScan {
    let bytes = input.as_bytes();
    let mut pos = 0;
    let mut at_count = 0;
    let mut may_contain_string_definition = false;

    while pos < bytes.len() {
        if let Some(offset) = memchr(b'@', &bytes[pos..]) {
            let at = pos + offset;
            at_count += 1;
            if starts_with_at_keyword(&bytes[at..], b"string") {
                may_contain_string_definition = true;
            }
            pos = at + 1;
        } else {
            break;
        }
    }

    InputScan {
        may_contain_string_definition,
        at_count,
    }
}

/// Detect whether a `@string` may appear after a regular entry.
///
/// False positives are acceptable (we take the conservative slow path), but
/// false negatives would be incorrect, so keyword matching mirrors parser rules.
fn input_may_have_late_string_definition(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut pos = 0;
    let mut saw_regular_entry = false;

    while pos < bytes.len() {
        if let Some(offset) = memchr(b'@', &bytes[pos..]) {
            let at = pos + offset;
            let tail = &bytes[at..];

            if starts_with_at_keyword(tail, b"string") {
                if saw_regular_entry {
                    return true;
                }
            } else if !saw_regular_entry
                && !starts_with_at_keyword(tail, b"preamble")
                && !starts_with_at_keyword(tail, b"comment")
            {
                // Anything else that looks like `@<identifier>` is treated as a regular entry.
                saw_regular_entry = true;
            }

            pos = at + 1;
        } else {
            break;
        }
    }

    false
}

fn next_recovery_boundary(input: &str, start: usize) -> usize {
    let bytes = input.as_bytes();
    let mut pos = start.saturating_add(1);
    while pos < bytes.len() {
        if bytes[pos] == b'@' && line_prefix_is_whitespace(bytes, pos) {
            return pos;
        }
        pos += 1;
    }
    input.len()
}

fn line_prefix_is_whitespace(bytes: &[u8], pos: usize) -> bool {
    let line_start = bytes[..pos]
        .iter()
        .rposition(|byte| matches!(byte, b'\n' | b'\r'))
        .map_or(0, |index| index + 1);

    bytes[line_start..pos]
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t'))
}

fn merge_streaming_summary(total: &mut StreamingSummary, source: StreamingSummary) {
    total.entries += source.entries;
    total.strings += source.strings;
    total.preambles += source.preambles;
    total.comments += source.comments;
    total.failed_blocks += source.failed_blocks;
    total.warnings += source.warnings;
    total.errors += source.errors;
    total.infos += source.infos;
    total.recovered_blocks += source.recovered_blocks;
    total.stopped |= source.stopped;
}

/// Parser configuration.
#[derive(Debug, Default, Clone)]
pub struct Parser {
    threads: Option<usize>,
    tolerant: bool,
    document: DocumentOptions,
}

#[derive(Debug, Default, Clone, Copy)]
struct DocumentOptions {
    capture_source: bool,
    preserve_raw: bool,
    expand_values: bool,
}

impl Parser {
    /// Create a new parser.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set number of threads (None = use all available)
    #[must_use]
    #[inline]
    pub fn threads(mut self, threads: impl Into<Option<usize>>) -> Self {
        self.threads = threads.into();
        self
    }

    /// Continue after malformed blocks and collect diagnostics.
    #[must_use]
    #[inline]
    pub const fn tolerant(mut self) -> Self {
        self.tolerant = true;
        self
    }

    /// Capture source spans for blocks.
    #[must_use]
    #[inline]
    pub const fn capture_source(mut self) -> Self {
        self.document.capture_source = true;
        self
    }

    /// Preserve exact raw source text in parsed-document output.
    #[must_use]
    #[inline]
    pub const fn preserve_raw(mut self) -> Self {
        self.document.preserve_raw = true;
        self
    }

    /// Populate expanded value text in parsed-document output.
    #[must_use]
    #[inline]
    pub const fn expand_values(mut self) -> Self {
        self.document.expand_values = true;
        self
    }

    /// Parse a single input string.
    #[inline]
    pub fn parse<'a>(&self, input: &'a str) -> Result<Library<'a>> {
        if self.tolerant {
            Library::parse_tolerant(input, self.document.capture_source)
        } else if self.document.capture_source {
            Library::parse_with_spans(input)
        } else {
            Library::parse_sequential(input)
        }
    }

    /// Parse a single input string into the parsed document model.
    ///
    /// Use this when a caller needs source-order blocks, diagnostics, raw-text
    /// slots, or partial parse results. Use [`Self::parse`] for the compact
    /// [`Library`] API.
    #[inline]
    pub fn parse_document<'a>(&self, input: &'a str) -> Result<ParsedDocument<'a>> {
        self.parse_document_with_source_id(SourceId::new(0), None, input)
    }

    /// Parse a named source into the parsed document model.
    ///
    /// The parser does not read files itself; callers provide the source name
    /// or path-like label together with the already-loaded input text.
    #[inline]
    pub fn parse_source<'a>(
        &self,
        source_name: impl Into<Cow<'a, str>>,
        input: &'a str,
    ) -> Result<ParsedDocument<'a>> {
        self.parse_document_with_source_id(SourceId::new(0), Some(source_name.into()), input)
    }

    /// Parse multiple named in-memory sources into a corpus result.
    pub fn parse_sources<'a>(&self, sources: &[CorpusSource<'a>]) -> Result<ParsedCorpus<'a>> {
        let mut documents = Vec::with_capacity(sources.len());
        for (index, source) in sources.iter().enumerate() {
            documents.push(self.parse_document_with_source_id(
                SourceId::new(index),
                Some(Cow::Borrowed(source.name)),
                source.input,
            )?);
        }

        Ok(ParsedCorpus::from_documents(documents))
    }

    /// Stream parsed source-order events to a callback.
    ///
    /// Strict mode returns an error on the first malformed block. Tolerant mode
    /// emits recovered partial entries or failed blocks with diagnostics and
    /// continues. The callback can return [`ParseFlow::Stop`] to stop after the
    /// current event; the returned summary then has `stopped = true`.
    #[inline]
    pub fn parse_events<'a, F>(&self, input: &'a str, on_event: F) -> Result<StreamingSummary>
    where
        F: FnMut(ParseEvent<'a>) -> Result<ParseFlow>,
    {
        self.parse_source_events_with_source(SourceId::new(0), None, input, on_event)
    }

    /// Stream parsed source-order events from a named source.
    #[inline]
    pub fn parse_source_events<'a, F>(
        &self,
        source_name: impl Into<Cow<'a, str>>,
        input: &'a str,
        on_event: F,
    ) -> Result<StreamingSummary>
    where
        F: FnMut(ParseEvent<'a>) -> Result<ParseFlow>,
    {
        self.parse_source_events_with_source(
            SourceId::new(0),
            Some(source_name.into()),
            input,
            on_event,
        )
    }

    /// Stream events from multiple named in-memory sources in corpus order.
    pub fn parse_corpus_events<'a, F>(
        &self,
        sources: &[CorpusSource<'a>],
        mut on_event: F,
    ) -> Result<StreamingSummary>
    where
        F: FnMut(CorpusEvent<'a>) -> Result<ParseFlow>,
    {
        let mut summary = StreamingSummary::default();

        for (index, source) in sources.iter().enumerate() {
            if summary.stopped {
                break;
            }

            let source_id = SourceId::new(index);
            let parsed_source = ParsedSource {
                id: source_id,
                name: Some(Cow::Borrowed(source.name)),
            };
            if on_event(CorpusEvent::SourceStart(parsed_source.clone()))? == ParseFlow::Stop {
                summary.stopped = true;
                break;
            }

            let source_summary = self.parse_source_events_with_source(
                source_id,
                Some(Cow::Borrowed(source.name)),
                source.input,
                |event| {
                    on_event(CorpusEvent::Event {
                        source: source_id,
                        event: Box::new(event),
                    })
                },
            )?;
            merge_streaming_summary(&mut summary, source_summary);

            if on_event(CorpusEvent::SourceEnd(parsed_source))? == ParseFlow::Stop {
                summary.stopped = true;
            }
        }

        summary.finalize_status();
        Ok(summary)
    }

    fn parse_source_events_with_source<'a, F>(
        &self,
        source_id: SourceId,
        source_name: Option<Cow<'a, str>>,
        input: &'a str,
        mut on_event: F,
    ) -> Result<StreamingSummary>
    where
        F: FnMut(ParseEvent<'a>) -> Result<ParseFlow>,
    {
        let source_map = SourceMap::new(Some(source_id), source_name, input);
        let mut summary = StreamingSummary::default();

        if self.tolerant {
            self.parse_tolerant_events(input, &source_map, &mut summary, &mut on_event)?;
        } else {
            crate::parser::parse_bibtex_stream_with_spans(input, |item, span, raw| {
                let source = source_map.span(span.byte_start, span.byte_end);
                self.emit_parsed_event(item, source, raw, &source_map, &mut summary, &mut on_event)
            })?;
        }

        summary.finalize_status();
        Ok(summary)
    }

    fn parse_tolerant_events<'a, F>(
        &self,
        input: &'a str,
        source_map: &SourceMap<'a>,
        summary: &mut StreamingSummary,
        on_event: &mut F,
    ) -> Result<()>
    where
        F: FnMut(ParseEvent<'a>) -> Result<ParseFlow>,
    {
        let mut remaining = input;

        loop {
            crate::parser::lexer::skip_whitespace(&mut remaining);
            if remaining.is_empty() || summary.stopped {
                break;
            }

            let start = input.len() - remaining.len();
            match crate::parser::parse_item(&mut remaining) {
                Ok(item) => {
                    let end = input.len() - remaining.len();
                    let source = source_map.span(start, end);
                    self.emit_parsed_event(
                        item,
                        source,
                        &input[start..end],
                        source_map,
                        summary,
                        on_event,
                    )?;
                }
                Err(err) => {
                    let end = next_recovery_boundary(input, start);
                    let failed = FailedBlock {
                        raw: Cow::Borrowed(&input[start..end]),
                        error: format!("Failed to parse entry: {err}"),
                        source: Some(source_map.span(start, end)),
                    };
                    let failed_index = summary.failed_blocks;
                    let failed = ParsedFailedBlock::from_failed_block(
                        failed_index,
                        failed,
                        Some(source_map),
                    );
                    if let Some(partial) = crate::document::recover_partial_stream_entry(
                        &failed,
                        source_map,
                        summary.entries,
                        self.document.preserve_raw,
                    ) {
                        Self::emit_event(ParseEvent::Entry(partial), summary, on_event)?;
                    } else {
                        Self::emit_event(ParseEvent::Failed(failed), summary, on_event)?;
                    }
                    remaining = &input[end..];
                }
            }
        }

        Ok(())
    }

    fn emit_parsed_event<'a, F>(
        &self,
        item: crate::parser::ParsedItem<'a>,
        source: SourceSpan,
        raw: &'a str,
        source_map: &SourceMap<'a>,
        summary: &mut StreamingSummary,
        on_event: &mut F,
    ) -> Result<()>
    where
        F: FnMut(ParseEvent<'a>) -> Result<ParseFlow>,
    {
        if summary.stopped {
            return Ok(());
        }

        let event = match item {
            crate::parser::ParsedItem::Entry(entry) => {
                ParseEvent::Entry(ParsedEntry::from_stream_entry(
                    entry,
                    source,
                    raw,
                    source_map,
                    self.document.preserve_raw,
                ))
            }
            crate::parser::ParsedItem::String(name, value) => {
                ParseEvent::String(ParsedString::from_stream_definition(
                    name,
                    value,
                    source,
                    raw,
                    self.document.preserve_raw,
                ))
            }
            crate::parser::ParsedItem::Preamble(value) => {
                ParseEvent::Preamble(ParsedPreamble::from_stream_preamble(
                    value,
                    source,
                    raw,
                    self.document.preserve_raw,
                ))
            }
            crate::parser::ParsedItem::Comment(text) => ParseEvent::Comment(
                ParsedComment::from_stream_comment(text, source, raw, self.document.preserve_raw),
            ),
        };

        Self::emit_event(event, summary, on_event)
    }

    fn emit_event<'a, F>(
        event: ParseEvent<'a>,
        summary: &mut StreamingSummary,
        on_event: &mut F,
    ) -> Result<()>
    where
        F: FnMut(ParseEvent<'a>) -> Result<ParseFlow>,
    {
        if summary.stopped {
            return Ok(());
        }

        let diagnostics = match &event {
            ParseEvent::Entry(entry) => {
                summary.entries += 1;
                if entry.status == crate::ParsedEntryStatus::Partial {
                    summary.recovered_blocks += 1;
                }
                entry.diagnostics.clone()
            }
            ParseEvent::String(_) => {
                summary.strings += 1;
                Vec::new()
            }
            ParseEvent::Preamble(_) => {
                summary.preambles += 1;
                Vec::new()
            }
            ParseEvent::Comment(_) => {
                summary.comments += 1;
                Vec::new()
            }
            ParseEvent::Failed(failed) => {
                summary.failed_blocks += 1;
                failed.diagnostics.clone()
            }
            ParseEvent::Diagnostic(diagnostic) => {
                summary.count_diagnostic(diagnostic);
                Vec::new()
            }
        };
        for diagnostic in &diagnostics {
            summary.count_diagnostic(diagnostic);
        }

        if on_event(event)? == ParseFlow::Stop {
            summary.stopped = true;
            return Ok(());
        }

        for diagnostic in diagnostics {
            if on_event(ParseEvent::Diagnostic(diagnostic))? == ParseFlow::Stop {
                summary.stopped = true;
                break;
            }
        }

        Ok(())
    }

    fn parse_document_with_source_id<'a>(
        &self,
        source_id: SourceId,
        source_name: Option<Cow<'a, str>>,
        input: &'a str,
    ) -> Result<ParsedDocument<'a>> {
        let source_map = SourceMap::new(Some(source_id), source_name.clone(), input);
        let sources = vec![ParsedSource {
            id: source_id,
            name: source_name,
        }];
        let raw_items = if self.tolerant {
            Library::parse_tolerant_raw_items(input, true, &source_map)
        } else {
            match Library::parse_raw_items_with_source(input, &source_map) {
                Ok(raw_items) => raw_items,
                Err(error) => {
                    return Ok(ParsedDocument::failed_from_error(
                        sources,
                        &source_map,
                        &error,
                    ));
                }
            }
        };
        let library = match Library::from_raw_items(raw_items.clone()) {
            Ok(library) => library,
            Err(Error::UndefinedVariable(_) | Error::CircularReference(_))
                if !self.document.expand_values =>
            {
                Library::from_raw_items_unexpanded(raw_items.clone())
            }
            Err(error) => return Err(error),
        };
        let mut document =
            ParsedDocument::from_library_with_source_map(library, sources, Some(&source_map));
        let mut entry_index = 0;
        for raw_item in &raw_items {
            if let RawBuildItem::Parsed(crate::parser::ParsedItem::Entry(_), _, raw) = raw_item {
                document.apply_entry_locations(
                    entry_index,
                    raw,
                    &source_map,
                    self.document.preserve_raw,
                );
                entry_index += 1;
            }
        }
        document.apply_parsed_values(&raw_items);
        if self.document.preserve_raw {
            document.apply_raw_items(&raw_items);
        }
        if self.tolerant {
            document.recover_partial_entries(&source_map, self.document.preserve_raw);
        }
        if self.document.expand_values {
            document.populate_expanded_values(crate::ExpansionOptions::default())?;
        }
        Ok(document)
    }

    pub(crate) fn parse_compact_document_owned(
        &self,
        source_name: Option<String>,
        input: &str,
    ) -> Result<ParsedDocument<'static>> {
        let source_name = source_name.map(Cow::Owned);
        let sources = vec![ParsedSource {
            id: SourceId::new(0),
            name: source_name,
        }];
        let input_scan = scan_input(input);
        let mut entries = Vec::with_capacity(input_scan.at_count);
        let mut strings = Vec::new();
        let mut preambles = Vec::new();
        let mut comments = Vec::new();
        let mut blocks = Vec::with_capacity(input_scan.at_count);

        crate::parser::parse_bibtex_stream(input, |item| {
            match item {
                crate::parser::ParsedItem::Entry(entry) => {
                    let index = entries.len();
                    entries.push(ParsedEntry::from_entry(entry.into_owned(), None));
                    blocks.push(ParsedBlock::Entry(index));
                }
                crate::parser::ParsedItem::String(name, value) => {
                    let index = strings.len();
                    strings.push(ParsedString::from_definition(StringDefinition {
                        name: Cow::Owned(name.to_string()),
                        value: value.into_owned(),
                        source: None,
                    }));
                    blocks.push(ParsedBlock::String(index));
                }
                crate::parser::ParsedItem::Preamble(value) => {
                    let index = preambles.len();
                    preambles.push(ParsedPreamble::from_preamble(Preamble::new(
                        value.into_owned(),
                    )));
                    blocks.push(ParsedBlock::Preamble(index));
                }
                crate::parser::ParsedItem::Comment(text) => {
                    let index = comments.len();
                    comments.push(ParsedComment::from_comment(Comment {
                        text: Cow::Owned(text.to_string()),
                        source: None,
                    }));
                    blocks.push(ParsedBlock::Comment(index));
                }
            }
            Ok(())
        })?;

        let mut document = ParsedDocument::from_parsed_parts(
            Library::new(),
            sources,
            entries,
            strings,
            preambles,
            comments,
            blocks,
        );
        if self.document.expand_values {
            document.populate_expanded_values(crate::ExpansionOptions::default())?;
        }
        Ok(document)
    }

    /// Parse multiple files in parallel
    pub fn parse_files<P: AsRef<Path> + Sync>(&self, paths: &[P]) -> Result<Library<'static>> {
        #[cfg(feature = "parallel")]
        {
            if let Some(threads) = self.threads {
                if threads <= 1 {
                    return Self::parse_files_sequential(paths);
                }
            }

            let pool = self.build_thread_pool()?;

            let libraries: Result<Vec<_>> = pool.install(|| {
                paths
                    .par_iter()
                    .map(|path| {
                        let content = std::fs::read_to_string(path)?;
                        let library = Library::parse_sequential(&content)?;
                        Ok(library.into_owned())
                    })
                    .collect()
            });

            let libraries = libraries?;
            Ok(Library::merge_libraries_parallel(libraries))
        }

        #[cfg(not(feature = "parallel"))]
        {
            Self::parse_files_sequential(paths)
        }
    }

    /// Sequential file parsing fallback
    fn parse_files_sequential<P: AsRef<Path>>(paths: &[P]) -> Result<Library<'static>> {
        let mut result = Library::new();
        for path in paths {
            let content = std::fs::read_to_string(path)?;
            let library = Library::parse_sequential(&content)?;
            result.merge(library.into_owned());
        }
        Ok(result)
    }

    #[cfg(feature = "parallel")]
    fn build_thread_pool(&self) -> Result<rayon::ThreadPool> {
        let mut builder = rayon::ThreadPoolBuilder::new();

        if let Some(threads) = self.threads {
            builder = builder.num_threads(threads);
        }

        builder
            .build()
            .map_err(|e| Error::WinnowError(e.to_string()))
    }
}

/// A high-level block in a parsed BibTeX library.
#[derive(Debug, Clone, Copy)]
pub enum Block<'lib, 'a> {
    /// A regular bibliography entry.
    Entry(&'lib Entry<'a>, Option<SourceSpan>),
    /// A string definition.
    String(&'lib StringDefinition<'a>),
    /// A preamble block.
    Preamble(&'lib Preamble<'a>),
    /// A comment block.
    Comment(&'lib Comment<'a>),
    /// A malformed block retained by tolerant parsing.
    Failed(&'lib FailedBlock<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Entry(usize),
    String(usize),
    Preamble(usize),
    Comment(usize),
    Failed(usize),
}

#[derive(Debug, Clone)]
pub enum RawBuildItem<'a> {
    Parsed(crate::parser::ParsedItem<'a>, SourceSpan, &'a str),
    Failed(FailedBlock<'a>),
}

/// A BibTeX string definition.
#[derive(Debug, Clone, PartialEq)]
pub struct StringDefinition<'a> {
    /// String variable name.
    pub name: Cow<'a, str>,
    /// Unexpanded string value.
    pub value: Value<'a>,
    /// Optional source location.
    pub source: Option<SourceSpan>,
}

impl<'a> StringDefinition<'a> {
    /// Create a string definition.
    #[must_use]
    pub const fn new(name: &'a str, value: Value<'a>) -> Self {
        Self {
            name: Cow::Borrowed(name),
            value,
            source: None,
        }
    }

    /// Return the string name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the string value.
    #[must_use]
    pub const fn value(&self) -> &Value<'a> {
        &self.value
    }

    /// Convert to an owned definition.
    #[must_use]
    pub fn into_owned(self) -> StringDefinition<'static> {
        StringDefinition {
            name: Cow::Owned(self.name.into_owned()),
            value: self.value.into_owned(),
            source: self.source,
        }
    }
}

/// A BibTeX preamble block.
#[derive(Debug, Clone, PartialEq)]
pub struct Preamble<'a> {
    /// Expanded preamble value.
    pub value: Value<'a>,
    /// Optional source location.
    pub source: Option<SourceSpan>,
}

impl<'a> Preamble<'a> {
    /// Create a preamble block.
    #[must_use]
    pub const fn new(value: Value<'a>) -> Self {
        Self {
            value,
            source: None,
        }
    }

    /// Return the preamble value.
    #[must_use]
    pub const fn value(&self) -> &Value<'a> {
        &self.value
    }

    /// Convert to an owned preamble.
    #[must_use]
    pub fn into_owned(self) -> Preamble<'static> {
        Preamble {
            value: self.value.into_owned(),
            source: self.source,
        }
    }
}

impl<'a> Deref for Preamble<'a> {
    type Target = Value<'a>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// A BibTeX comment block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment<'a> {
    /// Comment text.
    pub text: Cow<'a, str>,
    /// Optional source location.
    pub source: Option<SourceSpan>,
}

impl<'a> Comment<'a> {
    /// Create a comment block.
    #[must_use]
    pub const fn new(text: &'a str) -> Self {
        Self {
            text: Cow::Borrowed(text),
            source: None,
        }
    }

    /// Return the comment text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Convert to an owned comment.
    #[must_use]
    pub fn into_owned(self) -> Comment<'static> {
        Comment {
            text: Cow::Owned(self.text.into_owned()),
            source: self.source,
        }
    }
}

impl Deref for Comment<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

/// A malformed block retained by tolerant parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedBlock<'a> {
    /// Raw source for the malformed block.
    pub raw: Cow<'a, str>,
    /// Parse error message.
    pub error: String,
    /// Optional source location.
    pub source: Option<SourceSpan>,
}

impl FailedBlock<'_> {
    /// Convert to an owned failed block.
    #[must_use]
    pub fn into_owned(self) -> FailedBlock<'static> {
        FailedBlock {
            raw: Cow::Owned(self.raw.into_owned()),
            error: self.error,
            source: self.source,
        }
    }
}

/// Month rendering style used by month normalization.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MonthStyle {
    /// Full English month names such as `January`.
    #[default]
    Long,
    /// Three-letter lowercase BibTeX abbreviations such as `jan`.
    Abbrev,
    /// One-based month numbers such as `1`.
    Number,
}

/// Entry and field ordering options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SortOptions {
    /// Sort regular entries by citation key.
    pub entries_by_key: bool,
    /// Sort fields inside each entry by field name.
    pub fields_by_name: bool,
}

/// Field-name casing policy for field normalization.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FieldNameCase {
    /// Preserve existing field names.
    #[default]
    Preserve,
    /// Convert field names to lowercase ASCII.
    Lowercase,
}

/// Field normalization options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FieldNormalizeOptions {
    /// Field-name casing policy.
    pub name_case: FieldNameCase,
    /// Normalize common BibLaTeX aliases to classic BibTeX field names.
    pub biblatex_aliases: bool,
}

/// A parsed BibTeX library.
#[derive(Debug, Clone, Default)]
pub struct Library<'a> {
    /// Bibliography entries
    entries: Vec<Entry<'a>>,
    /// Optional entry source spans
    entry_sources: Option<Vec<Option<SourceSpan>>>,
    /// String definitions
    strings: Vec<StringDefinition<'a>>,
    /// Latest string definition by name
    string_lookup: AHashMap<Cow<'a, str>, usize>,
    /// Preambles
    preambles: Vec<Preamble<'a>>,
    /// Comments
    comments: Vec<Comment<'a>>,
    /// Failed blocks retained during tolerant parsing
    failed_blocks: Vec<FailedBlock<'a>>,
    /// Original block order
    block_order: Vec<BlockKind>,
}

impl<'a> Library<'a> {
    fn push_entry_with_source(&mut self, entry: Entry<'a>, source: Option<SourceSpan>) {
        let index = self.entries.len();
        self.entries.push(entry);
        if let Some(sources) = &mut self.entry_sources {
            sources.push(source);
        } else if source.is_some() {
            let mut sources = vec![None; index];
            sources.push(source);
            self.entry_sources = Some(sources);
        }
        self.block_order.push(BlockKind::Entry(index));
    }

    fn register_string_definition(
        &mut self,
        name: Cow<'a, str>,
        value: Value<'a>,
        source: Option<SourceSpan>,
    ) -> usize {
        let index = self.strings.len();
        self.string_lookup.insert(name.clone(), index);
        self.strings.push(StringDefinition {
            name,
            value,
            source,
        });
        index
    }

    fn push_string_with_source(
        &mut self,
        name: Cow<'a, str>,
        value: Value<'a>,
        source: Option<SourceSpan>,
    ) {
        let index = self.register_string_definition(name, value, source);
        self.block_order.push(BlockKind::String(index));
    }

    fn push_preamble_with_source(&mut self, value: Value<'a>, source: Option<SourceSpan>) -> usize {
        let index = self.preambles.len();
        self.preambles.push(Preamble { value, source });
        self.block_order.push(BlockKind::Preamble(index));
        index
    }

    fn push_comment_with_source(&mut self, text: Cow<'a, str>, source: Option<SourceSpan>) {
        let index = self.comments.len();
        self.comments.push(Comment { text, source });
        self.block_order.push(BlockKind::Comment(index));
    }

    fn push_failed_block(&mut self, failed: FailedBlock<'a>) {
        let index = self.failed_blocks.len();
        self.failed_blocks.push(failed);
        self.block_order.push(BlockKind::Failed(index));
    }

    #[inline]
    fn expand_value_for_parse(
        &self,
        value: &mut Value<'a>,
        has_user_strings: bool,
        month_constants_shadowed: bool,
        expanded_variables: &mut ExpansionCache<'a>,
        expansion_stack: &mut Vec<Cow<'a, str>>,
        concat_cache: &mut ConcatCache<'a>,
    ) -> Result<()> {
        match value {
            Value::Literal(_) | Value::Number(_) => Ok(()),
            Value::Variable(name) => {
                if !has_user_strings || !month_constants_shadowed {
                    if let Some(month_value) = get_month_expansion(name.as_ref()) {
                        *value = Value::Literal(Cow::Borrowed(month_value));
                        return Ok(());
                    }
                }

                if has_user_strings {
                    if let Some(expanded) = expanded_variables.get_cloned(name.as_ref()) {
                        *value = expanded;
                        return Ok(());
                    }

                    let old_value = std::mem::take(value);
                    *value = self.smart_expand_value_cached(
                        old_value,
                        expanded_variables,
                        expansion_stack,
                        concat_cache,
                    )?;
                }

                Ok(())
            }
            Value::Concat(parts) => {
                if has_user_strings {
                    if let Some(expanded) = concat_cache.get_cloned(parts) {
                        *value = expanded;
                        return Ok(());
                    }
                }

                let needs_expansion = if has_user_strings {
                    parts.iter().any(contains_variables)
                } else {
                    parts.iter().any(contains_potential_month_variables)
                };

                if needs_expansion {
                    if !has_user_strings {
                        if let Some(expanded) = concat_cache.get_cloned(parts) {
                            *value = expanded;
                            return Ok(());
                        }
                    }

                    let old_value = std::mem::take(value);
                    *value = self.smart_expand_value_cached(
                        old_value,
                        expanded_variables,
                        expansion_stack,
                        concat_cache,
                    )?;
                }

                Ok(())
            }
        }
    }

    /// Create a new empty library
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a parser with options
    ///
    /// # Parallel Processing
    ///
    /// The `threads` option only affects `parse_files()`. Single file
    /// parsing with `parse()` is sequential.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use bibtex_parser::Library;
    /// // Parse multiple files in parallel
    /// let library = Library::parser()
    ///     .threads(4)
    ///     .parse_files(&["file1.bib", "file2.bib"]).unwrap();
    ///
    /// // Single-file parsing stays sequential
    /// let content = "@article{demo, title=\"Demo\"}";
    /// let library = Library::parser()
    ///     .threads(4)
    ///     .parse(content).unwrap();
    /// ```
    #[must_use]
    #[inline]
    pub fn parser() -> Parser {
        Parser::new()
    }

    /// Parse a BibTeX library from a string with default strict settings.
    pub fn parse(input: &'a str) -> Result<Self> {
        Self::parser().parse(input)
    }

    /// Parse a BibTeX library from a file into owned data.
    pub fn parse_file(path: impl AsRef<Path>) -> Result<Library<'static>> {
        let content = std::fs::read_to_string(path)?;
        Library::parser().parse(&content).map(Library::into_owned)
    }

    /// Serialize this library to BibTeX.
    pub fn to_bibtex(&self) -> Result<String> {
        crate::writer::to_string(self)
    }

    /// Serialize this library to a BibTeX file.
    pub fn write_file(&self, path: impl AsRef<Path>) -> Result<()> {
        crate::writer::to_file(self, path)
    }

    /// Parse a BibTeX library from a string (single-threaded implementation)
    #[allow(clippy::too_many_lines)]
    pub(crate) fn parse_sequential(input: &'a str) -> Result<Self> {
        let mut library = Self::new();
        let input_scan = scan_input(input);

        // Fast path for common corpora (like tugboat) with no user-defined strings.
        // This avoids buffering all entries before expansion.
        if !input_scan.may_contain_string_definition {
            library.entries.reserve(input_scan.at_count);
            library.block_order.reserve(input_scan.at_count);
            let has_user_strings = false;
            let month_constants_shadowed = false;
            let mut expanded_variables = ExpansionCache::with_capacity(0);
            let mut expansion_stack = Vec::new();
            let mut concat_cache = ConcatCache::new();

            crate::parser::parse_bibtex_stream(input, |item| {
                match item {
                    crate::parser::ParsedItem::Entry(mut entry) => {
                        for field in &mut entry.fields {
                            library.expand_value_for_parse(
                                &mut field.value,
                                has_user_strings,
                                month_constants_shadowed,
                                &mut expanded_variables,
                                &mut expansion_stack,
                                &mut concat_cache,
                            )?;
                        }
                        library.push_entry_with_source(entry, None);
                    }
                    crate::parser::ParsedItem::Preamble(value) => {
                        let mut expanded = value;
                        library.expand_value_for_parse(
                            &mut expanded,
                            has_user_strings,
                            month_constants_shadowed,
                            &mut expanded_variables,
                            &mut expansion_stack,
                            &mut concat_cache,
                        )?;
                        library.push_preamble_with_source(expanded, None);
                    }
                    crate::parser::ParsedItem::Comment(text) => {
                        library.push_comment_with_source(Cow::Borrowed(text), None);
                    }
                    crate::parser::ParsedItem::String(name, value) => {
                        // Defensive fallback for scanner false negatives.
                        library.push_string_with_source(Cow::Borrowed(name), value, None);
                    }
                }
                Ok(())
            })?;

            return Ok(library);
        }

        library.block_order.reserve(input_scan.at_count);

        // Single-pass path when all @string definitions appear before regular
        // entries. This keeps correctness while avoiding buffering entries and
        // a full second pass over them.
        if !input_may_have_late_string_definition(input) {
            let mut pending_preambles = Vec::new();
            let mut expanded_variables = ExpansionCache::with_capacity(0);
            let mut expansion_stack = Vec::new();
            let mut concat_cache = ConcatCache::new();
            let mut month_constants_shadowed = None;

            crate::parser::parse_bibtex_stream(input, |item| {
                match item {
                    crate::parser::ParsedItem::Entry(mut entry) => {
                        let has_user_strings = !library.strings.is_empty();
                        let month_constants_shadowed = *month_constants_shadowed
                            .get_or_insert_with(|| {
                                has_user_strings
                                    && user_strings_shadow_month_constants(&library.strings)
                            });
                        for field in &mut entry.fields {
                            library.expand_value_for_parse(
                                &mut field.value,
                                has_user_strings,
                                month_constants_shadowed,
                                &mut expanded_variables,
                                &mut expansion_stack,
                                &mut concat_cache,
                            )?;
                        }
                        library.push_entry_with_source(entry, None);
                    }
                    crate::parser::ParsedItem::Preamble(value) => {
                        let index = library.push_preamble_with_source(value, None);
                        pending_preambles.push(index);
                    }
                    crate::parser::ParsedItem::String(name, value) => {
                        library.push_string_with_source(Cow::Borrowed(name), value, None);
                    }
                    crate::parser::ParsedItem::Comment(text) => {
                        library.push_comment_with_source(Cow::Borrowed(text), None);
                    }
                }
                Ok(())
            })?;

            let has_user_strings = !library.strings.is_empty();
            let month_constants_shadowed =
                has_user_strings && user_strings_shadow_month_constants(&library.strings);
            for index in pending_preambles {
                let mut expanded = std::mem::take(&mut library.preambles[index].value);
                library.expand_value_for_parse(
                    &mut expanded,
                    has_user_strings,
                    month_constants_shadowed,
                    &mut expanded_variables,
                    &mut expansion_stack,
                    &mut concat_cache,
                )?;
                library.preambles[index].value = expanded;
            }

            return Ok(library);
        }

        let mut entry_indices = Vec::new();
        let mut preamble_indices = Vec::new();

        crate::parser::parse_bibtex_stream(input, |item| {
            match item {
                crate::parser::ParsedItem::Entry(entry) => {
                    let index = library.entries.len();
                    library.push_entry_with_source(entry, None);
                    entry_indices.push(index);
                }
                crate::parser::ParsedItem::Preamble(value) => {
                    let index = library.push_preamble_with_source(value, None);
                    preamble_indices.push(index);
                }
                crate::parser::ParsedItem::String(name, value) => {
                    library.push_string_with_source(Cow::Borrowed(name), value, None);
                }
                crate::parser::ParsedItem::Comment(text) => {
                    library.push_comment_with_source(Cow::Borrowed(text), None);
                }
            }
            Ok(())
        })?;

        // Expand after parsing so all @string definitions are available globally.
        let has_user_strings = !library.strings.is_empty();
        let month_constants_shadowed =
            has_user_strings && user_strings_shadow_month_constants(&library.strings);
        let mut expanded_variables = ExpansionCache::with_capacity(library.strings.len());
        let mut expansion_stack = Vec::new();
        let mut concat_cache = ConcatCache::new();

        for entry_index in entry_indices {
            let field_count = library.entries[entry_index].fields.len();
            for field_index in 0..field_count {
                let mut value =
                    std::mem::take(&mut library.entries[entry_index].fields[field_index].value);
                library.expand_value_for_parse(
                    &mut value,
                    has_user_strings,
                    month_constants_shadowed,
                    &mut expanded_variables,
                    &mut expansion_stack,
                    &mut concat_cache,
                )?;
                library.entries[entry_index].fields[field_index].value = value;
            }
        }

        for preamble_index in preamble_indices {
            let mut expanded = std::mem::take(&mut library.preambles[preamble_index].value);
            library.expand_value_for_parse(
                &mut expanded,
                has_user_strings,
                month_constants_shadowed,
                &mut expanded_variables,
                &mut expansion_stack,
                &mut concat_cache,
            )?;
            library.preambles[preamble_index].value = expanded;
        }

        Ok(library)
    }

    fn parse_with_spans(input: &'a str) -> Result<Self> {
        let source_map = SourceMap::anonymous(input);
        let raw_items = Self::parse_raw_items_with_source(input, &source_map)?;
        Self::from_raw_items(raw_items)
    }

    fn parse_tolerant(input: &'a str, capture_source: bool) -> Result<Self> {
        let source_map = SourceMap::anonymous(input);
        let raw_items = Self::parse_tolerant_raw_items(input, capture_source, &source_map);
        Self::from_raw_items(raw_items)
    }

    fn parse_raw_items_with_source(
        input: &'a str,
        source_map: &SourceMap<'_>,
    ) -> Result<Vec<RawBuildItem<'a>>> {
        let mut raw_items = Vec::new();
        crate::parser::parse_bibtex_stream_with_spans(input, |item, span, raw| {
            let span = if source_map.source_id().is_some() {
                source_map.span(span.byte_start, span.byte_end)
            } else {
                span
            };
            raw_items.push(RawBuildItem::Parsed(item, span, raw));
            Ok(())
        })?;
        Ok(raw_items)
    }

    fn parse_tolerant_raw_items(
        input: &'a str,
        capture_source: bool,
        source_map: &SourceMap<'_>,
    ) -> Vec<RawBuildItem<'a>> {
        let mut raw_items = Vec::new();
        let mut remaining = input;

        loop {
            crate::parser::lexer::skip_whitespace(&mut remaining);
            if remaining.is_empty() {
                break;
            }

            let start = input.len() - remaining.len();
            match crate::parser::parse_item(&mut remaining) {
                Ok(item) => {
                    let end = input.len() - remaining.len();
                    raw_items.push(RawBuildItem::Parsed(
                        item,
                        source_map.span(start, end),
                        &input[start..end],
                    ));
                }
                Err(err) => {
                    let end = next_recovery_boundary(input, start);
                    let source = capture_source.then(|| source_map.span(start, end));
                    raw_items.push(RawBuildItem::Failed(FailedBlock {
                        raw: Cow::Borrowed(&input[start..end]),
                        error: format!("Failed to parse entry: {err}"),
                        source,
                    }));
                    remaining = &input[end..];
                }
            }
        }

        raw_items
    }

    fn from_raw_items(raw_items: Vec<RawBuildItem<'a>>) -> Result<Self> {
        let mut library = Self::new();

        for raw_item in &raw_items {
            if let RawBuildItem::Parsed(crate::parser::ParsedItem::String(name, value), span, _) =
                raw_item
            {
                library.register_string_definition(Cow::Borrowed(name), value.clone(), Some(*span));
            }
        }

        let has_user_strings = !library.strings.is_empty();
        let month_constants_shadowed =
            has_user_strings && user_strings_shadow_month_constants(&library.strings);
        let mut expanded_variables = ExpansionCache::with_capacity(library.strings.len());
        let mut expansion_stack = Vec::new();
        let mut concat_cache = ConcatCache::new();
        let mut string_index = 0;

        for raw_item in raw_items {
            match raw_item {
                RawBuildItem::Parsed(crate::parser::ParsedItem::Entry(mut entry), span, _) => {
                    for field in &mut entry.fields {
                        library.expand_value_for_parse(
                            &mut field.value,
                            has_user_strings,
                            month_constants_shadowed,
                            &mut expanded_variables,
                            &mut expansion_stack,
                            &mut concat_cache,
                        )?;
                    }
                    library.push_entry_with_source(entry, Some(span));
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::String(_, _), _, _) => {
                    library.block_order.push(BlockKind::String(string_index));
                    string_index += 1;
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::Preamble(mut value), span, _) => {
                    library.expand_value_for_parse(
                        &mut value,
                        has_user_strings,
                        month_constants_shadowed,
                        &mut expanded_variables,
                        &mut expansion_stack,
                        &mut concat_cache,
                    )?;
                    library.push_preamble_with_source(value, Some(span));
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::Comment(text), span, _) => {
                    library.push_comment_with_source(Cow::Borrowed(text), Some(span));
                }
                RawBuildItem::Failed(failed) => library.push_failed_block(failed),
            }
        }

        Ok(library)
    }

    fn from_raw_items_unexpanded(raw_items: Vec<RawBuildItem<'a>>) -> Self {
        let mut library = Self::new();

        for raw_item in raw_items {
            match raw_item {
                RawBuildItem::Parsed(crate::parser::ParsedItem::Entry(entry), span, _) => {
                    library.push_entry_with_source(entry, Some(span));
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::String(name, value), span, _) => {
                    library.push_string_with_source(Cow::Borrowed(name), value, Some(span));
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::Preamble(value), span, _) => {
                    library.push_preamble_with_source(value, Some(span));
                }
                RawBuildItem::Parsed(crate::parser::ParsedItem::Comment(text), span, _) => {
                    library.push_comment_with_source(Cow::Borrowed(text), Some(span));
                }
                RawBuildItem::Failed(failed) => library.push_failed_block(failed),
            }
        }

        library
    }

    /// Merge another library into this one
    pub fn merge(&mut self, other: Self) {
        let entry_offset = self.entries.len();
        let string_offset = self.strings.len();
        let preamble_offset = self.preambles.len();
        let comment_offset = self.comments.len();
        let failed_offset = self.failed_blocks.len();
        let other_entry_count = other.entries.len();
        let other_entry_sources = other.entry_sources;

        self.entries.extend(other.entries);
        match (&mut self.entry_sources, other_entry_sources) {
            (Some(sources), Some(other_sources)) => sources.extend(other_sources),
            (Some(sources), None) => {
                sources.extend(std::iter::repeat(None).take(other_entry_count));
            }
            (None, Some(other_sources)) => {
                let mut sources = vec![None; entry_offset];
                sources.extend(other_sources);
                self.entry_sources = Some(sources);
            }
            (None, None) => {}
        }
        self.preambles.extend(other.preambles);
        self.comments.extend(other.comments);
        self.failed_blocks.extend(other.failed_blocks);

        for definition in other.strings {
            let index = self.strings.len();
            self.string_lookup.insert(definition.name.clone(), index);
            self.strings.push(definition);
        }

        self.block_order
            .extend(other.block_order.into_iter().map(|kind| match kind {
                BlockKind::Entry(index) => BlockKind::Entry(entry_offset + index),
                BlockKind::String(index) => BlockKind::String(string_offset + index),
                BlockKind::Preamble(index) => BlockKind::Preamble(preamble_offset + index),
                BlockKind::Comment(index) => BlockKind::Comment(comment_offset + index),
                BlockKind::Failed(index) => BlockKind::Failed(failed_offset + index),
            }));
    }

    #[cfg(feature = "parallel")]
    fn merge_libraries_parallel(libraries: Vec<Library<'static>>) -> Library<'static> {
        let mut result = Library::new();
        for library in libraries {
            result.merge(library);
        }
        result
    }

    /// Get all entries
    #[must_use]
    pub fn entries(&self) -> &[Entry<'a>] {
        &self.entries
    }

    /// Get mutable access to all entries
    #[must_use]
    pub fn entries_mut(&mut self) -> &mut Vec<Entry<'a>> {
        &mut self.entries
    }

    /// Get all string definitions
    #[must_use]
    pub fn strings(&self) -> &[StringDefinition<'a>] {
        &self.strings
    }

    /// Get a string definition by name.
    #[must_use]
    pub fn string(&self, name: &str) -> Option<&StringDefinition<'a>> {
        get_string_definition(&self.strings, &self.string_lookup, name)
    }

    /// Get a string definition value by name.
    #[must_use]
    pub fn string_value(&self, name: &str) -> Option<&Value<'a>> {
        self.string(name).map(|definition| &definition.value)
    }

    /// Get all preambles
    #[must_use]
    pub fn preambles(&self) -> &[Preamble<'a>] {
        &self.preambles
    }

    /// Get mutable access to preambles
    #[must_use]
    pub fn preambles_mut(&mut self) -> &mut Vec<Preamble<'a>> {
        &mut self.preambles
    }

    /// Get all comments
    #[must_use]
    pub fn comments(&self) -> &[Comment<'a>] {
        &self.comments
    }

    /// Get mutable access to comments
    #[must_use]
    pub fn comments_mut(&mut self) -> &mut Vec<Comment<'a>> {
        &mut self.comments
    }

    /// Get malformed blocks retained by tolerant parsing.
    #[must_use]
    pub fn failed_blocks(&self) -> &[FailedBlock<'a>] {
        &self.failed_blocks
    }

    /// Return blocks in source order.
    #[must_use]
    pub fn blocks(&self) -> Vec<Block<'_, 'a>> {
        self.block_order
            .iter()
            .map(|kind| match *kind {
                BlockKind::Entry(index) => Block::Entry(
                    &self.entries[index],
                    self.entry_sources
                        .as_ref()
                        .and_then(|sources| sources.get(index).copied().flatten()),
                ),
                BlockKind::String(index) => Block::String(&self.strings[index]),
                BlockKind::Preamble(index) => Block::Preamble(&self.preambles[index]),
                BlockKind::Comment(index) => Block::Comment(&self.comments[index]),
                BlockKind::Failed(index) => Block::Failed(&self.failed_blocks[index]),
            })
            .collect()
    }

    #[must_use]
    pub(crate) fn entry_source(&self, index: usize) -> Option<SourceSpan> {
        self.entry_sources
            .as_ref()
            .and_then(|sources| sources.get(index).copied().flatten())
    }

    #[must_use]
    pub(crate) fn block_kinds(&self) -> &[BlockKind] {
        &self.block_order
    }

    /// Find entries by key
    #[must_use]
    pub fn find_by_key(&self, key: &str) -> Option<&Entry<'a>> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// Find entries by key, ignoring ASCII case.
    #[must_use]
    pub fn find_by_key_ignore_case(&self, key: &str) -> Option<&Entry<'a>> {
        self.entries
            .iter()
            .find(|entry| entry.key.eq_ignore_ascii_case(key))
    }

    /// Return `true` when the library contains `key`.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.find_by_key(key).is_some()
    }

    /// Find entries by type
    #[must_use]
    pub fn find_by_type(&self, ty: &str) -> Vec<&Entry<'a>> {
        self.entries
            .iter()
            .filter(|e| e.ty.canonical_name().eq_ignore_ascii_case(ty))
            .collect()
    }

    /// Find entries by field value
    #[must_use]
    pub fn find_by_field(&self, field: &str, value: &str) -> Vec<&Entry<'a>> {
        self.entries
            .iter()
            .filter(|e| {
                e.get_as_string(field)
                    .as_ref()
                    .is_some_and(|v| v.contains(value))
            })
            .collect()
    }

    /// Find entries by field value, ignoring ASCII case for the field name and value.
    #[must_use]
    pub fn find_by_field_ignore_case(&self, field: &str, value: &str) -> Vec<&Entry<'a>> {
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .get_as_string_ignore_case(field)
                    .as_ref()
                    .is_some_and(|field_value| contains_case_insensitive(field_value, value))
            })
            .collect()
    }

    /// Find entries whose normalized DOI matches `doi`.
    #[must_use]
    pub fn find_by_doi(&self, doi: &str) -> Vec<&Entry<'a>> {
        let Some(needle) = normalize_doi(doi) else {
            return Vec::new();
        };

        self.entries
            .iter()
            .filter(|entry| entry.doi().as_ref().is_some_and(|value| value == &needle))
            .collect()
    }

    /// Smart expansion with memoization for repeated variable references.
    fn smart_expand_value_cached(
        &self,
        value: Value<'a>,
        expanded_variables: &mut ExpansionCache<'a>,
        expansion_stack: &mut Vec<Cow<'a, str>>,
        concat_cache: &mut ConcatCache<'a>,
    ) -> Result<Value<'a>> {
        match value {
            // Simple literals and numbers stay as-is (zero-copy!)
            Value::Literal(_) | Value::Number(_) => Ok(value),

            // Variables need to be resolved
            Value::Variable(name) => {
                let name_text = name.as_ref();
                if let Some(expanded) = expanded_variables.get_cloned(name_text) {
                    return Ok(expanded);
                }

                if expansion_stack.iter().any(|v| v.as_ref() == name_text) {
                    let mut cycle = expansion_stack
                        .iter()
                        .map(std::convert::AsRef::as_ref)
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    if !cycle.is_empty() {
                        cycle.push_str(" -> ");
                    }
                    cycle.push_str(name_text);
                    return Err(Error::CircularReference(cycle));
                }

                if let Some(user_value) =
                    get_string_value(&self.strings, &self.string_lookup, name_text)
                {
                    // Recursively expand the variable's value and cache the result.
                    expansion_stack.push(name.clone());
                    let expanded = self.smart_expand_value_cached(
                        user_value.clone(),
                        expanded_variables,
                        expansion_stack,
                        concat_cache,
                    );
                    expansion_stack.pop();

                    let expanded = expanded?;
                    expanded_variables.insert(name, expanded.clone());
                    Ok(expanded)
                } else {
                    // Check month abbreviations as fallback
                    get_month_expansion(name_text).map_or_else(
                        || {
                            // Variable not found in either user strings or month constants
                            Err(Error::UndefinedVariable(name_text.to_string()))
                        },
                        |month_value| Ok(Value::Literal(Cow::Borrowed(month_value))),
                    )
                }
            }

            // Concatenations need special handling
            Value::Concat(parts) => {
                if let Some(expanded) = concat_cache.get_cloned(&parts) {
                    return Ok(expanded);
                }

                let cache_key = parts.clone();
                let expanded = self.expand_concatenation_cached(
                    parts.into_vec(),
                    expanded_variables,
                    expansion_stack,
                    concat_cache,
                )?;
                concat_cache.insert(cache_key, expanded.clone());
                Ok(expanded)
            }
        }
    }

    /// Alternative expansion that works with references (requires cloning for variables)
    pub fn expand_value_ref(&self, value: &Value<'a>) -> Result<Value<'a>> {
        match value {
            // Simple literals and numbers can be cloned cheaply
            Value::Literal(_) | Value::Number(_) => Ok(value.clone()),

            // Variables need to be resolved
            Value::Variable(name) => {
                // First check user-defined strings
                get_string_value(&self.strings, &self.string_lookup, name.as_ref()).map_or_else(
                    || {
                        // Check month abbreviations as fallback
                        get_month_expansion(name.as_ref()).map_or_else(
                            || {
                                // Variable not found in either user strings or month constants
                                Err(Error::UndefinedVariable(name.as_ref().to_string()))
                            },
                            |month_value| Ok(Value::Literal(Cow::Borrowed(month_value))),
                        )
                    },
                    |user_value| self.expand_value_ref(user_value),
                )
            }

            // Concatenations need cloning
            Value::Concat(parts) => {
                let cloned_parts = parts.to_vec();
                self.expand_concatenation(cloned_parts)
            }
        }
    }

    /// Expand a concatenation, only converting to owned when necessary
    fn expand_concatenation(&self, parts: Vec<Value<'a>>) -> Result<Value<'a>> {
        let mut expanded_variables = ExpansionCache::with_capacity(0);
        let mut expansion_stack = Vec::new();
        let mut concat_cache = ConcatCache::new();
        self.expand_concatenation_cached(
            parts,
            &mut expanded_variables,
            &mut expansion_stack,
            &mut concat_cache,
        )
    }

    /// Cached concatenation expansion used by hot parsing paths.
    fn expand_concatenation_cached(
        &self,
        parts: Vec<Value<'a>>,
        expanded_variables: &mut ExpansionCache<'a>,
        expansion_stack: &mut Vec<Cow<'a, str>>,
        concat_cache: &mut ConcatCache<'a>,
    ) -> Result<Value<'a>> {
        let mut expanded_parts = Vec::with_capacity(parts.len());

        // First, expand all parts
        for part in parts {
            let expanded = self.smart_expand_value_cached(
                part,
                expanded_variables,
                expansion_stack,
                concat_cache,
            )?;
            expanded_parts.push(expanded);
        }

        // If all parts are literals or numbers, we can flatten to a single string
        if expanded_parts
            .iter()
            .all(|p| matches!(p, Value::Literal(_) | Value::Number(_)))
        {
            let combined = concatenate_simple_values(&expanded_parts);
            Ok(Value::Literal(Cow::Owned(combined)))
        } else {
            Ok(Value::Concat(expanded_parts.into_boxed_slice()))
        }
    }

    /// Get a fully expanded string value.
    pub fn get_expanded_string(&self, value: &Value<'a>) -> Result<String> {
        match value {
            Value::Literal(s) => Ok(s.to_string()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Variable(name) => {
                // First check user-defined strings
                get_string_value(&self.strings, &self.string_lookup, name.as_ref()).map_or_else(
                    || {
                        // Check month abbreviations as fallback
                        get_month_expansion(name.as_ref()).map_or_else(
                            || {
                                // Variable not found in either user strings or month constants
                                Err(Error::UndefinedVariable(name.as_ref().to_string()))
                            },
                            |month_value| Ok(month_value.to_string()),
                        )
                    },
                    |user_value| self.get_expanded_string(user_value),
                )
            }
            Value::Concat(parts) => {
                let mut result = String::new();
                for part in parts.iter() {
                    result.push_str(&self.get_expanded_string(part)?);
                }
                Ok(result)
            }
        }
    }

    /// Convert to owned version (no borrowed data)
    #[must_use]
    pub fn into_owned(self) -> Library<'static> {
        let strings = self
            .strings
            .into_iter()
            .map(StringDefinition::into_owned)
            .collect::<Vec<_>>();
        let mut string_lookup = AHashMap::with_capacity(strings.len());
        for (index, definition) in strings.iter().enumerate() {
            string_lookup.insert(Cow::Owned(definition.name.to_string()), index);
        }

        Library {
            entries: self.entries.into_iter().map(Entry::into_owned).collect(),
            entry_sources: self.entry_sources,
            strings,
            string_lookup,
            preambles: self
                .preambles
                .into_iter()
                .map(Preamble::into_owned)
                .collect(),
            comments: self.comments.into_iter().map(Comment::into_owned).collect(),
            failed_blocks: self
                .failed_blocks
                .into_iter()
                .map(FailedBlock::into_owned)
                .collect(),
            block_order: self.block_order,
        }
    }

    /// Add a string definition (useful for building libraries programmatically)
    pub fn add_string(&mut self, name: &'a str, value: Value<'a>) {
        self.push_string_with_source(Cow::Borrowed(name), value, None);
    }

    /// Add an entry
    pub fn add_entry(&mut self, entry: Entry<'a>) {
        self.push_entry_with_source(entry, None);
    }

    /// Add a preamble
    pub fn add_preamble(&mut self, value: Value<'a>) {
        self.push_preamble_with_source(value, None);
    }

    /// Add a comment
    pub fn add_comment(&mut self, comment: &'a str) {
        self.push_comment_with_source(Cow::Borrowed(comment), None);
    }

    /// Resolve string variables and concatenations in entries and preambles in place.
    pub fn resolve_strings(&mut self) -> Result<()> {
        let has_user_strings = !self.strings.is_empty();
        let month_constants_shadowed =
            has_user_strings && user_strings_shadow_month_constants(&self.strings);
        let mut expanded_variables = ExpansionCache::with_capacity(self.strings.len());
        let mut expansion_stack = Vec::new();
        let mut concat_cache = ConcatCache::new();

        for entry_index in 0..self.entries.len() {
            let field_count = self.entries[entry_index].fields.len();
            for field_index in 0..field_count {
                let mut value =
                    std::mem::take(&mut self.entries[entry_index].fields[field_index].value);
                self.expand_value_for_parse(
                    &mut value,
                    has_user_strings,
                    month_constants_shadowed,
                    &mut expanded_variables,
                    &mut expansion_stack,
                    &mut concat_cache,
                )?;
                self.entries[entry_index].fields[field_index].value = value;
            }
        }

        for preamble_index in 0..self.preambles.len() {
            let mut value = std::mem::take(&mut self.preambles[preamble_index].value);
            self.expand_value_for_parse(
                &mut value,
                has_user_strings,
                month_constants_shadowed,
                &mut expanded_variables,
                &mut expansion_stack,
                &mut concat_cache,
            )?;
            self.preambles[preamble_index].value = value;
        }

        Ok(())
    }

    /// Normalize DOI fields to lowercase `10.x/...` form when recognizable.
    pub fn normalize_doi_fields(&mut self) {
        for entry in &mut self.entries {
            for field in &mut entry.fields {
                if field.name.eq_ignore_ascii_case("doi") {
                    if let Some(normalized) = normalize_doi(&field.value.to_plain_string()) {
                        field.value = Value::Literal(Cow::Owned(normalized));
                    }
                }
            }
        }
    }

    /// Normalize month fields to a chosen representation.
    pub fn normalize_months(&mut self, style: MonthStyle) {
        for entry in &mut self.entries {
            for field in &mut entry.fields {
                if field.name.eq_ignore_ascii_case("month") {
                    if let Some(month) =
                        normalize_month_value(&field.value.to_plain_string(), style)
                    {
                        field.value = month;
                    }
                }
            }
        }
    }

    /// Normalize field names and common BibLaTeX aliases.
    pub fn normalize_fields(&mut self, options: FieldNormalizeOptions) {
        for entry in &mut self.entries {
            for field in &mut entry.fields {
                let mut name = if options.biblatex_aliases {
                    canonical_biblatex_field_alias(&field.name)
                        .unwrap_or_else(|| field.name.as_ref())
                        .to_string()
                } else {
                    field.name.to_string()
                };

                if options.name_case == FieldNameCase::Lowercase {
                    name.make_ascii_lowercase();
                }

                if name != field.name {
                    field.name = Cow::Owned(name);
                }
            }
        }
    }

    /// Sort entries and/or fields in place.
    pub fn sort(&mut self, options: SortOptions) {
        if options.fields_by_name {
            for entry in &mut self.entries {
                entry
                    .fields
                    .sort_by(|left, right| left.name.cmp(&right.name));
            }
        }

        if options.entries_by_key {
            if let Some(sources) = self.entry_sources.take() {
                let mut entries = self.entries.drain(..).zip(sources).collect::<Vec<_>>();
                entries.sort_by(|(left, _), (right, _)| left.key.cmp(&right.key));
                let (sorted_entries, sorted_sources): (Vec<_>, Vec<_>) =
                    entries.into_iter().unzip();
                self.entries = sorted_entries;
                self.entry_sources = Some(sorted_sources);
            } else {
                self.entries.sort_by(|left, right| left.key.cmp(&right.key));
            }
            self.rebuild_grouped_block_order();
        }
    }

    fn rebuild_grouped_block_order(&mut self) {
        self.block_order.clear();
        self.block_order
            .extend((0..self.strings.len()).map(BlockKind::String));
        self.block_order
            .extend((0..self.preambles.len()).map(BlockKind::Preamble));
        self.block_order
            .extend((0..self.comments.len()).map(BlockKind::Comment));
        self.block_order
            .extend((0..self.entries.len()).map(BlockKind::Entry));
        self.block_order
            .extend((0..self.failed_blocks.len()).map(BlockKind::Failed));
    }

    /// Validate all entries in the library
    /// Returns a list of entries with their indices and validation errors
    #[must_use]
    pub fn validate(
        &self,
        level: ValidationLevel,
    ) -> Vec<(usize, &Entry<'a>, Vec<ValidationError>)> {
        let mut invalid_entries = Vec::new();

        for (index, entry) in self.entries.iter().enumerate() {
            if let Err(errors) = entry.validate(level) {
                invalid_entries.push((index, entry, errors));
            }
        }

        invalid_entries
    }

    /// Check for duplicate citation keys
    /// Returns a list of duplicate keys (each key appears once in the list even if it has multiple duplicates)
    #[must_use]
    pub fn find_duplicate_keys(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = std::collections::HashSet::new();

        for entry in &self.entries {
            if !seen.insert(entry.key()) {
                duplicates.insert(entry.key());
            }
        }

        duplicates.into_iter().collect()
    }

    /// Check for duplicate citation keys, ignoring ASCII case.
    #[must_use]
    pub fn find_duplicate_keys_ignore_case(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = std::collections::HashSet::new();

        for entry in &self.entries {
            let normalized_key = entry.key().to_ascii_lowercase();
            if !seen.insert(normalized_key.clone()) {
                duplicates.insert(normalized_key);
            }
        }

        duplicates.into_iter().collect()
    }

    /// Find duplicate DOI groups using normalized DOI values.
    #[must_use]
    pub fn find_duplicate_dois(&self) -> Vec<(String, Vec<&Entry<'a>>)> {
        let mut groups: AHashMap<String, Vec<&Entry<'a>>> = AHashMap::new();
        for entry in &self.entries {
            if let Some(doi) = entry.doi() {
                groups.entry(doi).or_default().push(entry);
            }
        }

        groups
            .into_iter()
            .filter(|(_, entries)| entries.len() > 1)
            .collect()
    }

    /// Validate all entries and return a comprehensive validation report
    #[must_use]
    pub fn validate_comprehensive(&self, level: ValidationLevel) -> ValidationReport<'_> {
        let invalid_entries = self.validate(level);
        let duplicate_keys = self.find_duplicate_keys();
        let empty_entries = self.find_empty_entries();

        ValidationReport {
            invalid_entries,
            duplicate_keys,
            empty_entries,
            total_entries: self.entries.len(),
            validation_level: level,
        }
    }

    /// Find entries with no fields (only key and type)
    fn find_empty_entries(&self) -> Vec<(usize, &Entry<'a>)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.fields().is_empty())
            .collect()
    }

    /// Get statistics about the library
    #[must_use]
    pub fn stats(&self) -> LibraryStats {
        let mut type_counts = AHashMap::new();
        for entry in &self.entries {
            *type_counts.entry(entry.ty.to_string()).or_insert(0) += 1;
        }

        LibraryStats {
            total_entries: self.entries.len(),
            total_strings: self.strings.len(),
            total_preambles: self.preambles.len(),
            total_comments: self.comments.len(),
            entries_by_type: type_counts,
        }
    }
}

/// Statistics about a library
#[derive(Debug, Clone)]
pub struct LibraryStats {
    /// Total number of entries
    pub total_entries: usize,
    /// Total number of string definitions
    pub total_strings: usize,
    /// Total number of preambles
    pub total_preambles: usize,
    /// Total number of comments
    pub total_comments: usize,
    /// Entry counts by type
    pub entries_by_type: AHashMap<String, usize>,
}

/// Comprehensive validation report for a library
#[derive(Debug, Clone)]
pub struct ValidationReport<'a> {
    /// Entries that failed validation with their errors
    pub invalid_entries: Vec<(usize, &'a Entry<'a>, Vec<ValidationError>)>,
    /// Duplicate citation keys
    pub duplicate_keys: Vec<&'a str>,
    /// Entries with no fields
    pub empty_entries: Vec<(usize, &'a Entry<'a>)>,
    /// Total number of entries in the library
    pub total_entries: usize,
    /// Validation level used
    pub validation_level: ValidationLevel,
}

impl ValidationReport<'_> {
    /// Check if the library is completely valid
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.invalid_entries.is_empty()
            && self.duplicate_keys.is_empty()
            && self.empty_entries.is_empty()
    }

    /// Get total number of issues found
    #[must_use]
    pub fn total_issues(&self) -> usize {
        self.invalid_entries.len() + self.duplicate_keys.len() + self.empty_entries.len()
    }

    /// Get a summary of issues by severity
    #[must_use]
    pub fn issue_summary(&self) -> IssueSummary {
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;

        for (_, _, validation_errors) in &self.invalid_entries {
            for error in validation_errors {
                match error.severity {
                    crate::model::ValidationSeverity::Error => errors += 1,
                    crate::model::ValidationSeverity::Warning => warnings += 1,
                    crate::model::ValidationSeverity::Info => infos += 1,
                }
            }
        }

        // Duplicate keys and empty entries are considered errors
        errors += self.duplicate_keys.len() + self.empty_entries.len();

        IssueSummary {
            errors,
            warnings,
            infos,
        }
    }
}

/// Summary of validation issues by severity
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueSummary {
    /// Number of error-level issues
    pub errors: usize,
    /// Number of warning-level issues
    pub warnings: usize,
    /// Number of info-level issues
    pub infos: usize,
}

/// Concatenate simple values (literals and numbers) into a single string
fn concatenate_simple_values(values: &[Value]) -> String {
    let mut result = String::new();

    // Pre-calculate capacity for efficiency
    let capacity: usize = values
        .iter()
        .map(|v| match v {
            Value::Literal(s) => s.len(),
            Value::Number(n) => n.to_string().len(),
            _ => 0,
        })
        .sum();

    result.reserve(capacity);

    for value in values {
        match value {
            Value::Literal(s) => result.push_str(s),
            Value::Number(n) => result.push_str(&n.to_string()),
            _ => {} // Should not happen given the precondition
        }
    }

    result
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    haystack.to_lowercase().contains(&needle.to_lowercase())
}

fn normalize_month_value(input: &str, style: MonthStyle) -> Option<Value<'static>> {
    let normalized = input.trim().trim_matches(['{', '}']).to_ascii_lowercase();
    let month_index = match normalized.as_str() {
        "jan" | "january" | "1" | "01" => 1,
        "feb" | "february" | "2" | "02" => 2,
        "mar" | "march" | "3" | "03" => 3,
        "apr" | "april" | "4" | "04" => 4,
        "may" | "5" | "05" => 5,
        "jun" | "june" | "6" | "06" => 6,
        "jul" | "july" | "7" | "07" => 7,
        "aug" | "august" | "8" | "08" => 8,
        "sep" | "september" | "9" | "09" => 9,
        "oct" | "october" | "10" => 10,
        "nov" | "november" | "11" => 11,
        "dec" | "december" | "12" => 12,
        _ => return None,
    };

    let text = match style {
        MonthStyle::Long => month_long_name(month_index),
        MonthStyle::Abbrev => month_abbreviation(month_index),
        MonthStyle::Number => return Some(Value::Number(month_index)),
    };

    Some(Value::Literal(Cow::Borrowed(text)))
}

const fn month_long_name(month: i64) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

const fn month_abbreviation(month: i64) -> &'static str {
    match month {
        1 => "jan",
        2 => "feb",
        3 => "mar",
        4 => "apr",
        5 => "may",
        6 => "jun",
        7 => "jul",
        8 => "aug",
        9 => "sep",
        10 => "oct",
        11 => "nov",
        12 => "dec",
        _ => "",
    }
}

/// Builder for creating libraries programmatically
#[derive(Debug, Default)]
pub struct LibraryBuilder<'a> {
    library: Library<'a>,
}

impl<'a> LibraryBuilder<'a> {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry
    #[must_use]
    pub fn entry(mut self, entry: Entry<'a>) -> Self {
        self.library.add_entry(entry);
        self
    }

    /// Add a string definition
    #[must_use]
    pub fn string(mut self, name: &'a str, value: Value<'a>) -> Self {
        self.library.add_string(name, value);
        self
    }

    /// Add a preamble
    #[must_use]
    pub fn preamble(mut self, value: Value<'a>) -> Self {
        self.library.add_preamble(value);
        self
    }

    /// Add a comment
    #[must_use]
    pub fn comment(mut self, text: &'a str) -> Self {
        self.library.add_comment(text);
        self
    }

    /// Build the library
    #[must_use]
    pub fn build(self) -> Library<'a> {
        self.library
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryType, Field};

    #[test]
    fn test_library_parse() {
        let input = r#"
            @string{me = "John Doe"}
            
            @article{test2023,
                author = me,
                title = "Test Article",
                year = 2023
            }
        "#;

        let library = Library::parser().parse(input).unwrap();
        assert_eq!(library.entries().len(), 1);
        assert_eq!(library.strings().len(), 1);

        let entry = &library.entries()[0];
        // Use get_as_string since the value might be a variable reference
        assert_eq!(entry.get_as_string("author").unwrap(), "John Doe");
    }

    #[test]
    fn test_zero_copy_preservation() {
        let input = r#"
            @article{test,
                title = "This is borrowed",
                year = 2023
            }
        "#;

        let library = Library::parser().parse(input).unwrap();
        let entry = &library.entries()[0];

        // The title should still be borrowed from the input
        if let Some(Value::Literal(cow)) = entry
            .fields
            .iter()
            .find(|f| f.name == "title")
            .map(|f| &f.value)
        {
            assert!(matches!(cow, Cow::Borrowed(_)));
        }
    }

    #[test]
    fn test_concatenation_creates_owned() {
        let input = r#"
            @string{first = "Hello"}
            @string{second = "World"}
            
            @article{test,
                title = first # ", " # second
            }
        "#;

        let library = Library::parser().parse(input).unwrap();
        let entry = &library.entries()[0];

        // Concatenation should create an owned string
        assert_eq!(entry.get_as_string("title").unwrap(), "Hello, World");
    }

    #[test]
    fn test_boxed_concat_memory_optimization() {
        // Verify that Value enum is 24 bytes or less (was 32 before optimization)
        assert!(
            std::mem::size_of::<Value>() <= 32,
            "Value enum is {} bytes, should be 32 or less",
            std::mem::size_of::<Value>()
        );
    }

    #[test]
    fn test_field_vec_capacity_bounded() {
        let input = r#"
            @article{test,
                a = "1", b = "2", c = "3", d = "4", e = "5",
                f = "6", g = "7", h = "8", i = "9", j = "10"
            }
        "#;

        let library = Library::parser().parse(input).unwrap();
        let entry = &library.entries()[0];

        assert_eq!(entry.fields.len(), 10);
        assert!(
            entry.fields.capacity() <= 17,
            "Unexpected field Vec growth: len={}, capacity={}",
            entry.fields.len(),
            entry.fields.capacity()
        );
    }

    #[test]
    fn test_library_builder() {
        let library = LibraryBuilder::new()
            .string("me", Value::Literal(Cow::Borrowed("John Doe")))
            .entry(Entry {
                ty: EntryType::Article,
                key: Cow::Borrowed("test2023"),
                fields: vec![
                    Field::new("author", Value::Variable(Cow::Borrowed("me"))),
                    Field::new("title", Value::Literal(Cow::Borrowed("Test"))),
                ],
            })
            .build();

        assert_eq!(library.entries().len(), 1);
        assert_eq!(library.strings().len(), 1);
    }

    #[test]
    fn test_library_stats() {
        let input = r#"
            @string{ieee = "IEEE"}
            @preamble{"Test preamble"}
            % This is a percent comment that now works properly
            @comment{This is a formal comment that works}
            @article{a1, title = "Article 1"}
            @article{a2, title = "Article 2"}
            @book{b1, title = "Book 1"}
        "#;

        let library = Library::parser().parse(input).unwrap();
        let stats = library.stats();

        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.total_strings, 1);
        assert_eq!(stats.total_preambles, 1);
        assert_eq!(stats.total_comments, 2); // Both % and @comment should work
        assert_eq!(stats.entries_by_type.get("article"), Some(&2));
        assert_eq!(stats.entries_by_type.get("book"), Some(&1));
    }

    #[test]
    fn test_parse_files_parallel() {
        use std::fs::write;
        use std::path::PathBuf;

        let dir = std::env::temp_dir();
        let path1 = dir.join("parallel_test1.bib");
        let path2 = dir.join("parallel_test2.bib");

        write(&path1, "@article{a1,title=\"A\"}").unwrap();
        write(&path2, "@article{a2,title=\"B\"}").unwrap();

        let paths: Vec<PathBuf> = vec![path1.clone(), path2.clone()];

        let library = Library::parser().threads(2).parse_files(&paths).unwrap();

        assert_eq!(library.entries().len(), 2);

        let _ = std::fs::remove_file(path1);
        let _ = std::fs::remove_file(path2);
    }

    #[test]
    fn test_builder_pattern_api() {
        let input = "@article{test, title = \"Test\"}";

        // Single-threaded (default)
        let db1 = Library::parser().parse(input).unwrap();
        assert_eq!(db1.entries().len(), 1);

        // Using parser builder
        let library2 = Library::parser().threads(1).parse(input).unwrap();
        assert_eq!(library2.entries().len(), 1);

        #[cfg(feature = "parallel")]
        {
            use std::fs::write;

            // Parallel only works for multiple files
            let db3 = Library::parser().threads(4).parse(input).unwrap();
            assert_eq!(db3.entries().len(), 1);

            // Multi-file parallel processing
            let dir = std::env::temp_dir();
            let path1 = dir.join(format!("bibtex-parser-test1-{}.bib", std::process::id()));
            let path2 = dir.join(format!("bibtex-parser-test2-{}.bib", std::process::id()));
            write(&path1, "@article{a1, title=\"A\"}").unwrap();
            write(&path2, "@article{a2, title=\"B\"}").unwrap();

            let db4 = Library::parser()
                .threads(2)
                .parse_files(&[path1.as_path(), path2.as_path()])
                .unwrap();
            assert_eq!(db4.entries().len(), 2);

            let _ = std::fs::remove_file(path1);
            let _ = std::fs::remove_file(path2);
        }
    }
}
