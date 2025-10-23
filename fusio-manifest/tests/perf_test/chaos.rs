use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub enum ChaosScenario {
    None,
    NetworkLatency { delay_ms: u64 },
    NetworkBlocking { block_duration_secs: u64, num_blocks: usize },
    CpuOverload { num_threads: usize, utilization_pct: u8 },
    Combined { delay_ms: u64, num_threads: usize, utilization_pct: u8 },
}

impl ChaosScenario {
    pub fn label(&self) -> String {
        match self {
            ChaosScenario::None => "baseline".to_string(),
            ChaosScenario::NetworkLatency { delay_ms } => format!("net-delay-{}ms", delay_ms),
            ChaosScenario::NetworkBlocking { block_duration_secs, num_blocks } => {
                format!("net-block-{}s-{}x", block_duration_secs, num_blocks)
            }
            ChaosScenario::CpuOverload { num_threads, utilization_pct } => {
                format!("cpu-{}threads-{}pct", num_threads, utilization_pct)
            }
            ChaosScenario::Combined { delay_ms, num_threads, utilization_pct } => {
                format!("combined-{}ms-{}threads-{}pct", delay_ms, num_threads, utilization_pct)
            }
        }
    }
}

pub struct ChaosController {
    scenario: ChaosScenario,
    running: Arc<AtomicBool>,
    cpu_handles: Vec<JoinHandle<()>>,
    blocking_handle: Option<JoinHandle<()>>,
}

impl ChaosController {
    pub fn new(scenario: ChaosScenario) -> Self {
        Self {
            scenario,
            running: Arc::new(AtomicBool::new(false)),
            cpu_handles: Vec::new(),
            blocking_handle: None,
        }
    }

    pub fn start(&mut self) {
        match &self.scenario {
            ChaosScenario::None => {}
            ChaosScenario::NetworkLatency { .. } => {}
            ChaosScenario::NetworkBlocking { block_duration_secs, num_blocks } => {
                self.start_network_blocking(*block_duration_secs, *num_blocks);
            }
            ChaosScenario::CpuOverload { num_threads, utilization_pct } => {
                self.start_cpu_overload(*num_threads, *utilization_pct);
            }
            ChaosScenario::Combined { num_threads, utilization_pct, .. } => {
                self.start_cpu_overload(*num_threads, *utilization_pct);
            }
        }
    }

    fn start_network_blocking(&mut self, block_duration_secs: u64, num_blocks: usize) {
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let handle = tokio::spawn(async move {
            use rand::rngs::StdRng;
            use rand::{Rng, SeedableRng};
            let mut rng = StdRng::from_entropy();

            for i in 0..num_blocks {
                if !running.load(Ordering::SeqCst) {
                    break;
                }

                let wait_time = rng.gen_range(30..90);
                tokio::time::sleep(Duration::from_secs(wait_time)).await;

                if !running.load(Ordering::SeqCst) {
                    break;
                }

                println!("🔴 Network blocking event {}/{} - blocking for {}s", i + 1, num_blocks, block_duration_secs);
                tokio::time::sleep(Duration::from_secs(block_duration_secs)).await;
                println!("🟢 Network blocking event {}/{} - unblocked", i + 1, num_blocks);
            }
        });

        self.blocking_handle = Some(handle);
    }

    fn start_cpu_overload(&mut self, num_threads: usize, utilization_pct: u8) {
        self.running.store(true, Ordering::SeqCst);

        let work_duration_ms = utilization_pct as u64;
        let sleep_duration_ms = 100 - utilization_pct as u64;

        for _ in 0..num_threads {
            let running = self.running.clone();
            let handle = tokio::spawn(async move {
                while running.load(Ordering::SeqCst) {
                    let start = std::time::Instant::now();
                    while start.elapsed().as_millis() < work_duration_ms as u128 {
                        for _ in 0..1000 {
                            std::hint::black_box(0);
                        }
                    }

                    if sleep_duration_ms > 0 {
                        tokio::time::sleep(Duration::from_millis(sleep_duration_ms)).await;
                    }
                }
            });
            self.cpu_handles.push(handle);
        }
    }

    pub async fn apply_network_delay(&self) {
        if let Some(delay_ms) = self.get_network_delay_ms() {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    fn get_network_delay_ms(&self) -> Option<u64> {
        match &self.scenario {
            ChaosScenario::NetworkLatency { delay_ms } => Some(*delay_ms),
            ChaosScenario::Combined { delay_ms, .. } => Some(*delay_ms),
            _ => None,
        }
    }

    pub async fn stop(mut self) {
        self.running.store(false, Ordering::SeqCst);

        for handle in self.cpu_handles.drain(..) {
            let _ = handle.await;
        }

        if let Some(handle) = self.blocking_handle.take() {
            let _ = handle.await;
        }
    }
}

pub fn create_chaos_scenarios() -> Vec<ChaosScenario> {
    vec![
        ChaosScenario::None,
        ChaosScenario::NetworkLatency { delay_ms: 100 },
        ChaosScenario::NetworkLatency { delay_ms: 200 },
        ChaosScenario::NetworkLatency { delay_ms: 500 },
        ChaosScenario::NetworkBlocking { block_duration_secs: 10, num_blocks: 3 },
        ChaosScenario::CpuOverload { num_threads: 4, utilization_pct: 80 },
        ChaosScenario::Combined { delay_ms: 200, num_threads: 4, utilization_pct: 80 },
    ]
}
