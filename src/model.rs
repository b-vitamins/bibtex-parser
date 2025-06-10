//! Data models for BibTeX entries

use ahash::AHashMap;
use std::borrow::Cow;
use std::fmt;

/// A BibTeX entry (article, book, etc.)
#[derive(Debug, Clone, PartialEq)]
pub struct Entry<'a> {
    /// Entry type (article, book, inproceedings, etc.)
    pub ty: EntryType<'a>,
    /// Citation key
    pub key: Cow<'a, str>,
    /// Fields (author, title, year, etc.)
    pub fields: Vec<Field<'a>>,
}

impl<'a> Entry<'a> {
    /// Create a new entry
    #[must_use]
    pub const fn new(ty: EntryType<'a>, key: &'a str) -> Self {
        Self {
            ty,
            key: Cow::Borrowed(key),
            fields: Vec::new(),
        }
    }

    /// Get the entry type
    #[must_use]
    pub const fn entry_type(&self) -> &EntryType<'a> {
        &self.ty
    }

    /// Get the citation key
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Get a field value by name (case-insensitive)
    /// Note: This only returns string literals, not numbers
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        let name_lower = name.to_lowercase();
        self.fields
            .iter()
            .find(|f| f.name.to_lowercase() == name_lower)
            .and_then(|f| f.value.as_str())
    }

    /// Get a field value as a string, converting numbers if necessary
    #[must_use]
    pub fn get_as_string(&self, name: &str) -> Option<String> {
        let name_lower = name.to_lowercase();
        self.fields
            .iter()
            .find(|f| f.name.to_lowercase() == name_lower)
            .map(|f| match &f.value {
                Value::Literal(s) => s.to_string(),
                Value::Number(n) => n.to_string(),
                Value::Variable(v) => format!("{{{v}}}"),
                Value::Concat(parts) => parts
                    .iter()
                    .map(|p| match p {
                        Value::Literal(s) => s.to_string(),
                        Value::Number(n) => n.to_string(),
                        Value::Variable(v) => format!("{{{v}}}"),
                        Value::Concat(_) => p.to_string(),
                    })
                    .collect::<String>(),
            })
    }

    /// Get all fields
    #[must_use]
    pub fn fields(&self) -> &[Field<'a>] {
        &self.fields
    }

    /// Add a field
    pub fn add_field(&mut self, field: Field<'a>) {
        self.fields.push(field);
    }

    /// Check if entry has all required fields for its type
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.ty
            .required_fields()
            .iter()
            .all(|&field| self.get(field).is_some() || self.get_as_string(field).is_some())
    }

    /// Convert to owned version
    #[must_use]
    pub fn into_owned(self) -> Entry<'static> {
        Entry {
            ty: self.ty.into_owned(),
            key: Cow::Owned(self.key.into_owned()),
            fields: self.fields.into_iter().map(Field::into_owned).collect(),
        }
    }
}

/// BibTeX entry type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntryType<'a> {
    /// Article from a journal
    Article,
    /// Book with publisher
    Book,
    /// Part of a book
    InBook,
    /// Article in conference proceedings
    InProceedings,
    /// Conference proceedings
    Proceedings,
    /// Master's thesis
    MastersThesis,
    /// `PhD` thesis
    PhdThesis,
    /// Technical report
    TechReport,
    /// Unpublished work
    Unpublished,
    /// Miscellaneous
    Misc,
    /// Custom entry type
    Custom(Cow<'a, str>),
}

impl<'a> EntryType<'a> {
    /// Parse from string (case-insensitive)
    #[must_use]
    pub fn parse(s: &'a str) -> Self {
        match s.to_lowercase().as_str() {
            "article" => Self::Article,
            "book" => Self::Book,
            "inbook" => Self::InBook,
            "inproceedings" | "conference" => Self::InProceedings,
            "proceedings" => Self::Proceedings,
            "mastersthesis" => Self::MastersThesis,
            "phdthesis" => Self::PhdThesis,
            "techreport" => Self::TechReport,
            "unpublished" => Self::Unpublished,
            "misc" => Self::Misc,
            _ => Self::Custom(Cow::Borrowed(s)),
        }
    }

