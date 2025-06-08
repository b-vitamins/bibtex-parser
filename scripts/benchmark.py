#!/usr/bin/env python3
"""Generate benchmark reports from existing criterion results."""

import json
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import TextIO

try:
    from rich import box
    from rich.console import Console
    from rich.panel import Panel
    from rich.table import Table
except ImportError:
    print("Error: 'rich' library required")
    print("Install: pip install rich")
    print("Or: guix shell -m manifest.scm")
    sys.exit(1)

console = Console()

# Time constants
NS_TO_US = 1000
NS_TO_MS = 1_000_000
NS_TO_S = 1_000_000_000

# Display constants
OPS_MILLION = 1_000_000
OPS_THOUSAND = 1_000
# Speedup thresholds for coloring
SPEEDUP_EXCELLENT = 2.0
SPEEDUP_GOOD = 1.2
SPEEDUP_NEUTRAL = 1.0
MIN_SPEEDUP_SAMPLES = 2

# Benchmark parsing constants
MIN_COMPARISON_PARTS = 3
PARSER_COMPARISON_PARTS = 3  # parser_comparison/parser-name/entries

# Data constants - Updated based on actual measurements
BYTES_PER_ENTRY_ESTIMATE = {
    10: 300,  # Smaller entries
    50: 320,
    100: 335,
    500: 340,
    1000: 345,  # Average entries
    5000: 350,  # Larger entries with abstracts
}
DEFAULT_BYTES_PER_ENTRY = 340


def format_time(ns: float) -> str:
    """Format nanoseconds to appropriate unit."""
    if ns < NS_TO_US:
        return f"{ns:.0f} ns"
    if ns < NS_TO_MS:
        return f"{ns / NS_TO_US:.1f} µs"
    if ns < NS_TO_S:
        return f"{ns / NS_TO_MS:.1f} ms"
    return f"{ns / NS_TO_S:.2f} s"


# Memory size constants
KB = 1024
MB = KB * KB
GB = MB * KB


def format_memory(size_bytes: float) -> str:
    """Format bytes to appropriate unit."""
    if size_bytes < KB:
        return f"{size_bytes:.0f} B"
    if size_bytes < MB:
        return f"{size_bytes / KB:.1f} KB"
    if size_bytes < GB:
        return f"{size_bytes / MB:.1f} MB"
    return f"{size_bytes / GB:.2f} GB"


def calc_throughput(
    entries: int, time_ns: float, bytes_per_entry: int | None = None
) -> float:
    """Calculate throughput in MB/s."""
    if bytes_per_entry is None:
        bytes_per_entry = BYTES_PER_ENTRY_ESTIMATE.get(
            entries, DEFAULT_BYTES_PER_ENTRY
        )

    total_bytes = entries * bytes_per_entry
    time_s = time_ns / NS_TO_S
    return (total_bytes / MB) / time_s if time_s > 0 else 0


def load_results() -> dict[str, float]:
    """Load all benchmark results from criterion."""
    criterion_dir = Path("target/criterion")
    results = {}

    if not criterion_dir.exists():
        console.print("[red]No criterion results found![/red]")
        console.print("Run benchmarks first: [yellow]cargo bench[/yellow]")
        sys.exit(1)

    # Find all estimates.json files
    json_files = list(criterion_dir.glob("**/base/estimates.json"))

    if not json_files:
        console.print("[red]No benchmark results found![/red]")
        console.print("Make sure benchmarks completed successfully.")
        sys.exit(1)

    # Debug: show file paths
    if "--debug" in sys.argv:
        console.print("\n[dim]Found benchmark files:[/dim]")
        for json_file in sorted(json_files):
            console.print(f"  {json_file}")
        console.print()

    for json_file in json_files:
        # Build the full benchmark name from the directory structure
        parts = json_file.parts

        # Find the index of 'criterion' in the path
        try:
            criterion_idx = parts.index("criterion")
        except ValueError:
            continue

        # Build benchmark name from path components after 'criterion'
        # Skip 'base' and 'estimates.json'
        bench_parts = list(parts[criterion_idx + 1 : -2])

        # Reconstruct the full benchmark name
        bench_name = "/".join(bench_parts)

        try:
            with json_file.open() as f:
                data = json.load(f)
                results[bench_name] = data["mean"]["point_estimate"]
        except (json.JSONDecodeError, KeyError) as e:
            console.print(f"[dim]Error reading {bench_name}: {e}[/dim]")

    return results


def extract_entry_count(name: str) -> int | None:
    """Extract entry count from benchmark name parts."""
    parts = name.replace("-", "/").replace("_", "/").split("/")
    for part in reversed(parts):
        try:
            return int(part)
        except ValueError:
            continue
    return None


def parse_simple_entry_count(name: str) -> int | None:
    """Try to parse name as simple integer."""
    try:
        return int(name)
    except ValueError:
        return None


