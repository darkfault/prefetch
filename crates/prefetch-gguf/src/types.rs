/// GGML tensor data types with their properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GGMLType {
    F32 = 0,
    F16 = 1,
    Q4_0 = 2,
    Q4_1 = 3,
    // Q4_2 = 4, // removed
    // Q4_3 = 5, // removed
    Q5_0 = 6,
    Q5_1 = 7,
    Q8_0 = 8,
    Q8_1 = 9,
    Q2K = 10,
    Q3K = 11,
    Q4K = 12,
    Q5K = 13,
    Q6K = 14,
    Q8K = 15,
    IQ2XXS = 16,
    IQ2XS = 17,
    IQ3XXS = 18,
    IQ1S = 19,
    IQ4NL = 20,
    IQ3S = 21,
    IQ2S = 22,
    IQ4XS = 23,
    I8 = 24,
    I16 = 25,
    I32 = 26,
    I64 = 27,
    F64 = 28,
    IQ1M = 29,
    BF16 = 30,
}

impl GGMLType {
    /// Parse a GGML type from its integer tag.
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::F32),
            1 => Some(Self::F16),
            2 => Some(Self::Q4_0),
            3 => Some(Self::Q4_1),
            6 => Some(Self::Q5_0),
            7 => Some(Self::Q5_1),
            8 => Some(Self::Q8_0),
            9 => Some(Self::Q8_1),
            10 => Some(Self::Q2K),
            11 => Some(Self::Q3K),
            12 => Some(Self::Q4K),
            13 => Some(Self::Q5K),
            14 => Some(Self::Q6K),
            15 => Some(Self::Q8K),
            16 => Some(Self::IQ2XXS),
            17 => Some(Self::IQ2XS),
            18 => Some(Self::IQ3XXS),
            19 => Some(Self::IQ1S),
            20 => Some(Self::IQ4NL),
            21 => Some(Self::IQ3S),
            22 => Some(Self::IQ2S),
            23 => Some(Self::IQ4XS),
            24 => Some(Self::I8),
            25 => Some(Self::I16),
            26 => Some(Self::I32),
            27 => Some(Self::I64),
            28 => Some(Self::F64),
            29 => Some(Self::IQ1M),
            30 => Some(Self::BF16),
            _ => None,
        }
    }

    /// Block size for this type (number of elements per block).
    pub fn block_size(self) -> u64 {
        match self {
            Self::F32 | Self::F16 | Self::BF16 | Self::F64 => 1,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 => 1,
            Self::Q4_0 | Self::Q4_1 | Self::Q5_0 | Self::Q5_1 => 32,
            Self::Q8_0 | Self::Q8_1 => 32,
            Self::Q2K | Self::Q3K | Self::Q4K | Self::Q5K | Self::Q6K | Self::Q8K => 256,
            Self::IQ2XXS | Self::IQ2XS | Self::IQ2S => 256,
            Self::IQ3XXS | Self::IQ3S => 256,
            Self::IQ1S | Self::IQ1M => 256,
            Self::IQ4NL | Self::IQ4XS => 32,
        }
    }

    /// Bytes per block for this type.
    pub fn type_size(self) -> u64 {
        match self {
            Self::F32 => 4,
            Self::F16 => 2,
            Self::BF16 => 2,
            Self::F64 => 8,
            Self::I8 => 1,
            Self::I16 => 2,
            Self::I32 => 4,
            Self::I64 => 8,
            Self::Q4_0 => 18,   // 32 * 4 / 8 + 2 (scale)
            Self::Q4_1 => 20,   // 32 * 4 / 8 + 2 + 2 (scale + min)
            Self::Q5_0 => 22,   // 32 * 5 / 8 + 2
            Self::Q5_1 => 24,   // 32 * 5 / 8 + 2 + 2
            Self::Q8_0 => 34,   // 32 * 8 / 8 + 2
            Self::Q8_1 => 36,   // 32 * 8 / 8 + 2 + 2
            Self::Q2K => 84,
            Self::Q3K => 110,
            Self::Q4K => 144,
            Self::Q5K => 176,
            Self::Q6K => 210,
            Self::Q8K => 292,
            Self::IQ2XXS => 66,
            Self::IQ2XS => 74,
            Self::IQ2S => 82,
            Self::IQ3XXS => 98,
            Self::IQ3S => 110,
            Self::IQ1S => 50,
            Self::IQ1M => 56,
            Self::IQ4NL => 18,
            Self::IQ4XS => 18,
        }
    }

    /// Compute the byte size of a tensor with the given number of elements.
    pub fn tensor_byte_size(self, n_elements: u64) -> u64 {
        self.tensor_byte_size_checked(n_elements).unwrap_or(u64::MAX)
    }

    /// Compute byte size with checked arithmetic. Returns None on overflow.
    pub fn tensor_byte_size_checked(self, n_elements: u64) -> Option<u64> {
        let block_size = self.block_size();
        let n_blocks = n_elements
            .checked_add(block_size - 1)?
            .checked_div(block_size)?;
        n_blocks.checked_mul(self.type_size())
    }
}
