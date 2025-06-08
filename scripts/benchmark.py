#!/usr/bin/env python3
"""BibtTeX Parser Benchmark."""

import json
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path

try:
    from rich import box
    from rich.console import Console
    from rich.panel import Panel
    from rich.progress import Progress, SpinnerColumn, TextColumn
    from rich.table import Table
except ImportError:
    print("Error: 'rich' library required")
    print("Run: guix shell -m manifest.scm")
    sys.exit(1)

console = Console()

# Constants
NS_TO_US = 1000
NS_TO_MS = 1_000_000
NS_TO_S = 1_000_000_000
BYTES_PER_ENTRY = 350
MIB = 1048576


class Benchmark:
    """Minimal benchmark runner."""

    def __init__(self) -> None:
        """Initialize benchmark runner."""
        self.results: dict[str, float] = {}
        self.timestamp = datetime.now(UTC).strftime("%Y%m%d_%H%M%S")

    def run(self) -> None:
        """Run all benchmarks."""
        self.show_header()
        self.build()
        self.benchmark()
        self.parse_results()
        self.show_results()
        self.save_report()

    def show_header(self) -> None:
        """Display header."""
        console.print(
            Panel.fit(
                "[bold cyan]BibtTeX Parser Benchmarks[/bold cyan]",
                border_style="cyan",
                box=box.DOUBLE,
            )
        )
        console.print()

    def build(self) -> None:
        """Build the project."""
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
        ) as progress:
            progress.add_task("[yellow]Building...[/yellow]", total=None)
            subprocess.run(  # noqa: S603
                [  # noqa: S607
                    "cargo",
                    "build",
                    "--release",
                    "--benches",
                    "--features",
                    "compare_nom_bibtex",
                ],
                check=True,
                capture_output=True,
            )

    def benchmark(self) -> None:
        """Run benchmarks."""
        console.print("[yellow]Running benchmarks...[/yellow]\n")

        subprocess.run(  # noqa: S603
            ["cargo", "bench", "--bench", "parser"],  # noqa: S607
            capture_output=True,
            check=False,
        )
        subprocess.run(  # noqa: S603
            [  # noqa: S607
                "cargo",
                "bench",
                "--bench",
                "compare",
                "--features",
                "compare_nom_bibtex",
            ],
            capture_output=True,
            check=False,
        )

    def parse_results(self) -> None:
        """Parse benchmark results from criterion output."""
        criterion_dir = Path("target/criterion")

        for result_dir in criterion_dir.glob("**/estimates.json"):
            bench_name = result_dir.parent.parent.name
            with result_dir.open() as f:
                data = json.load(f)
                self.results[bench_name] = data["mean"]["point_estimate"]

    def show_results(self) -> None:
        """Display benchmark results."""
        # Parse Performance
        parse_table = Table(title="Parse Performance", box=box.ROUNDED)
        parse_table.add_column("Entries", justify="right", style="cyan")
        parse_table.add_column("Time", justify="right")
        parse_table.add_column("Throughput", justify="right", style="green")
        parse_table.add_column("vs nom-bibtex", justify="center")

        for entries in [10, 50, 100, 500, 1000, 5000]:
            key = f"bibtex_parser/parse/{entries}"
            if key in self.results:
                time_ns = self.results[key]
                time_str = self.format_time(time_ns)
                throughput = self.calc_throughput(entries, time_ns)

                # Compare with nom-bibtex
                nom_key = f"parser_comparison/nom-bibtex/{entries}"
                if nom_key in self.results:
                    speedup = self.results[nom_key] / time_ns
                    speedup_str = (
                        f"[green]{speedup:.1f}x[/green]"
                        if speedup > 1
                        else f"{speedup:.1f}x"
                    )
                else:
                    speedup_str = "-"

                parse_table.add_row(
                    f"{entries:,}",
                    time_str,
                    f"{throughput:.0f} MiB/s",
                    speedup_str,
                )

        console.print(parse_table)
        console.print()

        # Operations
        ops_table = Table(title="Operations", box=box.ROUNDED)
        ops_table.add_column("Operation", style="cyan")
        ops_table.add_column("Time", justify="right")

        ops = {
            "operations/find_by_key_hit": "Find by key",
            "operations/find_by_type_common": "Find by type",
            "operations/find_by_field": "Find by field",
        }

        for key, name in ops.items():
            if key in self.results:
                time_str = self.format_time(self.results[key])
                ops_table.add_row(name, time_str)

        console.print(ops_table)
        console.print()

        # Summary
        self.show_summary()

    def format_time(self, ns: float) -> str:
        """Format nanoseconds to appropriate unit."""
        if ns < NS_TO_US:
            return f"{ns:.0f} ns"
        if ns < NS_TO_MS:
            return f"{ns / NS_TO_US:.1f} µs"
        if ns < NS_TO_S:
            return f"{ns / NS_TO_MS:.1f} ms"
        return f"{ns / NS_TO_S:.2f} s"

    def calc_throughput(self, entries: int, time_ns: float) -> float:
        """Calculate throughput in MiB/s."""
        total_bytes = entries * BYTES_PER_ENTRY
        time_s = time_ns / NS_TO_S
        return (total_bytes / MIB) / time_s if time_s > 0 else 0

    def show_summary(self) -> None:
        """Display performance summary."""
        # Calculate average throughput
        throughputs = []
        for entries in [10, 50, 100, 500, 1000]:
            key = f"bibtex_parser/parse/{entries}"
            if key in self.results:
                throughputs.append(
                    self.calc_throughput(entries, self.results[key])
                )

        if throughputs:
            avg_throughput = sum(throughputs) / len(throughputs)

            summary = f"[green]Average: {avg_throughput:.0f} MiB/s[/green]"

            # vs nom-bibtex
            our_key = "bibtex_parser/parse/1000"
            nom_key = "parser_comparison/nom-bibtex/1000"
            if our_key in self.results and nom_key in self.results:
                speedup = self.results[nom_key] / self.results[our_key]
                summary += (
                    f" • [green]{speedup:.1f}x faster than nom-bibtex[/green]"
                )

            console.print(Panel(summary, title="Summary", box=box.DOUBLE))

    def save_report(self) -> None:
        """Save benchmark report to file."""
        report_dir = Path("benchmarks/reports")
        report_dir.mkdir(parents=True, exist_ok=True)

        report_file = report_dir / f"report_{self.timestamp}.md"

        with report_file.open("w") as f:
            timestamp = datetime.now(UTC).strftime("%Y-%m-%d %H:%M")
            f.write(f"# Benchmark Report - {timestamp}\n\n")
            f.write("## Results\n\n```json\n")
            json.dump(self.results, f, indent=2)
            f.write("\n```\n")

        console.print(f"\n[dim]Report: {report_file}[/dim]")


def main() -> None:
    """Run benchmarks."""
    if not Path("Cargo.toml").exists():
        console.print("[red]Error: Run from project root[/red]")
        sys.exit(1)

    try:
        Benchmark().run()
    except KeyboardInterrupt:
        console.print("\n[yellow]Interrupted[/yellow]")
        sys.exit(0)
    except subprocess.CalledProcessError as e:
        console.print(f"[red]Build/benchmark failed: {e}[/red]")
        sys.exit(1)


if __name__ == "__main__":
    main()
