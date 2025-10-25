//! Core types and structures for screenshot capture and WebP encoding

use std::fmt;
use std::time::{Duration, SystemTime};

/// Information about a display/monitor
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    /// Display index (0-based)
    pub index: usize,
    /// Display name or identifier
    pub name: String,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// X offset from primary display
    pub x: i32,
    /// Y offset from primary display
    pub y: i32,
    /// Scale factor (for HiDPI displays)
    pub scale_factor: f32,
    /// Whether this is the primary display
    pub is_primary: bool,
    /// Refresh rate in Hz
    pub refresh_rate: u32,
    /// Color depth in bits
    pub color_depth: u8,
}

impl Default for DisplayInfo {
    fn default() -> Self {
        Self {
            index: 0,
            name: "Primary Display".to_string(),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            is_primary: true,
            refresh_rate: 60,
            color_depth: 32,
        }
    }
}

impl DisplayInfo {
    /// Get the total pixel count
    pub fn pixel_count(&self) -> u32 {
        self.width * self.height
    }

    /// Get the display bounds as a rectangle
    pub fn bounds(&self) -> Rectangle {
        Rectangle {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

/// Pixel format for raw image data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// Red, Green, Blue, Alpha (8 bits per channel)
    RGBA8,
    /// Blue, Green, Red, Alpha (8 bits per channel)
    BGRA8,
    /// Red, Green, Blue (8 bits per channel)
    RGB8,
    /// Blue, Green, Red (8 bits per channel)
    BGR8,
    /// Grayscale (8 bits)
    Gray8,
    /// Grayscale with alpha (8 bits per channel)
    GrayA8,
}

impl PixelFormat {
    /// Get the number of bytes per pixel
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::RGBA8 | PixelFormat::BGRA8 => 4,
            PixelFormat::RGB8 | PixelFormat::BGR8 => 3,
            PixelFormat::GrayA8 => 2,
            PixelFormat::Gray8 => 1,
        }
    }

    /// Check if the format has an alpha channel
    pub fn has_alpha(&self) -> bool {
        matches!(self, PixelFormat::RGBA8 | PixelFormat::BGRA8 | PixelFormat::GrayA8)
    }

    /// Get the number of color channels
    pub fn channel_count(&self) -> usize {
        match self {
            PixelFormat::RGBA8 | PixelFormat::BGRA8 => 4,
            PixelFormat::RGB8 | PixelFormat::BGR8 => 3,
            PixelFormat::GrayA8 => 2,
            PixelFormat::Gray8 => 1,
        }
    }
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PixelFormat::RGBA8 => write!(f, "RGBA8"),
            PixelFormat::BGRA8 => write!(f, "BGRA8"),
            PixelFormat::RGB8 => write!(f, "RGB8"),
            PixelFormat::BGR8 => write!(f, "BGR8"),
            PixelFormat::Gray8 => write!(f, "Gray8"),
            PixelFormat::GrayA8 => write!(f, "GrayA8"),
        }
    }
}

/// Raw image data container
#[derive(Debug, Clone)]
pub struct RawImage {
    /// Pixel data
    pub data: Vec<u8>,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Pixel format
    pub format: PixelFormat,
    /// Stride (bytes per row, may include padding)
    pub stride: usize,
}

impl RawImage {
    /// Create a new RawImage
    pub fn new(data: Vec<u8>, width: u32, height: u32, format: PixelFormat) -> Self {
        let stride = (width as usize) * format.bytes_per_pixel();
        Self {
            data,
            width,
            height,
            format,
            stride,
        }
    }

    /// Create a new RawImage with custom stride
    pub fn with_stride(
        data: Vec<u8>,
        width: u32,
        height: u32,
        format: PixelFormat,
        stride: usize,
    ) -> Self {
        Self {
            data,
            width,
            height,
            format,
            stride,
        }
    }

    /// Get the total size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get the pixel count
    pub fn pixel_count(&self) -> usize {
        (self.width * self.height) as usize
    }

