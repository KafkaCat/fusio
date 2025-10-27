# Comprehensive Configuration Sweep Guide

## Overview

The comprehensive sweep test runs 96 different configuration combinations to analyze the relationship between system parameters and precondition failure rates in the fusio-manifest library.

## Configuration Space

The test explores all combinations of:

- **num_writers**: [2, 3, 4] - Number of concurrent writers
- **writer_rate**: [0.05, 0.1, 0.15, 0.2] - Operations per second per writer
- **key_overlap_ratio**: [0.1, 0.2, 0.3, 0.4] - Ratio of overlapping keys between writers
- **num_readers**: [4] - Number of concurrent readers (fixed)
- **reader_rate**: [5.0, 6.0] - Read operations per second per reader

**Total configurations**: 3 × 4 × 4 × 1 × 2 = **96 test cases**

## Test Execution

### Single Command Execution

```bash
cd fusio-manifest
cargo test --test performance_test test_comprehensive_sweep -- --ignored --nocapture
```

This single command will:
1. Generate all 96 configurations
2. Run tests in batches of 8 parallel executions
3. Export results to `comprehensive_sweep.csv`
4. Automatically generate visualization plots
5. Print top 5 best and worst configurations

### Expected Duration

- **Test duration**: 60 seconds per test
- **Parallel execution**: 8 concurrent tests
- **Total batches**: 12 (96 ÷ 8)
- **Estimated time**: **20-25 minutes**

### Output Files

After completion, you'll find:

1. `comprehensive_sweep.csv` - All test results with detailed metrics
2. `comprehensive_sweep.png` - Multi-panel visualization (7 plots)

## CSV Format

The exported CSV contains the following columns:

### Configuration Columns
- `config_label` - Short identifier (e.g., "W2_WR0.10_O0.2_RD4_RT5")
- `num_writers` - Number of concurrent writers
- `num_readers` - Number of concurrent readers
- `writer_rate` - Write operations per second per writer
- `reader_rate` - Read operations per second per reader
- `key_pool_size` - Total number of keys (fixed at 100)
- `key_overlap_ratio` - Overlap ratio between writer key sets
- `max_retry_count` - Maximum retries on precondition failure
- `duration_secs` - Test duration (60 seconds)

### Metrics Columns
- `precondition_failure_rate` - **Key metric**: Ratio of precondition failures
- `write_tps` - Successful write throughput
- `read_tps` - Read throughput
- `write_p50_ms` / `write_p95_ms` / `write_p99_ms` - Write latency percentiles
- `precond_p50_ms` / `precond_p99_ms` - Precondition failure latency
- `read_p50_ms` / `read_p95_ms` / `read_p99_ms` - Read latency percentiles
- `avg_retry_count` - Average number of retries per operation

## Visualization

### Automatic Generation

The test automatically calls `python3 plot_results.py comprehensive_sweep.csv` to generate visualizations.

### Manual Generation

If automatic generation fails:

```bash
python3 plot_results.py comprehensive_sweep.csv
```

### Plot Panels

The comprehensive visualization includes 7 panels:

1. **Failure Rate vs Writer Rate** - Scatter plot grouped by num_writers
2. **Write TPS vs Number of Writers** - Scatter plot grouped by overlap ratio
3. **Failure Rate Heatmap** - Color-coded heatmap (writers × overlap)
4. **Top 10 Best Configurations** - Horizontal bar chart of lowest failure rates
5. **Failure Rate vs Overlap** - Grouped by reader rate
6. **TPS vs Failure Rate** - Color-coded by number of writers
7. **Latency vs Failure Rate** - Color-coded by writer rate

## Interpreting Results

### Key Metrics to Watch

1. **Precondition Failure Rate < 10%** - Acceptable threshold
2. **Write TPS** - Higher is better (but watch failure rate)
3. **P99 Latency** - Should stay reasonable even with failures

### Expected Patterns

