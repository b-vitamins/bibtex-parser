# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive benchmarking infrastructure for performance baseline (Phase 1.1)
  - Parse performance benchmarks for various file sizes (10-5000 entries)
  - Query operation benchmarks (find_by_key, find_by_type, find_by_field)
  - Memory usage patterns benchmarks
  - Comparison benchmarks with `nom-bibtex` parser
- Memory profiling with custom allocator to measure actual heap usage
  - Tracks peak memory allocation
  - Calculates memory overhead ratio (memory used / input size)
  - Zero-copy efficiency validation
- Automated benchmark reporting with Python script
  - Rich terminal output with tables and color coding
  - Markdown report generation with historical tracking
  - Support for both performance and memory profiling
- Optional `nom-bibtex` dependency for comparison benchmarks
- Development environment support with `manifest.scm` for Guix

### Changed
- Reorganized benchmarks into separate files:
  - `benches/performance.rs` - Basic parsing benchmarks and comparison suite
  - `benches/memory.rs` - Memory profiling benchmarks

### Fixed
- Zero-copy regression in `database.rs` where string expansion was creating unnecessary owned values
- Parser handling of `%` comments which were being consumed by whitespace skipping

### Performance
- Baseline established: 341 MB/s average throughput
- 3.55x faster than nom-bibtex (2.96x - 4.01x range)
- Parse 1K entries in 0.9ms (well under 5ms goal)
- Memory overhead: 3.29x (needs optimization to meet <1.5x goal)

## [0.1.0] - TBD

### Planned
- **Phase 1**: Performance optimizations
  - [x] Measurement infrastructure
  - [ ] String interning
  - [ ] SIMD acceleration
  - [ ] Parallel parsing
  - [ ] Memory-mapped files
- **Phase 2**: Features
  - [ ] Streaming parser
  - [ ] Validation framework
  - [ ] LaTeX to Unicode conversion
  - [ ] Serde support
- **Phase 3**: Quality
  - [ ] Fuzzing
  - [ ] Enhanced error messages
  - [ ] Documentation

### Implemented
- Zero-copy BibTeX parser using winnow
- Standard entry types (article, book, inproceedings, etc.)
- String variable expansion
- Comment handling
- Database queries (find_by_key, find_by_type, find_by_field)
- BibTeX writer