//! WebP Screenshot Capture Library
//!
//! A high-performance, cross-platform library for capturing screenshots
//! and encoding them as WebP images with minimal overhead.
//!
//! # Example
//!
//! ```no_run
//! use webp_screenshot_rust::{WebPScreenshot, CaptureConfig};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Simple capture
//! let screenshot = WebPScreenshot::new()?;
//! let result = screenshot.capture_display(0)?;
//! result.save("screenshot.webp")?;
//!
//! // With custom configuration
//! let config = CaptureConfig {
//!     include_cursor: true,
//!     ..Default::default()
//! };
//! let screenshot = WebPScreenshot::with_config(config)?;
//! let result = screenshot.capture_display(0)?;
//! # Ok(())
//! # }
//! ```

#![allow(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod capture;
pub mod encoder;
pub mod error;
pub mod memory_pool;
pub mod pipeline;
pub mod types;

#[cfg(feature = "c-api")]
pub mod ffi;

// Re-export main types
pub use capture::{Capturer, ScreenCapture};
pub use encoder::{WebPEncoder, EncoderOptions};
pub use error::{CaptureError, CaptureResult, EncodingError, EncodingResult};
pub use memory_pool::{MemoryPool, PooledBuffer};
pub use pipeline::{StreamingPipeline, StreamingPipelineBuilder, ZeroCopyOptimizer};
pub use types::{
    CaptureConfig, CaptureMetadata, CaptureRegion, DisplayInfo, PerformanceStats, PixelFormat,
    RawImage, Rectangle, Screenshot, WebPConfig,
};

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Main entry point for screenshot capture
pub struct WebPScreenshot {
    capturer: Box<dyn ScreenCapture>,
    encoder: WebPEncoder,
    memory_pool: Arc<MemoryPool>,
    config: CaptureConfig,
    stats: PerformanceStats,
    zero_copy: Option<ZeroCopyOptimizer>,
    gpu_encoder: Option<encoder::gpu::GpuWebPEncoder>,
}

impl WebPScreenshot {
    /// Create a new WebPScreenshot instance with default configuration
    pub fn new() -> CaptureResult<Self> {
        Self::with_config(CaptureConfig::default())
    }

    /// Create a new instance with custom configuration
    pub fn with_config(config: CaptureConfig) -> CaptureResult<Self> {
        let zero_copy = if ZeroCopyOptimizer::is_supported() {
            Some(ZeroCopyOptimizer::new())
        } else {
            None
        };

        let gpu_encoder = if cfg!(feature = "gpu") {
            Some(encoder::gpu::GpuWebPEncoder::new())
        } else {
            None
        };

        Ok(Self {
            capturer: Capturer::new()?,
            encoder: WebPEncoder::new(),
            memory_pool: memory_pool::global_pool(),
            config,
            stats: PerformanceStats::default(),
            zero_copy,
            gpu_encoder,
        })
    }

    /// Get information about available displays
    pub fn get_displays(&self) -> CaptureResult<Vec<DisplayInfo>> {
        self.capturer.get_displays()
    }

    /// Capture a screenshot from a specific display
    pub fn capture_display(&mut self, display_index: usize) -> CaptureResult<Screenshot> {
        eprintln!("[LIB] ========================================");
        eprintln!("[LIB] capture_display called with display_index: {}", display_index);
        eprintln!("[LIB] Config region: {:?}", self.config.region);
        eprintln!("[LIB] ========================================");

        let start_time = Instant::now();
        let timestamp = SystemTime::now();

        // Retry logic
        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                std::thread::sleep(self.config.retry_delay);
                log::debug!("Retry attempt {} for display {}", attempt, display_index);
            }