- **Higher writer rate** → Higher failure rate
- **Higher overlap ratio** → Higher failure rate
- **More writers** → Higher failure rate
- **Reader rate** → Minimal impact (readers don't cause conflicts)

### Example Output

```
=== Running Comprehensive Configuration Sweep ===
Total configurations: 96
Duration per test: 60 seconds
Parallel execution: 8 concurrent tests
Estimated total time: 20-25 minutes

Batch 1/12 completed in 62.3s (8/96 tests, 8.3%) - ETA: 22 min
Batch 2/12 completed in 61.8s (16/96 tests, 16.7%) - ETA: 20 min
...
Batch 12/12 completed in 63.1s (96/96 tests, 100.0%)

✅ All tests completed in 23 minutes 12 seconds
Successfully completed: 96/96 tests
✅ Results exported to comprehensive_sweep.csv

Generating visualizations...
✅ Plots generated successfully

=== Top 5 Best Configs (Lowest Failure Rate) ===
1. W2_WR0.05_O0.1_RD4_RT5: 2.34% failure rate, 0.19 TPS
2. W2_WR0.05_O0.1_RD4_RT6: 2.45% failure rate, 0.18 TPS
3. W2_WR0.05_O0.2_RD4_RT5: 4.12% failure rate, 0.20 TPS
4. W2_WR0.10_O0.1_RD4_RT5: 5.67% failure rate, 0.38 TPS
5. W2_WR0.10_O0.1_RD4_RT6: 5.89% failure rate, 0.37 TPS

=== Top 5 Worst Configs (Highest Failure Rate) ===
1. W4_WR0.20_O0.4_RD4_RT6: 85.23% failure rate, 0.18 TPS
2. W4_WR0.20_O0.4_RD4_RT5: 84.67% failure rate, 0.19 TPS
3. W4_WR0.20_O0.3_RD4_RT6: 79.12% failure rate, 0.22 TPS
4. W4_WR0.15_O0.4_RD4_RT6: 76.45% failure rate, 0.24 TPS
5. W4_WR0.15_O0.4_RD4_RT5: 75.89% failure rate, 0.25 TPS
```

## Troubleshooting

### Test Failures

If individual tests fail:
- Check AWS credentials in `~/.aws/credentials`
- Verify S3 bucket accessibility: `liguoso-tonbo-s3`
- Check network connectivity
- Review test logs for specific errors

### Plot Generation Failures

If plots don't generate:

```bash
# Install required Python packages
pip install pandas matplotlib seaborn numpy

# Generate plots manually
python3 plot_results.py comprehensive_sweep.csv
```

### S3 Rate Limiting

If you encounter S3 rate limiting:
- Reduce `PARALLEL_LIMIT` from 8 to 4 in the test code
- Add delays between batches
- Use a different S3 prefix per test run

## Advanced Usage

### Customizing Configuration Space

Edit `fusio-manifest/tests/perf_test/utils.rs`:

```rust
pub fn generate_all_configs() -> Vec<WorkloadConfig> {
    let num_writers_values = [2, 3, 4, 5];  // Add more values
    let writer_rate_values = [0.05, 0.1, 0.15, 0.2, 0.25];  // Add 0.25
    // ... modify other parameters
}
```

### Changing Test Duration

Modify the duration in `generate_all_configs()`:

```rust
duration: Duration::from_secs(30),  // Reduce from 60 to 30 seconds
```

### Adjusting Parallelism

In `test_comprehensive_sweep()`:

```rust
const PARALLEL_LIMIT: usize = 4;  // Reduce from 8 to 4
```

## Next Steps

After analyzing results:

1. **Identify optimal configurations** - Lowest failure rate with acceptable TPS
2. **Set production parameters** - Based on findings
3. **Document thresholds** - When failure rate becomes unacceptable
4. **Run long-duration tests** - Validate chosen configs over 10-30 minutes
5. **Test under chaos** - Add network latency, CPU stress (Phase 4)

## Related Tests

- `test_baseline` - Quick 2-minute baseline test
- `test_overlap_sweep` - Focused overlap ratio analysis
- Future: `test_num_writers_sweep`, `test_writer_tps_sweep`
