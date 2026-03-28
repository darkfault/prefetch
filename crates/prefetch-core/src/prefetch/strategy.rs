/// How to order the prefetching of model layers.
#[derive(Debug, Clone)]
pub enum PrefetchStrategy {
    /// Load in inference execution order:
    /// embedding -> block 0 -> block 1 -> ... -> output_norm -> output
    InferenceOrder,

    /// Load only the embedding layer and the first N transformer blocks.
    /// Useful when memory is constrained — prioritize early layers.
    FirstNLayers(u32),

    /// Load the entire file sequentially from start to end.
    /// No GGUF awareness needed — works with any file.
    Sequential,
}

impl std::fmt::Display for PrefetchStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InferenceOrder => write!(f, "inference-order"),
            Self::FirstNLayers(n) => write!(f, "first-{n}-layers"),
            Self::Sequential => write!(f, "sequential"),
        }
    }
}

impl PrefetchStrategy {
    /// Parse from a CLI string.
    pub fn from_str_with_layers(s: &str, layers: Option<u32>) -> anyhow::Result<Self> {
        match s {
            "inference-order" => Ok(Self::InferenceOrder),
            "sequential" => Ok(Self::Sequential),
            "first-n-layers" => {
                let n = layers.unwrap_or(8);
                Ok(Self::FirstNLayers(n))
            }
            other => anyhow::bail!("unknown strategy: {other}. Options: inference-order, sequential, first-n-layers"),
        }
    }
}

/// Memory budget constraints for prefetching.
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    /// Maximum bytes to attempt to warm into cache.
    /// If None, warm the entire model.
    pub max_bytes: Option<u64>,

    /// Minimum free system RAM (in bytes) to maintain.
    /// Stop prefetching if free memory drops below this.
    pub min_free_ram: u64,

    /// Skip all budget checks (force prefetch regardless of memory).
    pub force: bool,
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            max_bytes: None,
            min_free_ram: 2 * 1024 * 1024 * 1024, // 2 GB
            force: false,
        }
    }
}

impl MemoryBudget {
    /// Check if we should continue prefetching given current free memory.
    pub fn should_continue(&self, bytes_warmed: u64) -> bool {
        if self.force {
            return true;
        }

        // Check max bytes budget
        if let Some(max) = self.max_bytes {
            if bytes_warmed >= max {
                return false;
            }
        }

        // Check system free memory
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        let available = sys.available_memory();
        let free = sys.free_memory();
        // Use available_memory if non-zero, otherwise fall back to free_memory
        let usable = if available > 0 { available } else { free };

        // If we can't determine free memory (both return 0), allow prefetching
        if usable == 0 {
            tracing::debug!("could not determine free memory, continuing prefetch");
            return true;
        }

        if usable < self.min_free_ram {
            tracing::warn!(
                free_mb = usable / (1024 * 1024),
                min_mb = self.min_free_ram / (1024 * 1024),
                "stopping prefetch: free memory below minimum threshold"
            );
            return false;
        }

        true
    }
}
