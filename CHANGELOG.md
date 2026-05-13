# Changelog

All notable changes to this project are documented here.

## Unreleased

## 0.2.3 - 2026-05-13

### Changed

- Made `citerra` the GitHub and PyPI-facing README while keeping `RUST.md` as
  the crates.io README for the `bibtex-parser` Rust crate.
- Removed stale repository-only files and renamed the release checklist to
  `RELEASE.md`.
- Simplified benchmark presentation so non-equivalent throughput baselines and
  writer timings are described separately from parser-output comparison tables.
- Added GitHub Linguist attributes so BibTeX fixtures do not dominate the
  repository language statistics.
- Updated release publishing to keep protected environment checks without
  creating GitHub deployment records for package publication jobs.

## 0.2.2 - 2026-05-13

### Changed

- Split Rust and Python package documentation so crates.io presents
  `bibtex-parser` and PyPI presents `citerra`.
- Updated package descriptions and Python package metadata to describe each
  package as a parser in its own language ecosystem.

## 0.2.1 - 2026-05-13

### Fixed

- Restored compatibility with the advertised Rust 1.75 minimum supported Rust
  version in value expansion.
- Made a parallel parser unit test use the platform temporary directory instead
  of a Unix-only path.
- Updated CI workflow linting to use a maintained action reference.
- Kept cross-platform Rust test jobs on Rust-facing features; Python extension
  linking remains covered by the wheel jobs.

### Changed

- Added repository agent instructions covering SemVer, Conventional Commits,
  pre-commit checks, release gates, and package naming.
- Added a local pre-commit configuration and Guix manifest support for it.

## 0.2.0 - 2026-05-13

### Added

- Canonical `Library<'a>` API with `Library::parse`, `Library::parser`, and top-level `parse`.
- `Parser` options for opt-in tolerant parsing and source-span capture.
- Tooling-oriented `ParsedDocument` model with source-order blocks, diagnostics, partial entries, raw text, and source-preserving metadata.
- Ordered high-level block access through `Block`, including entries, string definitions, preambles, comments, and tolerant parse failures.
- `StringDefinition`, `Preamble`, `Comment`, and `FailedBlock` types.
- `SourceSpan` for byte and line/column locations.
- Entry editing helpers: `set`, `set_literal`, `remove`, and `rename_field`.
- Typed entry helpers for title, year, date, journal, booktitle, URL, and keywords.
- Typed library transforms for DOI normalization, month normalization, field alias normalization, and sorting.
- `Library::to_bibtex` and `Library::write_file`.
- Citerra Python package built with PyO3 and maturin.
- Python document, entry, field, diagnostic, writer, value, name, date, and helper APIs.
- CI workflow for Rust checks, cross-platform tests, MSRV checks, and Python wheel smoke tests.
- Release workflow for GitHub releases, crates.io publication, PyPI publication, Python source distributions, and ABI3 wheels.

### Changed

- Standardized the public parsing surface around `Library` and `Parser`.
- Writer API now exposes `Writer`, `WriterConfig`, and `Writer::write_library` from the crate root.
- Writer preserves block order by default and uses sorted grouped output only when sorting is requested.
- `strings()`, `preambles()`, and `comments()` now return typed block records instead of raw storage internals.
- Removed the stale `profile` binary target from `Cargo.toml`.
- Renamed the implementation module behind `Library` to match the public API vocabulary.
- Expanded README coverage for Rust and Python usage.

### Performance

- Default strict parsing keeps the optimized sequential path.
- Tolerant parsing and source-span capture use separate paths only when explicitly requested.
- Python bindings use the Rust parser core through a native extension.
