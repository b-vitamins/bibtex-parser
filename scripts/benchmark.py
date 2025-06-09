#!/usr/bin/env python3
"""Single source for all benchmark operations.

Usage:
    ./scripts/benchmark.py          # Run all benchmarks and save report
    ./scripts/benchmark.py --perf   # Run only performance benchmarks
    ./scripts/benchmark.py --mem    # Run only memory benchmarks
    ./scripts/benchmark.py --delim  # Run only delimiter benchmarks
"""

import json
import subprocess
import sys
import time
from datetime import UTC, datetime
from pathlib import Path

try:
    from rich import box
    from rich.console import Console
    from rich.table import Table
except ImportError:
    print("Error: 'rich' library required")
    print("Install: pip install rich")
    sys.exit(1)

console = Console()

# Constants
NS_TO_US = 1000
NS_TO_MS = 1_000_000
MB = 1024 * 1024


def run_benchmark(bench_type: str) -> bool:
    """Run a specific benchmark and return success status."""
    bench_map = {"perf": "performance", "mem": "memory", "delim": "delimiter"}

    bench_name = bench_map.get(bench_type)
    if not bench_name:
        return False

    console.print(f"\n[yellow]Running {bench_name} benchmarks...[/yellow]")

    try:
        # For memory benchmark, we need special handling
        if bench_type == "mem":
            # First run the memory bench
            subprocess.run(
                ["cargo", "bench", "--bench", "memory"],
                capture_output=True,
                check=True,
            )
            # Then run the memory analysis
            result = subprocess.run(
                [
                    "cargo",
                    "run",
                    "--release",
                    "--bin",
                    "memanalysis",
                    "--",
                    "src/fixtures.rs",
                ],
                capture_output=True,
                text=True,
                check=True,
            )
            # Show memory stats
            show_memory_analysis(result.stdout)
        else:
            # Regular benchmark
            result = subprocess.run(
                ["cargo", "bench", "--bench", bench_name],
                capture_output=True,
                text=True,
                check=True,
            )
            # Show any useful output
            for line in result.stderr.split("\n"):
                if "entries" in line.lower() and "bytes" in line.lower():
                    console.print(f"[dim]{line.strip()}[/dim]")

        console.print(f"[green]✓ {bench_name} complete[/green]")
        return True

    except subprocess.CalledProcessError:
        console.print(f"[red]✗ {bench_name} failed[/red]")
        return False


def show_memory_analysis(output: str) -> None:
    """Display memory analysis results."""
    table = Table(box=box.SIMPLE_HEAD)
    table.add_column("Structure", style="cyan")
    table.add_column("Size", justify="right")
    table.add_column("Target", justify="right")
    table.add_column("Status", justify="center")

    # Parse the output for key metrics
    for line in output.split("\n"):
        if "Entry struct:" in line and "bytes" in line:
            size = line.split(":")[1].strip().split()[0]
            status = "[green]✓[/green]" if size == "64" else "[red]✗[/red]"
            table.add_row("Entry", f"{size} bytes", "64 bytes", status)
        elif "Value enum:" in line and "bytes" in line:
            size = line.split(":")[1].strip().split()[0]
            status = "[green]✓[/green]" if size == "24" else "[red]✗[/red]"
            table.add_row("Value", f"{size} bytes", "24 bytes", status)
        elif "Field struct:" in line and "bytes" in line:
            size = line.split(":")[1].strip().split()[0]
            status = (
                "[green]✓[/green]" if size == "40" else "[yellow]~[/yellow]"
            )
            table.add_row("Field", f"{size} bytes", "40 bytes", status)

    if table.row_count > 0:
        console.print(table)


def load_results() -> dict:
    """Load criterion benchmark results."""
    results = {}
    criterion_dir = Path("target/criterion")

    if not criterion_dir.exists():
        return results

    for json_file in criterion_dir.glob("**/base/estimates.json"):
        try:
            parts = list(json_file.parts)
            criterion_idx = parts.index("criterion")
            bench_name = "/".join(parts[criterion_idx + 1 : -2])

            with json_file.open() as f:
                data = json.load(f)
                results[bench_name] = data["mean"]["point_estimate"]
        except:
            continue

    return results


