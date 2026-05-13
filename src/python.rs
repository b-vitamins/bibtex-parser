#![allow(
    missing_docs,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::unnecessary_wraps,
    clippy::missing_const_for_fn,
    clippy::fn_params_excessive_bools,
    clippy::struct_excessive_bools,
    clippy::option_if_let_else,
    clippy::redundant_pub_crate,
    clippy::use_self
)]

use crate::{
    document_to_string, normalize_doi, parse_date_parts, parse_names, selected_entries_to_string,
    DateParseError, DateParts, Diagnostic, DiagnosticSeverity, DiagnosticTarget, EntryType,
    ParsedBlock, ParsedComment, ParsedDocument, ParsedEntry, ParsedEntryStatus, ParsedFailedBlock,
    ParsedField, ParsedPreamble, ParsedString, Parser, RawWriteMode, ResourceField, SourceSpan,
    TrailingComma, ValidationLevel, ValidationSeverity, Value, Writer, WriterConfig,
};
use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule, PyType};
use std::borrow::Cow;

pyo3::create_exception!(_native, BibtexParserError, pyo3::exceptions::PyException);

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyParser>()?;
    m.add_class::<PyDocument>()?;
    m.add_class::<PyEntry>()?;
    m.add_class::<PyField>()?;
    m.add_class::<PyValue>()?;
    m.add_class::<PyDiagnostic>()?;
    m.add_class::<PySourceSpan>()?;
    m.add_class::<PyComment>()?;
    m.add_class::<PyPreamble>()?;
    m.add_class::<PyStringDefinition>()?;
    m.add_class::<PyFailedBlock>()?;
    m.add_class::<PyBlock>()?;
    m.add_class::<PyWriterConfig>()?;
    m.add_class::<PyValidationIssue>()?;
    m.add_class::<PyPersonName>()?;
    m.add_class::<PyDateParts>()?;
    m.add_class::<PyResourceField>()?;
    m.add("BibtexParserError", m.py().get_type::<BibtexParserError>())?;
    m.add_function(wrap_pyfunction!(parse_text, m)?)?;
    m.add_function(wrap_pyfunction!(parse_file, m)?)?;
    m.add_function(wrap_pyfunction!(write, m)?)?;
    m.add_function(wrap_pyfunction!(normalize_doi_py, m)?)?;
    m.add_function(wrap_pyfunction!(parse_names_py, m)?)?;
    m.add_function(wrap_pyfunction!(parse_date_py, m)?)?;
    m.add_function(wrap_pyfunction!(latex_to_unicode_py, m)?)?;
    Ok(())
}

#[pyclass(name = "Parser")]
#[derive(Debug, Clone)]
struct PyParser {
    tolerant: bool,
    capture_source: bool,
    preserve_raw: bool,
    expand_values: bool,
    latex_to_unicode: bool,
}

#[pymethods]
impl PyParser {
    #[new]
    #[pyo3(signature = (tolerant=false, capture_source=true, preserve_raw=true, expand_values=false, latex_to_unicode=false))]
    fn new(
        tolerant: bool,
        capture_source: bool,
        preserve_raw: bool,
        expand_values: bool,
        latex_to_unicode: bool,
    ) -> Self {
        Self {
            tolerant,
            capture_source,
            preserve_raw,
            expand_values,
            latex_to_unicode,
        }
    }

    #[getter]
    const fn tolerant(&self) -> bool {
        self.tolerant
    }

    #[setter]
    fn set_tolerant(&mut self, tolerant: bool) {
        self.tolerant = tolerant;
    }

    #[getter]
    const fn capture_source(&self) -> bool {
        self.capture_source
    }

    #[setter]
    fn set_capture_source(&mut self, capture_source: bool) {
        self.capture_source = capture_source;
    }

    #[getter]
    const fn preserve_raw(&self) -> bool {
        self.preserve_raw
    }

    #[setter]
    fn set_preserve_raw(&mut self, preserve_raw: bool) {
        self.preserve_raw = preserve_raw;
    }

    #[getter]
    const fn expand_values(&self) -> bool {
        self.expand_values
    }

    #[setter]
    fn set_expand_values(&mut self, expand_values: bool) {
        self.expand_values = expand_values;
    }

    #[getter]
    const fn latex_to_unicode(&self) -> bool {
        self.latex_to_unicode
    }

    #[setter]
    fn set_latex_to_unicode(&mut self, latex_to_unicode: bool) {
        self.latex_to_unicode = latex_to_unicode;
    }

