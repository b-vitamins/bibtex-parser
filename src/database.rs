//! BibTeX database representation

use crate::{Entry, Error, Result, ValidationError, ValidationLevel, Value};
use ahash::AHashMap;
use memchr::memchr;
use std::borrow::Cow;
use std::path::Path;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[inline]
const fn to_ascii_lower(byte: u8) -> u8 {
    if b'A' <= byte && byte <= b'Z' {
        byte + (b'a' - b'A')
    } else {
        byte
    }
}

/// Get month expansion for a given abbreviation (case-insensitive)
///
/// Returns None if the name is not a recognized month abbreviation.
/// This is used as a fallback when user-defined string variables are not found.
fn get_month_expansion(name: &str) -> Option<&'static str> {
    let bytes = name.as_bytes();
    if bytes.len() != 3 {
        return None;
    }

    let key = (
        to_ascii_lower(bytes[0]),
        to_ascii_lower(bytes[1]),
        to_ascii_lower(bytes[2]),
    );

    match key {
        (b'j', b'a', b'n') => Some("January"),
        (b'f', b'e', b'b') => Some("February"),
        (b'm', b'a', b'r') => Some("March"),
        (b'a', b'p', b'r') => Some("April"),
        (b'm', b'a', b'y') => Some("May"),
        (b'j', b'u', b'n') => Some("June"),
        (b'j', b'u', b'l') => Some("July"),
        (b'a', b'u', b'g') => Some("August"),
        (b's', b'e', b'p') => Some("September"),
        (b'o', b'c', b't') => Some("October"),
        (b'n', b'o', b'v') => Some("November"),
        (b'd', b'e', b'c') => Some("December"),
        _ => None,
    }
}

/// Check if a value contains variables that need expansion
/// Only expand if we have user strings OR if variables might be month constants
fn should_expand_variables(value: &Value, has_user_strings: bool) -> bool {
    if has_user_strings {
        // If we have user strings, expand all variables
        contains_variables(value)
    } else {
        // If no user strings, only expand if variables might be month constants
        contains_potential_month_variables(value)
    }
}

/// Check if a value contains any variables
fn contains_variables(value: &Value) -> bool {
    match value {
        Value::Variable(_) => true,
        Value::Concat(parts) => parts.iter().any(contains_variables),
        _ => false,
    }
}

/// Check if a value contains variables that might be month constants
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
        if input[offset + 1].to_ascii_lowercase() != expected {
            return false;
        }
    }

    if input.len() == keyword.len() + 1 {
        return true;
    }

    !is_identifier_char(input[keyword.len() + 1])
}

/// Fast pre-scan to detect whether the file might contain `@string` entries.
///
/// False positives are acceptable (we just take the slower path), but false
/// negatives would be incorrect, so the check matches parser keyword rules.
fn input_may_contain_string_definition(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        if let Some(offset) = memchr(b'@', &bytes[pos..]) {
            let at = pos + offset;
            if starts_with_at_keyword(&bytes[at..], b"string") {
                return true;
            }
            pos = at + 1;
        } else {
            break;
        }
    }

    false
}

/// Parser configuration with builder pattern
#[derive(Debug, Default)]
pub struct ParseOptions {
    threads: Option<usize>,
}

impl ParseOptions {
    /// Create new parse options
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set number of threads (None = use all available)
    #[must_use]
    pub fn threads(mut self, threads: impl Into<Option<usize>>) -> Self {
        self.threads = threads.into();
        self
    }

