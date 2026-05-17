//! Data models for BibTeX entries

use ahash::AHashMap;
use memchr::memchr2;
use std::borrow::Cow;
use std::fmt;

/// Validation strictness level for BibTeX entries
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ValidationLevel {
    /// Only check that required fields exist
    Minimal,
    /// Check required fields and common issues (default)
    #[default]
    Standard,
    /// Strict validation including field formats and cross-references
    Strict,
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
    /// Informational note
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

/// A structured BibTeX person name.
///
/// BibTeX supports the forms `First von Last`, `von Last, First`, and
/// `von Last, Jr, First`. This type keeps those four logical parts separate
/// while preserving the exact token text from the source value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonName {
    /// Exact source text for this name segment, trimmed of surrounding whitespace.
    pub raw: String,
    /// Given names and initials.
    pub first: String,
    /// Lowercase particles such as `von`, `van`, `de`, or `der`.
    pub von: String,
    /// Family name.
    pub last: String,
    /// Junior part such as `Jr.` in `Last, Jr., First`.
    pub jr: String,
    /// Given-name tokens.
    pub given: Vec<String>,
    /// Family-name tokens.
    pub family: Vec<String>,
    /// Prefix or particle tokens.
    pub prefix: Vec<String>,
    /// Suffix tokens.
    pub suffix: Vec<String>,
    /// Literal organization or preserved braced name.
    pub literal: Option<String>,
}

impl PersonName {
    /// Return the display form used by most bibliography styles.
    #[must_use]
    pub fn display_name(&self) -> String {
        if let Some(literal) = &self.literal {
            return literal.clone();
        }

        let mut parts = Vec::new();
        if !self.first.is_empty() {
            parts.push(self.first.as_str());
        }
        if !self.von.is_empty() {
            parts.push(self.von.as_str());
        }
        if !self.last.is_empty() {
            parts.push(self.last.as_str());
        }

        let mut name = parts.join(" ");
        if !self.jr.is_empty() {
            if !name.is_empty() {
                name.push_str(", ");
            }
            name.push_str(&self.jr);
        }
        name
    }

    /// Return `true` when every name component is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
            && self.first.is_empty()
            && self.von.is_empty()
            && self.last.is_empty()
            && self.jr.is_empty()
            && self.literal.is_none()
    }

    /// Return `true` when the name is a braced literal or organization name.
    #[must_use]
    pub const fn is_literal(&self) -> bool {
        self.literal.is_some()
    }

    /// Return the display name after LaTeX-to-Unicode conversion.
    #[cfg(feature = "latex_to_unicode")]
    #[must_use]
    pub fn unicode_display_name(&self) -> String {
        crate::latex_unicode::latex_to_unicode(&self.display_name())
    }
}

/// Parse a BibTeX `author` or `editor` field into structured person names.
///
/// Splitting respects balanced braces, so organization names such as
/// `{The Unicode Consortium}` and literal `and` inside braces stay intact.
#[must_use]
pub fn parse_names(input: &str) -> Vec<PersonName> {
    split_bibtex_names(input)
        .into_iter()
        .map(parse_single_name)
        .filter(|name| !name.is_empty())
        .collect()
}

/// Parsed bibliography date parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateParts {
    /// Four-digit year.
    pub year: i32,
    /// One-based month, when present.
    pub month: Option<u8>,
    /// One-based day of month, when present.
    pub day: Option<u8>,
}

impl DateParts {
    /// Return `true` when both month and day are present.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.month.is_some() && self.day.is_some()
    }
}

/// Explicit date parse failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateParseError {
    /// Input was empty after trimming BibTeX delimiters.
    Empty,
    /// Year was missing or not a four-digit number.
    InvalidYear,
    /// Month was present but outside `1..=12` or unrecognized.
    InvalidMonth,
    /// Day was present but invalid for the parsed year and month.
    InvalidDay,
    /// Input used a shape outside the supported common bibliography forms.
    UnsupportedFormat,
}

impl fmt::Display for DateParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty date"),
            Self::InvalidYear => f.write_str("invalid date year"),
            Self::InvalidMonth => f.write_str("invalid date month"),
            Self::InvalidDay => f.write_str("invalid date day"),
            Self::UnsupportedFormat => f.write_str("unsupported date format"),
        }
    }
}

impl std::error::Error for DateParseError {}

/// Common resource or identifier field kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    /// Local file attachment field.
    File,
    /// URL field.
    Url,
    /// DOI field.
    Doi,
    /// `PubMed` identifier.
    Pmid,
    /// `PubMed Central` identifier.
    Pmcid,
    /// ISBN field.
    Isbn,
    /// ISSN field.
    Issn,
    /// Generic eprint field.
    Eprint,
    /// arXiv identifier.
    Arxiv,
    /// Cross-reference citation key.
    Crossref,
}

impl ResourceKind {
    /// Return a stable lowercase kind name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Url => "url",
            Self::Doi => "doi",
            Self::Pmid => "pmid",
            Self::Pmcid => "pmcid",
            Self::Isbn => "isbn",
            Self::Issn => "issn",
            Self::Eprint => "eprint",
            Self::Arxiv => "arxiv",
            Self::Crossref => "crossref",
        }
    }
}

/// Classified resource or identifier field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceField {
    /// Classified field kind.
    pub kind: ResourceKind,
    /// Original field name spelling.
    pub field_name: String,
    /// Plain text value.
    pub value: String,
    /// Normalized value when the kind has a stable local normalization.
    pub normalized: Option<String>,
}

/// Parse a common bibliography date shape into parts.
///
/// Supported input shapes are `YYYY`, `YYYY-MM`, and `YYYY-MM-DD`.
/// Month names and BibTeX month abbreviations are accepted by entry helpers
/// when a separate `month` field is combined with a `year` field.
pub fn parse_date_parts(input: &str) -> std::result::Result<DateParts, DateParseError> {
    let cleaned = trim_bibtex_scalar(input);
    if cleaned.is_empty() {
        return Err(DateParseError::Empty);
    }

    let parts = cleaned.split('-').collect::<Vec<_>>();
    match parts.as_slice() {
        [year] => Ok(DateParts {
            year: parse_year(year)?,
            month: None,
            day: None,
        }),
        [year, month] => {
            let year = parse_year(year)?;
            let month = parse_month_number(month).ok_or(DateParseError::InvalidMonth)?;
            Ok(DateParts {
                year,
                month: Some(month),
                day: None,
            })
        }
        [year, month, day] => {
            let year = parse_year(year)?;
            let month = parse_month_number(month).ok_or(DateParseError::InvalidMonth)?;
            let day = parse_day_number(day, year, month)?;
            Ok(DateParts {
                year,
                month: Some(month),
                day: Some(day),
            })
        }
        _ => Err(DateParseError::UnsupportedFormat),
    }
}