    #[pyo3(signature = (text, source=None))]
    fn parse(&self, text: &str, source: Option<String>) -> PyResult<PyDocument> {
        parse_document_with_options(self, text, source)
    }

    #[pyo3(signature = (path))]
    fn parse_file(&self, path: &str) -> PyResult<PyDocument> {
        let text = std::fs::read_to_string(path).map_err(map_error)?;
        self.parse(&text, Some(path.to_string()))
    }

    fn __repr__(&self) -> String {
        format!(
            "Parser(tolerant={}, capture_source={}, preserve_raw={}, expand_values={}, latex_to_unicode={})",
            self.tolerant,
            self.capture_source,
            self.preserve_raw,
            self.expand_values,
            self.latex_to_unicode
        )
    }
}

#[pyclass(name = "Document")]
#[derive(Debug, Clone)]
struct PyDocument {
    inner: ParsedDocument<'static>,
}

#[pymethods]
impl PyDocument {
    #[getter]
    fn status(&self) -> &'static str {
        parse_status_name(self.inner.status())
    }

    #[getter]
    fn entries(&self) -> Vec<PyEntry> {
        self.inner
            .entries()
            .iter()
            .cloned()
            .map(|inner| PyEntry {
                inner: inner.into_owned(),
            })
            .collect()
    }

    #[getter]
    fn comments(&self) -> Vec<PyComment> {
        self.inner
            .comments()
            .iter()
            .cloned()
            .map(PyComment::from)
            .collect()
    }

    #[getter]
    fn preambles(&self) -> Vec<PyPreamble> {
        self.inner
            .preambles()
            .iter()
            .cloned()
            .map(PyPreamble::from)
            .collect()
    }

    #[getter]
    fn strings(&self) -> Vec<PyStringDefinition> {
        self.inner
            .strings()
            .iter()
            .cloned()
            .map(PyStringDefinition::from)
            .collect()
    }

    #[getter]
    fn diagnostics(&self) -> Vec<PyDiagnostic> {
        self.inner
            .diagnostics()
            .iter()
            .cloned()
            .map(PyDiagnostic::from)
            .collect()
    }

    #[getter]
    fn failed_blocks(&self) -> Vec<PyFailedBlock> {
        self.inner
            .failed_blocks()
            .iter()
            .cloned()
            .map(PyFailedBlock::from)
            .collect()
    }

    #[getter]
    fn blocks(&self) -> Vec<PyBlock> {
        self.inner
            .blocks()
            .iter()
            .copied()
            .map(PyBlock::from)
            .collect()
    }

    fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let summary = self.inner.summary();
        let dict = PyDict::new(py);
        dict.set_item("status", parse_status_name(summary.status))?;
        dict.set_item("entries", summary.entries)?;
        dict.set_item("warnings", summary.warnings)?;
        dict.set_item("errors", summary.errors)?;
        dict.set_item("infos", summary.infos)?;
        dict.set_item("failed_blocks", summary.failed_blocks)?;
        dict.set_item("recovered_blocks", summary.recovered_blocks)?;
        Ok(dict)
    }

    fn stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("entries", self.inner.entries().len())?;
        dict.set_item("comments", self.inner.comments().len())?;
        dict.set_item("preambles", self.inner.preambles().len())?;
        dict.set_item("strings", self.inner.strings().len())?;
        dict.set_item("failed_blocks", self.inner.failed_blocks().len())?;
        dict.set_item("diagnostics", self.inner.diagnostics().len())?;
        Ok(dict)
    }

    fn entry(&self, key: &str) -> Option<PyEntry> {
        self.inner
            .entries()
            .iter()
            .find(|entry| entry.key() == key)
            .cloned()
            .map(|inner| PyEntry {
                inner: inner.into_owned(),
            })
    }

    fn keys(&self) -> Vec<String> {
        self.inner
            .entries()
            .iter()
            .map(|entry| entry.key().to_string())
            .collect()
    }

    fn rename_key(&mut self, old: &str, new: String) -> bool {
        self.inner.rename_key(old, Cow::Owned(new))
    }

    fn set_entry_type(&mut self, key: &str, entry_type: &str) -> bool {
        let Some(entry) = self.inner.entry_mut_by_key(key) else {
            return false;
        };
        entry.set_entry_type(EntryType::parse(entry_type).into_owned());
        true
    }

    fn set_field(&mut self, key: &str, name: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        let parsed_value = value_from_py(value)?;
        let Some(entry) = self.inner.entry_mut_by_key(key) else {
            return Ok(false);
        };
        if !entry.replace_field_value(name, parsed_value.clone()) {
            entry.add_field(Cow::Owned(name.to_string()), parsed_value);
        }
        Ok(true)
    }

    fn add_field(&mut self, key: &str, name: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        let parsed_value = value_from_py(value)?;
        let Some(entry) = self.inner.entry_mut_by_key(key) else {
            return Ok(false);
        };
        entry.add_field(Cow::Owned(name.to_string()), parsed_value);
        Ok(true)
    }

    fn rename_field(&mut self, key: &str, old: &str, new: String) -> usize {
        self.inner
            .entry_mut_by_key(key)
            .map_or(0, |entry| entry.rename_field(old, Cow::Owned(new)))
    }

    fn remove_field(&mut self, key: &str, name: &str) -> usize {
        self.inner
            .entry_mut_by_key(key)
            .map_or(0, |entry| entry.remove_field(name))
    }

    fn remove_export_fields(&mut self, names: Vec<String>) -> usize {
        let borrowed_names = names.iter().map(String::as_str).collect::<Vec<_>>();
        self.inner.remove_export_fields(&borrowed_names)
    }

    #[pyo3(signature = (config=None))]
    fn write(&self, config: Option<&PyWriterConfig>) -> PyResult<String> {
        write_document(&self.inner, config.map(PyWriterConfig::to_rust))
    }

    fn write_selected(&self, keys: Vec<String>) -> PyResult<String> {
        let borrowed = keys.iter().map(String::as_str).collect::<Vec<_>>();
        selected_entries_to_string(&self.inner, &borrowed).map_err(map_error)
    }

    #[pyo3(signature = (level="standard"))]
    fn validate(&self, level: &str) -> PyResult<Vec<PyValidationIssue>> {
        let level = validation_level(level)?;
        let mut issues = Vec::new();
        for (index, entry) in self.inner.entries().iter().enumerate() {
            let structured = entry.clone().into_entry();
            if let Err(errors) = structured.validate(level) {
                issues.extend(errors.into_iter().map(|error| PyValidationIssue {
                    entry_index: index,
                    key: entry.key().to_string(),
                    field: error.field,
                    severity: validation_severity_name(error.severity).to_string(),
                    message: error.message,
                }));
            }
        }

        let mut seen = std::collections::HashSet::new();
        let mut duplicate_keys = std::collections::HashSet::new();
        for entry in self.inner.entries() {
            let key = entry.key().to_string();
            if !seen.insert(key.clone()) {
                duplicate_keys.insert(key);
            }
        }
        for key in duplicate_keys {
            issues.push(PyValidationIssue {
                entry_index: 0,
                key: key.clone(),
                field: None,
                severity: "error".to_string(),
                message: format!("Duplicate entry key '{key}'"),
            });
        }

        Ok(issues)
    }

    fn latex_to_unicode(&mut self) -> PyResult<()> {
        apply_latex_to_unicode(&mut self.inner)
    }

    fn __len__(&self) -> usize {
        self.inner.entries().len()
    }

    fn __repr__(&self) -> String {
        let summary = self.inner.summary();
        format!(
            "Document(status={:?}, entries={}, diagnostics={})",
            parse_status_name(summary.status),
            summary.entries,
            summary.errors + summary.warnings + summary.infos
        )
    }
}