    /// Check if the image data is valid
    pub fn is_valid(&self) -> bool {
        let expected_size = self.stride * (self.height as usize);
        self.data.len() >= expected_size
    }

    /// Get a pixel at the given coordinates
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<&[u8]> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let offset = (y as usize) * self.stride + (x as usize) * self.format.bytes_per_pixel();
        let pixel_size = self.format.bytes_per_pixel();

        self.data.get(offset..offset + pixel_size)
    }
}

/// WebP encoding configuration
#[derive(Debug, Clone)]
pub struct WebPConfig {
    /// Quality factor (0-100, where 100 is best quality)
    pub quality: u8,
    /// Compression method (0-6, where 0 is fastest, 6 is best compression)
    pub method: u8,
    /// Enable lossless compression
    pub lossless: bool,
    /// Near-lossless encoding quality (0-100, only with lossless=true)
    pub near_lossless: u8,
    /// Number of segments (1-4)
    pub segments: u8,
    /// Spatial noise shaping strength (0-100)
    pub sns_strength: u8,
    /// Filter strength (0-100)
    pub filter_strength: u8,
    /// Filter sharpness (0-7)
    pub filter_sharpness: u8,
    /// Enable auto-filter
    pub auto_filter: bool,
    /// Alpha channel compression (true = compressed, false = uncompressed)
    pub alpha_compression: bool,
    /// Alpha filtering level (0-2)
    pub alpha_filtering: u8,
    /// Alpha quality (0-100)
    pub alpha_quality: u8,
    /// Number of entropy-analysis passes (1-10)
    pub pass: u8,
    /// Thread count (0 = auto-detect)
    pub thread_count: usize,
    /// Enable low memory mode
    pub low_memory: bool,
    /// Preserve RGB values under transparency
    pub exact: bool,
}

impl Default for WebPConfig {
    fn default() -> Self {
        Self {
            quality: 80,
            method: 4,
            lossless: false,
            near_lossless: 100,
            segments: 4,
            sns_strength: 50,
            filter_strength: 60,
            filter_sharpness: 0,
            auto_filter: false,
            alpha_compression: true,
            alpha_filtering: 1,
            alpha_quality: 100,
            pass: 1,
            thread_count: 0,
            low_memory: false,
            exact: false,
        }
    }
}

impl WebPConfig {
    /// Create a high-quality preset
    pub fn high_quality() -> Self {
        Self {
            quality: 95,
            method: 6,
            pass: 10,
            ..Default::default()
        }
    }

    /// Create a fast encoding preset
    pub fn fast() -> Self {
        Self {
            quality: 75,
            method: 0,
            pass: 1,
            ..Default::default()
        }
    }

    /// Create a lossless encoding preset
    pub fn lossless() -> Self {
        Self {
            lossless: true,
            quality: 100,
            method: 6,
            ..Default::default()
        }
    }

    /// Create a balanced preset (good quality/speed tradeoff)
    pub fn balanced() -> Self {
        Self {
            quality: 85,
            method: 4,
            pass: 6,
            ..Default::default()
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.quality > 100 {
            return Err(format!("Quality must be 0-100, got {}", self.quality));
        }
        if self.method > 6 {
            return Err(format!("Method must be 0-6, got {}", self.method));
        }
        if self.segments < 1 || self.segments > 4 {
            return Err(format!("Segments must be 1-4, got {}", self.segments));
        }
        if self.filter_sharpness > 7 {
            return Err(format!(
                "Filter sharpness must be 0-7, got {}",
                self.filter_sharpness
            ));
        }
        if self.alpha_filtering > 2 {
            return Err(format!(
                "Alpha filtering must be 0-2, got {}",
                self.alpha_filtering
            ));
        }
        if self.pass < 1 || self.pass > 10 {
            return Err(format!("Pass must be 1-10, got {}", self.pass));
        }
        Ok(())
    }
}

/// Capture configuration
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// WebP encoding configuration
    pub webp_config: WebPConfig,
    /// Include cursor in capture
    pub include_cursor: bool,
    /// Capture region (None for full display)
    pub region: Option<CaptureRegion>,
    /// Enable hardware acceleration if available
    pub use_hardware_acceleration: bool,
    /// Maximum capture retries
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: Duration,
    /// Capture timeout
    pub timeout: Duration,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            webp_config: WebPConfig::default(),
            include_cursor: false,
            region: None,
            use_hardware_acceleration: true,
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
            timeout: Duration::from_secs(5),
        }
    }
}

