# Comprehensive Sweep V2 - Full Matrix Testing

## Overview

The comprehensive sweep test now sweeps through **both writer AND reader configurations** to measure latency and precondition failures across a complete configuration matrix. This provides insights into how both read and write workloads scale.

## Configuration Matrix

### Test Parameters
- **Writers**: 1, 2, 3
- **Writer rates**: 0.02, 0.05, 0.1, 0.2 ops/sec
- **Readers**: 3, 4, 5, 6
- **Reader rates**: 100, 200, 300 ops/sec
- **Duration**: 60 seconds per config
- **Exclusion**: W1@0.1 (already tested in baseline)
- **Total configs**: (3 × 4 × 4 × 3) - 12 = **132 configurations**

### Why These Parameters?
- **Writer sweep**: Measure how write contention increases with more writers and higher rates
- **Reader sweep**: Measure how read load affects system latency
- **Full matrix**: Understand interaction between read and write workloads
- **60-second duration**: Balance between statistical significance and total test time

## Running the Test

```bash
cargo test -p fusio-manifest --test performance_test test_comprehensive_sweep -- --ignored --nocapture
```

### Expected Duration
- **Total configs**: 132
- **Parallel execution**: 8 concurrent tests per batch
- **Duration per test**: 60 seconds
- **Estimated total time**: ~17 minutes (132 × 60s ÷ 8 parallel ÷ 60s/min)

### What Happens
1. Test runs 132 configurations in batches of 8
2. Each batch runs in parallel to maximize efficiency
3. Progress is printed for each batch
4. Results exported to `comprehensive_sweep_v2.csv`
5. **Automatic generation of 3 graphs** (with baseline reference lines):
   - Read latency vs configuration (includes baseline p50/p99 horizontal lines)
   - Write latency vs configuration (includes baseline p50/p99 horizontal lines)
   - Precondition failure vs writer configuration (includes baseline 0% line)

## Output Files

### CSV: `comprehensive_sweep_v2.csv`
Contains columns for all 132 configs (W1@0.1 excluded - see test_baseline.csv):
- `config_label`: Full configuration identifier
- `num_writers`, `num_readers`, `writer_rate`, `reader_rate`
- `write_p50_ms`, `write_p95_ms`, `write_p99_ms`: Writer latency percentiles
- `read_p50_ms`, `read_p95_ms`, `read_p99_ms`: Reader latency percentiles
- `precondition_failure_rate`: Percentage of write conflicts
- `write_tps`, `read_tps`: Actual throughput achieved

### Graph 1: `comprehensive_sweep_v2_read_latency.png`
**Purpose**: Show how reader latency changes across all configurations

**Visualization**:
- **X-axis**: Configuration labels sorted by readers first (R3@100, R3@150, ..., R6@300)
- **Y-axis**: Read latency in milliseconds
- **Lines**:
  - Green solid: Reader p50 latency
  - Dark green dashed: Reader p99 latency
  - Green dotted: Baseline p50 (horizontal reference line from test_baseline.csv)
  - Dark green dotted: Baseline p99 (horizontal reference line)
- **Width**: 20 inches to accommodate 132 data points
- **X-tick labels**: Every ~4th config to avoid overlap

**Key Questions**:
1. How does read latency scale with reader count?
2. Does reader rate (100, 200, 300 ops/sec) significantly impact latency?
3. At what reader configuration does latency start degrading?

### Graph 2: `comprehensive_sweep_v2_write_latency.png`
**Purpose**: Show how writer latency changes across all configurations

**Visualization**:
- **X-axis**: Configuration labels sorted by writers first (W1@0.02, W1@0.05, ..., W3@0.20)
- **Y-axis**: Write latency in milliseconds
- **Lines**:
  - Blue solid: Writer p50 latency
  - Dark blue dashed: Writer p99 latency
  - Blue dotted: Baseline p50 (horizontal reference line from test_baseline.csv)
  - Dark blue dotted: Baseline p99 (horizontal reference line)
- **Width**: 20 inches to accommodate 132 data points
- **X-tick labels**: Every ~4th config to avoid overlap
- **Note**: W1@0.1 configs excluded from sweep (see baseline graph for this data)

**Key Questions**:
1. How does write latency scale with writer count and rate?
2. Does reader load impact write latency (due to S3 contention)?
3. What is the optimal writer configuration for acceptable latency?

### Graph 3: `comprehensive_sweep_v2_precondition_failure.png`
**Purpose**: Show precondition failure rate vs writer configuration

