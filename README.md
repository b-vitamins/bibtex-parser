# bibtex-parser

[![Crates.io](https://img.shields.io/crates/v/bibtex-parser.svg)](https://crates.io/crates/bibtex-parser)
[![docs.rs](https://img.shields.io/docsrs/bibtex-parser.svg)](https://docs.rs/bibtex-parser)
[![License](https://img.shields.io/crates/l/bibtex-parser.svg)](LICENSE)

Fast BibTeX parsing for Rust applications that need both throughput and a real user-facing API.

`bibtex-parser` parses strict BibTeX at high speed, expands string variables and month constants, exposes top-level block order, provides ergonomic query/edit helpers, and writes BibTeX back out with configurable formatting.

## Features

- High-throughput single-file parsing with borrowed values where possible.
- Strict parsing by default, with explicit tolerant recovery for malformed corpora.
- String definitions, concatenation, month constants, preambles, comments, and ordered blocks.
- Source-span capture for entries and recovered failures when requested.
- Case-insensitive lookup, duplicate detection, DOI normalization, field normalization, sorting, and validation.
- Structured author/editor name parsing and typed entry helpers.
- Configurable writer for serializing, formatting, sorting, and writing files.
- Optional `parallel` feature for parsing multiple files concurrently.
- Optional `latex_to_unicode` feature for LaTeX accent conversion helpers.

## Install

```toml
[dependencies]
bibtex-parser = "0.1"
```

## Parse

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

## Query And Edit

```rust
use bibtex_parser::{Library, Result};

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

    let entry = &library.entries()[0];
    assert_eq!(entry.doi(), Some("10.1000/xyz".to_string()));
    assert_eq!(entry.get("note"), Some("accepted"));
    assert_eq!(entry.get("tags"), Some("rust; parsing, bibtex"));
    Ok(())
}
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

## Streaming

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

## Tolerant Parsing

Strict parsing is the default. Tolerant parsing is opt-in and keeps malformed blocks separate from valid entries.

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

## Write

```rust
use bibtex_parser::{Library, Result, Writer, WriterConfig};

fn main() -> Result<()> {
    let library = Library::parse(r#"@article{paper, title = "Fast BibTeX"}"#)?;

    let config = WriterConfig {
        indent: "  ".to_string(),
        align_values: true,
        sort_entries: true,
        ..Default::default()
    };

    let mut output = Vec::new();
    let mut writer = Writer::with_config(&mut output, config);
    writer.write_library(&library)?;

    Ok(())
}
```

For simple cases:

```rust
let bibtex = library.to_bibtex()?;
library.write_file("references.bib")?;
```

## Feature Flags

```toml
[dependencies]
bibtex-parser = { version = "0.1", features = ["parallel", "latex_to_unicode"] }
```

- `parallel`: enables Rayon-backed `Parser::parse_files` for multi-file workloads. Single-file parsing remains sequential.
- `latex_to_unicode`: enables LaTeX accent-to-Unicode conversion helpers.

## Semantics

- `Library::parse` and `Parser::parse` are strict by default and return an error on malformed BibTeX.
- `Parser::tolerant()` recovers valid blocks after malformed input and records failures in `Library::failed_blocks()`.
- String definitions and concatenations are expanded for the `Library` API. Use `parse_bibtex` when you need raw parsed items.
- Comments, preambles, strings, entries, and tolerant failures are available through `Library::blocks()` in source order.
- Writer defaults preserve library block order. Sorting and alignment are explicit formatting choices.

## Performance

The repository includes Criterion benchmarks for parser throughput, common library operations, and memory-oriented workloads. Exact numbers depend on CPU, compiler, governor, and thermal state, so measure on the machine that matters for your workload.

```sh
cargo bench --bench performance -- throughput/bibtex-parser
```

## License

Licensed under either of Apache-2.0 or MIT, at your option.
