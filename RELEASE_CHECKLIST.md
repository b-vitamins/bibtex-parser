# Release Checklist

Run these gates before cutting a release:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --all-features -- --ignored
cargo bench --all-features --no-run
cargo bench --bench performance --all-features
guix shell -m manifest.scm -- maturin build --release --out target/wheels
rm -rf target/python-test
python3 - <<'PY'
from pathlib import Path
from zipfile import ZipFile

wheel = sorted(Path("target/wheels").glob("bibtex_parser-*.whl"))[-1]
target = Path("target/python-test")
target.mkdir(parents=True, exist_ok=True)
with ZipFile(wheel) as archive:
    archive.extractall(target)
PY
guix shell -m manifest.scm -- env PYTHONPATH=target/python-test python3 -m pytest tests/python
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

Python benchmark results are recorded in `PYTHON.md`.
