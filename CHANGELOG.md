# Changelog

All notable changes to this project are documented here.

## Unreleased

### Added

- Canonical `Library<'a>` API with `Library::parse`, `Library::parser`, and top-level `parse`.
- `Parser` options for opt-in tolerant parsing and source-span capture.
- Ordered high-level block access through `Block`, including entries, string definitions, preambles, comments, and tolerant parse failures.
- `StringDefinition`, `Preamble`, `Comment`, and `FailedBlock` types.
- `SourceSpan` for byte and line/column locations.
- Entry editing helpers: `set`, `set_literal`, `remove`, and `rename_field`.
- Typed entry helpers for title, year, date, journal, booktitle, URL, and keywords.
- Typed library transforms for DOI normalization, month normalization, field alias normalization, and sorting.
- `Library::to_bibtex` and `Library::write_file`.

### Changed

- Replaced the pre-release `Database`/`ParseOptions` naming with `Library`/`Parser`.
- Writer API now exposes `Writer`, `WriterConfig`, and `Writer::write_library` from the crate root.
- Writer preserves block order by default and uses sorted grouped output only when sorting is requested.
- `strings()`, `preambles()`, and `comments()` now return typed block records instead of raw storage internals.
- Removed the stale `profile` binary target from `Cargo.toml`.

### Performance

- Default strict parsing keeps the optimized sequential path.
- Tolerant parsing and source-span capture use richer paths only when explicitly requested.
