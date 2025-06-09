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
  - `src/bin/diagnose.rs` - Comprehensive memory diagnostic with field distribution analysis
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
  - Abandoned SmallVec approach (caused 2.83x - 5.31x file-specific variability)
  - New focus: Fix oversized Entry struct (456 bytes → 64 bytes)

### Fixed
- Zero-copy regression in `database.rs` where string expansion was creating unnecessary owned values
- Parser handling of `%` comments which were being consumed by whitespace skipping

### Performance
- **Baseline established**: 341 MB/s average throughput
- **3.55x faster** than nom-bibtex (range: 2.96x - 4.01x)
- Parse 1K entries in 0.9ms (well under 5ms goal)
- Parse 5K entries in 5.4ms (well under 50ms goal)
- **Memory overhead**: 2.76x - 5.31x (needs optimization to meet <1.5x goal)

### Discovered
- **String interning is counterproductive** for BibTeX parsing
  - Pool overhead (~100KB) exceeds savings for typical files
  - Field names only account for ~200KB even with 4000+ entries
  - Our zero-copy design already prevents string allocations
- **SmallVec causes file-specific performance variability**
  - ICML 2005 (13 fields): 4.79x overhead - 0% inline storage
  - ICML 2010 (10 fields): 3.74x overhead - 100% inline storage
  - ICML 2012 (8 fields): 2.83x overhead - 100% inline storage
  - ICML 2015 (11-12 fields): 4.98x overhead - 0% inline storage
  - ICML 2020 (10-11 fields): 5.31x overhead - 4.3% inline storage
  - SmallVec<[Field; 10]> is 416 bytes, wastes 400 bytes when heap allocated
- **Entry struct is massively oversized**
  - Current: 456 bytes per entry (should be ~64 bytes)
  - For 1000 entries: 392 KB wasted just on struct padding
  - This is the primary cause of memory overhead
- **Optimal optimization strategy**:
  1. Fix Entry struct size (456 → 64 bytes) for immediate ~50% reduction
  2. Further compact Entry to 40 bytes if needed
  3. Skip complex optimizations (interning, SmallVec) that add variability

## [0.1.0] - TBD

### Planned
- **Phase 1**: Performance optimizations
  - [x] Measurement infrastructure
  - [ ] Fix structural overhead (Entry size)
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
5. Discovered SmallVec causes unacceptable file-specific variability

### Phase 1.2 - Fix Structural Overhead (Next)
Based on diagnostic analysis of real-world BibTeX files:

#### 1.2a - Fix Entry Size
- Current Entry struct is 456 bytes (!!!)
- Should be 64 bytes with proper Vec<Field>
- Expected impact: ~50% memory reduction immediately
- Rationale: Struct padding/alignment issues are wasting 392 bytes per entry

#### 1.2b - Compact Entry Representation  
- Further reduce Entry from 64 → 40 bytes
- Use u8 for entry type, Box<[Field]> for fields
- Expected impact: Additional 10-15% reduction
- Rationale: Every byte counts when multiplied by thousands of entries

#### 1.2c - Field Name Deduplication (Optional)
- Only 10-15 unique field names per file
- Replace &str with u16 indices
- Expected impact: ~3-5% reduction
- May skip due to complexity vs benefit ratio

### Key Learnings
1. **Always profile before optimizing** - String interning seemed obvious but made things worse
2. **Beware of "clever" optimizations** - SmallVec added complexity and file-specific variability
3. **Check struct sizes** - Entry being 456 bytes was the real problem, not algorithms
4. **Simple solutions win** - Fixing Entry size is simpler and more effective than complex schemes
5. **Zero-copy works** - 100% of strings remain borrowed in typical usage

### Failed Optimization Attempts (Valuable Lessons)
1. **String Interning with lasso** (2024-12-09)
   - Hypothesis: Deduplicate repeated field names and values
   - Result: 20-126% INCREASE in memory usage
   - Why: Pool overhead (~100KB) exceeded savings (~200KB)
   
2. **SmallVec<[Field; 10]>** (2024-12-09)
   - Hypothesis: Avoid heap allocations for typical entries
   - Result: 2.83x - 5.31x overhead depending on field count
   - Why: 416-byte struct, wastes 400 bytes when > 10 fields
   - Lesson: File-specific performance is unacceptable for a parser