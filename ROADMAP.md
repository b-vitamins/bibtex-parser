# bibtex-parser Roadmap

## Current Shape

`bibtex-parser` is a Rust-first BibTeX library centered on `Library<'a>`.
The default parser is strict and optimized for throughput. Richer behavior such
as tolerant recovery and source spans is opt-in.

Implemented core capabilities:

- strict parsing into `Library`
- ordered block access through `Block`
- string variables, concatenation, month constants, preambles, and comments
- opt-in tolerant parsing with `FailedBlock`
- opt-in source spans
- name parsing, DOI normalization, validation, duplicate checks, and query helpers
- typed transforms for DOI, months, field aliases, and sorting
- configurable writer with `Writer` and `WriterConfig`
- multi-file parallel parsing behind the `parallel` feature

## Near-Term Work

- Keep default parse throughput as the primary performance metric.
- Add more writer golden tests for comments, failed blocks, and formatting options.
- Expand tolerant parsing recovery tests with real malformed corpora.
- Add property tests for parse/write/parse stability.
- Add API examples for transforms and block iteration.
- Revisit single-file parallel parsing only after the sequential parser has no obvious wins left.

## Explicit Non-Goals For The Rust Core

- No Python-style middleware stack.
- No network metadata fetching.
- No full Biber engine.
- No default heavyweight LaTeX parser.
- No compatibility aliases for pre-release API names.
