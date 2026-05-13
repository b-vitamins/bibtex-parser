# citerra

[![CI](https://github.com/b-vitamins/citerra/actions/workflows/ci.yml/badge.svg)](https://github.com/b-vitamins/citerra/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/citerra.svg)](https://pypi.org/project/citerra/)
[![Python](https://img.shields.io/pypi/pyversions/citerra.svg)](https://pypi.org/project/citerra/)
[![License](https://img.shields.io/pypi/l/citerra.svg)](https://github.com/b-vitamins/citerra/blob/master/LICENSE)

BibTeX parser for Python.

`citerra` parses, validates, edits, and writes BibTeX documents. It supports
strict parsing by default, opt-in tolerant recovery, diagnostics with source
locations, raw-text retention, source-preserving writes, name/date/identifier
helpers, and plain-record projection for application code.

The package is distributed as ABI3 wheels for Python 3.8 and newer.

## Performance Snapshot

Measured on `tests/fixtures/tugboat.bib`: 2,701,551 bytes, 73,993 lines, and
3,644 entries. Hardware was AMD Ryzen 5 5600G, 6 cores / 12 threads. Measured
on 2026-05-14 with Python `3.11.14`; throughput is input-size normalized.

For a BibTeX parser, the relevant speed measurements are:

| Workload | Why it matters | `citerra` result |
| --- | --- | ---: |
| Structured parse | Load a bibliography into entries and fields for application logic | 0.008 s, 322.2 MiB/s |
| Source-preserving parse | Keep raw text, source locations, diagnostics, and source-order blocks for tools | 0.011 s, 232.8 MiB/s |
| Raw-preserving write | Write retained source text after low-churn edits | 0.002 s, 1149.8 MiB/s |
| Normalized write | Serialize structured data with configured formatting | 0.010 s, 259.7 MiB/s |

The comparison used `citerra` 0.2.3, `bibtexparser` 1.4.4,
`bibtexparser` 2.0.0b9, and `pybtex` 0.26.1. `citerra` structured parse
disables source capture and raw preservation for the closest parser-output
comparison. Relative time is normalized to the first row in each table.

| Python parser / mode | Version | Output retained | Median parse time | Throughput | Approx. entries/s | Relative time |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| `citerra` structured parse | 0.2.3 | Entries, fields, strings, comments, preambles | 0.008 s | 322.2 MiB/s | 455.7k | 1.0x |
| `citerra` source-preserving parse | 0.2.3 | Structured data, raw text, locations, diagnostics | 0.011 s | 232.8 MiB/s | 329.3k | 1.4x |
| `bibtexparser` parse | 2.0.0b9 | Entries/library model | 0.367 s | 7.0 MiB/s | 9.9k | 45.9x |
| `pybtex` parse | 0.26.1 | Bibliography data | 0.863 s | 3.0 MiB/s | 4.2k | 107.9x |
| `bibtexparser` parse | 1.4.4 | Entries/database model | 10.758 s | 0.2 MiB/s | 0.34k | 1345.2x |

| Python writer / mode | Version | Median write time | Throughput | Relative time |
| --- | ---: | ---: | ---: | ---: |
| `citerra` raw-preserving write | 0.2.3 | 0.002 s | 1149.8 MiB/s | 1.0x |
| `citerra` normalized write | 0.2.3 | 0.010 s | 259.7 MiB/s | 4.4x |
| `bibtexparser` write | 1.4.4 | 0.106 s | 24.3 MiB/s | 47.4x |
| `bibtexparser` write | 2.0.0b9 | 0.497 s | 5.2 MiB/s | 222.0x |
| `pybtex` write | 0.26.1 | 3.790 s | 0.7 MiB/s | 1691.3x |

The workflow table below sums the median parse and write measurements for each
parser/version. It is a round-trip estimate for parse-edit-write workloads, not
a separate end-to-end benchmark.

| Workflow | Median time | Throughput | Relative time |
| --- | ---: | ---: | ---: |
| `citerra` source-preserving parse + raw-preserving write | 0.013 s | 193.6 MiB/s | 1.0x |
| `citerra` structured parse + normalized write | 0.018 s | 143.8 MiB/s | 1.3x |
| `bibtexparser` 2.0.0b9 parse + write | 0.864 s | 3.0 MiB/s | 65.0x |
| `pybtex` 0.26.1 parse + write | 4.653 s | 0.6 MiB/s | 349.7x |
| `bibtexparser` 1.4.4 parse + write | 10.864 s | 0.2 MiB/s | 816.5x |

Reproduction commands are listed in [Reproducing Benchmarks](#reproducing-benchmarks).

## Install

```sh
pip install citerra
```

The distribution name and import name are both `citerra`:

```python
import citerra
```

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

File helpers are available:

```python
from pathlib import Path
import citerra

document = citerra.parse_path("references.bib", tolerant=True)
Path("normalized.bib").write_text(citerra.dumps(document), encoding="utf-8")
```

File-like helpers are also available:

```python
with open("references.bib", encoding="utf-8") as handle:
    document = citerra.load(handle, tolerant=True)

text = citerra.dumps(document)
```

## Document Model

- `Document` contains entries, comments, preambles, string definitions,
  source-order blocks, diagnostics, and validation helpers.
- `Entry` exposes the citation key, entry type, fields, source text, semantic
  helpers, and field mutation methods.
- `Field` exposes the original field name, parsed value, optional raw source
  text, and optional source location.
- `Value` represents string literals, numbers, variables, and concatenations.
- `Diagnostic` reports parse or validation problems with stable codes and
  source locations when available.

## Tolerant Parsing And Diagnostics

```python
text = '''
@article{ok, title = "Good"}
@article{bad, title = "Missing close"
@book{recovered, title = "Recovered"}
'''

document = citerra.parse(
    text,
    tolerant=True,
    capture_source=True,
    preserve_raw=True,
    source="refs/main.bib",
)

if document.status != "ok":
    for diagnostic in document.diagnostics:
        span = diagnostic.source
        if span is None:
            print(diagnostic.code, diagnostic.message)
        else:
            print(diagnostic.code, span.line, span.column, diagnostic.message)
```

## Raw Text And Source-Preserving Writes

```python
text = '@article{paper, title = "Example Paper"}'

document = citerra.parse(
    text,
    tolerant=True,
    capture_source=True,
    preserve_raw=True,
)

entry = document.entry("paper")
if entry is not None:
    print(entry.raw)
    print(entry.field("title").raw_value)
```

Use `WriterConfig(preserve_raw=True)` for low-churn output that reuses retained
source text where possible. Use `WriterConfig(preserve_raw=False)` for
normalized structured output.

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

## Plain Records

Some application code wants ordinary dictionaries for filtering, indexing, or
bulk transforms. `citerra` provides explicit helpers for that shape without
changing the document model:

```python
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

Plain records use `ENTRYTYPE` and `ID` keys for the entry type and citation key.

## Helpers

```python
assert citerra.normalize_doi("https://doi.org/10.1000/XYZ.") == "10.1000/xyz"
assert citerra.latex_to_unicode("Jos\\'e") == "José"

names = citerra.parse_names("Jane Doe and {Research Group}")
assert names[1].literal == "Research Group"

date = citerra.parse_date("2026-05-13")
assert (date.year, date.month, date.day) == (2026, 5, 13)
```

## Reproducing Benchmarks

The comparison script uses whichever optional packages are installed in the
active environment. `bibtexparser` 1.x and 2.x use the same package name, so
their rows are measured in separate environments.

```sh
python python/benchmarks/compare_parsers.py tests/fixtures/tugboat.bib --iterations 15 --warmups 3
python python/benchmarks/compare_parsers.py tests/fixtures/tugboat.bib --write --iterations 15 --warmups 3
```

## Implementation

`citerra` is implemented as a native extension. Wheels include the parser
engine, so ordinary Python installs do not require a Rust toolchain.

## Rust Crate

The Rust crate is published as `bibtex-parser` on crates.io:

```toml
[dependencies]
bibtex-parser = "0.2"
```

See [RUST.md](RUST.md) for Rust usage.

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

## License

Licensed under either of Apache-2.0 or MIT, at your option.
