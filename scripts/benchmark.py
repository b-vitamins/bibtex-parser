#!/usr/bin/env python3
"""Generate benchmark reports from existing criterion results."""

import json
import subprocess
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
MIN_MEMORY_PARSE_PARTS = 5  # For parsing memory output
MEMORY_OVERHEAD_EXCELLENT = 1.5
MEMORY_OVERHEAD_GOOD = 2.0

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

    # If only running memory benchmarks, criterion results are optional
    if "--memory" in sys.argv and not criterion_dir.exists():
        console.print(
            "[yellow]No criterion results found, running memory benchmarks only.[/yellow]"
        )
        return {}

    if not criterion_dir.exists():
        console.print("[red]No criterion results found![/red]")
        console.print("Run benchmarks first: [yellow]cargo bench[/yellow]")
        sys.exit(1)

    # Find all estimates.json files
    json_files = list(criterion_dir.glob("**/base/estimates.json"))

    if not json_files:
        if "--memory" in sys.argv:
            console.print(
                "[yellow]No benchmark results found, running memory benchmarks only.[/yellow]"
            )
            return {}
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


def show_memory_time_performance(memory_benches: dict) -> None:
    """Display memory usage patterns from criterion benchmarks."""
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


def run_memory_profiling() -> dict[str, dict]:
    """Run memory profiling and parse results."""
    console.print("\n[yellow]Running memory profiling benchmark...[/yellow]")

    try:
        # Run the memory benchmark (S603, S607: cargo is expected to be in PATH)
        result = subprocess.run(  # noqa: S603
            ["cargo", "bench", "--bench", "memory"],  # noqa: S607
            capture_output=True,
            text=True,
            check=True,
        )

        # Parse the output
        memory_data = {}
        lines = result.stdout.strip().split("\n")

        for line in lines:
            if line.startswith("memory_parse/"):
                parts = line.split("\t")
                if len(parts) >= MIN_MEMORY_PARSE_PARTS:
                    # memory_parse/entries  input_size  peak  current  overhead
                    entries = int(parts[0].split("/")[1])
                    memory_data[entries] = {
                        "input_size": int(parts[1]),
                        "peak": int(parts[2]),
                        "current": int(parts[3]),
                        "overhead": float(parts[4]),
                    }

        if memory_data:
            console.print(
                f"[green]Memory profiling complete - {len(memory_data)} data points collected[/green]"
            )
        else:
            console.print(
                "[yellow]No memory data collected - check benchmark output[/yellow]"
            )

    except subprocess.CalledProcessError as e:
        console.print(f"[red]Memory profiling failed: {e}[/red]")
        if e.stderr:
            console.print(f"[dim]stderr: {e.stderr}[/dim]")
        return {}
    except (ValueError, IndexError) as e:
        console.print(f"[red]Error parsing memory data: {e}[/red]")
        return {}
    else:
        return memory_data


def show_memory_performance(memory_data: dict) -> None:
    """Display memory usage table."""
    if not memory_data:
        return

    table = Table(title="Memory Usage Analysis", box=box.ROUNDED)
    table.add_column("Entries", justify="right", style="cyan")
    table.add_column("Input Size", justify="right", style="dim")
    table.add_column("Peak Memory", justify="right")
    table.add_column("Memory Overhead", justify="right", style="yellow")

    for entries in sorted(memory_data.keys()):
        data = memory_data[entries]

        # Format memory sizes
        input_size = format_memory(data["input_size"])
        peak_memory = format_memory(data["peak"])
        overhead = data["overhead"]

        # Color code overhead
        if overhead < MEMORY_OVERHEAD_EXCELLENT:
            overhead_str = f"[green]{overhead:.2f}x[/green]"
        elif overhead < MEMORY_OVERHEAD_GOOD:
            overhead_str = f"[yellow]{overhead:.2f}x[/yellow]"
        else:
            overhead_str = f"[red]{overhead:.2f}x[/red]"

        table.add_row(f"{entries:,}", input_size, peak_memory, overhead_str)

    console.print(table)
    console.print()


def calculate_performance_metrics(parse_benches: dict) -> tuple[float, dict]:
    """Calculate average throughput and time metrics."""
    throughputs = [calc_throughput(e, t) for e, t in parse_benches.items()]
    avg_throughput = sum(throughputs) / len(throughputs) if throughputs else 0

    # Check specific size goals
    time_metrics = {}
    size_goals = [
        (1000, 5, "1K entries"),  # < 5ms goal
        (10000, 50, "10K entries"),  # < 50ms goal (extrapolated)
    ]

    for size, goal_ms, label in size_goals:
        if size in parse_benches:
            time_ms = parse_benches[size] / NS_TO_MS
            time_metrics[label] = (time_ms, goal_ms, time_ms < goal_ms)

    return avg_throughput, time_metrics


