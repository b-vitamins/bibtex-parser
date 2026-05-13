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
    line_ascii: Vec<bool>,
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
        let mut line_ascii = Vec::new();
        let mut current_line_ascii = true;
        line_starts.push(0);
        for (index, byte) in input.bytes().enumerate() {
            if byte >= 0x80 {
                current_line_ascii = false;
            }
            if byte == b'\n' {
                line_ascii.push(current_line_ascii);
                line_starts.push(index + 1);
                current_line_ascii = true;
            }
        }
        line_ascii.push(current_line_ascii);

        Self {
            source,
            name,
            input,
            line_starts,
            line_ascii,
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

    /// Return the underlying source text.
    #[must_use]
    pub const fn input(&self) -> &'a str {
        self.input
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
        let column = if self.line_ascii.get(line_index).copied().unwrap_or(false) {
            byte - line_start + 1
        } else {
            self.input[line_start..byte].chars().count() + 1
        };
        (line_index + 1, column)
    }

    /// Return the byte offset for a one-based line and column.
    ///
    /// Columns count Unicode scalar values. A column one past the end of the
    /// line resolves to the line-end byte offset.
    #[must_use]
    pub fn byte_at_line_column(&self, line: usize, column: usize) -> Option<usize> {
        if line == 0 || column == 0 {
            return None;
        }
        let line_start = *self.line_starts.get(line - 1)?;
        let line_end = self
            .line_starts
            .get(line)
            .map_or(self.input.len(), |next| next.saturating_sub(1));
        let line_text = self.input.get(line_start..line_end)?;
        if column == 1 {
            return Some(line_start);
        }
        let mut current_column = 1usize;
        for (offset, _) in line_text.char_indices() {
            if current_column == column {
                return Some(line_start + offset);
            }
            current_column += 1;
        }
        if current_column == column {
            Some(line_end)
        } else {
            None
        }
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

    pub(crate) const fn cursor(&self) -> SourceCursor<'_, 'a> {
        SourceCursor {
            map: self,
            line_index: 0,
        }
    }

    /// Return a borrowed slice for a source span when it belongs to this source.
    #[must_use]
    pub fn slice(&self, span: SourceSpan) -> Option<&'a str> {
        if span.source.is_some() && span.source != self.source {
            return None;
        }
        self.input.get(span.byte_start..span.byte_end)
    }

    /// Return a short line-oriented snippet for a span.
    #[must_use]
    pub fn snippet(&self, span: SourceSpan, max_chars: usize) -> Option<String> {
        if span.source.is_some() && span.source != self.source {
            return None;
        }

        let anchor_start = if span.is_empty() && span.byte_start > 0 {
            span.byte_start - 1
        } else {
            span.byte_start
        };
        let anchor_end = if span.is_empty() && span.byte_end > 0 {
            span.byte_end - 1
        } else {
            span.byte_end
        };

        let start = self.input[..anchor_start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let end = self.input[anchor_end..]
            .find('\n')
            .map_or(self.input.len(), |index| anchor_end + index);
        let snippet = self.input.get(start..end)?;

        if snippet.chars().count() <= max_chars {
            return Some(snippet.to_string());
        }

        Some(snippet.chars().take(max_chars).collect())
    }
}

pub(crate) struct SourceCursor<'map, 'source> {
    map: &'map SourceMap<'source>,
    line_index: usize,
}

impl SourceCursor<'_, '_> {
    pub(crate) fn span(&mut self, byte_start: usize, byte_end: usize) -> SourceSpan {
        let byte_start = byte_start.min(self.map.input.len());
        let byte_end = byte_end.min(self.map.input.len()).max(byte_start);
        let start_line_index = self.line_index_at(byte_start);
        let end_line_index = self.line_index_from(start_line_index, byte_end);
        let column = self.column_at(start_line_index, byte_start);
        let end_column = self.column_at(end_line_index, byte_end);
        let span = SourceSpan::with_end(
            byte_start,
            byte_end,
            start_line_index + 1,
            column,
            end_line_index + 1,
            end_column,
        );
        self.map
            .source
            .map_or(span, |source| span.with_source(source))
    }

    fn line_index_at(&mut self, byte: usize) -> usize {
        if self
            .map
            .line_starts
            .get(self.line_index)
            .is_some_and(|start| byte < *start)
        {
            self.line_index = self.map.line_index_for(byte);
            return self.line_index;
        }

        while self
            .map
            .line_starts
            .get(self.line_index + 1)
            .is_some_and(|next| *next <= byte)
        {
            self.line_index += 1;
        }

        self.line_index
    }

    fn line_index_from(&self, mut line_index: usize, byte: usize) -> usize {
        if self
            .map
            .line_starts
            .get(line_index)
            .is_some_and(|start| byte < *start)
        {
            return self.map.line_index_for(byte);
        }

        while self
            .map
            .line_starts
            .get(line_index + 1)
            .is_some_and(|next| *next <= byte)
        {
            line_index += 1;
        }

        line_index
    }

    fn column_at(&self, line_index: usize, byte: usize) -> usize {
        let line_start = self.map.line_starts[line_index];
        if self
            .map
            .line_ascii
            .get(line_index)
            .copied()
            .unwrap_or(false)
        {
            byte - line_start + 1
        } else {
            self.map.input[line_start..byte].chars().count() + 1
        }
    }
}

impl SourceMap<'_> {
    fn line_index_for(&self, byte: usize) -> usize {
        match self.line_starts.binary_search(&byte) {
            Ok(index) => index,
            Err(0) => 0,
            Err(index) => index - 1,
        }
    }
}
