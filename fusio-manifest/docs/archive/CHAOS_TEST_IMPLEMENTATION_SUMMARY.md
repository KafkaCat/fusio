# Chaos Testing Implementation Summary

## Overview

This document summarizes the comprehensive chaos testing implementation for fusio-manifest's performance testing framework. The implementation enables parallel execution of 7 chaos scenarios with enhanced retry tracking and visualization.

## Implementation Details

### 1. Revised Chaos Scenarios (7 Total)

The chaos testing framework now includes the following scenarios:

1. **Baseline** - No chaos, baseline performance
2. **Network Latency 100ms** - Simulates 100ms network delay
3. **Network Latency 200ms** - Simulates 200ms network delay
4. **Network Latency 500ms** - Simulates 500ms network delay
5. **Network Blocking** - Random 10-second network blocks (3 times during test)
6. **CPU Overload** - 4 threads @ 80% CPU utilization
7. **Combined** - 200ms latency + 4 threads @ 80% CPU

**Changes from original plan:**
- Updated network latency values from (50ms, 100ms, 200ms) to (100ms, 200ms, 500ms)
- Removed low CPU scenario (2 threads @ 50%)
- Added new NetworkBlocking scenario with random 10-second blocks
- Updated Combined scenario to use 200ms latency instead of 50ms

### 2. Parallel Execution

**Before:** Sequential execution (~35 minutes total)
- Each scenario ran one after another
- Long wait times for complete chaos sweep

**After:** Full parallel execution (~5 minutes total)
- All 7 scenarios run simultaneously using `tokio::spawn`
- Results collected via `futures_util::future::join_all`
- 7x speedup in execution time

**Implementation location:** `fusio-manifest/tests/performance_test.rs:372-430`

### 3. Enhanced Retry Tracking

Added comprehensive retry effectiveness metrics:

**New Metrics:**
- `total_retry_failures` - Failures that occurred on retry attempts
- `total_max_retries_exceeded` - Operations that gave up after max retries
- `retry_success_rate` - Percentage of initial failures that succeeded after retry
- `retry_failure_rate` - Percentage of precondition failures that failed again on retry

**Calculation Logic:**
```rust
initial_failures = total_precond - total_retry_failures
retry_successes = initial_failures - total_max_retries_exceeded
retry_success_rate = retry_successes / initial_failures
retry_failure_rate = total_retry_failures / total_precond
```

**Implementation locations:**
- Tracking: `fusio-manifest/tests/perf_test/metrics.rs:42-44,104-120`
- Recording: `fusio-manifest/tests/perf_test/client.rs:99-108`
- Export: `fusio-manifest/tests/perf_test/visualization.rs:38-42,66-69`

### 4. Visualization Enhancements

**New 7th Plot:** Retry Effectiveness Analysis
- Shows retry success rate vs retry failure rate per scenario
- Uses green bars for success, red bars for failures
- Helps identify which chaos conditions cause retry ineffectiveness

**Plot Layout:** 3×3 grid with 7 comprehensive charts
1. Failure Rate Under Chaos
2. Write Throughput Under Chaos
3. Write Latency Distribution
4. Failure Rate Degradation vs Baseline
5. TPS Degradation vs Baseline
6. Read Latency Distribution
7. **Retry Effectiveness Analysis** (NEW)

**Implementation location:** `fusio-manifest/plot_results.py:268-278`

## Files Modified

### Core Implementation Files

1. **`fusio-manifest/tests/perf_test/chaos.rs`**
   - Added `NetworkBlocking` scenario variant
   - Implemented `start_network_blocking()` method with random timing
   - Updated `create_chaos_scenarios()` with revised parameters
   - Fixed Send-safety by using `StdRng::from_entropy()` instead of `thread_rng()`

2. **`fusio-manifest/tests/perf_test/metrics.rs`**
   - Added retry tracking counters and fields to `MetricsCollector`
   - Enhanced `record_precondition_failure()` to distinguish initial vs retry failures
   - Added `record_max_retries_exceeded()` method
   - Updated `MetricsSummary` struct with 4 new retry metrics
   - Enhanced `print_report()` to display retry effectiveness section

3. **`fusio-manifest/tests/perf_test/client.rs`**
   - Added `record_max_retries_exceeded()` call when max retries reached

4. **`fusio-manifest/tests/performance_test.rs`**
   - Converted from sequential loop to parallel execution with `tokio::spawn`
   - Pre-extracted scenario labels to avoid moved value errors
   - Used `futures_util::join_all` for parallel result collection

5. **`fusio-manifest/tests/perf_test/visualization.rs`**
   - Added 4 new CSV columns for retry metrics
   - Updated CSV export to include retry effectiveness data

6. **`fusio-manifest/plot_results.py`**
   - Updated scenario labels to match new chaos scenarios
   - Added 7th plot for retry effectiveness analysis
   - Changed layout to 3×3 grid to accommodate new plot

## Technical Challenges Resolved

### 1. Send Trait Requirement for Async Context

