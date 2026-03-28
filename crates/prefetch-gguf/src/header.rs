use std::io::Read;

use crate::GgufError;

/// The GGUF magic number: "GGUF" as little-endian u32.
// "GGUF" in ASCII: G(0x47) G(0x47) U(0x55) F(0x46)
// Read as little-endian u32: 0x46554747
const GGUF_MAGIC: u32 = 0x46554747;

/// Supported GGUF versions.
const SUPPORTED_VERSIONS: &[u32] = &[2, 3];

/// The GGUF file header.
#[derive(Debug, Clone)]
pub struct GgufHeader {
    pub version: u32,
    pub tensor_count: u64,
    pub metadata_kv_count: u64,
}

/// Read and validate the GGUF file header.
pub fn read_header(reader: &mut impl Read) -> Result<GgufHeader, GgufError> {
    let magic = read_u32(reader)?;
    if magic != GGUF_MAGIC {
        return Err(GgufError::InvalidMagic(magic));
    }

    let version = read_u32(reader)?;
    if !SUPPORTED_VERSIONS.contains(&version) {
        return Err(GgufError::UnsupportedVersion(version));
    }

    let tensor_count = read_u64(reader)?;
    let metadata_kv_count = read_u64(reader)?;

    Ok(GgufHeader {
        version,
        tensor_count,
        metadata_kv_count,
    })
}

// --- Binary reading helpers ---

pub(crate) fn read_u8(reader: &mut impl Read) -> Result<u8, GgufError> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(buf[0])
}

pub(crate) fn read_u16(reader: &mut impl Read) -> Result<u16, GgufError> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(u16::from_le_bytes(buf))
}

pub(crate) fn read_u32(reader: &mut impl Read) -> Result<u32, GgufError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(u32::from_le_bytes(buf))
}

pub(crate) fn read_i32(reader: &mut impl Read) -> Result<i32, GgufError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(i32::from_le_bytes(buf))
}

pub(crate) fn read_u64(reader: &mut impl Read) -> Result<u64, GgufError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(u64::from_le_bytes(buf))
}

pub(crate) fn read_i64(reader: &mut impl Read) -> Result<i64, GgufError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(i64::from_le_bytes(buf))
}

pub(crate) fn read_f32(reader: &mut impl Read) -> Result<f32, GgufError> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(f32::from_le_bytes(buf))
}

pub(crate) fn read_f64(reader: &mut impl Read) -> Result<f64, GgufError> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(f64::from_le_bytes(buf))
}

pub(crate) fn read_bool(reader: &mut impl Read) -> Result<bool, GgufError> {
    let v = read_u8(reader)?;
    Ok(v != 0)
}

/// Read a GGUF string: u64 length followed by that many UTF-8 bytes (no null terminator).
pub(crate) fn read_string(reader: &mut impl Read) -> Result<String, GgufError> {
    let len = read_u64(reader)?;
    if len > 1024 * 1024 {
        return Err(GgufError::StringTooLong(len));
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).map_err(|_| GgufError::UnexpectedEof)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}
