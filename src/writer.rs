//! BibTeX writer for serializing libraries

use crate::{Block, Entry, Library, ParsedBlock, ParsedDocument, ParsedEntry, Result, Value};
use std::borrow::Cow;
use std::io::{self, Write};

/// Configuration for writing BibTeX
#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// Indentation string (default: "  ")
    pub indent: String,
    /// Whether to align field values (default: false)
    pub align_values: bool,
    /// Maximum line length for wrapping (default: 80)
    pub max_line_length: usize,
    /// Whether to sort entries by key (default: false)
    pub sort_entries: bool,
    /// Whether to sort fields within entries (default: false)
    pub sort_fields: bool,
    /// Raw-backed document writing behavior.
    pub raw_write_mode: RawWriteMode,
    /// Trailing comma behavior for structured entry writing.
    pub trailing_comma: TrailingComma,
    /// Separator written between document blocks.
    pub entry_separator: String,
}

/// Raw-backed document writing behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawWriteMode {
    /// Reuse retained raw text where possible.
    Preserve,
    /// Ignore retained raw text and write normalized structured data.
    Normalize,
}

/// Trailing comma behavior for structured entry writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailingComma {
    /// Omit a trailing comma after the final field.
    Omit,
    /// Add a trailing comma after the final field.
    Always,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
            align_values: false,
            max_line_length: 80,
            sort_entries: false,
            sort_fields: false,
            raw_write_mode: RawWriteMode::Preserve,
            trailing_comma: TrailingComma::Omit,
            entry_separator: "\n".to_string(),
        }
    }
}

/// BibTeX writer
#[derive(Debug)]
pub struct Writer<W: Write> {
    writer: W,
    config: WriterConfig,
}