**Visualization**:
- **X-axis**: Writer configuration only (W1@0.02, W1@0.05, ..., W3@0.20)
- **Y-axis**: Precondition failure rate (%)
- **Line**: Red with circular markers
- **Baseline**: Red dotted horizontal line at 0% (W1@0.1 from test_baseline.csv)
- **Aggregation**: Averaged across all reader configurations (since readers don't affect write conflicts)
- **Width**: 14 inches
- **Note**: Shows 11 writer configs (W1@0.1 excluded, already in baseline)

**Key Questions**:
1. At what writer count/rate do precondition failures spike?
2. What is the acceptable failure rate threshold?
3. How does retry overhead correlate with failure rate?

## Expected Results

### Single Writer (W1 × 3 rates × 4 readers × 3 reader_rates = 36 configs, excluding W1@0.1)
- **Precondition failures**: 0% (no write contention)
- **Writer latency**: Similar to baseline ~1.2s p50, ~1.5s p99
- **Reader latency**: Varies by reader load (100, 200, 300 ops/sec)
- **Note**: W1@0.1 results available in test_baseline.csv

### Two Writers (W2 × 4 rates × 4 readers × 3 reader_rates = 48 configs)
- **Precondition failures**: 5-15% (moderate contention)
- **Writer latency**: Slight increase due to retries
- **Reader latency**: May show slight degradation under heavy read load

### Three Writers (W3 × 4 rates × 4 readers × 3 reader_rates = 48 configs)
- **Precondition failures**: 15-30% (higher contention)
- **Writer latency**: More noticeable increase
- **Reader latency**: Potential degradation if S3 throttling occurs

### Reader Scaling (per writer config)
- **Low reader count (3 readers)**: Baseline latency
- **Medium reader count (4-5 readers)**: Slight increase
- **High reader count (6 readers)**: Potential S3 read throttling
- **Rate impact**: 300 ops/sec may show higher latency than 100 ops/sec

## Analyzing Results

### Read Latency Analysis
1. **Load individual graph**: `comprehensive_sweep_v2_read_latency.png`
2. **Look for trends**:
   - Does latency increase linearly with reader count?
   - Is there a threshold where latency spikes?
   - Does p99 spread significantly from p50?
3. **Identify optimal reader config**: Best balance of throughput and latency

### Write Latency Analysis
1. **Load individual graph**: `comprehensive_sweep_v2_write_latency.png`
2. **Look for trends**:
   - How much does latency increase from 1 to 3 writers?
   - Does reader load noticeably impact write latency?
   - Where is the p99 latency still acceptable (<5s)?
3. **Identify optimal writer config**: Maximum throughput with acceptable latency

### Precondition Failure Analysis
1. **Load individual graph**: `comprehensive_sweep_v2_precondition_failure.png`
2. **Look for thresholds**:
   - At what writer rate does failure rate exceed 20%?
   - Is there a "knee" in the curve where failures spike?
   - What is the maximum sustainable writer count?

### Cross-Analysis
Compare across all 3 graphs:
- Does write latency correlate with precondition failure rate?
- Do reader configs with high read latency also show high write latency?
- What is the optimal full-system configuration?

## Implementation Details

### Code Changes

**1. Config Generation (utils.rs:175-209)**
```rust
pub fn generate_all_configs_v2() -> Vec<WorkloadConfig> {
    let num_writers_values = [1, 2, 3];
    let writer_rate_values = [0.02, 0.05, 0.1, 0.2];
    let num_readers_values = [3, 4, 5, 6];
    let reader_rate_values = [100.0, 200.0, 300.0];

    // Skip W1@0.1 (already in baseline)
    if num_writers == 1 && writer_rate == 0.1 {
        continue;
    }

    // Nested loops create (3 × 4 × 4 × 3) - 12 = 132 configs
}
```

**2. Batched Execution (performance_test.rs:317-395)**
```rust
const PARALLEL_LIMIT: usize = 8;
let num_batches = (total_configs + PARALLEL_LIMIT - 1) / PARALLEL_LIMIT;

for batch_idx in 0..num_batches {
    // Spawn 8 concurrent tests
    for config_idx in start_idx..end_idx {
        tokio::spawn(async move { /* run test */ });
    }
    // Wait for batch to complete
    futures_util::future::join_all(handles).await;
}
```

**3. Visualization Functions (plot_results.py:319-453)**
```python
def plot_comprehensive_v2_read_latency(csv_file):
    # Sort by readers first: R3@100, R3@150, ...
    df.sort_values(['num_readers', 'reader_rate', 'num_writers', 'writer_rate'])

    # Load baseline and add horizontal reference lines
    if os.path.exists('test_baseline.csv'):
        baseline_df = pd.read_csv('test_baseline.csv')
        ax.axhline(y=baseline_p50, linestyle=':', label='Baseline p50')
        ax.axhline(y=baseline_p99, linestyle=':', label='Baseline p99')

def plot_comprehensive_v2_write_latency(csv_file):
    # Sort by writers first: W1@0.02, W1@0.05, ...
    df.sort_values(['num_writers', 'writer_rate', 'num_readers', 'reader_rate'])

    # Load baseline and add horizontal reference lines
    if os.path.exists('test_baseline.csv'):
        baseline_df = pd.read_csv('test_baseline.csv')
        ax.axhline(y=baseline_p50, linestyle=':', label='Baseline p50')
        ax.axhline(y=baseline_p99, linestyle=':', label='Baseline p99')

def plot_comprehensive_v2_precondition_failure(csv_file):
    # Group by writer config only (readers don't affect this)
    grouped = df.groupby(['num_writers', 'writer_rate']).agg({
        'precondition_failure_rate': 'mean'
    })

    # Add baseline 0% horizontal line
    if os.path.exists('test_baseline.csv'):
        baseline_df = pd.read_csv('test_baseline.csv')
        ax.axhline(y=baseline_rate, linestyle=':', label='Baseline (W1@0.1): 0%')
```

## Manual Graph Generation

If automatic generation fails:

```bash
# Generate all 3 graphs
python3 plot_results.py comprehensive_sweep_v2.csv --comprehensive

# Or generate individually
python3 -c "
import plot_results as pr
pr.plot_comprehensive_v2_read_latency('comprehensive_sweep_v2.csv')
pr.plot_comprehensive_v2_write_latency('comprehensive_sweep_v2.csv')
pr.plot_comprehensive_v2_precondition_failure('comprehensive_sweep_v2.csv')
"
```

## Troubleshooting

### Test takes too long
- **Expected**: ~17 minutes for 132 configs
- **If slower**: Check S3 latency with `aws s3 ls s3://liguoso-tonbo-s3/`
- **If much slower**: Possible S3 throttling, check AWS CloudWatch metrics

### Graphs are unreadable
- **X-axis labels overlap**: This is expected with 132 configs, graphs show every ~4th label
- **Solution**: Zoom in on PNG file or use interactive plotting
- **Alternative**: Filter CSV to specific reader/writer ranges and re-plot

### Baseline reference lines not showing
- **Cause**: test_baseline.csv not found in current directory
- **Solution**: Run baseline test first: `cargo test ... test_baseline`
- **Alternative**: Graphs will still generate, just without reference lines

### Memory issues
- **132 parallel manifests**: Each config creates an S3 manifest
- **Batching helps**: Only 8 concurrent at a time
- **If OOM**: Reduce PARALLEL_LIMIT from 8 to 4

### Precondition failure graph is flat
- **Possible cause**: Not enough write contention
- **Check**: Are writer rates too low? (<0.02 may have 0% failures)
- **Solution**: Increase writer rates or duration if needed

## Next Steps

1. ✅ Run comprehensive sweep: `cargo test ... test_comprehensive_sweep`
2. ✅ Review 3 generated graphs
3. ✅ Identify optimal configurations for:
   - Maximum reader throughput with acceptable latency
   - Maximum writer throughput with acceptable failures
   - Best full-system balance
4. Document findings in performance report
5. Consider focused follow-up sweeps on interesting regions

---

**Status**: ✅ Ready to run

**Prerequisites**: Run baseline test first to get reference data:
```bash
cargo test -p fusio-manifest --test performance_test test_baseline -- --ignored --nocapture
```

**Total runtime**: ~17 minutes (132 configs)

**Command**:
```bash
cargo test -p fusio-manifest --test performance_test test_comprehensive_sweep -- --ignored --nocapture
```

**Expected outputs**:
- `comprehensive_sweep_v2.csv` (132 rows, excluding W1@0.1)
- `comprehensive_sweep_v2_read_latency.png` (with baseline reference lines)
- `comprehensive_sweep_v2_write_latency.png` (with baseline reference lines)
- `comprehensive_sweep_v2_precondition_failure.png` (with baseline 0% line)