            match self.capture_display_internal(display_index, timestamp, start_time) {
                Ok(screenshot) => {
                    self.stats.successful_captures += 1;
                    self.stats.total_captures += 1;
                    return Ok(screenshot);
                }
                Err(e) if e.is_recoverable() && attempt < self.config.max_retries => {
                    last_error = Some(e);
                    continue;
                }
                Err(e) => {
                    self.stats.failed_captures += 1;
                    self.stats.total_captures += 1;
                    return Err(e);
                }
            }
        }

        self.stats.failed_captures += 1;
        self.stats.total_captures += 1;
        Err(last_error.unwrap_or_else(|| {
            CaptureError::CaptureFailed("Max retries exceeded".to_string())
        }))
    }

    /// Internal capture implementation
    fn capture_display_internal(
        &mut self,
        display_index: usize,
        timestamp: SystemTime,
        start_time: Instant,
    ) -> CaptureResult<Screenshot> {
        // Capture raw image
        let capture_start = Instant::now();

        let raw_image = if let Some(ref zero_copy) = self.zero_copy {
            // Disable zero-copy when capturing a specific region
            // Zero-copy is optimized for full-screen captures, not regions
            if zero_copy.is_enabled() && self.config.region.is_none() {
                eprintln!("[LIB] Using ZERO-COPY optimization path (full screen)");
                zero_copy.capture_zero_copy(&*self.capturer, display_index)?
            } else {
                if self.config.region.is_some() {
                    eprintln!("[LIB] Region set - disabling zero-copy, using normal GDI path");
                } else {
                    eprintln!("[LIB] Zero-copy available but disabled, using normal path");
                }
                self.capture_normal(display_index)?
            }
        } else {
            eprintln!("[LIB] No zero-copy, using normal capture path");
            self.capture_normal(display_index)?
        };

        let capture_duration = capture_start.elapsed();

        // Update stats
        self.stats.total_bytes_captured += raw_image.size() as u64;
        self.stats.total_capture_time += capture_duration;

        // Encode to WebP
        let encoding_start = Instant::now();

        let webp_data = if let Some(ref gpu_encoder) = self.gpu_encoder {
            if gpu_encoder.is_available() && gpu_encoder.is_size_suitable(raw_image.width, raw_image.height) {
                gpu_encoder.encode(&raw_image, &self.config.webp_config)?
            } else {
                self.encoder.encode(&raw_image, &self.config.webp_config)
                    .map_err(|e| CaptureError::Other(e.into()))?
            }
        } else {
            self.encoder.encode(&raw_image, &self.config.webp_config)
                .map_err(|e| CaptureError::Other(e.into()))?
        };

        let encoding_duration = encoding_start.elapsed();

        // Update stats
        self.stats.total_bytes_encoded += webp_data.len() as u64;
        self.stats.total_encoding_time += encoding_duration;

        // Update timing records
        let total_duration = start_time.elapsed();
        if self.stats.fastest_capture == Duration::ZERO
            || total_duration < self.stats.fastest_capture
        {
            self.stats.fastest_capture = total_duration;
        }
        if total_duration > self.stats.slowest_capture {
            self.stats.slowest_capture = total_duration;
        }

        // Build metadata
        let metadata = CaptureMetadata {
            timestamp,
            capture_duration,
            encoding_duration,
            original_size: raw_image.size(),
            compressed_size: webp_data.len(),
            implementation: self.capturer.implementation_name(),
        };

        Ok(Screenshot {
            data: webp_data,
            width: raw_image.width,
            height: raw_image.height,
            display_index,
            metadata,
        })
    }

    /// Normal capture without zero-copy
    fn capture_normal(&self, display_index: usize) -> CaptureResult<RawImage> {
        if let Some(region) = self.config.region {
            eprintln!("[LIB] capture_normal: Using region mode - {:?}", region);
            eprintln!("[LIB] Calling capturer.capture_region()");
            self.capturer.capture_region(region)
        } else {
            eprintln!("[LIB] capture_normal: Using display mode - display {}", display_index);
            eprintln!("[LIB] Calling capturer.capture_display()");
            self.capturer.capture_display(display_index)
        }
    }

    /// Capture screenshots from all available displays
    pub fn capture_all_displays(&mut self) -> Vec<CaptureResult<Screenshot>> {
        match self.get_displays() {
            Ok(displays) => displays
                .iter()
                .enumerate()
                .map(|(index, _)| self.capture_display(index))
                .collect(),
            Err(e) => vec![Err(e)],
        }
    }

    /// Capture with a custom encoder configuration
    pub fn capture_with_config(
        &mut self,
        display_index: usize,
        webp_config: WebPConfig,
    ) -> CaptureResult<Screenshot> {
        let original_config = self.config.webp_config.clone();
        self.config.webp_config = webp_config;
        let result = self.capture_display(display_index);
        self.config.webp_config = original_config;
        result
    }

    /// Create a streaming pipeline for continuous capture
    pub fn create_streaming_pipeline(&self) -> StreamingPipelineBuilder {
        StreamingPipelineBuilder::new()
    }

    /// Set the capture configuration
    pub fn set_config(&mut self, config: CaptureConfig) {
        self.config = config;
    }

    /// Get the current capture configuration
    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }

    /// Get performance statistics
    pub fn stats(&self) -> &PerformanceStats {
        &self.stats
    }

    /// Reset performance statistics
    pub fn reset_stats(&mut self) {
        self.stats = PerformanceStats::default();
    }

    /// Get memory pool statistics
    pub fn memory_stats(&self) -> memory_pool::PoolStats {
        self.memory_pool.stats()
    }

    /// Get zero-copy statistics
    pub fn zero_copy_stats(&self) -> Option<pipeline::zero_copy::ZeroCopyStats> {
        self.zero_copy.as_ref().map(|zc| zc.stats())
    }

    /// Get the implementation name
    pub fn implementation_name(&self) -> String {
        self.capturer.implementation_name()
    }

    /// Check if hardware acceleration is available
    pub fn is_hardware_accelerated(&self) -> bool {
        self.capturer.is_hardware_accelerated()
    }

    /// Get GPU encoder information
    pub fn gpu_info(&self) -> Option<String> {
        self.gpu_encoder.as_ref().map(|gpu| {
            format!(
                "{} - {}",
                gpu.backend_name(),
                gpu.device_info().unwrap_or_else(|| "Unknown".to_string())
            )
        })
    }
}

