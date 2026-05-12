# Python API

`bibtex-parser` exposes the native Rust document model to Python through PyO3
and maturin. The package name is `bibtex_parser`.

## Build

Use the project manifest for local development:

```sh
guix shell -m manifest.scm -- maturin build --release --out target/wheels
```

For local tests without installing through pip, unpack the built wheel into a
temporary import directory and run pytest with that directory on `PYTHONPATH`:

```sh
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

## Parse And Inspect

```python
import bibtex_parser

document = bibtex_parser.parse(
    '@article{paper, author = "Jane Doe", title = "Fast BibTeX", year = 2026}',
    expand_values=True,
)

entry = document.entry("paper")
assert entry.entry_type == "article"
assert entry.get("title") == "Fast BibTeX"
assert entry.date_parts().year == 2026
```

## Tolerant Parsing

```python
document = bibtex_parser.parse(text, tolerant=True)

if document.status != "ok":
    for diagnostic in document.diagnostics:
        print(diagnostic.code, diagnostic.message, diagnostic.source)
```

## Mutate And Write

```python
document.rename_key("paper", "paper-v2")
document.set_field("paper-v2", "note", "accepted")
document.remove_export_fields(["abstract", "keywords"])

output = document.write()
```

Use `WriterConfig(preserve_raw=True)` for low-churn source-preserving output and
`WriterConfig(preserve_raw=False)` for normalized structured output.

## Helpers

```python
assert bibtex_parser.normalize_doi("https://doi.org/10.1000/XYZ.") == "10.1000/xyz"
assert bibtex_parser.latex_to_unicode("Jos\\'e") == "José"

names = bibtex_parser.parse_names("Jane Doe and {Research Group}")
assert names[1].literal == "Research Group"
```

## Benchmark

```sh
PYTHONPATH=target/python-test \
  python3 python/benchmarks/benchmark_parser.py tests/fixtures/tugboat.bib --iterations 20
```

Recorded on this workspace for the default source-preserving Python parse mode:

```text
path=tests/fixtures/tugboat.bib
bytes=2701551
iterations=20
median_gb_s=0.060
best_gb_s=0.062
```
