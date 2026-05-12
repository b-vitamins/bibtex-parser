# Release Checklist

Run these gates before cutting a release:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --all-features -- --ignored
cargo bench --all-features --no-run
cargo bench --bench performance --all-features
```

The default test suite covers strict parsing, tolerant recovery, diagnostics,
source locations, raw preservation, value views, low-churn writing, editing
helpers, semantic helpers, streaming, multi-file corpus parsing, and
parse-write-parse safety.

The ignored tests in `tests/tooling_regression_corpus.rs` are release-scale
stress checks for hundred-thousand-entry collection and million-entry streaming
workloads. They are intentionally excluded from routine test runs.

Criterion reports release-critical modes separately:

- `throughput/bibtex-parser`
- `throughput/bibtex-parser-tolerant`
- `throughput/bibtex-parser-source-preserving`
- `throughput/bibtex-parser-streaming`
- `writing/library_to_string`
- `writing/raw_document_to_string`
- `corpus/parse_sources`
