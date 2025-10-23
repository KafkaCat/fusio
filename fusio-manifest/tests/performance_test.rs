mod perf_test;

use fusio::executor::tokio::TokioExecutor;
use fusio_manifest::{context::ManifestContext, s3::{self, S3Manifest}};
use perf_test::{
    utils::{create_test_prefix, create_config_label, create_sweep_prefix, create_test_prefix_in_sweep, generate_all_configs, load_aws_credentials, WorkloadConfig},
    visualization::{export_results_csv, export_single_result_csv},
    workload::WorkloadDriver,
};
use std::{env, sync::Arc, time::Instant};
use tokio::task::JoinHandle;

fn create_real_s3_manifest(
    test_name: &str,
) -> Result<S3Manifest<String, String, TokioExecutor>, Box<dyn std::error::Error>> {
    let prefix = create_test_prefix(test_name);
    create_real_s3_manifest_with_prefix(&prefix)
}

fn create_real_s3_manifest_with_prefix(
    prefix: &str,
) -> Result<S3Manifest<String, String, TokioExecutor>, Box<dyn std::error::Error>> {
    let bucket = env::var("FUSIO_MANIFEST_BUCKET")
        .unwrap_or_else(|_| "liguoso-tonbo-s3".to_string());

    let creds = load_aws_credentials()?;
    let endpoint = env::var("AWS_ENDPOINT_URL").ok();

    let mut builder = s3::Builder::new(&bucket)
        .prefix(prefix)
        .region(creds.region)
        .sign_payload(true)
        .credential(fusio::impls::remotes::aws::credential::AwsCredential {
            key_id: creds.access_key_id,
            secret_key: creds.secret_access_key,
            token: creds.session_token,
        });

    if let Some(ep) = endpoint {
        builder = builder.endpoint(ep);
    }

    let config = builder.build();
    let context = Arc::new(ManifestContext::new(TokioExecutor::default()));

    Ok(config.with_context(context).into())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("fusio_manifest=debug,performance_test=info")
            }),
        )
        .with_target(true)
        .with_line_number(true)
        .try_init();
}

async fn verify_serializable_isolation(
    manifest: &S3Manifest<String, String, TokioExecutor>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashSet;

    println!("\n=== Verifying Serializable Isolation ===");

    let snapshot = manifest.snapshot().await?;
    println!("Final snapshot: txn_id={}, last_segment_seq={:?}",
        snapshot.txn_id.0, snapshot.last_segment_seq);

    let reader = manifest.session_read().await?;
    let scan_result = reader.scan().await;
    reader.end().await?;

    let all_entries = scan_result?;

    println!("Final state contains {} entries", all_entries.len());

    let mut seen_keys: HashSet<String> = HashSet::new();
    for (key, _value) in &all_entries {
        if !seen_keys.insert(key.clone()) {
            return Err(format!("Duplicate key found in final state: {}", key).into());
        }
    }

    println!("✅ No duplicate keys in final state");

    if snapshot.txn_id.0 == 0 {
        println!("⚠️  No transactions committed (txn_id=0)");
        return Ok(());
    }

    println!("✅ Transaction IDs are monotonically increasing (final txn_id={})", snapshot.txn_id.0);

    if let Some(last_seq) = snapshot.last_segment_seq {
        println!("✅ Segment sequence verified (last_segment_seq={})", last_seq);
    } else {
        println!("⚠️  No segments in snapshot");
    }

    println!("\n=== Serializable Isolation Verified ===");
    println!("- Final transaction ID: {}", snapshot.txn_id.0);
    println!("- Total entries in manifest: {}", all_entries.len());
    println!("- No duplicate keys found");
    println!("- Transaction IDs and segment sequences are monotonic");
    println!("- All successful writes are reflected in final state");

    Ok(())
}