impl Default for WebPScreenshot {
    fn default() -> Self {
        Self::new().expect("Failed to initialize WebPScreenshot")
    }
}

/// Convenience function to capture the primary display
pub fn capture_primary_display() -> CaptureResult<Screenshot> {
    let mut screenshot = WebPScreenshot::new()?;
    screenshot.capture_display(0)
}

/// Convenience function to capture with specific quality
pub fn capture_with_quality(display_index: usize, quality: u8) -> CaptureResult<Screenshot> {
    let config = CaptureConfig {
        webp_config: WebPConfig {
            quality,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut screenshot = WebPScreenshot::with_config(config)?;
    screenshot.capture_display(display_index)
}

/// Get available displays
pub fn get_displays() -> CaptureResult<Vec<DisplayInfo>> {
    let capturer = Capturer::new()?;
    capturer.get_displays()
}

/// Library version information
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Get library capabilities
pub fn capabilities() -> String {
    let mut caps = Vec::new();

    // Platform
    #[cfg(target_os = "windows")]
    caps.push("Windows");
    #[cfg(target_os = "macos")]
    caps.push("macOS");
    #[cfg(target_os = "linux")]
    caps.push("Linux");

    // SIMD
    let simd_caps = encoder::simd::global_simd_converter().capabilities();
    if !simd_caps.is_empty() && simd_caps != "None (scalar)" {
        caps.push(&simd_caps);
    }

    // Features
    if ZeroCopyOptimizer::is_supported() {
        caps.push("Zero-Copy");
    }

    #[cfg(feature = "gpu")]
    caps.push("GPU");

    #[cfg(feature = "parallel")]
    caps.push("Parallel");

    caps.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = version();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_capabilities() {
        let caps = capabilities();
        println!("Library capabilities: {}", caps);
        assert!(!caps.is_empty());
    }

    #[test]
    fn test_screenshot_creation() {
        // This test might fail on systems without display access
        let result = WebPScreenshot::new();
        // Just check that creation doesn't panic
        drop(result);
    }

    #[test]
    fn test_config_creation() {
        let config = CaptureConfig::default();
        assert_eq!(config.webp_config.quality, 80);
        assert!(!config.include_cursor);
    }
}