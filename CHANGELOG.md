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
- **Phase 1.2 Complete** - Memory optimizations achieving <1.5x target
  - Boxing of Concat variant in Value enum (32 → 24 bytes)
  - Vector shrink_to_fit optimization eliminating over-allocation
  - Memory test suite validating struct sizes
  - Realistic test fixtures based on actual academic entries (src/fixtures.rs)
- **Phase 1.3 In Progress** - SIMD Acceleration Analysis (2025-06-09)
  - Created comprehensive profiling tools (`analyze_patterns.rs`, `simd_potential.rs`, `profile_parser.rs`)
  - Collected detailed performance data with perf, flame graphs, and custom analysis
  - Generated pattern distribution analysis for realistic BibTeX files
  - Discovered critical insights about optimization targets
- Memory profiling with custom allocator
  - Tracks peak memory allocation
  - Calculates memory overhead ratio (memory used / input size)
  - Zero-copy efficiency validation
- Diagnostic tools for deep memory analysis
  - `src/bin/memanalysis.rs` - Structure size and allocation analysis
  - `src/bin/tracealloc.rs` - Allocation tracing with backtraces
  - `src/bin/diagnose.rs` - Comprehensive memory diagnostic with field distribution analysis
  - `src/bin/analyze_patterns.rs` - Pattern distribution analysis for SIMD opportunities
  - `src/bin/simd_potential.rs` - SIMD optimization potential estimation
  - `src/bin/profile_parser.rs` - Component-level performance profiling
- Automated benchmark reporting with Python script
  - Rich terminal output with tables and color coding
  - Markdown report generation with historical tracking
  - Support for both performance and memory profiling
  - Updated to show Phase 1.2 completion status
- Optional `nom-bibtex` dependency for comparison benchmarks
- Development environment support with `manifest.scm` for Guix

### Changed
- **BREAKING: Value::Concat now contains Box<Vec<Value>>** instead of Vec<Value>
  - Reduces Value enum size from 32 to 24 bytes (25% reduction)
  - Saves 8 bytes per field value
- **Entry struct fixed from 456 → 64 bytes** (86% reduction)
  - This was the primary cause of memory overhead
  - Saved ~392 bytes per entry
- Added shrink_to_fit calls to eliminate vector over-allocation
  - Applied to entry fields after parsing
  - Applied to database collections after completion
  - Eliminates ~30% typical vector waste
- Memory benchmark now uses realistic data instead of synthetic
  - Based on real entries from NeurIPS, ICML, Physical Review, etc.
  - Average entry size ~600-900 bytes (was ~260 bytes)
  - Shows true memory efficiency of the parser
- Reorganized benchmarks into separate files:
  - `benches/performance.rs` - Basic parsing benchmarks and comparison suite
  - `benches/memory.rs` - Memory profiling benchmarks (now with realistic data)
- Updated implementation strategy based on profiling results
  - Abandoned string interning approach (increased memory by 20-126%!)
  - Abandoned SmallVec approach (caused 2.83x - 5.31x file-specific variability)
  - Focused on fixing structural issues instead of complex optimizations
- **Phase 1.3 Strategy Revision** - SIMD optimization targets updated based on profiling
  - De-prioritized whitespace skipping (average run only 1.4 bytes)
  - Prioritized delimiter finding as primary SIMD target
  - Added field value extraction as secondary target
  - Identified identifier validation as tertiary target

### Fixed
- Zero-copy regression in `database.rs` where string expansion was creating unnecessary owned values
- Parser handling of `%` comments which were being consumed by whitespace skipping
- Memory overhead now within target range (was 2.76x - 5.31x, now 0.75x - 1.14x)
- Benchmark warmup issues causing inconsistent results
  - Added comprehensive process-level warmup
  - Warmed up both bibtex-parser and nom-bibtex before measurements
  - Increased warmup duration to 3 seconds

### Performance
- **Throughput**: 359 MB/s average (improved from 341 MB/s)
- **Speed**: 3.43x faster than nom-bibtex (range: 3.05x - 4.00x)
- Parse 1K entries in 0.9ms (well under 5ms goal) ✓
- Parse 5K entries in 4.2ms (well under 50ms goal) ✓
- **Memory overhead**: 0.75x - 1.14x ✓ (was 2.76x - 5.31x)
  - Small files (10 entries): 1.14x
  - Medium files (50-100 entries): 0.78x  
  - Large files (500-5000 entries): 0.75x - 0.80x
  - Parser now uses LESS memory than input file size for most files!

### Discovered
- **Realistic data shows true memory efficiency**
  - Synthetic benchmark data (260 bytes/entry) was too small
  - Real entries are 500-900 bytes each
  - Larger content dilutes fixed structural overhead
  - Memory target was already achievable, just hidden by unrealistic benchmarks
- **Simple optimizations beat complex schemes**
  - Boxing one enum variant: 25% reduction
  - Shrinking vectors: 30% savings
  - Total impact: 71% memory reduction
  - Much simpler than string interning or SmallVec approaches
- **String interning is counterproductive** for BibTeX parsing
  - Pool overhead (~100KB) exceeds savings for typical files
  - Field names only account for ~200KB even with 4000+ entries
  - Our zero-copy design already prevents string allocations
- **SmallVec causes file-specific performance variability**
  - Files with ≤10 fields per entry: Good performance
  - Files with >10 fields per entry: 400 bytes wasted per entry
  - Causes unpredictable 2.83x - 5.31x overhead
  - Conclusion: Consistency more important than micro-optimization
- **Entry struct was the real problem**
  - Was 456 bytes due to padding/alignment issues
  - Should have been 64 bytes
  - Single biggest source of memory waste