#[pyclass(name = "Entry")]
#[derive(Debug, Clone)]
struct PyEntry {
    inner: ParsedEntry<'static>,
}

#[pymethods]
impl PyEntry {
    #[getter]
    fn key(&self) -> String {
        self.inner.key().to_string()
    }

    #[getter]
    fn entry_type(&self) -> String {
        self.inner.ty.to_string()
    }

    #[getter]
    fn status(&self) -> &'static str {
        match self.inner.status {
            ParsedEntryStatus::Complete => "complete",
            ParsedEntryStatus::Partial => "partial",
        }
    }

    #[getter]
    fn fields(&self) -> Vec<PyField> {
        self.inner
            .fields
            .iter()
            .cloned()
            .map(PyField::from)
            .collect()
    }

    #[getter]
    fn raw(&self) -> Option<String> {
        self.inner.raw.as_deref().map(ToOwned::to_owned)
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }

    fn get(&self, name: &str) -> Option<String> {
        self.inner.field_ignore_case(name).map(field_text)
    }

    fn field(&self, name: &str) -> Option<PyField> {
        self.inner
            .field_ignore_case(name)
            .cloned()
            .map(PyField::from)
    }

    fn authors(&self) -> Vec<PyPersonName> {
        self.inner
            .authors()
            .into_iter()
            .map(PyPersonName::from)
            .collect()
    }

    fn editors(&self) -> Vec<PyPersonName> {
        self.inner
            .editors()
            .into_iter()
            .map(PyPersonName::from)
            .collect()
    }

    fn translators(&self) -> Vec<PyPersonName> {
        self.inner
            .translators()
            .into_iter()
            .map(PyPersonName::from)
            .collect()
    }

    fn date_parts(&self) -> PyResult<Option<PyDateParts>> {
        Ok(self
            .inner
            .date_parts()
            .transpose()
            .map_err(date_error)?
            .map(PyDateParts::from))
    }

    fn doi(&self) -> Option<String> {
        self.inner.doi()
    }

    fn resource_fields(&self) -> Vec<PyResourceField> {
        self.inner
            .resource_fields()
            .into_iter()
            .map(PyResourceField::from)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "Entry(key={:?}, entry_type={:?}, fields={})",
            self.inner.key(),
            self.inner.ty,
            self.inner.fields.len()
        )
    }
}