def parse_parser_comparison(
    name: str, time: float, parse_benches: dict, nom_benches: dict
) -> bool:
    """Parse parser_comparison benchmarks. Returns True if handled."""
    if "parser_comparison" not in name:
        return False

    parts = name.split("/")
    if len(parts) >= PARSER_COMPARISON_PARTS:
        try:
            entries = int(parts[-1])
            if "nom-bibtex" in parts[1]:
                nom_benches[entries] = time
            elif "bibtex-parser" in parts[1]:
                parse_benches[entries] = time
        except ValueError:
            pass
    return True


def parse_bibtex_parser_benchmark(
    name: str, time: float, parse_benches: dict
) -> bool:
    """Parse direct bibtex_parser benchmarks. Returns True if handled."""
    if "bibtex_parser/parse" not in name:
        return False

    try:
        entries = int(name.split("/")[-1])
        parse_benches[entries] = time
    except ValueError:
        pass
    return True


def parse_operations_benchmark(
    name: str, time: float, ops_benches: dict
) -> bool:
    """Parse operations benchmarks. Returns True if handled."""
    if "operations/" in name:
        ops_benches[name] = time
        return True
    return False


def parse_memory_benchmark(
    name: str, time: float, memory_benches: dict
) -> bool:
    """Parse memory benchmarks. Returns True if handled."""
    if "memory_usage/" in name:
        memory_benches[name] = time
        return True
    return False


def parse_fallback_benchmark(
    name: str,
    time: float,
    parse_benches: dict,
    ops_benches: dict,
    memory_benches: dict,
) -> None:
    """Handle fallback parsing for old format benchmarks."""
    # Check if it's just a simple number
    try:
        entries = int(name)
    except ValueError:
        # Not a number, check for other patterns
        if "find_by" in name:
            ops_benches[name] = time
        elif "parse_and_query" in name or "string_expansion" in name:
            memory_benches[name] = time
    else:
        parse_benches[entries] = time


def parse_benchmark_results(
    results: dict[str, float],
) -> tuple[dict, dict, dict, dict]:
    """Group benchmarks by type."""
    parse_benches = {}
    ops_benches = {}
    nom_benches = {}
    memory_benches = {}

    # Debug: show all benchmark names
    if "--debug" in sys.argv:
        console.print("[dim]Available benchmarks:[/dim]")
        for name in sorted(results.keys()):
            console.print(f"  {name}: {format_time(results[name])}")
        console.print()

    # Process all benchmarks
    for name, time in results.items():
        # Try each parser in order, stopping at first match
        if parse_parser_comparison(name, time, parse_benches, nom_benches):
            continue
        if parse_bibtex_parser_benchmark(name, time, parse_benches):
            continue
        if parse_operations_benchmark(name, time, ops_benches):
            continue
        if parse_memory_benchmark(name, time, memory_benches):
            continue

        # Fallback for unmatched patterns
        parse_fallback_benchmark(
            name, time, parse_benches, ops_benches, memory_benches
        )

    return parse_benches, ops_benches, nom_benches, memory_benches


def show_parse_performance(parse_benches: dict, nom_benches: dict) -> None:
    """Display parse performance table."""
    if not parse_benches:
        return

    table = Table(title="Parse Performance", box=box.ROUNDED)
    table.add_column("Entries", justify="right", style="cyan")
    table.add_column("File Size", justify="right", style="dim")
    table.add_column("Time", justify="right")
    table.add_column("Throughput", justify="right", style="green")
    table.add_column("Entries/sec", justify="right", style="dim")
    table.add_column("vs nom-bibtex", justify="center")

    for entries in sorted(parse_benches.keys()):
        time_ns = parse_benches[entries]
        time_str = format_time(time_ns)

        # Calculate metrics
        bytes_est = entries * BYTES_PER_ENTRY_ESTIMATE.get(
            entries, DEFAULT_BYTES_PER_ENTRY
        )
        file_size = format_memory(bytes_est)
        throughput = calc_throughput(entries, time_ns)
        entries_per_sec = entries / (time_ns / NS_TO_S)

        # Format entries/sec
        if entries_per_sec > OPS_MILLION:
            entries_str = f"{entries_per_sec / OPS_MILLION:.1f}M"
        elif entries_per_sec > OPS_THOUSAND:
            entries_str = f"{entries_per_sec / OPS_THOUSAND:.0f}K"
        else:
            entries_str = f"{entries_per_sec:.0f}"

        # Compare with nom-bibtex if available
        speedup_str = "-"
        if entries in nom_benches:
            speedup = nom_benches[entries] / time_ns
            if speedup >= SPEEDUP_EXCELLENT:
                color = "bright_green"
            elif speedup >= SPEEDUP_GOOD:
                color = "green"
            elif speedup >= SPEEDUP_NEUTRAL:
                color = "yellow"
            else:
                color = "red"
            speedup_str = f"[{color}]{speedup:.2f}x[/{color}]"

        table.add_row(
            f"{entries:,}",
            file_size,
            time_str,
            f"{throughput:.0f} MB/s",
            entries_str,
            speedup_str,
        )

    console.print(table)
    console.print()