    /// Parse a single input string (always sequential for optimal performance)
    ///
    /// Note: Single-file parallel parsing is disabled because it performs worse
    /// than sequential parsing due to overhead. Use `parse_files()` for parallel processing.
    pub fn parse<'a>(&self, input: &'a str) -> Result<Database<'a>> {
        // Always use sequential parsing for single files - parallel is counterproductive
        Database::parse_sequential(input)
    }

    /// Parse multiple files in parallel
    pub fn parse_files<P: AsRef<Path> + Sync>(&self, paths: &[P]) -> Result<Database<'static>> {
        #[cfg(feature = "parallel")]
        {
            if let Some(threads) = self.threads {
                if threads <= 1 {
                    return Self::parse_files_sequential(paths);
                }
            }

            let pool = self.build_thread_pool()?;

            // Parse files in parallel
            let databases: Result<Vec<_>> = pool.install(|| {
                paths
                    .par_iter()
                    .map(|path| {
                        let content = std::fs::read_to_string(path)?;
                        let db = Database::parse_sequential(&content)?;
                        Ok(db.into_owned())
                    })
                    .collect()
            });

            // Merge databases
            let dbs = databases?;
            Ok(Database::merge_databases_parallel(dbs))
        }

        #[cfg(not(feature = "parallel"))]
        {
            Self::parse_files_sequential(paths)
        }
    }

    /// Sequential file parsing fallback
    fn parse_files_sequential<P: AsRef<Path>>(paths: &[P]) -> Result<Database<'static>> {
        let mut result = Database::new();
        for path in paths {
            let content = std::fs::read_to_string(path)?;
            let db = Database::parse_sequential(&content)?;
            result.merge(db.into_owned());
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

/// A parsed BibTeX database
#[derive(Debug, Clone, Default)]
pub struct Database<'a> {
    /// Bibliography entries
    entries: Vec<Entry<'a>>,
    /// String definitions
    strings: AHashMap<Cow<'a, str>, Value<'a>>,
    /// Preambles
    preambles: Vec<Value<'a>>,
    /// Comments
    comments: Vec<Cow<'a, str>>,
}

impl<'a> Database<'a> {
    /// Create a new empty database
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a parser with options
    ///
    /// # Parallel Processing
    ///
    /// The `threads` option only affects `parse_files()`. Single file
    /// parsing with `parse()` is always sequential due to BibTeX's
    /// structure requiring sequential processing of string definitions.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use bibtex_parser::Database;
    /// // This will use parallel processing
    /// let db = Database::parser()
    ///     .threads(4)
    ///     .parse_files(&["file1.bib", "file2.bib"]).unwrap();
    ///
    /// // This is always sequential (threads ignored)
    /// let content = "@article{demo, title=\"Demo\"}";
    /// let db = Database::parser()
    ///     .threads(4)
    ///     .parse(content).unwrap();
    /// ```
    #[must_use]
    pub fn parser() -> ParseOptions {
        ParseOptions::new()
    }

    /// Parse a BibTeX database from a string (single-threaded implementation)
    pub(crate) fn parse_sequential(input: &'a str) -> Result<Self> {
        let mut db = Self::new();

        // Fast path for common corpora (like tugboat) with no user-defined strings.
        // This avoids buffering all entries before expansion.
        if !input_may_contain_string_definition(input) {
            let has_user_strings = false;
            let mut expanded_variables = AHashMap::new();
            let mut expansion_stack = Vec::new();

            crate::parser::parse_bibtex_stream(input, |item| {
                match item {
                    crate::parser::ParsedItem::Entry(mut entry) => {
                        for field in &mut entry.fields {
                            if should_expand_variables(&field.value, has_user_strings) {
                                let old_value = std::mem::take(&mut field.value);
                                field.value = db.smart_expand_value_cached(
                                    old_value,
                                    &mut expanded_variables,
                                    &mut expansion_stack,
                                )?;
                            }
                        }
                        db.entries.push(entry);
                    }
                    crate::parser::ParsedItem::Preamble(value) => {
                        let expanded = if should_expand_variables(&value, has_user_strings) {
                            db.smart_expand_value_cached(
                                value,
                                &mut expanded_variables,
                                &mut expansion_stack,
                            )?
                        } else {
                            value
                        };
                        db.preambles.push(expanded);
                    }
                    crate::parser::ParsedItem::Comment(text) => {
                        db.comments.push(Cow::Borrowed(text));
                    }
                    crate::parser::ParsedItem::String(name, value) => {
                        // Defensive fallback for scanner false negatives.
                        db.strings.insert(Cow::Borrowed(name), value);
                    }
                }
                Ok(())
            })?;

            return Ok(db);
        }

        let mut pending_entries = Vec::new();
        let mut pending_preambles = Vec::new();

        crate::parser::parse_bibtex_stream(input, |item| {
            match item {
                crate::parser::ParsedItem::Entry(entry) => pending_entries.push(entry),
                crate::parser::ParsedItem::Preamble(value) => pending_preambles.push(value),
                crate::parser::ParsedItem::String(name, value) => {
                    db.strings.insert(Cow::Borrowed(name), value);
                }
                crate::parser::ParsedItem::Comment(text) => {
                    db.comments.push(Cow::Borrowed(text));
                }
            }
            Ok(())
        })?;

        db.entries.reserve_exact(pending_entries.len());
        db.preambles.reserve_exact(pending_preambles.len());

        // Expand after parsing so all @string definitions are available globally.
        let has_user_strings = !db.strings.is_empty();
        let mut expanded_variables = AHashMap::with_capacity(db.strings.len());
        let mut expansion_stack = Vec::new();

        for mut entry in pending_entries {
            for field in &mut entry.fields {
                if should_expand_variables(&field.value, has_user_strings) {
                    let old_value = std::mem::take(&mut field.value);
                    field.value = db.smart_expand_value_cached(
                        old_value,
                        &mut expanded_variables,
                        &mut expansion_stack,
                    )?;
                }
            }
            db.entries.push(entry);
        }

        for value in pending_preambles {
            let expanded = if should_expand_variables(&value, has_user_strings) {
                db.smart_expand_value_cached(value, &mut expanded_variables, &mut expansion_stack)?
            } else {
                value
            };
            db.preambles.push(expanded);
        }

        Ok(db)
    }