def load_previous() -> dict:
    """Load previous report for comparison."""
    report_dir = Path("benchmarks/reports")
    if not report_dir.exists():
        return {}

    reports = sorted(report_dir.glob("report_*.md"), reverse=True)
    if not reports:
        return {}

    # Extract JSON from report
    content = reports[0].read_text()
    if "```json" in content:
        start = content.find("```json") + 7
        end = content.find("```", start)
        try:
            data = json.loads(content[start:end])
            return data.get("criterion", {})
        except:
            return {}
    return {}


def format_time(ns: float) -> str:
    """Format time nicely."""
    if ns < NS_TO_US:
        return f"{ns:.0f}ns"
    if ns < NS_TO_MS:
        return f"{ns / NS_TO_US:.1f}µs"
    return f"{ns / NS_TO_MS:.1f}ms"


def format_delta(current: float, previous: float) -> str:
    """Format change from previous."""
    if previous == 0:
        return ""

    change = ((current - previous) / previous) * 100
    if abs(change) < 2:
        return ""
    if change < 0:
        return f" [green]▼{abs(change):.0f}%[/green]"
    return f" [red]▲{change:.0f}%[/red]"


def show_perf_results(results: dict, previous: dict) -> None:
    """Show performance benchmark results."""
    # Parse throughput results
    table = Table(title="Parse Performance", box=box.SIMPLE)
    table.add_column("Entries", justify="right", style="cyan")
    table.add_column("Time", justify="right")
    table.add_column("MB/s", justify="right", style="green")

    parse_results = {}
    for name, time in results.items():
        if "parse_throughput/bibtex_parser" in name:
            try:
                entries = int(name.split("/")[-1])
                parse_results[entries] = time
            except:
                continue

    for entries in sorted(parse_results.keys()):
        time = parse_results[entries]
        throughput = (entries * 340 / MB) / (time / 1e9)

        prev_key = f"parse_throughput/bibtex_parser/{entries}"
        delta = format_delta(time, previous.get(prev_key, 0))

        table.add_row(
            f"{entries:,}", format_time(time) + delta, f"{throughput:.0f}"
        )

    if parse_results:
        console.print(table)
        console.print()

    # Query operations
    query_table = Table(title="Query Operations", box=box.SIMPLE)
    query_table.add_column("Operation", style="cyan")
    query_table.add_column("Time", justify="right")

    queries = [
        ("find_by_key_hit", "Key lookup (hit)"),
        ("find_by_key_miss", "Key lookup (miss)"),
        ("find_by_type_common", "Type filter (common)"),
    ]

    for key, label in queries:
        for name, time in results.items():
            if f"query_operations/{key}" in name:
                delta = format_delta(time, previous.get(name, 0))
                query_table.add_row(label, format_time(time) + delta)
                break

    if query_table.row_count > 0:
        console.print(query_table)


def show_delim_results(results: dict, previous: dict) -> None:
    """Show delimiter benchmark results."""
    table = Table(title="Delimiter Finding (1K entries)", box=box.SIMPLE)
    table.add_column("Method", style="cyan")
    table.add_column("Time", justify="right")
    table.add_column("Speedup", justify="right", style="green")

    delim_times = {}
    for name, time in results.items():
        if "delimiter_throughput" in name and "/1000" in name:
            method = name.split("/")[1]
            delim_times[method] = time

    scalar = delim_times.get("scalar", 0)

    for method in ["scalar", "two_pass_memchr", "unrolled"]:
        if method in delim_times:
            time = delim_times[method]
            speedup = (
                f"{scalar / time:.1f}x"
                if scalar > 0 and method != "scalar"
                else "-"
            )

            prev_key = f"delimiter_throughput/{method}/1000"
            delta = format_delta(time, previous.get(prev_key, 0))

            table.add_row(method, format_time(time) + delta, speedup)

    if delim_times:
        console.print(table)


