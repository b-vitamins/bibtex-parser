# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a high-performance BibTeX parser written in Rust, focusing on zero-copy parsing and memory efficiency. It achieves ~700 MB/s throughput with minimal memory overhead (0.94x-1.08x of input size).

**Requirements**: Rust 1.75+ (specified in Cargo.toml)

## Infrastructure Imports

@~/.claude/instructions/python-development.md
@~/.claude/instructions/guix-workflow.md
@~/.claude/instructions/code-quality.md
@~/.claude/instructions/agent-chains.md

## Common Development Commands

All commands should be run within the Guix environment:
```bash
guix shell -m manifest.scm --
```

### Build & Test
```bash
# Build the library
guix shell -m manifest.scm -- cargo build --release

# Run all tests
guix shell -m manifest.scm -- cargo test

# Run a specific test
guix shell -m manifest.scm -- cargo test test_name

# Run tests with output
guix shell -m manifest.scm -- cargo test -- --nocapture

# Run performance comparison test
guix shell -m manifest.scm -- cargo test test_vldb_performance -- --nocapture
```

### Benchmarking
```bash
# Run performance benchmarks
guix shell -m manifest.scm -- cargo bench --bench performance

# Run memory benchmarks
guix shell -m manifest.scm -- cargo bench --bench memory

# Run delimiter benchmarks
guix shell -m manifest.scm -- cargo bench --bench delimiter

# Run parallel benchmarks (requires parallel feature)
guix shell -m manifest.scm -- cargo bench --features parallel --bench parallel

# Generate comprehensive benchmark report
guix shell -m manifest.scm -- python scripts/benchmark.py
```

### Linting & Type Checking
```bash
# Format code
guix shell -m manifest.scm -- cargo fmt

# Check for linting issues
guix shell -m manifest.scm -- cargo clippy

# Type check
guix shell -m manifest.scm -- cargo check
```

### Development Tools
```bash
# Run diagnostic tool
guix shell -m manifest.scm -- cargo run --bin diagnose -- path/to/file.bib

# Profile parser performance
guix shell -m manifest.scm -- cargo run --bin profile_parser -- path/to/file.bib

# Analyze patterns in BibTeX files
guix shell -m manifest.scm -- cargo run --bin analyze_patterns -- path/to/file.bib

# Check SIMD optimization potential
guix shell -m manifest.scm -- cargo run --bin simd_potential -- path/to/file.bib

# Test fixtures
guix shell -m manifest.scm -- cargo run --bin test_fixtures

# Allocation tracing
guix shell -m manifest.scm -- cargo run --bin tracealloc -- path/to/file.bib
```

### Example Usage
```bash
# Run basic usage example
guix shell -m manifest.scm -- cargo run --example basic

# Run query operations example
guix shell -m manifest.scm -- cargo run --example query
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

## Specialized Agent Integration

### BibTeX-Specific Agents

- **bibtex-citation-checker**: Validates BibTeX syntax, required fields, and citation consistency
- **bibtex-entry-enricher**: Enhances BibTeX entries with missing fields, DOIs, and metadata
- **bibtex-duplicate-detector**: Identifies and resolves duplicate entries across files
- **bibtex-formatter**: Standardizes formatting and field ordering
- **bibtex-validator**: Comprehensive validation of BibTeX structure and semantics

### Rust Development Agents

- **rust-clippy-fixer**: Applies Clippy suggestions for idiomatic Rust code
- **cargo-dependency-updater**: Manages dependency updates safely
- **rust-analyzer-helper**: LSP integration and IDE configuration
- **rust-performance-analyzer**: Identifies performance bottlenecks and optimization opportunities
- **rust-security-auditor**: Security vulnerability scanning for dependencies

### Testing Infrastructure Agents

- **rust-unit-test-generator**: Creates comprehensive unit tests for parser components
- **rust-property-test-generator**: Generates property-based tests using proptest
- **rust-integration-test-generator**: Creates end-to-end integration test scenarios
- **test-coverage-analyzer**: Measures and reports test coverage metrics
- **rust-benchmark-analyzer**: Analyzes benchmark results and performance regressions

## Agent Chains for Common Workflows

### BibTeX Processing Workflow
1. **bibtex-citation-checker** - Validate input syntax
2. **bibtex-duplicate-detector** - Remove duplicates
3. **bibtex-entry-enricher** - Enhance metadata
4. **bibtex-formatter** - Standardize output

### Quality Control Pipeline
1. **rust-clippy-fixer** - Fix linting issues
2. **cargo-dependency-updater** - Update dependencies
3. **rust-security-auditor** - Security scan
4. **test-coverage-analyzer** - Verify coverage
5. **rust-benchmark-analyzer** - Performance validation

### Development Cycle Chain
1. **rust-unit-test-generator** - Create missing tests
2. **rust-property-test-generator** - Add property tests
3. **rust-clippy-fixer** - Apply style fixes
4. **rust-performance-analyzer** - Optimize bottlenecks
5. **security-secret-scanner** - Pre-commit security check

### Release Preparation Chain
1. **bibtex-validator** - Validate test fixtures
2. **test-coverage-analyzer** - Ensure 100% critical path coverage
3. **rust-benchmark-analyzer** - Verify performance targets
4. **cargo-dependency-updater** - Final dependency check
5. **git-commit-formatter** - Format release commit

## Proactive Agent Usage

### Automatic Triggers

**On BibTeX File Changes**:
- Run `bibtex-citation-checker` immediately
- Follow with `bibtex-duplicate-detector` if multiple files

**Before Commit**:
- Always run `security-secret-scanner`
- Run `rust-clippy-fixer` for any Rust changes
- Run `git-commit-formatter` for message formatting

**On Performance Concerns**:
- Start with `rust-performance-analyzer` for bottleneck identification
- Use `rust-benchmark-analyzer` to validate improvements
- Chain with `test-coverage-analyzer` to ensure optimizations are tested

**For BibTeX Data Quality**:
- Run `bibtex-entry-enricher` on incomplete entries
- Use `bibtex-formatter` before committing formatted output
- Apply `bibtex-validator` for comprehensive validation

### Context-Specific Recommendations

**Parser Development**:
- Use `rust-property-test-generator` extensively for parser robustness
- Apply `rust-unit-test-generator` for edge cases
- Run `rust-performance-analyzer` after parser changes

**Memory Optimization**:
- Chain `rust-performance-analyzer` → `test-coverage-analyzer`
- Verify struct sizes with custom tests
- Benchmark memory usage patterns

**Error Handling**:
- Use `rust-unit-test-generator` for error path coverage
- Apply `rust-integration-test-generator` for end-to-end error scenarios
- Validate with `test-coverage-analyzer`

## Quality Control Pipeline

### Pre-Commit Checklist
```bash
# 1. Security scan (MANDATORY)
guix shell -m manifest.scm -- security-secret-scanner

