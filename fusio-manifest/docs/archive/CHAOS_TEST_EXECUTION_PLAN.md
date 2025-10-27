# Chaos Test Execution Plan (Updated)

## Overview

This document outlines the execution plan for running chaos engineering tests on fusio-manifest. The chaos testing framework has been updated with enhanced retry tracking, parallel execution, and revised chaos scenarios.

## Changes from Original Plan

### Scenario Updates

**Removed:**
- CPU Overload: 2 threads @ 50%
- Network Latency: 50ms

**Added:**
- Network Latency: 500ms (high latency)
- Network Blocking: Random 10-second blocks (3 times during test)

**Updated:**
- Network Latency: Now 100ms, 200ms, 500ms (previously 50ms, 100ms, 200ms)
- Combined scenario: Now 200ms + 4 threads @ 80% (previously 50ms + 2 threads @ 50%)

### Execution Mode Updates

**Before:** Sequential execution (~35 minutes)
- Each scenario ran one after another
- Total time: 7 scenarios × 5 minutes = 35 minutes

**After:** Parallel execution (~5 minutes)
- All 7 scenarios run simultaneously
- Total time: ~5 minutes (fastest scenario completion)
- 7x speedup

### Metrics Enhancement

**New Retry Metrics Added:**
- `total_retry_failures` - Precondition failures that occurred on retry attempts
- `total_max_retries_exceeded` - Operations that gave up after max retries
- `retry_success_rate` - Percentage of initial failures that succeeded after retry
- `retry_failure_rate` - Percentage of precondition failures that failed on retry

**CSV Export:** Updated from 20 to 24 columns

**Visualization:** Added 7th plot for retry effectiveness analysis

## Test Scenarios (7 Total)

### 1. Baseline
**Purpose:** Establish performance baseline without chaos
**Parameters:** No chaos injection
**Expected:** Best performance metrics

### 2. Network Latency - 100ms
**Purpose:** Test moderate network delay impact
**Parameters:** 100ms delay on all S3 operations
**Expected:** Moderate latency increase, slight TPS reduction

### 3. Network Latency - 200ms
**Purpose:** Test significant network delay impact
**Parameters:** 200ms delay on all S3 operations
**Expected:** 2x latency increase vs 100ms, moderate TPS reduction

### 4. Network Latency - 500ms
**Purpose:** Test severe network delay impact
**Parameters:** 500ms delay on all S3 operations
**Expected:** 5x latency increase vs baseline, significant TPS reduction

### 5. Network Blocking
**Purpose:** Test intermittent network outages
**Parameters:**
- 10-second network blocks
- 3 random occurrences during test
- Blocks happen at random intervals (30-90 seconds apart)
**Expected:**
- Temporary TPS drops to zero during blocks
- High retry failures during blocking periods
- Recovery after unblock

### 6. CPU Overload - 4 threads @ 80%
**Purpose:** Test CPU contention impact
**Parameters:** 4 threads consuming 80% CPU each
**Expected:**
- Slower request processing
- Increased latency
- Potential retry timeouts

### 7. Combined - 200ms + 4 threads @ 80%
**Purpose:** Test worst-case scenario with multiple chaos factors
**Parameters:** Both 200ms network latency AND 4 threads @ 80% CPU
**Expected:**
- Cumulative degradation from both factors
- Highest precondition failure rate
- Lowest TPS
- Most retry failures

## Test Configuration

### Workload Parameters
These parameters are loaded from the best configuration found in `comprehensive_sweep.csv`:

**Expected Best Config:**
- num_writers: 4-8
- num_readers: 4-8
- writer_rate: 1-5 ops/sec
- reader_rate: 10-50 ops/sec
- key_pool_size: 100-500
- key_overlap_ratio: 0.3-0.7
- max_retry_count: 1
- value_size: 1KB

### Test Duration
- **Per scenario:** 5 minutes (300 seconds)
- **Total time (parallel):** ~5 minutes
- **Total time (sequential):** ~35 minutes

### Execution Mode
**Parallel execution enabled** - All 7 scenarios run simultaneously using `tokio::spawn`

## Prerequisites

### 1. AWS Credentials
Ensure AWS credentials are configured:

