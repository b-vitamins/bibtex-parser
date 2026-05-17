# bibtex-parser

[![CI](https://github.com/b-vitamins/citerra/actions/workflows/ci.yml/badge.svg)](https://github.com/b-vitamins/citerra/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/bibtex-parser.svg)](https://crates.io/crates/bibtex-parser)
[![docs.rs](https://img.shields.io/docsrs/bibtex-parser.svg)](https://docs.rs/bibtex-parser)
[![License](https://img.shields.io/crates/l/bibtex-parser.svg)](LICENSE)

BibTeX parser for Rust.

`bibtex-parser` parses BibTeX into structured Rust types. It supports strict
parsing by default, opt-in tolerant recovery, source locations, raw-text
retention, semantic helpers, editing primitives, streaming, multi-file corpus
parsing, and configurable serialization.

## Performance Snapshot

Measured on `tests/fixtures/tugboat.bib`: 2,701,551 bytes, 73,993 lines, and
3,644 entries. Hardware was AMD Ryzen 5 5600G, 6 cores / 12 threads. Measured
on 2026-05-14 with Rust `1.93.0`; throughput is input-size normalized.

The table compares parser modes that return reusable bibliography data.
Throughput-only baselines that intentionally do less work are noted below the
table rather than listed as peers.

| Rust parser / mode | Version | Median time | Throughput | Output retained |
| --- | ---: | ---: | ---: | --- |
| `bibtex-parser` strict `Library` | 0.3.0 | 3.424 ms | 752.4 MiB/s | Entries, fields, strings, comments, preambles |
| `bibtex-parser` tolerant `Library` | 0.3.0 | 4.166 ms | 618.4 MiB/s | Recovery and failed-block tracking |
| `serde_bibtex` borrowed entries | 0.7.1 | 7.011 ms | 367.5 MiB/s | Borrowed entries |
| `serde_bibtex` owned entries | 0.7.1 | 8.718 ms | 295.5 MiB/s | Owned entries with month macros |
| `biblatex` raw bibliography | 0.11.0 | 9.529 ms | 270.4 MiB/s | Raw BibLaTeX bibliography |
| `bibtex-parser` streaming events | 0.3.0 | 15.596 ms | 165.2 MiB/s | Source-order callback events |
| `bibtex-parser` source-preserving document | 0.3.0 | 22.944 ms | 112.3 MiB/s | Raw text, source locations, diagnostics model |
| `nom-bibtex` | 0.6.0 | 25.297 ms | 101.9 MiB/s | Parsed bibliography |

Two narrower `serde_bibtex` baselines were also measured: parse-and-discard at
1.03 GiB/s and selected-field deserialization at 635.1 MiB/s. Those rows are
useful throughput baselines, but they do not return the same document surface
as a full parser mode.

The writer benchmark measured raw-preserving document output at 1.49 GiB/s and
normalized `Library` output at 423.2 MiB/s.

Reproduction commands are listed in [Reproducing Benchmarks](#reproducing-benchmarks).

## Install

```toml
[dependencies]
bibtex-parser = "0.4"
```

Enable optional functionality as needed:

```toml
[dependencies]
bibtex-parser = { version = "0.4", features = ["parallel", "latex_to_unicode"] }
```

- `parallel`: Rayon-backed parsing for multiple files.
- `latex_to_unicode`: LaTeX accent-to-Unicode conversion helpers.
- `python-extension`: PyO3 extension module used by the `citerra` package.

## Core Types

- `Library` is the compact bibliography collection for application code that
  needs entries, fields, strings, comments, preambles, validation, transforms,
  and writing.
- `ParsedDocument` is the source-preserving document for tooling that needs
  diagnostics, source-order blocks, raw source text, partial entries, failed
  blocks, and source locations.
- `Parser` configures strict versus tolerant parsing, source capture, raw-text
  preservation, expanded values, streaming, and multi-source parsing.

Strict parsing is the default. Tolerant parsing is explicit.

## Parse

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

## Tolerant Parsing

Use tolerant mode when a corpus may contain malformed entries but valid entries
before or after those regions should still be returned.

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

## Diagnostics And Source Locations

Parse into `ParsedDocument` when callers need source-order blocks,
diagnostics, locations, partial entries, or raw text.

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

## Query, Edit, And Write

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

## Semantic Helpers

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

## Streaming And Multi-File Parsing

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

Use `Parser::parse_files` with the `parallel` feature for Rayon-backed parsing
of multiple files from disk.

## Semantics

- `Library::parse` and `Parser::parse` are strict by default.
- `Parser::tolerant()` recovers valid blocks after malformed input and records
  failures and diagnostics.
- `Library` expands string definitions and month constants for field access.
- `Value::from_bibtex_source` parses macro and concatenation-aware value
  fragments for application code that needs to preserve BibTeX value structure.
- `ParsedDocument` can retain raw entry, field, value, comment, preamble, string,
  and failed-block text when `preserve_raw()` is enabled.
- Source columns are 1-based Unicode scalar columns. Byte spans are also exposed
  for exact source slicing.
- Writer defaults preserve source order. Sorting, alignment, trailing commas,
  and normalized output are explicit choices.

## Reproducing Benchmarks

The repository includes Criterion benchmarks for parser throughput, tolerant
parsing, source-preserving parsing, streaming, writing, corpus parsing, common
library operations, and memory-oriented workloads.

Run the parser table:

```sh
guix shell -m manifest.scm -- env CC=gcc cargo bench --bench performance --all-features -- --noplot throughput
```

Run the writing table:

```sh
guix shell -m manifest.scm -- env CC=gcc cargo bench --bench performance --all-features -- --noplot writing
```

Python benchmark notes are in [README.md](README.md).

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
```

Build and test the Python wheel:

```sh
guix shell -m manifest.scm -- maturin build --release --out target/wheels
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

## Python Package

The Python package is `citerra`. See [README.md](README.md).

## Release Process

The release workflow builds the Rust crate, Python source distribution, and
ABI3 wheels for Linux, macOS, and Windows. Tagging `vX.Y.Z` runs release
validation, creates a GitHub release, publishes to crates.io, and publishes to
PyPI when the required repository environments and secrets are configured.

See [RELEASE.md](RELEASE.md) for the exact release gates
and setup requirements.

## License

Licensed under either of Apache-2.0 or MIT, at your option.
