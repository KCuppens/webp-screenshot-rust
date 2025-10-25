//! WebP encoder implementation using libwebp

use crate::{
    encoder::{EncoderStats, ImageEncoder},
    error::{EncodingError, EncodingResult},
    types::{PixelFormat, RawImage, WebPConfig},
};

use std::time::Instant;

/// Re-export encoder options
pub use crate::types::WebPConfig as EncoderOptions;

/// WebP encoder using the webp crate
pub struct WebPEncoder {
    stats: EncoderStats,
}

impl Default for WebPEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl WebPEncoder {
    /// Create a new WebP encoder
    pub fn new() -> Self {
        Self {
            stats: EncoderStats::default(),
        }
    }

    /// Get encoder statistics
    pub fn stats(&self) -> &EncoderStats {
        &self.stats
    }

    /// Encode an image to WebP format
    pub fn encode(&mut self, image: &RawImage, config: &WebPConfig) -> EncodingResult<Vec<u8>> {
        let start_time = Instant::now();

        // Validate configuration
        config.validate()
            .map_err(|e| EncodingError::InvalidConfiguration(e))?;

        // Validate image dimensions
        if image.width == 0 || image.height == 0 {
            return Err(EncodingError::InvalidDimensions {
                width: image.width,
                height: image.height,
            });
        }

        // Encode based on pixel format
        let result = self.encode_with_format(image, config)?;

        // Update statistics
        let encoding_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
        self.stats.update(image.size(), result.len(), encoding_time_ms);

        Ok(result)
    }

    /// Encode with specific pixel format handling
    fn encode_with_format(&self, image: &RawImage, config: &WebPConfig) -> EncodingResult<Vec<u8>> {
        // Convert to appropriate format and encode
        let encoded = match image.format {
            PixelFormat::RGBA8 => {
                let encoder = webp::Encoder::from_rgba(&image.data, image.width, image.height);
                if config.lossless {
                    encoder.encode_lossless()
                } else {
                    encoder.encode(config.quality as f32)
                }
            }
            PixelFormat::RGB8 => {
                let encoder = webp::Encoder::from_rgb(&image.data, image.width, image.height);
                if config.lossless {
                    encoder.encode_lossless()
                } else {
                    encoder.encode(config.quality as f32)
                }
            }
            PixelFormat::BGRA8 => {
                // Convert BGRA to RGBA first
                let mut rgba_data = image.data.clone();
                self.convert_bgra_to_rgba_inplace(&mut rgba_data);
                let encoder = webp::Encoder::from_rgba(&rgba_data, image.width, image.height);
                if config.lossless {
                    encoder.encode_lossless()
                } else {
                    encoder.encode(config.quality as f32)
                }
            }
            PixelFormat::BGR8 => {
                // Convert BGR to RGB first
                let mut rgb_data = image.data.clone();
                self.convert_bgr_to_rgb_inplace(&mut rgb_data);
                let encoder = webp::Encoder::from_rgb(&rgb_data, image.width, image.height);
                if config.lossless {
                    encoder.encode_lossless()
                } else {
                    encoder.encode(config.quality as f32)
                }
            }
            _ => {
                return Err(EncodingError::UnsupportedFormat(
                    format!("Unsupported pixel format: {:?}", image.format)
                ));
            }
        };

        Ok(encoded.to_vec())
    }

    /// Convert BGRA to RGBA in-place
    fn convert_bgra_to_rgba_inplace(&self, data: &mut [u8]) {
        for chunk in data.chunks_exact_mut(4) {
            chunk.swap(0, 2); // Swap B and R
        }
    }

    /// Convert BGR to RGB in-place
    fn convert_bgr_to_rgb_inplace(&self, data: &mut [u8]) {
        for chunk in data.chunks_exact_mut(3) {
            chunk.swap(0, 2); // Swap B and R
        }
    }

    /// Check if the encoder supports a specific format
    pub fn supports_format(&self, format: PixelFormat) -> bool {
        matches!(
            format,
            PixelFormat::RGBA8 | PixelFormat::RGB8 | PixelFormat::BGRA8 | PixelFormat::BGR8
        )
    }

    /// Get encoder capabilities
    pub fn capabilities(&self) -> Vec<&'static str> {
        vec![
            "WebP encoding",
            "Lossy compression",
            "Lossless compression",
            "RGBA/RGB support",
            "BGRA/BGR conversion",
            "Quality control",
        ]
    }
}

impl ImageEncoder for WebPEncoder {
    fn encode(&self, image: &RawImage, config: &WebPConfig) -> EncodingResult<Vec<u8>> {
        // Use the internal encode_with_format method
        self.encode_with_format(image, config)
    }

    fn name(&self) -> &str {
        "WebPEncoder"
    }

    fn supports_format(&self, format: PixelFormat) -> bool {
        self.supports_format(format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        let encoder = WebPEncoder::new();
        assert_eq!(encoder.stats.images_encoded, 0);
    }

    #[test]
    fn test_format_support() {
        let encoder = WebPEncoder::new();
        assert!(encoder.supports_format(PixelFormat::RGBA8));
        assert!(encoder.supports_format(PixelFormat::RGB8));
        assert!(encoder.supports_format(PixelFormat::BGRA8));
        assert!(encoder.supports_format(PixelFormat::BGR8));
    }

    #[test]
    fn test_config_validation() {
        let mut config = WebPConfig::default();
        assert!(config.validate().is_ok());

        config.quality = 101.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_encode_rgba() {
        let encoder = WebPEncoder::new();
        let test_image = RawImage {
            data: vec![255u8; 100 * 100 * 4],
            width: 100,
            height: 100,
            format: PixelFormat::RGBA8,
        };

        let config = WebPConfig::default();
        let result = encoder.encode(&test_image, &config);
        assert!(result.is_ok());

        let webp_data = result.unwrap();
        assert!(!webp_data.is_empty());
    }
}