def save_report(results: dict) -> None:
    """Save benchmark report."""
    report_dir = Path("benchmarks/reports")
    report_dir.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.now(UTC)
    filename = f"report_{timestamp.strftime('%Y%m%d_%H%M%S')}.md"
    report_file = report_dir / filename

    with report_file.open("w") as f:
        f.write("# Benchmark Report\n\n")
        f.write(f"**Date**: {timestamp.strftime('%Y-%m-%d %H:%M:%S UTC')}\n\n")

        # Calculate summary stats
        f.write("## Summary\n\n")

        # Average throughput from 1K entry benchmark
        for name, time in results.items():
            if "parse_throughput/bibtex_parser/1000" in name:
                throughput = (1000 * 340 / MB) / (time / 1e9)
                f.write(f"- Parser throughput: {throughput:.0f} MB/s\n")
                break

        # Delimiter speedup
        scalar = next(
            (
                t
                for n, t in results.items()
                if "delimiter_throughput/scalar/1000" in n
            ),
            0,
        )
        memchr = next(
            (
                t
                for n, t in results.items()
                if "delimiter_throughput/two_pass_memchr/1000" in n
            ),
            0,
        )
        if scalar > 0 and memchr > 0:
            f.write(f"- Delimiter speedup: {scalar / memchr:.1f}x\n")

        f.write("\n## Raw Results\n\n")
        f.write("<details><summary>JSON data</summary>\n\n```json\n")
        json.dump({"criterion": results}, f, indent=2)
        f.write("\n```\n\n</details>\n")

    # Update symlink
    latest = report_dir / "latest.md"
    if latest.exists():
        latest.unlink()
    latest.symlink_to(filename)

    console.print(f"\n[green]Report saved to {report_file}[/green]")


def main() -> None:
    """Main entry point."""
    args = sys.argv[1:]

    # Determine what to run
    run_all = not args
    run_perf = run_all or "--perf" in args
    run_mem = run_all or "--mem" in args
    run_delim = run_all or "--delim" in args

    # Header
    console.print("\n[bold cyan]BibTeX Parser Benchmarks[/bold cyan]\n")

    # Run requested benchmarks
    if run_perf:
        run_benchmark("perf")
    if run_mem:
        run_benchmark("mem")
    if run_delim:
        run_benchmark("delim")

    # Wait for criterion to write files
    if run_perf or run_delim:
        time.sleep(1)

        # Load and display results
        console.print("\n[yellow]Loading results...[/yellow]\n")
        results = load_results()
        previous = load_previous()

        if run_perf:
            show_perf_results(results, previous)
        if run_delim:
            show_delim_results(results, previous)

        # Save report only when running all
        if run_all and results:
            save_report(results)

    console.print("\n[green]Done![/green]\n")


if __name__ == "__main__":
    main()
#!/usr/bin/env python3
"""Single source for all benchmark operations."""

import sys

try:
    from rich import box
    from rich.console import Console
    from rich.table import Table
except ImportError:
    print("Error: 'rich' library required")
    print("Install: pip install rich")
    sys.exit(1)

console = Console()

# Constants
NS_TO_US = 1000
NS_TO_MS = 1_000_000
NS_TO_S = 1_000_000_000
KB = 1024
MB = KB * KB

BENCHMARK_TYPES = {
    "--perf": ("performance", "Performance Benchmarks"),
    "--mem": ("memory", "Memory Benchmarks"),
    "--delim": ("delimiter", "Delimiter Benchmarks"),
}


def format_time(ns: float) -> str:
    """Format nanoseconds to appropriate unit."""
    if ns < NS_TO_US:
        return f"{ns:.0f}ns"
    if ns < NS_TO_MS:
        return f"{ns / NS_TO_US:.1f}µs"
    if ns < NS_TO_S:
        return f"{ns / NS_TO_MS:.1f}ms"
    return f"{ns / NS_TO_S:.2f}s"


def format_memory(bytes: float) -> str:
    """Format bytes to appropriate unit."""
    if bytes < KB:
        return f"{bytes:.0f}B"
    if bytes < MB:
        return f"{bytes / KB:.1f}KB"
    return f"{bytes / MB:.1f}MB"