#[pyclass(name = "Field")]
#[derive(Debug, Clone)]
struct PyField {
    inner: ParsedField<'static>,
}

#[pymethods]
impl PyField {
    #[getter]
    fn name(&self) -> String {
        self.inner.name.to_string()
    }

    #[getter]
    fn value(&self) -> PyValue {
        PyValue {
            inner: self.inner.value.value.clone().into_owned(),
        }
    }

    #[getter]
    fn raw(&self) -> Option<String> {
        self.inner.raw.as_deref().map(ToOwned::to_owned)
    }

    #[getter]
    fn raw_value(&self) -> Option<String> {
        self.inner.value.raw.as_deref().map(ToOwned::to_owned)
    }

    #[getter]
    fn expanded(&self) -> Option<String> {
        self.inner.value.expanded.as_deref().map(ToOwned::to_owned)
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }

    #[getter]
    fn value_source(&self) -> Option<PySourceSpan> {
        self.inner.value_source.map(PySourceSpan::from)
    }

    fn plain_text(&self) -> String {
        self.inner.value.plain_text()
    }

    fn lossy_text(&self) -> String {
        self.inner.value.lossy_text()
    }

    fn unicode_text(&self) -> PyResult<String> {
        unicode_text(&self.inner.value.value)
    }
}

impl From<ParsedField<'static>> for PyField {
    fn from(inner: ParsedField<'static>) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "Value")]
#[derive(Debug, Clone)]
struct PyValue {
    inner: Value<'static>,
}

#[pymethods]
impl PyValue {
    #[classmethod]
    fn literal(_cls: &Bound<'_, PyType>, text: String) -> Self {
        Self {
            inner: Value::Literal(Cow::Owned(text)),
        }
    }

    #[classmethod]
    fn number(_cls: &Bound<'_, PyType>, number: i64) -> Self {
        Self {
            inner: Value::Number(number),
        }
    }

    #[classmethod]
    fn variable(_cls: &Bound<'_, PyType>, name: String) -> Self {
        Self {
            inner: Value::Variable(Cow::Owned(name)),
        }
    }

    #[classmethod]
    fn concat(_cls: &Bound<'_, PyType>, parts: Vec<PyValue>) -> Self {
        Self {
            inner: Value::Concat(
                parts
                    .into_iter()
                    .map(|part| part.inner)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
        }
    }

    #[classmethod]
    fn from_plain_string(_cls: &Bound<'_, PyType>, text: String) -> Self {
        Self {
            inner: Value::from_plain_string(Cow::Owned(text)),
        }
    }

    #[getter]
    fn kind(&self) -> &'static str {
        value_kind(&self.inner)
    }

    #[getter]
    fn text(&self) -> Option<String> {
        match &self.inner {
            Value::Literal(text) | Value::Variable(text) => Some(text.to_string()),
            Value::Number(_) | Value::Concat(_) => None,
        }
    }

    #[getter]
    fn number_value(&self) -> Option<i64> {
        match self.inner {
            Value::Number(number) => Some(number),
            Value::Literal(_) | Value::Variable(_) | Value::Concat(_) => None,
        }
    }

    #[getter]
    fn parts(&self) -> Vec<PyValue> {
        match &self.inner {
            Value::Concat(parts) => parts
                .iter()
                .cloned()
                .map(|inner| PyValue {
                    inner: inner.into_owned(),
                })
                .collect(),
            Value::Literal(_) | Value::Number(_) | Value::Variable(_) => Vec::new(),
        }
    }

    fn to_plain_string(&self) -> String {
        self.inner.to_plain_string()
    }

    fn to_lossy_string(&self) -> String {
        self.inner.to_lossy_string()
    }

    fn to_bibtex_source(&self) -> String {
        self.inner.to_bibtex_source()
    }

    fn to_unicode_string(&self) -> PyResult<String> {
        unicode_text(&self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_plain_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "Value(kind={:?}, source={:?})",
            value_kind(&self.inner),
            self.inner.to_bibtex_source()
        )
    }
}

#[pyclass(name = "Diagnostic")]
#[derive(Debug, Clone)]
struct PyDiagnostic {
    inner: Diagnostic,
}

#[pymethods]
impl PyDiagnostic {
    #[getter]
    fn severity(&self) -> &'static str {
        diagnostic_severity_name(self.inner.severity)
    }

    #[getter]
    fn code(&self) -> String {
        self.inner.code.to_string()
    }

    #[getter]
    fn message(&self) -> String {
        self.inner.message.clone()
    }

    #[getter]
    fn target(&self) -> String {
        diagnostic_target_name(&self.inner.target)
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }

    #[getter]
    fn snippet(&self) -> Option<String> {
        self.inner.snippet.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "Diagnostic(severity={:?}, code={:?}, message={:?})",
            diagnostic_severity_name(self.inner.severity),
            self.inner.code,
            self.inner.message
        )
    }
}

