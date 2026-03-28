use std::collections::HashMap;
use std::io::Read;

use crate::header::{
    read_bool, read_f32, read_f64, read_i32, read_i64, read_string, read_u16, read_u32, read_u64,
    read_u8,
};
use crate::GgufError;

/// GGUF metadata value types.
const GGUF_TYPE_UINT8: u32 = 0;
const GGUF_TYPE_INT8: u32 = 1;
const GGUF_TYPE_UINT16: u32 = 2;
const GGUF_TYPE_INT16: u32 = 3;
const GGUF_TYPE_UINT32: u32 = 4;
const GGUF_TYPE_INT32: u32 = 5;
const GGUF_TYPE_FLOAT32: u32 = 6;
const GGUF_TYPE_BOOL: u32 = 7;
const GGUF_TYPE_STRING: u32 = 8;
const GGUF_TYPE_ARRAY: u32 = 9;
const GGUF_TYPE_UINT64: u32 = 10;
const GGUF_TYPE_INT64: u32 = 11;
const GGUF_TYPE_FLOAT64: u32 = 12;

/// A parsed GGUF metadata value.
#[derive(Debug, Clone)]
pub enum MetadataValue {
    UInt8(u8),
    Int8(i8),
    UInt16(u16),
    Int16(i16),
    UInt32(u32),
    Int32(i32),
    Float32(f32),
    Bool(bool),
    String(String),
    Array(Vec<MetadataValue>),
    UInt64(u64),
    Int64(i64),
    Float64(f64),
}

impl MetadataValue {
    /// Try to extract as u64 (works for all unsigned integer types).
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::UInt8(v) => Some(*v as u64),
            Self::UInt16(v) => Some(*v as u64),
            Self::UInt32(v) => Some(*v as u64),
            Self::UInt64(v) => Some(*v),
            Self::Int32(v) if *v >= 0 => Some(*v as u64),
            Self::Int64(v) if *v >= 0 => Some(*v as u64),
            _ => None,
        }
    }

    /// Try to extract as string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
}

/// Read all metadata key-value pairs.
pub fn read_metadata(
    reader: &mut impl Read,
    count: u64,
) -> Result<HashMap<String, MetadataValue>, GgufError> {
    let mut map = HashMap::with_capacity(count as usize);

    for _ in 0..count {
        let key = read_string(reader)?;
        let value = read_value(reader)?;
        map.insert(key, value);
    }

    Ok(map)
}

/// Read a single typed metadata value.
fn read_value(reader: &mut impl Read) -> Result<MetadataValue, GgufError> {
    let value_type = read_u32(reader)?;
    read_typed_value(reader, value_type)
}

fn read_typed_value(reader: &mut impl Read, value_type: u32) -> Result<MetadataValue, GgufError> {
    match value_type {
        GGUF_TYPE_UINT8 => Ok(MetadataValue::UInt8(read_u8(reader)?)),
        GGUF_TYPE_INT8 => Ok(MetadataValue::Int8(read_u8(reader)? as i8)),
        GGUF_TYPE_UINT16 => Ok(MetadataValue::UInt16(read_u16(reader)?)),
        GGUF_TYPE_INT16 => Ok(MetadataValue::Int16(read_u16(reader)? as i16)),
        GGUF_TYPE_UINT32 => Ok(MetadataValue::UInt32(read_u32(reader)?)),
        GGUF_TYPE_INT32 => Ok(MetadataValue::Int32(read_i32(reader)?)),
        GGUF_TYPE_FLOAT32 => Ok(MetadataValue::Float32(read_f32(reader)?)),
        GGUF_TYPE_BOOL => Ok(MetadataValue::Bool(read_bool(reader)?)),
        GGUF_TYPE_STRING => Ok(MetadataValue::String(read_string(reader)?)),
        GGUF_TYPE_UINT64 => Ok(MetadataValue::UInt64(read_u64(reader)?)),
        GGUF_TYPE_INT64 => Ok(MetadataValue::Int64(read_i64(reader)?)),
        GGUF_TYPE_FLOAT64 => Ok(MetadataValue::Float64(read_f64(reader)?)),
        GGUF_TYPE_ARRAY => {
            let element_type = read_u32(reader)?;
            let len = read_u64(reader)?;
            if len > crate::limits::MAX_ARRAY_LENGTH {
                return Err(GgufError::ArrayTooLong(len));
            }
            // Cap initial capacity to avoid huge upfront allocations
            let mut elements = Vec::with_capacity(len.min(8192) as usize);
            for _ in 0..len {
                elements.push(read_typed_value(reader, element_type)?);
            }
            Ok(MetadataValue::Array(elements))
        }
        _ => Err(GgufError::InvalidValueType(value_type)),
    }
}