- **Phase 1.3 Profiling Insights** (2025-06-09)
  - **Whitespace patterns unsuitable for SIMD**
    - Average run length only 1.4 bytes in realistic files
    - 17.9% of content is whitespace, but highly fragmented
    - Zero runs ≥16 bytes (SIMD minimum for efficiency)
  - **Delimiter finding is the real bottleneck**
    - 60,000+ delimiters in 1000-entry file
    - Currently requires sequential scanning
    - memchr demonstrates 28x speedup potential
  - **Parser already highly efficient**
    - Current throughput: ~600 MB/s on realistic data
    - Only 15.7µs to parse 10 entries
    - Flame graphs show parsing dominates, not memory allocation
  - **Academic BibTeX files have unique characteristics**
    - Dense format with minimal whitespace
    - Short field names but long values (abstracts)
    - High delimiter density compared to regular text

## [0.1.0] - TBD

### Planned for Release
- **Phase 1**: Performance optimizations
  - [x] Measurement infrastructure ✓
  - [x] Fix structural overhead ✓
  - [ ] SIMD acceleration (revised approach)
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
- [x] Memory overhead: <1.5x file size ✓ (achieved 0.75x - 1.14x)
- [ ] Parse performance: 10x improvement over baseline
- [ ] Zero panics from fuzzing (100M iterations)

---

## Implementation Notes

### Phase 1.1 - Measurement Infrastructure (✅ Complete)
Established comprehensive baseline metrics through:
1. Created benchmark suite comparing with nom-bibtex
2. Built memory profiling infrastructure
3. Developed diagnostic tools for deep analysis
4. Discovered string interning is harmful for this use case
5. Discovered SmallVec causes unacceptable file-specific variability

### Phase 1.2 - Fix Structural Overhead (✅ Complete)
Successfully reduced memory overhead to target levels:

#### 1.2a - Fix Entry Size (✅ Complete)
- **Issue**: Entry struct was 456 bytes instead of 64
- **Solution**: Fixed struct definition and alignment
- **Result**: 456 → 64 bytes (86% reduction)
- **Impact**: Primary fix, saved ~400KB per 1000 entries

#### 1.2b - Optimize Value Enum (✅ Complete)  
- **Issue**: Value enum was 32 bytes due to large Concat variant
- **Solution**: Box the Vec in Concat variant
- **Result**: 32 → 24 bytes (25% reduction)
- **Impact**: Saved 90-280 KB on typical files

#### 1.2c - Eliminate Vector Waste (✅ Complete)
- **Issue**: Vectors over-allocate by ~30% typically
- **Solution**: Call shrink_to_fit() after parsing
- **Result**: 0% wasted capacity
- **Impact**: Saved 100-400 KB on typical files

### Phase 1.3 - SIMD Acceleration (In Progress)
Deep profiling revealed need to revise approach:

#### 1.3a - Profiling Infrastructure (✅ Complete)
- **Tools Created**:
  - `analyze_patterns.rs` - Character distribution and pattern analysis
  - `simd_potential.rs` - SIMD speedup estimation
  - `profile_parser.rs` - Component timing breakdown
  - Shell script for comprehensive data collection
- **Data Collected**:
  - Flame graphs showing parsing hotspots
  - Pattern distribution for 10-1000 entry files
  - CPU performance counters
  - Comparison of realistic vs synthetic data

#### 1.3b - Revised SIMD Strategy
Based on profiling data:

1. **Delimiter Finding** (PRIMARY TARGET)
   - Current: Sequential byte scanning
   - Opportunity: 28x speedup demonstrated by memchr
   - Plan: Multi-delimiter SIMD search for @, {, }, =, ,
   - Impact: Could improve overall performance by 15-25%

2. **Field Value Extraction** (SECONDARY)
   - Current: Byte-by-byte until delimiter
   - Opportunity: SIMD scan for field terminators
   - Plan: Vectorized search for comma/brace in values
   - Impact: Benefits long abstracts and descriptions

3. **Identifier Validation** (TERTIARY)
   - Current: Per-character validation
   - Opportunity: Validate 16 bytes at once
   - Plan: SIMD character class checking
   - Impact: Minor, but consistent improvement

4. **Whitespace Skipping** (DEPRIORITIZED)
   - Finding: Average runs too short (1.4 bytes)
   - Decision: Not worth SIMD complexity
   - Alternative: Optimize delimiter finding instead

Expected combined impact: 20-40% performance improvement

### Key Learnings
1. **Always profile before optimizing** - String interning seemed obvious but made things worse
2. **Check struct sizes first** - Entry being 456 bytes was the root cause
3. **Simple solutions win** - Boxing one variant beat complex schemes
4. **Realistic benchmarks matter** - Synthetic data hid our actual performance
5. **Zero-copy works** - 100% of strings remain borrowed in typical usage
6. **Profile real workloads** - Academic BibTeX has different patterns than expected
7. **SIMD needs the right patterns** - Short runs make vectorization ineffective

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

3. **SIMD Whitespace Skipping** (2025-06-09) [Not implemented, analysis only]
   - Hypothesis: Vectorize whitespace processing for speedup
   - Finding: Average whitespace run only 1.4 bytes
   - Decision: Abandon in favor of delimiter-focused SIMD
   - Lesson: Profile before implementing complex optimizations

### Memory Optimization Results Summary
```
Before: 2.76x - 5.31x overhead (file-dependent)
After:  0.75x - 1.14x overhead (exceeds target!)

Breakdown of improvements:
- Entry struct fix:     ~50% reduction  
- Value enum boxing:    ~15% reduction
- Vector shrinking:     ~10% reduction
- Combined effect:      ~71% total reduction

The parser now uses LESS memory than the input file size for most real-world files!
```