```bash
export AWS_ACCESS_KEY_ID=<your-key-id>
export AWS_SECRET_ACCESS_KEY=<your-secret-key>
export AWS_REGION=ap-southeast-1
export FUSIO_MANIFEST_BUCKET=liguoso-tonbo-s3
```

### 2. Comprehensive Sweep Completed
The chaos test requires `comprehensive_sweep.csv` to exist:

```bash
# If not already run, execute:
cargo test -p fusio-manifest --test performance_test test_comprehensive_sweep -- --ignored --nocapture
```

This will:
- Run 96 configuration combinations
- Take ~20-25 minutes
- Generate `comprehensive_sweep.csv` with best config

### 3. Python Dependencies
For visualization:

```bash
pip3 install pandas matplotlib seaborn
```

## Execution Steps

### Step 1: Run Chaos Sweep Test

```bash
# Run all 7 scenarios in parallel (~5 minutes)
cargo test -p fusio-manifest --test performance_test test_chaos_sweep -- --ignored --nocapture
```

**What happens:**
1. Loads best configuration from `comprehensive_sweep.csv`
2. Creates 7 chaos scenarios with revised parameters
3. Spawns 7 parallel test executions using `tokio::spawn`
4. Each scenario:
   - Creates isolated S3 prefix (e.g., `perf-test-chaos-baseline-1729699200/`)
   - Initializes chaos controller
   - Runs workload with writers and readers
   - Tracks all writes and reads
   - Verifies serializable isolation
   - Reports metrics
5. Waits for all scenarios to complete using `futures_util::join_all`
6. Exports results to `chaos_sweep.csv`
7. Prints summary table

**Expected Output:**
```
=== Running Chaos Sweep (7 scenarios in parallel = ~5 minutes) ===

[1/7] Starting scenario: baseline
[2/7] Starting scenario: net-delay-100ms
[3/7] Starting scenario: net-delay-200ms
[4/7] Starting scenario: net-delay-500ms
[5/7] Starting scenario: net-block-10s-3x
[6/7] Starting scenario: cpu-4threads-80pct
[7/7] Starting scenario: combined-200ms-4threads-80pct

... (parallel execution logs) ...

🔴 Network blocking event 1/3 - blocking for 10s
🟢 Network blocking event 1/3 - unblocked

... (more logs) ...

=== Enhanced Verification Complete ===
✅ All successful writes accounted for
✅ Readers only observed committed data (causal consistency)
✅ Monotonic reads verified for all readers

=== Chaos Sweep Results Summary ===

Scenario                               Failure%   WriteTPS   P99(ms)   RetrySucc%   RetryFail%
------------------------------------------------------------------------------------------------------
1. baseline                              5.23%     12.45     234.5      85.2%         8.1%
2. net-delay-100ms                       6.78%     11.23     456.7      82.3%        11.2%
3. net-delay-200ms                       8.92%      9.87     789.3      78.5%        15.8%
4. net-delay-500ms                      12.34%      7.45    1456.2      71.2%        23.4%
5. net-block-10s-3x                     18.56%      6.23    2345.6      65.8%        32.1%
6. cpu-4threads-80pct                    9.45%      8.92     987.4      76.3%        18.2%
7. combined-200ms-4threads-80pct        15.67%      5.67    1987.3      68.9%        28.5%

✅ Results exported to chaos_sweep.csv
```

**Generated Files:**
- `chaos_sweep.csv` - Raw metrics data (24 columns × 7 rows)

### Step 2: Generate Visualization

```bash
# Create comprehensive chaos plot with 7 charts
python3 plot_results.py chaos_sweep.csv --chaos
```

**What happens:**
1. Loads `chaos_sweep.csv` with 24 columns including retry metrics
2. Creates 3×3 grid layout (7 plots total)
3. Generates 7 comprehensive charts:
   - **Chart 1:** Failure Rate Under Chaos (bar chart)
   - **Chart 2:** Write Throughput Under Chaos (bar chart)
   - **Chart 3:** Write Latency Distribution (p50/p95/p99 lines)
   - **Chart 4:** Failure Rate Degradation vs Baseline (bar chart)
   - **Chart 5:** TPS Degradation vs Baseline (bar chart)
   - **Chart 6:** Read Latency Distribution (p50/p99 lines)
   - **Chart 7:** Retry Effectiveness Analysis (NEW - success vs failure bars)