def format_throughput(mb_per_sec: float) -> str:
    """Format throughput."""
    return f"{mb_per_sec:.0f} MB/s"


def format_change(current: float, previous: float) -> str:
    """Format change percentage with color."""
    if previous == 0:
        return "[dim]new[/dim]"

    change = ((current - previous) / previous) * 100
    if abs(change) < 1:
        return "[dim]~same[/dim]"
    if change < -5:
        return f"[green]▼{abs(change):.1f}%[/green]"  # Lower is better for time
    if change > 5:
        return f"[red]▲{change:.1f}%[/red]"
    return f"[yellow]{change:+.1f}%[/yellow]"


def run_cargo_bench(bench_name: str) -> bool:
    """Run cargo bench for a specific benchmark."""
    console.print(f"\n[yellow]Running {bench_name} benchmarks...[/yellow]")

    try:
        result = subprocess.run(
            ["cargo", "bench", "--bench", bench_name],
            capture_output=True,
            text=True,
            check=True,
        )

        # Show any important output (like delimiter counts)
        for line in result.stderr.split("\n"):
            if any(
                keyword in line for keyword in ["Input:", "Entries:", "Pattern"]
            ):
                console.print(f"[dim]{line}[/dim]")

        console.print(f"[green]✓ {bench_name} benchmarks complete[/green]")
        return True

    except subprocess.CalledProcessError as e:
        console.print(f"[red]✗ {bench_name} benchmarks failed[/red]")
        if e.stderr:
            console.print(f"[dim]{e.stderr}[/dim]")
        return False


def load_criterion_results() -> dict[str, float]:
    """Load benchmark results from criterion output."""
    results = {}
    criterion_dir = Path("target/criterion")

    if not criterion_dir.exists():
        return results

    # Find all estimates.json files
    for json_file in criterion_dir.glob("**/base/estimates.json"):
        parts = list(json_file.parts)

        try:
            criterion_idx = parts.index("criterion")
            bench_name = "/".join(parts[criterion_idx + 1 : -2])

            with json_file.open() as f:
                data = json.load(f)
                results[bench_name] = data["mean"]["point_estimate"]

        except (ValueError, json.JSONDecodeError, KeyError):
            continue

    return results


def load_previous_report() -> dict:
    """Load the most recent report for comparison."""
    report_dir = Path("benchmarks/reports")
    if not report_dir.exists():
        return {}

    # Find most recent report
    reports = sorted(report_dir.glob("report_*.md"), reverse=True)
    if not reports:
        return {}

    # Parse the JSON from the report
    report_content = reports[0].read_text()
    if "```json" in report_content:
        json_start = report_content.find("```json") + 7
        json_end = report_content.find("```", json_start)
        try:
            return json.loads(report_content[json_start:json_end])
        except json.JSONDecodeError:
            return {}

    return {}


def show_performance_results(results: dict, previous: dict) -> None:
    """Display performance benchmark results."""
    table = Table(title="Parse Performance", box=box.SIMPLE)
    table.add_column("Entries", style="cyan", justify="right")
    table.add_column("Time", justify="right")
    table.add_column("Throughput", justify="right", style="green")
    table.add_column("vs Previous", justify="center")

    # Extract parse benchmarks
    parse_results = {}
    for name, time in results.items():
        if "parse_throughput" in name and "bibtex_parser" in name:
            parts = name.split("/")
            if len(parts) >= 3:
                try:
                    entries = int(parts[-1])
                    parse_results[entries] = time
                except ValueError:
                    continue

    # Show results
    for entries in sorted(parse_results.keys()):
        time_ns = parse_results[entries]
        time_str = format_time(time_ns)

        # Calculate throughput (simplified)
        bytes_per_entry = 340  # Average
        throughput = (entries * bytes_per_entry / MB) / (time_ns / NS_TO_S)
        throughput_str = format_throughput(throughput)

        # Compare to previous
        prev_key = f"parse_throughput/bibtex_parser/{entries}"
        prev_time = previous.get("criterion", {}).get(prev_key, 0)
        change_str = format_change(time_ns, prev_time)

        table.add_row(f"{entries:,}", time_str, throughput_str, change_str)

    if parse_results:
        console.print(table)
        console.print()

    # Query operations
    query_table = Table(title="Query Operations", box=box.SIMPLE)
    query_table.add_column("Operation", style="cyan")
    query_table.add_column("Time", justify="right")
    query_table.add_column("vs Previous", justify="center")

    query_ops = {
        "find_by_key_hit": "Find by key (hit)",
        "find_by_key_miss": "Find by key (miss)",
        "find_by_type_common": "Find by type (common)",
    }

    for op_key, op_name in query_ops.items():
        for name, time in results.items():
            if f"query_operations/{op_key}" in name:
                time_str = format_time(time)

                prev_time = previous.get("criterion", {}).get(name, 0)
                change_str = format_change(time, prev_time)

                query_table.add_row(op_name, time_str, change_str)
                break

    if any(f"query_operations/{k}" in results for k in query_ops):
        console.print(query_table)
        console.print()


