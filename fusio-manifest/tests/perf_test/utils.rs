use std::time::{Duration, SystemTime};
use std::path::PathBuf;
use std::env;
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use ini::Ini;
use rand::{seq::SliceRandom, rngs::StdRng, SeedableRng};

#[derive(Debug, Clone)]
pub struct WorkloadConfig {
    pub num_writers: usize,
    pub num_readers: usize,
    pub duration: Duration,
    pub writer_rate: f64,
    pub reader_rate: f64,
    pub value_size: usize,
    pub max_retry_count: usize,

    pub key_pool_size: usize,
    pub key_overlap_ratio: f64,
    pub write_delete_ratio: f64,
}

impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            num_writers: 1,
            num_readers: 2,
            duration: Duration::from_secs(120),
            writer_rate: 0.1,
            reader_rate: 100.0,
            value_size: 256,
            max_retry_count: 1,

            key_pool_size: 100,
            key_overlap_ratio: 0.0,
            write_delete_ratio: 0.0,
        }
    }
}

pub struct KeyRegistry {
    written_keys: Arc<Mutex<Vec<String>>>,
    next_key_id: Arc<AtomicUsize>,
}

impl KeyRegistry {
    pub fn new() -> Self {
        Self {
            written_keys: Arc::new(Mutex::new(Vec::new())),
            next_key_id: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn allocate_next_key(&self) -> String {
        let id = self.next_key_id.fetch_add(1, Ordering::SeqCst);
        format!("key_{:06}", id)
    }

    pub fn register_written_key(&self, key: String) {
        self.written_keys.lock().unwrap().push(key);
    }

    pub fn get_random_keys(&self, count: usize) -> Vec<String> {
        let keys = self.written_keys.lock().unwrap();
        if keys.is_empty() {
            return Vec::new();
        }

        let mut rng = StdRng::from_entropy();
        let sample_size = count.min(keys.len());
        keys.choose_multiple(&mut rng, sample_size)
            .cloned()
            .collect()
    }

    pub fn all_keys(&self) -> Vec<String> {
        self.written_keys.lock().unwrap().clone()
    }

    pub fn key_count(&self) -> usize {
        self.written_keys.lock().unwrap().len()
    }
}

pub struct KeyPool {
    writer_key_sets: Vec<Vec<String>>,
    reader_keys: Vec<String>,
}

impl KeyPool {
    pub fn new(total_keys: usize, num_writers: usize, overlap_ratio: f64) -> Self {
        let all_keys: Vec<String> = (0..total_keys)
            .map(|i| format!("key_{:06}", i))
            .collect();

        if num_writers == 0 {
            return Self {
                writer_key_sets: vec![],
                reader_keys: all_keys,
            };
        }

        let keys_per_writer = total_keys / num_writers;
        let overlap_size = (keys_per_writer as f64 * overlap_ratio) as usize;

        let mut writer_key_sets = Vec::with_capacity(num_writers);
        for writer_id in 0..num_writers {
            let start_idx = if writer_id == 0 {
                0
            } else {
                (writer_id * keys_per_writer).saturating_sub(overlap_size)
            };

            let end_idx = ((writer_id + 1) * keys_per_writer).min(total_keys);

            let writer_keys: Vec<String> = all_keys[start_idx..end_idx].to_vec();
            writer_key_sets.push(writer_keys);
        }

        Self {
            writer_key_sets,
            reader_keys: all_keys,
        }
    }

    pub fn writer_keys(&self, writer_id: usize) -> &[String] {
        &self.writer_key_sets[writer_id]
    }

    pub fn reader_keys(&self) -> &[String] {
        &self.reader_keys
    }
}

pub fn create_test_prefix(test_name: &str) -> String {
    format!(
        "chaos-tests/{}/{}",
        test_name,
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    )
}

pub fn create_sweep_prefix() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();

    let secs = now.as_secs();
    let datetime = chrono::DateTime::from_timestamp(secs as i64, 0)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());

    format!(
        "chaos-tests/comprehensive-sweep-{}",
        datetime.format("%Y%m%d-%H%M%S")
    )
}

pub fn create_test_prefix_in_sweep(sweep_prefix: &str, test_index: usize, config_label: &str) -> String {
    format!("{}/test-{:03}-{}", sweep_prefix, test_index, config_label)
}

