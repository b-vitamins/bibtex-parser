//! BibTeX writer for serializing databases

use crate::{Database, Entry, Result, Value};
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
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
            align_values: false,
            max_line_length: 80,
            sort_entries: false,
            sort_fields: false,
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

    /// Write a complete database
    pub fn write_database(&mut self, db: &Database) -> io::Result<()> {
        // Write preambles
        for preamble in db.preambles() {
            self.write_preamble(preamble)?;
            writeln!(self.writer)?;
        }

        // Write strings
        let mut strings: Vec<_> = db.strings().iter().collect();
        if self.config.sort_entries {
            strings.sort_by_key(|(k, _)| *k);
        }

        for (name, value) in strings {
            self.write_string(name, value)?;
            writeln!(self.writer)?;
        }

        // Write entries
        let mut entries = db.entries().iter().collect::<Vec<_>>();
        if self.config.sort_entries {
            entries.sort_by_key(|e| e.key);
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
            fields.sort_by_key(|f| f.name);
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

            if i < fields.len() - 1 {
                writeln!(self.writer, ",")?;
            } else {
                writeln!(self.writer)?;
            }
        }

        writeln!(self.writer, "}}")?;
        Ok(())
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
                // Parts is now Box<Vec<Value>>
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
fn needs_quoting(s: &str) -> bool {
    s.contains(['{', '}', ',', '='])
}

/// Escape quotes in a string
fn escape_quotes(s: &str) -> String {
    s.replace('"', "\\\"")
}

/// Convenience function to write a database to a string
pub fn to_string(db: &Database) -> Result<String> {
    let mut buf = Vec::new();
    let mut writer = Writer::new(&mut buf);
    writer.write_database(db)?;
    Ok(String::from_utf8(buf).expect("valid UTF-8"))
}

/// Convenience function to write a database to a file
pub fn to_file(db: &Database, path: impl AsRef<std::path::Path>) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut writer = Writer::new(file);
    writer.write_database(db)?;
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
            key: "test2023",
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