def calculate_speedup_metrics(
    parse_benches: dict, nom_benches: dict
) -> dict | None:
    """Calculate speedup metrics vs nom-bibtex."""
    if not nom_benches or len(nom_benches) < MIN_SPEEDUP_SAMPLES:
        return None

    speedups = []
    for entries, time in parse_benches.items():
        if entries in nom_benches:
            speedups.append(nom_benches[entries] / time)

    if speedups:
        return {
            "avg": sum(speedups) / len(speedups),
            "min": min(speedups),
            "max": max(speedups),
        }
    return None


def calculate_memory_metrics(memory_data: dict | None) -> tuple[float, bool]:
    """Calculate memory overhead metrics."""
    if not memory_data:
        return 0.0, False

    overheads = [data["overhead"] for data in memory_data.values()]
    if overheads:
        avg_overhead = sum(overheads) / len(overheads)
        meets_goal = avg_overhead < MEMORY_OVERHEAD_EXCELLENT
        return avg_overhead, meets_goal
    return 0.0, False


def format_summary_lines(
    avg_throughput: float,
    time_metrics: dict,
    speedup_metrics: dict | None,
    memory_metrics: tuple[float, bool],
) -> list[str]:
    """Format all summary lines."""
    summary_lines = []

    # Only show throughput if we have data
    if avg_throughput > 0:
        summary_lines.append(
            f"[green]Average throughput: {avg_throughput:.0f} MB/s[/green]"
        )

        # Time goals
        for label, (time_ms, goal_ms, meets_goal) in time_metrics.items():
            if meets_goal:
                status = (
                    f"[green]✓ {time_ms:.1f}ms[/green] (goal: <{goal_ms}ms)"
                )
            else:
                status = (
                    f"[yellow]{time_ms:.1f}ms[/yellow] (goal: <{goal_ms}ms)"
                )
            summary_lines.append(f"{label}: {status}")

        # Speedup vs nom-bibtex
        if speedup_metrics:
            summary_lines.append(
                f"[green]vs nom-bibtex: {speedup_metrics['avg']:.2f}x avg "
                f"({speedup_metrics['min']:.2f}x - {speedup_metrics['max']:.2f}x)[/green]"
            )
        else:
            summary_lines.append(
                "[yellow]Note: nom-bibtex comparison not available[/yellow]"
            )
            summary_lines.append("[dim]Run: cargo bench --bench compare[/dim]")

    # Memory overhead
    avg_overhead, meets_memory_goal = memory_metrics
    if avg_overhead > 0:
        status = (
            "[green]✓[/green]" if meets_memory_goal else "[yellow]![/yellow]"
        )
        summary_lines.append(
            f"Memory overhead: {avg_overhead:.2f}x {status} (goal: <{MEMORY_OVERHEAD_EXCELLENT}x)"
        )

    # Phase 1 Goals
    summary_lines.append("\n[bold]Phase 1 Goals:[/bold]")
    summary_lines.append("[ ] 10x performance improvement")

    if avg_overhead > 0:
        if meets_memory_goal:
            summary_lines.append("[x] Memory < 1.5x file size")
        else:
            summary_lines.append("[ ] Memory < 1.5x file size")
    else:
        summary_lines.append("[ ] Memory < 1.5x file size")

    summary_lines.append("[ ] Parse 1MB in < 5ms")

    return summary_lines


def show_summary(
    parse_benches: dict, nom_benches: dict, memory_data: dict | None = None
) -> None:
    """Display performance summary with goals."""
    # Handle case where we only have memory data
    if not parse_benches and not memory_data:
        return

    # Calculate all metrics
    avg_throughput = 0.0
    time_metrics = {}
    speedup_metrics = None

    if parse_benches:
        avg_throughput, time_metrics = calculate_performance_metrics(
            parse_benches
        )
        speedup_metrics = calculate_speedup_metrics(parse_benches, nom_benches)

    memory_metrics = calculate_memory_metrics(memory_data)

    # Format and display
    summary_lines = format_summary_lines(
        avg_throughput, time_metrics, speedup_metrics, memory_metrics
    )

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
    f: TextIO,
    parse_benches: dict,
    nom_benches: dict,
    memory_data: dict | None = None,
) -> None:
    """Write report summary section."""
    f.write("## Summary\n\n")

    # Average throughput (if available)
    if parse_benches:
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

    # Memory overhead
    if memory_data:
        overheads = [data["overhead"] for data in memory_data.values()]
        if overheads:
            avg_overhead = sum(overheads) / len(overheads)
            f.write(f"- **Memory overhead**: {avg_overhead:.2f}x average\n")

    f.write("\n")


