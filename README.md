# bibtex-parser

[![CI](https://github.com/b-vitamins/bibtex-parser/actions/workflows/ci.yml/badge.svg)](https://github.com/b-vitamins/bibtex-parser/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/bibtex-parser.svg)](https://crates.io/crates/bibtex-parser)
[![docs.rs](https://img.shields.io/docsrs/bibtex-parser.svg)](https://docs.rs/bibtex-parser)
[![PyPI](https://img.shields.io/pypi/v/citerra.svg)](https://pypi.org/project/citerra/)
[![License](https://img.shields.io/crates/l/bibtex-parser.svg)](LICENSE)

Fast BibTeX parsing for Rust and Python applications that need throughput,
structured data, source-aware tooling, and predictable writing.

`bibtex-parser` provides a Rust-first parser and library API, plus a native
Python package built with PyO3 and maturin. It supports strict parsing,
explicit tolerant recovery, source locations, raw-text preservation, semantic
helpers, editing primitives, streaming, multi-file corpus parsing, and
configurable serialization.

## Install

Rust:

```toml
[dependencies]
bibtex-parser = "0.2"
```

Python:

```sh
pip install citerra
```

The Python distribution and import name are both `citerra`:

```python
import citerra
```

## Core Concepts

- `Library` is the compact Rust bibliography collection API for application
  code that wants entries, fields, strings, comments, preambles, validation,
  transforms, and writing.
- `ParsedDocument` is the richer Rust model for tooling that needs diagnostics,
  source-order blocks, raw source text, partial entries, failed blocks, and
  source locations.
- `citerra.Document` is the Python-native document model. It exposes the
  same tooling-oriented capabilities through Python classes and functions.
- `Parser` configures strict versus tolerant parsing, source capture, raw-text
  preservation, expanded values, streaming, and multi-source parsing.

Strict parsing is the default. Tolerant parsing is explicit.

## Rust Quick Start

```rust
use bibtex_parser::{Library, Result};

fn main() -> Result<()> {
    let input = r#"
        @string{venue = "VLDB"}
        @article{paper,
            author = "Jane Doe and John Smith",
            title = "Fast BibTeX",
            journal = venue,
            year = 2026
        }
    "#;

    let library = Library::parse(input)?;
    let entry = library.find_by_key("paper").unwrap();

    assert_eq!(entry.get("journal"), Some("VLDB"));
    assert_eq!(entry.year(), Some("2026".to_string()));
    assert_eq!(entry.authors().len(), 2);
    Ok(())
}
```

## Rust Tolerant Parsing And Diagnostics

Use tolerant mode when a real-world corpus may contain malformed entries but
valid entries before or after those regions should still be returned.

```rust
use bibtex_parser::{Block, Library, Result};

fn main() -> Result<()> {
    let library = Library::parser()
        .tolerant()
        .capture_source()
        .parse(r#"
            @article{ok, title = "Good"}
            @article{bad, title = "Missing close"
            @book{recovered, title = "Recovered"}
        "#)?;

    assert_eq!(library.entries().len(), 2);
    assert_eq!(library.failed_blocks().len(), 1);

    for block in library.blocks() {
        if let Block::Failed(failed) = block {
            eprintln!("bad block at {:?}", failed.source);
        }
    }

    Ok(())
}
```

For richer diagnostics and source-preserving output, parse into
`ParsedDocument`:

```rust
use bibtex_parser::{ParseStatus, Parser, Result};

fn main() -> Result<()> {
    let document = Parser::new()
        .tolerant()
        .capture_source()
        .preserve_raw()
        .expand_values()
        .parse_source("refs/main.bib", r#"
            @article{paper, title = "Fast BibTeX"}
            @article{broken, title = "Missing close"
        "#)?;

    assert_eq!(document.status(), ParseStatus::Partial);
    assert_eq!(document.entries()[0].key(), "paper");

    for diagnostic in document.diagnostics() {
        eprintln!("{}: {}", diagnostic.code, diagnostic.message);
    }

    Ok(())
}
```

## Rust Query, Edit, And Write

```rust
use bibtex_parser::{Library, Result, Writer, WriterConfig};

fn main() -> Result<()> {
    let mut library = Library::parse(r#"
        @article{paper,
            title = "Fast BibTeX",
            doi = "https://doi.org/10.1000/XYZ.",
            keywords = "rust; parsing, bibtex"
        }
    "#)?;

    let entry = &mut library.entries_mut()[0];
    entry.set_literal("note", "accepted");
    entry.rename_field("keywords", "tags");

    library.normalize_doi_fields();

    let config = WriterConfig {
        indent: "  ".to_string(),
        align_values: true,
        sort_fields: true,
        ..Default::default()
    };

    let mut output = Vec::new();
    Writer::with_config(&mut output, config).write_library(&library)?;

    let entry = &library.entries()[0];
    assert_eq!(entry.doi(), Some("10.1000/xyz".to_string()));
    assert_eq!(entry.get("note"), Some("accepted"));
    assert_eq!(entry.get("tags"), Some("rust; parsing, bibtex"));
    Ok(())
}
```

For simple serialization:

```rust
let bibtex = library.to_bibtex()?;
library.write_file("references.bib")?;
```

## Rust Semantic Helpers

```rust
use bibtex_parser::{Library, ResourceKind, Result};

fn main() -> Result<()> {
    let library = Library::parse(r#"
        @article{paper,
            author = "Jane Doe and {Research Group}",
            date = "2026-05-13",
            doi = "https://doi.org/10.1000/XYZ."
        }
    "#)?;

    let entry = &library.entries()[0];
    assert_eq!(entry.authors()[1].literal.as_deref(), Some("Research Group"));
    assert_eq!(entry.date_parts().unwrap().unwrap().month, Some(5));
    assert_eq!(entry.resource_fields()[0].kind, ResourceKind::Doi);
    assert_eq!(entry.doi(), Some("10.1000/xyz".to_string()));
    Ok(())
}
```

Helpers include name parsing, date extraction, DOI normalization, field
normalization, resource classification, duplicate-key detection, and validation.

## Rust Streaming And Multi-File Parsing

Streaming lets callers process source-order events without building a full
intermediate collection:

```rust
use bibtex_parser::{ParseEvent, ParseFlow, Parser, Result};

fn main() -> Result<()> {
    let mut entries = 0;
    let summary = Parser::new().parse_events("@article{paper, title = \"A\"}", |event| {
        if let ParseEvent::Entry(entry) = event {
            assert_eq!(entry.key(), "paper");
            entries += 1;
        }
        Ok(ParseFlow::Continue)
    })?;

    assert_eq!(entries, 1);
    assert_eq!(summary.entries, 1);
    Ok(())
}
```

Multi-source parsing keeps source identity and can detect duplicate keys across
files:

```rust
use bibtex_parser::{CorpusSource, Parser, Result, SourceId};

fn main() -> Result<()> {
    let sources = [
        CorpusSource::new("refs/a.bib", "@article{paper, title = \"A\"}"),
        CorpusSource::new("refs/b.bib", "@article{paper, title = \"B\"}"),
    ];

    let corpus = Parser::new().parse_sources(&sources)?;
    let duplicates = corpus.duplicate_keys();

    assert_eq!(corpus.entries().count(), 2);
    assert_eq!(duplicates[0].key, "paper");
    assert!(duplicates[0].cross_source);
    assert_eq!(duplicates[0].occurrences[1].source, SourceId::new(1));
    Ok(())
}
```

Enable the `parallel` feature for Rayon-backed parsing of multiple files from
disk with `Parser::parse_files`.

## Python Quick Start

```python
import citerra

document = citerra.parse(
    '@article{paper, author = "Jane Doe", title = "Fast BibTeX", year = 2026}',
    expand_values=True,
)

entry = document.entry("paper")
assert entry is not None
assert entry.entry_type == "article"
assert entry.get("title") == "Fast BibTeX"
assert entry.date_parts().year == 2026
```

`load`, `loads`, `dump`, and `dumps` are available for file-like workflows:

```python
from pathlib import Path
import citerra

document = citerra.parse_path("references.bib", tolerant=True)
Path("normalized.bib").write_text(citerra.dumps(document), encoding="utf-8")
```

## Python Diagnostics, Source, And Raw Text

```python
import citerra

document = citerra.parse(
    text,
    tolerant=True,
    capture_source=True,
    preserve_raw=True,
    source="refs/main.bib",
)

if document.status != "ok":
    for diagnostic in document.diagnostics:
        location = diagnostic.source
        if location is not None:
            print(diagnostic.code, location.line, location.column, diagnostic.message)
        else:
            print(diagnostic.code, diagnostic.message)

entry = document.entry("paper")
if entry is not None:
    print(entry.raw)
    print(entry.field("title").raw_value)
```

## Python Mutation And Writing

```python
import citerra

document = citerra.parse_path("references.bib", tolerant=True)

document.rename_key("paper", "paper-v2")
document.set_field("paper-v2", "note", "accepted")
document.remove_export_fields(["abstract", "keywords"])

config = citerra.WriterConfig(
    preserve_raw=True,
    trailing_comma=True,
)

output = document.write(config)
```

Use `preserve_raw=True` for low-churn source-preserving writes. Use
`preserve_raw=False` when normalized formatting is desired.

## Python Plain Records

Some application code wants ordinary dictionaries for filtering, indexing, or
bulk transforms. The Python package provides explicit helpers for that shape
without changing the native document model:

```python
import citerra

document = citerra.parse_path("references.bib")
records = citerra.document_to_dicts(document)

selected = [record for record in records if record.get("year") == "2026"]
text = citerra.write_entries(
    selected,
    field_order=["author", "title", "journal", "year", "doi"],
    sort_by=["ID"],
    trailing_comma=True,
)
```

## Python Helpers

```python
import citerra

assert citerra.normalize_doi("https://doi.org/10.1000/XYZ.") == "10.1000/xyz"
assert citerra.latex_to_unicode("Jos\\'e") == "José"

names = citerra.parse_names("Jane Doe and {Research Group}")
assert names[1].literal == "Research Group"

date = citerra.parse_date("2026-05-13")
assert (date.year, date.month, date.day) == (2026, 5, 13)
```

## Feature Flags

```toml
[dependencies]
bibtex-parser = { version = "0.2", features = ["parallel", "latex_to_unicode"] }
```

- `parallel`: enables Rayon-backed `Parser::parse_files` for multi-file workloads.
- `latex_to_unicode`: enables LaTeX accent-to-Unicode conversion helpers.
- `python-extension`: builds the PyO3 extension module used by the Python package.

## Semantics

- `Library::parse` and `Parser::parse` are strict by default.
- `Parser::tolerant()` recovers valid blocks after malformed input and records
  failures and diagnostics.
- `Library` expands string definitions and month constants for ergonomic field
  access.
- `ParsedDocument` can retain raw entry, field, value, comment, preamble, string,
  and failed-block text when `preserve_raw()` is enabled.
- Source columns are 1-based Unicode scalar columns. Byte spans are also exposed
  for exact source slicing.
- Writer defaults preserve source order. Sorting, alignment, trailing commas,
  and normalized output are explicit choices.

## Local Development

This repository uses a Guix manifest for local development:

```sh
guix shell -m manifest.scm --
```

Common gates:

```sh
cargo fmt --all -- --check
guix shell -m manifest.scm -- actionlint
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo bench --all-features --no-run
guix shell -m manifest.scm -- maturin build --release --out target/wheels
```

Python wheel smoke test without installing into the user environment:

```sh
rm -rf target/python-test
python3 - <<'PY'
from pathlib import Path
from zipfile import ZipFile

wheel = sorted(Path("target/wheels").glob("citerra-*.whl"))[-1]
target = Path("target/python-test")
target.mkdir(parents=True, exist_ok=True)
with ZipFile(wheel) as archive:
    archive.extractall(target)
PY
guix shell -m manifest.scm -- env PYTHONPATH=target/python-test python3 -m pytest tests/python
```

## Release Process

The release workflow builds the Rust crate, Python source distribution, and
ABI3 wheels for Linux, macOS, and Windows. Tagging `vX.Y.Z` runs release
validation, creates a GitHub release, publishes to crates.io, and publishes to
PyPI when the required repository environments and secrets are configured.

See [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) for the exact release gates
and setup requirements.

## Performance

The repository includes Criterion benchmarks for parser throughput, tolerant
parsing, source-preserving parsing, streaming, writing, corpus parsing, common
library operations, and memory-oriented workloads. Exact numbers depend on CPU,
compiler, governor, and thermal state, so measure on the machine that matters
for your workload.

```sh
cargo bench --bench performance -- throughput/bibtex-parser
```

## License

Licensed under either of Apache-2.0 or MIT, at your option.