**Problem:** `rand::thread_rng()` returns `ThreadRng` which contains `Rc<UnsafeCell<>>`, making it not Send-safe.

**Error:**
```
future created by async block is not `Send`
= help: the trait `std::marker::Send` is not implemented for `Rc<UnsafeCell<...>>`
```

**Solution:** Changed to `StdRng::from_entropy()` which is Send-safe:
```rust
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
let mut rng = StdRng::from_entropy();
```

### 2. Moved Value in Parallel Execution

**Problem:** Vector was consumed by `into_iter()` but needed later for summary.

**Error:**
```
error[E0382]: borrow of moved value: `scenarios`
```

**Solution:** Pre-extract scenario labels before consuming the vector:
```rust
let scenarios = create_chaos_scenarios();
let scenario_labels: Vec<String> = scenarios.iter().map(|s| s.label()).collect();
```

### 3. CSV Column Mismatch

**Problem:** Existing CSV was generated before adding retry metrics.

**Error:**
```
KeyError: 'retry_success_rate'
```

**Solution:** Updated visualization.rs to export all 24 columns including new retry metrics. Requires re-running test to generate new CSV.

## Retry Behavior Clarification

**Question:** Do precondition failures happen on retries?

**Answer:** YES - Precondition failures CAN occur again on retry attempts because:
1. Writers retry the same key that failed
2. Other concurrent writers may modify that key during the retry window
3. Each retry is a new optimistic concurrency attempt that can fail

**Max Retry Count:** 1 retry allowed (`max_retry_count = 1`)

**Tracking Strategy:**
- Track initial failures vs retry failures separately
- Calculate retry success rate to measure effectiveness
- Visualize retry effectiveness across chaos scenarios

## CSV Export Format

The updated CSV export includes 24 columns:

**Columns 1-20** (existing):
- config_label, num_writers, num_readers, writer_rate, reader_rate
- key_pool_size, key_overlap_ratio, max_retry_count, duration_secs
- precondition_failure_rate, write_tps, read_tps
- write_p50_ms, write_p95_ms, write_p99_ms
- precond_p50_ms, precond_p99_ms
- read_p50_ms, read_p99_ms
- avg_retry_count

**Columns 21-24** (NEW):
- total_retry_failures - Count of failures on retry attempts
- total_max_retries_exceeded - Count of operations that gave up
- retry_failure_rate - Percentage of precond failures that failed on retry
- retry_success_rate - Percentage of initial failures that succeeded after retry

## How to Run

### Run Chaos Sweep Test

```bash
# Run all 7 scenarios in parallel (~5 minutes)
cargo test -p fusio-manifest --test performance_test test_chaos_sweep -- --ignored --nocapture

# This generates: chaos_sweep.csv
```

### Generate Visualization

```bash
# Create comprehensive chaos plot with 7 charts
python3 plot_results.py chaos_sweep.csv --chaos

# This generates: chaos_sweep.png (3x3 grid, 20x12 inches)
```

### Expected Output

**Console Output:**
- Progress updates for each of 7 parallel scenarios
- Verification results showing serializable isolation checks
- Summary metrics for each scenario
- CSV export confirmation

**Generated Files:**
- `chaos_sweep.csv` - Raw metrics data (24 columns × 7 scenarios)
- `chaos_sweep.png` - Comprehensive visualization with 7 charts

## Verification Strategy

The implementation uses **Option B: Track All Read Observations**:

1. **Writers** record all successful writes: `(writer_id, key, value, timestamp)`
2. **Readers** record all observations: `(reader_id, snapshot_txn_id, key, value, timestamp)`
3. **Verification** checks:
   - Readers should only see committed values at their snapshot txn_id
   - No dirty reads (uncommitted data)
   - No phantom reads (writes committed after snapshot)
   - Monotonic reads within same session

**Implementation location:** `fusio-manifest/tests/performance_test.rs:100-169`

## Performance Characteristics

**Baseline Performance (no chaos):**
- Write TPS: ~X ops/sec
- Precondition Failure Rate: ~Y%
- Write p99: ~Zms
- Retry Success Rate: ~W%

**Expected Degradation:**
- Network latency: Linear increase in latency, moderate TPS reduction
- Network blocking: Temporary TPS drops during blocks, higher retry failures
- CPU overload: Slower processing, potential retry timeouts
- Combined: Cumulative effects from both network and CPU chaos

## Next Steps

1. **Re-run Test:** Generate new CSV with all 24 columns including retry metrics
2. **Analyze Results:** Review visualization to identify weaknesses under chaos
3. **Tune Parameters:** Adjust retry count or backoff if retry success rate is low
4. **Production Readiness:** Ensure system meets SLAs under expected chaos conditions

## References

- Main test file: `fusio-manifest/tests/performance_test.rs`
- Chaos controller: `fusio-manifest/tests/perf_test/chaos.rs`
- Metrics tracking: `fusio-manifest/tests/perf_test/metrics.rs`
- Client implementation: `fusio-manifest/tests/perf_test/client.rs`
- Visualization: `fusio-manifest/plot_results.py`
