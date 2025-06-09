# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Phase 1.1 Complete** - Comprehensive benchmarking infrastructure
  - Parse performance benchmarks for various file sizes (10-5000 entries)
  - Query operation benchmarks (find_by_key, find_by_type, find_by_field)
  - Memory usage patterns benchmarks
  - Comparison benchmarks with `nom-bibtex` parser
- Memory profiling with custom allocator
  - Tracks peak memory allocation
  - Calculates memory overhead ratio (memory used / input size)
  - Zero-copy efficiency validation
- Diagnostic tools for deep memory analysis
  - `src/bin/memanalysis.rs` - Structure size and allocation analysis
  - `src/bin/tracealloc.rs` - Allocation tracing with backtraces
  - `src/bin/diagnose.rs` - Comprehensive memory diagnostic
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
- Updated implementation strategy based on profiling results
  - Abandoned string interning approach (increased memory by 20-126%!)
  - New focus: SmallVec and enum size optimization

### Fixed
- Zero-copy regression in `database.rs` where string expansion was creating unnecessary owned values
- Parser handling of `%` comments which were being consumed by whitespace skipping

### Performance
- **Baseline established**: 341 MB/s average throughput
- **3.55x faster** than nom-bibtex (range: 2.96x - 4.01x)
- Parse 1K entries in 0.9ms (well under 5ms goal)
- Parse 5K entries in 5.4ms (well under 50ms goal)
- **Memory overhead**: 2.76x (needs optimization to meet <1.5x goal)

### Discovered
- **String interning is counterproductive** for BibTeX parsing
  - Pool overhead (~100KB) exceeds savings for typical files
  - Field names only account for ~200KB even with 4000+ entries
  - Our zero-copy design already prevents string allocations
- **Real memory overhead sources** (NeurIPS 2024.bib analysis):
  - Vec over-allocation: 43.8% capacity wasted (1.3MB)
  - Value enum size: 40 bytes vs 24 optimal (567KB overhead)
  - Field name duplication: Minor impact (214KB, only 3%)
- **Optimal optimization strategy**:
  1. SmallVec for Entry fields (21% reduction)
  2. Box large enum variants (9% reduction)
  3. Skip complex optimizations with <5% impact

## [0.1.0] - TBD

### Planned
- **Phase 1**: Performance optimizations
  - [x] Measurement infrastructure
  - [ ] Memory-efficient data structures (SmallVec, boxing)
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
  - [ ] Comprehensive documentation

### Target Metrics
- Memory overhead: <1.5x file size
- Parse performance: 10x improvement over baseline
- Zero panics from fuzzing (100M iterations)

---

## Implementation Notes

### Phase 1.1 - Measurement Infrastructure (Complete)
Established comprehensive baseline metrics through:
1. Created benchmark suite comparing with nom-bibtex
2. Built memory profiling infrastructure
3. Developed diagnostic tools for deep analysis
4. Discovered string interning is harmful for this use case

### Phase 1.2 - Memory Optimizations (Next)
Based on profiling real-world BibTeX files:

#### 1.2a - SmallVec Implementation
- Replace `Vec<Field>` with `SmallVec<[Field; 10]>`
- Expected impact: 1.3MB savings on 2.5MB file (21% reduction)
- Rationale: 90% of entries have ≤9 fields, but Vec allocates capacity 16

#### 1.2b - Box Large Enum Variants  
- Change `Concat(Vec<Value>)` to `Concat(Box<Vec<Value>>)`
- Expected impact: 567KB savings on 2.5MB file (9% reduction)
- Rationale: Concat variant forces entire enum to 40 bytes

#### 1.2c - Field Name Interning (Optional)
- Static array for common field names
- Expected impact: ~200KB savings (3% reduction)
- May skip due to complexity vs benefit ratio

### Key Learnings
1. **Always profile before optimizing** - String interning seemed obvious but made things worse
2. **Structural overhead matters** - Vec over-allocation and enum sizes have huge impact
3. **Zero-copy works** - 100% of strings remain borrowed in typical usage
4. **Simple solutions win** - SmallVec is simpler and more effective than string pools