    /// Get required fields for this entry type
    #[must_use]
    pub const fn required_fields(&self) -> &'static [&'static str] {
        match self {
            Self::Article => &["author", "title", "journal", "year"],
            Self::Book => &["author", "title", "publisher", "year"],
            Self::InBook => &["author", "title", "chapter", "publisher", "year"],
            Self::InProceedings => &["author", "title", "booktitle", "year"],
            Self::Proceedings => &["title", "year"],
            Self::MastersThesis | Self::PhdThesis => &["author", "title", "school", "year"],
            Self::TechReport => &["author", "title", "institution", "year"],
            Self::Unpublished => &["author", "title", "note"],
            Self::Misc | Self::Custom(_) => &[],
        }
    }

    /// Convert to owned version
    #[must_use]
    pub fn into_owned(self) -> EntryType<'static> {
        match self {
            Self::Custom(s) => EntryType::Custom(Cow::Owned(s.into_owned())),
            Self::Article => EntryType::Article,
            Self::Book => EntryType::Book,
            Self::InBook => EntryType::InBook,
            Self::InProceedings => EntryType::InProceedings,
            Self::Proceedings => EntryType::Proceedings,
            Self::MastersThesis => EntryType::MastersThesis,
            Self::PhdThesis => EntryType::PhdThesis,
            Self::TechReport => EntryType::TechReport,
            Self::Unpublished => EntryType::Unpublished,
            Self::Misc => EntryType::Misc,
        }
    }
}

impl fmt::Display for EntryType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Article => write!(f, "article"),
            Self::Book => write!(f, "book"),
            Self::InBook => write!(f, "inbook"),
            Self::InProceedings => write!(f, "inproceedings"),
            Self::Proceedings => write!(f, "proceedings"),
            Self::MastersThesis => write!(f, "mastersthesis"),
            Self::PhdThesis => write!(f, "phdthesis"),
            Self::TechReport => write!(f, "techreport"),
            Self::Unpublished => write!(f, "unpublished"),
            Self::Misc => write!(f, "misc"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

/// A field in a BibTeX entry
#[derive(Debug, Clone, PartialEq)]
pub struct Field<'a> {
    /// Field name
    pub name: Cow<'a, str>,
    /// Field value
    pub value: Value<'a>,
}

impl<'a> Field<'a> {
    /// Create a new field
    #[must_use]
    pub const fn new(name: &'a str, value: Value<'a>) -> Self {
        Self {
            name: Cow::Borrowed(name),
            value,
        }
    }

    /// Convert to owned version
    #[must_use]
    pub fn into_owned(self) -> Field<'static> {
        Field {
            name: Cow::Owned(self.name.into_owned()),
            value: self.value.into_owned(),
        }
    }
}

/// A value in a BibTeX field
///
/// # Memory Optimization
/// This enum is optimized to be 24 bytes instead of 32 bytes by boxing the Vec in Concat.
///
/// ## Why was it 32 bytes?
/// - Largest variant was `Concat(Vec<Value>)` at 24 bytes
/// - Add 8 bytes for discriminant = 32 bytes total
/// - Rust doesn't pack discriminant into padding
///
/// ## Why is it now 24 bytes?
/// - Largest variant is now `Literal(Cow<str>)` at 24 bytes  
/// - `Concat(Box<Vec<Value>>)` is only 8 bytes
/// - Total enum size matches largest variant: 24 bytes
///
/// This saves 8 bytes per field value, which adds up to significant savings.
/// For example, with 10,000 fields, this saves 80 KB of memory.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    /// String literal
    Literal(Cow<'a, str>),
    /// Number literal
    Number(i64),
    /// Concatenated values (boxed to reduce enum size from 32 to 24 bytes)
    Concat(Box<Vec<Value<'a>>>),
    /// Variable reference
    Variable(Cow<'a, str>),
}

impl Default for Value<'_> {
    fn default() -> Self {
        Self::Number(0)
    }
}

impl Value<'_> {
    /// Get the value as a string (if it's a simple literal)
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Literal(s) => Some(s),
            _ => None,
        }
    }

    /// Expand variables and concatenations to get final string
    #[must_use]
    pub fn expand(&self, strings: &AHashMap<&str, Value>) -> String {
        match self {
            Self::Literal(s) => s.to_string(),
            Self::Number(n) => n.to_string(),
            Self::Variable(name) => strings
                .get(name.as_ref())
                .map_or_else(|| format!("{{undefined:{name}}}"), |v| v.expand(strings)),
            Self::Concat(parts) => parts.iter().map(|p| p.expand(strings)).collect::<String>(),
        }
    }

    /// Convert to owned version
    #[must_use]
    pub fn into_owned(self) -> Value<'static> {
        match self {
            Self::Literal(s) => Value::Literal(Cow::Owned(s.into_owned())),
            Self::Number(n) => Value::Number(n),
            Self::Variable(s) => Value::Variable(Cow::Owned(s.into_owned())),
            Self::Concat(parts) => {
                Value::Concat(Box::new(parts.into_iter().map(Value::into_owned).collect()))
            }
        }
    }
}

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "{s}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::Variable(name) => write!(f, "{{{name}}}"),
            Self::Concat(parts) => {
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        write!(f, " # ")?;
                    }
                    write!(f, "{part}")?;
                }
                Ok(())
            }
        }
    }
}