pub fn create_config_label(config: &WorkloadConfig) -> String {
    format!(
        "W{}_WR{:.2}_RD{}_RT{}",
        config.num_writers,
        config.writer_rate,
        config.num_readers,
        config.reader_rate as u32
    )
}

pub fn generate_all_configs_v2() -> Vec<WorkloadConfig> {
    let num_writers_values = [1, 2, 3];
    let writer_rate_values = [0.02, 0.05, 0.1, 0.2];
    let num_readers_values = [3, 4, 5, 6];
    let reader_rate_values = [100.0, 200.0, 300.0];

    let mut configs = Vec::new();

    for &num_writers in &num_writers_values {
        for &writer_rate in &writer_rate_values {
            if num_writers == 1 && writer_rate == 0.1 {
                continue;
            }

            for &num_readers in &num_readers_values {
                for &reader_rate in &reader_rate_values {
                    configs.push(WorkloadConfig {
                        num_writers,
                        num_readers,
                        duration: Duration::from_secs(60),
                        writer_rate,
                        reader_rate,
                        value_size: 256,
                        max_retry_count: 1,
                        key_pool_size: 100,
                        key_overlap_ratio: 0.0,
                        write_delete_ratio: 0.0,
                    });
                }
            }
        }
    }

    // Add overlap ratio sweep configs: 2 writers @ 0.1 TPS with varying overlap
    let overlap_ratios = [0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5];
    for &overlap_ratio in &overlap_ratios {
        configs.push(WorkloadConfig {
            num_writers: 2,
            num_readers: 2,
            duration: Duration::from_secs(120),
            writer_rate: 0.1,
            reader_rate: 100.0,
            value_size: 256,
            max_retry_count: 1,
            key_pool_size: 100,
            key_overlap_ratio: overlap_ratio,
            write_delete_ratio: 0.0,
        });
    }

    configs
}

pub fn generate_all_configs() -> Vec<WorkloadConfig> {
    let num_writers_values = [2, 3, 4];
    let writer_rate_values = [0.05, 0.1, 0.15, 0.2];
    let key_overlap_ratio_values = [0.1, 0.2, 0.3, 0.4];
    let num_readers_values = [4];
    let reader_rate_values = [5.0, 6.0];

    let mut configs = Vec::new();

    for &num_writers in &num_writers_values {
        for &writer_rate in &writer_rate_values {
            for &key_overlap_ratio in &key_overlap_ratio_values {
                for &num_readers in &num_readers_values {
                    for &reader_rate in &reader_rate_values {
                        configs.push(WorkloadConfig {
                            num_writers,
                            num_readers,
                            duration: Duration::from_secs(60),
                            writer_rate,
                            reader_rate,
                            value_size: 256,
                            max_retry_count: 1,
                            key_pool_size: 100,
                            key_overlap_ratio,
                            write_delete_ratio: 0.1,
                        });
                    }
                }
            }
        }
    }

    configs
}

pub fn get_best_config_from_csv(csv_path: &str) -> Result<WorkloadConfig, Box<dyn std::error::Error>> {
    use csv::ReaderBuilder;

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(csv_path)?;

    let mut best_config: Option<WorkloadConfig> = None;
    let mut best_failure_rate = f64::MAX;

    for result in reader.records() {
        let record = result?;

        let num_writers: usize = record.get(1)
            .ok_or("Missing num_writers")?.parse()?;
        let num_readers: usize = record.get(2)
            .ok_or("Missing num_readers")?.parse()?;
        let writer_rate: f64 = record.get(3)
            .ok_or("Missing writer_rate")?.parse()?;
        let reader_rate: f64 = record.get(4)
            .ok_or("Missing reader_rate")?.parse()?;
        let key_pool_size: usize = record.get(5)
            .ok_or("Missing key_pool_size")?.parse()?;
        let key_overlap_ratio: f64 = record.get(6)
            .ok_or("Missing key_overlap_ratio")?.parse()?;
        let max_retry_count: usize = record.get(7)
            .ok_or("Missing max_retry_count")?.parse()?;
        let duration_secs: f64 = record.get(8)
            .ok_or("Missing duration")?.parse()?;

        let precondition_failure_rate: f64 = record.get(9)
            .ok_or("Missing precondition_failure_rate")?.parse()?;

        if precondition_failure_rate < best_failure_rate {
            best_failure_rate = precondition_failure_rate;
            best_config = Some(WorkloadConfig {
                num_writers,
                num_readers,
                duration: Duration::from_secs(duration_secs as u64),
                writer_rate,
                reader_rate,
                value_size: 256,
                max_retry_count,
                key_pool_size,
                key_overlap_ratio,
                write_delete_ratio: 0.1,
            });
        }
    }

    best_config.ok_or_else(|| "No valid configurations found in CSV".into())
}

