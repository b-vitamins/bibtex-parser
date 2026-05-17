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
    normalize_doi, parse_date_parts, parse_names, DateParseError, DateParts, Diagnostic,
    DiagnosticSeverity, DiagnosticTarget, EntryType, ParsedBlock, ParsedComment, ParsedDocument,
    ParsedEntry, ParsedEntryStatus, ParsedFailedBlock, ParsedField, ParsedPreamble, ParsedString,
    ParsedValue, Parser, RawWriteMode, ResourceField, SourceSpan, TrailingComma, ValidationLevel,
    ValidationSeverity, Value, Writer, WriterConfig,
};
use ahash::{AHashMap, AHashSet};
use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyIterator, PyList, PyModule, PyString, PyTuple, PyType};
use std::borrow::Cow;
use std::sync::Arc;

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
    m.add_function(wrap_pyfunction!(document_to_dicts_py, m)?)?;
    m.add_function(wrap_pyfunction!(write_entries_py, m)?)?;
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
    fn parse(&self, py: Python<'_>, text: &str, source: Option<String>) -> PyResult<PyDocument> {
        let options = self.clone();
        py.detach(move || parse_document_with_options(&options, text, source))
    }

    #[pyo3(signature = (path))]
    fn parse_file(&self, py: Python<'_>, path: &str) -> PyResult<PyDocument> {
        let options = self.clone();
        let path = path.to_string();
        py.detach(move || {
            let text = std::fs::read_to_string(&path).map_err(map_error)?;
            parse_document_with_options(&options, &text, Some(path))
        })
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
    raw_source: Option<Arc<str>>,
}

impl PyDocument {
    fn new(inner: ParsedDocument<'static>, raw_source: Option<Arc<str>>) -> Self {
        Self { inner, raw_source }
    }

    fn materialize_raw_source(&mut self) {
        if let Some(source) = self.raw_source.take() {
            self.inner.apply_raw_from_source(&source);
        }
    }

    fn raw_slice(&self, span: Option<SourceSpan>) -> Option<String> {
        raw_source_slice(self.raw_source.as_ref(), span)
    }
}