impl From<Diagnostic> for PyDiagnostic {
    fn from(inner: Diagnostic) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "SourceSpan")]
#[derive(Debug, Clone, Copy)]
struct PySourceSpan {
    #[pyo3(get)]
    source: Option<usize>,
    #[pyo3(get)]
    byte_start: usize,
    #[pyo3(get)]
    byte_end: usize,
    #[pyo3(get)]
    line: usize,
    #[pyo3(get)]
    column: usize,
    #[pyo3(get)]
    end_line: usize,
    #[pyo3(get)]
    end_column: usize,
}

#[pymethods]
impl PySourceSpan {
    fn __repr__(&self) -> String {
        format!(
            "SourceSpan(source={:?}, bytes={}..{}, start={}:{}, end={}:{})",
            self.source,
            self.byte_start,
            self.byte_end,
            self.line,
            self.column,
            self.end_line,
            self.end_column
        )
    }
}

impl From<SourceSpan> for PySourceSpan {
    fn from(span: SourceSpan) -> Self {
        Self {
            source: span.source.map(crate::SourceId::index),
            byte_start: span.byte_start,
            byte_end: span.byte_end,
            line: span.line,
            column: span.column,
            end_line: span.end_line,
            end_column: span.end_column,
        }
    }
}

#[pyclass(name = "Comment")]
#[derive(Debug, Clone)]
struct PyComment {
    inner: ParsedComment<'static>,
}

#[pymethods]
impl PyComment {
    #[getter]
    fn text(&self) -> String {
        self.inner.text.to_string()
    }

    #[getter]
    fn raw(&self) -> Option<String> {
        self.inner.raw.as_deref().map(ToOwned::to_owned)
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }
}