impl<W: Write> Writer<W> {
    /// Create a new writer with default configuration
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            config: WriterConfig::default(),
        }
    }

    /// Create a new writer with custom configuration
    pub const fn with_config(writer: W, config: WriterConfig) -> Self {
        Self { writer, config }
    }

    /// Access the writer configuration mutably
    #[must_use]
    pub fn config_mut(&mut self) -> &mut WriterConfig {
        &mut self.config
    }

    /// Consume the writer and return the underlying writer
    #[must_use]
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Write a complete library.
    pub fn write_library(&mut self, library: &Library) -> io::Result<()> {
        if self.config.sort_entries {
            return self.write_library_sorted(library);
        }

        for (index, block) in library.blocks().into_iter().enumerate() {
            if index > 0 {
                writeln!(self.writer)?;
            }
            match block {
                Block::Entry(entry, _) => self.write_entry(entry)?,
                Block::String(definition) => {
                    self.write_string(&definition.name, &definition.value)?;
                }
                Block::Preamble(preamble) => self.write_preamble(&preamble.value)?,
                Block::Comment(comment) => self.write_comment(comment.text())?,
                Block::Failed(failed) => self.writer.write_all(failed.raw.as_bytes())?,
            }
        }

        Ok(())
    }

    /// Write a parsed document, reusing retained raw blocks when configured.
    pub fn write_document(&mut self, document: &ParsedDocument) -> io::Result<()> {
        self.write_document_with_raw_source(document, None)
    }

    pub(crate) fn write_document_with_raw_source(
        &mut self,
        document: &ParsedDocument,
        raw_source: Option<&str>,
    ) -> io::Result<()> {
        for (index, block) in document.blocks().iter().copied().enumerate() {
            if index > 0 {
                self.writer
                    .write_all(self.config.entry_separator.as_bytes())?;
            }

            match block {
                ParsedBlock::Entry(entry_index) => {
                    self.write_parsed_entry_with_raw_source(
                        &document.entries()[entry_index],
                        raw_source,
                    )?;
                }
                ParsedBlock::String(string_index) => {
                    let string = &document.strings()[string_index];
                    if self.config.raw_write_mode == RawWriteMode::Preserve {
                        if let Some(raw) =
                            raw_text_with_source(string.raw.as_deref(), raw_source, string.source)
                        {
                            self.writer.write_all(raw.as_bytes())?;
                            continue;
                        }
                    }
                    self.write_string(&string.name, &string.value.value)?;
                }
                ParsedBlock::Preamble(preamble_index) => {
                    let preamble = &document.preambles()[preamble_index];
                    if self.config.raw_write_mode == RawWriteMode::Preserve {
                        if let Some(raw) = raw_text_with_source(
                            preamble.raw.as_deref(),
                            raw_source,
                            preamble.source,
                        ) {
                            self.writer.write_all(raw.as_bytes())?;
                            continue;
                        }
                    }
                    self.write_preamble(&preamble.value.value)?;
                }
                ParsedBlock::Comment(comment_index) => {
                    let comment = &document.comments()[comment_index];
                    if self.config.raw_write_mode == RawWriteMode::Preserve {
                        if let Some(raw) =
                            raw_text_with_source(comment.raw.as_deref(), raw_source, comment.source)
                        {
                            self.writer.write_all(raw.as_bytes())?;
                            continue;
                        }
                    }
                    self.write_comment(&comment.text)?;
                }
                ParsedBlock::Failed(failed_index) => {
                    self.writer
                        .write_all(document.failed_blocks()[failed_index].raw.as_bytes())?;
                }
            }
        }

        Ok(())
    }

    /// Write selected parsed-document entries in source order.
    ///
    /// Non-entry blocks are skipped. Duplicate keys in `keys` do not duplicate
    /// output entries.
    pub fn write_selected_entries(
        &mut self,
        document: &ParsedDocument,
        keys: &[&str],
    ) -> io::Result<()> {
        self.write_selected_entries_with_raw_source(document, keys, None)
    }

    pub(crate) fn write_selected_entries_with_raw_source(
        &mut self,
        document: &ParsedDocument,
        keys: &[&str],
        raw_source: Option<&str>,
    ) -> io::Result<()> {
        let mut written = 0usize;
        for block in document.blocks().iter().copied() {
            let ParsedBlock::Entry(entry_index) = block else {
                continue;
            };
            let entry = &document.entries()[entry_index];
            if !keys.iter().any(|key| *key == entry.key()) {
                continue;
            }
            if written > 0 {
                self.writer
                    .write_all(self.config.entry_separator.as_bytes())?;
            }
            self.write_parsed_entry_with_raw_source(entry, raw_source)?;
            written += 1;
        }

        Ok(())
    }

    fn write_library_sorted(&mut self, library: &Library) -> io::Result<()> {
        // Write preambles
        for preamble in library.preambles() {
            self.write_preamble(&preamble.value)?;
            writeln!(self.writer)?;
        }

        // Write strings
        let mut strings: Vec<_> = library.strings().iter().collect();
        if self.config.sort_entries {
            strings.sort_by(|a, b| a.name.cmp(&b.name));
        }

        for definition in strings {
            self.write_string(&definition.name, &definition.value)?;
            writeln!(self.writer)?;
        }

        // Write entries
        let mut entries = library.entries().iter().collect::<Vec<_>>();
        if self.config.sort_entries {
            entries.sort_by(|a, b| a.key.cmp(&b.key));
        }

        for (i, entry) in entries.iter().enumerate() {
            if i > 0 {
                writeln!(self.writer)?;
            }
            self.write_entry(entry)?;
        }

        Ok(())
    }

    /// Write a single entry
    pub fn write_entry(&mut self, entry: &Entry) -> io::Result<()> {
        writeln!(self.writer, "@{}{{{},", entry.ty, entry.key)?;

        let mut fields = entry.fields().to_vec();
        if self.config.sort_fields {
            fields.sort_by(|a, b| a.name.cmp(&b.name));
        }

        // Calculate alignment if needed
        let max_name_len = if self.config.align_values {
            fields.iter().map(|f| f.name.len()).max().unwrap_or(0)
        } else {
            0
        };

        for (i, field) in fields.iter().enumerate() {
            write!(self.writer, "{}", self.config.indent)?;
            write!(self.writer, "{}", field.name)?;

            if self.config.align_values {
                let padding = max_name_len - field.name.len();
                write!(self.writer, "{}", " ".repeat(padding))?;
            }

            write!(self.writer, " = ")?;
            self.write_value(&field.value)?;

            if i < fields.len() - 1 || self.config.trailing_comma == TrailingComma::Always {
                writeln!(self.writer, ",")?;
            } else {
                writeln!(self.writer)?;
            }
        }

        writeln!(self.writer, "}}")?;
        Ok(())
    }

    fn write_parsed_entry_with_raw_source(
        &mut self,
        entry: &ParsedEntry,
        raw_source: Option<&str>,
    ) -> io::Result<()> {
        if self.config.raw_write_mode == RawWriteMode::Preserve {
            if let Some(raw) = patched_entry_raw(entry, raw_source) {
                self.writer.write_all(raw.as_bytes())?;
                return Ok(());
            }
        }

        self.write_entry(&entry.clone().into_entry())
    }

    /// Write a string definition
    fn write_string(&mut self, name: &str, value: &Value) -> io::Result<()> {
        write!(self.writer, "@string{{{name} = ")?;
        self.write_value(value)?;
        writeln!(self.writer, "}}")?;
        Ok(())
    }

    /// Write a preamble
    fn write_preamble(&mut self, value: &Value) -> io::Result<()> {
        write!(self.writer, "@preamble{{")?;
        self.write_value(value)?;
        writeln!(self.writer, "}}")?;
        Ok(())
    }

    /// Write a comment.
    fn write_comment(&mut self, text: &str) -> io::Result<()> {
        let trimmed = text.trim_start();
        if trimmed.starts_with('%') || trimmed.starts_with('@') {
            self.writer.write_all(text.as_bytes())?;
            if !text.ends_with('\n') {
                writeln!(self.writer)?;
            }
        } else {
            writeln!(self.writer, "@comment{{{text}}}")?;
        }
        Ok(())
    }

    /// Write a value
    fn write_value(&mut self, value: &Value) -> io::Result<()> {
        match value {
            Value::Literal(s) => {
                // Quote if contains special characters
                if needs_quoting(s) {
                    write!(self.writer, "\"{}\"", escape_quotes(s))?;
                } else {
                    write!(self.writer, "{{{s}}}")?;
                }
            }
            Value::Number(n) => write!(self.writer, "{n}")?,
            Value::Variable(name) => write!(self.writer, "{name}")?,
            Value::Concat(parts) => {
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        write!(self.writer, " # ")?;
                    }
                    self.write_value(part)?;
                }
            }
        }
        Ok(())
    }
}