#[pymethods]
impl PyDocument {
    #[getter]
    fn status(&self) -> &'static str {
        parse_status_name(self.inner.status())
    }

    #[getter]
    fn entries(slf: PyRef<'_, Self>) -> Vec<PyEntry> {
        let len = slf.inner.entries().len();
        let py = slf.py();
        let document: Py<PyDocument> = slf.into();
        (0..len)
            .map(|index| PyEntry::view(document.clone_ref(py), index))
            .collect()
    }

    #[getter]
    fn comments(&self) -> Vec<PyComment> {
        let raw_source = self.raw_source.clone();
        self.inner
            .comments()
            .iter()
            .cloned()
            .map(|comment| PyComment::new(comment, raw_source.clone()))
            .collect()
    }

    #[getter]
    fn preambles(&self) -> Vec<PyPreamble> {
        let raw_source = self.raw_source.clone();
        self.inner
            .preambles()
            .iter()
            .cloned()
            .map(|preamble| PyPreamble::new(preamble, raw_source.clone()))
            .collect()
    }

    #[getter]
    fn strings(&self) -> Vec<PyStringDefinition> {
        let raw_source = self.raw_source.clone();
        self.inner
            .strings()
            .iter()
            .cloned()
            .map(|string| PyStringDefinition::new(string, raw_source.clone()))
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

    fn entry(slf: PyRef<'_, Self>, key: &str) -> Option<PyEntry> {
        let index = slf
            .inner
            .entries()
            .iter()
            .position(|entry| entry.key() == key)?;
        let py = slf.py();
        let document: Py<PyDocument> = slf.into();
        Some(PyEntry::view(document.clone_ref(py), index))
    }

    fn keys(&self) -> Vec<String> {
        self.inner
            .entries()
            .iter()
            .map(|entry| entry.key().to_string())
            .collect()
    }

    #[pyo3(signature = (value_mode="plain"))]
    fn to_dicts<'py>(&self, py: Python<'py>, value_mode: &str) -> PyResult<Bound<'py, PyList>> {
        document_to_dicts_for_py(py, &self.inner, value_mode)
    }

    #[pyo3(signature = (records, remove_missing=false, create_missing=false))]
    fn update_from_dicts<'py>(
        &mut self,
        py: Python<'py>,
        records: &Bound<'_, PyAny>,
        remove_missing: bool,
        create_missing: bool,
    ) -> PyResult<Bound<'py, PyDict>> {
        self.materialize_raw_source();
        let summary = update_document_from_dicts(
            py,
            &mut self.inner,
            records,
            remove_missing,
            create_missing,
        )?;
        summary.to_py_dict(py)
    }

    fn rename_key(&mut self, old: &str, new: String) -> bool {
        self.materialize_raw_source();
        self.inner.rename_key(old, Cow::Owned(new))
    }

    fn set_entry_type(&mut self, key: &str, entry_type: &str) -> bool {
        self.materialize_raw_source();
        let Some(entry) = self.inner.entry_mut_by_key(key) else {
            return false;
        };
        entry.set_entry_type(EntryType::parse(entry_type).into_owned());
        true
    }

    fn set_field(&mut self, key: &str, name: &str, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.materialize_raw_source();
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
        self.materialize_raw_source();
        let parsed_value = value_from_py(value)?;
        let Some(entry) = self.inner.entry_mut_by_key(key) else {
            return Ok(false);
        };
        entry.add_field(Cow::Owned(name.to_string()), parsed_value);
        Ok(true)
    }

    fn rename_field(&mut self, key: &str, old: &str, new: String) -> usize {
        self.materialize_raw_source();
        self.inner
            .entry_mut_by_key(key)
            .map_or(0, |entry| entry.rename_field(old, Cow::Owned(new)))
    }

    fn remove_field(&mut self, key: &str, name: &str) -> usize {
        self.materialize_raw_source();
        self.inner
            .entry_mut_by_key(key)
            .map_or(0, |entry| entry.remove_field(name))
    }

    fn remove_export_fields(&mut self, names: Vec<String>) -> usize {
        self.materialize_raw_source();
        let borrowed_names = names.iter().map(String::as_str).collect::<Vec<_>>();
        self.inner.remove_export_fields(&borrowed_names)
    }

    #[pyo3(signature = (config=None))]
    fn write(&self, py: Python<'_>, config: Option<&PyWriterConfig>) -> PyResult<String> {
        let config = config.map(PyWriterConfig::to_rust);
        py.detach(move || write_document(self, config))
    }

    fn write_selected(&self, py: Python<'_>, keys: Vec<String>) -> PyResult<String> {
        py.detach(move || {
            let borrowed = keys.iter().map(String::as_str).collect::<Vec<_>>();
            selected_entries_to_string(self, &borrowed).map_err(map_error)
        })
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
        self.materialize_raw_source();
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
struct PyEntry {
    inner: PyEntryInner,
}

enum PyEntryInner {
    View {
        document: Py<PyDocument>,
        index: usize,
    },
}

impl PyEntry {
    fn view(document: Py<PyDocument>, index: usize) -> Self {
        Self {
            inner: PyEntryInner::View { document, index },
        }
    }

    fn with_entry<R>(
        &self,
        py: Python<'_>,
        on_entry: impl FnOnce(&ParsedEntry<'static>) -> R,
    ) -> PyResult<R> {
        self.with_document_entry(py, |_, entry| on_entry(entry))
    }

    fn with_document_entry<R>(
        &self,
        py: Python<'_>,
        on_entry: impl FnOnce(&PyDocument, &ParsedEntry<'static>) -> R,
    ) -> PyResult<R> {
        match &self.inner {
            PyEntryInner::View { document, index } => {
                let document = document.borrow(py);
                let entry = document.inner.entries().get(*index).ok_or_else(|| {
                    PyRuntimeError::new_err("entry view no longer points to a valid entry")
                })?;
                Ok(on_entry(&document, entry))
            }
        }
    }
}

#[pymethods]
impl PyEntry {
    #[getter]
    fn key(&self, py: Python<'_>) -> PyResult<String> {
        self.with_entry(py, |entry| entry.key().to_string())
    }

    #[getter]
    fn entry_type(&self, py: Python<'_>) -> PyResult<String> {
        self.with_entry(py, |entry| entry.ty.to_string())
    }

    #[getter]
    fn status(&self, py: Python<'_>) -> PyResult<&'static str> {
        self.with_entry(py, |entry| match entry.status {
            ParsedEntryStatus::Complete => "complete",
            ParsedEntryStatus::Partial => "partial",
        })
    }

    #[getter]
    fn fields(&self, py: Python<'_>) -> PyResult<Vec<PyField>> {
        self.with_document_entry(py, |document, entry| {
            let raw_source = document.raw_source.clone();
            entry
                .fields
                .iter()
                .cloned()
                .map(|field| PyField::new(field, raw_source.clone()))
                .collect()
        })
    }

    #[getter]
    fn raw(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.with_document_entry(py, |document, entry| {
            entry
                .raw
                .as_deref()
                .map(ToOwned::to_owned)
                .or_else(|| document.raw_slice(entry.source))
        })
    }

    #[getter]
    fn source(&self, py: Python<'_>) -> PyResult<Option<PySourceSpan>> {
        self.with_entry(py, |entry| entry.source.map(PySourceSpan::from))
    }

    fn get(&self, py: Python<'_>, name: &str) -> PyResult<Option<String>> {
        self.with_entry(py, |entry| entry.field_ignore_case(name).map(field_text))
    }

    fn field(&self, py: Python<'_>, name: &str) -> PyResult<Option<PyField>> {
        self.with_document_entry(py, |document, entry| {
            entry
                .field_ignore_case(name)
                .cloned()
                .map(|field| PyField::new(field, document.raw_source.clone()))
        })
    }

    fn authors(&self, py: Python<'_>) -> PyResult<Vec<PyPersonName>> {
        self.with_entry(py, |entry| {
            entry
                .authors()
                .into_iter()
                .map(PyPersonName::from)
                .collect()
        })
    }

    fn editors(&self, py: Python<'_>) -> PyResult<Vec<PyPersonName>> {
        self.with_entry(py, |entry| {
            entry
                .editors()
                .into_iter()
                .map(PyPersonName::from)
                .collect()
        })
    }

    fn translators(&self, py: Python<'_>) -> PyResult<Vec<PyPersonName>> {
        self.with_entry(py, |entry| {
            entry
                .translators()
                .into_iter()
                .map(PyPersonName::from)
                .collect()
        })
    }

    fn date_parts(&self, py: Python<'_>) -> PyResult<Option<PyDateParts>> {
        self.with_entry(py, |entry| entry.date_parts().transpose())?
            .map_err(date_error)
            .map(|parts| parts.map(PyDateParts::from))
    }

    fn doi(&self, py: Python<'_>) -> PyResult<Option<String>> {
        self.with_entry(py, ParsedEntry::doi)
    }

    fn resource_fields(&self, py: Python<'_>) -> PyResult<Vec<PyResourceField>> {
        self.with_entry(py, |entry| {
            entry
                .resource_fields()
                .into_iter()
                .map(PyResourceField::from)
                .collect()
        })
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        self.with_entry(py, |entry| {
            format!(
                "Entry(key={:?}, entry_type={:?}, fields={})",
                entry.key(),
                entry.ty,
                entry.fields.len()
            )
        })
    }
}

#[pyclass(name = "Field")]
#[derive(Debug, Clone)]
struct PyField {
    inner: ParsedField<'static>,
    raw_source: Option<Arc<str>>,
}

impl PyField {
    fn new(inner: ParsedField<'static>, raw_source: Option<Arc<str>>) -> Self {
        Self { inner, raw_source }
    }
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
        self.inner
            .raw
            .as_deref()
            .map(ToOwned::to_owned)
            .or_else(|| raw_source_slice(self.raw_source.as_ref(), self.inner.source))
    }

    #[getter]
    fn raw_value(&self) -> Option<String> {
        self.inner
            .value
            .raw
            .as_deref()
            .map(ToOwned::to_owned)
            .or_else(|| raw_source_slice(self.raw_source.as_ref(), self.inner.value_source))
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
        Self::new(inner, None)
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

    #[classmethod]
    fn from_bibtex_source(_cls: &Bound<'_, PyType>, source: &str) -> PyResult<Self> {
        Ok(Self {
            inner: Value::from_bibtex_source(source)
                .map_err(|error| PyValueError::new_err(error.to_string()))?
                .into_owned(),
        })
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
    raw_source: Option<Arc<str>>,
}

impl PyComment {
    fn new(inner: ParsedComment<'static>, raw_source: Option<Arc<str>>) -> Self {
        Self { inner, raw_source }
    }
}

#[pymethods]
impl PyComment {
    #[getter]
    fn text(&self) -> String {
        self.inner.text.to_string()
    }

    #[getter]
    fn raw(&self) -> Option<String> {
        self.inner
            .raw
            .as_deref()
            .map(ToOwned::to_owned)
            .or_else(|| raw_source_slice(self.raw_source.as_ref(), self.inner.source))
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }
}

impl From<ParsedComment<'static>> for PyComment {
    fn from(inner: ParsedComment<'static>) -> Self {
        Self::new(inner, None)
    }
}

#[pyclass(name = "Preamble")]
#[derive(Debug, Clone)]
struct PyPreamble {
    inner: ParsedPreamble<'static>,
    raw_source: Option<Arc<str>>,
}

impl PyPreamble {
    fn new(inner: ParsedPreamble<'static>, raw_source: Option<Arc<str>>) -> Self {
        Self { inner, raw_source }
    }
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
        self.inner
            .raw
            .as_deref()
            .map(ToOwned::to_owned)
            .or_else(|| raw_source_slice(self.raw_source.as_ref(), self.inner.source))
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }
}

impl From<ParsedPreamble<'static>> for PyPreamble {
    fn from(inner: ParsedPreamble<'static>) -> Self {
        Self::new(inner, None)
    }
}

#[pyclass(name = "StringDefinition")]
#[derive(Debug, Clone)]
struct PyStringDefinition {
    inner: ParsedString<'static>,
    raw_source: Option<Arc<str>>,
}

impl PyStringDefinition {
    fn new(inner: ParsedString<'static>, raw_source: Option<Arc<str>>) -> Self {
        Self { inner, raw_source }
    }
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
        self.inner
            .raw
            .as_deref()
            .map(ToOwned::to_owned)
            .or_else(|| raw_source_slice(self.raw_source.as_ref(), self.inner.source))
    }

    #[getter]
    fn source(&self) -> Option<PySourceSpan> {
        self.inner.source.map(PySourceSpan::from)
    }
}

impl From<ParsedString<'static>> for PyStringDefinition {
    fn from(inner: ParsedString<'static>) -> Self {
        Self::new(inner, None)
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
    py: Python<'_>,
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
    py.detach(move || parse_document_with_options(&parser, text, source))
}

#[pyfunction]
#[pyo3(signature = (path, tolerant=false, capture_source=true, preserve_raw=true, expand_values=false, latex_to_unicode=false))]
fn parse_file(
    py: Python<'_>,
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
    let path = path.to_string();
    py.detach(move || {
        let text = std::fs::read_to_string(&path).map_err(map_error)?;
        parse_document_with_options(&parser, &text, Some(path))
    })
}

#[pyfunction]
#[pyo3(signature = (document, config=None))]
fn write(
    py: Python<'_>,
    document: &PyDocument,
    config: Option<&PyWriterConfig>,
) -> PyResult<String> {
    let config = config.map(PyWriterConfig::to_rust);
    py.detach(move || write_document(document, config))
}

#[pyfunction(name = "_document_to_dicts")]
#[pyo3(signature = (document, value_mode="plain"))]
fn document_to_dicts_py<'py>(
    py: Python<'py>,
    document: &PyDocument,
    value_mode: &str,
) -> PyResult<Bound<'py, PyList>> {
    document_to_dicts_for_py(py, &document.inner, value_mode)
}

