//! Data models for BibTeX entries

use ahash::AHashMap;
use std::borrow::Cow;
use std::fmt;

/// Validation strictness level for BibTeX entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationLevel {
    /// Only check that required fields exist
    Minimal,
    /// Check required fields and common issues (default)
    Standard,
    /// Strict validation including field formats and cross-references
    Strict,
}

impl Default for ValidationLevel {
    fn default() -> Self {
        Self::Standard
    }
}

/// Represents a validation error for a BibTeX entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The field that failed validation (if applicable)
    pub field: Option<String>,
    /// Description of the validation failure
    pub message: String,
    /// Severity of the error
    pub severity: ValidationSeverity,
}

/// Severity level for validation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    /// Must be fixed for valid BibTeX
    Error,
    /// Should be fixed but might work
    Warning,
    /// Informational, best practices
    Info,
}

impl ValidationError {
    /// Create a new error-level validation error
    #[must_use]
    pub fn error(field: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            field: field.map(String::from),
            message: message.into(),
            severity: ValidationSeverity::Error,
        }
    }

    /// Create a new warning-level validation error
    #[must_use]
    pub fn warning(field: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            field: field.map(String::from),
            message: message.into(),
            severity: ValidationSeverity::Warning,
        }
    }

    /// Create a new info-level validation error
    #[must_use]
    pub fn info(field: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            field: field.map(String::from),
            message: message.into(),
            severity: ValidationSeverity::Info,
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = self.field.as_deref().unwrap_or("<entry>");
        write!(f, "[{:?}] {}: {}", self.severity, field, self.message)
    }
}

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

    /// Get a field value by name (case-sensitive)
    /// Note: This only returns string literals, not numbers
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.name == name)
            .and_then(|f| f.value.as_str())
    }

    /// Get a field value by name (case-insensitive)
    /// Returns the first field whose name matches ignoring case
    /// Note: This only returns string literals, not numbers
    #[must_use]
    pub fn get_ignore_case(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
            .and_then(|f| f.value.as_str())
    }

    /// Get a field value as a string, converting numbers if necessary (case-sensitive)
    #[must_use]
    pub fn get_as_string(&self, name: &str) -> Option<String> {
        self.fields
            .iter()
            .find(|f| f.name == name)
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

    /// Get a field value as a string, converting numbers if necessary (case-insensitive)
    #[must_use]
    pub fn get_as_string_ignore_case(&self, name: &str) -> Option<String> {
        self.fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
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

    /// Validate the entry according to the specified level
    /// Returns Ok(()) if valid, or Err with a list of validation errors
    pub fn validate(&self, level: ValidationLevel) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Always check required fields
        self.validate_required_fields(&mut errors);

        match level {
            ValidationLevel::Minimal => {
                // Only required fields
            }
            ValidationLevel::Standard => {
                // Additional standard checks
                self.validate_common_issues(&mut errors);
            }
            ValidationLevel::Strict => {
                // All checks
                self.validate_common_issues(&mut errors);
                self.validate_field_formats(&mut errors);
                self.validate_cross_references(&mut errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate required fields for the entry type
    fn validate_required_fields(&self, errors: &mut Vec<ValidationError>) {
        // Special handling for book entries which can have either author or editor
        match self.ty {
            EntryType::Book => {
                // Check title, publisher, year
                for &field in &["title", "publisher", "year"] {
                    if !self.has_field(field) {
                        errors.push(ValidationError::error(
                            Some(field),
                            format!(
                                "Required field '{}' is missing for {} entry",
                                field, self.ty
                            ),
                        ));
                    }
                }
                // Check author OR editor
                if !self.has_field("author") && !self.has_field("editor") {
                    errors.push(ValidationError::error(
                        None,
                        "Book entry must have either 'author' or 'editor' field",
                    ));
                }
            }
            _ => {
                // Standard required field checking
                for &field in self.ty.required_fields() {
                    if !self.has_field(field) {
                        errors.push(ValidationError::error(
                            Some(field),
                            format!(
                                "Required field '{}' is missing for {} entry",
                                field, self.ty
                            ),
                        ));
                    }
                }
            }
        }
    }

    /// Validate common issues that might cause problems
    fn validate_common_issues(&self, errors: &mut Vec<ValidationError>) {
        // Check for common issues

        // Year should be a valid number and recent
        if let Some(year_str) = self.get_as_string_ignore_case("year") {
            if let Ok(year) = year_str.parse::<i32>() {
                if !(1000..=2100).contains(&year) {
                    errors.push(ValidationError::warning(
                        Some("year"),
                        format!("Year {year} seems unlikely"),
                    ));
                }
            } else {
                errors.push(ValidationError::warning(
                    Some("year"),
                    "Year should be a number",
                ));
            }
        }

        // Pages should have valid format (e.g., "12-24" or "12--24")
        if let Some(pages) = self.get_ignore_case("pages") {
            if !is_valid_page_range(pages) {
                errors.push(ValidationError::warning(
                    Some("pages"),
                    "Pages should be in format '12-34' or '12--34'",
                ));
            }
        }

        // Author and editor shouldn't both be missing for some types (but not books, handled above)
        match self.ty {
            EntryType::InBook | EntryType::InProceedings => {
                if !self.has_field("author") && !self.has_field("editor") {
                    errors.push(ValidationError::warning(
                        None,
                        "Entry should have either 'author' or 'editor' field",
                    ));
                }
            }
            _ => {}
        }

        // Check for empty fields
        for field in &self.fields {
            if let Some(value_str) = field.value.as_str() {
                if value_str.trim().is_empty() {
                    errors.push(ValidationError::warning(
                        Some(&field.name),
                        "Field has empty value",
                    ));
                }
            }
        }
    }

    /// Validate specific field formats for strict checking
    fn validate_field_formats(&self, errors: &mut Vec<ValidationError>) {
        // DOI format
        if let Some(doi) = self.get_ignore_case("doi") {
            if !doi.starts_with("10.") {
                errors.push(ValidationError::warning(
                    Some("doi"),
                    "DOI should start with '10.'",
                ));
            }
        }

        // URL format
        if let Some(url) = self.get_ignore_case("url") {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                errors.push(ValidationError::warning(
                    Some("url"),
                    "URL should start with http:// or https://",
                ));
            }
        }

        // ISBN format (basic check)
        if let Some(isbn) = self.get_ignore_case("isbn") {
            let digits_only: String = isbn.chars().filter(char::is_ascii_digit).collect();
            if digits_only.len() != 10 && digits_only.len() != 13 {
                errors.push(ValidationError::warning(
                    Some("isbn"),
                    "ISBN should have 10 or 13 digits",
                ));
            }
        }

        // Month should be valid
        if let Some(month) = self.get_ignore_case("month") {
            if !is_valid_month(month) {
                errors.push(ValidationError::info(
                    Some("month"),
                    "Month should be a standard abbreviation (jan, feb, etc.) or full name",
                ));
            }
        }

        // Volume and number should be numeric if present
        for field_name in &["volume", "number"] {
            if let Some(value) = self.get_ignore_case(field_name) {
                if value.parse::<i32>().is_err() && !value.contains('-') {
                    errors.push(ValidationError::info(
                        Some(field_name),
                        format!("{field_name} should typically be numeric"),
                    ));
                }
            }
        }
    }

    /// Validate cross-references for strict checking
    fn validate_cross_references(&self, errors: &mut Vec<ValidationError>) {
        if let Some(crossref) = self.get_ignore_case("crossref") {
            if crossref.trim().is_empty() {
                errors.push(ValidationError::error(
                    Some("crossref"),
                    "Cross-reference is empty",
                ));
            }
        }
    }

    /// Helper to check if entry has a field (case-insensitive)
    fn has_field(&self, name: &str) -> bool {
        self.get_ignore_case(name).is_some() || self.get_as_string_ignore_case(name).is_some()
    }

    /// Check if entry has all required fields for its type (backward compatible)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validate(ValidationLevel::Minimal).is_ok()
    }

    /// Get a field value with LaTeX sequences converted to Unicode (case-sensitive)
    ///
    /// This method converts common LaTeX escape sequences like `\'e` to `é` and `\"{o}` to `ö`.
    /// Returns `None` if the field doesn't exist or isn't a string literal.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "latex_to_unicode")]
    /// # {
    /// # use bibtex_parser::Database;
    /// let bibtex = r#"@article{test, author = "Jos\'e Garc\'ia"}"#;
    /// let db = Database::parser().parse(bibtex).unwrap();
    /// let entry = &db.entries()[0];
    /// assert_eq!(entry.get_unicode("author"), Some("José García".to_string()));
    /// # }
    /// ```
    #[cfg(feature = "latex_to_unicode")]
    #[must_use]
    pub fn get_unicode(&self, name: &str) -> Option<String> {
        self.get(name).map(crate::latex_unicode::latex_to_unicode)
    }

    /// Get a field value with LaTeX sequences converted to Unicode (case-insensitive)
    ///
    /// This method converts common LaTeX escape sequences like `\'e` to `é` and `\"{o}` to `ö`.
    /// Returns `None` if the field doesn't exist or isn't a string literal.
    /// Field name matching is case-insensitive.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "latex_to_unicode")]
    /// # {
    /// # use bibtex_parser::Database;
    /// let bibtex = r#"@article{test, TITLE = "M\\\"uller's work"}"#;
    /// let db = Database::parser().parse(bibtex).unwrap();
    /// let entry = &db.entries()[0];
    /// assert_eq!(entry.get_unicode_ignore_case("title"), Some("Müller's work".to_string()));
    /// # }
    /// ```
    #[cfg(feature = "latex_to_unicode")]
    #[must_use]
    pub fn get_unicode_ignore_case(&self, name: &str) -> Option<String> {
        self.get_ignore_case(name)
            .map(crate::latex_unicode::latex_to_unicode)
    }

    /// Get a field value as string with LaTeX conversion (case-sensitive)
    ///
    /// Similar to `get_as_string()` but converts LaTeX sequences to Unicode.
    /// This handles all field types (literals, numbers, variables, concatenations).
    #[cfg(feature = "latex_to_unicode")]
    #[must_use]
    pub fn get_as_unicode_string(&self, name: &str) -> Option<String> {
        self.get_as_string(name)
            .map(|s| crate::latex_unicode::latex_to_unicode(&s))
    }

    /// Get a field value as string with LaTeX conversion (case-insensitive)
    ///
    /// Similar to `get_as_string_ignore_case()` but converts LaTeX sequences to Unicode.
    /// This handles all field types (literals, numbers, variables, concatenations).
    #[cfg(feature = "latex_to_unicode")]
    #[must_use]
    pub fn get_as_unicode_string_ignore_case(&self, name: &str) -> Option<String> {
        self.get_as_string_ignore_case(name)
            .map(|s| crate::latex_unicode::latex_to_unicode(&s))
    }

    /// Get all fields with LaTeX converted to Unicode
    ///
    /// Returns a vector of (`field_name`, `unicode_value`) pairs for all string literal fields.
    /// Non-string fields (numbers, variables) are excluded.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "latex_to_unicode")]
    /// # {
    /// # use bibtex_parser::Database;
    /// let bibtex = r#"@article{test,
    ///     author = "Jos\'e Garc\'ia",
    ///     title = "\\alpha and \\beta particles",
    ///     year = 2024
    /// }"#;
    /// let db = Database::parser().parse(bibtex).unwrap();
    /// let entry = &db.entries()[0];
    /// let unicode_fields = entry.fields_unicode();
    ///
    /// let author = unicode_fields.iter()
    ///     .find(|(k, _)| k == "author")
    ///     .map(|(_, v)| v.as_str())
    ///     .unwrap();
    /// assert_eq!(author, "José García");
    /// # }
    /// ```
    #[cfg(feature = "latex_to_unicode")]
    #[must_use]
    pub fn fields_unicode(&self) -> Vec<(String, String)> {
        self.fields
            .iter()
            .filter_map(|f| {
                f.value.as_str().map(|s| {
                    (
                        f.name.to_string(),
                        crate::latex_unicode::latex_to_unicode(s),
                    )
                })
            })
            .collect()
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
        let bytes = s.as_bytes();
        if bytes.is_empty() {
            return Self::Custom(Cow::Borrowed(s));
        }

        match ascii_lower(bytes[0]) {
            b'a' if s.eq_ignore_ascii_case("article") => Self::Article,
            b'b' if s.eq_ignore_ascii_case("book") => Self::Book,
            b'c' if s.eq_ignore_ascii_case("conference") => Self::InProceedings,
            b'i' if s.eq_ignore_ascii_case("inbook") => Self::InBook,
            b'i' if s.eq_ignore_ascii_case("inproceedings") => Self::InProceedings,
            b'm' if s.eq_ignore_ascii_case("mastersthesis") => Self::MastersThesis,
            b'm' if s.eq_ignore_ascii_case("misc") => Self::Misc,
            b'p' if s.eq_ignore_ascii_case("phdthesis") => Self::PhdThesis,
            b'p' if s.eq_ignore_ascii_case("proceedings") => Self::Proceedings,
            b't' if s.eq_ignore_ascii_case("techreport") => Self::TechReport,
            b'u' if s.eq_ignore_ascii_case("unpublished") => Self::Unpublished,
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

#[inline]
const fn ascii_lower(byte: u8) -> u8 {
    if b'A' <= byte && byte <= b'Z' {
        byte + (b'a' - b'A')
    } else {
        byte
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

    /// Check if field name matches (case-insensitive)
    #[must_use]
    pub fn name_eq_ignore_case(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
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

/// Check if a string is a valid page range
/// Accepts formats like "12", "12-34", "12--34", "12-34,45-67"
fn is_valid_page_range(pages: &str) -> bool {
    if pages.trim().is_empty() {
        return false;
    }

    // Accept single page numbers
    if pages.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    // Check for range patterns - must contain dash or comma
    if !pages.contains('-') && !pages.contains(',') {
        return false;
    }

    // Split by comma for multiple ranges
    for range in pages.split(',') {
        let range = range.trim();
        if range.is_empty() {
            continue;
        }

        // Check individual range
        if range.contains("--") {
            // LaTeX-style double dash
            let parts: Vec<&str> = range.split("--").collect();
            if parts.len() != 2 || parts.iter().any(|p| p.trim().is_empty()) {
                return false;
            }
        } else if range.contains('-') {
            // Single dash
            let parts: Vec<&str> = range.split('-').collect();
            if parts.len() != 2 || parts.iter().any(|p| p.trim().is_empty()) {
                return false;
            }
        }
    }

    true
}

/// Check if a month value is valid
/// Accepts standard month abbreviations and full month names
fn is_valid_month(month: &str) -> bool {
    let month_lower = month.to_lowercase();

    // Standard BibTeX month abbreviations and full names
    matches!(
        month_lower.as_str(),
        "jan"
            | "feb"
            | "mar"
            | "apr"
            | "may"
            | "jun"
            | "jul"
            | "aug"
            | "sep"
            | "oct"
            | "nov"
            | "dec"
            | "january"
            | "february"
            | "march"
            | "april"
            | "june"
            | "july"
            | "august"
            | "september"
            | "october"
            | "november"
            | "december"
    ) || month.parse::<i32>().is_ok_and(|m| (1..=12).contains(&m))
}
