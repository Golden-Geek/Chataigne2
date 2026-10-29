use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Marker columns in source-normalized coordinates.
pub const MARKER_COLUMNS: usize = 8;
/// Marker rows in source-normalized coordinates.
pub const MARKER_ROWS: usize = 2;
/// Six packed words occupy sixteen twelve-bit opaque RGB cells.
pub const MARKER_CELLS: usize = MARKER_COLUMNS * MARKER_ROWS;
/// Top-left marker location in source-normalized coordinates.
pub const MARKER_ORIGIN: [f32; 2] = [0.04, 0.04];
/// Complete marker size in source-normalized coordinates.
pub const MARKER_EXTENT: [f32; 2] = [0.4, 0.16];
const CHECKSUM_SALT: u32 = 0x4756_4d31;

/// Fixture identity and timing encoded through the actual image, independently of SDK
/// acceptance counters. QPC is a producer CPU stamp, never a GPU or scanout timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardStamp {
    /// Stable logical source identity within the qualification scene.
    pub source: u32,
    /// Monotonically increasing fixture frame identity, without truncation to 32 bits.
    pub frame: u64,
    /// Machine-wide QueryPerformanceCounter coordinate captured by the producer.
    pub qpc: u64,
}

impl CardStamp {
    /// Packs the source/frame/QPC words and checksum used by both GPU paths.
    pub fn words(self) -> [u32; 6] {
        let mut words = [
            self.source,
            self.frame as u32,
            (self.frame >> 32) as u32,
            self.qpc as u32,
            (self.qpc >> 32) as u32,
            0,
        ];
        words[5] = words[..5].iter().fold(CHECKSUM_SALT, |checksum, word| checksum ^ word);
        words
    }

    /// Validates raw GPU-decoded words without inferring source identity or clock order.
    pub fn from_words(words: [u32; 6]) -> Result<Self, MarkerError> {
        if words[..5].iter().fold(CHECKSUM_SALT, |checksum, word| checksum ^ word) != words[5] {
            return Err(MarkerError::Checksum);
        }
        Ok(Self {
            source: words[0],
            frame: u64::from(words[1]) | (u64::from(words[2]) << 32),
            qpc: u64::from(words[3]) | (u64::from(words[4]) << 32),
        })
    }
}

/// Corrupt or incorrectly sampled marker data. Invalid observations are not silently
/// clamped or included in successful frame-age distributions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum MarkerError {
    /// A decoded cell contains bits outside its twelve-bit payload.
    #[error("marker cell {index} exceeds twelve bits")]
    InvalidCell {
        /// Zero-based cell containing invalid bits.
        index: usize,
    },
    /// The six reconstructed words do not satisfy the marker checksum.
    #[error("marker checksum does not match")]
    Checksum,
}

/// Encodes six little-endian words as a contiguous least-significant-bit-first stream of
/// twelve-bit cells. Each cell maps its low/middle/high nibble to R/G/B at level/15, alpha one.
pub fn encode_marker(stamp: CardStamp) -> [u16; MARKER_CELLS] {
    let words = stamp.words();
    std::array::from_fn(|cell| {
        let bit = cell * 12;
        let word = bit / 32;
        let shift = bit % 32;
        let mut value = words[word] >> shift;
        if shift > 20 {
            value |= words[word + 1] << (32 - shift);
        }
        (value & 0xfff) as u16
    })
}

/// Reconstructs and validates the exact image marker. Callers separately validate expected
/// source identity, frame order, QPC range and repetition against their sample window.
pub fn decode_marker(cells: [u16; MARKER_CELLS]) -> Result<CardStamp, MarkerError> {
    let mut words = [0_u32; 6];
    for (cell, value) in cells.into_iter().enumerate() {
        if value > 0xfff {
            return Err(MarkerError::InvalidCell { index: cell });
        }
        for offset in 0..12 {
            let bit = cell * 12 + offset;
            words[bit / 32] |= u32::from((value >> offset) & 1) << (bit % 32);
        }
    }
    CardStamp::from_words(words)
}

/// Shared HLSL helpers for producer encoding and consumer decoding. Geometry constants are
/// emitted from the Rust contract above, so both GPU paths sample the same cell centers.
/// The caller appends its own shader entry points and cbuffer/resource declarations.
pub fn marker_hlsl() -> String {
    format!(
        "static const uint MARKER_COLUMNS = {MARKER_COLUMNS};\n\
         static const uint MARKER_ROWS = {MARKER_ROWS};\n\
         static const float2 MARKER_ORIGIN = float2({:.9}, {:.9});\n\
         static const float2 MARKER_EXTENT = float2({:.9}, {:.9});\n{}",
        MARKER_ORIGIN[0], MARKER_ORIGIN[1], MARKER_EXTENT[0], MARKER_EXTENT[1], HLSL,
    )
}

const HLSL: &str = r#"
uint MarkerWord(uint index, uint4 first, uint4 second) {
    return index < 4 ? first[index] : second[index - 4];
}
uint MarkerCell(uint cell, uint4 first, uint4 second) {
    uint bit = cell * 12;
    uint word = bit / 32;
    uint shift = bit % 32;
    uint value = MarkerWord(word, first, second) >> shift;
    if (shift > 20) value |= MarkerWord(word + 1, first, second) << (32 - shift);
    return value & 0xfff;
}
float3 MarkerRgb(uint cell, uint4 first, uint4 second) {
    uint value = MarkerCell(cell, first, second);
    return float3(value & 15, (value >> 4) & 15, (value >> 8) & 15) / 15.0;
}
uint MarkerCellFromRgb(float3 rgb) {
    uint3 nibble = (uint3)round(saturate(rgb) * 15.0);
    return nibble.x | (nibble.y << 4) | (nibble.z << 8);
}
float2 MarkerCellCenter(uint cell) {
    float2 grid = float2(cell % MARKER_COLUMNS, cell / MARKER_COLUMNS);
    return MARKER_ORIGIN + (grid + 0.5) * MARKER_EXTENT / float2(MARKER_COLUMNS, MARKER_ROWS);
}
uint MarkerDecodedWord(uint word, uint cells[16]) {
    uint value = 0;
    [unroll] for (uint bit = 0; bit < 32; bit++) {
        uint position = word * 32 + bit;
        value |= ((cells[position / 12] >> (position % 12)) & 1) << bit;
    }
    return value;
}
void MarkerRead(Texture2D<float4> image, float2 sourceOrigin, float2 sourceExtent, out uint words[6]) {
    uint width, height;
    image.GetDimensions(width, height);
    uint cells[16];
    [unroll] for (uint cell = 0; cell < 16; cell++) {
        float2 uv = sourceOrigin + MarkerCellCenter(cell) * sourceExtent;
        uint2 coordinate = min((uint2)(uv * float2(width, height)), uint2(width - 1, height - 1));
        cells[cell] = MarkerCellFromRgb(image.Load(int3(coordinate, 0)).rgb);
    }
    [unroll] for (uint word = 0; word < 6; word++) words[word] = MarkerDecodedWord(word, cells);
}
"#;
