//! Source identity and byte-to-line location utilities.

use crate::{SourceId, SourceSpan};
use std::borrow::Cow;

/// A line-indexed view of a parsed source.
///
/// Byte offsets are zero-based UTF-8 offsets. Lines and columns are one-based,
/// and columns count Unicode scalar values, not bytes. End positions point to
/// the location immediately after the covered byte range.
#[derive(Debug, Clone)]
pub struct SourceMap<'a> {
    source: Option<SourceId>,
    name: Option<Cow<'a, str>>,
    input: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourceMap<'a> {
    /// Create an anonymous source map.
    #[must_use]
    pub fn anonymous(input: &'a str) -> Self {
        Self::new(None, None, input)
    }

    /// Create a source map with a document-local source identifier and optional name.
    #[must_use]
    pub fn new(source: Option<SourceId>, name: Option<Cow<'a, str>>, input: &'a str) -> Self {
        let mut line_starts = Vec::new();
        line_starts.push(0);
        for (index, byte) in input.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }

        Self {
            source,
            name,
            input,
            line_starts,
        }
    }

    /// Return this source's identifier, when it has one.
    #[must_use]
    pub const fn source_id(&self) -> Option<SourceId> {
        self.source
    }

    /// Return this source's caller-provided name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Return the number of bytes in the source.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.input.len()
    }

    /// Return true when this source is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    /// Return the line and column for a byte offset.
    ///
    /// Offsets past end-of-file are clamped to the end of the source.
    #[must_use]
    pub fn line_column(&self, byte: usize) -> (usize, usize) {
        let byte = byte.min(self.input.len());
        let line_index = match self.line_starts.binary_search(&byte) {
            Ok(index) => index,
            Err(0) => 0,
            Err(index) => index - 1,
        };
        let line_start = self.line_starts[line_index];
        let column = self.input[line_start..byte].chars().count() + 1;
        (line_index + 1, column)
    }

    /// Create a source span for a byte range.
    ///
    /// The range is clamped to the source length. The returned span keeps the
    /// half-open byte offsets and one-based start/end line-column positions.
    #[must_use]
    pub fn span(&self, byte_start: usize, byte_end: usize) -> SourceSpan {
        let byte_start = byte_start.min(self.input.len());
        let byte_end = byte_end.min(self.input.len()).max(byte_start);
        let (line, column) = self.line_column(byte_start);
        let (end_line, end_column) = self.line_column(byte_end);
        let span = SourceSpan::with_end(byte_start, byte_end, line, column, end_line, end_column);
        self.source.map_or(span, |source| span.with_source(source))
    }

    /// Return a borrowed slice for a source span when it belongs to this source.
    #[must_use]
    pub fn slice(&self, span: SourceSpan) -> Option<&'a str> {
        if span.source.is_some() && span.source != self.source {
            return None;
        }
        self.input.get(span.byte_start..span.byte_end)
    }
}
