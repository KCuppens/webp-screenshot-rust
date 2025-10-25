//! WebP encoding module

pub mod webp;
pub mod simd;
pub mod gpu;

pub use webp::{WebPEncoder, EncoderOptions};
pub use simd::{SimdConverter, global_simd_converter};

use crate::{
    error::EncodingResult,
    types::{RawImage, WebPConfig},
};

/// Trait for image encoders
pub trait ImageEncoder {
    /// Encode raw image data to a specific format
    fn encode(&self, image: &RawImage, config: &WebPConfig) -> EncodingResult<Vec<u8>>;

    /// Get encoder name
    fn name(&self) -> &str;

    /// Check if encoder supports a specific pixel format
    fn supports_format(&self, format: crate::types::PixelFormat) -> bool;
}

/// Encoder statistics
#[derive(Debug, Clone, Default)]
pub struct EncoderStats {
    /// Total images encoded
    pub images_encoded: u64,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Total bytes output
    pub bytes_output: u64,
    /// Average compression ratio
    pub average_compression_ratio: f64,
    /// Average encoding time in milliseconds
    pub average_encoding_time_ms: f64,
}

impl EncoderStats {
    /// Update statistics with a new encoding
    pub fn update(&mut self, input_size: usize, output_size: usize, time_ms: f64) {
        self.images_encoded += 1;
        self.bytes_processed += input_size as u64;
        self.bytes_output += output_size as u64;

        let compression_ratio = output_size as f64 / input_size as f64;
        self.average_compression_ratio =
            (self.average_compression_ratio * (self.images_encoded - 1) as f64 + compression_ratio)
            / self.images_encoded as f64;

        self.average_encoding_time_ms =
            (self.average_encoding_time_ms * (self.images_encoded - 1) as f64 + time_ms)
            / self.images_encoded as f64;
    }

    /// Get space savings percentage
    pub fn space_savings_percent(&self) -> f64 {
        if self.bytes_processed == 0 {
            0.0
        } else {
            (1.0 - (self.bytes_output as f64 / self.bytes_processed as f64)) * 100.0
        }
    }
}