/// Capture region specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl CaptureRegion {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn from_rect(rect: Rectangle) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

/// Rectangle structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rectangle {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains_point(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && py >= self.y
            && px < self.x + self.width as i32
            && py < self.y + self.height as i32
    }

    pub fn area(&self) -> u32 {
        self.width * self.height
    }
}

/// Screenshot result with metadata
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// Encoded WebP data
    pub data: Vec<u8>,
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Display index this was captured from
    pub display_index: usize,
    /// Capture metadata
    pub metadata: CaptureMetadata,
}

impl Screenshot {
    /// Get the size of the encoded data
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Save to file
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(path)?;
        file.write_all(&self.data)?;
        Ok(())
    }
}

/// Capture metadata
#[derive(Debug, Clone)]
pub struct CaptureMetadata {
    /// Timestamp of capture
    pub timestamp: SystemTime,
    /// Time taken to capture
    pub capture_duration: Duration,
    /// Time taken to encode
    pub encoding_duration: Duration,
    /// Original uncompressed size
    pub original_size: usize,
    /// Compressed size
    pub compressed_size: usize,
    /// Implementation used
    pub implementation: String,
}

impl CaptureMetadata {
    /// Calculate compression ratio (0.0 - 1.0)
    pub fn compression_ratio(&self) -> f64 {
        if self.original_size == 0 {
            0.0
        } else {
            self.compressed_size as f64 / self.original_size as f64
        }
    }

    /// Get total processing time
    pub fn total_duration(&self) -> Duration {
        self.capture_duration + self.encoding_duration
    }

    /// Get space savings percentage
    pub fn space_savings_percent(&self) -> f64 {
        if self.original_size == 0 {
            0.0
        } else {
            (1.0 - self.compression_ratio()) * 100.0
        }
    }
}

/// Performance statistics
#[derive(Debug, Clone, Default)]
pub struct PerformanceStats {
    pub total_captures: u64,
    pub successful_captures: u64,
    pub failed_captures: u64,
    pub total_bytes_captured: u64,
    pub total_bytes_encoded: u64,
    pub total_capture_time: Duration,
    pub total_encoding_time: Duration,
    pub fastest_capture: Duration,
    pub slowest_capture: Duration,
}

impl PerformanceStats {
    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_captures == 0 {
            0.0
        } else {
            (self.successful_captures as f64 / self.total_captures as f64) * 100.0
        }
    }

    /// Get average capture time
    pub fn average_capture_time(&self) -> Duration {
        if self.successful_captures == 0 {
            Duration::ZERO
        } else {
            self.total_capture_time / self.successful_captures as u32
        }
    }

    /// Get average compression ratio
    pub fn average_compression_ratio(&self) -> f64 {
        if self.total_bytes_captured == 0 {
            0.0
        } else {
            self.total_bytes_encoded as f64 / self.total_bytes_captured as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_format_bytes_per_pixel() {
        assert_eq!(PixelFormat::RGBA8.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::RGB8.bytes_per_pixel(), 3);
        assert_eq!(PixelFormat::Gray8.bytes_per_pixel(), 1);
    }

    #[test]
    fn test_webp_config_validation() {
        let mut config = WebPConfig::default();
        assert!(config.validate().is_ok());

        config.quality = 101;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_raw_image_pixel_access() {
        let data = vec![255; 1920 * 1080 * 4];
        let image = RawImage::new(data, 1920, 1080, PixelFormat::RGBA8);

        let pixel = image.get_pixel(0, 0).unwrap();
        assert_eq!(pixel, &[255, 255, 255, 255]);

        assert!(image.get_pixel(1920, 0).is_none());
    }
}