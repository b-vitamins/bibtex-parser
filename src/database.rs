//! BibTeX database representation

use crate::{Entry, Error, Result, Value};
use ahash::AHashMap;
use std::borrow::Cow;

/// A parsed BibTeX database
#[derive(Debug, Clone, Default)]
pub struct Database<'a> {
    /// Bibliography entries
    entries: Vec<Entry<'a>>,
    /// String definitions
    strings: AHashMap<&'a str, Value<'a>>,
    /// Preambles
    preambles: Vec<Value<'a>>,
    /// Comments
    comments: Vec<&'a str>,
}

impl<'a> Database<'a> {
    /// Create a new empty database
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a BibTeX database from a string
    pub fn parse(input: &'a str) -> Result<Self> {
        let items = crate::parser::parse_bibtex(input)?;
        let mut db = Self::new();

        // First pass: collect string definitions
        for item in &items {
            if let crate::parser::ParsedItem::String(name, value) = item {
                db.strings.insert(name, value.clone());
            }
        }

        // Second pass: process entries and expand variables
        for item in items {
            match item {
                crate::parser::ParsedItem::Entry(mut entry) => {
                    // Expand variables in field values and convert to strings
                    for field in &mut entry.fields {
                        let expanded_string = db.expand_value_to_string(&field.value)?;
                        field.value = Value::Literal(Cow::Owned(expanded_string));
                    }
                    db.entries.push(entry);
                }
                crate::parser::ParsedItem::String(_, _) => {
                    // Already processed
                }
                crate::parser::ParsedItem::Preamble(value) => {
                    let expanded = db.expand_value(&value)?;
                    db.preambles.push(expanded);
                }
                crate::parser::ParsedItem::Comment(text) => {
                    db.comments.push(text);
                }
            }
        }

        Ok(db)
    }

    /// Get all entries
    #[must_use]
    pub fn entries(&self) -> &[Entry<'a>] {
        &self.entries
    }

    /// Get all string definitions
    #[must_use]
    pub const fn strings(&self) -> &AHashMap<&'a str, Value<'a>> {
        &self.strings
    }

    /// Get all preambles
    #[must_use]
    pub fn preambles(&self) -> &[Value<'a>] {
        &self.preambles
    }

    /// Get all comments
    #[must_use]
    pub fn comments(&self) -> &[&'a str] {
        &self.comments
    }

    /// Find entries by key
    #[must_use]
    pub fn find_by_key(&self, key: &str) -> Option<&Entry<'a>> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// Find entries by type
    #[must_use]
    pub fn find_by_type(&self, ty: &str) -> Vec<&Entry<'a>> {
        self.entries
            .iter()
            .filter(|e| e.ty.to_string().eq_ignore_ascii_case(ty))
            .collect()
    }

    /// Find entries by field value
    #[must_use]
    pub fn find_by_field(&self, field: &str, value: &str) -> Vec<&Entry<'a>> {
        self.entries
            .iter()
            .filter(|e| e.get(field).is_some_and(|v| v.contains(value)))
            .collect()
    }

    /// Expand variables in a value to get a string
    fn expand_value_to_string(&self, value: &Value<'a>) -> Result<String> {
        match value {
            Value::Literal(s) => Ok(s.to_string()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Variable(name) => self
                .strings
                .get(name)
                .ok_or_else(|| Error::UndefinedVariable((*name).to_string()))
                .and_then(|v| self.expand_value_to_string(v)),
            Value::Concat(parts) => {
                let mut result = String::new();
                for part in parts {
                    result.push_str(&self.expand_value_to_string(part)?);
                }
                Ok(result)
            }
        }
    }

    /// Expand variables in a value
    fn expand_value(&self, value: &Value<'a>) -> Result<Value<'a>> {
        match value {
            Value::Variable(name) => self
                .strings
                .get(name)
                .ok_or_else(|| Error::UndefinedVariable((*name).to_string()))
                .and_then(|v| self.expand_value(v)),
            Value::Concat(parts) => {
                let expanded_parts = parts
                    .iter()
                    .map(|p| self.expand_value(p))
                    .collect::<Result<Vec<_>>>()?;

                // If all parts are literals, we can flatten to a single literal
                if expanded_parts
                    .iter()
                    .all(|p| matches!(p, Value::Literal(_)))
                {
                    let combined = expanded_parts
                        .iter()
                        .filter_map(|p| match p {
                            Value::Literal(s) => Some(s.as_ref()),
                            _ => None,
                        })
                        .collect::<String>();
                    Ok(Value::Literal(Cow::Owned(combined)))
                } else {
                    Ok(Value::Concat(expanded_parts))
                }
            }
            _ => Ok(value.clone()),
        }
    }

    /// Convert to owned version (no borrowed data)
    #[must_use]
    pub fn into_owned(self) -> Database<'static> {
        Database {
            entries: self.entries.into_iter().map(Entry::into_owned).collect(),
            strings: self
                .strings
                .into_iter()
                .map(|(k, v)| {
                    let owned_key: &'static str = Box::leak(k.to_string().into_boxed_str());
                    (owned_key, v.into_owned())
                })
                .collect(),
            preambles: self.preambles.into_iter().map(Value::into_owned).collect(),
            comments: self
                .comments
                .into_iter()
                .map(|c| {
                    let owned_comment: &'static str = Box::leak(c.to_string().into_boxed_str());
                    owned_comment
                })
                .collect(),
        }
    }
}

/// Builder for creating databases programmatically
#[derive(Debug, Default)]
pub struct DatabaseBuilder<'a> {
    db: Database<'a>,
}

impl<'a> DatabaseBuilder<'a> {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry
    #[must_use]
    pub fn entry(mut self, entry: Entry<'a>) -> Self {
        self.db.entries.push(entry);
        self
    }

    /// Add a string definition
    #[must_use]
    pub fn string(mut self, name: &'a str, value: Value<'a>) -> Self {
        self.db.strings.insert(name, value);
        self
    }

    /// Add a preamble
    #[must_use]
    pub fn preamble(mut self, value: Value<'a>) -> Self {
        self.db.preambles.push(value);
        self
    }

    /// Add a comment
    #[must_use]
    pub fn comment(mut self, text: &'a str) -> Self {
        self.db.comments.push(text);
        self
    }

    /// Build the database
    #[must_use]
    pub fn build(self) -> Database<'a> {
        self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryType, Field};

    #[test]
    fn test_database_parse() {
        let input = r#"
            @string{me = "John Doe"}
            
            @article{test2023,
                author = me,
                title = "Test Article",
                year = 2023
            }
        "#;

        let db = Database::parse(input).unwrap();
        assert_eq!(db.entries().len(), 1);
        assert_eq!(db.strings().len(), 1);

        let entry = &db.entries()[0];
        assert_eq!(entry.get("author"), Some("John Doe"));
    }

    #[test]
    fn test_database_builder() {
        let db = DatabaseBuilder::new()
            .string("me", Value::Literal(Cow::Borrowed("John Doe")))
            .entry(Entry {
                ty: EntryType::Article,
                key: "test2023",
                fields: vec![
                    Field::new("author", Value::Variable("me")),
                    Field::new("title", Value::Literal(Cow::Borrowed("Test"))),
                ],
            })
            .build();

        assert_eq!(db.entries().len(), 1);
        assert_eq!(db.strings().len(), 1);
    }
}