def show_operations_performance(ops_benches: dict) -> None:
    """Display operations performance table."""
    if not ops_benches:
        return

    table = Table(title="Query Operations (1000 entries)", box=box.ROUNDED)
    table.add_column("Operation", style="cyan")
    table.add_column("Time", justify="right")
    table.add_column("Ops/sec", justify="right", style="green")

    op_names = {
        "find_by_key_hit": "Find by key (hit)",
        "find_by_key_miss": "Find by key (miss)",
        "find_by_type_common": "Find by type (20% match)",
        "find_by_type_rare": "Find by type (0% match)",
        "find_by_field": "Find by field (year)",
    }

    for key, display_name in op_names.items():
        for bench_name, time_ns in ops_benches.items():
            if key in bench_name:
                time_str = format_time(time_ns)
                ops_per_sec = NS_TO_S / time_ns

                if ops_per_sec > OPS_MILLION:
                    ops_str = f"{ops_per_sec / OPS_MILLION:.1f}M"
                elif ops_per_sec > OPS_THOUSAND:
                    ops_str = f"{ops_per_sec / OPS_THOUSAND:.0f}K"
                else:
                    ops_str = f"{ops_per_sec:.0f}"

                table.add_row(display_name, time_str, ops_str)
                break

    console.print(table)
    console.print()


def show_memory_performance(memory_benches: dict) -> None:
    """Display memory usage patterns."""
    if not memory_benches:
        return

    table = Table(title="Memory Usage Patterns", box=box.ROUNDED)
    table.add_column("Test", style="cyan")
    table.add_column("Time", justify="right")
    table.add_column("Notes", style="dim")

    test_names = {
        "parse_and_query": ("Parse + Query", "100 entries, typical usage"),
        "string_expansion": ("String Expansion", "Complex concatenations"),
    }

    for key, (name, notes) in test_names.items():
        for bench_name, time_ns in memory_benches.items():
            if key in bench_name:
                time_str = format_time(time_ns)
                table.add_row(name, time_str, notes)
                break

    if table.row_count > 0:
        console.print(table)
        console.print()


def show_summary(parse_benches: dict, nom_benches: dict) -> None:
    """Display performance summary with goals."""
    if not parse_benches:
        return

    # Calculate averages
    throughputs = [calc_throughput(e, t) for e, t in parse_benches.items()]
    avg_throughput = sum(throughputs) / len(throughputs) if throughputs else 0

    summary_lines = []

    # Throughput
    summary_lines.append(
        f"[green]Average throughput: {avg_throughput:.0f} MB/s[/green]"
    )

    # Time for specific sizes (Phase 1 goals)
    size_goals = [
        (1000, 5, "1K entries"),  # < 5ms goal
        (10000, 50, "10K entries"),  # < 50ms goal (extrapolated)
    ]

    for size, goal_ms, label in size_goals:
        if size in parse_benches:
            time_ms = parse_benches[size] / NS_TO_MS
            if time_ms < goal_ms:
                status = (
                    f"[green]✓ {time_ms:.1f}ms[/green] (goal: <{goal_ms}ms)"
                )
            else:
                status = (
                    f"[yellow]{time_ms:.1f}ms[/yellow] (goal: <{goal_ms}ms)"
                )
            summary_lines.append(f"{label}: {status}")

    # Speedup vs nom-bibtex
    if nom_benches and len(nom_benches) >= MIN_SPEEDUP_SAMPLES:
        speedups = []
        for entries, time in parse_benches.items():
            if entries in nom_benches:
                speedups.append(nom_benches[entries] / time)
        if speedups:
            avg_speedup = sum(speedups) / len(speedups)
            min_speedup = min(speedups)
            max_speedup = max(speedups)
            summary_lines.append(
                f"[green]vs nom-bibtex: {avg_speedup:.2f}x avg "
                f"({min_speedup:.2f}x - {max_speedup:.2f}x)[/green]"
            )
    elif not nom_benches:
        summary_lines.append(
            "[yellow]Note: nom-bibtex comparison not available[/yellow]"
        )
        summary_lines.append("[dim]Run: cargo bench --bench compare[/dim]")

    # Phase 1 Goals Progress
    summary_lines.append("\n[bold]Phase 1 Goals:[/bold]")
    summary_lines.append("[ ] 10x performance improvement")
    summary_lines.append("[ ] Memory < 1.5x file size")
    summary_lines.append("[ ] Parse 1MB in < 5ms")

    console.print(
        Panel("\n".join(summary_lines), title="Summary", box=box.DOUBLE)
    )