/// Check if a string needs quoting
#[must_use]
fn needs_quoting(s: &str) -> bool {
    s.contains(['{', '}', ',', '='])
}

/// Escape quotes in a string
#[must_use]
fn escape_quotes(s: &str) -> String {
    s.replace('"', "\\\"")
}

fn raw_text_with_source<'a>(
    raw: Option<&'a str>,
    raw_source: Option<&'a str>,
    span: Option<crate::SourceSpan>,
) -> Option<&'a str> {
    raw.or_else(|| source_slice(raw_source, span?))
}

fn source_slice(raw_source: Option<&str>, span: crate::SourceSpan) -> Option<&str> {
    let raw_source = raw_source?;
    raw_source.get(span.byte_start..span.byte_end)
}

fn patched_entry_raw<'entry>(
    entry: &'entry ParsedEntry<'_>,
    raw_source: Option<&'entry str>,
) -> Option<Cow<'entry, str>> {
    let source = entry.source?;
    let raw = raw_text_with_source(entry.raw.as_deref(), raw_source, Some(source))?;
    let mut replacements = Vec::new();

    push_token_replacement(
        &mut replacements,
        raw,
        source.byte_start,
        entry.entry_type_source,
        &entry.ty.to_string(),
        |raw_type| crate::EntryType::parse(raw_type) == entry.ty,
    )?;
    push_token_replacement(
        &mut replacements,
        raw,
        source.byte_start,
        entry.key_source,
        &entry.key,
        |raw_key| raw_key == entry.key,
    )?;

    for field in &entry.fields {
        push_token_replacement(
            &mut replacements,
            raw,
            source.byte_start,
            field.name_source,
            &field.name,
            |raw_name| raw_name == field.name,
        )?;

        if field.value.raw.is_none() {
            let value_source = field.value_source?;
            if source_slice(raw_source, value_source).is_none() {
                let start = value_source.byte_start.checked_sub(source.byte_start)?;
                let end = value_source.byte_end.checked_sub(source.byte_start)?;
                replacements.push((start, end, field.value.value.to_bibtex_source()));
            }
        }
    }

    if replacements.is_empty() {
        return Some(Cow::Borrowed(raw));
    }

    replacements.sort_by_key(|(start, _, _)| *start);
    let mut output = String::with_capacity(raw.len());
    let mut cursor = 0;
    for (start, end, replacement) in replacements {
        if start < cursor || end > raw.len() {
            return None;
        }
        output.push_str(&raw[cursor..start]);
        output.push_str(&replacement);
        cursor = end;
    }
    output.push_str(&raw[cursor..]);
    Some(Cow::Owned(output))
}