/// Normalize a field name to ASCII lowercase.
#[must_use]
pub fn normalize_field_name_ascii(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

/// Return the crate's built-in BibLaTeX-to-BibTeX field alias, if any.
#[must_use]
pub fn canonical_biblatex_field_alias(name: &str) -> Option<&'static str> {
    match normalize_field_name_ascii(name).as_str() {
        "journaltitle" => Some("journal"),
        "date" => Some("year"),
        "institution" => Some("school"),
        "location" => Some("address"),
        _ => None,
    }
}

/// Normalize a field name with ASCII lowercase and built-in BibLaTeX aliases.
#[must_use]
pub fn normalize_biblatex_field_name(name: &str) -> String {
    canonical_biblatex_field_alias(name)
        .map_or_else(|| normalize_field_name_ascii(name), ToOwned::to_owned)
}

/// Classify a common resource or identifier field name.
#[must_use]
pub fn classify_resource_field(name: &str) -> Option<ResourceKind> {
    match normalize_field_name_ascii(name).as_str() {
        "file" => Some(ResourceKind::File),
        "url" => Some(ResourceKind::Url),
        "doi" => Some(ResourceKind::Doi),
        "pmid" => Some(ResourceKind::Pmid),
        "pmcid" => Some(ResourceKind::Pmcid),
        "isbn" => Some(ResourceKind::Isbn),
        "issn" => Some(ResourceKind::Issn),
        "eprint" => Some(ResourceKind::Eprint),
        "arxiv" => Some(ResourceKind::Arxiv),
        "crossref" => Some(ResourceKind::Crossref),
        _ => None,
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

    /// Get a field by name (case-sensitive).
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Field<'a>> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Get a field by name (case-insensitive).
    #[must_use]
    pub fn field_ignore_case(&self, name: &str) -> Option<&Field<'a>> {
        self.fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// Get a field value by name (case-sensitive)
    /// Note: This only returns string literals, not numbers
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.field(name).and_then(|f| f.value.as_str())
    }

    /// Get a field value by name (case-insensitive)
    /// Returns the first field whose name matches ignoring case
    /// Note: This only returns string literals, not numbers
    #[must_use]
    pub fn get_ignore_case(&self, name: &str) -> Option<&str> {
        self.field_ignore_case(name).and_then(|f| f.value.as_str())
    }

    /// Get a field value as a string, converting numbers if necessary (case-sensitive)
    #[must_use]
    pub fn get_as_string(&self, name: &str) -> Option<String> {
        self.field(name).map(|f| value_to_lossy_string(&f.value))
    }

    /// Get a field value as a string, converting numbers if necessary (case-insensitive)
    #[must_use]
    pub fn get_as_string_ignore_case(&self, name: &str) -> Option<String> {
        self.field_ignore_case(name)
            .map(|f| value_to_lossy_string(&f.value))
    }

    /// Get the first string-literal field matching any of the names, case-insensitively.
    #[must_use]
    pub fn get_any_ignore_case(&self, names: &[&str]) -> Option<&str> {
        names.iter().find_map(|name| self.get_ignore_case(name))
    }

    /// Get the first field matching any of the names as a string, case-insensitively.
    #[must_use]
    pub fn get_any_as_string_ignore_case(&self, names: &[&str]) -> Option<String> {
        names
            .iter()
            .find_map(|name| self.get_as_string_ignore_case(name))
    }

    /// Return `true` when a field exists, ignoring ASCII case.
    #[must_use]
    pub fn has_field(&self, name: &str) -> bool {
        self.field_ignore_case(name).is_some()
    }

    /// Return `true` when any field in `names` exists, ignoring ASCII case.
    #[must_use]
    pub fn has_any_field(&self, names: &[&str]) -> bool {
        names.iter().any(|name| self.has_field(name))
    }

    /// Return the normalized DOI, if the entry has a recognizable DOI field.
    ///
    /// This accepts common input forms such as `10.1000/xyz`,
    /// `doi:10.1000/xyz`, and `https://doi.org/10.1000/xyz`.
    #[must_use]
    pub fn doi(&self) -> Option<String> {
        self.get_as_string_ignore_case("doi")
            .and_then(|doi| normalize_doi(&doi))
    }

    /// Parse the `author` field into structured BibTeX names.
    #[must_use]
    pub fn authors(&self) -> Vec<PersonName> {
        self.get_as_string_ignore_case("author")
            .map_or_else(Vec::new, |authors| parse_names(&authors))
    }

    /// Parse the `editor` field into structured BibTeX names.
    #[must_use]
    pub fn editors(&self) -> Vec<PersonName> {
        self.get_as_string_ignore_case("editor")
            .map_or_else(Vec::new, |editors| parse_names(&editors))
    }

    /// Parse the `translator` field into structured BibTeX names.
    #[must_use]
    pub fn translators(&self) -> Vec<PersonName> {
        self.get_as_string_ignore_case("translator")
            .map_or_else(Vec::new, |translators| parse_names(&translators))
    }

    /// Parse a specific date-like field into date parts.
    #[must_use]
    pub fn date_parts_for(
        &self,
        field: &str,
    ) -> Option<std::result::Result<DateParts, DateParseError>> {
        self.get_as_string_ignore_case(field)
            .map(|value| parse_date_parts(&value))
    }

    /// Return issued date parts for this entry.
    ///
    /// `date`, `issued`, `eventdate`, `origdate`, and `urldate` are checked
    /// before falling back to `year` plus an optional `month` field.
    #[must_use]
    pub fn date_parts(&self) -> Option<std::result::Result<DateParts, DateParseError>> {
        for field in &["date", "issued", "eventdate", "origdate", "urldate"] {
            if let Some(value) = self.get_as_string_ignore_case(field) {
                return Some(parse_date_parts(&value));
            }
        }

        let year = self.get_as_string_ignore_case("year")?;
        let mut parts = match parse_date_parts(&year) {
            Ok(parts) => parts,
            Err(error) => return Some(Err(error)),
        };
        if let Some(month) = self.get_as_string_ignore_case("month") {
            match parse_month_number(&month) {
                Some(month) => parts.month = Some(month),
                None => return Some(Err(DateParseError::InvalidMonth)),
            }
        }
        Some(Ok(parts))
    }

    /// Return classified resource and identifier fields in source order.
    #[must_use]
    pub fn resource_fields(&self) -> Vec<ResourceField> {
        let archive_prefix = self
            .get_as_string_ignore_case("archiveprefix")
            .or_else(|| self.get_as_string_ignore_case("eprinttype"));

        self.fields
            .iter()
            .filter_map(|field| {
                resource_field_from_parts(
                    &field.name,
                    field.value.to_plain_string(),
                    archive_prefix.as_deref(),
                )
            })
            .collect()
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

    /// Set a field value, replacing the first matching field or appending it.
    pub fn set(&mut self, name: &'a str, value: Value<'a>) {
        if let Some(field) = self.fields.iter_mut().find(|field| field.name == name) {
            field.value = value;
        } else {
            self.fields.push(Field::new(name, value));
        }
    }

    /// Set a field to a string literal.
    pub fn set_literal(&mut self, name: &'a str, value: &'a str) {
        self.set(name, Value::Literal(Cow::Borrowed(value)));
    }

    /// Remove all fields whose name matches exactly.
    pub fn remove(&mut self, name: &str) -> Vec<Field<'a>> {
        let mut removed = Vec::new();
        let mut index = 0;
        while index < self.fields.len() {
            if self.fields[index].name == name {
                removed.push(self.fields.remove(index));
            } else {
                index += 1;
            }
        }
        removed
    }

    /// Rename all fields whose name matches exactly.
    pub fn rename_field(&mut self, old: &str, new: &'a str) -> usize {
        let mut renamed = 0;
        for field in &mut self.fields {
            if field.name == old {
                field.name = Cow::Borrowed(new);
                renamed += 1;
            }
        }
        renamed
    }

    /// Return the title field as a string.
    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.get_any_as_string_ignore_case(&["title"])
    }

    /// Return the year field as a string.
    #[must_use]
    pub fn year(&self) -> Option<String> {
        self.get_any_as_string_ignore_case(&["year"])
    }

    /// Return the date field as a string.
    #[must_use]
    pub fn date(&self) -> Option<String> {
        self.get_any_as_string_ignore_case(&["date"])
    }

    /// Return the journal field, accepting BibLaTeX's `journaltitle` alias.
    #[must_use]
    pub fn journal(&self) -> Option<String> {
        self.get_any_as_string_ignore_case(&["journal", "journaltitle"])
    }

    /// Return the book title field as a string.
    #[must_use]
    pub fn booktitle(&self) -> Option<String> {
        self.get_any_as_string_ignore_case(&["booktitle"])
    }

    /// Return the URL field as a string.
    #[must_use]
    pub fn url(&self) -> Option<String> {
        self.get_any_as_string_ignore_case(&["url"])
    }

    /// Return keywords split on commas or semicolons.
    #[must_use]
    pub fn keywords(&self) -> Vec<String> {
        self.get_any_as_string_ignore_case(&["keywords", "keyword"])
            .map(|keywords| {
                keywords
                    .split([',', ';'])
                    .map(str::trim)
                    .filter(|keyword| !keyword.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default()
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
        for &field_group in self.ty.required_field_groups() {
            if self.has_any_field(field_group) {
                continue;
            }

            if field_group == ["author", "editor"] {
                errors.push(ValidationError::error(
                    None,
                    format!(
                        "{} entry must have either 'author' or 'editor' field",
                        self.ty
                    ),
                ));
                continue;
            }

            let primary_field = field_group[0];
            let message = if field_group.len() == 1 {
                format!(
                    "Required field '{}' is missing for {} entry",
                    primary_field, self.ty
                )
            } else {
                format!(
                    "Required field '{}' is missing for {} entry (accepted aliases: {})",
                    primary_field,
                    self.ty,
                    field_group.join(", ")
                )
            };

            errors.push(ValidationError::error(Some(primary_field), message));
        }
    }

    /// Validate common issues that might cause problems
    fn validate_common_issues(&self, errors: &mut Vec<ValidationError>) {
        // Check for common issues

        // Year should be a valid number and recent
        if let Some(year_str) = self.get_any_as_string_ignore_case(&["year", "date"]) {
            if let Ok(year) = year_str.parse::<i32>() {
                if !(1000..=2100).contains(&year) {
                    errors.push(ValidationError::warning(
                        Some(if self.has_field("year") {
                            "year"
                        } else {
                            "date"
                        }),
                        format!("Year {year} seems unlikely"),
                    ));
                }
            } else {
                errors.push(ValidationError::warning(
                    Some(if self.has_field("year") {
                        "year"
                    } else {
                        "date"
                    }),
                    "Year/date should be a number",
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
            EntryType::InBook | EntryType::InProceedings | EntryType::InCollection
                if !self.has_any_field(&["author", "editor"]) =>
            {
                errors.push(ValidationError::warning(
                    None,
                    "Entry should have either 'author' or 'editor' field",
                ));
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
        if let Some(doi) = self.get_as_string_ignore_case("doi") {
            if normalize_doi(&doi).is_none() {
                errors.push(ValidationError::warning(
                    Some("doi"),
                    "DOI should start with '10.' or a DOI URL/prefix",
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
            if !is_valid_isbn_shape(isbn) {
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

    /// Check whether the entry has the minimal required fields for its type.
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
    /// # use bibtex_parser::Library;
    /// let bibtex = r#"@article{test, author = "Jos\'e Garc\'ia"}"#;
    /// let library = Library::parser().parse(bibtex).unwrap();
    /// let entry = &library.entries()[0];
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
    /// # use bibtex_parser::Library;
    /// let bibtex = r#"@article{test, TITLE = "M\\\"uller's work"}"#;
    /// let library = Library::parser().parse(bibtex).unwrap();
    /// let entry = &library.entries()[0];
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
    /// # use bibtex_parser::Library;
    /// let bibtex = r#"@article{test,
    ///     author = "Jos\'e Garc\'ia",
    ///     title = "\\alpha and \\beta particles",
    ///     year = 2024
    /// }"#;
    /// let library = Library::parser().parse(bibtex).unwrap();
    /// let entry = &library.entries()[0];
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
    /// Booklet without a named publisher
    Booklet,
    /// A multi-volume book (`biblatex`)
    MvBook,
    /// Part of a book
    InBook,
    /// A self-contained book part published as a book (`biblatex`)
    BookInBook,
    /// Supplemental material in a book (`biblatex`)
    SuppBook,
    /// A collection with its own title
    Collection,
    /// A multi-volume collection (`biblatex`)
    MvCollection,
    /// A contribution to a collection
    InCollection,
    /// Supplemental material in a collection (`biblatex`)
    SuppCollection,
    /// Article in conference proceedings
    InProceedings,
    /// Conference proceedings
    Proceedings,
    /// Multi-volume proceedings (`biblatex`)
    MvProceedings,
    /// A reference work (`biblatex`)
    Reference,
    /// A contribution to a reference work (`biblatex`)
    InReference,
    /// Technical documentation or manual
    Manual,
    /// Master's thesis
    MastersThesis,
    /// `PhD` thesis
    PhdThesis,
    /// Generic thesis (`biblatex`)
    Thesis,
    /// Technical report
    TechReport,
    /// Generic report (`biblatex`)
    Report,
    /// Patent or patent request (`biblatex`)
    Patent,
    /// Periodical issue (`biblatex`)
    Periodical,
    /// Online resource (`biblatex`)
    Online,
    /// Software artifact (`biblatex` and common repository exports)
    Software,
    /// Dataset artifact (`biblatex` and common repository exports)
    Dataset,
    /// Entry set (`biblatex`)
    Set,
    /// Reusable data-only entry (`biblatex`)
    XData,
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
    #[inline(never)]
    pub fn parse(s: &'a str) -> Self {
        let bytes = s.as_bytes();
        if bytes.is_empty() {
            return Self::Custom(Cow::Borrowed(s));
        }

        match (bytes.len(), ascii_lower(bytes[0])) {
            (3, b's') if eq_ascii_lower(bytes, b"set") => Self::Set,
            (4, b'b') if eq_ascii_lower(bytes, b"book") => Self::Book,
            (4, b'm') if eq_ascii_lower(bytes, b"misc") => Self::Misc,
            (6, b'i') if eq_ascii_lower(bytes, b"inbook") => Self::InBook,
            (6, b'm') if eq_ascii_lower(bytes, b"manual") => Self::Manual,
            (6, b'm') if eq_ascii_lower(bytes, b"mvbook") => Self::MvBook,
            (6, b'o') if eq_ascii_lower(bytes, b"online") => Self::Online,
            (6, b'p') if eq_ascii_lower(bytes, b"patent") => Self::Patent,
            (6, b'r') if eq_ascii_lower(bytes, b"report") => Self::Report,
            (6, b't') if eq_ascii_lower(bytes, b"thesis") => Self::Thesis,
            (7, b'a') if eq_ascii_lower(bytes, b"article") => Self::Article,
            (7, b'b') if eq_ascii_lower(bytes, b"booklet") => Self::Booklet,
            (7, b'd') if eq_ascii_lower(bytes, b"dataset") => Self::Dataset,
            (8, b's') if eq_ascii_lower(bytes, b"software") => Self::Software,
            (8, b's') if eq_ascii_lower(bytes, b"suppbook") => Self::SuppBook,
            (9, b'r') if eq_ascii_lower(bytes, b"reference") => Self::Reference,
            (9, b'p') if eq_ascii_lower(bytes, b"phdthesis") => Self::PhdThesis,
            (10, b'b') if eq_ascii_lower(bytes, b"bookinbook") => Self::BookInBook,
            (10, b'c') if eq_ascii_lower(bytes, b"conference") => Self::InProceedings,
            (10, b'c') if eq_ascii_lower(bytes, b"collection") => Self::Collection,
            (10, b'p') if eq_ascii_lower(bytes, b"periodical") => Self::Periodical,
            (10, b't') if eq_ascii_lower(bytes, b"techreport") => Self::TechReport,
            (11, b'i') if eq_ascii_lower(bytes, b"inreference") => Self::InReference,
            (11, b'p') if eq_ascii_lower(bytes, b"proceedings") => Self::Proceedings,
            (11, b'u') if eq_ascii_lower(bytes, b"unpublished") => Self::Unpublished,
            (12, b'i') if eq_ascii_lower(bytes, b"incollection") => Self::InCollection,
            (12, b'm') if eq_ascii_lower(bytes, b"mvcollection") => Self::MvCollection,
            (13, b'i') if eq_ascii_lower(bytes, b"inproceedings") => Self::InProceedings,
            (13, b'm') if eq_ascii_lower(bytes, b"mastersthesis") => Self::MastersThesis,
            (13, b'm') if eq_ascii_lower(bytes, b"mvproceedings") => Self::MvProceedings,
            (14, b's') if eq_ascii_lower(bytes, b"suppcollection") => Self::SuppCollection,
            (5, b'x') if eq_ascii_lower(bytes, b"xdata") => Self::XData,
            _ => Self::Custom(Cow::Borrowed(s)),
        }
    }

    /// Get required fields for this entry type
    #[must_use]
    pub const fn required_fields(&self) -> &'static [&'static str] {
        match self {
            Self::Article => &["author", "title", "journal", "year"],
            Self::Book | Self::MvBook => &["author", "title", "publisher", "year"],
            Self::Booklet | Self::Manual => &["title"],
            Self::InBook | Self::BookInBook | Self::SuppBook => {
                &["author", "title", "chapter", "publisher", "year"]
            }
            Self::Collection | Self::MvCollection | Self::Reference => {
                &["editor", "title", "publisher", "year"]
            }
            Self::InCollection | Self::SuppCollection | Self::InReference => {
                &["author", "title", "booktitle", "publisher", "year"]
            }
            Self::InProceedings => &["author", "title", "booktitle", "year"],
            Self::Proceedings | Self::MvProceedings | Self::Periodical => &["title", "year"],
            Self::MastersThesis | Self::PhdThesis | Self::Thesis => {
                &["author", "title", "school", "year"]
            }
            Self::TechReport => &["author", "title", "institution", "year"],
            Self::Report => &["author", "title", "type", "institution", "year"],
            Self::Patent => &["author", "title", "number", "year"],
            Self::Online => &["title", "url"],
            Self::Software | Self::Dataset => &["author", "title", "year"],
            Self::Unpublished => &["author", "title", "note"],
            Self::Misc | Self::Set | Self::XData | Self::Custom(_) => &[],
        }
    }

    /// Get required field groups for validation.
    ///
    /// Each inner group is an OR-list. For example, `["author", "editor"]`
    /// means either field satisfies that requirement.
    #[must_use]
    pub const fn required_field_groups(&self) -> &'static [&'static [&'static str]] {
        match self {
            Self::Article => &[
                &["author"],
                &["title"],
                &["journal", "journaltitle"],
                &["year", "date"],
            ],
            Self::Book | Self::MvBook => &[
                &["author", "editor"],
                &["title"],
                &["publisher"],
                &["year", "date"],
            ],
            Self::Booklet | Self::Manual => &[&["title"]],
            Self::InBook | Self::BookInBook | Self::SuppBook => &[
                &["author", "editor"],
                &["title"],
                &["chapter", "pages"],
                &["publisher"],
                &["year", "date"],
            ],
            Self::Collection | Self::MvCollection | Self::Reference => &[
                &["editor", "author"],
                &["title"],
                &["publisher"],
                &["year", "date"],
            ],
            Self::InCollection | Self::SuppCollection | Self::InReference => &[
                &["author", "editor"],
                &["title"],
                &["booktitle"],
                &["publisher"],
                &["year", "date"],
            ],
            Self::InProceedings => &[
                &["author", "editor"],
                &["title"],
                &["booktitle"],
                &["year", "date"],
            ],
            Self::Proceedings | Self::MvProceedings | Self::Periodical => {
                &[&["title"], &["year", "date"]]
            }
            Self::MastersThesis | Self::PhdThesis | Self::Thesis => &[
                &["author"],
                &["title"],
                &["school", "institution"],
                &["year", "date"],
            ],
            Self::TechReport => &[&["author"], &["title"], &["institution"], &["year", "date"]],
            Self::Report => &[
                &["author", "editor"],
                &["title"],
                &["type"],
                &["institution"],
                &["year", "date"],
            ],
            Self::Patent => &[&["author"], &["title"], &["number"], &["year", "date"]],
            Self::Online => &[&["title"], &["url", "doi"], &["year", "date", "urldate"]],
            Self::Software | Self::Dataset => &[
                &["author", "editor"],
                &["title"],
                &["year", "date", "version"],
            ],
            Self::Unpublished => &[&["author"], &["title"], &["note"]],
            Self::Misc | Self::Set | Self::XData | Self::Custom(_) => &[],
        }
    }

    /// Return the canonical lowercase entry type name.
    #[must_use]
    pub fn canonical_name(&self) -> &str {
        match self {
            Self::Article => "article",
            Self::Book => "book",
            Self::Booklet => "booklet",
            Self::MvBook => "mvbook",
            Self::InBook => "inbook",
            Self::BookInBook => "bookinbook",
            Self::SuppBook => "suppbook",
            Self::Collection => "collection",
            Self::MvCollection => "mvcollection",
            Self::InCollection => "incollection",
            Self::SuppCollection => "suppcollection",
            Self::InProceedings => "inproceedings",
            Self::Proceedings => "proceedings",
            Self::MvProceedings => "mvproceedings",
            Self::Reference => "reference",
            Self::InReference => "inreference",
            Self::Manual => "manual",
            Self::MastersThesis => "mastersthesis",
            Self::PhdThesis => "phdthesis",
            Self::Thesis => "thesis",
            Self::TechReport => "techreport",
            Self::Report => "report",
            Self::Patent => "patent",
            Self::Periodical => "periodical",
            Self::Online => "online",
            Self::Software => "software",
            Self::Dataset => "dataset",
            Self::Set => "set",
            Self::XData => "xdata",
            Self::Unpublished => "unpublished",
            Self::Misc => "misc",
            Self::Custom(s) => s,
        }
    }

    /// Return common aliases that parse to this entry type.
    #[must_use]
    pub const fn aliases(&self) -> &'static [&'static str] {
        match self {
            Self::InProceedings => &["conference"],
            Self::TechReport => &["techreport"],
            Self::MastersThesis => &["mastersthesis"],
            Self::PhdThesis => &["phdthesis"],
            _ => &[],
        }
    }

    /// Return `true` for the classic BibTeX entry types.
    #[must_use]
    pub const fn is_classic_bibtex(&self) -> bool {
        matches!(
            self,
            Self::Article
                | Self::Book
                | Self::Booklet
                | Self::InBook
                | Self::InCollection
                | Self::InProceedings
                | Self::Manual
                | Self::MastersThesis
                | Self::PhdThesis
                | Self::Proceedings
                | Self::TechReport
                | Self::Unpublished
                | Self::Misc
        )
    }

    /// Return `true` for entry types that are specific to BibLaTeX or common repository exports.
    #[must_use]
    pub const fn is_extended(&self) -> bool {
        !self.is_classic_bibtex() && !matches!(self, Self::Custom(_))
    }

    /// Convert to owned version
    #[must_use]
    pub fn into_owned(self) -> EntryType<'static> {
        match self {
            Self::Custom(s) => EntryType::Custom(Cow::Owned(s.into_owned())),
            Self::Article => EntryType::Article,
            Self::Book => EntryType::Book,
            Self::Booklet => EntryType::Booklet,
            Self::MvBook => EntryType::MvBook,
            Self::InBook => EntryType::InBook,
            Self::BookInBook => EntryType::BookInBook,
            Self::SuppBook => EntryType::SuppBook,
            Self::Collection => EntryType::Collection,
            Self::MvCollection => EntryType::MvCollection,
            Self::InCollection => EntryType::InCollection,
            Self::SuppCollection => EntryType::SuppCollection,
            Self::InProceedings => EntryType::InProceedings,
            Self::Proceedings => EntryType::Proceedings,
            Self::MvProceedings => EntryType::MvProceedings,
            Self::Reference => EntryType::Reference,
            Self::InReference => EntryType::InReference,
            Self::Manual => EntryType::Manual,
            Self::MastersThesis => EntryType::MastersThesis,
            Self::PhdThesis => EntryType::PhdThesis,
            Self::Thesis => EntryType::Thesis,
            Self::TechReport => EntryType::TechReport,
            Self::Report => EntryType::Report,
            Self::Patent => EntryType::Patent,
            Self::Periodical => EntryType::Periodical,
            Self::Online => EntryType::Online,
            Self::Software => EntryType::Software,
            Self::Dataset => EntryType::Dataset,
            Self::Set => EntryType::Set,
            Self::XData => EntryType::XData,
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

#[inline]
fn eq_ascii_lower(input: &[u8], expected: &[u8]) -> bool {
    if input.len() != expected.len() {
        return false;
    }

    let mut index = 0usize;
    while index < input.len() {
        if ascii_lower(input[index]) != expected[index] {
            return false;
        }
        index += 1;
    }

    true
}

impl fmt::Display for EntryType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical_name())
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
/// Concatenation parts are stored out of line so the common literal, number,
/// and variable variants stay compact.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    /// String literal
    Literal(Cow<'a, str>),
    /// Number literal
    Number(i64),
    /// Concatenated values (boxed to reduce enum size)
    Concat(Box<[Self]>),
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
            Self::Literal(s) => normalize_text_projection(s),
            Self::Number(n) => n.to_string(),
            Self::Variable(name) => strings
                .get(name.as_ref())
                .map_or_else(|| format!("{{undefined:{name}}}"), |v| v.expand(strings)),
            Self::Concat(parts) => parts.iter().map(|p| p.expand(strings)).collect::<String>(),
        }
    }

    /// Project this value to ordinary text without adding unresolved-variable markers.
    ///
    /// Literals and numbers become their text form, variables become their
    /// variable name, and concatenations are joined recursively. This allocates
    /// when the value is not already a simple borrowed literal.
    #[must_use]
    pub fn to_plain_string(&self) -> String {
        value_to_plain_string(self)
    }

    /// Project this value to display text, marking unresolved variables as `{name}`.
    #[must_use]
    pub fn to_lossy_string(&self) -> String {
        value_to_lossy_string(self)
    }

    /// Create a literal value from ordinary text.
    #[must_use]
    pub fn from_plain_string<'a>(text: impl Into<Cow<'a, str>>) -> Value<'a> {
        Value::Literal(text.into())
    }

    /// Parse a BibTeX value fragment into a structured value.
    ///
    /// The input should be a field-value fragment such as `{Title}`, `venue`, or
    /// `"Part " # suffix`. Leading and trailing whitespace are accepted, but
    /// any other trailing text is rejected.
    pub fn from_bibtex_source(source: &str) -> crate::Result<Value<'_>> {
        let mut input = source;
        crate::parser::lexer::skip_whitespace(&mut input);
        let value = crate::parser::value::parse_value(&mut input)
            .map_err(|_| crate::Error::WinnowError("invalid BibTeX value source".to_string()))?;
        crate::parser::lexer::skip_whitespace(&mut input);
        if input.is_empty() {
            Ok(value)
        } else {
            Err(crate::Error::WinnowError(format!(
                "trailing text after BibTeX value: {input:?}"
            )))
        }
    }

    /// Serialize this value as a BibTeX value fragment.
    ///
    /// This is a normalized source projection. Use source-preserving parsing
    /// when callers need the exact original spelling or delimiters.
    #[must_use]
    pub fn to_bibtex_source(&self) -> String {
        match self {
            Self::Literal(text) => literal_to_bibtex_source(text),
            Self::Number(number) => number.to_string(),
            Self::Variable(name) => name.to_string(),
            Self::Concat(parts) => parts
                .iter()
                .map(Self::to_bibtex_source)
                .collect::<Vec<_>>()
                .join(" # "),
        }
    }

    /// Project this value to ordinary text and convert common LaTeX sequences to Unicode.
    #[cfg(feature = "latex_to_unicode")]
    #[must_use]
    pub fn to_unicode_plain_string(&self) -> String {
        crate::latex_unicode::latex_to_unicode(&self.to_plain_string())
    }

    /// Convert to owned version
    #[must_use]
    pub fn into_owned(self) -> Value<'static> {
        match self {
            Self::Literal(s) => Value::Literal(Cow::Owned(s.into_owned())),
            Self::Number(n) => Value::Number(n),
            Self::Variable(s) => Value::Variable(Cow::Owned(s.into_owned())),
            Self::Concat(parts) => Value::Concat(
                parts
                    .into_vec()
                    .into_iter()
                    .map(Value::into_owned)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
        }
    }
}

fn literal_to_bibtex_source(text: &str) -> String {
    if is_balanced_braced_literal_content(text) {
        format!("{{{text}}}")
    } else {
        format!("\"{}\"", escape_quoted_literal(text))
    }
}

fn is_balanced_braced_literal_content(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut pos = 0usize;

    while let Some(offset) = memchr2(b'{', b'}', &bytes[pos..]) {
        let idx = pos + offset;
        if is_escaped_delimiter(bytes, idx) {
            pos = idx + 1;
            continue;
        }

        match bytes[idx] {
            b'{' => depth += 1,
            b'}' => {
                let Some(new_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = new_depth;
            }
            _ => unreachable!(),
        }
        pos = idx + 1;
    }

    depth == 0
}

fn is_escaped_delimiter(input: &[u8], delimiter: usize) -> bool {
    if delimiter == 0 || input[delimiter - 1] != b'\\' {
        return false;
    }

    let mut slash_count = 0usize;
    let mut pos = delimiter;
    while pos > 0 && input[pos - 1] == b'\\' {
        slash_count += 1;
        pos -= 1;
    }
    slash_count % 2 == 1
}

fn escape_quoted_literal(text: &str) -> String {
    text.replace('"', "\\\"")
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

fn value_to_lossy_string(value: &Value<'_>) -> String {
    match value {
        Value::Literal(s) => normalize_text_projection(s),
        Value::Number(n) => n.to_string(),
        Value::Variable(v) => format!("{{{v}}}"),
        Value::Concat(parts) => parts.iter().map(value_to_lossy_string).collect(),
    }
}

fn value_to_plain_string(value: &Value<'_>) -> String {
    match value {
        Value::Literal(text) => normalize_text_projection(text),
        Value::Number(number) => number.to_string(),
        Value::Variable(name) => name.to_string(),
        Value::Concat(parts) => parts.iter().map(value_to_plain_string).collect(),
    }
}

pub(crate) fn normalize_text_projection(text: &str) -> String {
    if !text
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'\n' | b'\r'))
    {
        return text.to_string();
    }

    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                normalized.push('\n');
                while chars.peek().is_some_and(|next| matches!(next, ' ' | '\t')) {
                    chars.next();
                }
            }
            '\n' => {
                normalized.push('\n');
                while chars.peek().is_some_and(|next| matches!(next, ' ' | '\t')) {
                    chars.next();
                }
            }
            _ => normalized.push(ch),
        }
    }
    normalized
}

/// Normalize a DOI from common raw forms into lowercase `10.x/...` form.
#[must_use]
pub fn normalize_doi(input: &str) -> Option<String> {
    let mut doi = input.trim();
    if doi.is_empty() {
        return None;
    }

    for prefix in [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi:",
        "DOI:",
    ] {
        if let Some(stripped) = doi.strip_prefix(prefix) {
            doi = stripped.trim();
            break;
        }
    }

    let doi = doi.trim_end_matches(['.', ',', ';']);
    if doi.len() > 3 && doi.starts_with("10.") && doi.contains('/') {
        Some(doi.to_ascii_lowercase())
    } else {
        None
    }
}

fn resource_field_from_parts(
    field_name: &str,
    value: String,
    archive_prefix: Option<&str>,
) -> Option<ResourceField> {
    let mut kind = classify_resource_field(field_name)?;
    if kind == ResourceKind::Eprint
        && archive_prefix.is_some_and(|prefix| prefix.eq_ignore_ascii_case("arxiv"))
    {
        kind = ResourceKind::Arxiv;
    }
    let normalized = normalize_resource_value(kind, &value);
    Some(ResourceField {
        kind,
        field_name: field_name.to_string(),
        value,
        normalized,
    })
}

fn normalize_resource_value(kind: ResourceKind, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    match kind {
        ResourceKind::Doi => normalize_doi(trimmed),
        ResourceKind::Pmid => normalize_ascii_digits(trimmed),
        ResourceKind::Pmcid => Some(normalize_pmcid(trimmed)),
        ResourceKind::Isbn => normalize_isbn(trimmed),
        ResourceKind::Issn => normalize_issn(trimmed),
        ResourceKind::Arxiv => Some(normalize_arxiv(trimmed)),
        ResourceKind::File | ResourceKind::Url | ResourceKind::Eprint | ResourceKind::Crossref => {
            Some(trimmed.to_string())
        }
    }
}

fn normalize_ascii_digits(input: &str) -> Option<String> {
    let compact = input.trim();
    compact
        .chars()
        .all(|ch| ch.is_ascii_digit())
        .then(|| compact.to_string())
}

fn normalize_pmcid(input: &str) -> String {
    let compact = input
        .trim()
        .trim_start_matches("pmcid:")
        .trim_start_matches("PMCID:")
        .trim();
    if compact
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("pmc"))
    {
        compact.to_ascii_uppercase()
    } else {
        format!("PMC{compact}")
    }
}

fn normalize_isbn(input: &str) -> Option<String> {
    let compact = input
        .chars()
        .filter(|ch| !matches!(ch, '-' | ' '))
        .collect::<String>()
        .to_ascii_uppercase();
    is_valid_isbn_shape(&compact).then_some(compact)
}

fn normalize_issn(input: &str) -> Option<String> {
    let compact = input
        .chars()
        .filter(|ch| !matches!(ch, '-' | ' '))
        .collect::<String>()
        .to_ascii_uppercase();
    (compact.len() == 8
        && compact
            .chars()
            .enumerate()
            .all(|(index, ch)| ch.is_ascii_digit() || (index == 7 && ch == 'X')))
    .then_some(compact)
}

fn normalize_arxiv(input: &str) -> String {
    input
        .trim()
        .trim_start_matches("arXiv:")
        .trim_start_matches("arxiv:")
        .trim()
        .to_string()
}

fn trim_bibtex_scalar(input: &str) -> &str {
    let mut value = input.trim();
    loop {
        let trimmed = value.trim();
        if trimmed.len() >= 2
            && ((trimmed.starts_with('{') && trimmed.ends_with('}'))
                || (trimmed.starts_with('"') && trimmed.ends_with('"')))
        {
            value = trimmed[1..trimmed.len() - 1].trim();
        } else {
            return trimmed;
        }
    }
}

fn parse_year(input: &str) -> std::result::Result<i32, DateParseError> {
    let input = input.trim();
    if input.len() != 4 || !input.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(DateParseError::InvalidYear);
    }
    input
        .parse::<i32>()
        .map_err(|_| DateParseError::InvalidYear)
}

fn parse_month_number(input: &str) -> Option<u8> {
    let normalized = trim_bibtex_scalar(input).to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    if let Ok(month) = normalized.parse::<u8>() {
        return (1..=12).contains(&month).then_some(month);
    }

    match normalized.as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

fn parse_day_number(input: &str, year: i32, month: u8) -> std::result::Result<u8, DateParseError> {
    let input = input.trim();
    if input.is_empty() || input.len() > 2 || !input.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(DateParseError::InvalidDay);
    }
    let day = input
        .parse::<u8>()
        .map_err(|_| DateParseError::InvalidDay)?;
    (1..=days_in_month(year, month))
        .contains(&day)
        .then_some(day)
        .ok_or(DateParseError::InvalidDay)
}

const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn is_valid_isbn_shape(isbn: &str) -> bool {
    let compact: String = isbn.chars().filter(|c| !matches!(c, '-' | ' ')).collect();

    match compact.len() {
        10 => compact
            .chars()
            .enumerate()
            .all(|(index, ch)| ch.is_ascii_digit() || (index == 9 && matches!(ch, 'x' | 'X'))),
        13 => compact.chars().all(|ch| ch.is_ascii_digit()),
        _ => false,
    }
}

fn split_bibtex_names(input: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut iter = input.char_indices().peekable();

    while let Some((index, ch)) = iter.next() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            'a' | 'A' if depth == 0 && starts_name_separator(input, index) => {
                let candidate = input[start..index].trim();
                if !candidate.is_empty() {
                    names.push(candidate);
                }
                start = index + 3;
                while input[start..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
                {
                    start += input[start..].chars().next().map_or(0, char::len_utf8);
                }
                while iter
                    .peek()
                    .is_some_and(|(_, next_ch)| next_ch.is_whitespace())
                {
                    iter.next();
                }
            }
            _ => {}
        }
    }

    let candidate = input[start..].trim();
    if !candidate.is_empty() {
        names.push(candidate);
    }

    names
}

fn starts_name_separator(input: &str, index: usize) -> bool {
    let tail = &input[index..];
    let Some(rest) = tail.get(..3) else {
        return false;
    };
    if !rest.eq_ignore_ascii_case("and") {
        return false;
    }

    let before_is_boundary = input[..index]
        .chars()
        .next_back()
        .map_or(true, char::is_whitespace);
    let after_is_boundary = tail[3..].chars().next().map_or(true, char::is_whitespace);

    before_is_boundary && after_is_boundary
}

fn parse_single_name(input: &str) -> PersonName {
    let raw = input.trim();
    if let Some(literal) = braced_literal_name(raw) {
        return person_name(
            raw,
            String::new(),
            String::new(),
            literal.clone(),
            String::new(),
            Some(literal),
        );
    }

    let parts = split_top_level_commas(input);
    match parts.as_slice() {
        [last] => parse_first_von_last(last),
        [last, first] => {
            let (von, last) = split_von_last(last);
            person_name(
                raw,
                normalize_name_part(first),
                von,
                last,
                String::new(),
                None,
            )
        }
        [last, jr, first, ..] => {
            let (von, last) = split_von_last(last);
            person_name(
                raw,
                normalize_name_part(first),
                von,
                last,
                normalize_name_part(jr),
                None,
            )
        }
        [] => empty_person_name(raw),
    }
}

fn parse_first_von_last(input: &str) -> PersonName {
    let raw = input.trim();
    let words = split_name_words(input);
    match words.len() {
        0 => empty_person_name(raw),
        1 => person_name(
            raw,
            String::new(),
            String::new(),
            normalize_name_part(words[0]),
            String::new(),
            None,
        ),
        _ => {
            let von_start = words
                .iter()
                .position(|word| starts_with_lowercase_letter(word));
            let (first, von, last) = von_start.map_or_else(
                || {
                    (
                        join_name_words(&words[..words.len() - 1]),
                        String::new(),
                        normalize_name_part(words[words.len() - 1]),
                    )
                },
                |von_start| {
                    let last_start = words[von_start + 1..]
                        .iter()
                        .position(|word| !starts_with_lowercase_letter(word))
                        .map_or(words.len() - 1, |offset| von_start + 1 + offset);

                    (
                        join_name_words(&words[..von_start]),
                        join_name_words(&words[von_start..last_start]),
                        join_name_words(&words[last_start..]),
                    )
                },
            );

            person_name(raw, first, von, last, String::new(), None)
        }
    }
}

fn person_name(
    raw: &str,
    first: String,
    von: String,
    last: String,
    jr: String,
    literal: Option<String>,
) -> PersonName {
    let given = split_component_tokens(&first);
    let family = split_component_tokens(&last);
    let prefix = split_component_tokens(&von);
    let suffix = split_component_tokens(&jr);
    PersonName {
        raw: raw.to_string(),
        first,
        von,
        last,
        jr,
        given,
        family,
        prefix,
        suffix,
        literal,
    }
}

fn empty_person_name(raw: &str) -> PersonName {
    person_name(
        raw,
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        None,
    )
}

fn split_component_tokens(input: &str) -> Vec<String> {
    split_name_words(input)
        .into_iter()
        .map(normalize_name_part)
        .filter(|part| !part.is_empty())
        .collect()
}

fn split_von_last(input: &str) -> (String, String) {
    let words = split_name_words(input);
    if words.is_empty() {
        return (String::new(), String::new());
    }

    if let Some(last_start) = words
        .iter()
        .rposition(|word| starts_with_lowercase_letter(word))
    {
        if last_start + 1 < words.len() {
            return (
                join_name_words(&words[..=last_start]),
                join_name_words(&words[last_start + 1..]),
            );
        }
    }

    if words.len() == 1 {
        (String::new(), normalize_name_part(words[0]))
    } else {
        (
            join_name_words(&words[..words.len() - 1]),
            normalize_name_part(words[words.len() - 1]),
        )
    }
}

fn split_top_level_commas(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;

    for (index, ch) in input.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(input[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }

    parts.push(input[start..].trim());
    parts
}

fn split_name_words(input: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = None;
    let mut depth = 0usize;

    for (index, ch) in input.char_indices() {
        match ch {
            '{' => {
                depth += 1;
                start.get_or_insert(index);
            }
            '}' => {
                depth = depth.saturating_sub(1);
            }
            ch if ch.is_whitespace() && depth == 0 => {
                if let Some(word_start) = start.take() {
                    words.push(input[word_start..index].trim());
                }
            }
            _ => {
                start.get_or_insert(index);
            }
        }
    }

    if let Some(word_start) = start {
        words.push(input[word_start..].trim());
    }

    words.into_iter().filter(|word| !word.is_empty()).collect()
}

fn join_name_words(words: &[&str]) -> String {
    words
        .iter()
        .map(|word| normalize_name_part(word))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_name_part(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('{') && trimmed.ends_with('}') {
        trimmed[1..trimmed.len() - 1].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn braced_literal_name(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }

    let mut depth = 0usize;
    for (index, ch) in trimmed.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && index != trimmed.len() - 1 {
                    return None;
                }
            }
            _ => {}
        }
    }

    (depth == 0).then(|| normalize_name_part(trimmed))
}

fn starts_with_lowercase_letter(input: &str) -> bool {
    normalize_name_part(input)
        .chars()
        .find(|ch| ch.is_alphabetic())
        .is_some_and(char::is_lowercase)
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
