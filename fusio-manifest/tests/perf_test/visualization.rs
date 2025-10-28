use std::{fs::File, path::Path};

use csv::Writer;

use crate::perf_test::{
    metrics::MetricsSummary,
    utils::{create_config_label, WorkloadConfig},
};

pub fn export_results_csv(
    filename: &str,
    results: &[(WorkloadConfig, MetricsSummary)],
) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(filename);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut wtr = Writer::from_writer(File::create(filename)?);

    wtr.write_record(&[
        "config_label",
        "num_writers",
        "num_readers",
        "writer_rate",
        "reader_rate",
        "key_pool_size",
        "key_overlap_ratio",
        "max_retry_count",
        "duration_secs",
        "precondition_failure_rate",
        "write_tps",
        "read_tps",
        "write_p50_ms",
        "write_p95_ms",
        "write_p99_ms",
        "precond_p50_ms",
        "precond_p99_ms",
        "read_p50_ms",
        "read_p99_ms",
        "avg_retry_count",
        "total_retry_failures",
        "total_max_retries_exceeded",
        "retry_failure_rate",
        "retry_success_rate",
    ])?;

    for (config, summary) in results {
        wtr.write_record(&[
            create_config_label(config),
            config.num_writers.to_string(),
            config.num_readers.to_string(),
            config.writer_rate.to_string(),
            config.reader_rate.to_string(),
            config.key_pool_size.to_string(),
            config.key_overlap_ratio.to_string(),
            config.max_retry_count.to_string(),
            config.duration.as_secs().to_string(),
            format!("{:.4}", summary.precondition_failure_rate),
            format!("{:.2}", summary.write_tps),
            format!("{:.2}", summary.read_tps),
            format!("{:.2}", summary.write_p50_ms),
            format!("{:.2}", summary.write_p95_ms),
            format!("{:.2}", summary.write_p99_ms),
            format!("{:.2}", summary.precond_failure_latency_p50_ms),
            format!("{:.2}", summary.precond_failure_latency_p99_ms),
            format!("{:.2}", summary.read_p50_ms),
            format!("{:.2}", summary.read_p99_ms),
            format!("{:.2}", summary.avg_retry_count),
            summary.total_retry_failures.to_string(),
            summary.total_max_retries_exceeded.to_string(),
            format!("{:.4}", summary.retry_failure_rate),
            format!("{:.4}", summary.retry_success_rate),
        ])?;
    }

    wtr.flush()?;
    println!("\n✅ Results exported to {}", filename);
    Ok(())
}

pub fn export_single_result_csv(
    filename: &str,
    config: &WorkloadConfig,
    summary: &MetricsSummary,
) -> Result<(), Box<dyn std::error::Error>> {
    export_results_csv(filename, &[(config.clone(), summary.clone())])
}