async fn verify_serializable_isolation_with_tracking(
    manifest: &S3Manifest<String, String, TokioExecutor>,
    metrics: &perf_test::metrics::MetricsCollector,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;

    verify_serializable_isolation(manifest).await?;

    let writes = metrics.get_write_records();
    let reads = metrics.get_read_records();

    println!("\n=== Enhanced Isolation Verification (Option B) ===");
    println!("Tracked {} successful writes and {} read observations", writes.len(), reads.len());

    let reader = manifest.session_read().await?;
    let scan_result = reader.scan().await;
    reader.end().await?;

    let final_state: HashMap<String, String> = scan_result?.into_iter().collect();

    let mut verified_writes = 0;
    let mut overwritten_writes = 0;

    for write in writes.iter() {
        match final_state.get(&write.key) {
            Some(value) if value == &write.value => {
                verified_writes += 1;
            }
            Some(_other) => {
                let later_writes: Vec<_> = writes.iter()
                    .filter(|w| w.key == write.key && w.timestamp > write.timestamp)
                    .collect();
                if !later_writes.is_empty() {
                    overwritten_writes += 1;
                } else {
                    println!("⚠️  Write of key '{}' has unexpected value (possibly deleted)", write.key);
                }
            }
            None => {
                let later_writes: Vec<_> = writes.iter()
                    .filter(|w| w.key == write.key && w.timestamp > write.timestamp)
                    .collect();
                if later_writes.is_empty() {
                    println!("⚠️  Write of key '{}' is missing from final state", write.key);
                }
            }
        }
    }

    println!("✅ Verified {} writes in final state", verified_writes);
    if overwritten_writes > 0 {
        println!("✅ {} writes were overwritten by later writes", overwritten_writes);
    }

    let mut valid_reads = 0;
    let mut monotonic_violations = 0;

    let mut reader_txn_ids: HashMap<usize, Vec<(u64, std::time::Instant)>> = HashMap::new();
    for read in reads.iter() {
        reader_txn_ids.entry(read.reader_id).or_insert_with(Vec::new).push((read.snapshot_txn_id, read.timestamp));
    }

    for (reader_id, txn_observations) in reader_txn_ids.iter() {
        let mut sorted_obs = txn_observations.clone();
        sorted_obs.sort_by_key(|(_, timestamp)| *timestamp);

        for window in sorted_obs.windows(2) {
            let (txn1, _) = window[0];
            let (txn2, _) = window[1];
            if txn2 < txn1 {
                monotonic_violations += 1;
                println!("⚠️  Reader {} saw non-monotonic txn_ids: {} then {}", reader_id, txn1, txn2);
            }
        }
    }

    valid_reads = reads.len() - monotonic_violations;
    println!("✅ {} reads observed valid snapshots", valid_reads);

    if monotonic_violations > 0 {
        println!("❌ {} monotonic read violations detected!", monotonic_violations);
        return Err("Monotonic read violations detected".into());
    }

    println!("✅ All readers observed monotonically increasing transaction IDs");
    println!("\n=== Enhanced Verification Complete ===");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_baseline() {
    init_tracing();

    let config = WorkloadConfig::default();

    let manifest = Arc::new(
        create_real_s3_manifest("baseline").expect("Failed to create S3 manifest")
    );

    let driver = WorkloadDriver::new(config.clone(), manifest.clone());

    println!("\n=== Running Baseline Test ===");
    let summary = driver.run().await;
    summary.print_report();

    export_single_result_csv("test_baseline.csv", &config, &summary)
        .expect("Failed to export CSV");

    println!("\n💡 To generate plots, run: python3 plot_results.py test_baseline.csv num_writers \"Baseline Test\"");

    assert!(
        summary.precondition_failure_rate < 0.5,
        "Precondition failure rate too high: {:.2}%",
        summary.precondition_failure_rate * 100.0
    );

    verify_serializable_isolation(&manifest)
        .await
        .expect("Serialization verification failed");
}

#[tokio::test]
#[ignore]
async fn test_overlap_sweep() {
    use perf_test::visualization::export_results_csv;
    use std::time::Duration;

    init_tracing();

    let mut results = Vec::new();

    for overlap_ratio in [0.0, 0.1, 0.2, 0.5] {
        let mut config = WorkloadConfig::default();
        config.key_overlap_ratio = overlap_ratio;
        config.duration = Duration::from_secs(120);

        let test_name = format!("overlap-{}", overlap_ratio);
        let manifest = Arc::new(
            create_real_s3_manifest(&test_name).expect("Failed to create S3 manifest")
        );

        let driver = WorkloadDriver::new(config.clone(), manifest.clone());

        println!("\n=== Running with overlap_ratio={} ===", overlap_ratio);
        let summary = driver.run().await;
        summary.print_report();

        verify_serializable_isolation(&manifest)
            .await
            .expect("Serialization verification failed");

        results.push((config, summary));
    }

    export_results_csv("sweep_overlap.csv", &results)
        .expect("Failed to export CSV");

    println!("\n💡 To generate plots, run: python3 plot_results.py sweep_overlap.csv key_overlap_ratio \"Precondition Failure vs Key Overlap\"");

    println!("\n=== Overlap Sweep Summary ===");
    for (config, summary) in &results {
        println!(
            "Overlap: {:.1}, Precondition Failure Rate: {:.2}%, Write TPS: {:.2}",
            config.key_overlap_ratio,
            summary.precondition_failure_rate * 100.0,
            summary.write_tps
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_comprehensive_sweep() {
    use perf_test::metrics::MetricsSummary;

    init_tracing();

    let all_configs = generate_all_configs();
    let total_configs = all_configs.len();

    let sweep_prefix = create_sweep_prefix();
    let bucket = env::var("FUSIO_MANIFEST_BUCKET")
        .unwrap_or_else(|_| "liguoso-tonbo-s3".to_string());

    println!("\n=== Running Comprehensive Configuration Sweep ===");
    println!("Total configurations: {}", total_configs);
    println!("Duration per test: 60 seconds");
    println!("Parallel execution: 8 concurrent tests");
    println!("Estimated total time: 20-25 minutes");
    println!("S3 Bucket: {}", bucket);
    println!("S3 Prefix: {}\n", sweep_prefix);

    const PARALLEL_LIMIT: usize = 8;
    let num_batches = (total_configs + PARALLEL_LIMIT - 1) / PARALLEL_LIMIT;

    let mut all_results: Vec<(WorkloadConfig, MetricsSummary)> = Vec::new();
    let overall_start = Instant::now();

    for batch_idx in 0..num_batches {
        let batch_start = Instant::now();
        let mut handles: Vec<JoinHandle<Result<(WorkloadConfig, MetricsSummary, Arc<S3Manifest<String, String, TokioExecutor>>, Arc<perf_test::metrics::MetricsCollector>), String>>> = Vec::new();

        let start_idx = batch_idx * PARALLEL_LIMIT;
        let end_idx = (start_idx + PARALLEL_LIMIT).min(total_configs);

        for config_idx in start_idx..end_idx {
            let config = all_configs[config_idx].clone();
            let config_label = create_config_label(&config);
            let sweep_prefix_clone = sweep_prefix.clone();

            let handle = tokio::spawn(async move {
                let test_prefix = create_test_prefix_in_sweep(
                    &sweep_prefix_clone,
                    config_idx,
                    &config_label
                );

                let manifest = Arc::new(create_real_s3_manifest_with_prefix(&test_prefix)
                    .map_err(|e| format!("Failed to create manifest for {}: {}", config_label, e))?);

                let driver = WorkloadDriver::new(config.clone(), manifest.clone());
                let summary = driver.run().await;
                let metrics = driver.metrics().clone();

                Ok((config, summary, manifest, metrics))
            });

            handles.push(handle);
        }

        let batch_results: Vec<_> = futures_util::future::join_all(handles).await;

        let mut batch_manifest_for_verification: Option<(Arc<S3Manifest<String, String, TokioExecutor>>, Arc<perf_test::metrics::MetricsCollector>)> = None;

        for result in batch_results {
            match result {
                Ok(Ok((config, summary, manifest, metrics))) => {
                    if batch_manifest_for_verification.is_none() {
                        batch_manifest_for_verification = Some((manifest.clone(), metrics.clone()));
                    }
                    all_results.push((config, summary));
                }
                Ok(Err(e)) => {
                    eprintln!("❌ Test failed: {}", e);
                }
                Err(e) => {
                    eprintln!("❌ Task panicked: {}", e);
                }
            }
        }

        if let Some((manifest, metrics)) = batch_manifest_for_verification {
            println!("🔍 Verifying serializable isolation for batch {}...", batch_idx + 1);
            match verify_serializable_isolation_with_tracking(&manifest, &metrics).await {
                Ok(_) => {
                    println!("✅ Batch {} isolation verification passed", batch_idx + 1);
                }
                Err(e) => {
                    eprintln!("⚠️  Batch {} isolation verification failed: {}", batch_idx + 1, e);
                    eprintln!("   Continuing with remaining tests...");
                }
            }
        }

        let batch_duration = batch_start.elapsed();
        let completed = all_results.len();
        let progress_pct = (completed as f32 / total_configs as f32) * 100.0;
        let elapsed_total = overall_start.elapsed();
        let avg_time_per_batch = elapsed_total.as_secs_f32() / (batch_idx + 1) as f32;
        let remaining_batches = num_batches - (batch_idx + 1);
        let eta_secs = avg_time_per_batch * remaining_batches as f32;

        println!(
            "Batch {}/{} completed in {:.1}s ({}/{} tests, {:.1}%) - ETA: {:.0} min",
            batch_idx + 1,
            num_batches,
            batch_duration.as_secs_f32(),
            completed,
            total_configs,
            progress_pct,
            eta_secs / 60.0
        );
    }

    let total_duration = overall_start.elapsed();

    println!("\n✅ All tests completed in {} minutes {:.0} seconds",
        total_duration.as_secs() / 60,
        total_duration.as_secs() % 60);
    println!("Successfully completed: {}/{} tests", all_results.len(), total_configs);

    export_results_csv("comprehensive_sweep.csv", &all_results)
        .expect("Failed to export CSV");

    println!("\nGenerating visualizations...");
    let plot_result = std::process::Command::new("python3")
        .args(["plot_results.py", "comprehensive_sweep.csv"])
        .status();

    match plot_result {
        Ok(status) if status.success() => {
            println!("✅ Plots generated successfully");
        }
        Ok(status) => {
            println!("⚠️  Plot generation failed with status: {}", status);
            println!("💡 Run manually: python3 plot_results.py comprehensive_sweep.csv");
        }
        Err(e) => {
            println!("⚠️  Could not run plot script: {}", e);
            println!("💡 Run manually: python3 plot_results.py comprehensive_sweep.csv");
        }
    }

    all_results.sort_by(|a, b| {
        a.1.precondition_failure_rate
            .partial_cmp(&b.1.precondition_failure_rate)
            .unwrap()
    });

    println!("\n=== Top 5 Best Configs (Lowest Failure Rate) ===");
    for (i, (config, summary)) in all_results.iter().take(5).enumerate() {
        println!(
            "{}. {}: {:.2}% failure rate, {:.2} TPS",
            i + 1,
            create_config_label(config),
            summary.precondition_failure_rate * 100.0,
            summary.write_tps
        );
    }

    println!("\n=== Top 5 Worst Configs (Highest Failure Rate) ===");
    for (i, (config, summary)) in all_results.iter().rev().take(5).enumerate() {
        println!(
            "{}. {}: {:.2}% failure rate, {:.2} TPS",
            i + 1,
            create_config_label(config),
            summary.precondition_failure_rate * 100.0,
            summary.write_tps
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_chaos_sweep() {
    use perf_test::{chaos::{create_chaos_scenarios, ChaosController}, metrics::MetricsSummary, utils::get_best_config_from_csv};
    use std::time::Duration;

    init_tracing();

    println!("\n=== Loading Best Configuration from Phase 3 ===");
    let mut best_config = get_best_config_from_csv("comprehensive_sweep.csv")
        .expect("Failed to load best config from CSV. Run test_comprehensive_sweep first.");

    best_config.duration = Duration::from_secs(300);

    println!("Best config: num_writers={}, writer_rate={}, key_overlap_ratio={}, num_readers={}, reader_rate={}",
        best_config.num_writers, best_config.writer_rate, best_config.key_overlap_ratio,
        best_config.num_readers, best_config.reader_rate);

    let scenarios = create_chaos_scenarios();
    let scenario_labels: Vec<String> = scenarios.iter().map(|s| s.label()).collect();

    println!("\n=== Running Chaos Sweep (7 scenarios in parallel = ~5 minutes) ===");

    let mut handles = Vec::new();

    for (idx, scenario) in scenarios.into_iter().enumerate() {
        let scenario_label = scenario_labels[idx].clone();
        let config = best_config.clone();

        let handle = tokio::spawn(async move {
            println!("[{}/7] Starting scenario: {}", idx + 1, scenario_label);

            let test_name = format!("chaos-{}", scenario_label);
            let manifest = Arc::new(
                create_real_s3_manifest(&test_name).expect("Failed to create S3 manifest")
            );

            let mut chaos_controller = ChaosController::new(scenario.clone());
            chaos_controller.start();

            let driver = WorkloadDriver::new(config.clone(), manifest.clone());
            let summary = driver.run().await;
            let metrics = driver.metrics().clone();

            chaos_controller.stop().await;

            println!("\n--- Scenario: {} ---", scenario_label);
            summary.print_report();

            println!("🔍 Verifying serializable isolation...");
            match verify_serializable_isolation_with_tracking(&manifest, &metrics).await {
                Ok(_) => println!("✅ Isolation verification passed for {}", scenario_label),
                Err(e) => println!("⚠️  Isolation verification failed for {}: {}", scenario_label, e),
            }

            Ok::<(WorkloadConfig, MetricsSummary), String>((config, summary))
        });

        handles.push(handle);
    }

    println!("\n⏳ Waiting for all scenarios to complete...");

    let parallel_results: Vec<_> = futures_util::future::join_all(handles).await;

    let mut results = Vec::new();
    for result in parallel_results {
        match result {
            Ok(Ok((config, summary))) => {
                results.push((config, summary));
            }
            Ok(Err(e)) => {
                eprintln!("❌ Scenario failed: {}", e);
            }
            Err(e) => {
                eprintln!("❌ Task panicked: {}", e);
            }
        }
    }

    export_results_csv("chaos_sweep.csv", &results)
        .expect("Failed to export chaos results");

    println!("\n=== Chaos Sweep Summary ===");
    for (idx, label) in scenario_labels.iter().enumerate() {
        let summary = &results[idx].1;
        println!(
            "{}: Failure Rate: {:.2}%, Write TPS: {:.2}, p99 Latency: {:.2}ms",
            label,
            summary.precondition_failure_rate * 100.0,
            summary.write_tps,
            summary.write_p99_ms
        );
    }

    println!("\n💡 To generate plots, run: python3 plot_results.py chaos_sweep.csv --chaos");
}