# 2. Format and lint
guix shell -m manifest.scm -- cargo fmt
guix shell -m manifest.scm -- rust-clippy-fixer

# 3. Type check and build
guix shell -m manifest.scm -- cargo check
guix shell -m manifest.scm -- cargo build --release

# 4. Test suite
guix shell -m manifest.scm -- cargo test
guix shell -m manifest.scm -- test-coverage-analyzer

# 5. BibTeX validation on test fixtures
guix shell -m manifest.scm -- bibtex-citation-checker tests/fixtures/
guix shell -m manifest.scm -- bibtex-validator tests/fixtures/

# 6. Performance regression check
guix shell -m manifest.scm -- cargo bench
guix shell -m manifest.scm -- rust-benchmark-analyzer
```

### Release Quality Gates

#### Performance Requirements
- Sequential parsing: ≥650 MB/s
- Memory overhead: ≤1.1x input size
- SIMD speedup: ≥1.8x for delimiter finding
- Entry struct size: ≤64 bytes

#### Test Coverage Requirements
- Unit test coverage: ≥95%
- Integration test coverage: ≥90%
- Property test validation for all parser components
- Benchmark stability: <5% variance

#### Code Quality Standards
- Zero Clippy warnings on default lints
- Zero unsafe code blocks without documentation
- All public APIs documented with examples
- Error types implement proper Display/Debug traits

### Continuous Integration Pipeline
```bash
# Stage 1: Basic validation
guix shell -m manifest.scm -- cargo fmt --check
guix shell -m manifest.scm -- cargo clippy -- -D warnings
guix shell -m manifest.scm -- cargo check

# Stage 2: Testing
guix shell -m manifest.scm -- cargo test
guix shell -m manifest.scm -- cargo test --features parallel

# Stage 3: Performance validation
guix shell -m manifest.scm -- cargo bench --bench performance
guix shell -m manifest.scm -- cargo bench --bench memory

# Stage 4: BibTeX-specific validation
guix shell -m manifest.scm -- bibtex-citation-checker tests/fixtures/
guix shell -m manifest.scm -- bibtex-validator tests/fixtures/

# Stage 5: Security and dependency audit
guix shell -m manifest.scm -- cargo audit
guix shell -m manifest.scm -- security-secret-scanner
```

## Development Environment Setup

### Initial Project Setup
```bash
# Clone and enter environment
cd /home/b/projects/bibtex-parser
guix shell -m manifest.scm

# Install development tools (if needed)
cargo install cargo-audit cargo-watch

# Run initial validation
cargo fmt && cargo clippy && cargo test
```

### IDE Integration
- Configure rust-analyzer to use `guix shell -m manifest.scm`
- Set benchmark targets in IDE for performance monitoring
- Configure test discovery for property tests and integration tests

### Agent Integration Workflow
1. Install agents from user's ~/.claude/agents/ directory
2. Configure project-specific agent chains in .claude/workflows/
3. Set up automatic triggers for file changes and commits
4. Integrate performance monitoring with benchmark agents