4. Saves to `chaos_sweep.png` (20×12 inches, 300 DPI)

**Generated Files:**
- `chaos_sweep.png` - Comprehensive visualization

### Step 3: Analyze Results

#### 3.1 Review Console Output
Check for:
- ✅ Serializable isolation verification passed for all scenarios
- ⚠️ Any isolation violations (should be zero)
- 📊 Precondition failure rates (should be reasonable, <20% even under chaos)
- 🔄 Retry effectiveness (success rate should be >50%)

#### 3.2 Examine CSV Data
Open `chaos_sweep.csv` and analyze:

**Key Metrics to Check:**
- `precondition_failure_rate` - Should increase under chaos but stay manageable
- `write_tps` - Expected to decrease under chaos
- `write_p99_ms` - Will increase with latency/CPU chaos
- `retry_success_rate` - Should be >50%, ideally >70%
- `retry_failure_rate` - Should be <30%, ideally <20%
- `total_max_retries_exceeded` - Should be low (indicates giving up)

**Comparison Analysis:**
```bash
# Quick analysis using command line
cat chaos_sweep.csv | column -t -s,
```

#### 3.3 Review Visualization
Open `chaos_sweep.png` and look for:

**Chart 1 - Failure Rate:**
- Baseline should be lowest
- Network blocking and combined should be highest
- No scenario should exceed 20-30% failure rate

**Chart 2 - Write TPS:**
- Baseline should be highest
- Degradation should be proportional to chaos severity
- Combined should be lowest but not zero

**Chart 3 - Write Latency:**
- p99 should increase linearly with network latency
- CPU chaos should show moderate increase
- All scenarios should have p50 < p95 < p99

**Chart 4 - Failure Rate Degradation:**
- Shows % increase vs baseline
- Network blocking should show highest degradation
- Helps identify which chaos factors impact failure rate most

**Chart 5 - TPS Degradation:**
- Shows % decrease vs baseline
- All values should be negative (degradation)
- Helps quantify throughput impact

**Chart 6 - Read Latency:**
- Should follow similar pattern to write latency
- Reads should be faster than writes
- Network chaos should impact reads similarly

**Chart 7 - Retry Effectiveness (NEW):**
- Green bars (success rate) should be higher than red bars (failure rate)
- Baseline should have highest retry success rate
- Network blocking should have lowest retry success rate
- Identifies which chaos conditions make retries ineffective

## Expected Results

### Performance Degradation

| Scenario | Failure Rate | TPS | Write P99 | Retry Success |
|----------|-------------|-----|-----------|---------------|
| Baseline | 5-8% | 100% | Baseline | 80-90% |
| 100ms latency | 6-10% | 90-95% | +100ms | 75-85% |
| 200ms latency | 8-12% | 80-90% | +200ms | 70-80% |
| 500ms latency | 12-18% | 60-75% | +500ms | 60-75% |
| Network blocking | 15-25% | 50-70% | +200-300ms | 50-70% |
| CPU 4@80% | 9-15% | 70-85% | +150ms | 65-80% |
| Combined | 18-28% | 45-65% | +350ms | 50-70% |

### Isolation Verification

**All scenarios should pass:**
- ✅ No duplicate keys in final state
- ✅ Monotonically increasing transaction IDs
- ✅ All successful writes accounted for
- ✅ Readers only observe committed data
- ✅ Monotonic reads within same reader session

**If any scenario fails verification:**
1. Check logs for specific violation details
2. Examine write/read tracking data
3. Investigate timing of violations vs chaos events
4. May indicate concurrency bug requiring investigation

### Retry Effectiveness Analysis

**Good Retry Behavior:**
- retry_success_rate > 70%
- retry_failure_rate < 20%
- total_max_retries_exceeded < 5% of precondition failures

**Poor Retry Behavior (needs tuning):**
- retry_success_rate < 50%
- retry_failure_rate > 30%
- total_max_retries_exceeded > 10% of precondition failures

**Possible Improvements if Retry is Ineffective:**
- Increase max_retry_count from 1 to 2-3
- Add exponential backoff between retries
- Add jitter to retry timing
- Reduce key_overlap_ratio to decrease contention

