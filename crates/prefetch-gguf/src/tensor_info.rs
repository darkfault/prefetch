use std::io::Read;

use crate::header::{read_string, read_u32, read_u64};
use crate::limits;
use crate::types::GGMLType;
use crate::GgufError;

/// Information about a single tensor in the GGUF file.
///
/// This is parsed from the tensor info array in the file header.
/// The `offset` is relative to the start of the tensor data section.
#[derive(Debug, Clone)]
pub struct TensorInfo {
    /// Tensor name (e.g., "blk.0.attn_q.weight", "token_embd.weight").
    pub name: String,
    /// Dimensions of the tensor.
    pub dimensions: Vec<u64>,
    /// The quantization / data type.
    pub ggml_type: GGMLType,
    /// Offset relative to the tensor data section start.
    pub offset: u64,
    /// Computed byte size based on dimensions and type.
    pub byte_size: u64,
}

impl TensorInfo {
    /// Total number of elements in the tensor.
    pub fn n_elements(&self) -> u64 {
        self.dimensions.iter().product::<u64>().max(1)
    }
}

/// Read all tensor info descriptors from the header.
pub fn read_tensor_infos(
    reader: &mut impl Read,
    count: u64,
) -> Result<Vec<TensorInfo>, GgufError> {
    // count is already validated against MAX_TENSOR_COUNT in parse_gguf()
    let mut tensors = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let name = read_string(reader)?;

        let n_dims = read_u32(reader)?;
        if n_dims > limits::MAX_TENSOR_DIMS {
            return Err(GgufError::TooManyDimensions(n_dims));
        }

        let mut dimensions = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            dimensions.push(read_u64(reader)?);
        }

        let type_id = read_u32(reader)?;
        let ggml_type = GGMLType::from_u32(type_id)
            .ok_or(GgufError::InvalidGGMLType(type_id))?;

        let offset = read_u64(reader)?;

        // Use checked arithmetic to compute element count and byte size
        let n_elements = dimensions
            .iter()
            .copied()
            .try_fold(1u64, |acc, dim| acc.checked_mul(dim))
            .ok_or(GgufError::IntegerOverflow)?
            .max(1);

        let byte_size = ggml_type
            .tensor_byte_size_checked(n_elements)
            .ok_or(GgufError::IntegerOverflow)?;

        tensors.push(TensorInfo {
            name,
            dimensions,
            ggml_type,
            offset,
            byte_size,
        });
    }

    Ok(tensors)
}
