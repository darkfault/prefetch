use std::collections::HashMap;

use crate::metadata::MetadataValue;
use crate::tensor_info::TensorInfo;
use crate::GgufError;

/// The logical kind of a layer group.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayerKind {
    /// Token embedding table — accessed first during inference.
    TokenEmbedding,
    /// A transformer block (attention + FFN). Index is the block number.
    TransformerBlock(u32),
    /// Output normalization layer.
    OutputNorm,
    /// Output projection / language model head.
    OutputHead,
    /// Anything that doesn't match known patterns.
    Other(String),
}

impl LayerKind {
    /// Returns the inference execution order for sorting.
    /// Lower values are accessed first during inference.
    fn sort_key(&self) -> (u32, u32) {
        match self {
            Self::TokenEmbedding => (0, 0),
            Self::TransformerBlock(n) => (1, *n),
            Self::OutputNorm => (2, 0),
            Self::OutputHead => (3, 0),
            Self::Other(_) => (4, 0),
        }
    }
}

impl std::fmt::Display for LayerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenEmbedding => write!(f, "token_embedding"),
            Self::TransformerBlock(n) => write!(f, "block.{n}"),
            Self::OutputNorm => write!(f, "output_norm"),
            Self::OutputHead => write!(f, "output_head"),
            Self::Other(name) => write!(f, "other({name})"),
        }
    }
}

/// A group of tensors that belong to the same logical layer.
#[derive(Debug, Clone)]
pub struct LayerGroup {
    /// What kind of layer this is.
    pub kind: LayerKind,
    /// Indices into `ModelLayout::tensors`.
    pub tensor_indices: Vec<usize>,
    /// Start byte offset in the file (absolute).
    pub file_offset_start: u64,
    /// End byte offset in the file (absolute, exclusive).
    pub file_offset_end: u64,
    /// Total bytes for this layer group.
    pub total_bytes: u64,
}

/// Complete layout of a GGUF model file.
#[derive(Debug, Clone)]
pub struct ModelLayout {
    /// Absolute file offset where tensor data begins.
    pub tensor_data_offset: u64,
    /// All tensor descriptors.
    pub tensors: Vec<TensorInfo>,
    /// Logical layer groups, sorted in inference order.
    pub layer_groups: Vec<LayerGroup>,
    /// Total file size.
    pub file_size: u64,
    /// All metadata key-value pairs from the GGUF header.
    pub metadata: HashMap<String, crate::MetadataValue>,
    /// Model name from metadata, if available.
    pub model_name: Option<String>,
    /// Architecture name from metadata (e.g., "llama", "mistral").
    pub architecture: Option<String>,
    /// Number of transformer blocks from metadata.
    pub block_count: Option<u64>,
}

impl ModelLayout {
    /// Total bytes of tensor data.
    pub fn total_tensor_bytes(&self) -> u64 {
        self.tensors.iter().map(|t| t.byte_size).sum()
    }

    /// Get layer groups sorted in inference execution order.
    pub fn inference_ordered_groups(&self) -> Vec<&LayerGroup> {
        let mut groups: Vec<&LayerGroup> = self.layer_groups.iter().collect();
        groups.sort_by_key(|g| g.kind.sort_key());
        groups
    }

    /// Get only the first N transformer blocks (plus embedding).
    pub fn first_n_layers(&self, n: u32) -> Vec<&LayerGroup> {
        self.layer_groups
            .iter()
            .filter(|g| match &g.kind {
                LayerKind::TokenEmbedding => true,
                LayerKind::TransformerBlock(idx) => *idx < n,
                _ => false,
            })
            .collect()
    }
}

/// Classify a tensor name into its logical layer kind.
fn classify_tensor(name: &str) -> LayerKind {
    // Token embedding
    if name == "token_embd.weight" || name == "token_embd.bias" {
        return LayerKind::TokenEmbedding;
    }

    // Transformer blocks: "blk.{N}.{component}"
    if let Some(rest) = name.strip_prefix("blk.") {
        if let Some(dot_pos) = rest.find('.') {
            if let Ok(block_idx) = rest[..dot_pos].parse::<u32>() {
                return LayerKind::TransformerBlock(block_idx);
            }
        }
    }

    // Output normalization
    if name == "output_norm.weight" || name == "output_norm.bias" {
        return LayerKind::OutputNorm;
    }

    // Output head
    if name == "output.weight" || name == "output.bias" {
        return LayerKind::OutputHead;
    }

    LayerKind::Other(name.to_string())
}

/// Build the model layout from parsed tensor info and metadata.
pub fn build_layout(
    tensors: Vec<TensorInfo>,
    tensor_data_offset: u64,
    file_size: u64,
    metadata: &HashMap<String, MetadataValue>,
) -> Result<ModelLayout, GgufError> {
    // Group tensors by their logical layer
    let mut groups_map: HashMap<String, (LayerKind, Vec<usize>)> = HashMap::new();

    for (idx, tensor) in tensors.iter().enumerate() {
        let kind = classify_tensor(&tensor.name);
        let key = format!("{kind}");
        groups_map
            .entry(key)
            .or_insert_with(|| (kind.clone(), Vec::new()))
            .1
            .push(idx);
    }

    // Build LayerGroup entries with byte ranges
    let mut layer_groups: Vec<LayerGroup> = groups_map
        .into_values()
        .map(|(kind, tensor_indices)| {
            let mut min_offset = u64::MAX;
            let mut max_end = 0u64;
            let mut total_bytes = 0u64;

            for &idx in &tensor_indices {
                let tensor = &tensors[idx];
                let abs_start = tensor_data_offset + tensor.offset;
                let abs_end = abs_start + tensor.byte_size;
                min_offset = min_offset.min(abs_start);
                max_end = max_end.max(abs_end);
                total_bytes += tensor.byte_size;
            }

            LayerGroup {
                kind,
                tensor_indices,
                file_offset_start: min_offset,
                file_offset_end: max_end,
                total_bytes,
            }
        })
        .collect();

    // Sort by inference order
    layer_groups.sort_by_key(|g| g.kind.sort_key());

    // Extract useful metadata
    let model_name = metadata
        .get("general.name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let architecture = metadata
        .get("general.architecture")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let block_count = metadata
        .get("general.block_count")
        .or_else(|| {
            // Try architecture-specific key: {arch}.block_count
            architecture.as_ref().and_then(|arch| {
                metadata.get(&format!("{arch}.block_count"))
            })
        })
        .and_then(|v| v.as_u64());

    Ok(ModelLayout {
        tensor_data_offset,
        tensors,
        layer_groups,
        file_size,
        metadata: metadata.clone(),
        model_name,
        architecture,
        block_count,
    })
}