## Troubleshooting

### Issue: Test fails with "chaos_sweep.csv not found"
**Cause:** Trying to run chaos test before comprehensive sweep
**Solution:** Run comprehensive sweep first to generate best config

### Issue: Serializable isolation verification fails
**Cause:** Potential concurrency bug or chaos-induced anomaly
**Solution:**
1. Review specific violation in logs
2. Check if violation correlates with chaos event timing
3. Re-run test to confirm reproducibility
4. If reproducible, investigate manifest code for race conditions

### Issue: All scenarios show very high failure rates (>50%)
**Cause:** Configuration is too aggressive
**Solution:**
1. Check best config loaded from CSV
2. Manually reduce writer_rate or increase key_pool_size
3. Reduce key_overlap_ratio

### Issue: Visualization fails with KeyError
**Cause:** Old CSV file missing new retry metric columns
**Solution:** Re-run test to regenerate CSV with all 24 columns

### Issue: Network blocking events don't appear in logs
**Cause:** Test duration too short or blocking timing unlucky
**Solution:**
1. Check scenario label matches "net-block-10s-3x"
2. Verify ChaosController.start() was called
3. Increase test duration if needed

### Issue: Parallel execution uses too much CPU
**Cause:** 7 scenarios + CPU chaos = high system load
**Solution:**
1. Reduce number of parallel scenarios (batch execution)
2. Run scenarios sequentially with `--sequential` flag (if implemented)
3. Run on more powerful machine

## Success Criteria

The chaos test is considered successful if:

1. **All scenarios complete** without panics or crashes
2. **Isolation verification passes** for all 7 scenarios
3. **Precondition failure rates** remain under 30% even under worst chaos
4. **Retry success rates** remain above 50% for most scenarios
5. **System recovers** after chaos (baseline scenario should show good performance)
6. **No data corruption** (no duplicate keys, no lost writes)
7. **CSV and PNG** files generated successfully

## Next Steps After Execution

### 1. Performance Tuning
If results show issues:
- Adjust retry parameters (max_retry_count, backoff)
- Tune workload configuration (rates, pool size, overlap)
- Consider implementing more sophisticated retry strategies

### 2. Production Readiness Assessment
Use results to determine:
- Maximum acceptable network latency
- CPU resource requirements
- Failure rate SLAs
- Retry strategy effectiveness

### 3. Documentation
Update production documentation with:
- Observed behavior under chaos
- Recommended operational parameters
- Known limitations and mitigations

### 4. Continuous Testing
Consider:
- Adding chaos tests to CI/CD pipeline
- Running periodically to catch regressions
- Expanding chaos scenarios based on production incidents

## Files Reference

**Test Implementation:**
- `fusio-manifest/tests/performance_test.rs` - Main test orchestration
- `fusio-manifest/tests/perf_test/chaos.rs` - Chaos scenario definitions
- `fusio-manifest/tests/perf_test/client.rs` - Writer/reader client implementation
- `fusio-manifest/tests/perf_test/workload.rs` - Workload driver
- `fusio-manifest/tests/perf_test/metrics.rs` - Metrics collection and retry tracking
- `fusio-manifest/tests/perf_test/utils.rs` - Configuration utilities
- `fusio-manifest/tests/perf_test/visualization.rs` - CSV export with retry metrics

**Visualization:**
- `fusio-manifest/plot_results.py` - Python plotting script with 7-chart layout

**Documentation:**
- `fusio-manifest/CHAOS_TEST_IMPLEMENTATION_SUMMARY.md` - Implementation details
- `fusio-manifest/CHAOS_TEST_EXECUTION_PLAN.md` - This file
- `fusio-manifest/PHASE4_PROGRESS.md` - Original Phase 4 progress

**Generated Artifacts:**
- `chaos_sweep.csv` - Raw metrics (24 columns × 7 rows)
- `chaos_sweep.png` - Comprehensive visualization (3×3 grid)

## Questions?

For issues or questions:
1. Check troubleshooting section above
2. Review implementation summary document
3. Examine test logs for specific error messages
4. Check AWS credentials and S3 bucket access