#[pyfunction(name = "_write_entries")]
#[pyo3(signature = (entries, comments=None, preambles=None, strings=None, field_order=None, sort_by=None, reverse=false, trailing_comma=false, entry_separator="\n\n"))]
fn write_entries_py(
    entries: &Bound<'_, PyAny>,
    comments: Option<&Bound<'_, PyAny>>,
    preambles: Option<&Bound<'_, PyAny>>,
    strings: Option<&Bound<'_, PyAny>>,
    field_order: Option<Vec<String>>,
    sort_by: Option<Vec<String>>,
    reverse: bool,
    trailing_comma: bool,
    entry_separator: &str,
) -> PyResult<String> {
    render_plain_entries(
        entries,
        comments,
        preambles,
        strings,
        field_order.as_deref(),
        sort_by.as_deref(),
        reverse,
        trailing_comma,
        entry_separator,
    )
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

    if !options.tolerant {
        let mut document = if !options.capture_source && !options.preserve_raw {
            parser
                .parse_compact_document_owned(source, text)
                .map_err(map_error)?
        } else {
            parser
                .parse_source_document_owned(source, text)
                .map_err(map_error)?
        };
        let raw_source = if options.preserve_raw {
            Some(Arc::<str>::from(text))
        } else {
            None
        };
        if options.latex_to_unicode {
            if let Some(source) = &raw_source {
                document.apply_raw_from_source(source);
            }
            apply_latex_to_unicode(&mut document)?;
        }
        return Ok(PyDocument::new(
            document,
            if options.latex_to_unicode {
                None
            } else {
                raw_source
            },
        ));
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
    Ok(PyDocument::new(document, None))
}

fn write_document(document: &PyDocument, config: Option<WriterConfig>) -> PyResult<String> {
    let mut buffer = Vec::new();
    let raw_source = document.raw_source.as_deref();
    match config {
        Some(config) => Writer::with_config(&mut buffer, config)
            .write_document_with_raw_source(&document.inner, raw_source)
            .map_err(map_error)?,
        None => Writer::new(&mut buffer)
            .write_document_with_raw_source(&document.inner, raw_source)
            .map_err(map_error)?,
    }
    String::from_utf8(buffer).map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

fn selected_entries_to_string(document: &PyDocument, keys: &[&str]) -> PyResult<String> {
    let mut buffer = Vec::new();
    Writer::new(&mut buffer)
        .write_selected_entries_with_raw_source(
            &document.inner,
            keys,
            document.raw_source.as_deref(),
        )
        .map_err(map_error)?;
    String::from_utf8(buffer).map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

#[derive(Debug)]
struct PlainRecord {
    entry_type: Option<String>,
    key: String,
    fields: Vec<(String, Value<'static>)>,
}

fn render_plain_entries(
    entries: &Bound<'_, PyAny>,
    comments: Option<&Bound<'_, PyAny>>,
    preambles: Option<&Bound<'_, PyAny>>,
    strings: Option<&Bound<'_, PyAny>>,
    field_order: Option<&[String]>,
    sort_by: Option<&[String]>,
    reverse: bool,
    trailing_comma: bool,
    entry_separator: &str,
) -> PyResult<String> {
    let mut rendered = Vec::new();

    render_comments(&mut rendered, comments)?;
    render_preambles(&mut rendered, preambles)?;
    render_strings(&mut rendered, strings)?;

    let mut records = plain_records_from_py(entries, field_order)?;
    if let Some(sort_by) = sort_by {
        records.sort_by_cached_key(|record| sort_record_key(record, sort_by));
        if reverse {
            records.reverse();
        }
    }

    rendered.extend(
        records
            .iter()
            .map(|record| render_plain_record(record, trailing_comma)),
    );

    Ok(rendered
        .into_iter()
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join(entry_separator))
}

fn render_comments(
    rendered: &mut Vec<String>,
    comments: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let Some(comments) = comments else {
        return Ok(());
    };
    for comment in PyIterator::from_object(comments)? {
        let text = comment?.str()?.to_string();
        let stripped = text.trim_start();
        if stripped.starts_with('%') || stripped.starts_with('@') {
            rendered.push(text);
        } else {
            rendered.push(format!("@comment{{{text}}}"));
        }
    }
    Ok(())
}

fn render_preambles(
    rendered: &mut Vec<String>,
    preambles: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let Some(preambles) = preambles else {
        return Ok(());
    };
    for preamble in PyIterator::from_object(preambles)? {
        let value = value_from_py(&preamble?)?;
        rendered.push(format!("@preamble{{{}}}", value.to_bibtex_source()));
    }
    Ok(())
}

fn render_strings(rendered: &mut Vec<String>, strings: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let Some(strings) = strings else {
        return Ok(());
    };

    if let Ok(dict) = strings.cast::<PyDict>() {
        for (name, value) in dict.iter() {
            let name = name.str()?.to_string();
            let value = value_from_py(&value)?;
            rendered.push(format!("@string{{{name} = {}}}", value.to_bibtex_source()));
        }
        return Ok(());
    }

    for item in PyIterator::from_object(strings)? {
        let item = item?;
        let pair = item.cast::<PyTuple>().map_err(|_| {
            PyTypeError::new_err("strings must be a dict or an iterable of (name, value) pairs")
        })?;
        if pair.len() != 2 {
            return Err(PyValueError::new_err(
                "string definition pairs must contain exactly two items",
            ));
        }
        let name = pair.get_item(0)?.str()?.to_string();
        let value = value_from_py(&pair.get_item(1)?)?;
        rendered.push(format!("@string{{{name} = {}}}", value.to_bibtex_source()));
    }
    Ok(())
}

fn plain_records_from_py(
    entries: &Bound<'_, PyAny>,
    field_order: Option<&[String]>,
) -> PyResult<Vec<PlainRecord>> {
    let mut records = Vec::new();
    for item in PyIterator::from_object(entries)? {
        let item = item?;
        let dict = item
            .cast::<PyDict>()
            .map_err(|_| PyTypeError::new_err("entries must be dictionaries"))?;
        records.push(plain_record_from_dict(dict, field_order)?);
    }
    Ok(records)
}

fn plain_record_from_dict(
    dict: &Bound<'_, PyDict>,
    field_order: Option<&[String]>,
) -> PyResult<PlainRecord> {
    let entry_type = dict
        .get_item("ENTRYTYPE")?
        .map(|value| value.str().map(|s| s.to_string().trim().to_string()))
        .transpose()?;
    let key = dict
        .get_item("ID")?
        .map_or_else(String::new, |value| {
            value.str().map(|s| s.to_string()).unwrap_or_default()
        })
        .trim()
        .to_string();

    let mut fields = Vec::new();
    let mut seen = AHashSet::new();
    seen.insert("ENTRYTYPE".to_string());
    seen.insert("ID".to_string());

    if let Some(field_order) = field_order {
        for field in field_order {
            if seen.contains(field) {
                continue;
            }
            if let Some(value) = dict.get_item(field)? {
                fields.push((field.clone(), value_from_py(&value)?));
                seen.insert(field.clone());
            }
        }
    }

    for (name, value) in dict.iter() {
        let name = name.str()?.to_string();
        if seen.contains(&name) {
            continue;
        }
        fields.push((name.clone(), value_from_py(&value)?));
        seen.insert(name);
    }

    Ok(PlainRecord {
        entry_type,
        key,
        fields,
    })
}

fn render_plain_record(record: &PlainRecord, trailing_comma: bool) -> String {
    let entry_type = record.entry_type.as_deref().unwrap_or("article");
    let entry_type = if entry_type.is_empty() {
        "article"
    } else {
        entry_type
    };
    let mut output = format!("@{entry_type}{{{},", record.key);
    for (index, (name, value)) in record.fields.iter().enumerate() {
        output.push('\n');
        output.push_str("  ");
        output.push_str(name);
        output.push_str(" = ");
        output.push_str(&value.to_bibtex_source());
        if index < record.fields.len() - 1 || trailing_comma {
            output.push(',');
        }
    }
    output.push_str("\n}");
    output
}

fn sort_record_key(record: &PlainRecord, sort_by: &[String]) -> Vec<String> {
    sort_by
        .iter()
        .map(|name| {
            if name == "ENTRYTYPE" {
                return record.entry_type.clone().unwrap_or_default();
            }
            if name == "ID" {
                return record.key.clone();
            }
            record
                .fields
                .iter()
                .find(|(field, _)| field == name)
                .map_or_else(String::new, |(_, value)| value.to_plain_string())
        })
        .collect()
}

#[derive(Debug, Default)]
struct RecordUpdateSummary {
    matched_entries: usize,
    added_entries: usize,
    changed_entry_types: usize,
    added_fields: usize,
    changed_fields: usize,
    unchanged_fields: usize,
    removed_fields: usize,
}

impl RecordUpdateSummary {
    fn to_py_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("matched_entries", self.matched_entries)?;
        dict.set_item("added_entries", self.added_entries)?;
        dict.set_item("changed_entry_types", self.changed_entry_types)?;
        dict.set_item("added_fields", self.added_fields)?;
        dict.set_item("changed_fields", self.changed_fields)?;
        dict.set_item("unchanged_fields", self.unchanged_fields)?;
        dict.set_item("removed_fields", self.removed_fields)?;
        Ok(dict)
    }
}

fn update_document_from_dicts(
    _py: Python<'_>,
    document: &mut ParsedDocument<'static>,
    records: &Bound<'_, PyAny>,
    remove_missing: bool,
    create_missing: bool,
) -> PyResult<RecordUpdateSummary> {
    let records = plain_records_from_py(records, None)?;
    let mut seen_records = AHashSet::new();
    for record in &records {
        validate_record(record)?;
        if !seen_records.insert(record.key.clone()) {
            return Err(PyValueError::new_err(format!(
                "duplicate record ID {:?}",
                record.key
            )));
        }
    }

    let mut key_index = AHashMap::new();
    for (index, entry) in document.entries().iter().enumerate() {
        key_index.entry(entry.key().to_string()).or_insert(index);
    }

    let mut summary = RecordUpdateSummary::default();
    for record in records {
        if let Some(index) = key_index.get(&record.key).copied() {
            summary.matched_entries += 1;
            let entry = &mut document.entries_mut()[index];
            apply_record_to_entry(entry, &record, remove_missing, &mut summary);
        } else if create_missing {
            let entry = parsed_entry_from_record(record);
            document.push_entry(entry);
            summary.added_entries += 1;
        } else {
            return Err(PyValueError::new_err(format!(
                "record ID {:?} does not match an existing entry",
                record.key
            )));
        }
    }

    Ok(summary)
}

fn validate_record(record: &PlainRecord) -> PyResult<()> {
    if record.key.is_empty() {
        return Err(PyValueError::new_err("record ID must not be empty"));
    }
    for (field, _) in &record.fields {
        if !is_valid_bibtex_identifier(field) {
            return Err(PyValueError::new_err(format!(
                "invalid BibTeX field name {field:?}"
            )));
        }
    }
    Ok(())
}

fn apply_record_to_entry(
    entry: &mut ParsedEntry<'static>,
    record: &PlainRecord,
    remove_missing: bool,
    summary: &mut RecordUpdateSummary,
) {
    if let Some(record_entry_type) = &record.entry_type {
        let entry_type = crate::EntryType::parse(record_entry_type).into_owned();
        if entry.ty != entry_type {
            entry.set_entry_type(entry_type);
            summary.changed_entry_types += 1;
        }
    }

    let mut present = AHashSet::new();
    for (field_name, value) in &record.fields {
        present.insert(field_name.as_str());
        if let Some(field) = entry
            .fields
            .iter_mut()
            .find(|field| field.name == *field_name)
        {
            if parsed_field_value_matches(field, value) {
                summary.unchanged_fields += 1;
            } else {
                field.value.value = value.clone();
                field.value.raw = None;
                field.value.expanded = None;
                field.raw = None;
                summary.changed_fields += 1;
            }
        } else {
            entry.add_field(Cow::Owned(field_name.clone()), value.clone());
            summary.added_fields += 1;
        }
    }

    if remove_missing {
        let mut index = 0usize;
        while index < entry.fields.len() {
            if present.contains(entry.fields[index].name.as_ref()) {
                index += 1;
            } else {
                let _ = entry.remove_field_by_index(index);
                summary.removed_fields += 1;
            }
        }
    }
}

fn parsed_entry_from_record(record: PlainRecord) -> ParsedEntry<'static> {
    let entry_type = record
        .entry_type
        .as_deref()
        .filter(|entry_type| !entry_type.is_empty())
        .unwrap_or("article");
    ParsedEntry {
        ty: crate::EntryType::parse(entry_type).into_owned(),
        key: Cow::Owned(record.key),
        fields: record
            .fields
            .into_iter()
            .map(|(name, value)| ParsedField {
                name: Cow::Owned(name),
                value: ParsedValue::new(value),
                raw: None,
                source: None,
                name_source: None,
                value_source: None,
            })
            .collect(),
        status: ParsedEntryStatus::Complete,
        source: None,
        entry_type_source: None,
        key_source: None,
        delimiter: None,
        raw: None,
        removed_field_sources: None,
        diagnostics: Vec::new(),
    }
}

fn parsed_field_value_matches(field: &ParsedField<'_>, value: &Value<'_>) -> bool {
    field.value.value == *value || field.value.plain_text() == value.to_plain_string()
}

fn is_valid_bibtex_identifier(name: &str) -> bool {
    !name.is_empty() && crate::parser::simd::scan_identifier(name.as_bytes()) == name.len()
}

fn document_to_dicts_for_py<'py>(
    py: Python<'py>,
    document: &ParsedDocument<'_>,
    value_mode: &str,
) -> PyResult<Bound<'py, PyList>> {
    let value_mode = RecordValueMode::parse(value_mode)?;
    let records = PyList::empty(py);
    let entry_type_key = PyString::new(py, "ENTRYTYPE");
    let id_key = PyString::new(py, "ID");
    let mut field_keys = AHashMap::new();
    for entry in document.entries() {
        let record = PyDict::new(py);
        record.set_item(&entry_type_key, entry.ty.canonical_name())?;
        record.set_item(&id_key, entry.key())?;
        for field in &entry.fields {
            let key = cached_py_string(py, &mut field_keys, field.name.as_ref());
            set_record_field_text(py, &record, key.bind(py), field, value_mode)?;
        }
        records.append(record)?;
    }
    Ok(records)
}