def write_performance_table(
    f: TextIO, parse_benches: dict, nom_benches: dict
) -> None:
    """Write parse performance table."""
    if not parse_benches:
        return

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


def write_memory_table(f: TextIO, memory_data: dict) -> None:
    """Write memory usage table to report."""
    if not memory_data:
        return

    f.write("## Memory Usage\n\n")
    f.write("| Entries | Input Size | Peak Memory | Overhead |\n")
    f.write("|---------|------------|-------------|----------|\n")

    for entries in sorted(memory_data.keys()):
        data = memory_data[entries]
        input_size = format_memory(data["input_size"])
        peak_memory = format_memory(data["peak"])
        overhead = data["overhead"]

        f.write(
            f"| {entries:,} | {input_size} | {peak_memory} | {overhead:.2f}x |\n"
        )

    f.write("\n")


def save_report(
    results: dict[str, float], memory_data: dict | None = None
) -> None:
    """Save results to markdown report."""
    report_dir = Path("benchmarks/reports")
    report_dir.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.now(UTC)
    report_file = (
        report_dir / f"report_{timestamp.strftime('%Y%m%d_%H%M%S')}.md"
    )

    parse_benches = {}
    ops_benches = {}
    nom_benches = {}

    if results:
        parse_benches, ops_benches, nom_benches, _ = parse_benchmark_results(
            results
        )

    with report_file.open("w") as f:
        write_report_header(f, timestamp)
        write_report_summary(f, parse_benches, nom_benches, memory_data)

        if parse_benches:
            write_performance_table(f, parse_benches, nom_benches)

        # Add memory table if available
        if memory_data:
            write_memory_table(f, memory_data)

        # Raw data
        f.write("## Raw Results\n\n")
        f.write("<details>\n<summary>Click to expand</summary>\n\n")
        f.write("```json\n")

        # Include memory data in raw results
        all_results = {}
        if results:
            all_results["criterion"] = results
        if memory_data:
            all_results["memory"] = memory_data

        json.dump(all_results, f, indent=2, sort_keys=True)
        f.write("\n```\n\n</details>\n")

    console.print(f"\n[dim]Report saved: {report_file}[/dim]")

    # Create latest symlink
    latest = report_dir / "latest.md"
    if latest.exists() or latest.is_symlink():
        latest.unlink()
    latest.symlink_to(report_file.name)


def display_header() -> None:
    """Display the benchmark report header."""
    console.print(
        Panel.fit(
            "[bold cyan]BibTeX Parser Benchmark Report[/bold cyan]\n"
            "[dim]Phase 1.1 - Performance Baseline[/dim]",
            border_style="cyan",
        )
    )
    console.print()


def process_criterion_results(results: dict) -> tuple[dict, dict, dict, dict]:
    """Process criterion benchmark results and display them."""
    if not results:
        return {}, {}, {}, {}

    parse_benches, ops_benches, nom_benches, memory_benches = (
        parse_benchmark_results(results)
    )

    if parse_benches:
        show_parse_performance(parse_benches, nom_benches)
    if ops_benches:
        show_operations_performance(ops_benches)
    if memory_benches:
        show_memory_time_performance(memory_benches)

    return parse_benches, ops_benches, nom_benches, memory_benches


def main() -> None:
    """Generate benchmark report from existing criterion results."""
    display_header()

    # Load results
    results = load_results()
    if results:
        console.print(
            f"[green]Found {len(results)} benchmark results[/green]\n"
        )

    # Debug mode: show all loaded benchmarks
    if "--debug" in sys.argv and results:
        console.print("[dim]Loaded benchmarks:[/dim]")
        for name, time in sorted(results.items()):
            console.print(f"  {name}: {format_time(time)}")
        console.print()

    # Process criterion results
    parse_benches, _, nom_benches, _ = process_criterion_results(results)

    # Run memory profiling if requested
    memory_data = {}
    if "--memory" in sys.argv:
        memory_data = run_memory_profiling()
        if memory_data:
            show_memory_performance(memory_data)

    # Show summary if we have any data
    if parse_benches or nom_benches or memory_data:
        show_summary(parse_benches, nom_benches, memory_data)

    # Save report
    save_report(results, memory_data)


if __name__ == "__main__":
    main()