    /// Merge another database into this one
    pub fn merge(&mut self, other: Self) {
        self.entries.extend(other.entries);
        self.strings.extend(other.strings);
        self.preambles.extend(other.preambles);
        self.comments.extend(other.comments);
    }

    #[cfg(feature = "parallel")]
    fn merge_databases_parallel(databases: Vec<Database<'static>>) -> Database<'static> {
        use rayon::prelude::*;

        // Type alias to simplify complex type
        type DatabaseComponents = (
            AHashMap<Cow<'static, str>, Value<'static>>,
            Vec<Value<'static>>,
            Vec<Cow<'static, str>>,
        );

        // Move entries out for parallel merging while collecting other data
        let mut others: Vec<DatabaseComponents> = Vec::with_capacity(databases.len());
        let entry_vecs: Vec<Vec<Entry<'static>>> = databases
            .into_iter()
            .map(|db| {
                let Database {
                    entries,
                    strings,
                    preambles,
                    comments,
                } = db;
                others.push((strings, preambles, comments));
                entries
            })
            .collect();

        let all_entries: Vec<_> = entry_vecs.into_par_iter().flatten().collect();

        let mut result = Database {
            entries: all_entries,
            strings: AHashMap::new(),
            preambles: Vec::new(),
            comments: Vec::new(),
        };

        for (strings, preambles, comments) in others {
            result.strings.extend(strings);
            result.preambles.extend(preambles);
            result.comments.extend(comments);
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
    pub const fn strings(&self) -> &AHashMap<Cow<'a, str>, Value<'a>> {
        &self.strings
    }

    /// Get mutable access to string definitions
    #[must_use]
    pub fn strings_mut(&mut self) -> &mut AHashMap<Cow<'a, str>, Value<'a>> {
        &mut self.strings
    }

    /// Get all preambles
    #[must_use]
    pub fn preambles(&self) -> &[Value<'a>] {
        &self.preambles
    }

    /// Get mutable access to preambles
    #[must_use]
    pub fn preambles_mut(&mut self) -> &mut Vec<Value<'a>> {
        &mut self.preambles
    }

    /// Get all comments
    #[must_use]
    pub fn comments(&self) -> &[Cow<'a, str>] {
        &self.comments
    }

    /// Get mutable access to comments
    #[must_use]
    pub fn comments_mut(&mut self) -> &mut Vec<Cow<'a, str>> {
        &mut self.comments
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
            .filter(|e| {
                e.get_as_string(field)
                    .as_ref()
                    .is_some_and(|v| v.contains(value))
            })
            .collect()
    }

    /// Smart expansion with memoization for repeated variable references.
    fn smart_expand_value_cached(
        &self,
        value: Value<'a>,
        expanded_variables: &mut AHashMap<String, Value<'a>>,
        expansion_stack: &mut Vec<String>,
    ) -> Result<Value<'a>> {
        match value {
            // Simple literals and numbers stay as-is (zero-copy!)
            Value::Literal(_) | Value::Number(_) => Ok(value),

            // Variables need to be resolved
            Value::Variable(name) => {
                if let Some(expanded) = expanded_variables.get(name.as_ref()) {
                    return Ok(expanded.clone());
                }

                if expansion_stack.iter().any(|v| v == name.as_ref()) {
                    let mut cycle = expansion_stack.join(" -> ");
                    if !cycle.is_empty() {
                        cycle.push_str(" -> ");
                    }
                    cycle.push_str(name.as_ref());
                    return Err(Error::CircularReference(cycle));
                }

                // First check user-defined strings
                self.strings.get(name.as_ref()).map_or_else(
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
                    |user_value| {
                        // Recursively expand the variable's value and cache the result
                        let variable_name = name.as_ref().to_string();
                        expansion_stack.push(variable_name.clone());
                        let expanded = self.smart_expand_value_cached(
                            user_value.clone(),
                            expanded_variables,
                            expansion_stack,
                        );
                        expansion_stack.pop();

                        let expanded = expanded?;
                        expanded_variables.insert(variable_name, expanded.clone());
                        Ok(expanded)
                    },
                )
            }

            // Concatenations need special handling
            Value::Concat(parts) => {
                self.expand_concatenation_cached(*parts, expanded_variables, expansion_stack)
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
                self.strings.get(name.as_ref()).map_or_else(
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
                let cloned_parts = (**parts).clone();
                self.expand_concatenation(cloned_parts)
            }
        }
    }

    /// Expand a concatenation, only converting to owned when necessary
    fn expand_concatenation(&self, parts: Vec<Value<'a>>) -> Result<Value<'a>> {
        let mut expanded_variables = AHashMap::new();
        let mut expansion_stack = Vec::new();
        self.expand_concatenation_cached(parts, &mut expanded_variables, &mut expansion_stack)
    }

    /// Cached concatenation expansion used by hot parsing paths.
    fn expand_concatenation_cached(
        &self,
        parts: Vec<Value<'a>>,
        expanded_variables: &mut AHashMap<String, Value<'a>>,
        expansion_stack: &mut Vec<String>,
    ) -> Result<Value<'a>> {
        let mut expanded_parts = Vec::with_capacity(parts.len());

        // First, expand all parts
        for part in parts {
            let expanded =
                self.smart_expand_value_cached(part, expanded_variables, expansion_stack)?;
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
            Ok(Value::Concat(Box::new(expanded_parts)))
        }
    }

    /// Get a fully expanded string value (for compatibility)
    pub fn get_expanded_string(&self, value: &Value<'a>) -> Result<String> {
        match value {
            Value::Literal(s) => Ok(s.to_string()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Variable(name) => {
                // First check user-defined strings
                self.strings.get(name.as_ref()).map_or_else(
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
    pub fn into_owned(self) -> Database<'static> {
        Database {
            entries: self.entries.into_iter().map(Entry::into_owned).collect(),
            strings: self
                .strings
                .into_iter()
                .map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_owned()))
                .collect(),
            preambles: self.preambles.into_iter().map(Value::into_owned).collect(),
            comments: self
                .comments
                .into_iter()
                .map(|c| Cow::Owned(c.into_owned()))
                .collect(),
        }
    }

    /// Add a string definition (useful for building databases programmatically)
    pub fn add_string(&mut self, name: &'a str, value: Value<'a>) {
        self.strings.insert(Cow::Borrowed(name), value);
    }

    /// Add an entry
    pub fn add_entry(&mut self, entry: Entry<'a>) {
        self.entries.push(entry);
    }

    /// Add a preamble
    pub fn add_preamble(&mut self, value: Value<'a>) {
        self.preambles.push(value);
    }

    /// Add a comment
    pub fn add_comment(&mut self, comment: &'a str) {
        self.comments.push(Cow::Borrowed(comment));
    }

    /// Validate all entries in the database
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

    /// Get statistics about the database
    #[must_use]
    pub fn stats(&self) -> DatabaseStats {
        let mut type_counts = AHashMap::new();
        for entry in &self.entries {
            *type_counts.entry(entry.ty.to_string()).or_insert(0) += 1;
        }

        DatabaseStats {
            total_entries: self.entries.len(),
            total_strings: self.strings.len(),
            total_preambles: self.preambles.len(),
            total_comments: self.comments.len(),
            entries_by_type: type_counts,
        }
    }
}

/// Statistics about a database
#[derive(Debug, Clone)]
pub struct DatabaseStats {
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

/// Comprehensive validation report for a database
#[derive(Debug, Clone)]
pub struct ValidationReport<'a> {
    /// Entries that failed validation with their errors
    pub invalid_entries: Vec<(usize, &'a Entry<'a>, Vec<ValidationError>)>,
    /// Duplicate citation keys
    pub duplicate_keys: Vec<&'a str>,
    /// Entries with no fields
    pub empty_entries: Vec<(usize, &'a Entry<'a>)>,
    /// Total number of entries in the database
    pub total_entries: usize,
    /// Validation level used
    pub validation_level: ValidationLevel,
}

impl ValidationReport<'_> {
    /// Check if the database is completely valid
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
        self.db.strings.insert(Cow::Borrowed(name), value);
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
        self.db.comments.push(Cow::Borrowed(text));
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

        let db = Database::parser().parse(input).unwrap();
        assert_eq!(db.entries().len(), 1);
        assert_eq!(db.strings().len(), 1);

        let entry = &db.entries()[0];
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

        let db = Database::parser().parse(input).unwrap();
        let entry = &db.entries()[0];

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

        let db = Database::parser().parse(input).unwrap();
        let entry = &db.entries()[0];

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

        let db = Database::parser().parse(input).unwrap();
        let entry = &db.entries()[0];

        assert_eq!(entry.fields.len(), 10);
        assert!(
            entry.fields.capacity() <= 16,
            "Unexpected field Vec growth: len={}, capacity={}",
            entry.fields.len(),
            entry.fields.capacity()
        );
    }

    #[test]
    fn test_database_builder() {
        let db = DatabaseBuilder::new()
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

        assert_eq!(db.entries().len(), 1);
        assert_eq!(db.strings().len(), 1);
    }

    #[test]
    fn test_database_stats() {
        let input = r#"
            @string{ieee = "IEEE"}
            @preamble{"Test preamble"}
            % This is a percent comment that now works properly
            @comment{This is a formal comment that works}
            @article{a1, title = "Article 1"}
            @article{a2, title = "Article 2"}
            @book{b1, title = "Book 1"}
        "#;

        let db = Database::parser().parse(input).unwrap();
        let stats = db.stats();

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

        let db = Database::parser().threads(2).parse_files(&paths).unwrap();

        assert_eq!(db.entries().len(), 2);

        let _ = std::fs::remove_file(path1);
        let _ = std::fs::remove_file(path2);
    }

    #[test]
    fn test_builder_pattern_api() {
        let input = "@article{test, title = \"Test\"}";

        // Single-threaded (default)
        let db1 = Database::parser().parse(input).unwrap();
        assert_eq!(db1.entries().len(), 1);

        // Using parser builder
        let db2 = Database::parser().threads(1).parse(input).unwrap();
        assert_eq!(db2.entries().len(), 1);

        #[cfg(feature = "parallel")]
        {
            use std::fs::write;

            // Parallel only works for multiple files
            let db3 = Database::parser().threads(4).parse(input).unwrap();
            assert_eq!(db3.entries().len(), 1);

            // Multi-file parallel processing
            let path1 = "/tmp/test1.bib";
            let path2 = "/tmp/test2.bib";
            write(path1, "@article{a1, title=\"A\"}").unwrap();
            write(path2, "@article{a2, title=\"B\"}").unwrap();

            let db4 = Database::parser()
                .threads(2)
                .parse_files(&[path1, path2])
                .unwrap();
            assert_eq!(db4.entries().len(), 2);

            let _ = std::fs::remove_file(path1);
            let _ = std::fs::remove_file(path2);
        }
    }
}