fn push_token_replacement(
    replacements: &mut Vec<(usize, usize, String)>,
    raw: &str,
    base: usize,
    span: Option<crate::SourceSpan>,
    replacement: &str,
    unchanged: impl FnOnce(&str) -> bool,
) -> Option<()> {
    let span = span?;
    let start = span.byte_start.checked_sub(base)?;
    let end = span.byte_end.checked_sub(base)?;
    let original = raw.get(start..end)?;
    if !unchanged(original) {
        replacements.push((start, end, replacement.to_string()));
    }
    Some(())
}

/// Convenience function to write a library to a string.
#[must_use = "Check the result to detect serialization errors"]
pub fn to_string(library: &Library) -> Result<String> {
    let mut buf = Vec::new();
    let mut writer = Writer::new(&mut buf);
    writer.write_library(library)?;
    Ok(String::from_utf8(buf).expect("valid UTF-8"))
}

/// Convenience function to write a parsed document to a string.
#[must_use = "Check the result to detect serialization errors"]
pub fn document_to_string(document: &ParsedDocument) -> Result<String> {
    let mut buf = Vec::new();
    let mut writer = Writer::new(&mut buf);
    writer.write_document(document)?;
    Ok(String::from_utf8(buf).expect("valid UTF-8"))
}

/// Convenience function to write selected parsed-document entries to a string.
#[must_use = "Check the result to detect serialization errors"]
pub fn selected_entries_to_string(document: &ParsedDocument, keys: &[&str]) -> Result<String> {
    let mut buf = Vec::new();
    let mut writer = Writer::new(&mut buf);
    writer.write_selected_entries(document, keys)?;
    Ok(String::from_utf8(buf).expect("valid UTF-8"))
}

/// Convenience function to write a library to a file.
#[must_use = "Check the result to detect IO or serialization errors"]
pub fn to_file(library: &Library, path: impl AsRef<std::path::Path>) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut writer = Writer::new(file);
    writer.write_library(library)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryType, Field};
    use std::borrow::Cow;

    #[test]
    fn test_write_entry() {
        let entry = Entry {
            ty: EntryType::Article,
            key: Cow::Borrowed("test2023"),
            fields: vec![
                Field::new("author", Value::Literal(Cow::Borrowed("John Doe"))),
                Field::new("title", Value::Literal(Cow::Borrowed("Test Article"))),
                Field::new("year", Value::Number(2023)),
            ],
        };

        let mut buf = Vec::new();
        let mut writer = Writer::new(&mut buf);
        writer.write_entry(&entry).unwrap();

        let result = String::from_utf8(buf).unwrap();
        assert!(result.contains("@article{test2023,"));
        assert!(result.contains("author = {John Doe}"));
        assert!(result.contains("title = {Test Article}"));
        assert!(result.contains("year = 2023"));
    }
}
