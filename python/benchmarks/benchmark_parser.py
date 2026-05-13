from __future__ import annotations

import argparse
import statistics
import time
from pathlib import Path

import citerra


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", nargs="?", default="tests/fixtures/tugboat.bib")
    parser.add_argument("--iterations", type=int, default=20)
    parser.add_argument("--tolerant", action="store_true")
    args = parser.parse_args()

    text = Path(args.path).read_text()
    measurements = []
    for _ in range(args.iterations):
        start = time.perf_counter()
        document = citerra.parse(text, tolerant=args.tolerant)
        elapsed = time.perf_counter() - start
        measurements.append(len(text) / elapsed / 1_000_000_000)
        assert len(document.entries) > 0

    print(f"path={args.path}")
    print(f"bytes={len(text)}")
    print(f"iterations={args.iterations}")
    print(f"median_gb_s={statistics.median(measurements):.3f}")
    print(f"max_gb_s={max(measurements):.3f}")


if __name__ == "__main__":
    main()