fn cached_py_string<'a>(
    py: Python<'_>,
    cache: &mut AHashMap<&'a str, Py<PyString>>,
    text: &'a str,
) -> Py<PyString> {
    if let Some(key) = cache.get(text) {
        return key.clone_ref(py);
    }

    let key = PyString::new(py, text).unbind();
    cache.insert(text, key.clone_ref(py));
    key
}

fn set_record_field_text(
    py: Python<'_>,
    record: &Bound<'_, PyDict>,
    key: &Bound<'_, PyString>,
    field: &ParsedField<'_>,
    value_mode: RecordValueMode,
) -> PyResult<()> {
    match value_mode {
        RecordValueMode::Plain => match &field.value.value {
            Value::Literal(text) if !needs_text_projection(text) => {
                record.set_item(key, text.as_ref())
            }
            Value::Variable(name) => record.set_item(key, name.as_ref()),
            Value::Number(number) => {
                let mut buffer = itoa::Buffer::new();
                record.set_item(key, buffer.format(*number))
            }
            Value::Concat(_) | Value::Literal(_) => record.set_item(key, field.value.plain_text()),
        },
        RecordValueMode::Expanded => record.set_item(
            key,
            field
                .value
                .expanded
                .as_deref()
                .map_or_else(|| field.value.plain_text(), ToOwned::to_owned),
        ),
        RecordValueMode::Value => record.set_item(
            key,
            Py::new(
                py,
                PyValue {
                    inner: field.value.value.clone().into_owned(),
                },
            )?,
        ),
    }
}

#[derive(Debug, Clone, Copy)]
enum RecordValueMode {
    Plain,
    Expanded,
    Value,
}

impl RecordValueMode {
    fn parse(value: &str) -> PyResult<Self> {
        match value {
            "plain" => Ok(Self::Plain),
            "expanded" => Ok(Self::Expanded),
            "value" => Ok(Self::Value),
            _ => Err(PyValueError::new_err(
                "value_mode must be 'plain', 'expanded', or 'value'",
            )),
        }
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
            let text = match field.value.expanded.as_deref() {
                Some(expanded) => latex_to_unicode(expanded)?,
                None => latex_to_unicode(&field.value.value.to_plain_string())?,
            };
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

fn raw_source_slice(source: Option<&Arc<str>>, span: Option<SourceSpan>) -> Option<String> {
    let source = source?;
    let span = span?;
    source
        .get(span.byte_start..span.byte_end)
        .map(ToOwned::to_owned)
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

fn needs_text_projection(text: &str) -> bool {
    text.as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'\n' | b'\r'))
}