def show_memory_results(results: dict, previous: dict) -> None:
    """Display memory benchmark results."""
    # Run memory profiling subprocess
    console.print("[yellow]Running memory profiling...[/yellow]")

    try:
        result = subprocess.run(
            [
                "cargo",
                "run",
                "--release",
                "--bin",
                "memanalysis",
                "--",
                "benches/performance.rs",
            ],
            capture_output=True,
            text=True,
            check=True,
        )

        # Simple memory stats table
        table = Table(title="Memory Usage", box=box.SIMPLE)
        table.add_column("Metric", style="cyan")
        table.add_column("Size", justify="right")
        table.add_column("Status", justify="center")

        # Parse key metrics from output
        for line in result.stdout.split("\n"):
            if "Entry struct:" in line and "bytes" in line:
                if "64 bytes" in line:
                    table.add_row(
                        "Entry struct", "64 bytes", "[green]✓[/green]"
                    )
                else:
                    table.add_row(
                        "Entry struct",
                        line.split(":")[1].strip(),
                        "[red]✗[/red]",
                    )
            elif "Value enum:" in line and "bytes" in line:
                if "24 bytes" in line:
                    table.add_row("Value enum", "24 bytes", "[green]✓[/green]")
                else:
                    table.add_row(
                        "Value enum", line.split(":")[1].strip(), "[red]✗[/red]"
                    )

        if table.row_count > 0:
            console.print(table)
            console.print()

    except subprocess.CalledProcessError:
        console.print("[red]Memory profiling failed[/red]")


def show_delimiter_results(results: dict, previous: dict) -> None:
    """Display delimiter benchmark results."""
    table = Table(title="Delimiter Finding Performance", box=box.SIMPLE)
    table.add_column("Method", style="cyan")
    table.add_column("Time (1K entries)", justify="right")
    table.add_column("Speedup", justify="right", style="green")
    table.add_column("vs Previous", justify="center")

    # Extract delimiter results for 1000 entries
    delim_results = {}
    for name, time in results.items():
        if "delimiter_throughput" in name and "/1000" in name:
            method = name.split("/")[1]
            delim_results[method] = time

    # Calculate speedups
    scalar_time = delim_results.get("scalar", 0)

    for method in ["scalar", "two_pass_memchr", "naive_memchr", "unrolled"]:
        if method in delim_results:
            time = delim_results[method]
            time_str = format_time(time)

            speedup = (
                scalar_time / time
                if scalar_time > 0 and method != "scalar"
                else 1
            )
            speedup_str = f"{speedup:.2f}x" if method != "scalar" else "-"

            prev_key = f"delimiter_throughput/{method}/1000"
            prev_time = previous.get("criterion", {}).get(prev_key, 0)
            change_str = format_change(time, prev_time)

            table.add_row(method, time_str, speedup_str, change_str)

    if delim_results:
        console.print(table)
        console.print()


