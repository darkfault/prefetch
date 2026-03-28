//! Lightweight GGUF file format parser.
//!
//! Parses only headers, metadata, and tensor info — never reads tensor data.
//! This is designed for understanding model layout so that prefetch operations
//! can warm pages in inference-optimal order.

pub mod header;
pub mod metadata;
pub mod tensor_info;
pub mod types;
pub mod layout;

use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::Path;

pub use header::GgufHeader;
pub use layout::{LayerGroup, LayerKind, ModelLayout};
pub use metadata::MetadataValue;
pub use tensor_info::TensorInfo;
pub use types::GGMLType;

/// Parse a GGUF file and return its complete model layout.
///
/// This reads only the header, metadata, and tensor info sections.
/// The actual tensor data is never read — we only need byte ranges.
pub fn parse_gguf(path: &Path) -> Result<ModelLayout, GgufError> {
    let file = File::open(path).map_err(|e| GgufError::Io(e.to_string()))?;
    let file_size = file.metadata().map_err(|e| GgufError::Io(e.to_string()))?.len();
    let mut reader = BufReader::new(file);

    let header = header::read_header(&mut reader)?;

    // Validate counts against safety limits
    if header.tensor_count > limits::MAX_TENSOR_COUNT {
        return Err(GgufError::TooManyTensors(header.tensor_count));
    }
    if header.metadata_kv_count > limits::MAX_METADATA_KV_COUNT {
        return Err(GgufError::TooManyMetadataKVs(header.metadata_kv_count));
    }

    tracing::debug!(
        version = header.version,
        tensors = header.tensor_count,
        metadata_kvs = header.metadata_kv_count,
        "parsed GGUF header"
    );

    // Read metadata to find alignment value
    let metadata = metadata::read_metadata(&mut reader, header.metadata_kv_count)?;
    let alignment = metadata
        .get("general.alignment")
        .and_then(|v| v.as_u64())
        .unwrap_or(32) as u64;

    // Read tensor info descriptors
    let tensors = tensor_info::read_tensor_infos(&mut reader, header.tensor_count)?;

    // The tensor data starts after the tensor info array, aligned
    let current_pos = reader.seek(SeekFrom::Current(0))
        .map_err(|e| GgufError::Io(e.to_string()))?;
    let tensor_data_offset = align_offset(current_pos, alignment);

    tracing::debug!(
        tensor_data_offset,
        alignment,
        file_size,
        "computed tensor data section"
    );

    let layout = layout::build_layout(tensors, tensor_data_offset, file_size, &metadata)?;
    Ok(layout)
}

/// Align an offset up to the given alignment boundary.
fn align_offset(offset: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return offset;
    }
    (offset + alignment - 1) / alignment * alignment
}

/// Safety limits to prevent resource exhaustion from malicious files.
pub mod limits {
    /// Maximum number of tensors allowed in a GGUF file.
    /// The largest known models (Llama 405B) have ~1,000 tensors.
    pub const MAX_TENSOR_COUNT: u64 = 100_000;

    /// Maximum number of metadata key-value pairs.
    pub const MAX_METADATA_KV_COUNT: u64 = 100_000;

    /// Maximum number of dimensions per tensor.
    /// Realistically never exceeds 4-5.
    pub const MAX_TENSOR_DIMS: u32 = 16;

    /// Maximum string length (1 MB).
    pub const MAX_STRING_LENGTH: u64 = 1024 * 1024;

    /// Maximum array length in metadata values.
    pub const MAX_ARRAY_LENGTH: u64 = 10_000_000;
}

#[derive(Debug, thiserror::Error)]
pub enum GgufError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("invalid GGUF magic: expected 0x46554747, got 0x{0:08X}")]
    InvalidMagic(u32),
    #[error("unsupported GGUF version: {0} (supported: 2, 3)")]
    UnsupportedVersion(u32),
    #[error("invalid metadata value type: {0}")]
    InvalidValueType(u32),
    #[error("invalid GGML type: {0}")]
    InvalidGGMLType(u32),
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("string too long: {0} bytes")]
    StringTooLong(u64),
    #[error("tensor count {0} exceeds safety limit")]
    TooManyTensors(u64),
    #[error("metadata kv count {0} exceeds safety limit")]
    TooManyMetadataKVs(u64),
    #[error("tensor dimensions {0} exceeds safety limit")]
    TooManyDimensions(u32),
    #[error("array length {0} exceeds safety limit")]
    ArrayTooLong(u64),
    #[error("integer overflow computing tensor byte size")]
    IntegerOverflow,
}