#[derive(Debug, Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub region: String,
}

pub fn load_aws_credentials() -> Result<AwsCredentials, Box<dyn std::error::Error>> {
    let profile = env::var("AWS_PROFILE").unwrap_or_else(|_| "default".to_string());

    if let Ok(key_id) = env::var("AWS_ACCESS_KEY_ID") {
        let secret_key = env::var("AWS_SECRET_ACCESS_KEY")?;
        let token = env::var("AWS_SESSION_TOKEN").ok();
        let region = env::var("AWS_REGION")
            .or_else(|_| env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".to_string());

        println!("Using AWS credentials from environment variables");
        return Ok(AwsCredentials {
            access_key_id: key_id,
            secret_access_key: secret_key,
            session_token: token,
            region,
        });
    }

    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map_err(|_| "Cannot determine home directory")?;

    let credentials_path = PathBuf::from(home).join(".aws").join("credentials");
    let config_path = PathBuf::from(env::var("HOME").or_else(|_| env::var("USERPROFILE"))?)
        .join(".aws")
        .join("config");

    if !credentials_path.exists() {
        return Err(format!(
            "AWS credentials file not found at {:?}. Please create it, set AWS_ACCESS_KEY_ID environment variable, or use EC2 IAM role.",
            credentials_path
        ).into());
    }

    println!("Using AWS credentials from file: {:?}", credentials_path);
    let credentials_ini = Ini::load_from_file(&credentials_path)
        .map_err(|e| format!("Failed to parse credentials file: {}", e))?;

    let section = credentials_ini
        .section(Some(&profile))
        .ok_or_else(|| format!("Profile '{}' not found in credentials file", profile))?;

    let access_key_id = section
        .get("aws_access_key_id")
        .ok_or_else(|| format!("aws_access_key_id not found in profile '{}'", profile))?
        .to_string();

    let secret_access_key = section
        .get("aws_secret_access_key")
        .ok_or_else(|| format!("aws_secret_access_key not found in profile '{}'", profile))?
        .to_string();

    let session_token = section.get("aws_session_token").map(|s| s.to_string());

    let region = env::var("AWS_REGION")
        .or_else(|_| {
            if config_path.exists() {
                if let Ok(config_ini) = Ini::load_from_file(&config_path) {
                    let config_section_name = if profile == "default" {
                        "default".to_string()
                    } else {
                        format!("profile {}", profile)
                    };

                    if let Some(section) = config_ini.section(Some(&config_section_name)) {
                        if let Some(region) = section.get("region") {
                            return Ok(region.to_string());
                        }
                    }
                }
            }
            Err(env::VarError::NotPresent)
        })
        .unwrap_or_else(|_| "us-east-1".to_string());

    Ok(AwsCredentials {
        access_key_id,
        secret_access_key,
        session_token,
        region,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_pool_no_overlap() {
        let pool = KeyPool::new(100, 4, 0.0);
        assert_eq!(pool.writer_key_sets.len(), 4);
        assert_eq!(pool.reader_keys.len(), 100);

        for i in 0..4 {
            assert_eq!(pool.writer_keys(i).len(), 25);
        }
    }

    #[test]
    fn test_key_pool_with_overlap() {
        let pool = KeyPool::new(100, 3, 0.2);

        assert_eq!(pool.writer_key_sets.len(), 3);

        let keys_per_writer = 100 / 3;
        let overlap = (keys_per_writer as f64 * 0.2) as usize;

        let w0_keys = pool.writer_keys(0);
        let w1_keys = pool.writer_keys(1);

        let overlap_count = w0_keys
            .iter()
            .filter(|k| w1_keys.contains(k))
            .count();

        assert!(overlap_count >= overlap.saturating_sub(1));
        assert!(overlap_count <= overlap + 1);
    }
}
