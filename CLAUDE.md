# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a high-performance BibTeX parser written in Rust, focusing on zero-copy parsing and memory efficiency. It achieves ~700 MB/s throughput with minimal memory overhead (0.94x-1.08x of input size).

**Requirements**: Rust 1.75+ (specified in Cargo.toml)

## Common Development Commands

### Build & Test
```bash
# Build the library
cargo build --release

# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run performance comparison test
cargo test test_vldb_performance -- --nocapture
```

### Benchmarking
```bash
# Run performance benchmarks
cargo bench --bench performance

# Run memory benchmarks
cargo bench --bench memory

# Run delimiter benchmarks
cargo bench --bench delimiter

# Run parallel benchmarks (requires parallel feature)
cargo bench --features parallel --bench parallel

# Generate comprehensive benchmark report
python scripts/benchmark.py
```

### Linting & Type Checking
```bash
# Format code
cargo fmt

# Check for linting issues
cargo clippy

# Type check
cargo check
```

### Development Tools
```bash
# Run diagnostic tool
cargo run --bin diagnose -- path/to/file.bib

# Profile parser performance
cargo run --bin profile_parser -- path/to/file.bib

# Analyze patterns in BibTeX files
cargo run --bin analyze_patterns -- path/to/file.bib

# Check SIMD optimization potential
cargo run --bin simd_potential -- path/to/file.bib

# Test fixtures
cargo run --bin test_fixtures

# Allocation tracing
cargo run --bin tracealloc -- path/to/file.bib
```

### Example Usage
```bash
# Run basic usage example
cargo run --example basic

# Run query operations example
cargo run --example query
```

## Architecture Overview

### Core Components

1. **Parser Architecture** (`src/parser/`)
   - Uses winnow parser combinators for zero-copy parsing
   - SIMD-optimized delimiter finding in `delimiter.rs` (2x performance boost)
   - Modular sub-parsers for entries, fields, values, and comments
   - Smart string variable expansion during parsing

2. **Data Model** (`src/model.rs`)
   - Optimized Entry struct (64 bytes, down from 456)
   - Zero-copy design with borrowed data where possible
   - Efficient hash maps using ahash

3. **Database** (`src/database.rs`)
   - Main API entry point with builder pattern
   - Sequential parsing via `parse()` and `parse_file()`
   - Parallel parsing via `parse_files()` (requires `parallel` feature)
   - Query API for finding entries by key/type

4. **Performance Critical Paths**
   - Delimiter finding uses SIMD via memchr
   - Custom allocator tracking in benchmarks
   - Careful memory layout optimization

### Key Design Decisions

- **Zero-copy where possible**: Parser returns borrowed data to minimize allocations
- **SIMD acceleration**: Used for delimiter finding (@ and string boundaries)
- **Builder pattern**: DatabaseBuilder for configurable parsing options
- **Feature-gated parallelism**: Optional rayon dependency for multi-file parsing
- **Comprehensive benchmarking**: Memory profiling, throughput testing, comparison with other parsers
- **Memory optimization**: Entry struct (64 bytes), Value enum (24 bytes), vector shrinking to minimize waste
- **Error handling**: Uses thiserror with line/column information in all parsing errors

## Testing Strategy

- Unit tests embedded in source files
- Integration tests in `tests/integration_tests.rs`
- Memory optimization tests verify struct sizes remain optimal
- Property-based testing with proptest for parser robustness
- Real-world test fixtures in `tests/fixtures/`

## Current Development Status

Working on Phase 1.5 of the roadmap: implementing parallel single-file parsing. The project is ~70% complete toward v0.1.0 release targeted for Q1 2025.

## Performance Targets

- Sequential parsing: 650-700 MB/s (achieved)
- Memory overhead: <1.1x input size (achieved: 0.94x-1.08x)
- Entry struct size: ≤64 bytes (achieved)
- SIMD speedup: 2x for delimiter finding (achieved)

## API Usage Patterns

### Builder Pattern for Parallel Parsing
```rust
// Multi-file parallel parsing (requires parallel feature)
let db = Database::parser()
    .threads(8)
    .parse_files(&["file1.bib", "file2.bib"])?;

// Note: Single-file parsing is always sequential
let db = Database::parse(bibtex_str)?;
```

### Writer API
The writer supports configurable formatting options for generating BibTeX output.

```rust
use bibtex_parser::writer::{Writer, WriterConfig};

let config = WriterConfig {
    indent: "  ".to_string(),
    align_values: true,
    sort_entries: true,
    ..Default::default()
};

let mut output = Vec::new();
let mut writer = Writer::with_config(&mut output, config);
writer.write_database(&db)?;
```

## Feature Flags

- `parallel`: Enables multi-file parallel parsing with rayon dependency
  - Use for processing multiple BibTeX files efficiently
  - Single-file parsing remains sequential regardless of this flag
  - Required for parallel benchmarks: `cargo bench --features parallel --bench parallel`