def save_report(all_results: dict) -> None:
    """Save benchmark report."""
    report_dir = Path("benchmarks/reports")
    report_dir.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.now(UTC)
    report_file = (
        report_dir / f"report_{timestamp.strftime('%Y%m%d_%H%M%S')}.md"
    )

    with report_file.open("w") as f:
        f.write("# Benchmark Report\n\n")
        f.write(
            f"**Generated**: {timestamp.strftime('%Y-%m-%d %H:%M:%S UTC')}\n"
        )
        f.write("**Version**: bibtex-parser v0.1.0\n\n")

        # Summary statistics
        f.write("## Summary\n\n")

        # Parse performance
        parse_results = {
            k: v
            for k, v in all_results.items()
            if "parse_throughput" in k and "bibtex_parser" in k
        }
        if parse_results:
            # Calculate average throughput
            total_throughput = 0
            count = 0
            for name, time in parse_results.items():
                if "/1000" in name:  # Use 1K entries as reference
                    throughput = (1000 * 340 / MB) / (time / NS_TO_S)
                    total_throughput += throughput
                    count += 1

            if count > 0:
                f.write(
                    f"- **Average throughput**: {total_throughput / count:.0f} MB/s\n"
                )

        # Delimiter speedup
        delim_results = {
            k: v
            for k, v in all_results.items()
            if "delimiter_throughput" in k and "/1000" in k
        }
        if len(delim_results) >= 2:
            scalar = next(
                (v for k, v in delim_results.items() if "scalar" in k), 0
            )
            memchr = next(
                (v for k, v in delim_results.items() if "two_pass" in k), 0
            )
            if scalar > 0 and memchr > 0:
                f.write(f"- **Delimiter speedup**: {scalar / memchr:.1f}x\n")

        f.write("\n")

        # Raw results
        f.write("## Raw Results\n\n")
        f.write("<details>\n<summary>Click to expand</summary>\n\n")
        f.write("```json\n")
        json.dump({"criterion": all_results}, f, indent=2, sort_keys=True)
        f.write("\n```\n\n</details>\n")

    # Update latest symlink
    latest = report_dir / "latest.md"
    if latest.exists() or latest.is_symlink():
        latest.unlink()
    latest.symlink_to(report_file.name)

    console.print(f"\n[green]Report saved:[/green] {report_file}")


def main() -> None:
    """Main entry point."""
    # Parse command line arguments
    args = set(sys.argv[1:])

    # Determine what to run
    if not args or not any(arg in BENCHMARK_TYPES for arg in args):
        # Run all benchmarks
        benchmarks_to_run = list(BENCHMARK_TYPES.values())
        save_report_flag = True
    else:
        # Run specific benchmarks
        benchmarks_to_run = [
            (name, title)
            for arg, (name, title) in BENCHMARK_TYPES.items()
            if arg in args
        ]
        save_report_flag = False

    # Header
    console.print("\n[bold cyan]BibTeX Parser Benchmarks[/bold cyan]")
    console.print(
        "[dim]"
        + ", ".join(title for _, title in benchmarks_to_run)
        + "[/dim]\n"
    )

    # Load previous results for comparison
    previous = load_previous_report()
    if previous:
        console.print("[dim]Comparing to previous results...[/dim]\n")

    # Run benchmarks
    for bench_name, _ in benchmarks_to_run:
        if not run_cargo_bench(bench_name):
            console.print(f"[red]Failed to run {bench_name} benchmarks[/red]")
            continue

        # Give criterion time to write results
        time.sleep(0.5)

    # Load all results
    console.print("\n[yellow]Loading results...[/yellow]")
    results = load_criterion_results()

    if not results:
        console.print("[red]No benchmark results found![/red]")
        return

    # Display results based on what was run
    console.print()

    if any(name == "performance" for name, _ in benchmarks_to_run):
        show_performance_results(results, previous)

    if any(name == "memory" for name, _ in benchmarks_to_run):
        show_memory_results(results, previous)

    if any(name == "delimiter" for name, _ in benchmarks_to_run):
        show_delimiter_results(results, previous)

    # Save report if running all benchmarks
    if save_report_flag:
        save_report(results)

    console.print("\n[green]Done![/green]")


if __name__ == "__main__":
    main()
