# Python API

The Python package is named `citerra` on PyPI and imported as
`citerra`.

```sh
pip install citerra
```

The package exposes the native Rust document model through PyO3. Wheels are
built as ABI3 extensions for Python 3.8 and newer.

## Parse

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

File-like helpers are available:

```python
with open("references.bib", encoding="utf-8") as handle:
    document = citerra.load(handle, tolerant=True)

text = citerra.dumps(document)
```

## Tolerant Parsing And Diagnostics

```python
document = citerra.parse(
    text,
    tolerant=True,
    capture_source=True,
    preserve_raw=True,
    source="refs/main.bib",
)

for diagnostic in document.diagnostics:
    span = diagnostic.source
    if span is None:
        print(diagnostic.code, diagnostic.message)
    else:
        print(diagnostic.code, span.line, span.column, diagnostic.message)
```

## Mutate And Write

```python
document.rename_key("paper", "paper-v2")
document.set_field("paper-v2", "note", "accepted")
document.remove_export_fields(["abstract", "keywords"])

config = citerra.WriterConfig(
    preserve_raw=True,
    trailing_comma=True,
)
output = document.write(config)
```

Use `WriterConfig(preserve_raw=True)` for low-churn source-preserving output and
`WriterConfig(preserve_raw=False)` for normalized structured output.

## Plain Records

```python
records = citerra.document_to_dicts(document)
records.sort(key=lambda record: record.get("year", ""))

text = citerra.write_entries(
    records,
    field_order=["author", "title", "journal", "year", "doi"],
    sort_by=["ID"],
)
```

## Helpers

```python
assert citerra.normalize_doi("https://doi.org/10.1000/XYZ.") == "10.1000/xyz"
assert citerra.latex_to_unicode("Jos\\'e") == "José"

names = citerra.parse_names("Jane Doe and {Research Group}")
assert names[1].literal == "Research Group"
```

## Local Build

Use the project manifest for local development:

```sh
guix shell -m manifest.scm -- maturin build --release --out target/wheels
```

For local tests without installing into the user environment, unpack the built
wheel into a temporary import directory and run pytest with that directory on
`PYTHONPATH`:

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
median_gb_s=0.053
max_gb_s=0.057
```
