use std::time::{Duration, Instant};

/// Progress update during a prefetch operation.
#[derive(Debug, Clone)]
pub struct PrefetchProgress {
    /// Current layer being prefetched.
    pub current_layer: String,
    /// Bytes advised so far.
    pub bytes_advised: u64,
    /// Total bytes to advise.
    pub total_bytes: u64,
    /// Number of layer groups completed.
    pub layers_completed: usize,
    /// Total number of layer groups.
    pub total_layers: usize,
    /// Elapsed time since prefetch started.
    pub elapsed: Duration,
}

impl PrefetchProgress {
    /// Completion percentage.
    pub fn percent(&self) -> f64 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.bytes_advised as f64 / self.total_bytes as f64) * 100.0
    }

    /// Estimated throughput in MB/s.
    pub fn throughput_mbps(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs < 0.001 {
            return 0.0;
        }
        (self.bytes_advised as f64 / (1024.0 * 1024.0)) / secs
    }
}

/// Tracks prefetch progress over time.
pub(crate) struct ProgressTracker {
    start: Instant,
    bytes_advised: u64,
    total_bytes: u64,
    layers_completed: usize,
    total_layers: usize,
    current_layer: String,
}

impl ProgressTracker {
    pub fn new(total_bytes: u64, total_layers: usize) -> Self {
        Self {
            start: Instant::now(),
            bytes_advised: 0,
            total_bytes,
            layers_completed: 0,
            total_layers,
            current_layer: String::new(),
        }
    }

    pub fn set_current_layer(&mut self, name: String) {
        self.current_layer = name;
    }

    pub fn add_bytes(&mut self, bytes: u64) {
        self.bytes_advised += bytes;
    }

    pub fn complete_layer(&mut self) {
        self.layers_completed += 1;
    }

    pub fn snapshot(&self) -> PrefetchProgress {
        PrefetchProgress {
            current_layer: self.current_layer.clone(),
            bytes_advised: self.bytes_advised,
            total_bytes: self.total_bytes,
            layers_completed: self.layers_completed,
            total_layers: self.total_layers,
            elapsed: self.start.elapsed(),
        }
    }
}