impl From<ParsedComment<'static>> for PyComment {
    fn from(inner: ParsedComment<'static>) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "Preamble")]
#[derive(Debug, Clone)]
struct PyPreamble {
    inner: ParsedPreamble<'static>,
}

#[pymethods]
impl PyPreamble {
    #[getter]
    fn value(&self) -> PyValue {
        PyValue {
            inner: self.inner.value.value.clone(),
        }
    }

    #[getter]
    fn raw(&self) -> Option<String> {
        self.inner.raw.as_deref().map(ToOwned::to_owned)
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }
}

impl From<ParsedPreamble<'static>> for PyPreamble {
    fn from(inner: ParsedPreamble<'static>) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "StringDefinition")]
#[derive(Debug, Clone)]
struct PyStringDefinition {
    inner: ParsedString<'static>,
}

#[pymethods]
impl PyStringDefinition {
    #[getter]
    fn name(&self) -> String {
        self.inner.name.to_string()
    }

    #[getter]
    fn value(&self) -> PyValue {
        PyValue {
            inner: self.inner.value.value.clone(),
        }
    }

    #[getter]
    fn raw(&self) -> Option<String> {
        self.inner.raw.as_deref().map(ToOwned::to_owned)
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }
}

impl From<ParsedString<'static>> for PyStringDefinition {
    fn from(inner: ParsedString<'static>) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "FailedBlock")]
#[derive(Debug, Clone)]
struct PyFailedBlock {
    inner: ParsedFailedBlock<'static>,
}

#[pymethods]
impl PyFailedBlock {
    #[getter]
    fn raw(&self) -> String {
        self.inner.raw.to_string()
    }

    #[getter]
    fn error(&self) -> String {
        self.inner.error.clone()
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }

    #[getter]
    fn diagnostics(&self) -> Vec<PyDiagnostic> {
        self.inner
            .diagnostics
            .iter()
            .cloned()
            .map(PyDiagnostic::from)
            .collect()
    }
}

impl From<ParsedFailedBlock<'static>> for PyFailedBlock {
    fn from(inner: ParsedFailedBlock<'static>) -> Self {
        Self { inner }
    }
}

#[pyclass(name = "Block")]
#[derive(Debug, Clone, Copy)]
struct PyBlock {
    #[pyo3(get)]
    kind: &'static str,
    #[pyo3(get)]
    index: usize,
}

impl From<ParsedBlock> for PyBlock {
    fn from(block: ParsedBlock) -> Self {
        match block {
            ParsedBlock::Entry(index) => Self {
                kind: "entry",
                index,
            },
            ParsedBlock::String(index) => Self {
                kind: "string",
                index,
            },
            ParsedBlock::Preamble(index) => Self {
                kind: "preamble",
                index,
            },
            ParsedBlock::Comment(index) => Self {
                kind: "comment",
                index,
            },
            ParsedBlock::Failed(index) => Self {
                kind: "failed",
                index,
            },
        }
    }
}

#[pyclass(name = "WriterConfig")]
#[derive(Debug, Clone)]
struct PyWriterConfig {
    #[pyo3(get, set)]
    indent: String,
    #[pyo3(get, set)]
    align_values: bool,
    #[pyo3(get, set)]
    max_line_length: usize,
    #[pyo3(get, set)]
    sort_entries: bool,
    #[pyo3(get, set)]
    sort_fields: bool,
    #[pyo3(get, set)]
    preserve_raw: bool,
    #[pyo3(get, set)]
    trailing_comma: bool,
    #[pyo3(get, set)]
    entry_separator: String,
}

#[pymethods]
impl PyWriterConfig {
    #[new]
    #[pyo3(signature = (indent="  ".to_string(), align_values=false, max_line_length=80, sort_entries=false, sort_fields=false, preserve_raw=true, trailing_comma=false, entry_separator="\n".to_string()))]
    fn new(
        indent: String,
        align_values: bool,
        max_line_length: usize,
        sort_entries: bool,
        sort_fields: bool,
        preserve_raw: bool,
        trailing_comma: bool,
        entry_separator: String,
    ) -> Self {
        Self {
            indent,
            align_values,
            max_line_length,
            sort_entries,
            sort_fields,
            preserve_raw,
            trailing_comma,
            entry_separator,
        }
    }
}

impl PyWriterConfig {
    fn to_rust(&self) -> WriterConfig {
        WriterConfig {
            indent: self.indent.clone(),
            align_values: self.align_values,
            max_line_length: self.max_line_length,
            sort_entries: self.sort_entries,
            sort_fields: self.sort_fields,
            raw_write_mode: if self.preserve_raw {
                RawWriteMode::Preserve
            } else {
                RawWriteMode::Normalize
            },
            trailing_comma: if self.trailing_comma {
                TrailingComma::Always
            } else {
                TrailingComma::Omit
            },
            entry_separator: self.entry_separator.clone(),
        }
    }
}

#[pyclass(name = "ValidationIssue")]
#[derive(Debug, Clone)]
struct PyValidationIssue {
    #[pyo3(get)]
    entry_index: usize,
    #[pyo3(get)]
    key: String,
    #[pyo3(get)]
    field: Option<String>,
    #[pyo3(get)]
    severity: String,
    #[pyo3(get)]
    message: String,
}

#[pyclass(name = "PersonName")]
#[derive(Debug, Clone)]
struct PyPersonName {
    #[pyo3(get)]
    raw: String,
    #[pyo3(get)]
    given: Vec<String>,
    #[pyo3(get)]
    family: Vec<String>,
    #[pyo3(get)]
    prefix: Vec<String>,
    #[pyo3(get)]
    suffix: Vec<String>,
    #[pyo3(get)]
    literal: Option<String>,
}

#[pymethods]
impl PyPersonName {
    fn display_name(&self) -> String {
        if let Some(literal) = &self.literal {
            return literal.clone();
        }
        let mut parts = self.given.clone();
        parts.extend(self.prefix.clone());
        parts.extend(self.family.clone());
        let mut name = parts.join(" ");
        if !self.suffix.is_empty() {
            if !name.is_empty() {
                name.push_str(", ");
            }
            name.push_str(&self.suffix.join(" "));
        }
        name
    }
}

impl From<crate::PersonName> for PyPersonName {
    fn from(name: crate::PersonName) -> Self {
        Self {
            raw: name.raw,
            given: name.given,
            family: name.family,
            prefix: name.prefix,
            suffix: name.suffix,
            literal: name.literal,
        }
    }
}

#[pyclass(name = "DateParts")]
#[derive(Debug, Clone, Copy)]
struct PyDateParts {
    #[pyo3(get)]
    year: i32,
    #[pyo3(get)]
    month: Option<u8>,
    #[pyo3(get)]
    day: Option<u8>,
}

impl From<DateParts> for PyDateParts {
    fn from(parts: DateParts) -> Self {
        Self {
            year: parts.year,
            month: parts.month,
            day: parts.day,
        }
    }
}

#[pyclass(name = "ResourceField")]
#[derive(Debug, Clone)]
struct PyResourceField {
    #[pyo3(get)]
    kind: String,
    #[pyo3(get)]
    field_name: String,
    #[pyo3(get)]
    value: String,
    #[pyo3(get)]
    normalized: Option<String>,
}

impl From<ResourceField> for PyResourceField {
    fn from(resource: ResourceField) -> Self {
        Self {
            kind: resource.kind.as_str().to_string(),
            field_name: resource.field_name,
            value: resource.value,
            normalized: resource.normalized,
        }
    }
}

#[pyfunction]
#[pyo3(signature = (text, tolerant=false, capture_source=true, preserve_raw=true, expand_values=false, latex_to_unicode=false, source=None))]
fn parse_text(
    text: &str,
    tolerant: bool,
    capture_source: bool,
    preserve_raw: bool,
    expand_values: bool,
    latex_to_unicode: bool,
    source: Option<String>,
) -> PyResult<PyDocument> {
    let parser = PyParser::new(
        tolerant,
        capture_source,
        preserve_raw,
        expand_values,
        latex_to_unicode,
    );
    parser.parse(text, source)
}

#[pyfunction]
#[pyo3(signature = (path, tolerant=false, capture_source=true, preserve_raw=true, expand_values=false, latex_to_unicode=false))]
fn parse_file(
    path: &str,
    tolerant: bool,
    capture_source: bool,
    preserve_raw: bool,
    expand_values: bool,
    latex_to_unicode: bool,
) -> PyResult<PyDocument> {
    let parser = PyParser::new(
        tolerant,
        capture_source,
        preserve_raw,
        expand_values,
        latex_to_unicode,
    );
    parser.parse_file(path)
}

#[pyfunction]
#[pyo3(signature = (document, config=None))]
fn write(document: &PyDocument, config: Option<&PyWriterConfig>) -> PyResult<String> {
    document.write(config)
}

#[pyfunction(name = "normalize_doi")]
fn normalize_doi_py(input: &str) -> Option<String> {
    normalize_doi(input)
}

#[pyfunction(name = "parse_names")]
fn parse_names_py(input: &str) -> Vec<PyPersonName> {
    parse_names(input)
        .into_iter()
        .map(PyPersonName::from)
        .collect()
}

#[pyfunction(name = "parse_date")]
fn parse_date_py(input: &str) -> PyResult<PyDateParts> {
    parse_date_parts(input)
        .map(PyDateParts::from)
        .map_err(date_error)
}

#[pyfunction(name = "latex_to_unicode")]
fn latex_to_unicode_py(input: &str) -> PyResult<String> {
    latex_to_unicode(input)
}

fn parse_document_with_options(
    options: &PyParser,
    text: &str,
    source: Option<String>,
) -> PyResult<PyDocument> {
    let mut parser = Parser::new();
    if options.tolerant {
        parser = parser.tolerant();
    }
    if options.capture_source {
        parser = parser.capture_source();
    }
    if options.preserve_raw {
        parser = parser.preserve_raw();
    }
    if options.expand_values {
        parser = parser.expand_values();
    }

    let document = if let Some(source) = source {
        parser.parse_source(source, text)
    } else {
        parser.parse_document(text)
    }
    .map_err(map_error)?;
    let mut document = document.into_owned();
    if options.latex_to_unicode {
        apply_latex_to_unicode(&mut document)?;
    }
    Ok(PyDocument { inner: document })
}

fn write_document(
    document: &ParsedDocument<'static>,
    config: Option<WriterConfig>,
) -> PyResult<String> {
    if let Some(config) = config {
        let mut buffer = Vec::new();
        Writer::with_config(&mut buffer, config)
            .write_document(document)
            .map_err(map_error)?;
        String::from_utf8(buffer).map_err(|error| PyRuntimeError::new_err(error.to_string()))
    } else {
        document_to_string(document).map_err(map_error)
    }
}

fn value_from_py(value: &Bound<'_, PyAny>) -> PyResult<Value<'static>> {
    if let Ok(value) = value.extract::<PyRef<'_, PyValue>>() {
        return Ok(value.inner.clone());
    }
    if let Ok(text) = value.extract::<String>() {
        return Ok(Value::Literal(Cow::Owned(text)));
    }
    if let Ok(number) = value.extract::<i64>() {
        return Ok(Value::Number(number));
    }
    Err(PyTypeError::new_err(
        "expected a citerra.Value, str, or int",
    ))
}

fn apply_latex_to_unicode(document: &mut ParsedDocument<'static>) -> PyResult<()> {
    for entry in document.entries_mut() {
        for field in &mut entry.fields {
            let text = latex_to_unicode(&field.value.value.to_plain_string())?;
            field.value.value = Value::Literal(Cow::Owned(text));
            field.value.raw = None;
            field.value.expanded = None;
            field.raw = None;
        }
        entry.raw = None;
    }
    Ok(())
}

fn latex_to_unicode(input: &str) -> PyResult<String> {
    #[cfg(feature = "latex_to_unicode")]
    {
        Ok(crate::latex_unicode::latex_to_unicode(input))
    }

    #[cfg(not(feature = "latex_to_unicode"))]
    {
        let _ = input;
        Err(PyRuntimeError::new_err(
            "latex_to_unicode support was not enabled for this build",
        ))
    }
}

fn unicode_text(value: &Value<'_>) -> PyResult<String> {
    latex_to_unicode(&value.to_plain_string())
}

fn map_error(error: impl std::error::Error) -> PyErr {
    BibtexParserError::new_err(error.to_string())
}

fn date_error(error: DateParseError) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn validation_level(level: &str) -> PyResult<ValidationLevel> {
    match level {
        "minimal" => Ok(ValidationLevel::Minimal),
        "standard" => Ok(ValidationLevel::Standard),
        "strict" => Ok(ValidationLevel::Strict),
        _ => Err(PyValueError::new_err(
            "validation level must be 'minimal', 'standard', or 'strict'",
        )),
    }
}

fn parse_status_name(status: crate::ParseStatus) -> &'static str {
    match status {
        crate::ParseStatus::Ok => "ok",
        crate::ParseStatus::Partial => "partial",
        crate::ParseStatus::Failed => "failed",
    }
}

fn diagnostic_severity_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
    }
}

fn validation_severity_name(severity: ValidationSeverity) -> &'static str {
    match severity {
        ValidationSeverity::Error => "error",
        ValidationSeverity::Warning => "warning",
        ValidationSeverity::Info => "info",
    }
}

fn diagnostic_target_name(target: &DiagnosticTarget) -> String {
    match target {
        DiagnosticTarget::File => "file".to_string(),
        DiagnosticTarget::Block(index) => format!("block:{index}"),
        DiagnosticTarget::Entry(index) => format!("entry:{index}"),
        DiagnosticTarget::Field { entry, field } => format!("field:{entry}:{field}"),
        DiagnosticTarget::Value { entry, field } => format!("value:{entry}:{field}"),
        DiagnosticTarget::FailedBlock(index) => format!("failed-block:{index}"),
    }
}

fn value_kind(value: &Value<'_>) -> &'static str {
    match value {
        Value::Literal(_) => "literal",
        Value::Number(_) => "number",
        Value::Concat(_) => "concat",
        Value::Variable(_) => "variable",
    }
}

fn field_text(field: &ParsedField<'_>) -> String {
    field
        .value
        .expanded
        .as_deref()
        .map_or_else(|| field.value.plain_text(), ToOwned::to_owned)
}
