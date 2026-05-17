from __future__ import annotations

import argparse
import gc
import importlib
import importlib.metadata as metadata
import statistics
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Case:
    name: str
    version: str
    run: Callable[[], int | str]


def package_version(name: str) -> str:
    try:
        return metadata.version(name)
    except metadata.PackageNotFoundError:
        return "unknown"


def benchmark_case(
    case: Case,
    *,
    iterations: int,
    warmups: int,
    size: int,
) -> tuple[str, str, int, float, float]:
    for _ in range(warmups):
        result = case.run()
        assert result

    samples: list[float] = []
    last_count = 0
    for _ in range(iterations):
        gc.collect()
        start = time.perf_counter()
        result = case.run()
        elapsed = time.perf_counter() - start
        assert result
        last_count = len(result) if isinstance(result, str) else int(result)
        samples.append(elapsed)

    median = statistics.median(samples)
    throughput = size / median / 1024 / 1024
    return case.name, case.version, last_count, median, throughput


def load_cases(text: str, *, write: bool) -> list[Case]:
    cases: list[Case] = []

    try:
        import citerra

        if write:
            raw_document = citerra.parse(text, capture_source=True, preserve_raw=True)
            structured_document = citerra.parse(text, capture_source=False, preserve_raw=False)
            plain_records = raw_document.to_dicts()
            cases.append(
                Case(
                    "citerra raw-preserving write",
                    package_version("citerra"),
                    lambda: citerra.dumps(raw_document),
                )
            )
            cases.append(
                Case(
                    "citerra normalized write",
                    package_version("citerra"),
                    lambda: citerra.dumps(structured_document),
                )
            )
            cases.append(
                Case(
                    "citerra plain-record write",
                    package_version("citerra"),
                    lambda: citerra.write_entries(plain_records),
                )
            )
            cases.append(
                Case(
                    "citerra parse-record-update-write",
                    package_version("citerra"),
                    lambda: parse_record_update_write(citerra, text),
                )
            )
        else:
            cases.append(
                Case(
                    "citerra structured parse",
                    package_version("citerra"),
                    lambda: len(
                        citerra.parse(
                            text,
                            capture_source=False,
                            preserve_raw=False,
                        ).entries
                    ),
                )
            )
            cases.append(
                Case(
                    "citerra source-preserving parse",
                    package_version("citerra"),
                    lambda: len(citerra.parse(text, capture_source=True, preserve_raw=True).entries),
                )
            )
    except ImportError:
        pass

    try:
        bibtexparser = importlib.import_module("bibtexparser")
        version = package_version("bibtexparser")
        if hasattr(bibtexparser, "parse_string"):
            library = bibtexparser.parse_string(text) if write else None
            cases.append(
                Case(
                    "bibtexparser parse/write",
                    version,
                    (
                        lambda: bibtexparser.write_string(library)
                        if write
                        else len(bibtexparser.parse_string(text).entries)
                    ),
                )
            )
        elif hasattr(bibtexparser, "loads"):
            database = bibtexparser.loads(text) if write else None
            cases.append(
                Case(
                    "bibtexparser parse/write",
                    version,
                    (
                        lambda: bibtexparser.dumps(database)
                        if write
                        else len(bibtexparser.loads(text).entries)
                    ),
                )
            )
    except ImportError:
        pass

    try:
        pybtex_in = importlib.import_module("pybtex.database.input.bibtex")
        version = package_version("pybtex")
        if write:
            pybtex_out = importlib.import_module("pybtex.database.output.bibtex")
            data = pybtex_in.Parser().parse_string(text)
            writer = pybtex_out.Writer()
            cases.append(Case("pybtex write", version, lambda: writer.to_string(data)))
        else:
            cases.append(
                Case(
                    "pybtex parse",
                    version,
                    lambda: len(pybtex_in.Parser().parse_string(text).entries),
                )
            )
    except ImportError:
        pass

    return cases


def parse_record_update_write(citerra: object, text: str) -> str:
    document = citerra.parse(text, capture_source=True, preserve_raw=True)
    records = document.to_dicts()
    for record in records:
        record.setdefault("note", "accepted")
    document.update_from_dicts(records)
    return document.write()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", nargs="?", default="tests/fixtures/tugboat.bib")
    parser.add_argument("--iterations", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()

    text = Path(args.path).read_text(encoding="utf-8")
    size = len(text.encode("utf-8"))
    cases = load_cases(text, write=args.write)

    print("name\tversion\tcount_or_output_bytes\tmedian_seconds\tmib_per_second")
    for case in cases:
        name, version, count, median, throughput = benchmark_case(
            case,
            iterations=args.iterations,
            warmups=args.warmups,
            size=size,
        )
        print(f"{name}\t{version}\t{count}\t{median:.6f}\t{throughput:.1f}")


if __name__ == "__main__":
    main()