def write_report_header(f: TextIO, timestamp: datetime) -> None:
    """Write report header."""
    f.write("# Benchmark Report\n\n")
    f.write(f"**Generated**: {timestamp.strftime('%Y-%m-%d %H:%M:%S UTC')}\n")
    f.write("**Version**: bibtex-parser v0.1.0 (pre-optimization)\n")
    f.write("**Phase**: 1.1 - Baseline Metrics\n\n")


def write_report_summary(
    f: TextIO, parse_benches: dict, nom_benches: dict
) -> None:
    """Write report summary section."""
    if not parse_benches:
        return

    f.write("## Summary\n\n")

    # Average throughput
    throughputs = [calc_throughput(e, t) for e, t in parse_benches.items()]
    avg_throughput = sum(throughputs) / len(throughputs)
    f.write(f"- **Average throughput**: {avg_throughput:.0f} MB/s\n")

    # Specific benchmarks
    for entries in [100, 1000, 5000]:
        if entries in parse_benches:
            time_str = format_time(parse_benches[entries])
            f.write(f"- **Parse {entries:,} entries**: {time_str}\n")

    # nom-bibtex comparison
    if nom_benches:
        speedups = []
        for e, t in parse_benches.items():
            if e in nom_benches:
                speedups.append(nom_benches[e] / t)
        if speedups:
            avg_speedup = sum(speedups) / len(speedups)
            f.write(f"- **vs nom-bibtex**: {avg_speedup:.2f}x average\n")

    f.write("\n")


def write_performance_table(
    f: TextIO, parse_benches: dict, nom_benches: dict
) -> None:
    """Write parse performance table."""
    f.write("## Parse Performance\n\n")
    f.write("| Entries | Time | Throughput | vs nom-bibtex |\n")
    f.write("|---------|------|------------|---------------|\n")

    for entries in sorted(parse_benches.keys()):
        time_str = format_time(parse_benches[entries])
        throughput = calc_throughput(entries, parse_benches[entries])

        speedup_str = "-"
        if entries in nom_benches:
            speedup = nom_benches[entries] / parse_benches[entries]
            speedup_str = f"{speedup:.2f}x"

        f.write(
            f"| {entries:,} | {time_str} | "
            f"{throughput:.0f} MB/s | {speedup_str} |\n"
        )

    f.write("\n")


def save_report(results: dict[str, float]) -> None:
    """Save results to markdown report."""
    report_dir = Path("benchmarks/reports")
    report_dir.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.now(UTC)
    report_file = (
        report_dir / f"report_{timestamp.strftime('%Y%m%d_%H%M%S')}.md"
    )

    parse_benches, ops_benches, nom_benches, _ = parse_benchmark_results(
        results
    )

    with report_file.open("w") as f:
        write_report_header(f, timestamp)
        write_report_summary(f, parse_benches, nom_benches)
        write_performance_table(f, parse_benches, nom_benches)

        # Raw data
        f.write("## Raw Results\n\n")
        f.write("<details>\n<summary>Click to expand</summary>\n\n")
        f.write("```json\n")
        json.dump(results, f, indent=2, sort_keys=True)
        f.write("\n```\n\n</details>\n")

    console.print(f"\n[dim]Report saved: {report_file}[/dim]")

    # Create latest symlink
    latest = report_dir / "latest.md"
    if latest.exists() or latest.is_symlink():
        latest.unlink()
    latest.symlink_to(report_file.name)


def main() -> None:
    """Generate benchmark report from existing criterion results."""
    console.print(
        Panel.fit(
            "[bold cyan]BibTeX Parser Benchmark Report[/bold cyan]\n"
            "[dim]Phase 1.1 - Performance Baseline[/dim]",
            border_style="cyan",
        )
    )
    console.print()

    # Load results
    results = load_results()
    console.print(f"[green]Found {len(results)} benchmark results[/green]\n")

    # Debug mode: show all loaded benchmarks
    if "--debug" in sys.argv:
        console.print("[dim]Loaded benchmarks:[/dim]")
        for name, time in sorted(results.items()):
            console.print(f"  {name}: {format_time(time)}")
        console.print()

    # Display results
    parse_benches, ops_benches, nom_benches, memory_benches = (
        parse_benchmark_results(results)
    )

    if parse_benches:
        show_parse_performance(parse_benches, nom_benches)
    if ops_benches:
        show_operations_performance(ops_benches)
    if memory_benches:
        show_memory_performance(memory_benches)

    # Always show summary if we have any parse benchmarks
    if parse_benches or nom_benches:
        show_summary(parse_benches, nom_benches)

    # Save report
    save_report(results)


if __name__ == "__main__":
    main()
