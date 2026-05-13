# bibtex-parser

[![CI](https://github.com/b-vitamins/bibtex-parser/actions/workflows/ci.yml/badge.svg)](https://github.com/b-vitamins/bibtex-parser/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/bibtex-parser.svg)](https://crates.io/crates/bibtex-parser)
[![docs.rs](https://img.shields.io/docsrs/bibtex-parser.svg)](https://docs.rs/bibtex-parser)
[![PyPI](https://img.shields.io/pypi/v/citerra.svg)](https://pypi.org/project/citerra/)
[![License](https://img.shields.io/crates/l/bibtex-parser.svg)](LICENSE)

BibTeX parsing for Rust and Python applications.

`bibtex-parser` provides a Rust parser and library API, plus a Python package
built with PyO3 and maturin. It supports strict parsing, explicit tolerant
recovery, source locations, raw-text preservation, semantic helpers, editing
primitives, streaming, multi-file corpus parsing, and configurable
serialization.

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
- `ParsedDocument` is the Rust model for tooling that needs diagnostics,
  source-order blocks, raw source text, partial entries, failed blocks, and
  source locations.
- `citerra.Document` is the Python document model. It exposes source-order
  blocks, diagnostics, raw text, editing operations, and writing through Python
  classes and functions.
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
            title = "Example Paper",
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

Use tolerant mode when a corpus may contain malformed entries but
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

For diagnostics and source-preserving output, parse into `ParsedDocument`:

```rust
use bibtex_parser::{ParseStatus, Parser, Result};

fn main() -> Result<()> {
    let document = Parser::new()
        .tolerant()
        .capture_source()
        .preserve_raw()
        .expand_values()
        .parse_source("refs/main.bib", r#"
            @article{paper, title = "Example Paper"}
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
            title = "Example Paper",
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
    '@article{paper, author = "Jane Doe", title = "Example Paper", year = 2026}',
    expand_values=True,
)

entry = document.entry("paper")
assert entry is not None
assert entry.entry_type == "article"
assert entry.get("title") == "Example Paper"
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
- `Library` expands string definitions and month constants for field access.
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

## Benchmarks And Comparisons

The repository includes Criterion benchmarks for parser throughput, tolerant
parsing, source-preserving parsing, streaming, writing, corpus parsing, common
library operations, and memory-oriented workloads. The tables below are one run
on `tests/fixtures/tugboat.bib`:

- 2,701,551 bytes, 73,993 lines, 3,644 entries.
- AMD Ryzen 5 5600G, 6 cores / 12 threads.
- Rust `1.93.0`, Python `3.11.14`.
- Measured on 2026-05-13. Throughput is input-size normalized.

Reproduce the Rust parser table:

```sh
cargo bench --bench performance --all-features -- --noplot throughput
```

| Rust parser / mode | Version | Median time | Throughput | Notes |
| --- | ---: | ---: | ---: | --- |
| `serde_bibtex` ignore | 0.7.1 | 2.411 ms | 1.04 GiB/s | Parses and discards data |
| `serde_bibtex` selected struct | 0.7.1 | 4.143 ms | 621.9 MiB/s | Deserializes selected fields |
| `bibtex-parser` strict `Library` | 0.2.0 | 5.234 ms | 492.3 MiB/s | Entries, fields, strings, comments, preambles |
| `serde_bibtex` borrowed entries | 0.7.1 | 6.872 ms | 374.9 MiB/s | Borrowed parsed entries |
| `bibtex-parser` tolerant `Library` | 0.2.0 | 7.015 ms | 367.3 MiB/s | Recovery and failed-block tracking |
| `biblatex` raw bibliography | 0.11.0 | 10.839 ms | 237.7 MiB/s | Raw BibLaTeX bibliography |
| `serde_bibtex` owned entries | 0.7.1 | 13.469 ms | 191.3 MiB/s | Owned entries with month macros |
| `bibtex-parser` streaming events | 0.2.0 | 21.943 ms | 117.4 MiB/s | Source-order callback events |
| `bibtex-parser` source-preserving document | 0.2.0 | 31.862 ms | 80.9 MiB/s | Raw text, source locations, diagnostics model |
| `nom-bibtex` | 0.6.0 | 34.334 ms | 75.0 MiB/s | Parsed bibliography |

Reproduce the Rust writing table:

```sh
cargo bench --bench performance --all-features -- --noplot writing
```

| Rust writer mode | Version | Median time | Throughput |
| --- | ---: | ---: | ---: |
| Raw-preserving document write | 0.2.0 | 1.816 ms | 1.39 GiB/s |
| Normalized `Library` write | 0.2.0 | 5.325 ms | 483.8 MiB/s |

The Python comparison used the local `citerra` wheel plus `bibtexparser` 1.4.4,
`bibtexparser` 2.0.0b9, and `pybtex` 0.26.1. The comparison script uses
whichever optional packages are installed in the active environment:

```sh
python python/benchmarks/compare_parsers.py tests/fixtures/tugboat.bib
python python/benchmarks/compare_parsers.py tests/fixtures/tugboat.bib --write
```

| Python parser / mode | Version | Median parse time | Throughput | Relative time |
| --- | ---: | ---: | ---: | ---: |
| `citerra` structured parse | 0.2.0 | 0.058 s | 44.3 MiB/s | 1.0x |
| `citerra` source-preserving parse | 0.2.0 | 0.065 s | 39.9 MiB/s | 1.1x |
| `bibtexparser` parse | 2.0.0b9 | 0.372 s | 6.9 MiB/s | 6.4x |
| `pybtex` parse | 0.26.1 | 0.859 s | 3.0 MiB/s | 14.8x |
| `bibtexparser` parse | 1.4.4 | 10.483 s | 0.2 MiB/s | 180.1x |

| Python writer / mode | Version | Median write time | Throughput | Relative time |
| --- | ---: | ---: | ---: | ---: |
| `citerra` raw-preserving write | 0.2.0 | 0.003 s | 953.2 MiB/s | 1.0x |
| `citerra` normalized write | 0.2.0 | 0.014 s | 181.3 MiB/s | 5.3x |
| `bibtexparser` write | 1.4.4 | 0.106 s | 24.3 MiB/s | 39.2x |
| `bibtexparser` write | 2.0.0b9 | 0.493 s | 5.2 MiB/s | 182.2x |
| `pybtex` write | 0.26.1 | 3.942 s | 0.7 MiB/s | 1458.5x |

## License

Licensed under either of Apache-2.0 or MIT, at your option.
