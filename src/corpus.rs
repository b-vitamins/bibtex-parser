//! Corpus-level parsed bibliography model.

use crate::{
    Diagnostic, ParseEvent, ParseStatus, ParsedDocument, ParsedEntry, ParsedSource, SourceId,
    SourceSpan,
};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Borrowed input source for corpus parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusSource<'a> {
    /// Human-readable source name or path.
    pub name: &'a str,
    /// BibTeX source text.
    pub input: &'a str,
}

/// Corpus-level streaming event.
#[derive(Debug, Clone, PartialEq)]
pub enum CorpusEvent<'a> {
    /// A source is about to be parsed.
    SourceStart(ParsedSource<'a>),
    /// A source-order parse event from one source.
    Event {
        /// Corpus-wide source id.
        source: SourceId,
        /// Parsed source event.
        event: Box<ParseEvent<'a>>,
    },
    /// A source finished parsing or was stopped.
    SourceEnd(ParsedSource<'a>),
}

impl<'a> CorpusSource<'a> {
    /// Create a corpus source from a name and input string.
    #[must_use]
    pub const fn new(name: &'a str, input: &'a str) -> Self {
        Self { name, input }
    }
}

/// Parsed multi-source bibliography corpus.
#[derive(Debug, Clone)]
pub struct ParsedCorpus<'a> {
    documents: Vec<ParsedDocument<'a>>,
    sources: Vec<ParsedSource<'a>>,
    duplicate_keys: Vec<DuplicateKeyGroup>,
    status: ParseStatus,
}

impl<'a> ParsedCorpus<'a> {
    pub(crate) fn from_documents(documents: Vec<ParsedDocument<'a>>) -> Self {
        let sources = documents
            .iter()
            .flat_map(|document| document.sources().iter().cloned())
            .collect::<Vec<_>>();
        let duplicate_keys = find_duplicate_keys(&documents);
        let status = corpus_status(&documents);

        Self {
            documents,
            sources,
            duplicate_keys,
            status,
        }
    }

    /// Return parsed documents in corpus input order.
    #[must_use]
    pub fn documents(&self) -> &[ParsedDocument<'a>] {
        &self.documents
    }

    /// Return corpus sources in input order.
    #[must_use]
    pub fn sources(&self) -> &[ParsedSource<'a>] {
        &self.sources
    }

    /// Return a source by corpus-wide source id.
    #[must_use]
    pub fn source(&self, id: SourceId) -> Option<&ParsedSource<'a>> {
        self.sources.iter().find(|source| source.id == id)
    }

    /// Iterate entries across all documents in corpus order.
    pub fn entries(&self) -> impl Iterator<Item = &ParsedEntry<'a>> + '_ {
        self.documents
            .iter()
            .flat_map(|document| document.entries().iter())
    }

    /// Iterate diagnostics across all documents in corpus order.
    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> + '_ {
        self.documents
            .iter()
            .flat_map(|document| document.diagnostics().iter())
    }

    /// Return duplicate citation key groups.
    #[must_use]
    pub fn duplicate_keys(&self) -> &[DuplicateKeyGroup] {
        &self.duplicate_keys
    }

    /// Return aggregate corpus parse status.
    #[must_use]
    pub const fn status(&self) -> ParseStatus {
        self.status
    }
}

/// Duplicate citation key group with source provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateKeyGroup {
    /// Citation key text.
    pub key: String,
    /// Occurrences in corpus order.
    pub occurrences: Vec<DuplicateKeyOccurrence>,
    /// `true` when occurrences come from more than one source.
    pub cross_source: bool,
}

impl DuplicateKeyGroup {
    /// Return `true` when every occurrence is in the same source.
    #[must_use]
    pub const fn is_same_source(&self) -> bool {
        !self.cross_source
    }
}

/// One duplicate citation key occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateKeyOccurrence {
    /// Corpus-wide source id.
    pub source: SourceId,
    /// Source name or path, when available.
    pub source_name: Option<String>,
    /// Document index inside the corpus.
    pub document_index: usize,
    /// Entry index inside that parsed document.
    pub entry_index: usize,
    /// Key token location, when available.
    pub key_source: Option<SourceSpan>,
}

fn find_duplicate_keys(documents: &[ParsedDocument<'_>]) -> Vec<DuplicateKeyGroup> {
    let mut groups: BTreeMap<String, Vec<DuplicateKeyOccurrence>> = BTreeMap::new();

    for (document_index, document) in documents.iter().enumerate() {
        for (entry_index, entry) in document.entries().iter().enumerate() {
            let source = entry
                .source
                .and_then(|span| span.source)
                .unwrap_or_else(|| SourceId::new(document_index));
            let source_name = document
                .sources()
                .iter()
                .find(|parsed_source| parsed_source.id == source)
                .and_then(|parsed_source| parsed_source.name.as_ref())
                .map(Cow::as_ref)
                .map(ToOwned::to_owned);

            groups
                .entry(entry.key().to_string())
                .or_default()
                .push(DuplicateKeyOccurrence {
                    source,
                    source_name,
                    document_index,
                    entry_index,
                    key_source: entry.key_source,
                });
        }
    }

    groups
        .into_iter()
        .filter_map(|(key, occurrences)| {
            if occurrences.len() < 2 {
                return None;
            }
            let first_source = occurrences[0].source;
            let cross_source = occurrences
                .iter()
                .any(|occurrence| occurrence.source != first_source);
            Some(DuplicateKeyGroup {
                key,
                occurrences,
                cross_source,
            })
        })
        .collect()
}

fn corpus_status(documents: &[ParsedDocument<'_>]) -> ParseStatus {
    let has_content = documents.iter().any(|document| {
        !document.entries().is_empty()
            || !document.strings().is_empty()
            || !document.preambles().is_empty()
    });
    let has_problem = documents
        .iter()
        .any(|document| document.status() != ParseStatus::Ok);

    if !has_problem {
        ParseStatus::Ok
    } else if has_content {
        ParseStatus::Partial
    } else {
        ParseStatus::Failed
    }
}
